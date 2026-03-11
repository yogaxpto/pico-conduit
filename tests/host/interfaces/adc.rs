use pico_socketeer::interfaces::adc::{
    handle_read_with_raw, raw_to_celsius, raw_to_voltage, validate_read,
};
use pico_socketeer::protocol::{AdcChannel, Command, ERROR_MISSING_FIELD, ResponseData};

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
        commands: None,
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
            assert!(
                (voltage - 1.650).abs() < 0.01,
                "voltage should be ~1.65: {voltage}"
            );
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
            assert!(
                (voltage - 3.3).abs() < 0.001,
                "full scale voltage should be ~3.3V"
            );
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
            assert!(
                celsius > -40.0 && celsius < 85.0,
                "temperature out of range: {celsius}"
            );
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
    assert!(
        temp > 20.0 && temp < 35.0,
        "room temp should be 20-35°C: {temp}"
    );
}

// ----- ADC conversion boundary edge cases -----

#[test]
fn raw_to_voltage_single_lsb() {
    // raw=1: voltage = (1/4095) * 3.3 ≈ 0.000806
    let v = raw_to_voltage(1);
    let expected = 3.3 / 4095.0;
    assert!(
        (v - expected).abs() < 0.0001,
        "single LSB voltage should be ~{expected}: {v}"
    );
}

#[test]
fn raw_to_celsius_zero_raw() {
    // raw=0 → voltage=0.0 → celsius = 27 - (0 - 0.706) / 0.001721 ≈ 27 + 410 = ~437
    // Very high temperature (unrealistic, but mathematically correct)
    let temp = raw_to_celsius(0);
    assert!(
        temp > 400.0,
        "raw=0 should give very high celsius (voltage=0 << 0.706): {temp}"
    );
}

#[test]
fn raw_to_celsius_full_scale() {
    // raw=4095 → voltage≈3.3 → celsius = 27 - (3.3 - 0.706) / 0.001721 ≈ 27 - 1508 ≈ -1481
    // Very negative temperature (unrealistic, but mathematically correct)
    let temp = raw_to_celsius(4095);
    assert!(
        temp < -1000.0,
        "raw=4095 should give very negative celsius (voltage=3.3 >> 0.706): {temp}"
    );
}
