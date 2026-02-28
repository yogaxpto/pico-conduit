//! PWM interface handler.
//!
//! Controls PWM slices on the RP2350. The `channel` field in commands refers to a
//! PWM channel number (0–15 on RP2350, which has 8 slices × 2 channels each).

use crate::protocol::{
    Command, Response, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE,
};

/// Maximum PWM channel index on RP2350 (8 slices × 2 channels = 16 total, indexed 0–15).
pub const MAX_PWM_CHANNEL: u8 = 15;

/// Validate the PWM channel number from a command.
pub fn validate_channel<'a>(cmd: &Command<'a>) -> Result<u8, Response<'a>> {
    let ch = match cmd.channel {
        Some(c) => c,
        None => return Err(Response::error(cmd.id, ERROR_MISSING_FIELD)),
    };
    if ch > MAX_PWM_CHANNEL {
        return Err(Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE));
    }
    Ok(ch)
}

/// Handle a `pwm set_duty` command.
///
/// Sets the raw 16-bit duty cycle: 0 = always off, 65535 = always on.
pub fn handle_set_duty<'a>(cmd: &Command<'a>) -> Response<'a> {
    let _ch = match validate_channel(cmd) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let _duty = match cmd.duty_u16 {
        Some(d) => d,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    // In the real firmware: configure the PWM slice duty cycle via embassy-rp Pwm peripheral
    Response::ok(cmd.id, None)
}

/// Handle a `pwm set_freq` command.
///
/// Sets the PWM frequency in Hz for the channel's slice.
pub fn handle_set_freq<'a>(cmd: &Command<'a>) -> Response<'a> {
    let _ch = match validate_channel(cmd) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let freq = match cmd.freq_hz {
        Some(f) => f,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    if freq == 0 {
        return Response::error(cmd.id, ERROR_VALUE_OUT_OF_RANGE);
    }
    // In the real firmware: compute divider and configure the PWM slice
    Response::ok(cmd.id, None)
}

/// Handle a `pwm enable` command.
pub fn handle_enable<'a>(cmd: &Command<'a>) -> Response<'a> {
    let _ch = match validate_channel(cmd) {
        Ok(c) => c,
        Err(r) => return r,
    };
    // In the real firmware: enable the PWM slice
    Response::ok(cmd.id, None)
}

/// Handle a `pwm disable` command.
pub fn handle_disable<'a>(cmd: &Command<'a>) -> Response<'a> {
    let _ch = match validate_channel(cmd) {
        Ok(c) => c,
        Err(r) => return r,
    };
    // In the real firmware: disable the PWM slice
    Response::ok(cmd.id, None)
}
