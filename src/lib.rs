//! pico-socketeer library — all testable logic modules.
//!
//! This lib crate is `no_std`-compatible and compiles for both the embedded target
//! (thumbv8m.main-none-eabihf) and the host (for Tier 1 + Tier 2 tests).
//!
//! - Tier 1: host unit tests in `#[cfg(test)]` modules inside each source file
//! - Tier 2: mock-hardware tests using `embedded-hal-mock` in `#[cfg(test)]` modules
//!
//! The firmware binary (`src/main.rs`) imports from this lib and adds the embedded-only
//! networking glue (`src/net.rs`).

#![no_std]

pub mod led;
pub mod protocol;
pub mod router;
pub mod provisioning;
pub mod interfaces;
