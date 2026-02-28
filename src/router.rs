//! Message router — dispatches parsed commands to the appropriate interface handler.
//!
//! The router validates `interface` and `action` fields and returns structured error responses
//! for unknown values. Each interface handler is responsible for further validation of its
//! own required parameters.

use crate::protocol::{Command, ERROR_UNKNOWN_ACTION, ERROR_UNKNOWN_INTERFACE, Response};

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
        _ => return Err(Response::error(cmd.id, ERROR_UNKNOWN_INTERFACE)),
    };

    if !valid_action {
        return Err(Response::error(cmd.id, ERROR_UNKNOWN_ACTION));
    }

    Ok((interface, action))
}
