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
