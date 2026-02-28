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

use crate::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE, Response,
    ResponseData,
};

/// Validate an `Option<u8>` peripheral index field, returning [`ERROR_MISSING_FIELD`] if
/// `None` or [`ERROR_VALUE_OUT_OF_RANGE`] if the value exceeds `max`.
pub fn validate_index<'a>(
    cmd: &Command<'a>,
    field: Option<u8>,
    max: u8,
) -> Result<u8, Response<'a>> {
    let idx = match field {
        Some(v) => v,
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };
    if idx > max {
        return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE));
    }
    Ok(idx)
}

/// Guard that returns [`ERROR_NOT_CONFIGURED`] when a peripheral has not been configured.
pub fn check_configured<'a>(cmd: &Command<'a>, configured: bool) -> Result<(), Response<'a>> {
    if configured {
        Ok(())
    } else {
        Err(Response::error(cmd.id, ERROR_NOT_CONFIGURED))
    }
}

/// Build a [`ResponseData::Bytes`] response, capping at `max_len` and the 64-byte heapless limit.
pub fn bytes_response<'a>(id: &'a str, data: &[u8], max_len: usize) -> Response<'a> {
    let take = max_len.min(data.len()).min(64);
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(&data[..take]).ok();
    Response::ok(id, Some(ResponseData::Bytes { bytes }))
}
