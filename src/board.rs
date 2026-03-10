//! Board-specific constants gated on the active board feature.
//!
//! These values are compile-time constants that compile on both the embedded
//! target and the host test runner.

/// Total flash size for the active board variant (see `memory-pico*.x`).
#[cfg(feature = "pico2w")]
pub const FLASH_SIZE: usize = 4 * 1024 * 1024;
/// Total flash size for the active board variant (see `memory-pico*.x`).
#[cfg(feature = "pico1w")]
pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

/// Size of the credential storage region in bytes.
pub const CRED_REGION_SIZE: usize = 8 * 1024;

/// Credential storage flash offset (last 8 KB of flash).
#[cfg(any(feature = "pico2w", feature = "pico1w"))]
pub const CRED_FLASH_OFFSET: u32 = (FLASH_SIZE - CRED_REGION_SIZE) as u32;

/// TCP port for the JSON-over-TCP command interface.
pub const TCP_PORT: u16 = 4242;

/// WebSocket port for the JSON-over-WebSocket command interface.
pub const WS_PORT: u16 = 4243;

/// Expected SYSINFO CHIP_ID PART value for the active board.
#[cfg(feature = "pico2w")]
pub const EXPECTED_CHIP_PART: u16 = 0x4;
/// Expected SYSINFO CHIP_ID PART value for the active board.
#[cfg(feature = "pico1w")]
pub const EXPECTED_CHIP_PART: u16 = 0x2;

/// Validate that a SYSINFO CHIP_ID PART value matches the expected board.
///
/// Returns `Ok(())` on match, `Err` with diagnostic on mismatch.
#[cfg(any(feature = "pico2w", feature = "pico1w"))]
pub fn validate_chip_part(actual_part: u16) -> Result<(), &'static str> {
    if actual_part == EXPECTED_CHIP_PART {
        Ok(())
    } else {
        Err("platform mismatch: firmware built for wrong chip")
    }
}
