//! USB CDC (virtual serial port) interface handler.
//!
//! Supports `read` and `write` actions on the USB CDC ACM virtual serial port.

use crate::protocol::{
    Command, Response, ResponseData, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED,
    ERROR_VALUE_OUT_OF_RANGE,
};

/// Handle a USB CDC `write` command.
///
/// `configured` indicates whether the USB device stack is initialized and connected.
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    match &cmd.bytes {
        Some(b) if !b.is_empty() => {}
        _ => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    // In the real firmware: write bytes to the USB CDC ACM class
    Response::ok(cmd.id, None)
}

/// Handle a USB CDC `read` command.
///
/// `rx_data` is the data available from the USB CDC RX buffer (provided by caller / mock).
pub fn handle_read_with_data<'a>(
    cmd: &'a Command<'a>,
    configured: bool,
    rx_data: &[u8],
) -> Response<'a> {
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let len = match cmd.len {
        Some(l) if l > 0 => l,
        Some(_) => return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE),
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    let take = len.min(rx_data.len()).min(64);
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(&rx_data[..take]).ok();
    Response::ok(cmd.id, Some(ResponseData::Bytes { bytes }))
}
