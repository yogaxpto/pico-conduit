//! Wi-Fi connectivity, TCP socket server, and LED control.
//!
//! Embedded-only module (not compiled during `cargo test --lib`).
//!
//! Architecture: a single `net_main_task` owns the `cyw43::Control` handle and drives
//! both the LED (via `cyw43::Control::gpio_set`) and Wi-Fi (via `cyw43::Control::join` /
//! `set_power_management`). This avoids the need to share `Control` between tasks.
//!
//! The `LED_SIGNAL` static is defined here and exported to `main.rs`.

use cyw43::JoinOptions;
use embassy_executor::Spawner;
use embassy_net::{Config, IpListenEndpoint, Stack, StackResources, tcp::TcpSocket};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::pio::Pio;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer, with_timeout};
use embedded_io_async::Write as _;
use static_cell::StaticCell;

use pico_socketeer::led::{LedState, SOS_TIMING};
use pico_socketeer::protocol::{FrameReader, MAX_MSG_LEN, parse_command, serialize_response};
use pico_socketeer::provisioning::storage::load_credentials;
use pico_socketeer::router::{DeviceState, dispatch, validate_route};

// ---- CYW43 firmware blobs ----
const CYW43_FW: &[u8] = include_bytes!("../cyw43-firmware/43439A0.bin");
const CYW43_CLM: &[u8] = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

// ---- Configuration constants ----
pub const TCP_PORT: u16 = 4242;
const TCP_READ_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RECONNECT_SECS: u64 = 600; // 10 minutes

// ---- Static storage (no heap) ----
static STACK_RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();

// ---- LED signal ----
/// Global LED state signal. Signal `LedState::*` from any context to update the LED.
pub static LED_SIGNAL: Signal<CriticalSectionRawMutex, LedState> = Signal::new();

// ---- Interrupt bindings ----
embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
});

// ---- Task type aliases ----
type CywSpi =
    cyw43_pio::PioSpi<'static, embassy_rp::peripherals::PIO0, 0, embassy_rp::peripherals::DMA_CH0>;
type CywRunner = cyw43::Runner<'static, Output<'static>, CywSpi>;

// ---- Background tasks ----

/// CYW43 driver background task (must be spawned before Wi-Fi operations).
#[embassy_executor::task]
async fn cyw43_task(runner: CywRunner) -> ! {
    runner.run().await
}

/// Embassy-net stack polling task.
#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

// ---- LED helper ----

/// Drive the LED for one cycle of the current state.
/// Returns `true` if a new signal was received mid-pattern (for SOS early-exit).
async fn drive_led(control: &mut cyw43::Control<'_>, state: &LedState) -> bool {
    match state {
        LedState::Booting => {
            for _ in 0..3u8 {
                control.gpio_set(0, true).await;
                Timer::after_millis(100).await;
                control.gpio_set(0, false).await;
                Timer::after_millis(100).await;
            }
            Timer::after_millis(1000).await;
        }
        LedState::Provisioning => {
            control.gpio_set(0, true).await;
            Timer::after_secs(1).await;
            control.gpio_set(0, false).await;
            Timer::after_secs(1).await;
        }
        LedState::Scanning => {
            for _ in 0..2u8 {
                control.gpio_set(0, true).await;
                Timer::after_millis(100).await;
                control.gpio_set(0, false).await;
                Timer::after_millis(100).await;
            }
            Timer::after_millis(700).await;
        }
        LedState::Connecting => {
            control.gpio_set(0, true).await;
            Timer::after_millis(100).await;
            control.gpio_set(0, false).await;
            Timer::after_millis(100).await;
        }
        LedState::Connected => {
            control.gpio_set(0, true).await;
            // Solid ON — the signal will change state when something happens
            Timer::after_millis(100).await;
        }
        LedState::Reconnecting => {
            control.gpio_set(0, true).await;
            Timer::after_millis(250).await;
            control.gpio_set(0, false).await;
            Timer::after_millis(250).await;
        }
        LedState::Error => {
            for (on, ms) in SOS_TIMING {
                control.gpio_set(0, *on).await;
                Timer::after_millis(*ms).await;
            }
            return LED_SIGNAL.signaled();
        }
        LedState::Saving => {
            for _ in 0..5u8 {
                control.gpio_set(0, true).await;
                Timer::after_millis(100).await;
                control.gpio_set(0, false).await;
                Timer::after_millis(100).await;
            }
            control.gpio_set(0, false).await;
        }
    }
    false
}

// ---- Main initialization ----

/// Reconnect backoff: 5s → 10s → 20s → 40s → 60s (capped).
fn backoff_duration(attempt: u32) -> Duration {
    match attempt {
        0 => Duration::from_secs(5),
        1 => Duration::from_secs(10),
        2 => Duration::from_secs(20),
        3 => Duration::from_secs(40),
        _ => Duration::from_secs(60),
    }
}

/// Main network initialization: set up CYW43, spawn tasks, start TCP server.
pub async fn start(spawner: Spawner, p: embassy_rp::Peripherals) {
    // Initialize CYW43 via PIO-based SPI
    let pwr = Output::new(p.PIN_23, Level::Low);
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
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, CYW43_FW).await;

    spawner.must_spawn(cyw43_task(runner));
    control.init(CYW43_CLM).await;

    // Drive the Booting LED pattern
    drive_led(&mut control, &LedState::Booting).await;

    // Check credentials
    let flash_creds = load_credentials();

    // Check compile-time overrides (Phase 6a development convenience)
    let compile_ssid = option_env!("PICO_WIFI_SSID");
    let compile_pass = option_env!("PICO_WIFI_PASS");

    if let (Some(ssid), Some(pass)) = (compile_ssid, compile_pass) {
        defmt::info!("Using compile-time credentials");
        sta_mode(spawner, net_device, &mut control, ssid, pass).await;
    } else if let Some(creds) = flash_creds {
        defmt::info!("Using flash credentials");
        sta_mode(
            spawner,
            net_device,
            &mut control,
            &creds.ssid,
            &creds.password,
        )
        .await;
    } else {
        defmt::warn!("No credentials — provisioning mode (stub)");
        drive_led(&mut control, &LedState::Provisioning).await;
        loop {
            drive_led(&mut control, &LedState::Provisioning).await;
        }
    }
}

async fn sta_mode(
    spawner: Spawner,
    net_device: cyw43::NetDriver<'static>,
    control: &mut cyw43::Control<'_>,
    ssid: &str,
    password: &str,
) {
    // Set up network stack with DHCP
    let config = Config::dhcpv4(Default::default());
    let seed = 0x_dead_beef_cafe_babe_u64;
    let resources = STACK_RESOURCES.init(StackResources::new());
    let (stack, net_runner) = embassy_net::new(net_device, config, resources, seed);
    spawner.must_spawn(net_task(net_runner));

    // Join Wi-Fi
    defmt::info!("Joining Wi-Fi: {}", ssid);
    loop {
        drive_led(control, &LedState::Connecting).await;
        match control
            .join(ssid, JoinOptions::new(password.as_bytes()))
            .await
        {
            Ok(()) => break,
            Err(e) => {
                defmt::warn!("Join failed: status {}", e.status);
                Timer::after_secs(3).await;
            }
        }
    }

    // Wait for DHCP
    stack.wait_config_up().await;
    if let Some(cfg) = stack.config_v4() {
        defmt::info!("IP: {}", cfg.address);
    }

    // Enable power save after STA join
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    defmt::info!("Listening on TCP port {}", TCP_PORT);
    tcp_server(stack, control).await;
}

async fn tcp_server(stack: Stack<'_>, control: &mut cyw43::Control<'_>) {
    let mut rx_buf = [0u8; MAX_MSG_LEN];
    let mut tx_buf = [0u8; MAX_MSG_LEN];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);

    let mut reconnect_attempt: u32 = 0;
    let mut total_offline_secs: u64 = 0;

    loop {
        // Drive Connected LED (solid ON) while waiting
        drive_led(control, &LedState::Connected).await;

        defmt::info!("accept() on port {}", TCP_PORT);
        match socket
            .accept(IpListenEndpoint {
                addr: None,
                port: TCP_PORT,
            })
            .await
        {
            Ok(()) => {
                // Disable PM while client is active
                control
                    .set_power_management(cyw43::PowerManagementMode::None)
                    .await;
                defmt::info!("Client connected");
                reconnect_attempt = 0;
                total_offline_secs = 0;

                handle_client(&mut socket).await;

                // Re-enable PM
                control
                    .set_power_management(cyw43::PowerManagementMode::PowerSave)
                    .await;
                defmt::info!("Client disconnected");
            }
            Err(e) => {
                defmt::warn!("accept() error: {:?}", e);
                socket.abort();

                let backoff = backoff_duration(reconnect_attempt);
                let secs = backoff.as_secs();
                total_offline_secs = total_offline_secs.saturating_add(secs);

                if total_offline_secs >= MAX_RECONNECT_SECS {
                    defmt::error!("Connection failed 10 min — SOS");
                    loop {
                        drive_led(control, &LedState::Error).await;
                    }
                }

                defmt::warn!("reconnect attempt {} after {}s", reconnect_attempt, secs);
                drive_led(control, &LedState::Reconnecting).await;
                Timer::after(backoff).await;
                reconnect_attempt = reconnect_attempt.saturating_add(1);
            }
        }
    }
}

async fn handle_client(socket: &mut TcpSocket<'_>) {
    let mut frame_reader = FrameReader::new();
    let mut resp_buf = [0u8; MAX_MSG_LEN];
    let mut byte_buf = [0u8; 1];
    let mut device_state = DeviceState::default();

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
    }
}
