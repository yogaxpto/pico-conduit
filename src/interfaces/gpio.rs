//! GPIO interface handler.
//!
//! Supports `read`, `write`, and `set_mode` actions.
//! Rejects pin 29 (CYW43 SPI DIO) and other reserved pins.

use crate::interfaces::is_pin_available;
use crate::protocol::{
    Command, ERROR_INVALID_PIN, ERROR_MISSING_FIELD, ERROR_UNKNOWN_ACTION,
    ERROR_VALUE_OUT_OF_RANGE, Response, ResponseData,
};
use embedded_hal::digital::{InputPin, OutputPin};

/// Validate the GPIO pin number from a command, rejecting reserved pins.
fn validate_pin<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    let Some(pin_num) = cmd.pin else {
        return Err(Response::error(cmd.id, ERROR_MISSING_FIELD));
    };
    if !is_pin_available(pin_num) {
        return Err(Response::error(cmd.id, ERROR_INVALID_PIN));
    }
    Ok(pin_num)
}

/// Handle a GPIO read command using the provided input pin.
pub fn handle_read<'a, P: InputPin>(pin: &mut P, cmd: &Command<'a>) -> Response<'a> {
    let _pin_num = match validate_pin(cmd) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Ok(high) = pin.is_high() else {
        return Response::error(cmd.id, crate::protocol::ERROR_PERIPHERAL_ERROR);
    };
    Response::ok(
        cmd.id,
        Some(ResponseData::GpioRead {
            value: u8::from(high),
        }),
    )
}

/// Handle a GPIO write command using the provided output pin.
pub fn handle_write<'a, P: OutputPin>(pin: &mut P, cmd: &Command<'a>) -> Response<'a> {
    let _pin_num = match validate_pin(cmd) {
        Ok(p) => p,
        Err(r) => return r,
    };
    match cmd.value {
        Some(0) => {
            if pin.set_low().is_err() {
                return Response::error(cmd.id, crate::protocol::ERROR_PERIPHERAL_ERROR);
            }
        }
        Some(1) => {
            if pin.set_high().is_err() {
                return Response::error(cmd.id, crate::protocol::ERROR_PERIPHERAL_ERROR);
            }
        }
        Some(_) => return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE),
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    }
    Response::ok(cmd.id, None)
}

/// Handle a GPIO set_mode command (mode/pull configuration).
pub fn handle_set_mode<'a>(cmd: &Command<'a>) -> Response<'a> {
    let _pin_num = match validate_pin(cmd) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let _mode = match cmd.mode {
        Some(m) if m == "input" || m == "output" => m,
        Some(_) => return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE),
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    let _pull = match cmd.pull {
        Some(p) if matches!(p, "up" | "down" | "none") => p,
        Some(_) => return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE),
        None => "none", // pull is optional for set_mode
    };
    // In the actual firmware, this configures the RP2350 PAC registers.
    // For the stub + host tests, we return success.
    Response::ok(cmd.id, None)
}

/// Dispatch a GPIO command to the appropriate handler.
pub const fn handle_unknown_action<'a>(cmd: &Command<'a>) -> Response<'a> {
    Response::error(cmd.id, ERROR_UNKNOWN_ACTION)
}
