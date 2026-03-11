//! UART interface handler.
//!
//! Supports `read`, `write`, and `configure` actions on UART0 or UART1.

use super::try_r;
use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE, Response};

/// UART configuration parameters, stored per-peripheral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UartConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub parity: UartParity,
    pub stop_bits: u8,
    pub configured: bool,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            baud: 115_200,
            data_bits: 8,
            parity: UartParity::None,
            stop_bits: 1,
            configured: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UartParity {
    None,
    Odd,
    Even,
}

/// Validate the UART peripheral index from a command.
///
/// # Errors
///
/// Returns `Err` if the `uart` field is absent or out of range (0–1).
pub const fn validate_uart<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    super::validate_index(cmd, cmd.uart, 1)
}

/// Handle a UART `configure` command.
///
/// # Errors
///
/// Returns `Err` if the peripheral index is invalid, `baud` is missing or zero,
/// `data_bits` is not 7 or 8, `parity` is unrecognised, or `stop_bits` is not 1 or 2.
pub fn handle_configure<'a>(cmd: &Command<'a>) -> Result<UartConfig, Response<'a>> {
    let _uart_idx = validate_uart(cmd)?;

    let baud = match cmd.baud {
        Some(b) if b > 0 => b,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };

    let data_bits = match cmd.data_bits {
        Some(d) if d == 7 || d == 8 => d,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => 8, // default
    };

    let parity = match cmd.parity {
        Some("none") | None => UartParity::None,
        Some("odd") => UartParity::Odd,
        Some("even") => UartParity::Even,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
    };

    let stop_bits = match cmd.stop_bits {
        Some(s) if s == 1 || s == 2 => s,
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => 1, // default
    };

    Ok(UartConfig {
        baud,
        data_bits,
        parity,
        stop_bits,
        configured: true,
    })
}

/// Handle a UART `write` command.
///
/// `configured` indicates whether the UART has been configured via `configure` first.
/// The caller (router) is responsible for validating the peripheral index.
#[must_use]
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(super::decode_bytes(cmd, cmd.bytes));
    // In the real firmware: write bytes to the UART peripheral
    Response::ok(cmd.id, None)
}

/// Handle a UART `read` command.
///
/// `rx_data` is the data available in the UART RX buffer (provided by the caller).
#[must_use]
pub fn handle_read_with_data<'a>(
    cmd: &'a Command<'a>,
    configured: bool,
    rx_data: &[u8],
) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(validate_uart(cmd));
    let len = try_r!(super::require_positive(cmd, cmd.len));
    super::bytes_response(cmd.id, rx_data, len)
}
