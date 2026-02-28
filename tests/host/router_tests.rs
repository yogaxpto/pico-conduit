use pico_socketeer::protocol::{Command, ERROR_UNKNOWN_ACTION, ERROR_UNKNOWN_INTERFACE};
use pico_socketeer::router::validate_route;

fn make_cmd<'a>(id: &'a str, interface: Option<&'a str>, action: Option<&'a str>) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface,
        action,
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
fn gpio_read_is_valid_route() {
    let cmd = make_cmd("1", Some("gpio"), Some("read"));
    let result = validate_route(&cmd);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn gpio_write_is_valid_route() {
    let cmd = make_cmd("2", Some("gpio"), Some("write"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn uart_configure_is_valid_route() {
    let cmd = make_cmd("3", Some("uart"), Some("configure"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn spi_transfer_is_valid_route() {
    let cmd = make_cmd("4", Some("spi"), Some("transfer"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn i2c_write_read_is_valid_route() {
    let cmd = make_cmd("5", Some("i2c"), Some("write_read"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn pwm_set_duty_is_valid_route() {
    let cmd = make_cmd("6", Some("pwm"), Some("set_duty"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn adc_read_is_valid_route() {
    let cmd = make_cmd("7", Some("adc"), Some("read"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn usb_write_is_valid_route() {
    let cmd = make_cmd("8", Some("usb"), Some("write"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn config_get_is_valid_route() {
    let cmd = make_cmd("9", Some("config"), Some("get"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn unknown_interface_returns_error() {
    let cmd = make_cmd("10", Some("radio"), Some("send"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_INTERFACE));
}

#[test]
fn unknown_action_returns_error() {
    let cmd = make_cmd("11", Some("gpio"), Some("teleport"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_ACTION));
}

#[test]
fn missing_interface_returns_error() {
    let cmd = make_cmd("12", None, Some("read"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_INTERFACE));
}

#[test]
fn missing_action_returns_error() {
    let cmd = make_cmd("13", Some("gpio"), None);
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_ACTION));
}

#[test]
fn all_valid_interfaces_accepted() {
    let routes = [
        ("gpio", "read"),
        ("gpio", "write"),
        ("gpio", "set_mode"),
        ("uart", "read"),
        ("uart", "write"),
        ("uart", "configure"),
        ("spi", "transfer"),
        ("spi", "write"),
        ("spi", "configure"),
        ("i2c", "read"),
        ("i2c", "write"),
        ("i2c", "write_read"),
        ("i2c", "configure"),
        ("pwm", "set_duty"),
        ("pwm", "set_freq"),
        ("pwm", "enable"),
        ("pwm", "disable"),
        ("adc", "read"),
        ("usb", "read"),
        ("usb", "write"),
        ("config", "get"),
    ];
    for (iface, action) in routes {
        let cmd = make_cmd("x", Some(iface), Some(action));
        assert!(
            validate_route(&cmd).is_ok(),
            "route {iface}/{action} should be valid"
        );
    }
}

// ----- Router edge cases -----

#[test]
fn uppercase_interface_returns_unknown_interface() {
    // Router is case-sensitive — "GPIO" is not "gpio"
    let cmd = make_cmd("e1", Some("GPIO"), Some("read"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_INTERFACE));
}

#[test]
fn empty_interface_returns_unknown_interface() {
    let cmd = make_cmd("e2", Some(""), Some("read"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_INTERFACE));
}

#[test]
fn both_none_returns_unknown_interface() {
    // Interface is checked first, so both None gives unknown_interface
    let cmd = make_cmd("e3", None, None);
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_INTERFACE));
}
