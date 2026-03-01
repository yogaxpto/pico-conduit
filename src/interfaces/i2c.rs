//! I2C interface handler.
//!
//! Supports `read`, `write`, `write_read`, and `configure` actions on I2C0 or I2C1.
//! The I2C master operates at 100 kHz or 400 kHz.

use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE, Response};
use super::try_r;

/// I2C configuration parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct I2cConfig {
    pub freq_hz: u32,
    pub configured: bool,
}

impl Default for I2cConfig {
    fn default() -> Self {
        Self {
            freq_hz: 100_000,
            configured: false,
        }
    }
}

/// Validate the I2C peripheral index from a command (0 or 1).
pub fn validate_i2c<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    super::validate_index(cmd, cmd.i2c, 1)
}

/// Validate the I2C device address from a command.
fn validate_addr<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    match cmd.addr {
        Some(a) => Ok(a),
        None => Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    }
}

/// Handle an I2C `configure` command.
pub fn handle_configure<'a>(cmd: &Command<'a>) -> Result<I2cConfig, Response<'a>> {
    let _i2c_idx = validate_i2c(cmd)?;

    let freq_hz = match cmd.freq_hz {
        Some(100_000) | Some(400_000) => cmd.freq_hz.unwrap(),
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };

    Ok(I2cConfig {
        freq_hz,
        configured: true,
    })
}

/// Handle an I2C `read` command.
///
/// `rx_data` is the data the I2C slave would return (provided by caller / mock).
pub fn handle_read<'a>(cmd: &'a Command<'a>, configured: bool, rx_data: &[u8]) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(validate_i2c(cmd));
    try_r!(validate_addr(cmd));
    let len = try_r!(super::require_positive(cmd, cmd.len));
    super::bytes_response(cmd.id, rx_data, len)
}

/// Handle an I2C `write` command.
///
/// The caller (router) is responsible for validating the peripheral index.
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(validate_addr(cmd));
    try_r!(super::require_bytes(cmd, cmd.bytes.as_ref()));
    Response::ok(cmd.id, None)
}

/// Handle an I2C `write_read` command.
///
/// Writes `write_bytes`, then reads `read_len` bytes. `rx_data` is what the slave returns.
pub fn handle_write_read<'a>(
    cmd: &'a Command<'a>,
    configured: bool,
    rx_data: &[u8],
) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(validate_i2c(cmd));
    try_r!(validate_addr(cmd));
    try_r!(super::require_bytes(cmd, cmd.write_bytes.as_ref()));
    let read_len = try_r!(super::require_positive(cmd, cmd.read_len));
    super::bytes_response(cmd.id, rx_data, read_len)
}
