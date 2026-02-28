//! Message router — dispatches parsed commands to the appropriate interface handler.
//!
//! The router validates `interface` and `action` fields and returns structured error responses
//! for unknown values. Each interface handler is responsible for further validation of its
//! own required parameters.

use crate::interfaces::{i2c, pwm, spi, uart, usb};
use crate::protocol::{
    Command, ERROR_NOT_CONFIGURED, ERROR_UNKNOWN_ACTION, ERROR_UNKNOWN_INTERFACE, Response,
    ResponseData,
};
use core::fmt::Write as _;

/// Peripheral configuration state tracked across commands within a single TCP session.
///
/// Created at connection accept and dropped on disconnect. Configure actions update
/// the relevant fields; subsequent read/write actions check the `configured` flag.
pub struct DeviceState {
    pub uart: [uart::UartConfig; 2],
    pub spi: [spi::SpiConfig; 2],
    pub i2c: [i2c::I2cConfig; 2],
    pub usb_configured: bool,
    /// Wi-Fi SSID (populated by firmware, empty in host tests).
    pub config_ssid: heapless::String<32>,
    /// Device IP address (populated by firmware, empty in host tests).
    pub config_ip: heapless::String<16>,
    /// Whether the device is currently connected to Wi-Fi.
    pub config_connected: bool,
    /// Set by `system/reboot_to_bootloader`; checked by `net.rs` after the response is flushed.
    pub pending_reboot: bool,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            uart: [uart::UartConfig::default(), uart::UartConfig::default()],
            spi: [spi::SpiConfig::default(), spi::SpiConfig::default()],
            i2c: [i2c::I2cConfig::default(), i2c::I2cConfig::default()],
            usb_configured: false,
            config_ssid: heapless::String::new(),
            config_ip: heapless::String::new(),
            config_connected: false,
            pending_reboot: false,
        }
    }
}

/// Dispatch a command to the appropriate interface handler.
///
/// This function matches on `cmd.interface` and delegates to the corresponding module.
/// Returns `Err` with an error code if the interface or action is unknown.
///
/// In the full firmware, each branch calls into `crate::interfaces::*::handle(cmd, hw)`.
/// In this routing layer we only validate the interface/action strings.
pub fn validate_route<'a>(cmd: &Command<'a>) -> Result<(&'a str, &'a str), Response<'a>> {
    let interface = match cmd.interface {
        Some(i) => i,
        None => return Err(Response::error(cmd.id, ERROR_UNKNOWN_INTERFACE)),
    };
    let action = match cmd.action {
        Some(a) => a,
        None => return Err(Response::error(cmd.id, ERROR_UNKNOWN_ACTION)),
    };

    // Validate that the interface is one we know about
    let valid_action = match interface {
        "gpio" => matches!(action, "read" | "write" | "set_mode"),
        "uart" => matches!(action, "read" | "write" | "configure"),
        "spi" => matches!(action, "transfer" | "write" | "configure"),
        "i2c" => matches!(action, "read" | "write" | "write_read" | "configure"),
        "pwm" => matches!(action, "set_duty" | "set_freq" | "enable" | "disable"),
        "adc" => matches!(action, "read"),
        "usb" => matches!(action, "read" | "write"),
        "config" => matches!(action, "get"),
        "system" => matches!(action, "get_version" | "reboot_to_bootloader"),
        _ => return Err(Response::error(cmd.id, ERROR_UNKNOWN_INTERFACE)),
    };

    if !valid_action {
        return Err(Response::error(cmd.id, ERROR_UNKNOWN_ACTION));
    }

    Ok((interface, action))
}

/// Dispatch a validated command to its interface handler, updating peripheral state as needed.
///
/// `route` is the `(interface, action)` tuple returned by [`validate_route`].
/// Actions that require hardware peripherals (GPIO pins, ADC reads, UART/SPI/I2C RX data)
/// return [`ERROR_NOT_CONFIGURED`] — the firmware extends this with actual peripheral access.
pub fn dispatch<'a>(
    cmd: &Command<'a>,
    route: (&str, &str),
    state: &mut DeviceState,
) -> Response<'a> {
    match route {
        // ---- GPIO ----
        ("gpio", "set_mode") => crate::interfaces::gpio::handle_set_mode(cmd),
        ("gpio", "read") | ("gpio", "write") => {
            // Needs actual InputPin/OutputPin hardware — stub returns not_configured.
            Response::error(cmd.id, ERROR_NOT_CONFIGURED)
        }

        // ---- ADC ----
        ("adc", "read") => match crate::interfaces::adc::validate_read(cmd) {
            Ok(_channel) => Response::error(cmd.id, ERROR_NOT_CONFIGURED),
            Err(r) => r,
        },

        // ---- UART ----
        ("uart", "configure") => match uart::handle_configure(cmd) {
            Ok(cfg) => {
                let idx = cmd.uart.unwrap_or(0) as usize;
                state.uart[idx] = cfg;
                Response::ok(cmd.id, None)
            }
            Err(r) => r,
        },
        ("uart", "write") => {
            let idx = match uart::validate_uart(cmd) {
                Ok(i) => i as usize,
                Err(r) => return r,
            };
            uart::handle_write(cmd, state.uart[idx].configured)
        }
        ("uart", "read") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- SPI ----
        ("spi", "configure") => match spi::handle_configure(cmd) {
            Ok(cfg) => {
                let idx = cmd.spi.unwrap_or(0) as usize;
                state.spi[idx] = cfg;
                Response::ok(cmd.id, None)
            }
            Err(r) => r,
        },
        ("spi", "write") => {
            let idx = match spi::validate_spi(cmd) {
                Ok(i) => i as usize,
                Err(r) => return r,
            };
            spi::handle_write(cmd, state.spi[idx].configured)
        }
        ("spi", "transfer") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- I2C ----
        ("i2c", "configure") => match i2c::handle_configure(cmd) {
            Ok(cfg) => {
                let idx = cmd.i2c.unwrap_or(0) as usize;
                state.i2c[idx] = cfg;
                Response::ok(cmd.id, None)
            }
            Err(r) => r,
        },
        ("i2c", "write") => {
            let idx = match i2c::validate_i2c(cmd) {
                Ok(i) => i as usize,
                Err(r) => return r,
            };
            i2c::handle_write(cmd, state.i2c[idx].configured)
        }
        ("i2c", "read") | ("i2c", "write_read") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- PWM ----
        ("pwm", "set_duty") => pwm::handle_set_duty(cmd),
        ("pwm", "set_freq") => pwm::handle_set_freq(cmd),
        ("pwm", "enable") => pwm::handle_enable(cmd),
        ("pwm", "disable") => pwm::handle_disable(cmd),

        // ---- USB ----
        ("usb", "write") => usb::handle_write(cmd, state.usb_configured),
        ("usb", "read") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- Config ----
        ("config", "get") => Response::ok(
            cmd.id,
            Some(ResponseData::Config {
                ssid: state.config_ssid.clone(),
                ip: state.config_ip.clone(),
                connected: state.config_connected,
            }),
        ),

        // ---- System ----
        ("system", "get_version") => {
            let mut version: heapless::String<16> = heapless::String::new();
            let _ = version.write_str(env!("CARGO_PKG_VERSION"));
            Response::ok(cmd.id, Some(ResponseData::Version { version }))
        }
        ("system", "reboot_to_bootloader") => {
            state.pending_reboot = true;
            Response::ok(cmd.id, None)
        }

        // validate_route already rejected invalid routes
        _ => unreachable!(),
    }
}
