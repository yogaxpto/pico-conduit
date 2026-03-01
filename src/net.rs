//! Wi-Fi connectivity, TCP socket server, LED task, provisioning, and flash storage.
//!
//! Embedded-only module (not compiled during `cargo test --lib`).
//!
//! ## Architecture
//!
//! `CONTROL_MUTEX` owns the `cyw43::Control` handle after CYW43 init.  The mutex is held only
//! for the duration of individual async HAL calls (gpio_set, join, scan, …) and released
//! immediately after each call — never held across a `Timer::after_*`.
//!
//! ### Startup sequence
//!
//! 1. `start()` — factory-reset check via GPIO23 (Flex), CYW43 init, store control in mutex.
//! 2. Spawn `led_task` (drives LED via CONTROL_MUTEX).
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
    Config, IpListenEndpoint, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4,
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

use pico_socketeer::led::{LedPattern, LedState, LED_SIGNAL};
use pico_socketeer::protocol::{FrameReader, MAX_MSG_LEN, parse_command, serialize_response};
use pico_socketeer::provisioning::portal::{
    Method, decode_url_encoded, make_ap_ssid, parse_connect_form, parse_request_line,
};
use pico_socketeer::provisioning::storage::Credentials;
use pico_socketeer::router::{DeviceState, dispatch, validate_route};

// ── CYW43 firmware blobs ──────────────────────────────────────────────────────
const CYW43_FW: &[u8] = include_bytes!("../cyw43-firmware/43439A0.bin");
const CYW43_CLM: &[u8] = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

// ── Configuration constants ───────────────────────────────────────────────────
/// TCP port for the JSON-over-TCP command interface.
pub const TCP_PORT: u16 = 4242;
const TCP_READ_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RECONNECT_SECS: u16 = 600; // 10 minutes

// ── Flash credential storage constants ───────────────────────────────────────
/// Pico 2W has 4 MB of flash.
const FLASH_SIZE: usize = 4 * 1024 * 1024;
/// The CREDENTIALS region occupies the last 8 KB of flash (see memory.x).
/// Offset from flash base: 4 MB − 8 KB = 0x3FE000.
const CRED_FLASH_OFFSET: u32 = (FLASH_SIZE - 8 * 1024) as u32;
/// Magic sentinel stored in the first 4 bytes of a valid credential record.
const CRED_MAGIC: u32 = 0xC0FF_EE42;
/// Record layout: magic(4) + ssid_len(1) + pwd_len(1) + ssid(32) + pwd(64) = 102 bytes.
const CRED_RECORD_SIZE: usize = 102;

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
/// single-core RP2350 running Embassy's cooperative scheduler because:
/// 1. Only one async task runs at a time (cooperative, not preemptive).
/// 2. `CriticalSectionRawMutex` disables IRQs, so no concurrent access is possible.
struct ControlWrapper(cyw43::Control<'static>);
// SAFETY: see above — single-core cooperative scheduling + CS mutex.
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
});

// ── Task type aliases ─────────────────────────────────────────────────────────
type CywSpi =
    cyw43_pio::PioSpi<'static, embassy_rp::peripherals::PIO0, 0, embassy_rp::peripherals::DMA_CH0>;
/// GPIO23 doubles as the CYW43 WL_ON line.  We first sample it as `Flex` (factory-reset check),
/// then reconfigure as output and pass to the cyw43 driver.
type CywRunner = cyw43::Runner<'static, Flex<'static>, CywSpi>;
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
        return None;
    }

    let ssid_len = buf[4] as usize;
    let pwd_len = buf[5] as usize;
    if ssid_len > 32 || pwd_len > 64 {
        return None;
    }

    let ssid = core::str::from_utf8(&buf[6..6 + ssid_len]).ok()?;
    let pwd = core::str::from_utf8(&buf[38..38 + pwd_len]).ok()?;
    Credentials::new(ssid, pwd)
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
    buf[4] = ssid_b.len() as u8;
    buf[5] = pwd_b.len() as u8;
    buf[6..6 + ssid_b.len()].copy_from_slice(ssid_b);
    buf[38..38 + pwd_b.len()].copy_from_slice(pwd_b);

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
/// Note that on Pico 2W hardware this pin is also the CYW43 WL_ON line; the check is
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
fn backoff_duration(attempt: u8) -> Duration {
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
        cyw43_pio::DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    let state = CYW43_STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pin23, spi, CYW43_FW).await;

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
    let config = Config::dhcpv4(Default::default());
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

    defmt::info!("Listening on TCP port {}", TCP_PORT);
    tcp_server(stack, creds.ssid, config_ip).await;
}

// ── TCP server (STA mode) ─────────────────────────────────────────────────────

async fn tcp_server(
    stack: Stack<'static>,
    config_ssid: heapless::String<32>,
    config_ip: heapless::String<16>,
) {
    let mut rx_buf = [0u8; MAX_MSG_LEN];
    let mut tx_buf = [0u8; MAX_MSG_LEN];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);

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

                handle_client(&mut socket, &config_ssid, &config_ip).await;

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
                let secs = backoff.as_secs();
                total_offline_secs = total_offline_secs.saturating_add(secs as u16);

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

async fn handle_client(
    socket: &mut TcpSocket<'_>,
    config_ssid: &heapless::String<32>,
    config_ip: &heapless::String<16>,
) {
    let mut frame_reader = FrameReader::new();
    let mut resp_buf = [0u8; MAX_MSG_LEN];
    let mut byte_buf = [0u8; 1];
    let mut device_state = DeviceState {
        config_ssid: config_ssid.clone(),
        config_ip: config_ip.clone(),
        config_connected: true,
        ..DeviceState::default()
    };

    loop {
        let read_result = with_timeout(TCP_READ_TIMEOUT, socket.read(&mut byte_buf)).await;
        match read_result {
            Err(_) => {
                defmt::warn!("TCP read timeout — closing idle connection");
                socket.abort();
                return;
            }
            Ok(Err(e)) => {
                defmt::warn!("TCP read error: {:?}", e);
                socket.abort();
                return;
            }
            Ok(Ok(0)) => {
                socket.abort();
                return;
            }
            Ok(Ok(_)) => {}
        }

        let response = match frame_reader.push(byte_buf[0]) {
            Err(err_code) => {
                frame_reader.reset();
                Some(pico_socketeer::protocol::Response::error("", err_code))
            }
            Ok(None) => None,
            Ok(Some(frame)) => Some(match parse_command(frame) {
                Err(err_code) => pico_socketeer::protocol::Response::error("", err_code),
                Ok(cmd) => match validate_route(&cmd) {
                    Err(r) => r,
                    Ok(route) => dispatch(&cmd, route, &mut device_state),
                },
            }),
        };

        if let Some(resp) = response
            && let Ok(n) = serialize_response(&resp, &mut resp_buf)
            && socket.write_all(&resp_buf[..n]).await.is_err()
        {
            defmt::warn!("TCP write error");
            socket.abort();
            return;
        }

        if device_state.pending_reboot {
            defmt::info!("rebooting to USB bootloader");
            LED_SIGNAL.signal(LedState::Rebooting);
            Timer::after_millis(650).await; // 10 × 100 ms flash cycle + margin
            embassy_rp::rom_data::reset_to_usb_boot(0, 0);
            #[allow(clippy::empty_loop)]
            loop {}
        }
    }
}

// ── AP mode and captive portal ────────────────────────────────────────────────

/// AP provisioning mode — never returns (runs until credentials are saved and watchdog reboots).
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
    let ap_ip = Ipv4Address::new(192, 168, 4, 1);
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
            embassy_net::IpAddress::Ipv4(Ipv4Address::new(0, 0, 0, 0)),
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
                    embassy_net::IpAddress::Ipv4(Ipv4Address::new(255, 255, 255, 255)),
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
    // yiaddr: 192.168.4.2
    r[16] = 192;
    r[17] = 168;
    r[18] = 4;
    r[19] = 2;
    // siaddr: 192.168.4.1
    r[20] = 192;
    r[21] = 168;
    r[22] = 4;
    r[23] = 1;
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
    r[p + 2] = 192;
    r[p + 3] = 168;
    r[p + 4] = 4;
    r[p + 5] = 1;
    p += 6;
    // 51: lease time = 3600s = 0x00000E10
    r[p] = 51;
    r[p + 1] = 4;
    r[p + 2] = 0;
    r[p + 3] = 0;
    r[p + 4] = 0x0E;
    r[p + 5] = 0x10;
    p += 6;
    // 1: subnet mask 255.255.255.0
    r[p] = 1;
    r[p + 1] = 4;
    r[p + 2] = 255;
    r[p + 3] = 255;
    r[p + 4] = 255;
    r[p + 5] = 0;
    p += 6;
    // 3: router 192.168.4.1
    r[p] = 3;
    r[p + 1] = 4;
    r[p + 2] = 192;
    r[p + 3] = 168;
    r[p + 4] = 4;
    r[p + 5] = 1;
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
            tmp[tp] = b'0' + (n % 10) as u8;
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
    let req = match parse_request_line(&headers[..line_end.min(n)]) {
        Ok(r) => r,
        Err(_) => {
            send_http(socket, b"400 Bad Request", b"Bad Request").await;
            return;
        }
    };

    // Captive portal: redirect any request not targeting 192.168.4.1
    if let Some(host) = extract_header(headers, b"Host")
        && !host.starts_with(b"192.168.4.1")
    {
        send_redirect(socket, b"http://192.168.4.1/").await;
        return;
    }

    match (req.method, req.path) {
        (Method::Get, "/") | (Method::Get, "/index.html") => {
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
                    let _ = ssid_buf.push_str(form.ssid);
                    let _ = pwd_buf.push_str(form.password);

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

                    handle_provision(socket, &ssid_buf, &pwd_buf, flash, watchdog).await;
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
            .and_then(|r| r.ok())
            .is_some()
        } else {
            false
        }
    };

    if joined {
        if let Some(creds) = Credentials::new(ssid, password)
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
