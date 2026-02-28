//! Typed TCP client for the pico-socketeer firmware.
//!
//! # Example
//!
//! ```no_run
//! use pico_socketeer_client::PicoSocketeer;
//!
//! let mut pico = PicoSocketeer::connect("192.168.1.100:4242").unwrap();
//! pico.gpio_write(15, 1).unwrap();
//! let (raw, voltage) = pico.adc_read(0).unwrap();
//! println!("ADC raw={raw}, voltage={voltage:.3}V");
//! ```

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    /// The firmware returned `ok: false` with an error code.
    Firmware(String),
    /// The response was missing an expected field.
    BadResponse(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
            Error::Firmware(s) => write!(f, "firmware error: {s}"),
            Error::BadResponse(s) => write!(f, "bad response: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

// ── Response types ────────────────────────────────────────────────────────────

/// Raw response from the firmware.
#[derive(Debug, Deserialize)]
pub struct RawResponse {
    pub id: String,
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

/// Wi-Fi configuration info returned by `config/get`.
#[derive(Debug, Deserialize)]
pub struct ConfigInfo {
    pub ssid: String,
    pub ip: String,
    pub connected: bool,
}

// ── Client ────────────────────────────────────────────────────────────────────

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> String {
    NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string()
}

/// Typed TCP client for the pico-socketeer firmware.
///
/// Holds a `BufReader` over a cloned `TcpStream` for reading, and the original
/// stream for writing, so both halves can coexist.
pub struct PicoSocketeer {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl PicoSocketeer {
    /// Connect to a pico-socketeer device.
    ///
    /// `addr` is any value implementing `ToSocketAddrs`, e.g. `"192.168.1.100:4242"`.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);
        Ok(Self { reader, writer })
    }

    /// Send an arbitrary JSON value as a command and return the raw response.
    pub fn send_raw(&mut self, cmd: &Value) -> Result<RawResponse> {
        let mut line = serde_json::to_string(cmd)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;

        let mut resp_line = String::new();
        self.reader.read_line(&mut resp_line)?;
        let resp: RawResponse = serde_json::from_str(resp_line.trim())?;
        if !resp.ok {
            return Err(Error::Firmware(
                resp.error.unwrap_or_else(|| "unknown error".into()),
            ));
        }
        Ok(resp)
    }

    // ── Helper ────────────────────────────────────────────────────────────

    fn cmd(&mut self, fields: serde_json::Map<String, Value>) -> Result<RawResponse> {
        let mut map = fields;
        map.insert("version".into(), Value::Number(1.into()));
        map.insert("id".into(), Value::String(next_id()));
        self.send_raw(&Value::Object(map))
    }

    // ── GPIO ──────────────────────────────────────────────────────────────

    /// Write a digital value to a GPIO pin (0 = low, 1 = high).
    pub fn gpio_write(&mut self, pin: u8, value: u8) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "gpio".into());
        m.insert("action".into(), "write".into());
        m.insert("pin".into(), pin.into());
        m.insert("value".into(), value.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Read the current digital value of a GPIO pin. Returns 0 or 1.
    pub fn gpio_read(&mut self, pin: u8) -> Result<u8> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "gpio".into());
        m.insert("action".into(), "read".into());
        m.insert("pin".into(), pin.into());
        let resp = self.cmd(m)?;
        let v = resp
            .data
            .as_ref()
            .and_then(|d| d.get("value"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| Error::BadResponse("missing data.value".into()))?;
        Ok(v as u8)
    }

    /// Configure a GPIO pin mode and pull resistor.
    ///
    /// `mode`: `"input"` or `"output"`.
    /// `pull`: `"up"`, `"down"`, or `"none"`.
    pub fn gpio_set_mode(&mut self, pin: u8, mode: &str, pull: &str) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "gpio".into());
        m.insert("action".into(), "set_mode".into());
        m.insert("pin".into(), pin.into());
        m.insert("mode".into(), mode.into());
        m.insert("pull".into(), pull.into());
        self.cmd(m)?;
        Ok(())
    }

    // ── ADC ───────────────────────────────────────────────────────────────

    /// Read an ADC channel. Returns `(raw_12bit, voltage_volts)`.
    ///
    /// `channel`: 0 = GPIO26, 1 = GPIO27, 2 = GPIO28.
    pub fn adc_read(&mut self, channel: u8) -> Result<(u16, f32)> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "adc".into());
        m.insert("action".into(), "read".into());
        m.insert("adc_channel".into(), channel.into());
        let resp = self.cmd(m)?;
        let data = resp
            .data
            .as_ref()
            .ok_or_else(|| Error::BadResponse("missing data".into()))?;
        let raw = data
            .get("raw")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| Error::BadResponse("missing data.raw".into()))? as u16;
        let voltage = data
            .get("voltage")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| Error::BadResponse("missing data.voltage".into()))?
            as f32;
        Ok((raw, voltage))
    }

    /// Read the onboard temperature sensor. Returns degrees Celsius.
    pub fn adc_temp(&mut self) -> Result<f32> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "adc".into());
        m.insert("action".into(), "read".into());
        m.insert("adc_channel".into(), 3u8.into()); // Temp = channel 3
        let resp = self.cmd(m)?;
        let data = resp
            .data
            .as_ref()
            .ok_or_else(|| Error::BadResponse("missing data".into()))?;
        let celsius = data
            .get("celsius")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| Error::BadResponse("missing data.celsius".into()))?
            as f32;
        Ok(celsius)
    }

    // ── UART ──────────────────────────────────────────────────────────────

    /// Configure a UART peripheral.
    pub fn uart_configure(
        &mut self,
        uart: u8,
        baud: u32,
        data_bits: u8,
        parity: &str,
        stop_bits: u8,
    ) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "uart".into());
        m.insert("action".into(), "configure".into());
        m.insert("uart".into(), uart.into());
        m.insert("baud".into(), baud.into());
        m.insert("data_bits".into(), data_bits.into());
        m.insert("parity".into(), parity.into());
        m.insert("stop_bits".into(), stop_bits.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Write bytes to a UART peripheral.
    pub fn uart_write(&mut self, uart: u8, bytes: &[u8]) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "uart".into());
        m.insert("action".into(), "write".into());
        m.insert("uart".into(), uart.into());
        m.insert("bytes".into(), bytes_to_json(bytes));
        self.cmd(m)?;
        Ok(())
    }

    /// Read up to `len` bytes from a UART peripheral.
    pub fn uart_read(&mut self, uart: u8, len: usize) -> Result<Vec<u8>> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "uart".into());
        m.insert("action".into(), "read".into());
        m.insert("uart".into(), uart.into());
        m.insert("len".into(), len.into());
        let resp = self.cmd(m)?;
        extract_bytes(resp)
    }

    // ── SPI ───────────────────────────────────────────────────────────────

    /// Configure a SPI peripheral.
    pub fn spi_configure(&mut self, spi: u8, freq_hz: u32, cpol: u8, cpha: u8) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "spi".into());
        m.insert("action".into(), "configure".into());
        m.insert("spi".into(), spi.into());
        m.insert("freq_hz".into(), freq_hz.into());
        m.insert("cpol".into(), cpol.into());
        m.insert("cpha".into(), cpha.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Write bytes to a SPI peripheral (MOSI only).
    pub fn spi_write(&mut self, spi: u8, bytes: &[u8]) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "spi".into());
        m.insert("action".into(), "write".into());
        m.insert("spi".into(), spi.into());
        m.insert("bytes".into(), bytes_to_json(bytes));
        self.cmd(m)?;
        Ok(())
    }

    /// Full-duplex SPI transfer. Returns the MISO bytes.
    pub fn spi_transfer(&mut self, spi: u8, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "spi".into());
        m.insert("action".into(), "transfer".into());
        m.insert("spi".into(), spi.into());
        m.insert("bytes".into(), bytes_to_json(bytes));
        let resp = self.cmd(m)?;
        extract_bytes(resp)
    }

    // ── I2C ───────────────────────────────────────────────────────────────

    /// Configure an I2C peripheral. `freq_hz` must be 100_000 or 400_000.
    pub fn i2c_configure(&mut self, i2c: u8, freq_hz: u32) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "i2c".into());
        m.insert("action".into(), "configure".into());
        m.insert("i2c".into(), i2c.into());
        m.insert("freq_hz".into(), freq_hz.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Write bytes to an I2C device.
    pub fn i2c_write(&mut self, i2c: u8, addr: u8, bytes: &[u8]) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "i2c".into());
        m.insert("action".into(), "write".into());
        m.insert("i2c".into(), i2c.into());
        m.insert("addr".into(), addr.into());
        m.insert("bytes".into(), bytes_to_json(bytes));
        self.cmd(m)?;
        Ok(())
    }

    /// Read `len` bytes from an I2C device.
    pub fn i2c_read(&mut self, i2c: u8, addr: u8, len: usize) -> Result<Vec<u8>> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "i2c".into());
        m.insert("action".into(), "read".into());
        m.insert("i2c".into(), i2c.into());
        m.insert("addr".into(), addr.into());
        m.insert("len".into(), len.into());
        let resp = self.cmd(m)?;
        extract_bytes(resp)
    }

    /// Write bytes then read `read_len` bytes from an I2C device (combined transaction).
    pub fn i2c_write_read(
        &mut self,
        i2c: u8,
        addr: u8,
        write_bytes: &[u8],
        read_len: usize,
    ) -> Result<Vec<u8>> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "i2c".into());
        m.insert("action".into(), "write_read".into());
        m.insert("i2c".into(), i2c.into());
        m.insert("addr".into(), addr.into());
        m.insert("write_bytes".into(), bytes_to_json(write_bytes));
        m.insert("read_len".into(), read_len.into());
        let resp = self.cmd(m)?;
        extract_bytes(resp)
    }

    // ── PWM ───────────────────────────────────────────────────────────────

    /// Set the PWM duty cycle for a channel (0 = always off, 65535 = always on).
    pub fn pwm_set_duty(&mut self, channel: u8, duty_u16: u16) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "pwm".into());
        m.insert("action".into(), "set_duty".into());
        m.insert("channel".into(), channel.into());
        m.insert("duty_u16".into(), duty_u16.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Set the PWM frequency for a channel's slice.
    pub fn pwm_set_freq(&mut self, channel: u8, freq_hz: u32) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "pwm".into());
        m.insert("action".into(), "set_freq".into());
        m.insert("channel".into(), channel.into());
        m.insert("freq_hz".into(), freq_hz.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Enable a PWM channel.
    pub fn pwm_enable(&mut self, channel: u8) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "pwm".into());
        m.insert("action".into(), "enable".into());
        m.insert("channel".into(), channel.into());
        self.cmd(m)?;
        Ok(())
    }

    /// Disable a PWM channel.
    pub fn pwm_disable(&mut self, channel: u8) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "pwm".into());
        m.insert("action".into(), "disable".into());
        m.insert("channel".into(), channel.into());
        self.cmd(m)?;
        Ok(())
    }

    // ── USB CDC ───────────────────────────────────────────────────────────

    /// Write bytes to the USB CDC virtual serial port.
    pub fn usb_write(&mut self, bytes: &[u8]) -> Result<()> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "usb".into());
        m.insert("action".into(), "write".into());
        m.insert("bytes".into(), bytes_to_json(bytes));
        self.cmd(m)?;
        Ok(())
    }

    /// Read up to `len` bytes from the USB CDC virtual serial port.
    pub fn usb_read(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "usb".into());
        m.insert("action".into(), "read".into());
        m.insert("len".into(), len.into());
        let resp = self.cmd(m)?;
        extract_bytes(resp)
    }

    // ── Config ────────────────────────────────────────────────────────────

    /// Retrieve the device's current Wi-Fi configuration.
    pub fn config_get(&mut self) -> Result<ConfigInfo> {
        let mut m = serde_json::Map::new();
        m.insert("interface".into(), "config".into());
        m.insert("action".into(), "get".into());
        let resp = self.cmd(m)?;
        let data = resp
            .data
            .ok_or_else(|| Error::BadResponse("missing data".into()))?;
        let info: ConfigInfo = serde_json::from_value(data)?;
        Ok(info)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn bytes_to_json(bytes: &[u8]) -> Value {
    Value::Array(bytes.iter().map(|&b| Value::Number(b.into())).collect())
}

fn extract_bytes(resp: RawResponse) -> Result<Vec<u8>> {
    let data = resp
        .data
        .ok_or_else(|| Error::BadResponse("missing data".into()))?;
    let arr = data
        .get("bytes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::BadResponse("missing data.bytes".into()))?;
    arr.iter()
        .map(|v| {
            v.as_u64()
                .map(|n| n as u8)
                .ok_or_else(|| Error::BadResponse("non-integer in bytes array".into()))
        })
        .collect()
}
