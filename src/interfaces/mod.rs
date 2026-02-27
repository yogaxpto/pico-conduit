//! Hardware peripheral interface handlers.
//!
//! Each module exposes a `handle` function that takes a parsed [`Command`] and a reference
//! to the peripheral, and returns a [`Response`].
//!
//! All handlers are written against `embedded-hal` / `embedded-hal-async` traits so they
//! can be tested on the host with `embedded-hal-mock` (Tier 2 tests).

pub mod adc;
pub mod gpio;
pub mod i2c;
pub mod pwm;
pub mod spi;
pub mod uart;
pub mod usb;

/// RP2350 GPIO pins reserved for internal use — must never be exposed to client commands.
///
/// | Pin | Reserved for |
/// |-----|-------------|
/// | 23  | BOOTSEL button (active low) |
/// | 24  | CYW43 WL_ON |
/// | 25  | CYW43 SPI CLK |
/// | 26  | CYW43 SPI MOSI |
/// | 27  | CYW43 SPI MISO |
/// | 28  | CYW43 SPI CS |
/// | 29  | CYW43 SPI DIO (also ADC Ch3 — unavailable) |
pub const RESERVED_PINS: &[u8] = &[23, 24, 25, 26, 27, 28, 29];

/// Returns `true` if the given GPIO pin number is available for user commands.
pub fn is_pin_available(pin: u8) -> bool {
    pin <= 29 && !RESERVED_PINS.contains(&pin)
}
