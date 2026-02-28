//! Wi-Fi provisioning: credential storage and captive portal.
//!
//! # Modules
//! - [`storage`] — load/save Wi-Fi credentials from/to flash (Phase 6a)
//! - [`portal`] — HTTP captive portal request parsing (Phase 6c)

pub mod portal;
pub mod storage;
