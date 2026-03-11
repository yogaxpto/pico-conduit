//! ADC interface handler.
//!
//! Reads ADC channels 0–2 (GPIO26–28) and the onboard temperature sensor.
//! GPIO29 / ADC channel 3 is reserved (CYW43 SPI DIO) and must never be accessed.
//!
//! The ADC voltage conversion assumes the RP2350's 3.3V reference and 12-bit resolution.

use crate::protocol::{
    AdcChannel, Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE, Response, ResponseData,
};

/// ADC reference voltage for the RP2350 (3.3V).
pub const ADC_VREF: f32 = 3.3;
/// ADC resolution: 12-bit (0–4095).
pub const ADC_MAX: f32 = 4095.0;

/// Convert a raw 12-bit ADC reading to voltage.
pub fn raw_to_voltage(raw: u16) -> f32 {
    (f32::from(raw) / ADC_MAX) * ADC_VREF
}

/// Convert a raw ADC reading from the RP2350 temperature sensor to degrees Celsius.
///
/// Formula from the RP2350 datasheet: T = 27 - (ADC_voltage - 0.706) / 0.001721
pub fn raw_to_celsius(raw: u16) -> f32 {
    let voltage = raw_to_voltage(raw);
    27.0 - (voltage - 0.706) / 0.001_721
}

/// Handle an ADC read command with a pre-read raw value.
///
/// In the real firmware, the raw value is read from the RP2350 ADC peripheral before
/// calling this function. For host tests, a mock raw value is provided directly.
pub fn handle_read_with_raw<'a>(cmd: &Command<'a>, raw: u16) -> Response<'a> {
    let channel = match validate_read(cmd) {
        Ok(ch) => ch,
        Err(r) => return r,
    };

    match channel {
        AdcChannel::Ch0 | AdcChannel::Ch1 | AdcChannel::Ch2 => {
            let voltage = raw_to_voltage(raw);
            Response::ok(cmd.id, Some(ResponseData::AdcRead { raw, voltage }))
        }
        AdcChannel::Temp => {
            let celsius = raw_to_celsius(raw);
            Response::ok(cmd.id, Some(ResponseData::AdcTemp { celsius }))
        }
    }
}

/// Validate an ADC read command without performing the actual read.
///
/// Returns `Err` with an error response if the channel is invalid.
/// Returns `Ok(channel)` if the command is well-formed.
pub const fn validate_read<'a>(cmd: &Command<'a>) -> Result<AdcChannel, Response<'a>> {
    let Some(channel) = cmd.adc_channel else {
        return Err(Response::error(cmd.id, ERROR_MISSING_FIELD));
    };
    // ADC channel 3 (GPIO29) is reserved — embedded in the reserved-pins list.
    // Channels 0-2 and Temp are valid.
    match channel {
        AdcChannel::Ch0 | AdcChannel::Ch1 | AdcChannel::Ch2 | AdcChannel::Temp => Ok(channel),
    }
}

/// Reject an out-of-range numeric channel (e.g. 3, 4, ...).
pub const fn handle_invalid_channel<'a>(cmd: &Command<'a>) -> Response<'a> {
    Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)
}
