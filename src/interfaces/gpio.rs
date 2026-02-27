//! GPIO interface handler.
//!
//! Supports `read`, `write`, and `set_mode` actions.
//! Rejects pin 29 (CYW43 SPI DIO) and other reserved pins.

use crate::interfaces::is_pin_available;
use crate::protocol::{
    Command, Response, ResponseData, ERROR_INVALID_PIN, ERROR_MISSING_FIELD,
    ERROR_UNKNOWN_ACTION, ERROR_VALUE_OUT_OF_RANGE,
};
use embedded_hal::digital::{InputPin, OutputPin};

/// Handle a GPIO read command using the provided input pin.
pub fn handle_read<'a, P: InputPin>(pin: &mut P, cmd: &Command<'a>) -> Response<'a> {
    let pin_num = match cmd.pin {
        Some(p) => p,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    if !is_pin_available(pin_num) {
        return Response::error(cmd.id, ERROR_INVALID_PIN);
    }
    let high = match pin.is_high() {
        Ok(v) => v,
        Err(_) => return Response::error(cmd.id, crate::protocol::ERROR_PERIPHERAL_ERROR),
    };
    Response::ok(cmd.id, Some(ResponseData::GpioRead { value: if high { 1 } else { 0 } }))
}

/// Handle a GPIO write command using the provided output pin.
pub fn handle_write<'a, P: OutputPin>(pin: &mut P, cmd: &Command<'a>) -> Response<'a> {
    let pin_num = match cmd.pin {
        Some(p) => p,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    if !is_pin_available(pin_num) {
        return Response::error(cmd.id, ERROR_INVALID_PIN);
    }
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
    let pin_num = match cmd.pin {
        Some(p) => p,
        None => return Response::error(cmd.id, ERROR_MISSING_FIELD),
    };
    if !is_pin_available(pin_num) {
        return Response::error(cmd.id, ERROR_INVALID_PIN);
    }
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
pub fn handle_unknown_action<'a>(cmd: &Command<'a>) -> Response<'a> {
    Response::error(cmd.id, ERROR_UNKNOWN_ACTION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Command, ERROR_INVALID_PIN, ERROR_MISSING_FIELD, ERROR_VALUE_OUT_OF_RANGE};
    use embedded_hal_mock::eh1::digital::{Mock as PinMock, State, Transaction};

    fn make_cmd_write<'a>(id: &'a str, pin: Option<u8>, value: Option<u8>) -> Command<'a> {
        Command {
            version: Some(1),
            id,
            interface: Some("gpio"),
            action: Some("write"),
            pin,
            value,
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

    fn make_cmd_read<'a>(id: &'a str, pin: Option<u8>) -> Command<'a> {
        let mut cmd = make_cmd_write(id, pin, None);
        cmd.action = Some("read");
        cmd
    }

    fn make_cmd_set_mode<'a>(
        id: &'a str,
        pin: Option<u8>,
        mode: Option<&'a str>,
        pull: Option<&'a str>,
    ) -> Command<'a> {
        let mut cmd = make_cmd_write(id, pin, None);
        cmd.action = Some("set_mode");
        cmd.mode = mode;
        cmd.pull = pull;
        cmd
    }

    // ----- handle_write tests -----

    #[test]
    fn gpio_write_high_makes_correct_hal_calls() {
        let transactions = [Transaction::set(State::High)];
        let mut pin = PinMock::new(&transactions);
        let cmd = make_cmd_write("w1", Some(15), Some(1));
        let resp = handle_write(&mut pin, &cmd);
        assert!(resp.ok, "write high should succeed");
        pin.done();
    }

    #[test]
    fn gpio_write_low_makes_correct_hal_calls() {
        let transactions = [Transaction::set(State::Low)];
        let mut pin = PinMock::new(&transactions);
        let cmd = make_cmd_write("w2", Some(0), Some(0));
        let resp = handle_write(&mut pin, &cmd);
        assert!(resp.ok, "write low should succeed");
        pin.done();
    }

    #[test]
    fn gpio_write_missing_pin_returns_missing_field() {
        let mut pin = PinMock::new(&[]);
        let cmd = make_cmd_write("w3", None, Some(1));
        let resp = handle_write(&mut pin, &cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
        pin.done();
    }

    #[test]
    fn gpio_write_missing_value_returns_missing_field() {
        let mut pin = PinMock::new(&[]);
        let cmd = make_cmd_write("w4", Some(5), None);
        let resp = handle_write(&mut pin, &cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
        pin.done();
    }

    #[test]
    fn gpio_write_value_out_of_range() {
        let mut pin = PinMock::new(&[]);
        let cmd = make_cmd_write("w5", Some(5), Some(2));
        let resp = handle_write(&mut pin, &cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
        pin.done();
    }

    #[test]
    fn gpio_write_reserved_pin_returns_invalid_pin() {
        let mut pin = PinMock::new(&[]);
        let cmd = make_cmd_write("w6", Some(29), Some(1)); // GPIO29 is reserved
        let resp = handle_write(&mut pin, &cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_INVALID_PIN));
        pin.done();
    }

    // ----- handle_read tests -----

    #[test]
    fn gpio_read_high_returns_value_one() {
        let transactions = [Transaction::get(State::High)];
        let mut pin = PinMock::new(&transactions);
        let cmd = make_cmd_read("r1", Some(10));
        let resp = handle_read(&mut pin, &cmd);
        assert!(resp.ok);
        assert_eq!(resp.data, Some(ResponseData::GpioRead { value: 1 }));
        pin.done();
    }

    #[test]
    fn gpio_read_low_returns_value_zero() {
        let transactions = [Transaction::get(State::Low)];
        let mut pin = PinMock::new(&transactions);
        let cmd = make_cmd_read("r2", Some(10));
        let resp = handle_read(&mut pin, &cmd);
        assert!(resp.ok);
        assert_eq!(resp.data, Some(ResponseData::GpioRead { value: 0 }));
        pin.done();
    }

    #[test]
    fn gpio_read_missing_pin_returns_missing_field() {
        let mut pin = PinMock::new(&[]);
        let cmd = make_cmd_read("r3", None);
        let resp = handle_read(&mut pin, &cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
        pin.done();
    }

    #[test]
    fn gpio_read_reserved_pin_returns_invalid_pin() {
        let mut pin = PinMock::new(&[]);
        let cmd = make_cmd_read("r4", Some(29)); // reserved
        let resp = handle_read(&mut pin, &cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_INVALID_PIN));
        pin.done();
    }

    // ----- handle_set_mode tests -----

    #[test]
    fn gpio_set_mode_output_pull_up() {
        let cmd = make_cmd_set_mode("m1", Some(5), Some("output"), Some("up"));
        let resp = handle_set_mode(&cmd);
        assert!(resp.ok, "set_mode output/up should succeed");
    }

    #[test]
    fn gpio_set_mode_input_pull_down() {
        let cmd = make_cmd_set_mode("m2", Some(3), Some("input"), Some("down"));
        let resp = handle_set_mode(&cmd);
        assert!(resp.ok, "set_mode input/down should succeed");
    }

    #[test]
    fn gpio_set_mode_missing_pin_returns_error() {
        let cmd = make_cmd_set_mode("m3", None, Some("output"), None);
        let resp = handle_set_mode(&cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn gpio_set_mode_missing_mode_returns_error() {
        let cmd = make_cmd_set_mode("m4", Some(5), None, None);
        let resp = handle_set_mode(&cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
    }

    #[test]
    fn gpio_set_mode_invalid_mode_returns_error() {
        let cmd = make_cmd_set_mode("m5", Some(5), Some("tristate"), None);
        let resp = handle_set_mode(&cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
    }

    #[test]
    fn gpio_set_mode_reserved_pin_returns_invalid_pin() {
        let cmd = make_cmd_set_mode("m6", Some(29), Some("output"), None);
        let resp = handle_set_mode(&cmd);
        assert!(!resp.ok);
        assert_eq!(resp.error, Some(ERROR_INVALID_PIN));
    }

    // ----- is_pin_available -----

    #[test]
    fn pin_0_is_available() {
        assert!(is_pin_available(0));
    }

    #[test]
    fn pin_22_is_available() {
        assert!(is_pin_available(22));
    }

    #[test]
    fn pin_29_is_reserved() {
        assert!(!is_pin_available(29));
    }

    #[test]
    fn pin_23_is_reserved() {
        assert!(!is_pin_available(23));
    }
}
