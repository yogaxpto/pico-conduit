//! pico-conduit library — all testable logic modules.
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
// CLAUDE.md requires explicit 'a lifetimes on functions returning Command<'a>/Response<'a>.
// This conflicts with clippy::pedantic's elidable_lifetime_names lint.
#![allow(clippy::elidable_lifetime_names)]
// Embassy tasks run on a single-core cooperative executor and are never sent across threads.
#![allow(clippy::future_not_send)]

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

// codec-postcard is the only binary codec; guard against a hypothetical second one.
// When codec-cbor or another binary codec is added, add it to the lhs of this check.
// Currently this guard always passes — it documents the mutual-exclusion pattern.
// #[cfg(all(feature = "codec-postcard", feature = "codec-cbor"))]
// compile_error!("codec features `codec-postcard` and `codec-cbor` are mutually exclusive");

pub mod base64;
pub mod board;
pub mod codec;
pub mod interfaces;
pub mod led;
pub mod mqtt;
pub mod protocol;
pub mod provisioning;
pub mod router;
pub mod transport;
pub mod ws;
