//! Host tests for the batch command interface.
//!
//! The batch interface dispatches multiple inner commands in a single round-trip and
//! returns a `{"responses":[...]}` array in the response `data` field.

use pico_socketeer::protocol::{
    ERROR_BATCH_EMPTY, ERROR_BATCH_TOO_LARGE, MAX_BATCH_SIZE, parse_command,
    serialize_batch_response,
};
use pico_socketeer::router::{DeviceState, dispatch_batch, validate_route};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Parse a raw JSON string as a batch envelope, route it, and dispatch.
/// Returns the serialized JSON bytes (without newline).
fn run_batch(json: &str) -> heapless::Vec<u8, 4096> {
    let bytes = json.as_bytes();
    let cmd = parse_command(bytes).expect("parse_command failed");
    let _route = validate_route(&cmd).expect("validate_route failed");
    let mut state = DeviceState::default();
    let batch_resp = dispatch_batch(&cmd, &mut state);
    let mut out = [0u8; 4096];
    let n = serialize_batch_response(&batch_resp, &mut out).expect("serialize failed");
    heapless::Vec::from_slice(&out[..n]).unwrap()
}

// ── route validation ──────────────────────────────────────────────────────────

#[test]
fn batch_run_is_valid_route() {
    let json = r#"{"version":1,"id":"b1","interface":"batch","action":"run","commands":[]}"#;
    let bytes = json.as_bytes();
    let cmd = parse_command(bytes).unwrap();
    let result = validate_route(&cmd);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn batch_unknown_action_returns_error() {
    let json = r#"{"version":1,"id":"b1","interface":"batch","action":"execute","commands":[]}"#;
    let bytes = json.as_bytes();
    let cmd = parse_command(bytes).unwrap();
    let result = validate_route(&cmd);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.error,
        Some(pico_socketeer::protocol::ERROR_UNKNOWN_ACTION)
    );
}

// ── dispatch: single command ──────────────────────────────────────────────────

#[test]
fn batch_single_command_returns_one_response() {
    let json = r#"{"version":1,"id":"b1","interface":"batch","action":"run","commands":[
      {"version":1,"id":"c1","interface":"gpio","action":"set_mode","pin":0,"mode":"output"}
    ]}"#;
    let out = run_batch(json);
    let s = core::str::from_utf8(&out).unwrap();
    assert!(s.contains(r#""id":"b1""#), "outer id missing: {s}");
    assert!(s.contains(r#""ok":true"#), "outer ok missing: {s}");
    assert!(s.contains(r#""responses":"#), "responses key missing: {s}");
    assert!(s.contains(r#""id":"c1""#), "inner id missing: {s}");
}

// ── dispatch: multiple commands ───────────────────────────────────────────────

#[test]
fn batch_multiple_commands_returns_ordered_responses() {
    let json = r#"{"version":1,"id":"b2","interface":"batch","action":"run","commands":[
      {"version":1,"id":"r1","interface":"gpio","action":"set_mode","pin":0,"mode":"output"},
      {"version":1,"id":"r2","interface":"gpio","action":"set_mode","pin":1,"mode":"input"},
      {"version":1,"id":"r3","interface":"gpio","action":"set_mode","pin":2,"mode":"output"}
    ]}"#;
    let out = run_batch(json);
    let s = core::str::from_utf8(&out).unwrap();
    // Verify all three inner responses are present
    assert!(s.contains(r#""id":"r1""#), "r1 missing: {s}");
    assert!(s.contains(r#""id":"r2""#), "r2 missing: {s}");
    assert!(s.contains(r#""id":"r3""#), "r3 missing: {s}");
    // Verify ordering (r1 before r2 before r3)
    let pos1 = s.find(r#""id":"r1""#).unwrap();
    let pos2 = s.find(r#""id":"r2""#).unwrap();
    let pos3 = s.find(r#""id":"r3""#).unwrap();
    assert!(
        pos1 < pos2 && pos2 < pos3,
        "responses must be in command order"
    );
}

// ── dispatch: mixed success and failure ───────────────────────────────────────

#[test]
fn batch_mixed_success_failure_each_gets_own_response() {
    let json = r#"{"version":1,"id":"b3","interface":"batch","action":"run","commands":[
      {"version":1,"id":"ok1","interface":"gpio","action":"set_mode","pin":0,"mode":"output"},
      {"version":1,"id":"bad","interface":"gpio","action":"set_mode","pin":255,"mode":"output"},
      {"version":1,"id":"ok2","interface":"gpio","action":"set_mode","pin":1,"mode":"input"}
    ]}"#;
    let out = run_batch(json);
    let s = core::str::from_utf8(&out).unwrap();
    // The outer batch itself is ok
    assert!(s.contains(r#""id":"b3""#));
    // All three inner ids appear
    assert!(s.contains(r#""id":"ok1""#), "ok1 missing: {s}");
    assert!(s.contains(r#""id":"bad""#), "bad missing: {s}");
    assert!(s.contains(r#""id":"ok2""#), "ok2 missing: {s}");
    // The invalid pin command returns an error
    assert!(s.contains("invalid_pin"), "expected invalid_pin error: {s}");
}

// ── dispatch: empty batch rejected ────────────────────────────────────────────

#[test]
fn batch_empty_rejected() {
    let json = r#"{"version":1,"id":"b4","interface":"batch","action":"run","commands":[]}"#;
    let bytes = json.as_bytes();
    let cmd = parse_command(bytes).unwrap();
    let route = validate_route(&cmd).unwrap();
    let mut state = DeviceState::default();
    let resp = dispatch_batch(&cmd, &mut state);
    assert!(!resp.ok, "empty batch must return ok=false");
    assert_eq!(resp.error, Some(ERROR_BATCH_EMPTY));
}

// ── dispatch: too many commands rejected ──────────────────────────────────────

#[test]
fn batch_exceeds_max_rejected() {
    // Build a batch with MAX_BATCH_SIZE + 1 commands using minimal inner command JSON
    // so the total stays within MAX_MSG_LEN.
    // {"id":"0"} is 10 chars; 17 of them + commas + outer envelope ≈ 258 bytes < 1024.
    let mut json =
        String::from(r#"{"version":1,"id":"b","interface":"batch","action":"run","commands":["#);
    for i in 0..=MAX_BATCH_SIZE {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(r#"{{"id":"{i}"}}"#));
    }
    json.push_str("]}");

    let bytes = json.as_bytes();
    let cmd = parse_command(bytes).unwrap();
    let _route = validate_route(&cmd).unwrap();
    let mut state = DeviceState::default();
    let resp = dispatch_batch(&cmd, &mut state);
    assert!(!resp.ok, "oversized batch must return ok=false");
    assert_eq!(resp.error, Some(ERROR_BATCH_TOO_LARGE));
}

// ── error constants are stable strings ───────────────────────────────────────

#[test]
fn error_batch_empty_string() {
    assert_eq!(ERROR_BATCH_EMPTY, "batch_empty");
}

#[test]
fn error_batch_too_large_string() {
    assert_eq!(ERROR_BATCH_TOO_LARGE, "batch_too_large");
}

#[test]
fn max_batch_size_is_at_least_sixteen() {
    assert!(
        MAX_BATCH_SIZE >= 16,
        "MAX_BATCH_SIZE must support ≥16 commands"
    );
}
