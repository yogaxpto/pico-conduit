//! SPI interface handler.
//!
//! Supports `transfer`, `write`, and `configure` actions on SPI0 or SPI1.

use super::try_r;
use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE, Response};

/// SPI configuration parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct SpiConfig {
    pub freq_hz: u32,
    pub cpol: u8,
    pub cpha: u8,
    pub configured: bool,
}

impl Default for SpiConfig {
    fn default() -> Self {
        Self {
            freq_hz: 1_000_000,
            cpol: 0,
            cpha: 0,
            configured: false,
        }
    }
}

/// Validate the SPI peripheral index from a command (0 or 1).
pub fn validate_spi<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    super::validate_index(cmd, cmd.spi, 1)
}

/// Handle a SPI `configure` command.
pub fn handle_configure<'a>(cmd: &Command<'a>) -> Result<SpiConfig, Response<'a>> {
    let _spi_idx = validate_spi(cmd)?;

    let freq_hz = match cmd.freq_hz {
        Some(f) if f > 0 => f,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };

    let cpol = match cmd.cpol {
        Some(p) if p <= 1 => p,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => 0,
    };

    let cpha = match cmd.cpha {
        Some(p) if p <= 1 => p,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => 0,
    };

    Ok(SpiConfig {
        freq_hz,
        cpol,
        cpha,
        configured: true,
    })
}

/// Handle a SPI `transfer` (full-duplex) command.
///
/// `miso_data` is the data that the SPI slave would return (provided by caller / mock).
pub fn handle_transfer<'a>(
    cmd: &'a Command<'a>,
    configured: bool,
    miso_data: &[u8],
) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(validate_spi(cmd));
    let mosi = try_r!(super::decode_bytes(cmd, cmd.bytes));
    super::bytes_response(cmd.id, miso_data, mosi.len())
}

/// Handle a SPI `write` (MOSI only, MISO discarded) command.
///
/// The caller (router) is responsible for validating the peripheral index.
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(super::decode_bytes(cmd, cmd.bytes));
    Response::ok(cmd.id, None)
}
