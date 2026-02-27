//! Message protocol — JSON over TCP, newline-delimited.
//!
//! # Wire Format
//!
//! Commands are JSON objects terminated with a `\n` byte. The `version` field is mandatory
//! and must equal `1`. Responses are also JSON objects terminated with `\n`.
//!
//! **Command (client → Pico):**
//! ```json
//! {"version":1,"id":"abc123","interface":"gpio","action":"write","pin":15,"value":1}
//! ```
//!
//! **Response (success):**
//! ```json
//! {"id":"abc123","ok":true,"data":null,"error":null}
//! ```
//!
//! **Response (failure):**
//! ```json
//! {"id":"abc123","ok":false,"data":null,"error":"invalid_pin"}
//! ```
//!
//! # Error Codes
//!
//! All `"error"` values are `&'static str` — no heap allocation.
//! See [`ERROR_*`] constants for the full catalogue.
//!
//! # Max Message Size
//!
//! [`MAX_MSG_LEN`] = 512 bytes including the newline.

use serde::{Deserialize, Serialize};

/// Maximum frame length in bytes (including the newline terminator).
/// Commands exceeding this limit are rejected with `"error": "msg_too_large"`.
pub const MAX_MSG_LEN: usize = 512;

// --- Error code constants ---
// All error strings are &'static str — part of the v1 protocol stability contract.
// New codes may be added; existing codes must not be renamed.

pub const ERROR_MISSING_VERSION: &str = "missing_version";
pub const ERROR_UNSUPPORTED_VERSION: &str = "unsupported_version";
pub const ERROR_MSG_TOO_LARGE: &str = "msg_too_large";
pub const ERROR_MALFORMED_JSON: &str = "malformed_json";
pub const ERROR_MISSING_FIELD: &str = "missing_field";
pub const ERROR_UNKNOWN_INTERFACE: &str = "unknown_interface";
pub const ERROR_UNKNOWN_ACTION: &str = "unknown_action";
pub const ERROR_INVALID_PIN: &str = "invalid_pin";
pub const ERROR_VALUE_OUT_OF_RANGE: &str = "value_out_of_range";
pub const ERROR_PIN_IN_USE: &str = "pin_in_use";
pub const ERROR_NOT_CONFIGURED: &str = "not_configured";
pub const ERROR_PERIPHERAL_BUSY: &str = "peripheral_busy";
pub const ERROR_PERIPHERAL_ERROR: &str = "peripheral_error";

/// ADC channel selector.
///
/// Wire encoding: 0 = Ch0, 1 = Ch1, 2 = Ch2, 3 = Temp (onboard temperature sensor).
/// `serde-json-core` does not support `deserialize_any`, so the channel is always numeric.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AdcChannel {
    Ch0,
    Ch1,
    Ch2,
    Temp,
}

impl<'de> Deserialize<'de> for AdcChannel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct AdcChannelVisitor;
        impl<'de> serde::de::Visitor<'de> for AdcChannelVisitor {
            type Value = AdcChannel;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("0, 1, 2, or 3 (temperature sensor)")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                match v {
                    0 => Ok(AdcChannel::Ch0),
                    1 => Ok(AdcChannel::Ch1),
                    2 => Ok(AdcChannel::Ch2),
                    3 => Ok(AdcChannel::Temp),
                    _ => Err(E::custom("adc channel out of range (valid: 0, 1, 2, 3)")),
                }
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                if v < 0 {
                    Err(E::custom("adc channel cannot be negative"))
                } else {
                    self.visit_u64(v as u64)
                }
            }
        }
        // serde-json-core requires an explicit type hint; use u64 for numeric-only channels.
        d.deserialize_u64(AdcChannelVisitor)
    }
}

/// A parsed command from a client.
///
/// Borrows from the receive buffer — the buffer must be kept alive as long as the Command.
/// Optional fields are present or absent depending on the `interface` and `action`.
#[derive(Deserialize, Debug, PartialEq)]
pub struct Command<'a> {
    /// Protocol version — must be `Some(1)`. Missing → `missing_version`, other → `unsupported_version`.
    pub version: Option<u8>,
    /// Client-assigned request identifier, echoed in the response.
    pub id: &'a str,
    /// Hardware interface to target (e.g. `"gpio"`, `"uart"`, `"spi"`).
    pub interface: Option<&'a str>,
    /// Action to perform on the interface (e.g. `"read"`, `"write"`, `"configure"`).
    pub action: Option<&'a str>,

    // --- GPIO / general ---
    /// GPIO pin number (0–29, excluding reserved CYW43 pins).
    pub pin: Option<u8>,
    /// Digital value for GPIO write (0 = low, 1 = high).
    pub value: Option<u8>,
    /// GPIO pin mode: `"input"` or `"output"`.
    pub mode: Option<&'a str>,
    /// GPIO pull resistor: `"up"`, `"down"`, or `"none"`.
    pub pull: Option<&'a str>,

    // --- UART ---
    /// UART peripheral index: 0 or 1.
    pub uart: Option<u8>,
    /// Bytes to write (UART write, SPI transfer/write, I2C write).
    pub bytes: Option<heapless::Vec<u8, 64>>,
    /// Number of bytes to read.
    pub len: Option<usize>,
    /// UART baud rate.
    pub baud: Option<u32>,
    /// UART data bits: 7 or 8.
    pub data_bits: Option<u8>,
    /// UART parity: `"none"`, `"odd"`, or `"even"`.
    pub parity: Option<&'a str>,
    /// UART stop bits: 1 or 2.
    pub stop_bits: Option<u8>,

    // --- SPI ---
    /// SPI peripheral index: 0 or 1.
    pub spi: Option<u8>,
    /// SPI / I2C clock frequency in Hz.
    pub freq_hz: Option<u32>,
    /// SPI clock polarity: 0 or 1.
    pub cpol: Option<u8>,
    /// SPI clock phase: 0 or 1.
    pub cpha: Option<u8>,

    // --- I2C ---
    /// I2C peripheral index: 0 or 1.
    pub i2c: Option<u8>,
    /// I2C device address.
    pub addr: Option<u8>,
    /// I2C write_read: bytes to write before reading.
    pub write_bytes: Option<heapless::Vec<u8, 64>>,
    /// I2C write_read: number of bytes to read back.
    pub read_len: Option<usize>,

    // --- PWM ---
    /// PWM slice/channel number.
    pub channel: Option<u8>,
    /// PWM duty cycle (raw 16-bit: 0 = always off, 65535 = always on).
    pub duty_u16: Option<u16>,

    // --- ADC (separate field to avoid name collision with PWM channel) ---
    /// ADC channel: 0, 1, 2, or `"temp"` for the onboard temperature sensor.
    pub adc_channel: Option<AdcChannel>,
}

impl<'a> Command<'a> {
    /// Validate the version field and return a protocol-level error if invalid.
    pub fn check_version(&self) -> Result<(), &'static str> {
        match self.version {
            None => Err(ERROR_MISSING_VERSION),
            Some(1) => Ok(()),
            Some(_) => Err(ERROR_UNSUPPORTED_VERSION),
        }
    }
}

/// Response data payload for read operations.
///
/// Write/set actions return `data: null`; read/transfer actions carry result data.
#[derive(Serialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum ResponseData {
    /// GPIO read result: `{"value": 0}` or `{"value": 1}`.
    GpioRead { value: u8 },
    /// ADC channel read: `{"raw": 2048, "voltage": 1.650}` (12-bit, 0–4095).
    AdcRead { raw: u16, voltage: f32 },
    /// ADC temperature sensor: `{"celsius": 27.3}`.
    AdcTemp { celsius: f32 },
    /// Byte array result (UART read, SPI transfer, I2C read): `{"bytes": [0x0F, 0x42]}`.
    Bytes { bytes: heapless::Vec<u8, 64> },
    /// Config report: `{"ssid": "...", "ip": "...", "connected": true}`. Password never included.
    Config {
        ssid: heapless::String<32>,
        ip: heapless::String<16>,
        connected: bool,
    },
}

/// A response sent from the Pico to the client.
#[derive(Serialize, Debug)]
pub struct Response<'a> {
    /// Echoed request identifier from the Command.
    pub id: &'a str,
    /// `true` on success, `false` on any error.
    pub ok: bool,
    /// Result payload for read operations; `null` for write/set/configure actions.
    pub data: Option<ResponseData>,
    /// Error code string on failure; `null` on success.
    pub error: Option<&'static str>,
}

impl<'a> Response<'a> {
    /// Construct a successful response with optional data payload.
    pub fn ok(id: &'a str, data: Option<ResponseData>) -> Self {
        Self { id, ok: true, data, error: None }
    }

    /// Construct an error response.
    pub fn error(id: &'a str, error: &'static str) -> Self {
        Self { id, ok: false, data: None, error: Some(error) }
    }
}

/// Parse a JSON command from a byte slice (without the newline terminator).
///
/// Validates:
/// 1. Length ≤ [`MAX_MSG_LEN`]
/// 2. Valid JSON parse
/// 3. Version field present and equals 1
///
/// Returns the parsed [`Command`] borrowing from `buf`, or an error code string.
pub fn parse_command(buf: &[u8]) -> Result<Command<'_>, &'static str> {
    if buf.len() > MAX_MSG_LEN {
        return Err(ERROR_MSG_TOO_LARGE);
    }
    let (cmd, _) = serde_json_core::from_slice::<Command<'_>>(buf)
        .map_err(|_| ERROR_MALFORMED_JSON)?;
    cmd.check_version()?;
    Ok(cmd)
}

/// Serialize a response to a byte buffer, appending a newline.
///
/// Returns the number of bytes written (including the newline).
/// Returns `Err` if the buffer is too small.
pub fn serialize_response<'a>(resp: &Response<'a>, buf: &mut [u8]) -> Result<usize, &'static str> {
    let n = serde_json_core::to_slice(resp, buf).map_err(|_| ERROR_MSG_TOO_LARGE)?;
    if n >= buf.len() {
        return Err(ERROR_MSG_TOO_LARGE);
    }
    buf[n] = b'\n';
    Ok(n + 1)
}

/// A newline-delimited frame reader over a byte accumulation buffer.
///
/// Call [`FrameReader::push`] with each incoming byte; when a complete frame is available,
/// it returns `Some(slice)` of the frame (without the newline) for parsing.
pub struct FrameReader {
    buf: [u8; MAX_MSG_LEN],
    pos: usize,
    overflowed: bool,
}

impl FrameReader {
    pub const fn new() -> Self {
        Self { buf: [0u8; MAX_MSG_LEN], pos: 0, overflowed: false }
    }

    /// Reset the reader state (call after processing a frame or on error).
    pub fn reset(&mut self) {
        self.pos = 0;
        self.overflowed = false;
    }

    /// Push a single byte. Returns:
    /// - `Ok(Some(slice))` when a complete newline-terminated frame is ready
    /// - `Ok(None)` when more bytes are needed
    /// - `Err(ERROR_MSG_TOO_LARGE)` when the accumulated bytes exceed `MAX_MSG_LEN`
    pub fn push(&mut self, byte: u8) -> Result<Option<&[u8]>, &'static str> {
        if byte == b'\n' {
            if self.overflowed {
                self.reset();
                return Err(ERROR_MSG_TOO_LARGE);
            }
            let end = self.pos;
            self.pos = 0;
            return Ok(Some(&self.buf[..end]));
        }
        // MAX_MSG_LEN counts the newline terminator, so data portion is MAX_MSG_LEN - 1.
        // Trigger overflow when the next byte would be the MAX_MSG_LEN-th data byte.
        if self.pos >= MAX_MSG_LEN - 1 {
            self.overflowed = true;
            return Ok(None); // keep consuming until newline
        }
        self.buf[self.pos] = byte;
        self.pos += 1;
        Ok(None)
    }
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- Serialize / Deserialize round-trips -----

    #[test]
    fn parse_valid_gpio_write_command() {
        let json = br#"{"version":1,"id":"abc","interface":"gpio","action":"write","pin":15,"value":1}"#;
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
        assert!(s.contains("\"error\":\"invalid_pin\""), "missing error: {s}");
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
    fn frame_exactly_512_bytes_is_accepted() {
        // Build a valid JSON command that is exactly 512 bytes
        // Base: {"version":1,"id":"X","interface":"gpio","action":"read","pin":0}
        // len = 65. Pad "id" value to fill up to 512.
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
        assert!(result != Err(ERROR_MSG_TOO_LARGE), "512-byte frame should not return msg_too_large");
    }

    #[test]
    fn frame_513_bytes_returns_msg_too_large() {
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
        let resp = Response::ok("a1", Some(ResponseData::AdcRead { raw: 2048, voltage: 1.650 }));
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
        let resp = Response::ok("b1", Some(ResponseData::Bytes { bytes }));
        let mut buf = [0u8; 128];
        let n = serialize_response(&resp, &mut buf).unwrap();
        let s = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(s.contains("\"bytes\":[72,101,108]"), "bytes data shape: {s}");
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
        let json =
            br#"{"version":1,"id":"a","interface":"adc","action":"read","adc_channel":3}"#;
        let cmd = parse_command(json).unwrap();
        assert_eq!(cmd.adc_channel, Some(AdcChannel::Temp));
    }
}
