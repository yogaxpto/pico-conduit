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
//! All tests are `#[ignore]` and are skipped in CI. Run manually before tagging a release.

/// GPIO write → read round-trip via TCP.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn gpio_write_read_roundtrip() {
    todo!("connect to PICO_IP:4242, write GPIO15 high, read it back")
}

/// ADC read returns a valid (non-error) response.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn adc_read_returns_valid_response() {
    todo!("connect to PICO_IP:4242, send adc read channel 0, verify ok:true")
}

/// Malformed JSON returns a structured error response.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn malformed_json_returns_error_response() {
    todo!("send 'not json\\n', verify response contains ok:false error:malformed_json")
}

/// Drop and re-open TCP connection; device recovers.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn reconnect_after_disconnect() {
    todo!("connect, send command, disconnect, reconnect, send another command")
}

/// Protocol version mismatch returns unsupported_version.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn protocol_version_mismatch_returns_error() {
    todo!("send version:2 command, verify response error is unsupported_version")
}

/// Oversized message (513 bytes) returns msg_too_large; connection stays open.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn oversized_message_returns_error_connection_stays_open() {
    todo!("send 513-byte frame, verify msg_too_large, send valid command after, verify it succeeds")
}

/// Error code stability: each error code triggers the exact documented string.
#[test]
#[ignore = "requires hardware (set PICO_IP env var)"]
fn error_code_stability() {
    todo!("for each error code, send a triggering command and assert exact error string")
}
