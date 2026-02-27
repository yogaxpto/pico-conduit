//! UART interface handler.
//!
//! Supports `read`, `write`, and `configure` actions on UART0 or UART1.

use crate::protocol::{
    Command, Response, ResponseData, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED,
    ERROR_VALUE_OUT_OF_RANGE,
};

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
    let idx = match cmd.uart {
        Some(u) => u,
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };
    if idx > 1 {
        return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE));
    }
    Ok(idx)
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
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let _uart = match validate_uart(cmd) {
        Ok(u) => u,
        Err(r) => return r,
    };
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
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
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
    let take = len.min(rx_data.len()).min(64);
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(&rx_data[..take]).ok();
    Response::ok(cmd.id, Some(ResponseData::Bytes { bytes }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE};

    fn make_uart_cmd<'a>(
        id: &'a str,
        action: &'a str,
        uart: Option<u8>,
    ) -> Command<'a> {
        Command {
            version: Some(1),
            id,
            interface: Some("uart"),
            action: Some(action),
            pin: None,
            value: None,
            mode: None,
            pull: None,
            uart,
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
    fn uart_configure_valid() {
        let mut cmd = make_uart_cmd("c1", "configure", Some(0));
        cmd.baud = Some(115200);
        cmd.data_bits = Some(8);
        cmd.parity = Some("none");
        cmd.stop_bits = Some(1);
        let cfg = handle_configure(&cmd).unwrap();
        assert_eq!(cfg.baud, 115200);
        assert_eq!(cfg.data_bits, 8);
        assert_eq!(cfg.parity, UartParity::None);
        assert_eq!(cfg.stop_bits, 1);
        assert!(cfg.configured);
    }

    #[test]
    fn uart_configure_invalid_uart_index() {
        let mut cmd = make_uart_cmd("c2", "configure", Some(2)); // UART 0 or 1 only
        cmd.baud = Some(9600);
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn uart_configure_missing_baud() {
        let cmd = make_uart_cmd("c3", "configure", Some(0));
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn uart_configure_invalid_data_bits() {
        let mut cmd = make_uart_cmd("c4", "configure", Some(0));
        cmd.baud = Some(9600);
        cmd.data_bits = Some(6); // invalid
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn uart_configure_even_parity() {
        let mut cmd = make_uart_cmd("c5", "configure", Some(1));
        cmd.baud = Some(57600);
        cmd.parity = Some("even");
        let cfg = handle_configure(&cmd).unwrap();
        assert_eq!(cfg.parity, UartParity::Even);
    }

    #[test]
    fn uart_write_unconfigured_returns_not_configured() {
        let mut cmd = make_uart_cmd("w1", "write", Some(0));
        let mut bytes = heapless::Vec::new();
        bytes.push(0x41).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, false);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn uart_write_configured_succeeds() {
        let mut cmd = make_uart_cmd("w2", "write", Some(0));
        let mut bytes = heapless::Vec::new();
        bytes.extend_from_slice(&[b'H', b'i']).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, true);
        assert!(resp.ok, "configured write should succeed: {:?}", resp.error);
    }

    #[test]
    fn uart_write_missing_bytes_returns_missing_field() {
        let cmd = make_uart_cmd("w3", "write", Some(0));
        let resp = handle_write(&cmd, true);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn uart_read_configured_returns_bytes() {
        let mut cmd = make_uart_cmd("r1", "read", Some(0));
        cmd.len = Some(3);
        let data = [0x48u8, 0x65, 0x6C]; // "Hel"
        let resp = handle_read_with_data(&cmd, true, &data);
        assert!(resp.ok, "configured read should succeed: {:?}", resp.error);
        match resp.data {
            Some(ResponseData::Bytes { bytes }) => {
                assert_eq!(bytes.as_slice(), &[0x48, 0x65, 0x6C]);
            }
            _ => panic!("expected Bytes response"),
        }
    }

    #[test]
    fn uart_read_unconfigured_returns_not_configured() {
        let mut cmd = make_uart_cmd("r2", "read", Some(0));
        cmd.len = Some(4);
        let resp = handle_read_with_data(&cmd, false, &[]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn uart_read_missing_len_returns_missing_field() {
        let cmd = make_uart_cmd("r3", "read", Some(0));
        let resp = handle_read_with_data(&cmd, true, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }
}
