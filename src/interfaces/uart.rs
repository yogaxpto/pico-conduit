//! UART interface handler.
//!
//! Supports `read`, `write`, and `configure` actions on UART0 or UART1.

use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE, Response};

/// UART configuration parameters, stored per-peripheral.
#[derive(Clone, Debug, PartialEq)]
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
            baud: 115200,
            data_bits: 8,
            parity: UartParity::None,
            stop_bits: 1,
            configured: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UartParity {
    None,
    Odd,
    Even,
}

/// Validate the UART peripheral index from a command.
pub fn validate_uart<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    super::validate_index(cmd, cmd.uart, 1)
}

/// Handle a UART `configure` command.
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
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    if let Err(r) = super::check_configured(cmd, configured) {
        return r;
    }
    let _bytes = match &cmd.bytes {
        Some(b) if !b.is_empty() => b,
        Some(_) => return Response::error(cmd.id, ERROR_MISSING_FIELD),
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    // In the real firmware: write bytes to the UART peripheral
    Response::ok(cmd.id, None)
}

/// Handle a UART `read` command.
///
/// `rx_data` is the data available in the UART RX buffer (provided by the caller).
pub fn handle_read_with_data<'a>(
    cmd: &'a Command<'a>,
    configured: bool,
    rx_data: &[u8],
) -> Response<'a> {
    if let Err(r) = super::check_configured(cmd, configured) {
        return r;
    }
    let _uart = match validate_uart(cmd) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let len = match cmd.len {
        Some(l) if l > 0 => l,
        Some(_) => return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE),
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    super::bytes_response(cmd.id, rx_data, len)
}
