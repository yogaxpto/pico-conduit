//! MQTT helpers — topic construction, client ID, and backoff logic.
//!
//! This module is `no_std`-compatible and compiles on both embedded and host targets.
//! The embedded-only MQTT client task lives in `src/net.rs`.

use core::fmt::Write as _;

/// Build the MQTT command topic for a device: `pico/<last4hex>/cmd`.
///
/// `mac` is the 6-byte CYW43 MAC address. Only the last 2 bytes are used.
pub fn cmd_topic(mac: [u8; 6]) -> heapless::String<32> {
    let mut topic: heapless::String<32> = heapless::String::new();
    let _ = write!(topic, "pico/{:02x}{:02x}/cmd", mac[4], mac[5]);
    topic
}

/// Build the MQTT response topic for a device: `pico/<last4hex>/resp`.
///
/// `mac` is the 6-byte CYW43 MAC address. Only the last 2 bytes are used.
pub fn resp_topic(mac: [u8; 6]) -> heapless::String<32> {
    let mut topic: heapless::String<32> = heapless::String::new();
    let _ = write!(topic, "pico/{:02x}{:02x}/resp", mac[4], mac[5]);
    topic
}

/// Build the MQTT client ID: `pico-<last4hex>`.
///
/// `mac` is the 6-byte CYW43 MAC address. Only the last 2 bytes are used.
pub fn client_id(mac: [u8; 6]) -> heapless::String<16> {
    let mut id: heapless::String<16> = heapless::String::new();
    let _ = write!(id, "pico-{:02x}{:02x}", mac[4], mac[5]);
    id
}

/// Compute the reconnect backoff delay in seconds for a given attempt number.
///
/// Sequence: 5 → 10 → 20 → 40 → 60 (capped at 60s).
pub fn backoff_secs(attempt: u8) -> u16 {
    match attempt {
        0 => 5,
        1 => 10,
        2 => 20,
        3 => 40,
        _ => 60,
    }
}
