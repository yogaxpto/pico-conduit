//! LED status signaling.
//!
//! Defines [`LedState`], [`LedPattern`], and [`SOS_TIMING`]. These are `no_std`-compatible
//! and fully testable on the host.
//!
//! [`LED_SIGNAL`] is also defined here, gated to `target_os = "none"` because it depends
//! on `embassy_sync`. `led_task` and `set_led` live in `src/net.rs` (CYW43-specific).
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
//! | `Rebooting` | 10 rapid flashes then OFF | 10×(50ms ON/50ms OFF) then OFF | USB bootloader imminent |

// ── Pattern step sequences ────────────────────────────────────────────────────
// Each entry is (on: bool, duration_ms: u16).  The runner sets the LED to `on`
// then waits `duration_ms` milliseconds before advancing to the next step.

const BOOTING_STEPS: &[(bool, u16)] = &[
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (false, 1000), // 1 s trailing gap before repeating
];

const PROVISIONING_STEPS: &[(bool, u16)] = &[(true, 1000), (false, 1000)];

const SCANNING_STEPS: &[(bool, u16)] = &[
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (false, 700), // 700 ms gap before repeating
];

const CONNECTING_STEPS: &[(bool, u16)] = &[(true, 100), (false, 100)];

const RECONNECTING_STEPS: &[(bool, u16)] = &[(true, 250), (false, 250)];

// 5 rapid flashes — 10 steps
const SAVING_STEPS: &[(bool, u16)] = &[
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
];

// 10 rapid flashes — 20 steps
const REBOOTING_STEPS: &[(bool, u16)] = &[
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
    (true, 50),
    (false, 50),
];

// ── LedPattern ────────────────────────────────────────────────────────────────

/// Describes how the LED driver should play a state's pattern.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LedPattern {
    /// Set the LED to a fixed state and hold until the next signal.
    Solid(bool),
    /// Repeat the step sequence indefinitely until a new signal arrives.
    Repeat(&'static [(bool, u16)]),
    /// Play the step sequence once, turn the LED off, then wait for the next signal.
    OneShot(&'static [(bool, u16)]),
}

// ── LedState ──────────────────────────────────────────────────────────────────

/// All possible LED states for the device.
///
/// The 9 variants correspond to the LED Status Reference table in OBJECTIVE.md Phase 5b
/// and Phase 9a (Rebooting).
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
    /// USB bootloader reboot imminent — 10 rapid flashes then OFF (10×(50ms ON/50ms OFF))
    Rebooting,
}

impl LedState {
    /// Returns the [`LedPattern`] for this state.
    pub fn pattern(self) -> LedPattern {
        match self {
            LedState::Booting => LedPattern::Repeat(BOOTING_STEPS),
            LedState::Provisioning => LedPattern::Repeat(PROVISIONING_STEPS),
            LedState::Scanning => LedPattern::Repeat(SCANNING_STEPS),
            LedState::Connecting => LedPattern::Repeat(CONNECTING_STEPS),
            LedState::Connected => LedPattern::Solid(true),
            LedState::Reconnecting => LedPattern::Repeat(RECONNECTING_STEPS),
            LedState::Error => LedPattern::Repeat(SOS_TIMING),
            LedState::Saving => LedPattern::OneShot(SAVING_STEPS),
            LedState::Rebooting => LedPattern::OneShot(REBOOTING_STEPS),
        }
    }
}

// ── SOS timing ────────────────────────────────────────────────────────────────

/// SOS Morse timing: 9 ON/OFF pairs encoding · · · — — — · · · followed by a 2-second pause.
///
/// Each tuple is `(on: bool, duration_ms: u16)`.
/// - true  = LED on
/// - false = LED off
///
/// Pattern breakdown:
/// - 3 dots (S):  100ms ON / 100ms OFF each (last dot uses 300ms OFF as inter-letter gap)
/// - 3 dashes (O): 300ms ON / 100ms OFF each (last dash uses 300ms OFF as inter-letter gap)
/// - 3 dots (S):  100ms ON / 100ms OFF each (last dot uses 2000ms OFF as word gap + repeat pause)
pub const SOS_TIMING: &[(bool, u16)] = &[
    // 3 dots (S) — dit, dit, dit
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 300), // inter-letter gap after S
    // 3 dashes (O) — dah, dah, dah
    (true, 300),
    (false, 100),
    (true, 300),
    (false, 100),
    (true, 300),
    (false, 300), // inter-letter gap after O
    // 3 dots (S) — dit, dit, dit
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 100),
    (true, 100),
    (false, 2000), // 2-second pause before repeat
];

// ── LED_SIGNAL (embedded only) ────────────────────────────────────────────────

#[cfg(target_os = "none")]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

/// Signal any LED state change from anywhere in the firmware.
///
/// Gated to `target_os = "none"` because it depends on `embassy_sync`.
#[cfg(target_os = "none")]
pub static LED_SIGNAL: Signal<CriticalSectionRawMutex, LedState> = Signal::new();
