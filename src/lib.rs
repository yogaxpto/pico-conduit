//! pico-socketeer library — all testable logic modules.
//!
//! This lib crate is `no_std`-compatible and compiles for both the embedded target
//! (thumbv8m.main-none-eabihf) and the host (for Tier 1 + Tier 2 tests).
//!
//! - Tier 1 & 2: host tests live in `tests/host/` (run with `cargo test --test host`)
//! - Tier 4: TCP integration tests live in `tests/integration/` (require hardware)
//!
//! The firmware binary (`src/main.rs`) imports from this lib and adds the embedded-only
//! networking glue (`src/net.rs`).

#![no_std]

pub mod led;
pub mod protocol;
pub mod router;
pub mod provisioning;
pub mod interfaces;
