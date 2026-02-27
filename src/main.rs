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

mod net;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("pico-socketeer starting");

    let p = embassy_rp::init(Default::default());

    // Signal Booting LED state before anything else.
    // net::start will drive the LED inline via cyw43::Control::set_led.
    net::LED_SIGNAL.signal(pico_socketeer::led::LedState::Booting);

    net::start(spawner, p).await;
}
