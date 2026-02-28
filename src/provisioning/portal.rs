//! HTTP captive portal — request parsing for the provisioning server.
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    Get,
    Post,
}

/// A parsed HTTP request line.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestLine<'a> {
    pub method: Method,
    pub path: &'a str,
}

/// Error from HTTP or URL parsing.
#[derive(Debug, Clone, Copy, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectForm<'a> {
    pub ssid: &'a str,
    pub password: &'a str,
}

/// Parse an `application/x-www-form-urlencoded` body.
///
/// Extracts `ssid` and `password` fields (in any order).
/// Both fields are required; missing either returns [`ParseError::MissingFormField`].
///
/// Percent-decoding is performed in-place using `decode_url_encoded`.
///
/// # Note on lifetimes
///
/// The returned [`ConnectForm`] borrows from the decoded buffer, not from `body`.
/// Call [`decode_url_encoded`] first, then parse the result.
pub fn parse_connect_form<'a>(decoded: &'a str) -> Result<ConnectForm<'a>, ParseError> {
    let mut ssid = None;
    let mut password = None;

    for pair in decoded.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().ok_or(ParseError::MalformedFormBody)?;
        let val = kv.next().unwrap_or("");
        match key {
            "ssid" => ssid = Some(val),
            "password" => password = Some(val),
            _ => {} // unknown fields are ignored
        }
    }

    Ok(ConnectForm {
        ssid: ssid.ok_or(ParseError::MissingFormField)?,
        password: password.ok_or(ParseError::MissingFormField)?,
    })
}

/// Percent-decode a URL-encoded string into a fixed-size buffer.
///
/// Also replaces `+` with space. Returns the number of decoded bytes written to `out`.
///
/// `out` must be at least as large as `input` (decoded length is never longer than encoded).
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

fn hex_nibble(b: u8) -> Result<u8, ParseError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(ParseError::InvalidPercentEncoding),
    }
}

