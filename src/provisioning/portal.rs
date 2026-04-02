//! HTTP captive portal — request parsing and SSID helpers for the provisioning server.
//!
//! The portal runs on port 80 while the device is in AP mode. It serves:
//! - `GET /`            — SSID scan results + HTML form
//! - `POST /connect`    — parses credentials, tests connection
//! - `GET /status`      — JSON status (AP mode only)
//! - Any other host     — 302 redirect to `http://192.168.4.1/` (captive portal detection)
//!
//! This module handles the pure parsing logic (HTTP request line + URL-encoded body),
//! which is `no_std`-compatible and testable on the host.

/// An HTTP method parsed from a request line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// A parsed HTTP request line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestLine<'a> {
    pub method: Method,
    pub path: &'a str,
}

/// Error from HTTP or URL parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// The request line is malformed (missing method, path, or HTTP version).
    MalformedRequestLine,
    /// The HTTP method is not GET or POST.
    UnknownMethod,
    /// A URL-encoded form field could not be parsed.
    MalformedFormBody,
    /// A required form field is missing.
    MissingFormField,
    /// A percent-encoded sequence is invalid.
    InvalidPercentEncoding,
}

/// Parse the HTTP request line (first line of an HTTP request).
///
/// Expected format: `METHOD /path HTTP/1.x\r\n` or `METHOD /path HTTP/1.x\n`.
///
/// Returns a [`RequestLine`] borrowing from `line` (the first line only, without CRLF/LF).
///
/// # Errors
///
/// Returns `Err` if the request line is malformed or uses an unrecognised HTTP method.
pub fn parse_request_line(line: &[u8]) -> Result<RequestLine<'_>, ParseError> {
    // Strip trailing \r\n or \n
    let line = line
        .strip_suffix(b"\r\n")
        .or_else(|| line.strip_suffix(b"\n"))
        .unwrap_or(line);

    let line_str = core::str::from_utf8(line).map_err(|_| ParseError::MalformedRequestLine)?;

    let mut parts = line_str.splitn(3, ' ');
    let method_str = parts.next().ok_or(ParseError::MalformedRequestLine)?;
    let path = parts.next().ok_or(ParseError::MalformedRequestLine)?;
    let _http_ver = parts.next().ok_or(ParseError::MalformedRequestLine)?;

    let method = match method_str {
        "GET" => Method::Get,
        "POST" => Method::Post,
        _ => return Err(ParseError::UnknownMethod),
    };

    Ok(RequestLine { method, path })
}

/// Parsed form data from a `POST /connect` request body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectForm<'a> {
    pub ssid: &'a str,
    pub password: &'a str,
    /// MQTT broker host. Empty string means MQTT is disabled.
    pub mqtt_host: &'a str,
    /// MQTT broker port. Defaults to 1883 if not provided or invalid.
    pub mqtt_port: u16,
}

/// Parse an `application/x-www-form-urlencoded` body.
///
/// Extracts `ssid` and `password` fields (in any order).
/// Both fields are required; missing either returns [`ParseError::MissingFormField`].
/// Optional `mqtt_host` and `mqtt_port` fields default to `""` and `1883` respectively.
///
/// Percent-decoding is performed in-place using `decode_url_encoded`.
///
/// # Errors
///
/// Returns `Err` if a required form field is missing or the form body is malformed.
///
/// # Note on lifetimes
///
/// The returned [`ConnectForm`] borrows from the decoded buffer, not from `body`.
/// Call [`decode_url_encoded`] first, then parse the result.
pub fn parse_connect_form<'a>(decoded: &'a str) -> Result<ConnectForm<'a>, ParseError> {
    let mut ssid = None;
    let mut password = None;
    let mut mqtt_host = "";
    let mut mqtt_port: u16 = 1883;

    for pair in decoded.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().ok_or(ParseError::MalformedFormBody)?;
        let val = kv.next().unwrap_or("");
        match key {
            "ssid" => ssid = Some(val),
            "password" => password = Some(val),
            "mqtt_host" => mqtt_host = val,
            "mqtt_port" => {
                if let Ok(p) = val.parse::<u16>() {
                    mqtt_port = p;
                }
                // Non-numeric or empty: keep default 1883
            }
            _ => {} // unknown fields are ignored
        }
    }

    Ok(ConnectForm {
        ssid: ssid.ok_or(ParseError::MissingFormField)?,
        password: password.ok_or(ParseError::MissingFormField)?,
        mqtt_host,
        mqtt_port,
    })
}

/// Percent-decode a URL-encoded string into a fixed-size buffer.
///
/// Also replaces `+` with space. Returns the number of decoded bytes written to `out`.
///
/// `out` must be at least as large as `input` (decoded length is never longer than encoded).
///
/// # Errors
///
/// Returns `Err` if a `%xx` escape sequence contains non-hex characters.
pub fn decode_url_encoded<'a>(
    input: &'a [u8],
    out: &'a mut [u8; 128],
) -> Result<&'a str, ParseError> {
    let mut out_pos = 0;
    let mut in_pos = 0;

    while in_pos < input.len() {
        let b = input[in_pos];
        match b {
            b'+' => {
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedFormBody);
                }
                out[out_pos] = b' ';
                out_pos += 1;
                in_pos += 1;
            }
            b'%' => {
                if in_pos + 2 >= input.len() {
                    return Err(ParseError::InvalidPercentEncoding);
                }
                let hi = hex_nibble(input[in_pos + 1])?;
                let lo = hex_nibble(input[in_pos + 2])?;
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedFormBody);
                }
                out[out_pos] = (hi << 4) | lo;
                out_pos += 1;
                in_pos += 3;
            }
            _ => {
                if out_pos >= out.len() {
                    return Err(ParseError::MalformedFormBody);
                }
                out[out_pos] = b;
                out_pos += 1;
                in_pos += 1;
            }
        }
    }

    core::str::from_utf8(&out[..out_pos]).map_err(|_| ParseError::MalformedFormBody)
}

/// Build the Wi-Fi AP SSID from a 6-byte MAC address.
///
/// Result format: `"pico-setup-XXXX"` where `XXXX` are the last 4 hex digits of the MAC.
/// Stored in a `heapless::String<20>`.
///
/// # Example
///
/// ```rust
/// # use pico_conduit::provisioning::portal::make_ap_ssid;
/// let ssid = make_ap_ssid([0xAA, 0xBB, 0xCC, 0xDD, 0xA3, 0xF2]);
/// assert_eq!(ssid.as_str(), "pico-setup-A3F2");
/// ```
#[must_use]
pub fn make_ap_ssid(mac: [u8; 6]) -> heapless::String<20> {
    use core::fmt::Write as _;
    let mut ssid: heapless::String<20> = heapless::String::new();
    let _ = write!(ssid, "pico-setup-{:02X}{:02X}", mac[4], mac[5]);
    ssid
}

const fn hex_nibble(b: u8) -> Result<u8, ParseError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(ParseError::InvalidPercentEncoding),
    }
}
