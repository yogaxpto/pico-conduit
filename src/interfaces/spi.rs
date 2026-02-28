//! SPI interface handler.
//!
//! Supports `transfer`, `write`, and `configure` actions on SPI0 or SPI1.

use crate::protocol::{
    Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE, Response,
    ResponseData,
};

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
    let idx = match cmd.spi {
        Some(s) => s,
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };
    if idx > 1 {
        return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE));
    }
    Ok(idx)
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
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let _spi = match validate_spi(cmd) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let mosi = match &cmd.bytes {
        Some(b) if !b.is_empty() => b,
        _ => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    let take = mosi.len().min(miso_data.len()).min(64);
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(&miso_data[..take]).ok();
    Response::ok(cmd.id, Some(ResponseData::Bytes { bytes }))
}

/// Handle a SPI `write` (MOSI only, MISO discarded) command.
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let _spi = match validate_spi(cmd) {
        Ok(s) => s,
        Err(r) => return r,
    };
    match &cmd.bytes {
        Some(b) if !b.is_empty() => {}
        _ => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    Response::ok(cmd.id, None)
}
