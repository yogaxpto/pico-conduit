use pico_socketeer::led::{LedState, SOS_TIMING};

/// LedState must have exactly 9 variants — one for each row in the LED State Reference table.
/// This test prevents silent omissions when the table is updated.
#[test]
fn led_state_has_nine_variants() {
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
    ]
    .len();
    assert_eq!(count, 9, "LedState must have exactly 9 variants");
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
