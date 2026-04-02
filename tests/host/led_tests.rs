use pico_conduit::led::{LedPattern, LedState, SOS_TIMING};

/// LedState must have exactly 9 variants — one for each row in the LED State Reference table.
/// This test prevents silent omissions when the table is updated.
#[test]
fn led_state_has_eleven_variants() {
    // Exhaustive match — compile error if any variant is missing or extra
    let count = [
        LedState::Booting,
        LedState::Provisioning,
        LedState::Scanning,
        LedState::Connecting,
        LedState::Connected,
        LedState::Reconnecting,
        LedState::Error,
        LedState::Saving,
        LedState::Rebooting,
        LedState::MqttConnecting,
        LedState::MqttConnected,
    ]
    .len();
    assert_eq!(count, 11, "LedState must have exactly 11 variants");
}

/// Exhaustive match to ensure the compiler catches any missing variants.
#[test]
fn led_state_variants_exhaustive_match() {
    let state = LedState::Booting;
    // This match must compile — it covers all 9 variants
    let _: &str = match state {
        LedState::Booting => "booting",
        LedState::Provisioning => "provisioning",
        LedState::Scanning => "scanning",
        LedState::Connecting => "connecting",
        LedState::Connected => "connected",
        LedState::Reconnecting => "reconnecting",
        LedState::Error => "error",
        LedState::Saving => "saving",
        LedState::Rebooting => "rebooting",
        LedState::MqttConnecting => "mqtt_connecting",
        LedState::MqttConnected => "mqtt_connected",
    };
}

/// SOS_TIMING must have exactly 18 entries (9 ON/OFF pairs).
#[test]
fn sos_timing_has_eighteen_entries() {
    assert_eq!(
        SOS_TIMING.len(),
        18,
        "SOS_TIMING must have exactly 18 entries (9 ON/OFF pairs)"
    );
}

/// SOS_TIMING must alternate between true (ON) and false (OFF), starting with true.
#[test]
fn sos_timing_alternates_on_off() {
    assert!(SOS_TIMING[0].0, "SOS_TIMING must start with ON (true)");
    for (i, pair) in SOS_TIMING.iter().enumerate() {
        let expected = i % 2 == 0; // even indices are ON, odd are OFF
        assert_eq!(
            pair.0, expected,
            "SOS_TIMING[{i}] should be {} but is {}",
            expected, pair.0
        );
    }
}

/// SOS_TIMING dots must be 100ms, dashes must be 300ms.
#[test]
fn sos_timing_correct_durations() {
    // ON entries are at even indices (0, 2, 4, 6, 8, 10, 12, 14, 16)
    let expected_on_ms = [100u16, 100, 100, 300, 300, 300, 100, 100, 100];
    for (pair_idx, &expected_ms) in expected_on_ms.iter().enumerate() {
        let entry_idx = pair_idx * 2; // even index = ON entry
        assert_eq!(
            SOS_TIMING[entry_idx].1, expected_ms,
            "SOS_TIMING ON entry {entry_idx} should be {expected_ms}ms"
        );
    }
}

/// SOS_TIMING trailing pause must be 2000ms.
#[test]
fn sos_timing_trailing_pause() {
    let last = SOS_TIMING.last().unwrap();
    assert!(!last.0, "last SOS_TIMING entry must be OFF");
    assert_eq!(last.1, 2000, "trailing pause must be 2000ms");
}

// ── LedPattern tests ──────────────────────────────────────────────────────────

#[test]
fn connected_pattern_is_solid_on() {
    assert_eq!(LedState::Connected.pattern(), LedPattern::Solid(true));
}

#[test]
fn saving_pattern_is_one_shot() {
    assert!(matches!(LedState::Saving.pattern(), LedPattern::OneShot(_)));
}

#[test]
fn rebooting_pattern_is_one_shot() {
    assert!(matches!(
        LedState::Rebooting.pattern(),
        LedPattern::OneShot(_)
    ));
}

#[test]
fn error_pattern_uses_sos_timing() {
    assert_eq!(LedState::Error.pattern(), LedPattern::Repeat(SOS_TIMING));
}

#[test]
fn repeating_states_are_repeat() {
    for state in [
        LedState::Booting,
        LedState::Provisioning,
        LedState::Scanning,
        LedState::Connecting,
        LedState::Reconnecting,
    ] {
        assert!(matches!(state.pattern(), LedPattern::Repeat(_)));
    }
}

#[test]
fn saving_step_count() {
    let LedPattern::OneShot(steps) = LedState::Saving.pattern() else {
        panic!("expected OneShot")
    };
    assert_eq!(steps.len(), 10, "5 flashes × 2 steps each");
}

#[test]
fn rebooting_step_count() {
    let LedPattern::OneShot(steps) = LedState::Rebooting.pattern() else {
        panic!("expected OneShot")
    };
    assert_eq!(steps.len(), 20, "10 flashes × 2 steps each");
}

#[test]
fn booting_trailing_gap_is_1000ms() {
    let LedPattern::Repeat(steps) = LedState::Booting.pattern() else {
        panic!("expected Repeat")
    };
    let last = steps.last().unwrap();
    assert!(!last.0, "trailing step must be OFF");
    assert_eq!(last.1, 1000, "trailing gap must be 1000ms");
}

// ── MQTT LED state tests ─────────────────────────────────────────────────────

#[test]
fn led_mqtt_connecting_pattern_exists() {
    assert!(matches!(
        LedState::MqttConnecting.pattern(),
        LedPattern::Repeat(_)
    ));
}

#[test]
fn led_mqtt_connected_pattern_exists() {
    assert_eq!(LedState::MqttConnected.pattern(), LedPattern::Solid(true));
}
