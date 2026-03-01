use crate::fixtures::make_cmd;
use pico_socketeer::interfaces::{require_bytes, require_positive};
use pico_socketeer::protocol::{ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE};

#[test]
fn require_bytes_none_returns_missing_field() {
    let cmd = make_cmd("t1", None, None);
    let res = require_bytes(&cmd, None);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn require_bytes_empty_returns_missing_field() {
    let cmd = make_cmd("t2", None, None);
    let empty: heapless::Vec<u8, 64> = heapless::Vec::new();
    let res = require_bytes(&cmd, Some(&empty));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn require_bytes_nonempty_returns_ok() {
    let cmd = make_cmd("t3", None, None);
    let mut v: heapless::Vec<u8, 64> = heapless::Vec::new();
    v.push(0xAB).unwrap();
    let res = require_bytes(&cmd, Some(&v));
    assert!(res.is_ok());
    assert_eq!(res.unwrap()[0], 0xAB);
}

#[test]
fn require_positive_none_returns_missing_field() {
    let cmd = make_cmd("t4", None, None);
    let res = require_positive(&cmd, None);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn require_positive_zero_returns_out_of_range() {
    let cmd = make_cmd("t5", None, None);
    let res = require_positive(&cmd, Some(0));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_VALUE_OUT_OF_RANGE));
}

#[test]
fn require_positive_positive_returns_ok() {
    let cmd = make_cmd("t6", None, None);
    let res = require_positive(&cmd, Some(8));
    assert_eq!(res.unwrap(), 8);
}
