use pico_socketeer::interfaces::spi::{handle_configure, handle_transfer, handle_write};
use pico_socketeer::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE, ResponseData,
};

fn make_spi_cmd<'a>(id: &'a str, action: &'a str, spi: Option<u8>) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface: Some("spi"),
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
        spi,
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
    }
}

#[test]
fn spi_configure_valid() {
    let mut cmd = make_spi_cmd("c1", "configure", Some(0));
    cmd.freq_hz = Some(1_000_000);
    cmd.cpol = Some(0);
    cmd.cpha = Some(0);
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.freq_hz, 1_000_000);
    assert_eq!(cfg.cpol, 0);
    assert_eq!(cfg.cpha, 0);
    assert!(cfg.configured);
}

#[test]
fn spi_configure_missing_freq_returns_missing_field() {
    let cmd = make_spi_cmd("c2", "configure", Some(0));
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn spi_configure_invalid_spi_index() {
    let mut cmd = make_spi_cmd("c3", "configure", Some(2));
    cmd.freq_hz = Some(1_000_000);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn spi_configure_invalid_cpol() {
    let mut cmd = make_spi_cmd("c4", "configure", Some(0));
    cmd.freq_hz = Some(1_000_000);
    cmd.cpol = Some(2); // invalid
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn spi_transfer_configured_returns_miso_bytes() {
    let mut cmd = make_spi_cmd("t1", "transfer", Some(0));
    cmd.bytes = Some("3q0="); // base64 for [0xDE, 0xAD]
    let miso = [0xBE, 0xEF];
    let resp = handle_transfer(&cmd, true, &miso);
    assert!(resp.ok, "transfer should succeed: {:?}", resp.error);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.0.as_slice(), &[0xBE, 0xEF]);
        }
        _ => panic!("expected Bytes response"),
    }
}

#[test]
fn spi_transfer_unconfigured_returns_not_configured() {
    let mut cmd = make_spi_cmd("t2", "transfer", Some(0));
    cmd.bytes = Some("AA=="); // base64 for [0x00]
    let resp = handle_transfer(&cmd, false, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn spi_transfer_missing_bytes_returns_missing_field() {
    let cmd = make_spi_cmd("t3", "transfer", Some(0));
    let resp = handle_transfer(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn spi_write_configured_succeeds() {
    let mut cmd = make_spi_cmd("w1", "write", Some(1));
    cmd.bytes = Some("qw=="); // base64 for [0xAB]
    let resp = handle_write(&cmd, true);
    assert!(resp.ok, "spi write should succeed");
}

#[test]
fn spi_write_missing_bytes_returns_missing_field() {
    let cmd = make_spi_cmd("w3", "write", Some(0));
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn spi_write_unconfigured_returns_not_configured() {
    let mut cmd = make_spi_cmd("w2", "write", Some(0));
    cmd.bytes = Some("AA=="); // base64 for [0x00]
    let resp = handle_write(&cmd, false);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

// ----- SPI validation edge cases -----

#[test]
fn spi_configure_freq_zero_returns_error() {
    let mut cmd = make_spi_cmd("ec1", "configure", Some(0));
    cmd.freq_hz = Some(0);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn spi_configure_invalid_cpha() {
    let mut cmd = make_spi_cmd("ec2", "configure", Some(0));
    cmd.freq_hz = Some(1_000_000);
    cmd.cpha = Some(2); // only 0 or 1 valid
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn spi_transfer_empty_bytes_returns_missing_field() {
    let mut cmd = make_spi_cmd("ec3", "transfer", Some(0));
    cmd.bytes = Some(""); // empty
    let resp = handle_transfer(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn spi_write_empty_bytes_returns_missing_field() {
    let mut cmd = make_spi_cmd("ec5", "write", Some(0));
    cmd.bytes = Some(""); // empty
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn spi_transfer_mosi_longer_than_miso() {
    // MOSI has 4 bytes, MISO only 2 — response should have min(4, 2) = 2 bytes
    let mut cmd = make_spi_cmd("ec4", "transfer", Some(0));
    cmd.bytes = Some("AQIDBA=="); // base64 for [0x01, 0x02, 0x03, 0x04]
    let miso = [0xAA, 0xBB];
    let resp = handle_transfer(&cmd, true, &miso);
    assert!(resp.ok);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.0.len(), 2, "should return min(MOSI, MISO) bytes");
            assert_eq!(bytes.0.as_slice(), &[0xAA, 0xBB]);
        }
        _ => panic!("expected Bytes response"),
    }
}
