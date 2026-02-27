//! ADC interface handler.
//!
//! Reads ADC channels 0–2 (GPIO26–28) and the onboard temperature sensor.
//! GPIO29 / ADC channel 3 is reserved (CYW43 SPI DIO) and must never be accessed.
//!
//! The ADC voltage conversion assumes the RP2350's 3.3V reference and 12-bit resolution.

use crate::protocol::{
    AdcChannel, Command, Response, ResponseData, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE,
};

/// ADC reference voltage for the RP2350 (3.3V).
pub const ADC_VREF: f32 = 3.3;
/// ADC resolution: 12-bit (0–4095).
pub const ADC_MAX: f32 = 4095.0;

/// Convert a raw 12-bit ADC reading to voltage.
pub fn raw_to_voltage(raw: u16) -> f32 {
    (raw as f32 / ADC_MAX) * ADC_VREF
}

/// Convert a raw ADC reading from the RP2350 temperature sensor to degrees Celsius.
///
/// Formula from the RP2350 datasheet: T = 27 - (ADC_voltage - 0.706) / 0.001721
pub fn raw_to_celsius(raw: u16) -> f32 {
    let voltage = raw_to_voltage(raw);
    27.0 - (voltage - 0.706) / 0.001721
}

/// Handle an ADC read command with a pre-read raw value.
///
/// In the real firmware, the raw value is read from the RP2350 ADC peripheral before
/// calling this function. For host tests, a mock raw value is provided directly.
pub fn handle_read_with_raw<'a>(cmd: &Command<'a>, raw: u16) -> Response<'a> {
    let channel = match cmd.adc_channel {
        Some(ch) => ch,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
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
pub fn validate_read<'a>(cmd: &Command<'a>) -> Result<AdcChannel, Response<'a>> {
    let channel = match cmd.adc_channel {
        Some(ch) => ch,
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };
    // ADC channel 3 (GPIO29) is reserved — embedded in the reserved-pins list.
    // Channels 0-2 and Temp are valid.
    match channel {
        AdcChannel::Ch0 | AdcChannel::Ch1 | AdcChannel::Ch2 | AdcChannel::Temp => Ok(channel),
    }
}

/// Reject an out-of-range numeric channel (e.g. 3, 4, ...).
pub fn handle_invalid_channel<'a>(cmd: &Command<'a>) -> Response<'a> {
    Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AdcChannel, Command, ERROR_MISSING_FIELD};

    fn make_adc_cmd<'a>(id: &'a str, channel: Option<AdcChannel>) -> Command<'a> {
        Command {
            version: Some(1),
            id,
            interface: Some("adc"),
            action: Some("read"),
            pin: None,
            value: None,
            mode: None,
            pull: None,
            uart: None,
            bytes: None,
            len: None,
            baud: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            spi: None,
            freq_hz: None,
            cpol: None,
            cpha: None,
            i2c: None,
            addr: None,
            write_bytes: None,
            read_len: None,
            channel: None,
            duty_u16: None,
            adc_channel: channel,
        }
    }

    #[test]
    fn adc_channel_0_returns_raw_and_voltage() {
        let cmd = make_adc_cmd("a0", Some(AdcChannel::Ch0));
        let resp = handle_read_with_raw(&cmd, 2048);
        assert!(resp.ok, "adc channel 0 read should succeed");
        match resp.data {
            Some(ResponseData::AdcRead { raw, voltage }) => {
                assert_eq!(raw, 2048);
                // voltage ≈ (2048/4095) * 3.3 ≈ 1.650
                assert!((voltage - 1.650).abs() < 0.01, "voltage should be ~1.65: {voltage}");
            }
            _ => panic!("expected AdcRead data, got {:?}", resp.data),
        }
    }

    #[test]
    fn adc_channel_1_returns_raw_and_voltage() {
        let cmd = make_adc_cmd("a1", Some(AdcChannel::Ch1));
        let resp = handle_read_with_raw(&cmd, 0);
        assert!(resp.ok);
        match resp.data {
            Some(ResponseData::AdcRead { raw, voltage }) => {
                assert_eq!(raw, 0);
                assert!((voltage - 0.0).abs() < 0.001);
            }
            _ => panic!("expected AdcRead"),
        }
    }

    #[test]
    fn adc_channel_2_full_scale() {
        let cmd = make_adc_cmd("a2", Some(AdcChannel::Ch2));
        let resp = handle_read_with_raw(&cmd, 4095);
        assert!(resp.ok);
        match resp.data {
            Some(ResponseData::AdcRead { raw, voltage }) => {
                assert_eq!(raw, 4095);
                assert!((voltage - 3.3).abs() < 0.001, "full scale voltage should be ~3.3V");
            }
            _ => panic!("expected AdcRead"),
        }
    }

    #[test]
    fn adc_temp_returns_celsius() {
        let cmd = make_adc_cmd("at", Some(AdcChannel::Temp));
        // Typical room temperature ADC reading (~27°C corresponds to raw ≈ 876)
        let resp = handle_read_with_raw(&cmd, 876);
        assert!(resp.ok, "adc temp read should succeed");
        match resp.data {
            Some(ResponseData::AdcTemp { celsius }) => {
                // Just verify it's a plausible temperature
                assert!(celsius > -40.0 && celsius < 85.0, "temperature out of range: {celsius}");
            }
            _ => panic!("expected AdcTemp data, got {:?}", resp.data),
        }
    }

    #[test]
    fn adc_missing_channel_returns_missing_field() {
        let cmd = make_adc_cmd("am", None);
        let resp = handle_read_with_raw(&cmd, 0);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn adc_validate_read_ch0() {
        let cmd = make_adc_cmd("v0", Some(AdcChannel::Ch0));
        let result = validate_read(&cmd);
        assert_eq!(result.unwrap(), AdcChannel::Ch0);
    }

    #[test]
    fn adc_validate_read_temp() {
        let cmd = make_adc_cmd("vt", Some(AdcChannel::Temp));
        let result = validate_read(&cmd);
        assert_eq!(result.unwrap(), AdcChannel::Temp);
    }

    #[test]
    fn adc_validate_read_missing_channel_returns_error() {
        let cmd = make_adc_cmd("vm", None);
        let result = validate_read(&cmd);
        let resp = result.unwrap_err();
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn raw_to_voltage_midpoint() {
        let v = raw_to_voltage(2048);
        assert!((v - 1.650).abs() < 0.01, "midpoint voltage: {v}");
    }

    #[test]
    fn raw_to_voltage_zero() {
        assert!((raw_to_voltage(0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn raw_to_voltage_full_scale() {
        assert!((raw_to_voltage(4095) - 3.3).abs() < 0.001);
    }

    #[test]
    fn raw_to_celsius_reasonable_room_temp() {
        // Verify the formula gives a reasonable room temperature
        // At 27°C: voltage = 0.706 (from datasheet) → raw ≈ 876
        let temp = raw_to_celsius(876);
        assert!(temp > 20.0 && temp < 35.0, "room temp should be 20-35°C: {temp}");
    }
}
