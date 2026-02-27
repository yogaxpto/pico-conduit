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

#[cfg(test)]
mod tests {
    use super::*;

    // ----- Request line parsing -----

    #[test]
    fn parse_get_slash() {
        let line = b"GET / HTTP/1.0\r\n";
        let req = parse_request_line(line).unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/");
    }

    #[test]
    fn parse_get_slash_no_crlf() {
        let line = b"GET / HTTP/1.1\n";
        let req = parse_request_line(line).unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/");
    }

    #[test]
    fn parse_post_connect() {
        let line = b"POST /connect HTTP/1.0\r\n";
        let req = parse_request_line(line).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.path, "/connect");
    }

    #[test]
    fn parse_get_status() {
        let line = b"GET /status HTTP/1.1\r\n";
        let req = parse_request_line(line).unwrap();
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/status");
    }

    #[test]
    fn malformed_request_line_missing_path() {
        let line = b"GET\r\n";
        let err = parse_request_line(line).unwrap_err();
        assert_eq!(err, ParseError::MalformedRequestLine);
    }

    #[test]
    fn malformed_request_line_empty() {
        let err = parse_request_line(b"\r\n").unwrap_err();
        assert_eq!(err, ParseError::MalformedRequestLine);
    }

    #[test]
    fn unknown_method_returns_error() {
        let line = b"DELETE /resource HTTP/1.1\r\n";
        let err = parse_request_line(line).unwrap_err();
        assert_eq!(err, ParseError::UnknownMethod);
    }

    // ----- URL-encoded body parsing -----

    #[test]
    fn parse_connect_form_basic() {
        let body = "ssid=MyNet&password=secret";
        let form = parse_connect_form(body).unwrap();
        assert_eq!(form.ssid, "MyNet");
        assert_eq!(form.password, "secret");
    }

    #[test]
    fn parse_connect_form_reversed_order() {
        let body = "password=hunter2&ssid=WiFi";
        let form = parse_connect_form(body).unwrap();
        assert_eq!(form.ssid, "WiFi");
        assert_eq!(form.password, "hunter2");
    }

    #[test]
    fn parse_connect_form_missing_ssid() {
        let body = "password=secret";
        let err = parse_connect_form(body).unwrap_err();
        assert_eq!(err, ParseError::MissingFormField);
    }

    #[test]
    fn parse_connect_form_missing_password() {
        let body = "ssid=MyNet";
        let err = parse_connect_form(body).unwrap_err();
        assert_eq!(err, ParseError::MissingFormField);
    }

    #[test]
    fn parse_connect_form_extra_fields_ignored() {
        let body = "ssid=Net&password=Pass&extra=ignored";
        let form = parse_connect_form(body).unwrap();
        assert_eq!(form.ssid, "Net");
        assert_eq!(form.password, "Pass");
    }

    // ----- Percent-decoding -----

    #[test]
    fn decode_plain_ascii() {
        let mut out = [0u8; 128];
        let s = decode_url_encoded(b"hello", &mut out).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn decode_plus_to_space() {
        let mut out = [0u8; 128];
        let s = decode_url_encoded(b"hello+world", &mut out).unwrap();
        assert_eq!(s, "hello world");
    }

    #[test]
    fn decode_percent_encoded_at_sign() {
        let mut out = [0u8; 128];
        let s = decode_url_encoded(b"user%40example.com", &mut out).unwrap();
        assert_eq!(s, "user@example.com");
    }

    #[test]
    fn decode_percent_encoded_exclamation() {
        let mut out = [0u8; 128];
        let s = decode_url_encoded(b"pass%21word", &mut out).unwrap();
        assert_eq!(s, "pass!word");
    }

    #[test]
    fn decode_space_encoded_as_percent_20() {
        let mut out = [0u8; 128];
        let s = decode_url_encoded(b"My%20Network", &mut out).unwrap();
        assert_eq!(s, "My Network");
    }

    #[test]
    fn decode_invalid_percent_sequence() {
        let mut out = [0u8; 128];
        let err = decode_url_encoded(b"bad%GG", &mut out).unwrap_err();
        assert_eq!(err, ParseError::InvalidPercentEncoding);
    }

    #[test]
    fn decode_truncated_percent_sequence() {
        let mut out = [0u8; 128];
        let err = decode_url_encoded(b"end%4", &mut out).unwrap_err();
        assert_eq!(err, ParseError::InvalidPercentEncoding);
    }

    #[test]
    fn full_form_decode_and_parse() {
        // Simulate a form submission with percent-encoded SSID
        let raw_body = b"ssid=My%20Network&password=hunter2%21";
        let mut ssid_buf = [0u8; 128];
        // We need to decode the whole body then parse
        // For this test, decode the raw body as a whole
        let decoded = decode_url_encoded(raw_body, &mut ssid_buf).unwrap();
        let form = parse_connect_form(decoded).unwrap();
        assert_eq!(form.ssid, "My Network");
        assert_eq!(form.password, "hunter2!");
    }
}
