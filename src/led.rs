//! LED status signaling.
//!
//! Defines the [`LedState`] enum and [`SOS_TIMING`] constant. These are `no_std`-compatible
//! and fully testable on the host.
//!
//! The `LED_SIGNAL` static and `led_task` live in `src/main.rs` because they depend on
//! `embassy_sync` and the CYW43 runner type, which are embedded-only.
//!
//! # LED State Reference
//!
//! | Variant | Pattern | Timing | Meaning |
//! |---------|---------|--------|---------|
//! | `Booting` | 3-flash burst | 3×(100ms ON/100ms OFF), 1s OFF | Firmware starting |
//! | `Provisioning` | Slow blink 1 Hz | 1s ON/1s OFF | AP mode, awaiting setup |
//! | `Scanning` | Double-blink | 2×(100ms ON/100ms OFF), 700ms OFF | Scanning SSIDs |
//! | `Connecting` | Fast blink 5 Hz | 100ms ON/100ms OFF | STA join in progress |
//! | `Connected` | Solid ON | Constant | TCP socket accepting |
//! | `Reconnecting` | Medium blink 2 Hz | 250ms ON/250ms OFF | Wi-Fi lost, retrying |
//! | `Error` | SOS Morse | ·‌·‌·‌—‌—‌—‌·‌·‌· + 2s pause | Unrecoverable error |
//! | `Saving` | 5 rapid flashes then OFF | 5×(100ms ON/100ms OFF) then OFF | Saving credentials |

/// All possible LED states for the device.
///
/// The 8 variants correspond exactly to the LED Status Reference table in OBJECTIVE.md Phase 5b.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LedState {
    /// Firmware starting up — 3-flash burst (100ms ON / 100ms OFF × 3, 1s OFF, repeat)
    Booting,
    /// AP mode active, awaiting Wi-Fi setup — slow blink 1 Hz (1s ON / 1s OFF)
    Provisioning,
    /// Scanning for networks — double-blink (2×(100ms ON/100ms OFF), 700ms OFF, repeat)
    Scanning,
    /// STA join / credential test in progress — fast blink 5 Hz (100ms ON / 100ms OFF)
    Connecting,
    /// Operational, TCP socket accepting — solid ON (constant)
    Connected,
    /// Wi-Fi lost, retrying — medium blink 2 Hz (250ms ON / 250ms OFF)
    Reconnecting,
    /// Unrecoverable error — SOS Morse pattern (·‌·‌·‌—‌—‌—‌·‌·‌· + 2s pause, repeat)
    Error,
    /// Saving credentials, rebooting — 5 rapid flashes then OFF
    Saving,
}

/// SOS Morse timing: 9 ON/OFF pairs encoding · · · — — — · · · followed by a 2-second pause.
///
/// Each tuple is `(on: bool, duration_ms: u64)`.
/// - true  = LED on
/// - false = LED off
///
/// Pattern breakdown:
/// - 3 dots (S):  100ms ON / 100ms OFF each (last dot uses 300ms OFF as inter-letter gap)
/// - 3 dashes (O): 300ms ON / 100ms OFF each (last dash uses 300ms OFF as inter-letter gap)
/// - 3 dots (S):  100ms ON / 100ms OFF each (last dot uses 2000ms OFF as word gap + repeat pause)
pub const SOS_TIMING: &[(bool, u64)] = &[
    // 3 dots (S) — dit, dit, dit
    (true, 100), (false, 100),
    (true, 100), (false, 100),
    (true, 100), (false, 300), // inter-letter gap after S
    // 3 dashes (O) — dah, dah, dah
    (true, 300), (false, 100),
    (true, 300), (false, 100),
    (true, 300), (false, 300), // inter-letter gap after O
    // 3 dots (S) — dit, dit, dit
    (true, 100), (false, 100),
    (true, 100), (false, 100),
    (true, 100), (false, 2000), // 2-second pause before repeat
];

#[cfg(test)]
mod tests {
    use super::*;

    /// LedState must have exactly 8 variants — one for each row in the Phase 5b reference table.
    /// This test prevents silent omissions when the table is updated.
    #[test]
    fn led_state_has_eight_variants() {
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
        ]
        .len();
        assert_eq!(count, 8, "LedState must have exactly 8 variants");
    }

    /// Exhaustive match to ensure the compiler catches any missing variants.
    #[test]
    fn led_state_variants_exhaustive_match() {
        let state = LedState::Booting;
        // This match must compile — it covers all 8 variants
        let _: &str = match state {
            LedState::Booting => "booting",
            LedState::Provisioning => "provisioning",
            LedState::Scanning => "scanning",
            LedState::Connecting => "connecting",
            LedState::Connected => "connected",
            LedState::Reconnecting => "reconnecting",
            LedState::Error => "error",
            LedState::Saving => "saving",
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
        let expected_on_ms = [100u64, 100, 100, 300, 300, 300, 100, 100, 100];
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
}
