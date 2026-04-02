use pico_conduit::mqtt::{backoff_secs, client_id, cmd_topic, resp_topic};
use pico_conduit::provisioning::portal::make_ap_ssid;

// ----- Topic string construction -----

#[test]
fn mqtt_cmd_topic_format() {
    let topic = cmd_topic([0xAA, 0xBB, 0xCC, 0xDD, 0xA3, 0xF2]);
    assert_eq!(topic.as_str(), "pico/a3f2/cmd");
}

#[test]
fn mqtt_resp_topic_format() {
    let topic = resp_topic([0xAA, 0xBB, 0xCC, 0xDD, 0xA3, 0xF2]);
    assert_eq!(topic.as_str(), "pico/a3f2/resp");
}

#[test]
fn mqtt_topic_all_zeros() {
    assert_eq!(cmd_topic([0; 6]).as_str(), "pico/0000/cmd");
    assert_eq!(resp_topic([0; 6]).as_str(), "pico/0000/resp");
}

#[test]
fn mqtt_topic_all_ff() {
    assert_eq!(cmd_topic([0xFF; 6]).as_str(), "pico/ffff/cmd");
}

#[test]
fn mqtt_topic_fits_heapless_string() {
    // "pico/XXXX/resp" = 14 chars — well within heapless::String<32>
    let topic = resp_topic([0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]);
    assert!(topic.len() <= 32);
    assert_eq!(topic.len(), 14);
}

// ----- Client ID construction -----

#[test]
fn mqtt_client_id_format() {
    let id = client_id([0xAA, 0xBB, 0xCC, 0xDD, 0xA3, 0xF2]);
    assert_eq!(id.as_str(), "pico-a3f2");
}

#[test]
fn mqtt_client_id_matches_ap_ssid_suffix() {
    let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xA3, 0xF2];
    let id = client_id(mac);
    let ssid = make_ap_ssid(mac);
    // AP SSID is "pico-setup-A3F2", client ID is "pico-a3f2"
    // Last 4 chars of the SSID should match last 4 chars of client ID (case-insensitive)
    let ssid_suffix = &ssid.as_str()[ssid.len() - 4..];
    let id_suffix = &id.as_str()[id.len() - 4..];
    assert_eq!(
        ssid_suffix.to_ascii_lowercase(),
        id_suffix.to_ascii_lowercase(),
        "client ID suffix should match AP SSID suffix"
    );
}

// ----- Backoff sequence -----

#[test]
fn mqtt_backoff_sequence() {
    assert_eq!(backoff_secs(0), 5);
    assert_eq!(backoff_secs(1), 10);
    assert_eq!(backoff_secs(2), 20);
    assert_eq!(backoff_secs(3), 40);
    assert_eq!(backoff_secs(4), 60);
    assert_eq!(backoff_secs(5), 60);
    assert_eq!(backoff_secs(255), 60);
}

#[test]
fn mqtt_backoff_resets_on_success() {
    // After several failures, backoff should have grown
    assert_eq!(backoff_secs(4), 60);
    // On success, caller resets attempt to 0 — verify initial delay
    assert_eq!(backoff_secs(0), 5);
}
