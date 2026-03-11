//! USB CDC (virtual serial port) interface handler.
//!
//! Supports `read` and `write` actions on the USB CDC ACM virtual serial port.

use super::try_r;
use crate::protocol::{Command, Response};

/// Handle a USB CDC `write` command.
///
/// `configured` indicates whether the USB device stack is initialized and connected.
#[must_use]
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    try_r!(super::decode_bytes(cmd, cmd.bytes));
    // In the real firmware: write bytes to the USB CDC ACM class
    Response::ok(cmd.id, None)
}

/// Handle a USB CDC `read` command.
///
/// `rx_data` is the data available from the USB CDC RX buffer (provided by caller / mock).
#[must_use]
pub fn handle_read_with_data<'a>(
    cmd: &'a Command<'a>,
    configured: bool,
    rx_data: &[u8],
) -> Response<'a> {
    try_r!(super::check_configured(cmd, configured));
    let len = try_r!(super::require_positive(cmd, cmd.len));
    super::bytes_response(cmd.id, rx_data, len)
}
