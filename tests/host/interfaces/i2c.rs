use pico_socketeer::interfaces::i2c::{
    handle_configure, handle_read, handle_write, handle_write_read,
};
use pico_socketeer::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE, ResponseData,
};

fn make_i2c_cmd<'a>(
    id: &'a str,
    action: &'a str,
    i2c: Option<u8>,
    addr: Option<u8>,
) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface: Some("i2c"),
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
        i2c,
        addr,
        write_bytes: None,
        read_len: None,
        channel: None,
        duty_u16: None,
        adc_channel: None,
    }
}

#[test]
fn i2c_configure_100khz() {
    let mut cmd = make_i2c_cmd("c1", "configure", Some(0), None);
    cmd.freq_hz = Some(100_000);
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.freq_hz, 100_000);
    assert!(cfg.configured);
}

#[test]
fn i2c_configure_400khz() {
    let mut cmd = make_i2c_cmd("c2", "configure", Some(1), None);
    cmd.freq_hz = Some(400_000);
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.freq_hz, 400_000);
}

#[test]
fn i2c_configure_invalid_freq() {
    let mut cmd = make_i2c_cmd("c3", "configure", Some(0), None);
    cmd.freq_hz = Some(200_000); // not 100k or 400k
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn i2c_configure_missing_freq() {
    let cmd = make_i2c_cmd("c4", "configure", Some(0), None);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn i2c_configure_invalid_i2c_index() {
    let mut cmd = make_i2c_cmd("c5", "configure", Some(2), None);
    cmd.freq_hz = Some(100_000);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn i2c_read_configured_returns_bytes() {
    let mut cmd = make_i2c_cmd("r1", "read", Some(0), Some(0x48));
    cmd.len = Some(2);
    let rx = [0x0F, 0x42];
    let resp = handle_read(&cmd, true, &rx);
    assert!(resp.ok, "i2c read should succeed: {:?}", resp.error);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.0.as_slice(), &[0x0F, 0x42]);
        }
        _ => panic!("expected Bytes"),
    }
}

#[test]
fn i2c_read_unconfigured_returns_not_configured() {
    let mut cmd = make_i2c_cmd("r2", "read", Some(0), Some(0x48));
    cmd.len = Some(1);
    let resp = handle_read(&cmd, false, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn i2c_read_missing_addr_returns_missing_field() {
    let mut cmd = make_i2c_cmd("r3", "read", Some(0), None);
    cmd.len = Some(1);
    let resp = handle_read(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn i2c_write_configured_succeeds() {
    let mut cmd = make_i2c_cmd("w1", "write", Some(0), Some(0x20));
    cmd.bytes = Some("qg=="); // base64 for [0xAA]
    let resp = handle_write(&cmd, true);
    assert!(resp.ok, "i2c write should succeed: {:?}", resp.error);
}

#[test]
fn i2c_write_unconfigured_returns_not_configured() {
    let mut cmd = make_i2c_cmd("w2", "write", Some(0), Some(0x20));
    cmd.bytes = Some("AA=="); // base64 for [0x00]
    let resp = handle_write(&cmd, false);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn i2c_write_read_configured_returns_read_bytes() {
    let mut cmd = make_i2c_cmd("wr1", "write_read", Some(0), Some(0x68));
    cmd.write_bytes = Some("AA=="); // base64 for [0x00] (register address)
    cmd.read_len = Some(2);
    let rx = [0x12, 0x34];
    let resp = handle_write_read(&cmd, true, &rx);
    assert!(resp.ok, "write_read should succeed: {:?}", resp.error);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.0.as_slice(), &[0x12, 0x34]);
        }
        _ => panic!("expected Bytes"),
    }
}

#[test]
fn i2c_write_read_missing_write_bytes() {
    let mut cmd = make_i2c_cmd("wr2", "write_read", Some(0), Some(0x68));
    cmd.read_len = Some(2);
    let resp = handle_write_read(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

// ----- I2C validation edge cases -----

#[test]
fn i2c_write_missing_addr_returns_missing_field() {
    let mut cmd = make_i2c_cmd("ec6", "write", Some(0), None);
    cmd.bytes = Some("AA==");
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn i2c_write_read_missing_addr_returns_missing_field() {
    let mut cmd = make_i2c_cmd("ec7", "write_read", Some(0), None);
    cmd.write_bytes = Some("AA==");
    cmd.read_len = Some(2);
    let resp = handle_write_read(&cmd, true, &[0x00, 0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn i2c_configure_freq_zero_returns_error() {
    let mut cmd = make_i2c_cmd("ec1", "configure", Some(0), None);
    cmd.freq_hz = Some(0);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn i2c_write_empty_bytes_returns_missing_field() {
    let mut cmd = make_i2c_cmd("ec2", "write", Some(0), Some(0x20));
    cmd.bytes = Some(""); // empty
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn i2c_read_len_zero_returns_error() {
    let mut cmd = make_i2c_cmd("ec3", "read", Some(0), Some(0x48));
    cmd.len = Some(0);
    let resp = handle_read(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn i2c_write_read_read_len_zero_returns_error() {
    let mut cmd = make_i2c_cmd("ec4", "write_read", Some(0), Some(0x68));
    cmd.write_bytes = Some("AA=="); // base64 for [0x00]
    cmd.read_len = Some(0);
    let resp = handle_write_read(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn i2c_read_caps_at_payload_limit() {
    let mut cmd = make_i2c_cmd("ec5", "read", Some(0), Some(0x48));
    cmd.len = Some(600);
    let data = [0xBB; 600]; // more than MAX_PAYLOAD_LEN available
    let resp = handle_read(&cmd, true, &data);
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
