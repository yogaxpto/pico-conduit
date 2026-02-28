//! pico-socketeer firmware entry point.
//!
//! All testable logic lives in the library crate (`src/lib.rs` and its modules).
//! The embedded networking glue lives in `src/net.rs`.

#![no_std]
#![no_main]
#![deny(clippy::all)]

// Embedded-only: panic handler and RTT logger.
// Not included in test builds — the test harness provides its own runtime.
#[cfg(not(test))]
use {defmt_rtt as _, panic_probe as _};

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::clocks::ClockConfig;

mod net;

// ── Configuration ─────────────────────────────────────────────────────────────
/// CPU clock at boot (48 MHz, XOSC direct, PLL bypassed) — Phase 5c power management.
const BOOT_CLOCK_HZ: u32 = 48_000_000;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("pico-socketeer starting");

    // Boot at BOOT_CLOCK_HZ (XOSC direct, PLL bypassed) per OBJECTIVE Phase 5c.
    // `system_freq` returns Result; unwrap panics on invalid frequency (48 MHz is valid).
    let clock_cfg = ClockConfig::system_freq(BOOT_CLOCK_HZ).unwrap();
    let p = embassy_rp::init(embassy_rp::config::Config::new(clock_cfg));

    // Spawn the LED task first — it waits on LED_SIGNAL and drives the CYW43 LED GPIO.
    // The LED task blocks until CONTROL_MUTEX is populated in net::start().
    spawner.must_spawn(net::led_task());

    net::start(spawner, p).await;
}
