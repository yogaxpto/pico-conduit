use crate::fixtures::make_cmd;
use pico_conduit::protocol::{ERROR_UNKNOWN_ACTION, ERROR_UNKNOWN_INTERFACE, ResponseData};
use pico_conduit::router::{DeviceState, dispatch, validate_route};

// ── validate_route ────────────────────────────────────────────────────────────

#[test]
fn system_get_version_is_valid_route() {
    let cmd = make_cmd("1", Some("system"), Some("get_version"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn system_reboot_to_bootloader_is_valid_route() {
    let cmd = make_cmd("2", Some("system"), Some("reboot_to_bootloader"));
    assert!(validate_route(&cmd).is_ok());
}

#[test]
fn system_unknown_action_returns_error() {
    let cmd = make_cmd("3", Some("system"), Some("shutdown"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_ACTION));
}

#[test]
fn system_unknown_interface_still_errors() {
    let cmd = make_cmd("4", Some("notasystem"), Some("get_version"));
    let err = validate_route(&cmd).unwrap_err();
    assert_eq!(err.error, Some(ERROR_UNKNOWN_INTERFACE));
}

// ── dispatch ──────────────────────────────────────────────────────────────────

#[test]
fn get_version_returns_ok_with_version_string() {
    let cmd = make_cmd("5", Some("system"), Some("get_version"));
    let route = validate_route(&cmd).unwrap();
    let mut state = DeviceState::default();
    let resp = dispatch(&cmd, route, &mut state);
    assert!(resp.ok);
    assert!(resp.error.is_none());
    match resp.data {
        Some(ResponseData::Version { ref version }) => {
            assert!(!version.is_empty(), "version string must not be empty");
            // Version comes from CARGO_PKG_VERSION; format is x.y.z
            assert!(
                version.contains('.'),
                "version should be semver-like, got: {version}"
            );
        }
        other => panic!("expected Version data, got: {other:?}"),
    }
}

#[test]
fn get_version_does_not_set_pending_reboot() {
    let cmd = make_cmd("6", Some("system"), Some("get_version"));
    let route = validate_route(&cmd).unwrap();
    let mut state = DeviceState::default();
    dispatch(&cmd, route, &mut state);
    assert!(
        !state.pending_reboot,
        "get_version must not set pending_reboot"
    );
}

#[test]
fn reboot_to_bootloader_returns_ok_no_data() {
    let cmd = make_cmd("7", Some("system"), Some("reboot_to_bootloader"));
    let route = validate_route(&cmd).unwrap();
    let mut state = DeviceState::default();
    let resp = dispatch(&cmd, route, &mut state);
    assert!(resp.ok);
    assert!(resp.error.is_none());
    assert!(resp.data.is_none(), "reboot_to_bootloader returns no data");
}

#[test]
fn reboot_to_bootloader_sets_pending_reboot() {
    let cmd = make_cmd("8", Some("system"), Some("reboot_to_bootloader"));
    let route = validate_route(&cmd).unwrap();
    let mut state = DeviceState::default();
    assert!(!state.pending_reboot);
    dispatch(&cmd, route, &mut state);
    assert!(
        state.pending_reboot,
        "pending_reboot must be true after reboot_to_bootloader"
    );
}

#[test]
fn pending_reboot_starts_false() {
    let state = DeviceState::default();
    assert!(!state.pending_reboot);
}
