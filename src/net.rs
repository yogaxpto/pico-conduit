//! Wi-Fi connectivity, TCP socket server, LED task, provisioning, and flash storage.
#![allow(clippy::future_not_send)]
//!
//! Embedded-only module (not compiled during `cargo test --lib`).
//!
//! ## Architecture
//!
//! `CONTROL_MUTEX` owns the `cyw43::Control` handle after CYW43 init.  The mutex is held only
//! for the duration of individual async HAL calls (`gpio_set`, join, scan, …) and released
//! immediately after each call — never held across a `Timer::after_*`.
//!
//! ### Startup sequence
//!
//! 1. `start()` — factory-reset check via GPIO23 (Flex), CYW43 init, store control in mutex.
//! 2. Spawn `led_task` (drives LED via `CONTROL_MUTEX`).
//! 3. STA mode if credentials found; AP provisioning mode otherwise.
//!
//! ### AP provisioning
//!
//! AP mode → DHCP server (UDP 67) + HTTP captive portal (TCP 80) → credential test
//! → `save_credentials_flash` → watchdog reboot.

use core::fmt::Write as _;

use cyw43::{JoinOptions, ScanOptions};
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{
    Config, DhcpConfig, IpListenEndpoint, Ipv4Address, Ipv4Cidr, Stack, StackResources,
    StaticConfigV4,
};
use embassy_rp::flash::{Blocking, Flash};
use embassy_rp::gpio::{Flex, Level, Output, Pull};
use embassy_rp::pio::Pio;
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer, with_timeout};
use embedded_io_async::Write as _;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use static_cell::StaticCell;

#[cfg(feature = "transport-tcp")]
use pico_conduit::board::TCP_PORT;
use pico_conduit::board::{CRED_FLASH_OFFSET, FLASH_SIZE};
use pico_conduit::led::{LED_SIGNAL, LedPattern, LedState};
#[cfg(feature = "transport-tcp")]
use pico_conduit::protocol::FrameReader;
use pico_conduit::protocol::MAX_MSG_LEN;
#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
use pico_conduit::protocol::{parse_command, serialize_response};
use pico_conduit::provisioning::portal::{
    Method, decode_url_encoded, make_ap_ssid, parse_connect_form, parse_request_line,
};
use pico_conduit::provisioning::storage::Credentials;
#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
use pico_conduit::router::{DeviceState, dispatch, validate_route};
#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
use pico_conduit::transport::{Transport, TransportError};

// ── CYW43 firmware blobs ──────────────────────────────────────────────────────
const CYW43_FW: &cyw43::Aligned<cyw43::A4, [u8]> =
    cyw43::aligned_bytes!("../cyw43-firmware/43439A0.bin");
const CYW43_CLM: &[u8] = include_bytes!("../cyw43-firmware/43439A0_clm.bin");
const CYW43_NVRAM: &cyw43::Aligned<cyw43::A4, [u8]> =
    cyw43::aligned_bytes!("../cyw43-firmware/nvram_rp2040.bin");

// ── AP network constants ──────────────────────────────────────────────────────
/// Gateway / DHCP server address for the AP captive-portal network.
const AP_IP: [u8; 4] = [192, 168, 4, 1];
/// DHCP-assigned client address (always the same; single-client AP).
const DHCP_CLIENT_IP: [u8; 4] = [192, 168, 4, 2];
/// Subnet mask for the /24 AP network.
const SUBNET_MASK: [u8; 4] = [255, 255, 255, 0];
/// AP gateway IP as a byte-string for HTTP host-header matching.
const AP_IP_STR: &[u8] = b"192.168.4.1";
/// Redirect target for the captive-portal.
const AP_IP_URL: &[u8] = b"http://192.168.4.1/";
#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
const TCP_READ_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
const MAX_RECONNECT_SECS: u16 = 600; // 10 minutes

// ── Flash credential storage constants ───────────────────────────────────────
/// Magic sentinel v2 — includes MQTT broker fields.
/// Old v1 magic (`0xC0FF_EE42`) is treated as missing credentials (re-provision).
const CRED_MAGIC: u32 = 0xC0FF_EE43;
/// Record layout v2: magic(4) + `ssid_len(1)` + `pwd_len(1)` + ssid(32) + pwd(64)
///   + `mqtt_host_len(1)` + `mqtt_host(64)` + `mqtt_port(2)` = 169 bytes.
const CRED_RECORD_SIZE: usize = 169;

// ── TcpTransport ─────────────────────────────────────────────────────────────

/// TCP transport — wraps a `TcpSocket` and uses `FrameReader` for newline-delimited framing.
#[cfg(feature = "transport-tcp")]
struct TcpTransport<'a, 'b> {
    socket: &'a mut TcpSocket<'b>,
    frame_reader: FrameReader,
}

#[cfg(feature = "transport-tcp")]
impl<'a, 'b> TcpTransport<'a, 'b> {
    const fn new(socket: &'a mut TcpSocket<'b>) -> Self {
        Self {
            socket,
            frame_reader: FrameReader::new(),
        }
    }
}

#[cfg(feature = "transport-tcp")]
impl Transport for TcpTransport<'_, '_> {
    async fn read_frame<'c>(&mut self, buf: &'c mut [u8]) -> Result<&'c [u8], TransportError> {
        let mut byte_buf = [0u8; 1];
        loop {
            let read_result = with_timeout(TCP_READ_TIMEOUT, self.socket.read(&mut byte_buf)).await;
            match read_result {
                Err(_) => {
                    defmt::warn!("TCP read timeout — closing idle connection");
                    self.socket.abort();
                    return Err(TransportError::Timeout);
                }
                Ok(Err(e)) => {
                    defmt::warn!("TCP read error: {:?}", e);
                    self.socket.abort();
                    return Err(TransportError::Disconnected);
                }
                Ok(Ok(0)) => {
                    self.socket.abort();
                    return Err(TransportError::Disconnected);
                }
                Ok(Ok(_)) => {}
            }

            match self.frame_reader.push(byte_buf[0]) {
                Err(err_code) => {
                    self.frame_reader.reset();
                    return Err(TransportError::Protocol(err_code));
                }
                Ok(None) => {} // keep reading
                Ok(Some(frame)) => {
                    let len = frame.len();
                    buf[..len].copy_from_slice(frame);
                    return Ok(&buf[..len]);
                }
            }
        }
    }

    async fn write_frame(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if self.socket.write_all(data).await.is_err() {
            defmt::warn!("TCP write error");
            self.socket.abort();
            return Err(TransportError::Disconnected);
        }
        Ok(())
    }
}

// ── Platform validation ──────────────────────────────────────────────────────

/// Verify the firmware is running on the correct chip.
///
/// Reads the SYSINFO `CHIP_ID` register and panics on mismatch to prevent silent
/// malfunction if firmware built for one board is flashed to the other.
fn validate_platform() {
    let chip_id = embassy_rp::pac::SYSINFO.chip_id().read();
    let part = chip_id.part();

    if let Err(msg) = pico_conduit::board::validate_chip_part(part) {
        defmt::panic!(
            "{}: expected PART={=u16:#x}, got PART={=u16:#x}",
            msg,
            pico_conduit::board::EXPECTED_CHIP_PART,
            part
        );
    }
    defmt::info!("platform validated: PART={=u16:#x}", part);
}

// ── Static storage (no heap) ──────────────────────────────────────────────────
static STACK_RESOURCES_STA: StaticCell<StackResources<4>> = StaticCell::new();
static STACK_RESOURCES_AP: StaticCell<StackResources<4>> = StaticCell::new();
static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();
static FLASH_CELL: StaticCell<CredFlash> = StaticCell::new();

// ── Shared CYW43 control ──────────────────────────────────────────────────────
/// Newtype that makes `cyw43::Control<'static>` usable as a `static` inside `Mutex`.
///
/// # Safety
///
/// `cyw43::Control` contains `Cell` and `RefCell` which are not `Sync`.  This is safe on a
/// single-core RP2040/RP2350 running Embassy's cooperative scheduler because:
/// 1. Only one async task runs at a time (cooperative, not preemptive).
/// 2. `CriticalSectionRawMutex` disables IRQs, so no concurrent access is possible.
struct ControlWrapper(cyw43::Control<'static>);
// SAFETY: see above — single-core cooperative scheduling + CS mutex.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for ControlWrapper {}
unsafe impl Sync for ControlWrapper {}
impl core::ops::Deref for ControlWrapper {
    type Target = cyw43::Control<'static>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for ControlWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Global CYW43 control handle.  Populated in `start()` before any task uses it.
static CONTROL_MUTEX: Mutex<CriticalSectionRawMutex, Option<ControlWrapper>> = Mutex::new(None);

// ── Interrupt bindings ────────────────────────────────────────────────────────
embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

// ── Task type aliases ─────────────────────────────────────────────────────────
type CywSpi = cyw43_pio::PioSpi<'static, embassy_rp::peripherals::PIO0, 0>;
/// GPIO23 doubles as the CYW43 `WL_ON` line.  We first sample it as `Flex` (factory-reset check),
/// then reconfigure as output and pass to the cyw43 driver.
type CywRunner = cyw43::Runner<'static, cyw43::SpiBus<Flex<'static>, CywSpi>>;
type CredFlash = Flash<'static, embassy_rp::peripherals::FLASH, Blocking, FLASH_SIZE>;

// ── Background tasks ──────────────────────────────────────────────────────────

/// CYW43 driver runner — must be spawned before any Wi-Fi operation.
#[embassy_executor::task]
async fn cyw43_task(runner: CywRunner) -> ! {
    runner.run().await
}

/// Embassy-net stack runner (STA mode).
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

/// Embassy-net stack runner (AP mode — separate task slot from `net_task`).
#[embassy_executor::task]
async fn net_task_ap(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

// ── LED helpers ───────────────────────────────────────────────────────────────

/// Drive the LED (CYW43 GPIO0) on or off.  Holds `CONTROL_MUTEX` for the async HAL call only.
async fn set_led(on: bool) {
    let mut ctrl = CONTROL_MUTEX.lock().await;
    if let Some(c) = ctrl.as_mut() {
        c.gpio_set(0, on).await;
    }
}

/// LED task — generic pattern runner driven by [`LED_SIGNAL`] state changes.
#[embassy_executor::task]
pub async fn led_task() {
    loop {
        let state = LED_SIGNAL.wait().await;
        match state.pattern() {
            LedPattern::Solid(on) => {
                set_led(on).await;
            }
            LedPattern::OneShot(steps) => {
                for &(on, ms) in steps {
                    set_led(on).await;
                    Timer::after_millis(u64::from(ms)).await;
                }
                set_led(false).await;
            }
            LedPattern::Repeat(steps) => loop {
                for &(on, ms) in steps {
                    set_led(on).await;
                    Timer::after_millis(u64::from(ms)).await;
                }
                if LED_SIGNAL.signaled() {
                    break;
                }
            },
        }
    }
}

// ── Flash credential storage ──────────────────────────────────────────────────

/// Load credentials from the CREDENTIALS flash region.
///
/// Returns `None` if the magic number is absent (blank flash) or the record is corrupt.
fn load_credentials_flash(flash: &mut CredFlash) -> Option<Credentials> {
    let mut buf = [0u8; CRED_RECORD_SIZE];
    flash.read(CRED_FLASH_OFFSET, &mut buf).ok()?;

    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != CRED_MAGIC {
        // Old v1 magic (0xC0FF_EE42) or blank flash — treat as missing
        return None;
    }

    let ssid_len = buf[4] as usize;
    let pwd_len = buf[5] as usize;
    if ssid_len > 32 || pwd_len > 64 {
        return None;
    }

    let ssid = core::str::from_utf8(&buf[6..6 + ssid_len]).ok()?;
    let pwd = core::str::from_utf8(&buf[38..38 + pwd_len]).ok()?;

    // v2 fields: mqtt_host_len at offset 102, mqtt_host at 103, mqtt_port at 167
    let mqtt_host_len = buf[102] as usize;
    if mqtt_host_len > 64 {
        return None;
    }
    let mqtt_host = core::str::from_utf8(&buf[103..103 + mqtt_host_len]).ok()?;
    let mqtt_port = u16::from_le_bytes([buf[167], buf[168]]);

    Credentials::with_mqtt(ssid, pwd, mqtt_host, mqtt_port)
}

/// Erase the sector and write a credential record to the CREDENTIALS flash region.
///
/// Returns `true` on success.
fn save_credentials_flash(flash: &mut CredFlash, creds: &Credentials) -> bool {
    if flash
        .erase(CRED_FLASH_OFFSET, CRED_FLASH_OFFSET + 4096)
        .is_err()
    {
        return false;
    }

    let mut buf = [0xFFu8; CRED_RECORD_SIZE];
    buf[0..4].copy_from_slice(&CRED_MAGIC.to_le_bytes());
    let ssid_b = creds.ssid.as_bytes();
    let pwd_b = creds.password.as_bytes();
    #[allow(clippy::cast_possible_truncation)] // lengths bounded by heapless::String capacity
    {
        buf[4] = ssid_b.len() as u8;
        buf[5] = pwd_b.len() as u8;
    }
    buf[6..6 + ssid_b.len()].copy_from_slice(ssid_b);
    buf[38..38 + pwd_b.len()].copy_from_slice(pwd_b);

    // v2 MQTT fields
    let mqtt_host_b = creds.mqtt_host.as_bytes();
    #[allow(clippy::cast_possible_truncation)] // bounded by heapless::String capacity
    let mqtt_host_len = mqtt_host_b.len() as u8;
    buf[102] = mqtt_host_len;
    buf[103..103 + mqtt_host_b.len()].copy_from_slice(mqtt_host_b);
    buf[167..169].copy_from_slice(&creds.mqtt_port.to_le_bytes());

    flash.write(CRED_FLASH_OFFSET, &buf).is_ok()
}

/// Erase the CREDENTIALS flash region (factory reset).
fn erase_credentials_flash(flash: &mut CredFlash) {
    let _ = flash.erase(CRED_FLASH_OFFSET, CRED_FLASH_OFFSET + 4096);
}

// ── Factory reset check ───────────────────────────────────────────────────────

/// Check if GPIO23 is held low for 5 s after power-on.
///
/// Per OBJECTIVE Phase 6f: GPIO23 is specified as the BOOTSEL pin (active low) on RP2350.
/// Note that on Pico 2W hardware this pin is also the CYW43 `WL_ON` line; the check is
/// implemented as specified regardless.
async fn check_factory_reset(pin: &mut Flex<'_>) -> bool {
    pin.set_as_input();
    pin.set_pull(Pull::Up);
    Timer::after_millis(10).await; // settle

    if pin.is_high() {
        return false;
    }

    let mut held_ms: u16 = 0;
    while held_ms < 5000 {
        Timer::after_millis(100).await;
        if pin.is_high() {
            return false;
        }
        held_ms += 100;
    }
    true
}

// ── Reconnect backoff ─────────────────────────────────────────────────────────

/// Exponential backoff: 5s → 10s → 20s → 40s → 60s (capped at 60s).
#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
const fn backoff_duration(attempt: u8) -> Duration {
    match attempt {
        0 => Duration::from_secs(5),
        1 => Duration::from_secs(10),
        2 => Duration::from_secs(20),
        3 => Duration::from_secs(40),
        _ => Duration::from_secs(60),
    }
}

// ── Main initialization ───────────────────────────────────────────────────────

/// Network initialization entry point.
///
/// Called from `main()`.  Performs factory-reset check, CYW43 init, then dispatches to STA or AP
/// mode based on stored credentials.
pub async fn start(spawner: Spawner, p: embassy_rp::Peripherals) {
    // ── Platform validation ──────────────────────────────────────────────────
    validate_platform();

    // ── Factory reset check ──────────────────────────────────────────────────
    // Use GPIO23 as Flex (input with pull-up) to sample the BOOTSEL line before
    // reconfiguring it as the CYW43 WL_ON output.
    let mut pin23 = Flex::new(p.PIN_23);
    let do_factory_reset = check_factory_reset(&mut pin23).await;

    // Reconfigure as output (low = WL_ON de-asserted initially).
    pin23.set_as_output();
    pin23.set_low();

    // ── CYW43 init ───────────────────────────────────────────────────────────
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = cyw43_pio::PioSpi::new(
        &mut pio.common,
        pio.sm0,
        // CYW43_CLOCK_DIVIDER raises the SPI clock from ~37.5 MHz (default) to ~50 MHz,
        // reducing per-packet SPI transfer time by ~25%. See src/board.rs for derivation.
        pico_conduit::board::CYW43_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        embassy_rp::dma::Channel::new(p.DMA_CH0, Irqs),
    );

    let state = CYW43_STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) =
        cyw43::new(state, pin23, spi, CYW43_FW, CYW43_NVRAM).await;

    spawner.must_spawn(cyw43_task(runner));
    control.init(CYW43_CLM).await;

    // Store control in CONTROL_MUTEX before the LED task begins using it.
    *CONTROL_MUTEX.lock().await = Some(ControlWrapper(control));

    // Signal the LED task (spawned from main before start() is called).
    LED_SIGNAL.signal(LedState::Booting);

    // ── Factory reset ────────────────────────────────────────────────────────
    let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    let flash = FLASH_CELL.init(flash);

    if do_factory_reset {
        defmt::warn!("factory reset triggered via BOOTSEL hold");
        LED_SIGNAL.signal(LedState::Saving);
        erase_credentials_flash(flash);
        let mut watchdog = Watchdog::new(p.WATCHDOG);
        watchdog.trigger_reset();
        #[allow(clippy::empty_loop)]
        loop {}
    }

    // ── Load or provision credentials ────────────────────────────────────────
    let compile_ssid = option_env!("PICO_WIFI_SSID");
    let compile_pass = option_env!("PICO_WIFI_PASS");

    let creds = if let (Some(ssid), Some(pass)) = (compile_ssid, compile_pass) {
        defmt::info!("Using compile-time credentials");
        Credentials::new(ssid, pass)
    } else {
        load_credentials_flash(flash)
    };

    // Safety: net_device lifetime is tied to the CYW43 State/Runner which are in StaticCells.
    let net_device: cyw43::NetDriver<'static> = unsafe { core::mem::transmute(net_device) };

    if let Some(creds) = creds {
        sta_mode(spawner, net_device, creds).await;
    } else {
        defmt::warn!("No credentials — entering AP provisioning mode");
        ap_mode(spawner, net_device, flash, p.WATCHDOG).await;
    }
}

// ── STA mode ─────────────────────────────────────────────────────────────────

async fn sta_mode(spawner: Spawner, net_device: cyw43::NetDriver<'static>, creds: Credentials) {
    let config = Config::dhcpv4(DhcpConfig::default());
    let seed = 0x_dead_beef_cafe_babe_u64;
    let resources = STACK_RESOURCES_STA.init(StackResources::new());
    let (stack, net_runner) = embassy_net::new(net_device, config, resources, seed);
    spawner.must_spawn(net_task(net_runner));

    defmt::info!("Joining Wi-Fi: {}", creds.ssid.as_str());
    loop {
        LED_SIGNAL.signal(LedState::Connecting);
        let ok = {
            let mut ctrl = CONTROL_MUTEX.lock().await;
            if let Some(c) = ctrl.as_mut() {
                c.join(
                    creds.ssid.as_str(),
                    JoinOptions::new(creds.password.as_bytes()),
                )
                .await
                .is_ok()
            } else {
                false
            }
        };
        if ok {
            break;
        }
        defmt::warn!("Join failed, retrying in 3s");
        Timer::after_secs(3).await;
    }

    stack.wait_config_up().await;

    let mut config_ip: heapless::String<16> = heapless::String::new();
    if let Some(cfg) = stack.config_v4() {
        defmt::info!("IP: {}", cfg.address);
        let _ = write!(config_ip, "{}", cfg.address.address());
    }

    // Enable PM2 after STA join
    {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            c.set_power_management(cyw43::PowerManagementMode::PowerSave)
                .await;
        }
    }
    defmt::info!("wifi pm: power_save");

    #[cfg(feature = "transport-tcp")]
    {
        defmt::info!("Listening on TCP port {}", TCP_PORT);
        tcp_server(stack, creds.ssid, config_ip).await;
    }
    #[cfg(feature = "transport-websocket")]
    {
        defmt::info!(
            "Listening on WebSocket port {}",
            pico_conduit::board::WS_PORT
        );
        ws_server(stack, creds.ssid, config_ip).await;
    }
    #[cfg(feature = "transport-mqtt")]
    {
        mqtt_client(stack, creds, config_ip).await;
    }
}

// ── TCP server (STA mode) ─────────────────────────────────────────────────────

#[cfg(feature = "transport-tcp")]
async fn tcp_server(
    stack: Stack<'static>,
    config_ssid: heapless::String<32>,
    config_ip: heapless::String<16>,
) {
    let mut rx_buf = [0u8; pico_conduit::board::TCP_RX_BUF_SIZE];
    let mut tx_buf = [0u8; pico_conduit::board::TCP_TX_BUF_SIZE];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
    socket.set_nagle_enabled(!pico_conduit::board::TCP_NODELAY);

    let mut reconnect_attempt: u8 = 0;
    let mut total_offline_secs: u16 = 0;

    loop {
        LED_SIGNAL.signal(LedState::Connected);

        defmt::info!("accept() on port {}", TCP_PORT);
        match socket
            .accept(IpListenEndpoint {
                addr: None,
                port: TCP_PORT,
            })
            .await
        {
            Ok(()) => {
                // Disable PM while client is connected
                {
                    let mut ctrl = CONTROL_MUTEX.lock().await;
                    if let Some(c) = ctrl.as_mut() {
                        c.set_power_management(cyw43::PowerManagementMode::None)
                            .await;
                    }
                }
                defmt::info!("wifi pm: none");
                defmt::info!("Client connected");
                reconnect_attempt = 0;
                total_offline_secs = 0;

                {
                    let mut transport = TcpTransport::new(&mut socket);
                    handle_client(&mut transport, &config_ssid, &config_ip).await;
                }

                // Re-enable PM after client disconnects
                {
                    let mut ctrl = CONTROL_MUTEX.lock().await;
                    if let Some(c) = ctrl.as_mut() {
                        c.set_power_management(cyw43::PowerManagementMode::PowerSave)
                            .await;
                    }
                }
                defmt::info!("wifi pm: power_save");
                defmt::info!("Client disconnected");
            }
            Err(e) => {
                defmt::warn!("accept() error: {:?}", e);
                socket.abort();

                let backoff = backoff_duration(reconnect_attempt);
                #[allow(clippy::cast_possible_truncation)] // backoff_duration ≤ 60s, fits u16
                let secs = backoff.as_secs() as u16;
                total_offline_secs = total_offline_secs.saturating_add(secs);

                if total_offline_secs >= MAX_RECONNECT_SECS {
                    defmt::error!("Connection failed 10 min — SOS");
                    loop {
                        LED_SIGNAL.signal(LedState::Error);
                        Timer::after_secs(30).await;
                    }
                }

                defmt::warn!("reconnect attempt {} after {}s", reconnect_attempt, secs);
                LED_SIGNAL.signal(LedState::Reconnecting);
                Timer::after(backoff).await;
                reconnect_attempt = reconnect_attempt.saturating_add(1);
            }
        }
    }
}

// ── WebSocket transport (STA mode) ────────────────────────────────────────────

/// WebSocket transport — wraps a `TcpSocket` after HTTP upgrade handshake.
///
/// Frames are WebSocket text frames (opcode 0x1). Ping/pong is handled
/// transparently; close frames trigger `TransportError::Disconnected`.
#[cfg(feature = "transport-websocket")]
struct WsTransport<'a, 'b> {
    socket: &'a mut TcpSocket<'b>,
}

#[cfg(feature = "transport-websocket")]
impl WsTransport<'_, '_> {
    /// Read exactly `buf.len()` bytes from the socket with timeout.
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), TransportError> {
        let mut offset = 0;
        while offset < buf.len() {
            match with_timeout(TCP_READ_TIMEOUT, self.socket.read(&mut buf[offset..])).await {
                Err(_) => return Err(TransportError::Timeout),
                Ok(Err(_) | Ok(0)) => return Err(TransportError::Disconnected),
                Ok(Ok(n)) => offset += n,
            }
        }
        Ok(())
    }
}

#[cfg(feature = "transport-websocket")]
impl Transport for WsTransport<'_, '_> {
    async fn read_frame<'c>(&mut self, buf: &'c mut [u8]) -> Result<&'c [u8], TransportError> {
        use pico_conduit::ws::{
            OPCODE_CLOSE, OPCODE_PING, OPCODE_TEXT, encode_pong_frame, unmask,
        };

        loop {
            // Read first 2 bytes of WS frame header
            let mut hdr = [0u8; 2];
            if let Err(e) = self.read_exact(&mut hdr).await {
                self.socket.abort();
                return Err(e);
            }

            let opcode = hdr[0] & 0x0F;
            let masked = (hdr[1] & 0x80) != 0;
            let len7 = (hdr[1] & 0x7F) as usize;

            let payload_len = if len7 <= 125 {
                len7
            } else if len7 == 126 {
                let mut ext = [0u8; 2];
                if let Err(e) = self.read_exact(&mut ext).await {
                    self.socket.abort();
                    return Err(e);
                }
                ((ext[0] as usize) << 8) | (ext[1] as usize)
            } else {
                // 64-bit lengths not supported
                self.socket.abort();
                return Err(TransportError::Protocol(
                    pico_conduit::protocol::ERROR_MSG_TOO_LARGE,
                ));
            };

            if payload_len > MAX_MSG_LEN {
                self.socket.abort();
                return Err(TransportError::Protocol(
                    pico_conduit::protocol::ERROR_MSG_TOO_LARGE,
                ));
            }

            let mut mask_key = [0u8; 4];
            if masked && let Err(e) = self.read_exact(&mut mask_key).await {
                self.socket.abort();
                return Err(e);
            }

            // Read payload
            if payload_len > 0 {
                if let Err(e) = self.read_exact(&mut buf[..payload_len]).await {
                    self.socket.abort();
                    return Err(e);
                }
                if masked {
                    unmask(&mut buf[..payload_len], mask_key);
                }
            }

            match opcode {
                OPCODE_TEXT => return Ok(&buf[..payload_len]),
                OPCODE_CLOSE => {
                    // Send close frame back
                    let _ = self.socket.write_all(&[0x88, 0x00]).await;
                    self.socket.abort();
                    return Err(TransportError::Disconnected);
                }
                OPCODE_PING => {
                    // Respond with pong carrying the same payload
                    let mut pong_buf = [0u8; 127];
                    if let Ok(n) = encode_pong_frame(&buf[..payload_len], &mut pong_buf) {
                        let _ = self.socket.write_all(&pong_buf[..n]).await;
                    }
                }
                _ => {} // PONG and unknown opcodes: ignore
            }
        }
    }

    async fn write_frame(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let mut hdr = [0u8; 4];
        let hdr_len = pico_conduit::ws::encode_text_frame_header(data.len(), &mut hdr)
            .map_err(TransportError::Protocol)?;

        if self.socket.write_all(&hdr[..hdr_len]).await.is_err() {
            self.socket.abort();
            return Err(TransportError::Disconnected);
        }
        if self.socket.write_all(data).await.is_err() {
            self.socket.abort();
            return Err(TransportError::Disconnected);
        }
        Ok(())
    }
}

/// Perform the WebSocket HTTP upgrade handshake on an accepted TCP socket.
///
/// Reads HTTP headers, validates the `Upgrade: websocket` request, computes
/// `Sec-WebSocket-Accept`, and sends the HTTP 101 response.
#[cfg(feature = "transport-websocket")]
async fn ws_handshake(socket: &mut TcpSocket<'_>) -> Result<(), TransportError> {
    use pico_conduit::protocol::ERROR_WEBSOCKET_HANDSHAKE;

    let mut hdr_buf = [0u8; 512];
    let n = read_http_headers(socket, &mut hdr_buf).await;
    if n == 0 {
        return Err(TransportError::Disconnected);
    }
    let headers = &hdr_buf[..n];

    // Validate Upgrade header
    let upgrade = extract_header(headers, b"Upgrade");
    if !matches!(upgrade, Some(v) if v.eq_ignore_ascii_case(b"websocket")) {
        return Err(TransportError::Protocol(ERROR_WEBSOCKET_HANDSHAKE));
    }

    // Extract Sec-WebSocket-Key
    let key = extract_header(headers, b"Sec-WebSocket-Key")
        .ok_or(TransportError::Protocol(ERROR_WEBSOCKET_HANDSHAKE))?;
    // Trim trailing whitespace/CR
    let key = key.strip_suffix(b"\r").unwrap_or(key);
    let key = key.strip_suffix(b" ").unwrap_or(key);

    // Compute accept key
    let mut accept = [0u8; 28];
    let accept_len = pico_conduit::ws::compute_accept_key(key, &mut accept);

    // Build HTTP 101 Switching Protocols response
    let mut resp = [0u8; 160];
    let mut pos = 0;
    macro_rules! push {
        ($src:expr) => {
            for &b in $src {
                if pos < resp.len() {
                    resp[pos] = b;
                    pos += 1;
                }
            }
        };
    }
    push!(b"HTTP/1.1 101 Switching Protocols\r\n");
    push!(b"Upgrade: websocket\r\n");
    push!(b"Connection: Upgrade\r\n");
    push!(b"Sec-WebSocket-Accept: ");
    push!(&accept[..accept_len]);
    push!(b"\r\n\r\n");

    if socket.write_all(&resp[..pos]).await.is_err() {
        return Err(TransportError::Disconnected);
    }

    Ok(())
}

#[cfg(feature = "transport-websocket")]
async fn ws_server(
    stack: Stack<'static>,
    config_ssid: heapless::String<32>,
    config_ip: heapless::String<16>,
) {
    let mut rx_buf = [0u8; pico_conduit::board::TCP_RX_BUF_SIZE];
    let mut tx_buf = [0u8; pico_conduit::board::TCP_TX_BUF_SIZE];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
    socket.set_nagle_enabled(!pico_conduit::board::TCP_NODELAY);

    let mut reconnect_attempt: u8 = 0;
    let mut total_offline_secs: u16 = 0;

    loop {
        LED_SIGNAL.signal(LedState::Connected);

        defmt::info!("accept() on WS port {}", pico_conduit::board::WS_PORT);
        match socket
            .accept(IpListenEndpoint {
                addr: None,
                port: pico_conduit::board::WS_PORT,
            })
            .await
        {
            Ok(()) => {
                {
                    let mut ctrl = CONTROL_MUTEX.lock().await;
                    if let Some(c) = ctrl.as_mut() {
                        c.set_power_management(cyw43::PowerManagementMode::None)
                            .await;
                    }
                }
                defmt::info!("WS client connected");
                reconnect_attempt = 0;
                total_offline_secs = 0;

                // Perform WebSocket handshake, then run handle_client
                match ws_handshake(&mut socket).await {
                    Ok(()) => {
                        let mut transport = WsTransport {
                            socket: &mut socket,
                        };
                        handle_client(&mut transport, &config_ssid, &config_ip).await;
                    }
                    Err(_) => {
                        defmt::warn!("WebSocket handshake failed");
                    }
                }

                socket.abort();

                {
                    let mut ctrl = CONTROL_MUTEX.lock().await;
                    if let Some(c) = ctrl.as_mut() {
                        c.set_power_management(cyw43::PowerManagementMode::PowerSave)
                            .await;
                    }
                }
                defmt::info!("WS client disconnected");
            }
            Err(e) => {
                defmt::warn!("WS accept() error: {:?}", e);
                socket.abort();

                let backoff = backoff_duration(reconnect_attempt);
                #[allow(clippy::cast_possible_truncation)] // backoff_duration ≤ 60s, fits u16
                let secs = backoff.as_secs() as u16;
                total_offline_secs = total_offline_secs.saturating_add(secs);

                if total_offline_secs >= MAX_RECONNECT_SECS {
                    defmt::error!("WS connection failed 10 min — SOS");
                    loop {
                        LED_SIGNAL.signal(LedState::Error);
                        Timer::after_secs(30).await;
                    }
                }

                defmt::warn!("WS reconnect attempt {} after {}s", reconnect_attempt, secs);
                LED_SIGNAL.signal(LedState::Reconnecting);
                Timer::after(backoff).await;
                reconnect_attempt = reconnect_attempt.saturating_add(1);
            }
        }
    }
}

// ── MQTT client (STA mode) ───────────────────────────────────────────────────

#[cfg(feature = "transport-mqtt")]
#[allow(clippy::too_many_lines)] // MQTT client is inherently a long state-machine loop
async fn mqtt_client(stack: Stack<'static>, creds: Credentials, config_ip: heapless::String<16>) {
    use pico_conduit::mqtt;
    use rust_mqtt::Bytes;
    use rust_mqtt::buffer::BumpBuffer;
    use rust_mqtt::client::Client;
    use rust_mqtt::client::event::Event;
    use rust_mqtt::client::options::{
        ConnectOptions, PublicationOptions, RetainHandling, SubscriptionOptions,
    };
    use rust_mqtt::config::{KeepAlive, SessionExpiryInterval};
    use rust_mqtt::types::{MqttString, QoS, TopicName};

    if creds.mqtt_host.is_empty() {
        defmt::warn!("MQTT host is empty — MQTT transport disabled");
        #[allow(clippy::empty_loop)]
        loop {
            Timer::after_secs(3600).await;
        }
    }

    // Resolve MAC for topic/client-ID construction
    let mac = {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            c.address().await
        } else {
            [0u8; 6]
        }
    };
    let cmd_topic_str = mqtt::cmd_topic(mac);
    let resp_topic_str = mqtt::resp_topic(mac);
    let client_id_str = mqtt::client_id(mac);

    defmt::info!(
        "MQTT broker {}:{}, client_id={}, cmd={}, resp={}",
        creds.mqtt_host.as_str(),
        creds.mqtt_port,
        client_id_str.as_str(),
        cmd_topic_str.as_str(),
        resp_topic_str.as_str(),
    );

    let mut reconnect_attempt: u8 = 0;

    loop {
        LED_SIGNAL.signal(LedState::MqttConnecting);

        // Parse broker IP address
        let Ok(broker_ip) = creds.mqtt_host.as_str().parse::<core::net::Ipv4Addr>() else {
            defmt::error!("Invalid MQTT broker IP: {}", creds.mqtt_host.as_str());
            LED_SIGNAL.signal(LedState::Error);
            Timer::after_secs(60).await;
            continue;
        };
        let broker_addr = embassy_net::Ipv4Address::from(broker_ip.octets());

        // TCP connect to broker
        let mut rx_buf = [0u8; pico_conduit::board::TCP_RX_BUF_SIZE];
        let mut tx_buf = [0u8; pico_conduit::board::TCP_TX_BUF_SIZE];
        let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);
        socket.set_nagle_enabled(!pico_conduit::board::TCP_NODELAY);
        socket.set_timeout(Some(Duration::from_secs(90)));

        if let Err(e) = socket.connect((broker_addr, creds.mqtt_port)).await {
            defmt::warn!("MQTT TCP connect failed: {:?}", e);
            let secs = mqtt::backoff_secs(reconnect_attempt);
            defmt::warn!(
                "MQTT reconnect attempt {} after {}s",
                reconnect_attempt,
                secs
            );
            LED_SIGNAL.signal(LedState::Reconnecting);
            Timer::after_secs(u64::from(secs)).await;
            reconnect_attempt = reconnect_attempt.saturating_add(1);
            continue;
        }

        defmt::info!("MQTT TCP connected");

        // Set up MQTT client with BumpBuffer (no-alloc)
        let mut buf_storage = [0u8; 1024];
        let mut buffer = BumpBuffer::new(&mut buf_storage);
        let mut client: Client<'_, _, _, 1, 1, 1> = Client::new(&mut buffer);

        let connect_opts = ConnectOptions {
            session_expiry_interval: SessionExpiryInterval::Seconds(0),
            clean_start: true,
            keep_alive: KeepAlive::Seconds(60),
            will: None,
            user_name: None,
            password: None,
        };
        let mqtt_client_id = MqttString::try_from(client_id_str.as_str())
            .expect("client_id exceeds MqttString limit");

        // connect() takes socket by value — client owns the connection afterward
        match client
            .connect(socket, &connect_opts, Some(mqtt_client_id))
            .await
        {
            Ok(_) => {
                defmt::info!("MQTT CONNECT ok");
            }
            Err(e) => {
                defmt::warn!("MQTT CONNECT failed: {:?}", e);
                let secs = mqtt::backoff_secs(reconnect_attempt);
                LED_SIGNAL.signal(LedState::Reconnecting);
                Timer::after_secs(u64::from(secs)).await;
                reconnect_attempt = reconnect_attempt.saturating_add(1);
                continue;
            }
        }

        // Reset bump buffer after connect handshake to reclaim space
        // SAFETY: no references to connect-phase data are held at this point
        unsafe { client.buffer().reset() };

        // Subscribe to command topic
        let cmd_topic = unsafe {
            TopicName::new_unchecked(
                MqttString::from_slice(cmd_topic_str.as_str())
                    .expect("cmd topic exceeds MqttString limit"),
            )
        };
        let sub_opts = SubscriptionOptions {
            retain_handling: RetainHandling::SendIfNotSubscribedBefore,
            retain_as_published: false,
            no_local: false,
            qos: QoS::AtMostOnce,
        };
        if let Err(e) = client.subscribe(cmd_topic.clone().into(), sub_opts).await {
            defmt::warn!("MQTT SUBSCRIBE failed: {:?}", e);
            let secs = mqtt::backoff_secs(reconnect_attempt);
            LED_SIGNAL.signal(LedState::Reconnecting);
            Timer::after_secs(u64::from(secs)).await;
            reconnect_attempt = reconnect_attempt.saturating_add(1);
            continue;
        }

        // Wait for SUBACK before entering message loop
        match client.poll().await {
            Ok(Event::Suback(_)) => {
                defmt::info!("MQTT subscribed to {}", cmd_topic_str.as_str());
            }
            Ok(_) => {
                defmt::warn!("Expected SUBACK, got different event");
                continue;
            }
            Err(e) => {
                defmt::warn!("MQTT poll error waiting for SUBACK: {:?}", e);
                continue;
            }
        }

        LED_SIGNAL.signal(LedState::MqttConnected);
        reconnect_attempt = 0;

        // Process messages
        let mut state = pico_conduit::router::DeviceState::default();
        let _ = state.config_ssid.push_str(creds.ssid.as_str());
        let _ = state.config_ip.push_str(config_ip.as_str());
        state.config_connected = true;
        #[cfg(feature = "transport-mqtt")]
        {
            let _ = state.config_mqtt_host.push_str(creds.mqtt_host.as_str());
            state.config_mqtt_port = creds.mqtt_port;
        }

        loop {
            // Reset bump buffer before each poll to reclaim space from previous iteration
            // SAFETY: no references to previous poll data are held — response was serialized
            // and published (or dropped) before reaching this point
            unsafe { client.buffer().reset() };

            match client.poll().await {
                Ok(Event::Publish(publish)) => {
                    let payload = publish.message.as_ref();
                    defmt::debug!("MQTT PUBLISH received, {} bytes", payload.len());

                    let resp = match pico_conduit::protocol::parse_command(payload) {
                        Ok(cmd) => match pico_conduit::router::validate_route(&cmd) {
                            Ok(route) => pico_conduit::router::dispatch(&cmd, route, &mut state),
                            Err(err_resp) => err_resp,
                        },
                        Err(err_code) => pico_conduit::protocol::Response::error("", err_code),
                    };

                    // Serialize and publish response
                    let mut resp_buf = [0u8; MAX_MSG_LEN];
                    if let Ok(n) =
                        pico_conduit::protocol::serialize_response(&resp, &mut resp_buf)
                    {
                        let resp_topic = unsafe {
                            TopicName::new_unchecked(
                                MqttString::from_slice(resp_topic_str.as_str())
                                    .expect("resp topic exceeds MqttString limit"),
                            )
                        };
                        let pub_opts = PublicationOptions {
                            retain: false,
                            topic: resp_topic,
                            qos: QoS::AtMostOnce,
                        };
                        if let Err(e) = client.publish(&pub_opts, Bytes::from(&resp_buf[..n])).await
                        {
                            defmt::warn!("MQTT PUBLISH response failed: {:?}", e);
                            break;
                        }
                    }

                    if state.pending_reboot {
                        defmt::info!("Reboot requested via MQTT");
                        break;
                    }
                }
                Ok(Event::Pingresp) => {
                    defmt::debug!("MQTT PINGRESP");
                }
                Ok(_) => {}
                Err(e) => {
                    defmt::warn!("MQTT poll error: {:?}", e);
                    break;
                }
            }
        }

        // Client dropped here — socket released with it
        defmt::warn!("MQTT disconnected, will reconnect");
    }
}

#[cfg(any(feature = "transport-tcp", feature = "transport-websocket"))]
async fn handle_client<T: Transport>(
    transport: &mut T,
    config_ssid: &heapless::String<32>,
    config_ip: &heapless::String<16>,
) {
    let mut frame_buf = [0u8; MAX_MSG_LEN];
    let mut resp_buf = [0u8; MAX_MSG_LEN];
    let mut device_state = DeviceState {
        config_ssid: config_ssid.clone(),
        config_ip: config_ip.clone(),
        config_connected: true,
        ..DeviceState::default()
    };

    loop {
        let frame = match transport.read_frame(&mut frame_buf).await {
            Ok(frame) => frame,
            Err(TransportError::Protocol(err_code)) => {
                let resp = pico_conduit::protocol::Response::error("", err_code);
                if let Ok(n) = serialize_response(&resp, &mut resp_buf) {
                    let _ = transport.write_frame(&resp_buf[..n]).await;
                }
                continue;
            }
            Err(TransportError::Disconnected | TransportError::Timeout) => {
                return;
            }
        };

        let response = match parse_command(frame) {
            Err(err_code) => pico_conduit::protocol::Response::error("", err_code),
            Ok(cmd) => match validate_route(&cmd) {
                Err(r) => r,
                Ok(route) => dispatch(&cmd, route, &mut device_state),
            },
        };

        if let Ok(n) = serialize_response(&response, &mut resp_buf)
            && transport.write_frame(&resp_buf[..n]).await.is_err()
        {
            return;
        }

        if device_state.pending_reboot {
            defmt::info!("rebooting to USB bootloader");
            LED_SIGNAL.signal(LedState::Rebooting);
            Timer::after_millis(650).await;
            embassy_rp::rom_data::reset_to_usb_boot(0, 0);
            #[allow(clippy::empty_loop)]
            loop {}
        }
    }
}

// ── AP mode and captive portal ────────────────────────────────────────────────

/// AP provisioning mode — never returns (runs until credentials are saved and watchdog reboots).
#[allow(clippy::default_trait_access)] // heapless::Vec capacity can't be inferred without Default
async fn ap_mode(
    spawner: Spawner,
    net_device: cyw43::NetDriver<'static>,
    flash: &'static mut CredFlash,
    watchdog_peri: embassy_rp::Peri<'static, embassy_rp::peripherals::WATCHDOG>,
) {
    // Get MAC address to derive a unique SSID
    let mac = {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            c.address().await
        } else {
            [0u8; 6]
        }
    };
    let ap_ssid = make_ap_ssid(mac);
    defmt::info!("AP SSID: {}", ap_ssid.as_str());

    // Start CYW43 in open AP mode on channel 6
    {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            c.start_ap_open(ap_ssid.as_str(), 6).await;
        }
    }
    LED_SIGNAL.signal(LedState::Provisioning);

    // Embassy-net with static IP 192.168.4.1/24
    let ap_ip = Ipv4Address::new(AP_IP[0], AP_IP[1], AP_IP[2], AP_IP[3]);
    let config = Config::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(ap_ip, 24),
        gateway: Some(ap_ip),
        dns_servers: Default::default(),
    });
    let seed = 0x_cafe_babe_dead_beef_u64;
    let resources = STACK_RESOURCES_AP.init(StackResources::new());
    let (stack, net_runner) = embassy_net::new(net_device, config, resources, seed);
    spawner.must_spawn(net_task_ap(net_runner));

    // Give the stack a moment to come up
    Timer::after_millis(200).await;

    let mut watchdog = Watchdog::new(watchdog_peri);

    // Run DHCP server and HTTP portal concurrently
    embassy_futures::select::select(
        dhcp_server(&stack),
        portal_server(&stack, &ap_ssid, flash, &mut watchdog),
    )
    .await;
}

// ── Minimal DHCP server ───────────────────────────────────────────────────────

/// Minimal BOOTP/DHCP server that always assigns 192.168.4.2/24 to connecting clients.
///
/// Handles DHCPDISCOVER → DHCPOFFER and DHCPREQUEST → DHCPACK.
async fn dhcp_server(stack: &Stack<'static>) {
    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buf = [0u8; 600];
    let mut tx_buf = [0u8; 600];
    let mut socket = UdpSocket::new(*stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);

    if socket
        .bind(embassy_net::IpEndpoint::new(
            embassy_net::IpAddress::Ipv4(Ipv4Address::UNSPECIFIED),
            67,
        ))
        .is_err()
    {
        return;
    }

    let mut pkt = [0u8; 548];
    loop {
        let Ok((n, _meta)) = socket.recv_from(&mut pkt).await else {
            continue;
        };
        if n < 236 {
            continue;
        }
        // BOOTP op = 1 (BOOTREQUEST)
        if pkt[0] != 1 {
            continue;
        }

        let msg_type = dhcp_option_u8(&pkt[..n], 53).unwrap_or(0);
        // Only handle DHCPDISCOVER (1) and DHCPREQUEST (3)
        if msg_type != 1 && msg_type != 3 {
            continue;
        }

        let reply = build_dhcp_reply(&pkt[..n], msg_type);
        let _ = socket
            .send_to(
                &reply,
                embassy_net::IpEndpoint::new(
                    embassy_net::IpAddress::Ipv4(Ipv4Address::BROADCAST),
                    68,
                ),
            )
            .await;
    }
}

/// Extract a single-byte DHCP option value from a DHCP packet.
fn dhcp_option_u8(pkt: &[u8], option: u8) -> Option<u8> {
    if pkt.len() < 240 {
        return None;
    }
    let mut i = 240; // options start after 236-byte BOOTP header + 4-byte magic cookie
    while i < pkt.len() {
        let opt = pkt[i];
        match opt {
            255 => break, // END
            0 => {
                i += 1;
                continue;
            } // PAD
            _ => {}
        }
        if i + 1 >= pkt.len() {
            break;
        }
        let len = pkt[i + 1] as usize;
        if opt == option && len >= 1 && i + 2 < pkt.len() {
            return Some(pkt[i + 2]);
        }
        i = i.saturating_add(2 + len);
    }
    None
}

/// Build a DHCPOFFER (for DISCOVER) or DHCPACK (for REQUEST) reply.
fn build_dhcp_reply(req: &[u8], msg_type: u8) -> [u8; 300] {
    let mut r = [0u8; 300];

    // BOOTP header
    r[0] = 2; // BOOTREPLY
    r[1] = 1; // htype = Ethernet
    r[2] = 6; // hlen
    // xid
    r[4..8].copy_from_slice(&req[4..8]);
    // yiaddr: DHCP client address
    r[16..20].copy_from_slice(&DHCP_CLIENT_IP);
    // siaddr: DHCP server (gateway) address
    r[20..24].copy_from_slice(&AP_IP);
    // chaddr (client MAC)
    let chaddr_len = req[2] as usize;
    if chaddr_len <= 16 && 28 + chaddr_len <= req.len() {
        r[28..28 + chaddr_len].copy_from_slice(&req[28..28 + chaddr_len]);
    }

    // Magic cookie
    r[236] = 99;
    r[237] = 130;
    r[238] = 83;
    r[239] = 99;

    // Options
    let mut p = 240usize;
    // 53: DHCP message type — OFFER=2, ACK=5
    r[p] = 53;
    r[p + 1] = 1;
    r[p + 2] = if msg_type == 1 { 2 } else { 5 };
    p += 3;
    // 54: server identifier
    r[p] = 54;
    r[p + 1] = 4;
    r[p + 2..p + 6].copy_from_slice(&AP_IP);
    p += 6;
    // 51: lease time = 3600s = 0x00000E10
    r[p] = 51;
    r[p + 1] = 4;
    r[p + 2] = 0;
    r[p + 3] = 0;
    r[p + 4] = 0x0E;
    r[p + 5] = 0x10;
    p += 6;
    // 1: subnet mask
    r[p] = 1;
    r[p + 1] = 4;
    r[p + 2..p + 6].copy_from_slice(&SUBNET_MASK);
    p += 6;
    // 3: router (gateway)
    r[p] = 3;
    r[p + 1] = 4;
    r[p + 2..p + 6].copy_from_slice(&AP_IP);
    p += 6;
    // END
    r[p] = 255;

    r
}

// ── HTTP captive portal ───────────────────────────────────────────────────────

/// HTTP server on port 80 — serves the Wi-Fi setup form and handles credential submission.
async fn portal_server(
    stack: &Stack<'static>,
    ap_ssid: &str,
    flash: &'static mut CredFlash,
    watchdog: &mut Watchdog,
) {
    let mut rx_buf = [0u8; 512];
    let mut tx_buf = [0u8; 2048];
    loop {
        let mut socket = TcpSocket::new(*stack, &mut rx_buf, &mut tx_buf);
        socket.set_timeout(Some(Duration::from_secs(10)));

        if socket
            .accept(IpListenEndpoint {
                addr: None,
                port: 80,
            })
            .await
            .is_err()
        {
            continue;
        }

        handle_http_request(&mut socket, ap_ssid, flash, watchdog).await;
        socket.abort();
        Timer::after_millis(50).await;
    }
}

/// Read HTTP request headers (up to `buf.len()` bytes) until `\r\n\r\n`.
async fn read_http_headers(socket: &mut TcpSocket<'_>, buf: &mut [u8]) -> usize {
    let mut total = 0usize;
    let mut b = [0u8; 1];
    while total < buf.len() {
        match socket.read(&mut b).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                buf[total] = b[0];
                total += 1;
                if total >= 4 && &buf[total - 4..total] == b"\r\n\r\n" {
                    break;
                }
            }
        }
    }
    total
}

/// Extract the raw value bytes of an HTTP header from a raw header block.
fn extract_header<'a>(raw: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    // Skip the request line
    let mut i = 0;
    while i < raw.len() && raw[i] != b'\n' {
        i += 1;
    }
    i += 1;

    loop {
        let start = i;
        while i < raw.len() && raw[i] != b'\n' {
            i += 1;
        }
        let line = {
            let l = &raw[start..i];
            l.strip_suffix(b"\r").unwrap_or(l)
        };
        i += 1;
        if line.is_empty() {
            break;
        }
        if line.len() > name.len() + 1
            && line[name.len()] == b':'
            && line[..name.len()].eq_ignore_ascii_case(name)
        {
            let val = &line[name.len() + 1..];
            return Some(val.strip_prefix(b" ").unwrap_or(val));
        }
    }
    None
}

/// Find the byte offset of `needle` in `haystack`.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Write a complete HTTP response with the given status line and body.
async fn send_http(socket: &mut TcpSocket<'_>, status: &[u8], body: &[u8]) {
    // Build header in a fixed buffer
    let mut hdr = [0u8; 160];
    let mut pos = 0;
    macro_rules! push_bytes {
        ($src:expr) => {
            for &b in $src {
                if pos < hdr.len() {
                    hdr[pos] = b;
                    pos += 1;
                }
            }
        };
    }
    push_bytes!(b"HTTP/1.0 ");
    push_bytes!(status);
    push_bytes!(b"\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: ");
    // Render body length as ASCII
    let mut tmp = [0u8; 10];
    let mut tp = tmp.len();
    let mut n = body.len();
    if n == 0 {
        tp -= 1;
        tmp[tp] = b'0';
    } else {
        while n > 0 {
            tp -= 1;
            #[allow(clippy::cast_possible_truncation)] // n % 10 is 0-9, fits u8
            {
                tmp[tp] = b'0' + (n % 10) as u8;
            }
            n /= 10;
        }
    }
    push_bytes!(&tmp[tp..]);
    push_bytes!(b"\r\n\r\n");
    let _ = socket.write_all(&hdr[..pos]).await;
    let _ = socket.write_all(body).await;
}

/// Send an HTTP 302 redirect.
async fn send_redirect(socket: &mut TcpSocket<'_>, location: &[u8]) {
    let mut hdr = [0u8; 200];
    let mut pos = 0;
    macro_rules! push_bytes {
        ($src:expr) => {
            for &b in $src {
                if pos < hdr.len() {
                    hdr[pos] = b;
                    pos += 1;
                }
            }
        };
    }
    push_bytes!(b"HTTP/1.0 302 Found\r\nLocation: ");
    push_bytes!(location);
    push_bytes!(b"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let _ = socket.write_all(&hdr[..pos]).await;
}

/// Dispatch a single HTTP request and write the response.
async fn handle_http_request(
    socket: &mut TcpSocket<'_>,
    ap_ssid: &str,
    flash: &mut CredFlash,
    watchdog: &mut Watchdog,
) {
    let mut hdr_buf = [0u8; 512];
    let n = read_http_headers(socket, &mut hdr_buf).await;
    if n == 0 {
        return;
    }
    let headers = &hdr_buf[..n];

    // Parse request line
    let line_end = find_subsequence(headers, b"\r\n").unwrap_or(n);
    let Ok(req) = parse_request_line(&headers[..line_end.min(n)]) else {
        send_http(socket, b"400 Bad Request", b"Bad Request").await;
        return;
    };

    // Captive portal: redirect any request not targeting the AP gateway
    if let Some(host) = extract_header(headers, b"Host")
        && !host.starts_with(AP_IP_STR)
    {
        send_redirect(socket, AP_IP_URL).await;
        return;
    }

    match (req.method, req.path) {
        (Method::Get, "/" | "/index.html") => {
            serve_scan_form(socket, ap_ssid).await;
        }
        (Method::Get, "/status") => {
            send_http(socket, b"200 OK", b"{\"ssid\":\"\",\"connected\":false}").await;
        }
        (Method::Post, "/connect") => {
            let cl: usize = extract_header(headers, b"Content-Length")
                .and_then(|v| core::str::from_utf8(v).ok())
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(256);

            let mut body_buf = [0u8; 256];
            let mut read = 0;
            while read < cl {
                match socket.read(&mut body_buf[read..cl]).await {
                    Ok(0) | Err(_) => break,
                    Ok(k) => read += k,
                }
            }

            let mut dec_buf = [0u8; 128];
            let result = decode_url_encoded(&body_buf[..read], &mut dec_buf)
                .ok()
                .and_then(|d| parse_connect_form(d).ok());

            match result {
                Some(form) => {
                    // Copy to owned buffers (form borrows dec_buf which is local)
                    let mut ssid_buf: heapless::String<32> = heapless::String::new();
                    let mut pwd_buf: heapless::String<64> = heapless::String::new();
                    let mut mqtt_host_buf: heapless::String<64> = heapless::String::new();
                    let _ = ssid_buf.push_str(form.ssid);
                    let _ = pwd_buf.push_str(form.password);
                    let _ = mqtt_host_buf.push_str(form.mqtt_host);
                    let mqtt_port = form.mqtt_port;

                    // Serve "Testing…" page with auto-refresh
                    send_http(
                        socket,
                        b"200 OK",
                        b"<!DOCTYPE html><html><head>\
                          <meta http-equiv=\"refresh\" content=\"20;url=/\">\
                          </head><body><h1>Testing connection\xe2\x80\xa6</h1>\
                          <p>Connecting to Wi-Fi. Page refreshes in 20\xc2\xa0s.</p>\
                          </body></html>",
                    )
                    .await;
                    let _ = socket.flush().await;

                    handle_provision(
                        socket,
                        &ssid_buf,
                        &pwd_buf,
                        &mqtt_host_buf,
                        mqtt_port,
                        flash,
                        watchdog,
                    )
                    .await;
                }
                None => {
                    send_http(
                        socket,
                        b"400 Bad Request",
                        b"<!DOCTYPE html><html><body>\
                          <h1>Missing Fields</h1>\
                          <p>Both SSID and password are required.</p>\
                          <a href=\"/\">Try again</a></body></html>",
                    )
                    .await;
                }
            }
        }
        _ => {
            send_http(socket, b"404 Not Found", b"Not Found").await;
        }
    }
}

/// Trigger a CYW43 scan and serve an HTML form with the results.
async fn serve_scan_form(socket: &mut TcpSocket<'_>, _ap_ssid: &str) {
    LED_SIGNAL.signal(LedState::Scanning);

    let mut ssids: heapless::Vec<heapless::String<32>, 16> = heapless::Vec::new();
    {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            let mut scanner = c.scan(ScanOptions::default()).await;
            while let Some(bss) = scanner.next().await {
                if bss.ssid_len > 0 {
                    let len = bss.ssid_len as usize;
                    if let Ok(s) = core::str::from_utf8(&bss.ssid[..len.min(32)]) {
                        let mut hs: heapless::String<32> = heapless::String::new();
                        let _ = hs.push_str(s);
                        if !ssids.iter().any(|x| x == &hs) {
                            let _ = ssids.push(hs);
                        }
                    }
                }
            }
        }
    }

    LED_SIGNAL.signal(LedState::Provisioning);

    // Stream the response without a Content-Length (HTTP/1.0 + Connection: close is valid)
    let _ = socket
        .write_all(b"HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n")
        .await;
    let _ = socket
        .write_all(
            b"<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
        <title>pico-setup</title></head><body><h1>Wi-Fi Setup</h1>\
        <form method=\"POST\" action=\"/connect\">\
        <label>Network:<br><select name=\"ssid\">",
        )
        .await;

    for ssid in &ssids {
        let _ = socket.write_all(b"<option value=\"").await;
        let _ = socket.write_all(ssid.as_bytes()).await;
        let _ = socket.write_all(b"\">").await;
        let _ = socket.write_all(ssid.as_bytes()).await;
        let _ = socket.write_all(b"</option>").await;
    }

    let _ = socket
        .write_all(
            b"<option value=\"\">Hidden / other network</option>\
        </select></label><br>\
        <label>Manual SSID (leave blank to use selection above):<br>\
        <input type=\"text\" name=\"ssid_manual\"></label><br>\
        <label>Password:<br><input type=\"password\" name=\"password\"></label><br>\
        <hr><h2>MQTT (optional)</h2>\
        <label>Broker Host:<br><input type=\"text\" name=\"mqtt_host\" \
        placeholder=\"e.g. 192.168.1.100\"></label><br>\
        <label>Broker Port:<br><input type=\"number\" name=\"mqtt_port\" \
        value=\"1883\" min=\"1\" max=\"65535\"></label><br>\
        <input type=\"submit\" value=\"Connect\">\
        </form></body></html>",
        )
        .await;
}

/// Test submitted credentials: close AP, attempt STA join, save on success, restart AP on failure.
async fn handle_provision(
    _socket: &mut TcpSocket<'_>,
    ssid: &str,
    password: &str,
    mqtt_host: &str,
    mqtt_port: u16,
    flash: &mut CredFlash,
    watchdog: &mut Watchdog,
) {
    LED_SIGNAL.signal(LedState::Connecting);

    // Close AP mode before STA join
    {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            c.close_ap().await;
        }
    }

    // Attempt STA join (15 s timeout)
    let joined = {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            with_timeout(
                Duration::from_secs(15),
                c.join(ssid, JoinOptions::new(password.as_bytes())),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .is_some()
        } else {
            false
        }
    };

    if joined {
        if let Some(creds) = Credentials::with_mqtt(ssid, password, mqtt_host, mqtt_port)
            && save_credentials_flash(flash, &creds)
        {
            LED_SIGNAL.signal(LedState::Saving);
            defmt::info!("Credentials saved, rebooting via watchdog");
            Timer::after_secs(3).await;
            watchdog.trigger_reset();
            #[allow(clippy::empty_loop)]
            loop {}
        }
        // Flash write failed: leave STA and restart AP
        defmt::warn!("Flash save failed — restarting AP");
    } else {
        defmt::warn!("Credential test failed — restarting AP");
    }

    // Restart AP mode (open, channel 6, ssid "pico-setup" as placeholder;
    // real SSID will be set correctly on the next reboot into ap_mode)
    {
        let mut ctrl = CONTROL_MUTEX.lock().await;
        if let Some(c) = ctrl.as_mut() {
            c.leave().await;
            c.start_ap_open("pico-setup", 6).await;
        }
    }
    LED_SIGNAL.signal(LedState::Provisioning);
}
