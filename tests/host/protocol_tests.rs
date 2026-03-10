use pico_socketeer::protocol::{
    AdcChannel, Base64Bytes, ERROR_INVALID_ENCODING, ERROR_INVALID_PIN, ERROR_MALFORMED_JSON,
    ERROR_MISSING_FIELD, ERROR_MISSING_VERSION, ERROR_MSG_TOO_LARGE, ERROR_NOT_CONFIGURED,
    ERROR_PERIPHERAL_BUSY, ERROR_PERIPHERAL_ERROR, ERROR_PIN_IN_USE, ERROR_UNKNOWN_ACTION,
    ERROR_UNKNOWN_INTERFACE, ERROR_UNSUPPORTED_VERSION, ERROR_VALUE_OUT_OF_RANGE,
    ERROR_WEBSOCKET_HANDSHAKE, FrameReader, MAX_MSG_LEN, Response, ResponseData, parse_command,
    serialize_response,
};

// ----- Serialize / Deserialize round-trips -----

#[test]
fn parse_valid_gpio_write_command() {
    let json =
        br#"{"version":1,"id":"abc","interface":"gpio","action":"write","pin":15,"value":1}"#;
    let cmd = parse_command(json).unwrap();
    assert_eq!(cmd.id, "abc");
    assert_eq!(cmd.interface, Some("gpio"));
    assert_eq!(cmd.action, Some("write"));
    assert_eq!(cmd.pin, Some(15));
    assert_eq!(cmd.value, Some(1));
}

#[test]
fn serialize_ok_response() {
    let resp = Response::ok("abc", None);
    let mut buf = [0u8; 128];
    let n = serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"id\":\"abc\""), "missing id: {s}");
    assert!(s.contains("\"ok\":true"), "missing ok: {s}");
    assert!(s.ends_with('\n'), "missing newline: {s:?}");
}

#[test]
fn serialize_error_response() {
    let resp = Response::error("x1", ERROR_INVALID_PIN);
    let mut buf = [0u8; 128];
    let n = serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"ok\":false"), "missing ok:false: {s}");
    assert!(
        s.contains("\"error\":\"invalid_pin\""),
        "missing error: {s}"
    );
}

// ----- Version validation -----

#[test]
fn missing_version_returns_error() {
    let json = br#"{"id":"1","interface":"gpio","action":"read","pin":0}"#;
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_MISSING_VERSION);
}

#[test]
fn unsupported_version_returns_error() {
    let json = br#"{"version":2,"id":"1","interface":"gpio","action":"read","pin":0}"#;
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_UNSUPPORTED_VERSION);
}

#[test]
fn version_zero_returns_unsupported() {
    let json = br#"{"version":0,"id":"1","interface":"gpio","action":"read","pin":0}"#;
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_UNSUPPORTED_VERSION);
}

// ----- Frame size limits -----

#[test]
fn frame_exactly_max_msg_len_is_accepted() {
    // Build a valid JSON command that is exactly MAX_MSG_LEN bytes.
    // Pad "id" value to fill up to MAX_MSG_LEN.
    let base = br#"{"version":1,"id":""#;
    let suffix = br#"","interface":"gpio","action":"read","pin":0}"#;
    let id_len = MAX_MSG_LEN - base.len() - suffix.len();
    let mut buf = [0u8; MAX_MSG_LEN];
    buf[..base.len()].copy_from_slice(base);
    for i in 0..id_len {
        buf[base.len() + i] = b'a';
    }
    buf[base.len() + id_len..base.len() + id_len + suffix.len()].copy_from_slice(suffix);
    assert_eq!(buf.len(), MAX_MSG_LEN);
    // parse_command accepts exactly MAX_MSG_LEN bytes
    // (might fail JSON parse if id content breaks serde, but size check passes)
    let result = parse_command(&buf);
    // Should NOT return msg_too_large — either ok or other parse error
    assert!(
        result != Err(ERROR_MSG_TOO_LARGE),
        "{MAX_MSG_LEN}-byte frame should not return msg_too_large"
    );
}

#[test]
fn frame_over_max_msg_len_returns_msg_too_large() {
    let buf = [b'x'; MAX_MSG_LEN + 1];
    let err = parse_command(&buf).unwrap_err();
    assert_eq!(err, ERROR_MSG_TOO_LARGE);
}

// ----- Malformed JSON -----

#[test]
fn malformed_json_returns_error() {
    let json = b"not json at all";
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_MALFORMED_JSON);
}

#[test]
fn truncated_json_returns_error() {
    let json = br#"{"version":1,"id":"1""#; // missing closing brace
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_MALFORMED_JSON);
}

// ----- Error code catalogue — one test per error code -----

#[test]
fn error_code_missing_version() {
    // Already covered in missing_version_returns_error
    let json = br#"{"id":"1","interface":"gpio","action":"read","pin":0}"#;
    assert_eq!(parse_command(json).unwrap_err(), ERROR_MISSING_VERSION);
}

#[test]
fn error_code_unsupported_version() {
    let json = br#"{"version":99,"id":"1","interface":"gpio","action":"read"}"#;
    assert_eq!(parse_command(json).unwrap_err(), ERROR_UNSUPPORTED_VERSION);
}

#[test]
fn error_code_msg_too_large() {
    let buf = [b'x'; MAX_MSG_LEN + 1];
    assert_eq!(parse_command(&buf).unwrap_err(), ERROR_MSG_TOO_LARGE);
}

#[test]
fn error_code_malformed_json() {
    assert_eq!(parse_command(b"{bad}").unwrap_err(), ERROR_MALFORMED_JSON);
}

/// missing_field, unknown_interface, unknown_action, invalid_pin, value_out_of_range,
/// pin_in_use, not_configured, peripheral_busy, peripheral_error are returned by the router
/// and interface handlers, not by parse_command. We verify the constants match their string values.
#[test]
fn error_code_constants_match_strings() {
    assert_eq!(ERROR_MISSING_FIELD, "missing_field");
    assert_eq!(ERROR_UNKNOWN_INTERFACE, "unknown_interface");
    assert_eq!(ERROR_UNKNOWN_ACTION, "unknown_action");
    assert_eq!(ERROR_INVALID_PIN, "invalid_pin");
    assert_eq!(ERROR_VALUE_OUT_OF_RANGE, "value_out_of_range");
    assert_eq!(ERROR_PIN_IN_USE, "pin_in_use");
    assert_eq!(ERROR_NOT_CONFIGURED, "not_configured");
    assert_eq!(ERROR_PERIPHERAL_BUSY, "peripheral_busy");
    assert_eq!(ERROR_PERIPHERAL_ERROR, "peripheral_error");
    assert_eq!(ERROR_INVALID_ENCODING, "invalid_encoding");
    assert_eq!(ERROR_WEBSOCKET_HANDSHAKE, "ws_handshake_failed");
}

// ----- data field shapes for read operations -----

#[test]
fn response_data_gpio_read_shape() {
    let resp = Response::ok("g1", Some(ResponseData::GpioRead { value: 1 }));
    let mut buf = [0u8; 128];
    let n = serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"value\":1"), "gpio read data shape: {s}");
}

#[test]
fn response_data_adc_read_shape() {
    let resp = Response::ok(
        "a1",
        Some(ResponseData::AdcRead {
            raw: 2048,
            voltage: 1.650,
        }),
    );
    let mut buf = [0u8; 128];
    let n = serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"raw\":2048"), "adc read data shape: {s}");
    assert!(s.contains("\"voltage\""), "adc read voltage field: {s}");
}

#[test]
fn response_data_adc_temp_shape() {
    let resp = Response::ok("t1", Some(ResponseData::AdcTemp { celsius: 27.3 }));
    let mut buf = [0u8; 128];
    let n = serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"celsius\""), "adc temp data shape: {s}");
}

#[test]
fn response_data_bytes_shape() {
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(&[0x48, 0x65, 0x6C]).ok();
    let resp = Response::ok(
        "b1",
        Some(ResponseData::Bytes {
            bytes: Base64Bytes(bytes),
        }),
    );
    let mut buf = [0u8; 128];
    let n = serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    // 0x48, 0x65, 0x6C = "Hel" → base64 "SGVs"
    assert!(s.contains("\"bytes\":\"SGVs\""), "bytes data shape: {s}");
}

// ----- FrameReader -----

#[test]
fn frame_reader_accumulates_until_newline() {
    let mut fr = FrameReader::new();
    let input = b"hello\n";
    let mut result = None;
    for &byte in input {
        result = fr.push(byte).unwrap();
    }
    assert_eq!(result, Some(b"hello" as &[u8]));
}

#[test]
fn frame_reader_detects_oversized_frame() {
    let mut fr = FrameReader::new();
    // Push MAX_MSG_LEN bytes (no newline) then a newline
    for _ in 0..MAX_MSG_LEN {
        fr.push(b'x').unwrap();
    }
    // Now push newline — should return Err(msg_too_large)
    let result = fr.push(b'\n');
    assert_eq!(result, Err(ERROR_MSG_TOO_LARGE));
}

// ----- ADC channel deserialization -----

#[test]
fn adc_channel_numeric_deserialization() {
    let json = br#"{"version":1,"id":"a","interface":"adc","action":"read","adc_channel":2}"#;
    let cmd = parse_command(json).unwrap();
    assert_eq!(cmd.adc_channel, Some(AdcChannel::Ch2));
}

#[test]
fn adc_channel_temp_deserialization() {
    // Temperature sensor is encoded as channel 3 (numeric) since serde-json-core
    // does not support deserialize_any for mixed number/string field types.
    let json = br#"{"version":1,"id":"a","interface":"adc","action":"read","adc_channel":3}"#;
    let cmd = parse_command(json).unwrap();
    assert_eq!(cmd.adc_channel, Some(AdcChannel::Temp));
}

// ----- FrameReader edge cases -----

#[test]
fn frame_reader_exactly_max_data_bytes_succeeds() {
    // MAX_MSG_LEN includes the newline, so (MAX_MSG_LEN-1) data bytes + \n is the max valid frame.
    let mut fr = FrameReader::new();
    for _ in 0..MAX_MSG_LEN - 1 {
        assert_eq!(fr.push(b'A').unwrap(), None);
    }
    let result = fr.push(b'\n').unwrap();
    assert!(
        result.is_some(),
        "{} data bytes + newline should produce a valid frame",
        MAX_MSG_LEN - 1
    );
    assert_eq!(result.unwrap().len(), MAX_MSG_LEN - 1);
}

#[test]
fn frame_reader_consecutive_frames() {
    let mut fr = FrameReader::new();
    let input = b"hello\nworld\n";
    let mut frames: Vec<Vec<u8>> = Vec::new();
    for &byte in input {
        match fr.push(byte) {
            Ok(Some(slice)) => frames.push(slice.to_vec()),
            Ok(None) => {}
            Err(_) => panic!("unexpected error"),
        }
    }
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0], b"hello");
    assert_eq!(frames[1], b"world");
}

#[test]
fn frame_reader_overflow_recovery() {
    let mut fr = FrameReader::new();
    // Push well past MAX_MSG_LEN bytes (triggers overflow after MAX_MSG_LEN-1)
    for _ in 0..MAX_MSG_LEN + 100 {
        let _ = fr.push(b'x');
    }
    // Newline should return error (overflow)
    let result = fr.push(b'\n');
    assert_eq!(result, Err(ERROR_MSG_TOO_LARGE));
    // Now push a valid short frame — should recover
    for &byte in b"ok" {
        assert_eq!(fr.push(byte).unwrap(), None);
    }
    let result = fr.push(b'\n').unwrap();
    assert_eq!(result, Some(b"ok" as &[u8]));
}

#[test]
fn frame_reader_binary_data_with_null_bytes() {
    let mut fr = FrameReader::new();
    let input: [u8; 4] = [0x00, 0xFF, 0x80, b'\n'];
    let mut result = None;
    for &byte in &input {
        if let Ok(Some(slice)) = fr.push(byte) {
            result = Some(slice.to_vec());
        }
    }
    assert_eq!(result.unwrap(), &[0x00u8, 0xFF, 0x80]);
}

#[test]
fn frame_reader_empty_frame() {
    let mut fr = FrameReader::new();
    let result = fr.push(b'\n').unwrap();
    assert_eq!(result, Some(b"" as &[u8]));
}

// ----- ADC channel deserialization edge cases -----

#[test]
fn adc_channel_out_of_range_returns_malformed_json() {
    let json = br#"{"version":1,"id":"1","interface":"adc","action":"read","adc_channel":4}"#;
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_MALFORMED_JSON);
}

#[test]
fn adc_channel_negative_returns_malformed_json() {
    let json = br#"{"version":1,"id":"1","interface":"adc","action":"read","adc_channel":-1}"#;
    let err = parse_command(json).unwrap_err();
    assert_eq!(err, ERROR_MALFORMED_JSON);
}

// ----- Base64 bytes field parsing -----

#[test]
fn parse_command_with_base64_bytes() {
    // "AQI=" is base64 for [1, 2]
    let json =
        br#"{"version":1,"id":"b1","interface":"spi","action":"write","spi":0,"bytes":"AQI="}"#;
    let cmd = parse_command(json).unwrap();
    assert_eq!(cmd.bytes, Some("AQI="));
}

// ----- Response serialization edge cases -----

#[test]
fn serialize_response_buffer_too_small() {
    let resp = Response::ok("abc", None);
    let mut buf = [0u8; 10]; // way too small
    let result = serialize_response(&resp, &mut buf);
    assert_eq!(result, Err(ERROR_MSG_TOO_LARGE));
}
