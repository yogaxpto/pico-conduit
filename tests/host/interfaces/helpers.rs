use pico_socketeer::interfaces::{require_bytes, require_positive};
use pico_socketeer::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE,
};

fn dummy_cmd(id: &str) -> Command<'_> {
    Command {
        version: Some(1),
        id,
        interface: None,
        action: None,
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
    }
}

#[test]
fn require_bytes_none_returns_missing_field() {
    let cmd = dummy_cmd("t1");
    let res = require_bytes(&cmd, None);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn require_bytes_empty_returns_missing_field() {
    let cmd = dummy_cmd("t2");
    let empty: heapless::Vec<u8, 64> = heapless::Vec::new();
    let res = require_bytes(&cmd, Some(&empty));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn require_bytes_nonempty_returns_ok() {
    let cmd = dummy_cmd("t3");
    let mut v: heapless::Vec<u8, 64> = heapless::Vec::new();
    v.push(0xAB).unwrap();
    let res = require_bytes(&cmd, Some(&v));
    assert!(res.is_ok());
    assert_eq!(res.unwrap()[0], 0xAB);
}

#[test]
fn require_positive_none_returns_missing_field() {
    let cmd = dummy_cmd("t4");
    let res = require_positive(&cmd, None);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn require_positive_zero_returns_out_of_range() {
    let cmd = dummy_cmd("t5");
    let res = require_positive(&cmd, Some(0));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn require_positive_positive_returns_ok() {
    let cmd = dummy_cmd("t6");
    let res = require_positive(&cmd, Some(8));
    assert_eq!(res.unwrap(), 8);
}
