use pico_socketeer::provisioning::storage::{
    Credentials, erase_credentials, load_credentials, save_credentials,
};

#[test]
fn load_credentials_returns_none_on_blank_flash() {
    // Stub always returns None — simulates blank flash (all 0xFF)
    assert_eq!(load_credentials(), None);
}

#[test]
fn save_credentials_stub_returns_ok() {
    let creds = Credentials::new("TestSSID", "TestPass").unwrap();
    assert_eq!(save_credentials(&creds), Ok(()));
}

#[test]
fn credentials_new_valid() {
    let creds = Credentials::new("MyNetwork", "hunter2").unwrap();
    assert_eq!(creds.ssid.as_str(), "MyNetwork");
    assert_eq!(creds.password.as_str(), "hunter2");
}

#[test]
fn credentials_new_ssid_too_long() {
    // SSID > 32 chars should fail
    let long_ssid = "A".repeat(33);
    assert!(Credentials::new(&long_ssid, "pass").is_none());
}

#[test]
fn credentials_new_password_too_long() {
    // Password > 64 chars should fail
    let long_pass = "P".repeat(65);
    assert!(Credentials::new("ssid", &long_pass).is_none());
}

/// Stub round-trip: save then load should return None (stub doesn't persist).
/// Full implementation would return the saved credentials.
#[test]
fn save_then_load_stub_returns_none() {
    let creds = Credentials::new("Net", "Pass").unwrap();
    save_credentials(&creds).unwrap();
    // Stub: load still returns None (nothing persisted)
    assert_eq!(load_credentials(), None);
}

#[test]
fn erase_credentials_stub_returns_ok() {
    assert_eq!(erase_credentials(), Ok(()));
}

// ----- Credentials boundary length edge cases -----

#[test]
fn credentials_ssid_exactly_32_chars() {
    let ssid = "A".repeat(32);
    let creds = Credentials::new(&ssid, "pass").unwrap();
    assert_eq!(creds.ssid.len(), 32);
}

#[test]
fn credentials_password_exactly_64_chars() {
    let password = "P".repeat(64);
    let creds = Credentials::new("ssid", &password).unwrap();
    assert_eq!(creds.password.len(), 64);
}

#[test]
fn credentials_empty_ssid_succeeds() {
    // heapless::String allows empty — no minimum length enforced
    let creds = Credentials::new("", "pass").unwrap();
    assert_eq!(creds.ssid.as_str(), "");
}

#[test]
fn credentials_empty_password_succeeds() {
    // Open networks have no password
    let creds = Credentials::new("OpenNet", "").unwrap();
    assert_eq!(creds.password.as_str(), "");
}

// ----- MQTT credential fields -----

#[test]
fn credentials_new_defaults_mqtt_fields() {
    let creds = Credentials::new("ssid", "pass").unwrap();
    assert_eq!(creds.mqtt_host.as_str(), "");
    assert_eq!(creds.mqtt_port, 1883);
}

#[test]
fn credentials_mqtt_host_exactly_64_chars() {
    let host = "H".repeat(64);
    let creds = Credentials::with_mqtt("ssid", "pass", &host, 1883).unwrap();
    assert_eq!(creds.mqtt_host.len(), 64);
}

#[test]
fn credentials_mqtt_host_65_chars_rejected() {
    let host = "H".repeat(65);
    assert!(Credentials::with_mqtt("ssid", "pass", &host, 1883).is_none());
}

#[test]
fn credentials_mqtt_host_empty_is_valid() {
    let creds = Credentials::with_mqtt("ssid", "pass", "", 1883).unwrap();
    assert_eq!(creds.mqtt_host.as_str(), "");
}

#[test]
fn credentials_mqtt_port_default_1883() {
    let creds = Credentials::new("ssid", "pass").unwrap();
    assert_eq!(creds.mqtt_port, 1883);
}

#[test]
fn credentials_mqtt_port_custom() {
    let creds = Credentials::with_mqtt("ssid", "pass", "broker.local", 8883).unwrap();
    assert_eq!(creds.mqtt_port, 8883);
    assert_eq!(creds.mqtt_host.as_str(), "broker.local");
}
