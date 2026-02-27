//! SPI interface handler.
//!
//! Supports `transfer`, `write`, and `configure` actions on SPI0 or SPI1.

use crate::protocol::{
    Command, Response, ResponseData, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED,
    ERROR_VALUE_OUT_OF_RANGE,
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
        Self { freq_hz: 1_000_000, cpol: 0, cpha: 0, configured: false }
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

    Ok(SpiConfig { freq_hz, cpol, cpha, configured: true })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE};

    fn make_spi_cmd<'a>(id: &'a str, action: &'a str, spi: Option<u8>) -> Command<'a> {
        Command {
            version: Some(1),
            id,
            interface: Some("spi"),
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
            spi,
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
    fn spi_configure_valid() {
        let mut cmd = make_spi_cmd("c1", "configure", Some(0));
        cmd.freq_hz = Some(1_000_000);
        cmd.cpol = Some(0);
        cmd.cpha = Some(0);
        let cfg = handle_configure(&cmd).unwrap();
        assert_eq!(cfg.freq_hz, 1_000_000);
        assert_eq!(cfg.cpol, 0);
        assert_eq!(cfg.cpha, 0);
        assert!(cfg.configured);
    }

    #[test]
    fn spi_configure_missing_freq_returns_missing_field() {
        let cmd = make_spi_cmd("c2", "configure", Some(0));
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn spi_configure_invalid_spi_index() {
        let mut cmd = make_spi_cmd("c3", "configure", Some(2));
        cmd.freq_hz = Some(1_000_000);
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn spi_configure_invalid_cpol() {
        let mut cmd = make_spi_cmd("c4", "configure", Some(0));
        cmd.freq_hz = Some(1_000_000);
        cmd.cpol = Some(2); // invalid
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn spi_transfer_configured_returns_miso_bytes() {
        let mut cmd = make_spi_cmd("t1", "transfer", Some(0));
        let mut mosi = heapless::Vec::new();
        mosi.extend_from_slice(&[0xDE, 0xAD]).ok();
        cmd.bytes = Some(mosi);
        let miso = [0xBE, 0xEF];
        let resp = handle_transfer(&cmd, true, &miso);
        assert!(resp.ok, "transfer should succeed: {:?}", resp.error);
        match resp.data {
            Some(ResponseData::Bytes { bytes }) => {
                assert_eq!(bytes.as_slice(), &[0xBE, 0xEF]);
            }
            _ => panic!("expected Bytes response"),
        }
    }

    #[test]
    fn spi_transfer_unconfigured_returns_not_configured() {
        let mut cmd = make_spi_cmd("t2", "transfer", Some(0));
        let mut bytes = heapless::Vec::new();
        bytes.push(0x00).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_transfer(&cmd, false, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn spi_transfer_missing_bytes_returns_missing_field() {
        let cmd = make_spi_cmd("t3", "transfer", Some(0));
        let resp = handle_transfer(&cmd, true, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn spi_write_configured_succeeds() {
        let mut cmd = make_spi_cmd("w1", "write", Some(1));
        let mut bytes = heapless::Vec::new();
        bytes.push(0xAB).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, true);
        assert!(resp.ok, "spi write should succeed");
    }

    #[test]
    fn spi_write_unconfigured_returns_not_configured() {
        let mut cmd = make_spi_cmd("w2", "write", Some(0));
        let mut bytes = heapless::Vec::new();
        bytes.push(0x00).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, false);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }
}
