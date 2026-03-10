//! pico-socketeer library — all testable logic modules.
//!
//! This lib crate is `no_std`-compatible and compiles for both the embedded target
//! (`thumbv8m.main-none-eabihf` or `thumbv6m-none-eabi`) and the host (for Tier 1 + Tier 2
//! tests).
//!
//! - Tier 1 & 2: host tests live in `tests/host/` (run with `cargo test --test host`)
//! - Tier 4: TCP integration tests live in `tests/integration/` (require hardware)
//!
//! The firmware binary (`src/main.rs`) imports from this lib and adds the embedded-only
//! networking glue (`src/net.rs`).

#![no_std]
// Response<'a> is large (~540 bytes) because it embeds Base64Bytes (up to 512-byte payload).
// On no_std we cannot Box it, so allow the large-err lint crate-wide.
#![allow(clippy::result_large_err)]

#[cfg(all(feature = "pico2w", feature = "pico1w"))]
compile_error!("features `pico2w` and `pico1w` are mutually exclusive");

#[cfg(all(feature = "embedded", not(feature = "pico2w"), not(feature = "pico1w")))]
compile_error!("embedded builds require exactly one of `pico2w` or `pico1w`");

#[cfg(all(feature = "transport-tcp", feature = "transport-websocket"))]
compile_error!(
    "transport features `transport-tcp` and `transport-websocket` are mutually exclusive"
);

#[cfg(all(feature = "transport-tcp", feature = "transport-mqtt"))]
compile_error!("transport features `transport-tcp` and `transport-mqtt` are mutually exclusive");

#[cfg(all(feature = "transport-websocket", feature = "transport-mqtt"))]
compile_error!(
    "transport features `transport-websocket` and `transport-mqtt` are mutually exclusive"
);

#[cfg(all(
    feature = "embedded",
    not(feature = "transport-tcp"),
    not(feature = "transport-websocket"),
    not(feature = "transport-mqtt")
))]
compile_error!(
    "embedded builds require exactly one of: transport-tcp, transport-websocket, transport-mqtt"
);

pub mod base64;
pub mod board;
pub mod interfaces;
pub mod led;
pub mod mqtt;
pub mod protocol;
pub mod provisioning;
pub mod router;
pub mod transport;
pub mod ws;
