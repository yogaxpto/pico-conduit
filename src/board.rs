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
