use pico_socketeer::provisioning::portal::{
    ConnectForm, Method, ParseError, RequestLine, decode_url_encoded, parse_connect_form,
    parse_request_line,
};

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
    let raw_body = b"ssid=My%20Network&password=hunter2%21";
    let mut ssid_buf = [0u8; 128];
    let decoded = decode_url_encoded(raw_body, &mut ssid_buf).unwrap();
    let form = parse_connect_form(decoded).unwrap();
    assert_eq!(form.ssid, "My Network");
    assert_eq!(form.password, "hunter2!");
}
