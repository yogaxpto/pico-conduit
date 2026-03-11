//! Board-specific constants gated on the active board feature.
//!
//! These values are compile-time constants that compile on both the embedded
//! target and the host test runner.

#[cfg(any(feature = "pico2w", feature = "pico1w"))]
use fixed::FixedU32;
#[cfg(any(feature = "pico2w", feature = "pico1w"))]
use fixed::types::extra::U8;

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
#[allow(clippy::cast_possible_truncation)] // FLASH_SIZE ≤ 4 MB, well within u32 range
pub const CRED_FLASH_OFFSET: u32 = (FLASH_SIZE - CRED_REGION_SIZE) as u32;

/// TCP port for the JSON-over-TCP command interface.
pub const TCP_PORT: u16 = 4242;

/// CYW43439 SPI PIO clock divider for the Pico 2W (RP2350, 150 MHz system clock).
///
/// `0x0180` = 1.5 in `FixedU32<U8>` format → PIO clock = 150 / 1.5 = 100 MHz →
/// SPI SCK ≈ 50 MHz (PIO SPI uses two PIO cycles per SCK period).
/// Conservative step up from `DEFAULT_CLOCK_DIVIDER` (0x0200 → 37.5 MHz SPI).
/// If instability is observed, back off to `0x01C0` (1.75 → ~43 MHz SPI).
#[cfg(feature = "pico2w")]
pub const CYW43_CLOCK_DIVIDER: FixedU32<U8> = FixedU32::from_bits(0x0180);

/// CYW43439 SPI PIO clock divider for the Pico W (RP2040, 125 MHz system clock).
///
/// `0x0140` = 1.25 in `FixedU32<U8>` format → PIO clock = 125 / 1.25 = 100 MHz →
/// SPI SCK ≈ 50 MHz. Conservative step up from `DEFAULT_CLOCK_DIVIDER` (0x0200 → 31 MHz SPI).
#[cfg(feature = "pico1w")]
pub const CYW43_CLOCK_DIVIDER: FixedU32<U8> = FixedU32::from_bits(0x0140);

/// TCP socket receive buffer size in bytes.
///
/// 2× `MAX_MSG_LEN` provides headroom for TCP overhead and allows the CYW43 bridge
/// to burst longer before per-segment ACK round-trips stall throughput.
pub const TCP_RX_BUF_SIZE: usize = 2048;

/// TCP socket transmit buffer size in bytes.
///
/// Outbound responses for SPI/I2C transfers with base64 payloads can approach 1 KB,
/// so 2048 bytes avoids stalling the transmit side on back-to-back large responses.
pub const TCP_TX_BUF_SIZE: usize = 2048;

/// Whether to disable Nagle's algorithm (TCP_NODELAY) on all TCP sockets.
///
/// Nagle's algorithm coalesces small outgoing segments, adding 10–40 ms per response.
/// Setting this to `true` eliminates that latency for short JSON messages.
pub const TCP_NODELAY: bool = true;

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
pub const fn validate_chip_part(actual_part: u16) -> Result<(), &'static str> {
    if actual_part == EXPECTED_CHIP_PART {
        Ok(())
    } else {
        Err("platform mismatch: firmware built for wrong chip")
    }
}
