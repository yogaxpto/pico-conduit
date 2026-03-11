//! Host tests for the Codec trait abstraction (P7 — binary codec support).
//!
//! `JsonCodec` tests run unconditionally.
//! `PostcardCodec` tests are compiled only when `codec-postcard` feature is active.

use pico_socketeer::codec::{Codec, JsonCodec};
use pico_socketeer::protocol::{ERROR_MALFORMED_JSON, ERROR_MISSING_VERSION, Response};

// ── JsonCodec ─────────────────────────────────────────────────────────────────

#[test]
fn json_codec_parses_valid_gpio_command() {
    let json = br#"{"version":1,"id":"c1","interface":"gpio","action":"read","pin":5}"#;
    let cmd = JsonCodec::parse_command(json).unwrap();
    assert_eq!(cmd.id, "c1");
    assert_eq!(cmd.interface, Some("gpio"));
    assert_eq!(cmd.action, Some("read"));
    assert_eq!(cmd.pin, Some(5));
}

#[test]
fn json_codec_rejects_malformed_json() {
    let result = JsonCodec::parse_command(b"not json at all");
    assert_eq!(result.unwrap_err(), ERROR_MALFORMED_JSON);
}

#[test]
fn json_codec_rejects_missing_version() {
    let json = br#"{"id":"c1","interface":"gpio","action":"read","pin":0}"#;
    let result = JsonCodec::parse_command(json);
    assert_eq!(result.unwrap_err(), ERROR_MISSING_VERSION);
}

#[test]
fn json_codec_serializes_ok_response() {
    let resp = Response::ok("r1", None);
    let mut buf = [0u8; 128];
    let n = JsonCodec::serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"ok\":true"), "missing ok:true: {s}");
    assert!(s.contains("\"id\":\"r1\""), "missing id: {s}");
    assert!(s.ends_with('\n'), "missing trailing newline: {s:?}");
}

#[test]
fn json_codec_serializes_error_response() {
    use pico_socketeer::protocol::ERROR_INVALID_PIN;
    let resp = Response::error("e1", ERROR_INVALID_PIN);
    let mut buf = [0u8; 128];
    let n = JsonCodec::serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(s.contains("\"ok\":false"), "missing ok:false: {s}");
    assert!(s.contains("invalid_pin"), "missing error code: {s}");
}

// ── PostcardCodec ─────────────────────────────────────────────────────────────

#[cfg(feature = "codec-postcard")]
mod postcard_tests {
    use pico_socketeer::codec::{Codec, PostcardCodec, encode_command_postcard};
    use pico_socketeer::protocol::{
        AdcChannel, Command, ERROR_MALFORMED_JSON, Response, ResponseData,
    };

    fn gpio_read_cmd() -> Command<'static> {
        Command {
            version: Some(1),
            id: "p1",
            interface: Some("gpio"),
            action: Some("read"),
            pin: Some(5),
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

    fn adc_read_cmd() -> Command<'static> {
        Command {
            version: Some(1),
            id: "a1",
            interface: Some("adc"),
            action: Some("read"),
            adc_channel: Some(AdcChannel::Ch0),
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
            interval_ms: None,
            trigger: None,
            commands: None,
        }
    }

    #[test]
    fn postcard_codec_round_trips_gpio_command() {
        let cmd = gpio_read_cmd();
        let mut bin_buf = [0u8; 128];
        let n = encode_command_postcard(&cmd, &mut bin_buf).unwrap();

        let decoded = PostcardCodec::parse_command(&bin_buf[..n]).unwrap();
        assert_eq!(decoded.id, "p1");
        assert_eq!(decoded.interface, Some("gpio"));
        assert_eq!(decoded.action, Some("read"));
        assert_eq!(decoded.pin, Some(5));
        assert_eq!(decoded.value, None);
        assert_eq!(decoded.version, Some(1));
    }

    #[test]
    fn postcard_codec_round_trips_adc_command() {
        let cmd = adc_read_cmd();
        let mut bin_buf = [0u8; 128];
        let n = encode_command_postcard(&cmd, &mut bin_buf).unwrap();

        let decoded = PostcardCodec::parse_command(&bin_buf[..n]).unwrap();
        assert_eq!(decoded.id, "a1");
        assert_eq!(decoded.interface, Some("adc"));
        assert_eq!(decoded.adc_channel, Some(AdcChannel::Ch0));
    }

    #[test]
    fn postcard_command_smaller_than_json() {
        let json = br#"{"version":1,"id":"p1","interface":"gpio","action":"read","pin":5}"#;
        let json_len = json.len();

        let cmd = gpio_read_cmd();
        let mut bin_buf = [0u8; 128];
        let postcard_len = encode_command_postcard(&cmd, &mut bin_buf).unwrap();

        assert!(
            postcard_len < json_len,
            "postcard ({postcard_len}B) should be smaller than JSON ({json_len}B)"
        );
    }

    #[test]
    fn postcard_adc_command_smaller_than_json() {
        let json = br#"{"version":1,"id":"a1","interface":"adc","action":"read","adc_channel":0}"#;
        let json_len = json.len();

        let cmd = adc_read_cmd();
        let mut bin_buf = [0u8; 128];
        let postcard_len = encode_command_postcard(&cmd, &mut bin_buf).unwrap();

        assert!(
            postcard_len < json_len,
            "postcard ({postcard_len}B) should be smaller than JSON ({json_len}B)"
        );
    }

    #[test]
    fn postcard_codec_rejects_garbage_bytes() {
        let garbage = &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF];
        let result = PostcardCodec::parse_command(garbage);
        assert_eq!(result.unwrap_err(), ERROR_MALFORMED_JSON);
    }

    #[test]
    fn postcard_codec_serializes_ok_response() {
        let resp = Response::ok("r1", None);
        let mut buf = [0u8; 64];
        let n = PostcardCodec::serialize_response(&resp, &mut buf).unwrap();
        assert!(n > 0, "serialized length must be nonzero");
        // Must be more compact than equivalent JSON
        let json = br#"{"id":"r1","ok":true,"data":null,"error":null}"#;
        assert!(
            n < json.len(),
            "postcard ({n}B) should be smaller than JSON ({}B)",
            json.len()
        );
    }

    #[test]
    fn postcard_codec_serializes_gpio_read_response() {
        let resp = Response::ok("g1", Some(ResponseData::GpioRead { value: 1 }));
        let mut buf = [0u8; 64];
        let n = PostcardCodec::serialize_response(&resp, &mut buf).unwrap();
        assert!(n > 0);
        let json = br#"{"id":"g1","ok":true,"data":{"value":1},"error":null}"#;
        assert!(
            n < json.len(),
            "postcard ({n}B) should be smaller than JSON ({}B)",
            json.len()
        );
    }

    #[test]
    fn postcard_codec_serializes_adc_read_response() {
        let resp = Response::ok(
            "adc1",
            Some(ResponseData::AdcRead {
                raw: 2048,
                voltage: 1.65,
            }),
        );
        let mut buf = [0u8; 64];
        let n = PostcardCodec::serialize_response(&resp, &mut buf).unwrap();
        assert!(n > 0);
    }

    #[test]
    fn postcard_codec_serializes_error_response() {
        use pico_socketeer::protocol::ERROR_INVALID_PIN;
        let resp = Response::error("e1", ERROR_INVALID_PIN);
        let mut buf = [0u8; 64];
        let n = PostcardCodec::serialize_response(&resp, &mut buf).unwrap();
        assert!(n > 0);
        // Binary error response should be very compact
        assert!(n < 20, "binary error response should be compact: {n}B");
    }

    #[test]
    fn postcard_codec_serializes_version_response() {
        use heapless::String;
        let mut ver: String<16> = String::new();
        ver.push_str("0.1.0").unwrap();
        let resp = Response::ok("v1", Some(ResponseData::Version { version: ver }));
        let mut buf = [0u8; 64];
        let n = PostcardCodec::serialize_response(&resp, &mut buf).unwrap();
        assert!(n > 0);
    }
}

// ── Size constants ────────────────────────────────────────────────────────────

#[test]
fn codec_module_is_accessible() {
    // Ensures the public codec module compiles and is reachable
    let _ = core::mem::size_of::<JsonCodec>();
}
