use crate::fixtures::make_cmd;
use pico_conduit::protocol::{
    AdcChannel, ERROR_INVALID_PIN, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_UNKNOWN_ACTION,
    ERROR_UNKNOWN_INTERFACE, ERROR_VALUE_OUT_OF_RANGE,
};
use pico_conduit::router::{DeviceState, dispatch, validate_route};

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

// ----- Dispatch tests -----

#[test]
fn dispatch_gpio_set_mode_valid() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d1", Some("gpio"), Some("set_mode"));
    cmd.pin = Some(15);
    cmd.mode = Some("output");
    cmd.pull = Some("none");
    let resp = dispatch(&cmd, ("gpio", "set_mode"), &mut state);
    assert!(resp.ok);
}

#[test]
fn dispatch_gpio_set_mode_invalid_pin() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d2", Some("gpio"), Some("set_mode"));
    cmd.pin = Some(29); // reserved
    cmd.mode = Some("output");
    let resp = dispatch(&cmd, ("gpio", "set_mode"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_INVALID_PIN));
}

#[test]
fn dispatch_gpio_read_not_configured() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d3", Some("gpio"), Some("read"));
    cmd.pin = Some(15);
    let resp = dispatch(&cmd, ("gpio", "read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_gpio_write_not_configured() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d4", Some("gpio"), Some("write"));
    cmd.pin = Some(15);
    cmd.value = Some(1);
    let resp = dispatch(&cmd, ("gpio", "write"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_uart_configure_then_write() {
    let mut state = DeviceState::default();

    // Configure UART 0
    let mut cmd = make_cmd("d5a", Some("uart"), Some("configure"));
    cmd.uart = Some(0);
    cmd.baud = Some(115200);
    let resp = dispatch(&cmd, ("uart", "configure"), &mut state);
    assert!(resp.ok, "configure should succeed");
    assert!(state.uart[0].configured);

    // Write should now succeed
    let mut cmd = make_cmd("d5b", Some("uart"), Some("write"));
    cmd.uart = Some(0);
    cmd.bytes = Some("AQI="); // base64 for [0x01, 0x02]
    let resp = dispatch(&cmd, ("uart", "write"), &mut state);
    assert!(resp.ok, "write after configure should succeed");
}

#[test]
fn dispatch_uart_write_before_configure() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d6", Some("uart"), Some("write"));
    cmd.uart = Some(0);
    cmd.bytes = Some("AQ=="); // base64 for [0x01]
    let resp = dispatch(&cmd, ("uart", "write"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_uart_read_not_configured() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("d6r", Some("uart"), Some("read"));
    let resp = dispatch(&cmd, ("uart", "read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_spi_configure_then_write() {
    let mut state = DeviceState::default();

    // Configure SPI 0
    let mut cmd = make_cmd("d7a", Some("spi"), Some("configure"));
    cmd.spi = Some(0);
    cmd.freq_hz = Some(1_000_000);
    let resp = dispatch(&cmd, ("spi", "configure"), &mut state);
    assert!(resp.ok);
    assert!(state.spi[0].configured);

    // Write should succeed
    let mut cmd = make_cmd("d7b", Some("spi"), Some("write"));
    cmd.spi = Some(0);
    cmd.bytes = Some("/w=="); // base64 for [0xFF]
    let resp = dispatch(&cmd, ("spi", "write"), &mut state);
    assert!(resp.ok);
}

#[test]
fn dispatch_spi_transfer_not_configured() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("d7t", Some("spi"), Some("transfer"));
    let resp = dispatch(&cmd, ("spi", "transfer"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_i2c_configure_then_write() {
    let mut state = DeviceState::default();

    // Configure I2C 0
    let mut cmd = make_cmd("d8a", Some("i2c"), Some("configure"));
    cmd.i2c = Some(0);
    cmd.freq_hz = Some(100_000);
    let resp = dispatch(&cmd, ("i2c", "configure"), &mut state);
    assert!(resp.ok);
    assert!(state.i2c[0].configured);

    // Write should succeed
    let mut cmd = make_cmd("d8b", Some("i2c"), Some("write"));
    cmd.i2c = Some(0);
    cmd.addr = Some(0x50);
    cmd.bytes = Some("AAE="); // base64 for [0x00, 0x01]
    let resp = dispatch(&cmd, ("i2c", "write"), &mut state);
    assert!(resp.ok);
}

#[test]
fn dispatch_i2c_read_not_configured() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("d8r", Some("i2c"), Some("read"));
    let resp = dispatch(&cmd, ("i2c", "read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_i2c_write_read_not_configured() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("d8wr", Some("i2c"), Some("write_read"));
    let resp = dispatch(&cmd, ("i2c", "write_read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_pwm_set_duty_valid() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d9", Some("pwm"), Some("set_duty"));
    cmd.channel = Some(0);
    cmd.duty_u16 = Some(32768);
    let resp = dispatch(&cmd, ("pwm", "set_duty"), &mut state);
    assert!(resp.ok);
}

#[test]
fn dispatch_pwm_set_freq_valid() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d10", Some("pwm"), Some("set_freq"));
    cmd.channel = Some(0);
    cmd.freq_hz = Some(1000);
    let resp = dispatch(&cmd, ("pwm", "set_freq"), &mut state);
    assert!(resp.ok);
}

#[test]
fn dispatch_pwm_enable_disable() {
    let mut state = DeviceState::default();

    let mut cmd = make_cmd("d11a", Some("pwm"), Some("enable"));
    cmd.channel = Some(5);
    let resp = dispatch(&cmd, ("pwm", "enable"), &mut state);
    assert!(resp.ok);

    let mut cmd = make_cmd("d11b", Some("pwm"), Some("disable"));
    cmd.channel = Some(5);
    let resp = dispatch(&cmd, ("pwm", "disable"), &mut state);
    assert!(resp.ok);
}

#[test]
fn dispatch_adc_read_not_configured() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d12", Some("adc"), Some("read"));
    cmd.adc_channel = Some(AdcChannel::Ch0);
    let resp = dispatch(&cmd, ("adc", "read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_adc_read_missing_channel() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("d13", Some("adc"), Some("read"));
    let resp = dispatch(&cmd, ("adc", "read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
fn dispatch_usb_write_not_configured() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d14", Some("usb"), Some("write"));
    cmd.bytes = Some("QQ=="); // base64 for [0x41]
    let resp = dispatch(&cmd, ("usb", "write"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_usb_read_not_configured() {
    let mut state = DeviceState::default();
    let cmd = make_cmd("d14r", Some("usb"), Some("read"));
    let resp = dispatch(&cmd, ("usb", "read"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_NOT_CONFIGURED));
}

#[test]
fn dispatch_config_get() {
    let mut state = DeviceState::default();
    let _ = state.config_ssid.push_str("TestNet");
    let _ = state.config_ip.push_str("10.0.0.1");
    state.config_connected = true;
    let cmd = make_cmd("d15", Some("config"), Some("get"));
    let resp = dispatch(&cmd, ("config", "get"), &mut state);
    assert!(resp.ok);
    assert!(resp.data.is_some());
}

#[test]
fn dispatch_uart_configure_peripheral_1() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d16", Some("uart"), Some("configure"));
    cmd.uart = Some(1);
    cmd.baud = Some(9600);
    let resp = dispatch(&cmd, ("uart", "configure"), &mut state);
    assert!(resp.ok);
    assert!(state.uart[1].configured);
    assert!(!state.uart[0].configured);
}

#[test]
fn dispatch_pwm_set_duty_missing_channel() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d17", Some("pwm"), Some("set_duty"));
    cmd.duty_u16 = Some(100);
    // channel is None
    let resp = dispatch(&cmd, ("pwm", "set_duty"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_MISSING_FIELD));
}

#[test]
#[cfg(not(feature = "transport-mqtt"))]
fn config_get_excludes_mqtt_fields_without_feature() {
    // Without transport-mqtt feature, config/get response should NOT contain mqtt_host
    let mut state = DeviceState::default();
    let _ = state.config_ssid.push_str("TestNet");
    let cmd = make_cmd("mqtt1", Some("config"), Some("get"));
    let resp = dispatch(&cmd, ("config", "get"), &mut state);
    assert!(resp.ok);
    let mut buf = [0u8; 512];
    let n = pico_conduit::protocol::serialize_response(&resp, &mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..n]).unwrap();
    assert!(
        !s.contains("mqtt_host"),
        "config/get should not include mqtt_host without transport-mqtt feature: {s}"
    );
}

#[test]
fn dispatch_pwm_set_freq_zero() {
    let mut state = DeviceState::default();
    let mut cmd = make_cmd("d18", Some("pwm"), Some("set_freq"));
    cmd.channel = Some(0);
    cmd.freq_hz = Some(0);
    let resp = dispatch(&cmd, ("pwm", "set_freq"), &mut state);
    assert!(!resp.ok);
    assert_eq!(resp.error, Some(ERROR_VALUE_OUT_OF_RANGE));
}
