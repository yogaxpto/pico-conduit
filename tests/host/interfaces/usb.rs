use pico_socketeer::interfaces::usb::{handle_read_with_data, handle_write};
use pico_socketeer::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE, ResponseData,
};

fn make_usb_cmd<'a>(id: &'a str, action: &'a str) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface: Some("usb"),
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
        freq_hz: None,
        cpol: None,
        cpha: None,
        i2c: None,
        addr: None,
        write_bytes: None,
        read_len: None,
        channel: None,
        duty_u16: None,
        adc_channel: None,
        interval_ms: None,
        trigger: None,
        commands: None,
    }
}

#[test]
fn usb_write_configured_succeeds() {
    let mut cmd = make_usb_cmd("w1", "write");
    cmd.bytes = Some("SGk="); // base64 for b"Hi"
    let resp = handle_write(&cmd, true);
    assert!(resp.ok, "usb write should succeed: {:?}", resp.error);
}

#[test]
fn usb_write_unconfigured_returns_not_configured() {
    let mut cmd = make_usb_cmd("w2", "write");
    cmd.bytes = Some("AA=="); // base64 for [0x00]
    let resp = handle_write(&cmd, false);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn usb_write_missing_bytes_returns_missing_field() {
    let cmd = make_usb_cmd("w3", "write");
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn usb_read_configured_returns_bytes() {
    let mut cmd = make_usb_cmd("r1", "read");
    cmd.len = Some(3);
    let rx = [0x41, 0x42, 0x43]; // "ABC"
    let resp = handle_read_with_data(&cmd, true, &rx);
    assert!(resp.ok, "usb read should succeed: {:?}", resp.error);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.0.as_slice(), &[0x41, 0x42, 0x43]);
        }
        _ => panic!("expected Bytes"),
    }
}

#[test]
fn usb_read_unconfigured_returns_not_configured() {
    let mut cmd = make_usb_cmd("r2", "read");
    cmd.len = Some(1);
    let resp = handle_read_with_data(&cmd, false, &[]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn usb_read_missing_len_returns_missing_field() {
    let cmd = make_usb_cmd("r3", "read");
    let resp = handle_read_with_data(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

// ----- USB validation edge cases -----

#[test]
fn usb_read_len_zero_returns_error() {
    let mut cmd = make_usb_cmd("ec1", "read");
    cmd.len = Some(0);
    let resp = handle_read_with_data(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn usb_write_empty_bytes_returns_missing_field() {
    let mut cmd = make_usb_cmd("ec2", "write");
    cmd.bytes = Some(""); // empty
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn usb_read_caps_at_payload_limit() {
    let mut cmd = make_usb_cmd("ec3", "read");
    cmd.len = Some(600);
    let data = [0xCC; 600]; // more than MAX_PAYLOAD_LEN available
    let resp = handle_read_with_data(&cmd, true, &data);
    assert!(resp.ok);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(
                bytes.0.len(),
                pico_socketeer::protocol::MAX_PAYLOAD_LEN,
                "read should be capped at MAX_PAYLOAD_LEN bytes"
            );
        }
        _ => panic!("expected Bytes response"),
    }
}
