//! Wi-Fi credential storage in flash.
//!
//! Phase 6a stub: `load_credentials()` always returns `None` and `save_credentials()`
//! is a no-op. The full implementation uses `sequential-storage` for wear-levelled
//! key-value storage in the `CREDENTIALS` flash region defined in `memory.x`.
//!
//! # Compile-time override
//!
//! If `PICO_WIFI_SSID` and `PICO_WIFI_PASS` environment variables are set at build time,
//! the network stack uses them directly and skips flash storage entirely. This is a
//! development convenience — never use in production builds.

/// Wi-Fi credentials stored in flash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credentials {
    /// Wi-Fi SSID (up to 32 bytes).
    pub ssid: heapless::String<32>,
    /// Wi-Fi password (up to 64 bytes).
    pub password: heapless::String<64>,
    /// MQTT broker host (up to 64 bytes). Empty string means MQTT is disabled.
    pub mqtt_host: heapless::String<64>,
    /// MQTT broker port. Default: 1883.
    pub mqtt_port: u16,
}

impl Credentials {
    /// Create credentials from string slices.
    /// Returns `None` if either string exceeds the capacity.
    /// MQTT fields default to empty host and port 1883.
    #[must_use]
    pub fn new(ssid: &str, password: &str) -> Option<Self> {
        let ssid = heapless::String::try_from(ssid).ok()?;
        let password = heapless::String::try_from(password).ok()?;
        Some(Self {
            ssid,
            password,
            mqtt_host: heapless::String::new(),
            mqtt_port: 1883,
        })
    }

    /// Create credentials with MQTT broker configuration.
    /// Returns `None` if any string exceeds its capacity.
    #[must_use]
    pub fn with_mqtt(ssid: &str, password: &str, mqtt_host: &str, mqtt_port: u16) -> Option<Self> {
        let ssid = heapless::String::try_from(ssid).ok()?;
        let password = heapless::String::try_from(password).ok()?;
        let mqtt_host = heapless::String::try_from(mqtt_host).ok()?;
        Some(Self {
            ssid,
            password,
            mqtt_host,
            mqtt_port,
        })
    }
}

/// Error type for storage operations.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StorageError {
    /// Flash write failed.
    WriteError,
    /// Flash read returned corrupt data.
    CorruptData,
    /// Serialization or deserialization error.
    SerdeError,
}

/// Load credentials from flash storage.
///
/// Returns `Some(Credentials)` if valid credentials are found, `None` otherwise
/// (blank flash, erased sector, or corrupt data).
///
/// **Stub:** always returns `None`. Full implementation uses `sequential-storage`.
#[must_use]
pub const fn load_credentials() -> Option<Credentials> {
    // Phase 6a stub — always returns None.
    // Full implementation reads from the CREDENTIALS flash region via sequential-storage.
    // Compile-time credential override is handled in net::start via option_env!.
    None
}

/// Save credentials to flash storage.
///
/// Called after a successful provisioning test (Phase 6d).
///
/// **Stub:** always returns `Ok(())`. Full implementation writes to flash via `sequential-storage`.
///
/// # Errors
///
/// Returns `Err` if writing to flash fails (stub always returns `Ok(())`).
#[allow(unused_variables)]
pub const fn save_credentials(_creds: &Credentials) -> Result<(), StorageError> {
    // Phase 6a stub — no-op.
    // Full implementation writes to the CREDENTIALS flash region via sequential-storage.
    Ok(())
}

/// Erase the credentials flash sector.
///
/// Called during factory reset (Phase 6f) when BOOTSEL is held for 5 seconds at boot.
///
/// **Stub:** always returns `Ok(())`.
///
/// # Errors
///
/// Returns `Err` if erasing the flash sector fails (stub always returns `Ok(())`).
pub const fn erase_credentials() -> Result<(), StorageError> {
    // Phase 6f stub — no-op in stub mode.
    Ok(())
}
