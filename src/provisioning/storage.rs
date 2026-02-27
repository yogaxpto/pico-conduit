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
#[derive(Clone, Debug, PartialEq)]
pub struct Credentials {
    /// Wi-Fi SSID (up to 32 bytes).
    pub ssid: heapless::String<32>,
    /// Wi-Fi password (up to 64 bytes).
    pub password: heapless::String<64>,
}

impl Credentials {
    /// Create credentials from string slices.
    /// Returns `None` if either string exceeds the capacity.
    pub fn new(ssid: &str, password: &str) -> Option<Self> {
        let ssid = heapless::String::try_from(ssid).ok()?;
        let password = heapless::String::try_from(password).ok()?;
        Some(Self { ssid, password })
    }
}

/// Error type for storage operations.
#[derive(Debug, PartialEq, Clone, Copy)]
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
pub fn load_credentials() -> Option<Credentials> {
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
#[allow(unused_variables)]
pub fn save_credentials(_creds: &Credentials) -> Result<(), StorageError> {
    // Phase 6a stub — no-op.
    // Full implementation writes to the CREDENTIALS flash region via sequential-storage.
    Ok(())
}

/// Erase the credentials flash sector.
///
/// Called during factory reset (Phase 6f) when BOOTSEL is held for 5 seconds at boot.
///
/// **Stub:** always returns `Ok(())`.
pub fn erase_credentials() -> Result<(), StorageError> {
    // Phase 6f stub — no-op in stub mode.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
