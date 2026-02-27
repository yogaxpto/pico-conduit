//! I2C interface handler.
//!
//! Supports `read`, `write`, `write_read`, and `configure` actions on I2C0 or I2C1.
//! The I2C master operates at 100 kHz or 400 kHz.

use crate::protocol::{
    Command, Response, ResponseData, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED,
    ERROR_VALUE_OUT_OF_RANGE,
};

/// I2C configuration parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct I2cConfig {
    pub freq_hz: u32,
    pub configured: bool,
}

impl Default for I2cConfig {
    fn default() -> Self {
        Self { freq_hz: 100_000, configured: false }
    }
}

/// Validate the I2C peripheral index from a command (0 or 1).
pub fn validate_i2c<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    let idx = match cmd.i2c {
        Some(i) => i,
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };
    if idx > 1 {
        return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE));
    }
    Ok(idx)
}

/// Handle an I2C `configure` command.
pub fn handle_configure<'a>(cmd: &Command<'a>) -> Result<I2cConfig, Response<'a>> {
    let _i2c_idx = validate_i2c(cmd)?;

    let freq_hz = match cmd.freq_hz {
        Some(100_000) | Some(400_000) => cmd.freq_hz.unwrap(),
        Some(_) => return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE)),
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };

    Ok(I2cConfig { freq_hz, configured: true })
}

/// Handle an I2C `read` command.
///
/// `rx_data` is the data the I2C slave would return (provided by caller / mock).
pub fn handle_read<'a>(cmd: &'a Command<'a>, configured: bool, rx_data: &[u8]) -> Response<'a> {
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let _i2c = match validate_i2c(cmd) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let _addr = match cmd.addr {
        Some(a) => a,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
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

/// Handle an I2C `write` command.
pub fn handle_write<'a>(cmd: &Command<'a>, configured: bool) -> Response<'a> {
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let _i2c = match validate_i2c(cmd) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let _addr = match cmd.addr {
        Some(a) => a,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    match &cmd.bytes {
        Some(b) if !b.is_empty() => {}
        _ => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
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
    if !configured {
        return Response::error(cmd.id, ERROR_NOT_CONFIGURED);
    }
    let _i2c = match validate_i2c(cmd) {
        Ok(i) => i,
        Err(r) => return r,
    };
    let _addr = match cmd.addr {
        Some(a) => a,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    match &cmd.write_bytes {
        Some(b) if !b.is_empty() => {}
        _ => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    let read_len = match cmd.read_len {
        Some(l) if l > 0 => l,
        Some(_) => return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE),
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    let take = read_len.min(rx_data.len()).min(64);
    let mut bytes = heapless::Vec::new();
    bytes.extend_from_slice(&rx_data[..take]).ok();
    Response::ok(cmd.id, Some(ResponseData::Bytes { bytes }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Command, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_VALUE_OUT_OF_RANGE};

    fn make_i2c_cmd<'a>(id: &'a str, action: &'a str, i2c: Option<u8>, addr: Option<u8>) -> Command<'a> {
        Command {
            version: Some(1),
            id,
            interface: Some("i2c"),
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
            i2c,
            addr,
            write_bytes: None,
            read_len: None,
            channel: None,
            duty_u16: None,
            adc_channel: None,
        }
    }

    #[test]
    fn i2c_configure_100khz() {
        let mut cmd = make_i2c_cmd("c1", "configure", Some(0), None);
        cmd.freq_hz = Some(100_000);
        let cfg = handle_configure(&cmd).unwrap();
        assert_eq!(cfg.freq_hz, 100_000);
        assert!(cfg.configured);
    }

    #[test]
    fn i2c_configure_400khz() {
        let mut cmd = make_i2c_cmd("c2", "configure", Some(1), None);
        cmd.freq_hz = Some(400_000);
        let cfg = handle_configure(&cmd).unwrap();
        assert_eq!(cfg.freq_hz, 400_000);
    }

    #[test]
    fn i2c_configure_invalid_freq() {
        let mut cmd = make_i2c_cmd("c3", "configure", Some(0), None);
        cmd.freq_hz = Some(200_000); // not 100k or 400k
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn i2c_configure_missing_freq() {
        let cmd = make_i2c_cmd("c4", "configure", Some(0), None);
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn i2c_configure_invalid_i2c_index() {
        let mut cmd = make_i2c_cmd("c5", "configure", Some(2), None);
        cmd.freq_hz = Some(100_000);
        let err = handle_configure(&cmd).unwrap_err();
        assert_eq!(err.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn i2c_read_configured_returns_bytes() {
        let mut cmd = make_i2c_cmd("r1", "read", Some(0), Some(0x48));
        cmd.len = Some(2);
        let rx = [0x0F, 0x42];
        let resp = handle_read(&cmd, true, &rx);
        assert!(resp.ok, "i2c read should succeed: {:?}", resp.error);
        match resp.data {
            Some(ResponseData::Bytes { bytes }) => {
                assert_eq!(bytes.as_slice(), &[0x0F, 0x42]);
            }
            _ => panic!("expected Bytes"),
        }
    }

    #[test]
    fn i2c_read_unconfigured_returns_not_configured() {
        let mut cmd = make_i2c_cmd("r2", "read", Some(0), Some(0x48));
        cmd.len = Some(1);
        let resp = handle_read(&cmd, false, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn i2c_read_missing_addr_returns_missing_field() {
        let mut cmd = make_i2c_cmd("r3", "read", Some(0), None);
        cmd.len = Some(1);
        let resp = handle_read(&cmd, true, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn i2c_write_configured_succeeds() {
        let mut cmd = make_i2c_cmd("w1", "write", Some(0), Some(0x20));
        let mut bytes = heapless::Vec::new();
        bytes.push(0xAA).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, true);
        assert!(resp.ok, "i2c write should succeed: {:?}", resp.error);
    }

    #[test]
    fn i2c_write_unconfigured_returns_not_configured() {
        let mut cmd = make_i2c_cmd("w2", "write", Some(0), Some(0x20));
        let mut bytes = heapless::Vec::new();
        bytes.push(0x00).ok();
        cmd.bytes = Some(bytes);
        let resp = handle_write(&cmd, false);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
    }

    #[test]
    fn i2c_write_read_configured_returns_read_bytes() {
        let mut cmd = make_i2c_cmd("wr1", "write_read", Some(0), Some(0x68));
        let mut wb = heapless::Vec::new();
        wb.push(0x00).ok(); // register address
        cmd.write_bytes = Some(wb);
        cmd.read_len = Some(2);
        let rx = [0x12, 0x34];
        let resp = handle_write_read(&cmd, true, &rx);
        assert!(resp.ok, "write_read should succeed: {:?}", resp.error);
        match resp.data {
            Some(ResponseData::Bytes { bytes }) => {
                assert_eq!(bytes.as_slice(), &[0x12, 0x34]);
            }
            _ => panic!("expected Bytes"),
        }
    }

    #[test]
    fn i2c_write_read_missing_write_bytes() {
        let mut cmd = make_i2c_cmd("wr2", "write_read", Some(0), Some(0x68));
        cmd.read_len = Some(2);
        let resp = handle_write_read(&cmd, true, &[0x00]);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }
}
