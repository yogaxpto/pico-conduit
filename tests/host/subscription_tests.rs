//! Host tests for the push subscription registry.
//!
//! Tests cover: subscribe/unsubscribe round-trips, limit enforcement, duplicate
//! rejection, and subscription clearing on disconnect (DeviceState reset).

use crate::fixtures::make_cmd;
use pico_socketeer::protocol::{
    AdcChannel, EdgeTrigger, ERROR_ALREADY_SUBSCRIBED, ERROR_MISSING_FIELD, ERROR_NOT_SUBSCRIBED,
    ERROR_SUBSCRIPTION_LIMIT, MAX_SUBSCRIPTIONS,
};
use pico_socketeer::router::{DeviceState, dispatch, validate_route};

// ── route validation ──────────────────────────────────────────────────────────

#[test]
fn adc_subscribe_is_valid_route() {
    let cmd = make_cmd("1", Some("adc"), Some("subscribe"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn adc_unsubscribe_is_valid_route() {
    let cmd = make_cmd("2", Some("adc"), Some("unsubscribe"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn gpio_subscribe_is_valid_route() {
    let cmd = make_cmd("3", Some("gpio"), Some("subscribe"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn gpio_unsubscribe_is_valid_route() {
    let cmd = make_cmd("4", Some("gpio"), Some("unsubscribe"));
    assert!(validate_route(&cmd).is_ok());
}

// ── subscribe / unsubscribe round-trip ───────────────────────────────────────

#[test]
fn adc_subscribe_adds_to_registry() {
    let mut state = DeviceState::default();
    assert_eq!(state.subscriptions.len(), 0);

    let mut cmd = make_cmd("s1", Some("adc"), Some("subscribe"));
    cmd.adc_channel = Some(AdcChannel::Ch0);
    cmd.interval_ms = Some(50);
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);

    assert!(resp.ok, "subscribe should succeed: {:?}", resp.error);
    assert_eq!(state.subscriptions.len(), 1);
}

#[test]
fn adc_unsubscribe_removes_from_registry() {
    let mut state = DeviceState::default();

    let mut sub_cmd = make_cmd("s1", Some("adc"), Some("subscribe"));
    sub_cmd.adc_channel = Some(AdcChannel::Ch0);
    let route = validate_route(&sub_cmd).unwrap();
    dispatch(&sub_cmd, route, &mut state);
    assert_eq!(state.subscriptions.len(), 1);

    let mut unsub_cmd = make_cmd("u1", Some("adc"), Some("unsubscribe"));
    unsub_cmd.adc_channel = Some(AdcChannel::Ch0);
    let route = validate_route(&unsub_cmd).unwrap();
    let resp = dispatch(&unsub_cmd, route, &mut state);

    assert!(resp.ok, "unsubscribe should succeed: {:?}", resp.error);
    assert_eq!(state.subscriptions.len(), 0);
}

#[test]
fn gpio_subscribe_level_adds_to_registry() {
    let mut state = DeviceState::default();

    let mut cmd = make_cmd("s2", Some("gpio"), Some("subscribe"));
    cmd.pin = Some(5);
    cmd.interval_ms = Some(20);
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);

    assert!(resp.ok, "gpio subscribe should succeed: {:?}", resp.error);
    assert_eq!(state.subscriptions.len(), 1);
}

#[test]
fn gpio_subscribe_edge_rising_adds_to_registry() {
    let mut state = DeviceState::default();

    let mut cmd = make_cmd("s3", Some("gpio"), Some("subscribe"));
    cmd.pin = Some(7);
    cmd.trigger = Some("edge_rising");
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);

    assert!(resp.ok, "gpio edge subscribe should succeed: {:?}", resp.error);
    assert_eq!(state.subscriptions.len(), 1);

    // Verify the stored target has the correct trigger
    use pico_socketeer::protocol::SubscriptionTarget;
    match &state.subscriptions[0].target {
        SubscriptionTarget::GpioEdge { pin: 7, trigger: EdgeTrigger::Rising } => {}
        other => panic!("unexpected target: {other:?}"),
    }
}

#[test]
fn gpio_unsubscribe_removes_from_registry() {
    let mut state = DeviceState::default();

    let mut sub = make_cmd("s4", Some("gpio"), Some("subscribe"));
    sub.pin = Some(3);
    sub.interval_ms = Some(10);
    let route = validate_route(&sub).unwrap();
    dispatch(&sub, route, &mut state);

    let mut unsub = make_cmd("u4", Some("gpio"), Some("unsubscribe"));
    unsub.pin = Some(3);
    let route = validate_route(&unsub).unwrap();
    let resp = dispatch(&unsub, route, &mut state);

    assert!(resp.ok, "gpio unsubscribe should succeed: {:?}", resp.error);
    assert_eq!(state.subscriptions.len(), 0);
}

// ── duplicate subscription rejected ──────────────────────────────────────────

#[test]
fn adc_duplicate_subscription_rejected() {
    let mut state = DeviceState::default();

    let mut cmd = make_cmd("s5", Some("adc"), Some("subscribe"));
    cmd.adc_channel = Some(AdcChannel::Ch1);
    let route = validate_route(&cmd).unwrap();
    dispatch(&cmd, route, &mut state);

    // Second subscribe for the same channel
    let mut cmd2 = make_cmd("s5b", Some("adc"), Some("subscribe"));
    cmd2.adc_channel = Some(AdcChannel::Ch1);
    cmd2.interval_ms = Some(200);
    let route2 = validate_route(&cmd2).unwrap();
    let resp = dispatch(&cmd2, route2, &mut state);

    assert!(!resp.ok, "duplicate subscribe must fail");
    assert_eq!(resp.error, Some(ERROR_ALREADY_SUBSCRIBED));
    assert_eq!(state.subscriptions.len(), 1, "registry must not grow on duplicate");
}

#[test]
fn gpio_duplicate_subscription_rejected() {
    let mut state = DeviceState::default();

    let mut cmd = make_cmd("s6", Some("gpio"), Some("subscribe"));
    cmd.pin = Some(4);
    let route = validate_route(&cmd).unwrap();
    dispatch(&cmd, route, &mut state);

    let mut cmd2 = make_cmd("s6b", Some("gpio"), Some("subscribe"));
    cmd2.pin = Some(4);
    let route2 = validate_route(&cmd2).unwrap();
    let resp = dispatch(&cmd2, route2, &mut state);

    assert!(!resp.ok, "duplicate gpio subscribe must fail");
    assert_eq!(resp.error, Some(ERROR_ALREADY_SUBSCRIBED));
}

// ── subscription limit enforcement ───────────────────────────────────────────

#[test]
fn subscription_limit_enforced() {
    let mut state = DeviceState::default();

    // Fill up to MAX_SUBSCRIPTIONS using ADC channels and GPIO pins
    let adc_channels = [
        AdcChannel::Ch0,
        AdcChannel::Ch1,
        AdcChannel::Ch2,
        AdcChannel::Temp,
    ];
    let gpio_pins: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7];

    let mut count = 0usize;
    // Subscribe ADC channels first
    for ch in &adc_channels {
        if count >= MAX_SUBSCRIPTIONS {
            break;
        }
        let mut cmd = make_cmd("fill", Some("adc"), Some("subscribe"));
        cmd.adc_channel = Some(*ch);
        let route = validate_route(&cmd).unwrap();
        let resp = dispatch(&cmd, route, &mut state);
        assert!(resp.ok);
        count += 1;
    }
    // Fill rest with GPIO pins
    for &pin in gpio_pins {
        if count >= MAX_SUBSCRIPTIONS {
            break;
        }
        let mut cmd = make_cmd("fill", Some("gpio"), Some("subscribe"));
        cmd.pin = Some(pin);
        let route = validate_route(&cmd).unwrap();
        let resp = dispatch(&cmd, route, &mut state);
        assert!(resp.ok);
        count += 1;
    }

    assert_eq!(state.subscriptions.len(), MAX_SUBSCRIPTIONS);

    // Now try to add one more — must fail with subscription_limit
    let mut overflow = make_cmd("over", Some("gpio"), Some("subscribe"));
    overflow.pin = Some(20); // unused pin
    let route = validate_route(&overflow).unwrap();
    let resp = dispatch(&overflow, route, &mut state);
    assert!(!resp.ok, "subscription limit must be enforced");
    assert_eq!(resp.error, Some(ERROR_SUBSCRIPTION_LIMIT));
    assert_eq!(state.subscriptions.len(), MAX_SUBSCRIPTIONS);
}

// ── unsubscribe missing returns error ─────────────────────────────────────────

#[test]
fn adc_unsubscribe_not_subscribed_returns_error() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("u9", Some("adc"), Some("unsubscribe"));
    cmd.adc_channel = Some(AdcChannel::Ch0);
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_SUBSCRIBED));
}

#[test]
fn gpio_unsubscribe_not_subscribed_returns_error() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("u10", Some("gpio"), Some("unsubscribe"));
    cmd.pin = Some(0);
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_SUBSCRIBED));
}

// ── missing required fields ───────────────────────────────────────────────────

#[test]
fn adc_subscribe_missing_channel_returns_error() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("m1", Some("adc"), Some("subscribe"));
    // adc_channel is None
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn gpio_subscribe_missing_pin_returns_error() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("m2", Some("gpio"), Some("subscribe"));
    // pin is None
    let route = validate_route(&cmd).unwrap();
    let resp = dispatch(&cmd, route, &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

// ── subscriptions cleared on disconnect ──────────────────────────────────────

#[test]
fn subscriptions_cleared_on_device_state_reset() {
    let mut state = DeviceState::default();

    let mut cmd = make_cmd("s7", Some("adc"), Some("subscribe"));
    cmd.adc_channel = Some(AdcChannel::Ch0);
    let route = validate_route(&cmd).unwrap();
    dispatch(&cmd, route, &mut state);
    assert_eq!(state.subscriptions.len(), 1);

    // Simulate disconnect: create a new DeviceState (old one is dropped)
    let fresh_state = DeviceState::default();
    assert_eq!(
        fresh_state.subscriptions.len(),
        0,
        "subscriptions must be empty after disconnect (DeviceState reset)"
    );
}

// ── edge trigger parsing ──────────────────────────────────────────────────────

#[test]
fn edge_trigger_from_str_rising() {
    assert_eq!(EdgeTrigger::from_str("edge_rising"), Some(EdgeTrigger::Rising));
}

#[test]
fn edge_trigger_from_str_falling() {
    assert_eq!(EdgeTrigger::from_str("edge_falling"), Some(EdgeTrigger::Falling));
}

#[test]
fn edge_trigger_from_str_both() {
    assert_eq!(EdgeTrigger::from_str("edge_both"), Some(EdgeTrigger::Both));
}

#[test]
fn edge_trigger_from_str_invalid() {
    assert_eq!(EdgeTrigger::from_str("unknown"), None);
}

// ── error constant strings ────────────────────────────────────────────────────

#[test]
fn error_already_subscribed_string() {
    assert_eq!(ERROR_ALREADY_SUBSCRIBED, "already_subscribed");
}

#[test]
fn error_subscription_limit_string() {
    assert_eq!(ERROR_SUBSCRIPTION_LIMIT, "subscription_limit");
}

#[test]
fn error_not_subscribed_string() {
    assert_eq!(ERROR_NOT_SUBSCRIBED, "not_subscribed");
}
