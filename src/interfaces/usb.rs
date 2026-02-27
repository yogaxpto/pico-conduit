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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED};

    fn make_usb_cmd<'a>(id: &'a str, action: &'a str) -> Command<'a> {
        Command {
            version: Some(1),
            id,
            interface: Some("usb"),
            action: Some(action),
            pin: None,
            value: None,
            mode: None,
            pull: None,
            uart: None,
            bytes: None,
            len: None,
            baud: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            spi: None,
            freq_hz: None,
            cpol: None,
            cpha: None,
            i2c: None,
            addr: None,
            write_bytes: None,
            read_len: None,
            channel: None,
            duty_u16: None,
            adc_channel: None,
        }
    }

    #[test]
    fn usb_write_configured_succeeds() {
        let mut cmd = make_usb_cmd("w1", "write");
        let mut bytes = heapless::Vec::new();
        bytes.extend_from_slice(&[b'H', b'i']).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, true);
        assert!(resp.ok, "usb write should succeed: {:?}", resp.error);
    }

    #[test]
    fn usb_write_unconfigured_returns_not_configured() {
        let mut cmd = make_usb_cmd("w2", "write");
        let mut bytes = heapless::Vec::new();
        bytes.push(0x00).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, false);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn usb_write_missing_bytes_returns_missing_field() {
        let cmd = make_usb_cmd("w3", "write");
        let resp = handle_write(&cmd, true);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn usb_read_configured_returns_bytes() {
        let mut cmd = make_usb_cmd("r1", "read");
        cmd.len = Some(3);
        let rx = [0x41, 0x42, 0x43]; // "ABC"
        let resp = handle_read_with_data(&cmd, true, &rx);
        assert!(resp.ok, "usb read should succeed: {:?}", resp.error);
        match resp.data {
            Some(ResponseData::Bytes { bytes }) => {
                assert_eq!(bytes.as_slice(), &[0x41, 0x42, 0x43]);
            }
            _ => panic!("expected Bytes"),
        }
    }

    #[test]
    fn usb_read_unconfigured_returns_not_configured() {
        let mut cmd = make_usb_cmd("r2", "read");
        cmd.len = Some(1);
        let resp = handle_read_with_data(&cmd, false, &[]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn usb_read_missing_len_returns_missing_field() {
        let cmd = make_usb_cmd("r3", "read");
        let resp = handle_read_with_data(&cmd, true, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }
}
