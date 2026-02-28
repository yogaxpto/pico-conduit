use pico_socketeer::interfaces::pwm::{
    MAX_PWM_CHANNEL, handle_disable, handle_enable, handle_set_duty, handle_set_freq,
    validate_channel,
};
use pico_socketeer::protocol::{Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE};

fn make_pwm_cmd<'a>(
    id: &'a str,
    action: &'a str,
    channel: Option<u8>,
    duty_u16: Option<u16>,
    freq_hz: Option<u32>,
) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface: Some("pwm"),
        action: Some(action),
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
        freq_hz,
        cpol: None,
        cpha: None,
        i2c: None,
        addr: None,
        write_bytes: None,
        read_len: None,
        channel,
        duty_u16,
        adc_channel: None,
    }
}

#[test]
fn pwm_set_duty_succeeds() {
    let cmd = make_pwm_cmd("d1", "set_duty", Some(0), Some(32768), None);
    let resp = handle_set_duty(&cmd);
    assert!(resp.ok, "set_duty should succeed: {:?}", resp.error);
}

#[test]
fn pwm_set_duty_zero_always_off() {
    let cmd = make_pwm_cmd("d2", "set_duty", Some(3), Some(0), None);
    let resp = handle_set_duty(&cmd);
    assert!(resp.ok, "duty=0 (always off) should succeed");
}

#[test]
fn pwm_set_duty_max_always_on() {
    let cmd = make_pwm_cmd("d3", "set_duty", Some(3), Some(65535), None);
    let resp = handle_set_duty(&cmd);
    assert!(resp.ok, "duty=65535 (always on) should succeed");
}

#[test]
fn pwm_set_duty_missing_channel_returns_missing_field() {
    let cmd = make_pwm_cmd("d4", "set_duty", None, Some(32768), None);
    let resp = handle_set_duty(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn pwm_set_duty_missing_duty_returns_missing_field() {
    let cmd = make_pwm_cmd("d5", "set_duty", Some(0), None, None);
    let resp = handle_set_duty(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn pwm_set_duty_channel_out_of_range() {
    let cmd = make_pwm_cmd("d6", "set_duty", Some(16), Some(0), None); // max is 15
    let resp = handle_set_duty(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn pwm_set_freq_succeeds() {
    let cmd = make_pwm_cmd("f1", "set_freq", Some(0), None, Some(1000));
    let resp = handle_set_freq(&cmd);
    assert!(resp.ok, "set_freq 1kHz should succeed: {:?}", resp.error);
}

#[test]
fn pwm_set_freq_zero_returns_out_of_range() {
    let cmd = make_pwm_cmd("f2", "set_freq", Some(0), None, Some(0));
    let resp = handle_set_freq(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn pwm_set_freq_missing_freq_returns_missing_field() {
    let cmd = make_pwm_cmd("f3", "set_freq", Some(0), None, None);
    let resp = handle_set_freq(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn pwm_enable_succeeds() {
    let cmd = make_pwm_cmd("e1", "enable", Some(7), None, None);
    let resp = handle_enable(&cmd);
    assert!(resp.ok);
}

#[test]
fn pwm_enable_missing_channel() {
    let cmd = make_pwm_cmd("e2", "enable", None, None, None);
    let resp = handle_enable(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn pwm_disable_succeeds() {
    let cmd = make_pwm_cmd("x1", "disable", Some(1), None, None);
    let resp = handle_disable(&cmd);
    assert!(resp.ok);
}

#[test]
fn pwm_disable_channel_out_of_range() {
    let cmd = make_pwm_cmd("x2", "disable", Some(255), None, None);
    let resp = handle_disable(&cmd);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn pwm_validate_channel_zero() {
    let cmd = make_pwm_cmd("vc", "enable", Some(0), None, None);
    assert_eq!(validate_channel(&cmd).unwrap(), 0);
}

#[test]
fn pwm_validate_channel_max() {
    let cmd = make_pwm_cmd("vc", "enable", Some(MAX_PWM_CHANNEL), None, None);
    assert_eq!(validate_channel(&cmd).unwrap(), MAX_PWM_CHANNEL);
}
