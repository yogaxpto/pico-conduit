use crate::fixtures::make_cmd;
use pico_conduit::interfaces::{decode_bytes, require_positive};
use pico_conduit::protocol::{
    ERROR_INVALID_ENCODING, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE,
};

#[test]
fn decode_bytes_none_returns_missing_field() {
    let cmd = make_cmd("t1", None, None);
    let res = decode_bytes(&cmd, None);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn decode_bytes_empty_returns_missing_field() {
    let cmd = make_cmd("t2", None, None);
    let res = decode_bytes(&cmd, Some(""));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn decode_bytes_valid_base64_returns_ok() {
    let cmd = make_cmd("t3", None, None);
    // "qw==" is base64 for [0xAB]
    let res = decode_bytes(&cmd, Some("qw=="));
    assert!(res.is_ok());
    assert_eq!(res.unwrap()[0], 0xAB);
}

#[test]
fn decode_bytes_invalid_base64_returns_invalid_encoding() {
    let cmd = make_cmd("t3b", None, None);
    let res = decode_bytes(&cmd, Some("!!!"));
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().error, Some(ERROR_INVALID_ENCODING));
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
