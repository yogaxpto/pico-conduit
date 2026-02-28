//! Tier 4 TCP integration tests.
//!
//! These tests require physical hardware with firmware flashed and reachable over the network.
//!
//! # Setup
//!
//! 1. Flash the firmware: `cargo build --release && probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/pico-socketeer`
//! 2. Wait for the LED to show solid ON (connected)
//! 3. Run: `PICO_IP=192.168.1.x cargo test --test integration`
//!
//! Tests auto-skip when `PICO_IP` is not set. No `--ignored` flag needed.

use pico_socketeer_macros::require_env;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

// ---- Helpers ----

/// Read the `PICO_IP` env var and return `"{ip}:4242"`.
/// Panics if `PICO_IP` is not set — callers are guarded by `#[require_env("PICO_IP")]`.
fn pico_addr() -> String {
    let ip = std::env::var("PICO_IP").expect("PICO_IP must be set");
    format!("{ip}:4242")
}

/// Open a TCP connection to the Pico with a 5-second connect timeout.
fn connect(addr: &str) -> TcpStream {
    let stream = TcpStream::connect_timeout(
        &addr.parse().expect("invalid PICO_IP address"),
        Duration::from_secs(5),
    )
    .expect("failed to connect to Pico");
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
    stream
}

/// Send a JSON line (appends `\n`) and flush.
fn send_line(stream: &mut TcpStream, json: &str) {
    write!(stream, "{json}\n").expect("TCP write failed");
    stream.flush().expect("TCP flush failed");
}

/// Send raw bytes (no newline appended).
fn send_raw(stream: &mut TcpStream, data: &[u8]) {
    stream.write_all(data).expect("TCP write failed");
    stream.flush().expect("TCP flush failed");
}

/// Read one newline-terminated response and parse it as JSON.
fn read_response(stream: &TcpStream) -> Value {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("TCP read failed or timed out");
    serde_json::from_str(line.trim()).expect("response is not valid JSON")
}

/// Build a JSON command string with version=1.
fn cmd(id: &str, interface: &str, action: &str, extra: Value) -> String {
    let mut obj = json!({
        "version": 1,
        "id": id,
        "interface": interface,
        "action": action,
    });
    if let Value::Object(map) = extra {
        for (k, v) in map {
            obj[k] = v;
        }
    }
    serde_json::to_string(&obj).unwrap()
}

// ---- Tests ----

/// GPIO write -> read round-trip via TCP.
#[test]
#[require_env("PICO_IP")]
fn gpio_write_read_roundtrip() {
    let addr = pico_addr();
    let mut stream = connect(&addr);

    // Write GPIO15 high
    send_line(
        &mut stream,
        &cmd("w1", "gpio", "write", json!({"pin": 15, "value": 1})),
    );
    let resp = read_response(&stream);
    assert_eq!(resp["ok"], true, "gpio write failed: {resp}");

    // Read GPIO15 back
    send_line(&mut stream, &cmd("r1", "gpio", "read", json!({"pin": 15})));
    let resp = read_response(&stream);
    assert_eq!(resp["ok"], true, "gpio read failed: {resp}");
    assert_eq!(resp["data"]["value"], 1, "expected pin 15 high: {resp}");
}

/// ADC read returns a valid (non-error) response.
#[test]
#[require_env("PICO_IP")]
fn adc_read_returns_valid_response() {
    let addr = pico_addr();
    let mut stream = connect(&addr);

    send_line(
        &mut stream,
        &cmd("a1", "adc", "read", json!({"adc_channel": 0})),
    );
    let resp = read_response(&stream);
    assert_eq!(resp["ok"], true, "adc read failed: {resp}");
    assert!(resp["data"]["raw"].is_number(), "missing raw: {resp}");
    assert!(
        resp["data"]["voltage"].is_number(),
        "missing voltage: {resp}"
    );
}

/// Malformed JSON returns a structured error response.
#[test]
#[require_env("PICO_IP")]
fn malformed_json_returns_error_response() {
    let addr = pico_addr();
    let mut stream = connect(&addr);

    send_line(&mut stream, "not json");
    let resp = read_response(&stream);
    assert_eq!(resp["ok"], false, "expected error: {resp}");
    assert_eq!(resp["error"], "malformed_json", "wrong error code: {resp}");
}

/// Drop and re-open TCP connection; device recovers.
#[test]
#[require_env("PICO_IP")]
fn reconnect_after_disconnect() {
    let addr = pico_addr();

    // First connection
    {
        let mut stream = connect(&addr);
        send_line(
            &mut stream,
            &cmd("c1", "gpio", "set_mode", json!({"pin": 0, "mode": "input"})),
        );
        let resp = read_response(&stream);
        assert_eq!(resp["ok"], true, "first command failed: {resp}");
    }
    // stream dropped — TCP RST / FIN

    // Brief pause for device to re-enter accept()
    std::thread::sleep(Duration::from_millis(500));

    // Second connection
    {
        let mut stream = connect(&addr);
        send_line(
            &mut stream,
            &cmd("c2", "gpio", "set_mode", json!({"pin": 1, "mode": "input"})),
        );
        let resp = read_response(&stream);
        assert_eq!(
            resp["ok"], true,
            "second command after reconnect failed: {resp}"
        );
    }
}

/// Protocol version mismatch returns unsupported_version.
#[test]
#[require_env("PICO_IP")]
fn protocol_version_mismatch_returns_error() {
    let addr = pico_addr();
    let mut stream = connect(&addr);

    let bad_cmd = json!({
        "version": 2,
        "id": "v1",
        "interface": "gpio",
        "action": "read",
        "pin": 0
    });
    send_line(&mut stream, &serde_json::to_string(&bad_cmd).unwrap());
    let resp = read_response(&stream);
    assert_eq!(resp["ok"], false, "expected error: {resp}");
    assert_eq!(
        resp["error"], "unsupported_version",
        "wrong error code: {resp}"
    );
}

/// Oversized message (513 bytes) returns msg_too_large; connection stays open.
#[test]
#[require_env("PICO_IP")]
fn oversized_message_returns_error_connection_stays_open() {
    let addr = pico_addr();
    let mut stream = connect(&addr);

    // Send 513 data bytes (exceeding MAX_MSG_LEN=512 which includes the newline)
    // then a newline to trigger the frame.
    let oversized = vec![b'A'; 513];
    send_raw(&mut stream, &oversized);
    send_raw(&mut stream, b"\n");

    let resp = read_response(&stream);
    assert_eq!(resp["ok"], false, "expected error: {resp}");
    assert_eq!(resp["error"], "msg_too_large", "wrong error code: {resp}");

    // Connection should still be alive — send a valid command
    send_line(
        &mut stream,
        &cmd(
            "ok1",
            "gpio",
            "set_mode",
            json!({"pin": 0, "mode": "input"}),
        ),
    );
    let resp = read_response(&stream);
    assert_eq!(
        resp["ok"], true,
        "connection should survive after msg_too_large: {resp}"
    );
}

/// Error code stability: each error code triggers the exact documented string.
#[test]
#[require_env("PICO_IP")]
fn error_code_stability() {
    let addr = pico_addr();
    let mut stream = connect(&addr);

    let cases: &[(&str, &str)] = &[
        // (json to send, expected error string)
        // malformed_json
        ("not json", "malformed_json"),
        // missing_version
        (
            r#"{"id":"e1","interface":"gpio","action":"read","pin":0}"#,
            "missing_version",
        ),
        // unsupported_version
        (
            r#"{"version":99,"id":"e2","interface":"gpio","action":"read","pin":0}"#,
            "unsupported_version",
        ),
        // unknown_interface
        (
            r#"{"version":1,"id":"e3","interface":"quantum","action":"read"}"#,
            "unknown_interface",
        ),
        // unknown_action
        (
            r#"{"version":1,"id":"e4","interface":"gpio","action":"fly"}"#,
            "unknown_action",
        ),
        // missing_field (gpio set_mode without pin)
        (
            r#"{"version":1,"id":"e5","interface":"gpio","action":"set_mode","mode":"input"}"#,
            "missing_field",
        ),
        // invalid_pin (pin 29 is reserved)
        (
            r#"{"version":1,"id":"e6","interface":"gpio","action":"set_mode","pin":29,"mode":"input"}"#,
            "invalid_pin",
        ),
    ];

    for (json_str, expected_error) in cases {
        send_line(&mut stream, json_str);
        let resp = read_response(&stream);
        assert_eq!(
            resp["ok"], false,
            "expected error for {expected_error}: {resp}"
        );
        assert_eq!(
            resp["error"].as_str().unwrap(),
            *expected_error,
            "error mismatch for input: {json_str}"
        );
    }
}
