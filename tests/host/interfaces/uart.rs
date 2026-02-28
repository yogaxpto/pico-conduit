use pico_socketeer::interfaces::uart::{
    UartParity, handle_configure, handle_read_with_data, handle_write,
};
use pico_socketeer::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE, ResponseData,
};

fn make_uart_cmd<'a>(id: &'a str, action: &'a str, uart: Option<u8>) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface: Some("uart"),
        action: Some(action),
        pin: None,
        value: None,
        mode: None,
        pull: None,
        uart,
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
    }
}

#[test]
fn uart_configure_valid() {
    let mut cmd = make_uart_cmd("c1", "configure", Some(0));
    cmd.baud = Some(115200);
    cmd.data_bits = Some(8);
    cmd.parity = Some("none");
    cmd.stop_bits = Some(1);
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.baud, 115200);
    assert_eq!(cfg.data_bits, 8);
    assert_eq!(cfg.parity, UartParity::None);
    assert_eq!(cfg.stop_bits, 1);
    assert!(cfg.configured);
}

#[test]
fn uart_configure_invalid_uart_index() {
    let mut cmd = make_uart_cmd("c2", "configure", Some(2)); // UART 0 or 1 only
    cmd.baud = Some(9600);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn uart_configure_missing_baud() {
    let cmd = make_uart_cmd("c3", "configure", Some(0));
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn uart_configure_invalid_data_bits() {
    let mut cmd = make_uart_cmd("c4", "configure", Some(0));
    cmd.baud = Some(9600);
    cmd.data_bits = Some(6); // invalid
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn uart_configure_even_parity() {
    let mut cmd = make_uart_cmd("c5", "configure", Some(1));
    cmd.baud = Some(57600);
    cmd.parity = Some("even");
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.parity, UartParity::Even);
}

#[test]
fn uart_write_unconfigured_returns_not_configured() {
    let mut cmd = make_uart_cmd("w1", "write", Some(0));
    let mut bytes = heapless::Vec::new();
    bytes.push(0x41).ok();
    cmd.bytes = Some(bytes);
    let resp = handle_write(&cmd, false);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn uart_write_configured_succeeds() {
    let mut cmd = make_uart_cmd("w2", "write", Some(0));
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(b"Hi").ok();
    cmd.bytes = Some(bytes);
    let resp = handle_write(&cmd, true);
    assert!(resp.ok, "configured write should succeed: {:?}", resp.error);
}

#[test]
fn uart_write_missing_bytes_returns_missing_field() {
    let cmd = make_uart_cmd("w3", "write", Some(0));
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn uart_read_configured_returns_bytes() {
    let mut cmd = make_uart_cmd("r1", "read", Some(0));
    cmd.len = Some(3);
    let data = [0x48u8, 0x65, 0x6C]; // "Hel"
    let resp = handle_read_with_data(&cmd, true, &data);
    assert!(resp.ok, "configured read should succeed: {:?}", resp.error);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.as_slice(), &[0x48, 0x65, 0x6C]);
        }
        _ => panic!("expected Bytes response"),
    }
}

#[test]
fn uart_read_unconfigured_returns_not_configured() {
    let mut cmd = make_uart_cmd("r2", "read", Some(0));
    cmd.len = Some(4);
    let resp = handle_read_with_data(&cmd, false, &[]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn uart_read_missing_len_returns_missing_field() {
    let cmd = make_uart_cmd("r3", "read", Some(0));
    let resp = handle_read_with_data(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

// ----- UART validation edge cases -----

#[test]
fn uart_configure_baud_zero_returns_error() {
    let mut cmd = make_uart_cmd("ec1", "configure", Some(0));
    cmd.baud = Some(0);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn uart_write_empty_bytes_returns_missing_field() {
    let mut cmd = make_uart_cmd("ec2", "write", Some(0));
    cmd.bytes = Some(heapless::Vec::new()); // empty
    let resp = handle_write(&cmd, true);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn uart_read_len_zero_returns_error() {
    let mut cmd = make_uart_cmd("ec3", "read", Some(0));
    cmd.len = Some(0);
    let resp = handle_read_with_data(&cmd, true, &[0x00]);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn uart_configure_odd_parity() {
    let mut cmd = make_uart_cmd("ec4", "configure", Some(0));
    cmd.baud = Some(9600);
    cmd.parity = Some("odd");
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.parity, UartParity::Odd);
}

#[test]
fn uart_configure_stop_bits_two() {
    let mut cmd = make_uart_cmd("ec5", "configure", Some(0));
    cmd.baud = Some(9600);
    cmd.stop_bits = Some(2);
    let cfg = handle_configure(&cmd).unwrap();
    assert_eq!(cfg.stop_bits, 2);
}

#[test]
fn uart_configure_invalid_stop_bits() {
    let mut cmd = make_uart_cmd("ec6", "configure", Some(0));
    cmd.baud = Some(9600);
    cmd.stop_bits = Some(3);
    let err = handle_configure(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn uart_read_caps_at_64_bytes() {
    let mut cmd = make_uart_cmd("ec7", "read", Some(0));
    cmd.len = Some(100);
    let data = [0xAA; 80]; // more than 64 available
    let resp = handle_read_with_data(&cmd, true, &data);
    assert!(resp.ok);
    match resp.data {
        Some(ResponseData::Bytes { bytes }) => {
            assert_eq!(bytes.len(), 64, "read should be capped at 64 bytes");
        }
        _ => panic!("expected Bytes response"),
    }
}
