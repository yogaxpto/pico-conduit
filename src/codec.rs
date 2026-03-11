//! Wire codec abstraction — separates the serialisation format from transport framing.
//!
//! The [`Codec`] trait defines the two operations that differ between JSON and binary
//! encodings: parsing an incoming command frame and serialising an outgoing response.
//!
//! ## Available codecs
//!
//! | Codec | Feature | Format | Size (typical GPIO cmd) |
//! |-------|---------|--------|------------------------|
//! | [`JsonCodec`] | _(default, always available)_ | UTF-8 JSON | ~80 bytes |
//! | [`PostcardCodec`] | `codec-postcard` | postcard binary | ~15 bytes |
//!
//! ## Crate evaluation
//!
//! Three `no_std` binary codecs were evaluated:
//!
//! - **`rmp-serde`** (MessagePack + serde): requires `alloc` for most operations;
//!   not viable for bare-metal `no_std` without a heap allocator.
//! - **`minicbor`** (CBOR): `no_std` compatible, but uses its own derive macros
//!   rather than standard `serde` traits — would require rewriting all type annotations.
//! - **`postcard`**: compact binary format, excellent `no_std` + no-alloc support,
//!   fully `serde`-compatible. Chosen as the binary codec implementation.
//!
//! ## Mutual exclusion
//!
//! Only one binary codec may be active at a time. A `compile_error!` guard is emitted
//! when both `codec-postcard` and any future `codec-cbor` feature are enabled together.
//! (See `src/lib.rs`.)

use crate::protocol::{Command, Response};

/// A compile-time-selectable wire codec for commands and responses.
///
/// Both methods operate on raw byte slices (as returned by [`crate::transport::Transport`]),
/// allowing the codec to be swapped without changing any transport or router logic.
pub trait Codec {
    /// Parse a command from a raw byte frame.
    fn parse_command<'a>(bytes: &'a [u8]) -> Result<Command<'a>, &'static str>;

    /// Serialise a response into `buf`, returning the number of bytes written.
    fn serialize_response(resp: &Response<'_>, buf: &mut [u8]) -> Result<usize, &'static str>;
}

// ── JSON codec (always available) ────────────────────────────────────────────

/// JSON codec — the default wire encoding.
///
/// Delegates to [`crate::protocol::parse_command`] and
/// [`crate::protocol::serialize_response`].
pub struct JsonCodec;

impl Codec for JsonCodec {
    fn parse_command<'a>(bytes: &'a [u8]) -> Result<Command<'a>, &'static str> {
        crate::protocol::parse_command(bytes)
    }

    fn serialize_response(resp: &Response<'_>, buf: &mut [u8]) -> Result<usize, &'static str> {
        crate::protocol::serialize_response(resp, buf)
    }
}

// ── postcard codec (optional) ─────────────────────────────────────────────────

#[cfg(feature = "codec-postcard")]
pub use self::postcard_impl::{PostcardCodec, encode_command_postcard};

#[cfg(feature = "codec-postcard")]
mod postcard_impl {
    use super::{Codec, Command, Response};
    use crate::protocol::{
        ERROR_MALFORMED_JSON, ERROR_MSG_TOO_LARGE, MAX_PAYLOAD_LEN, ResponseData,
    };
    use serde::Serialize;

    /// Postcard binary codec.
    ///
    /// Commands are encoded as postcard binary frames (no newline delimiter).
    /// Responses are encoded as [`BinaryResponse`] — a flat tagged struct that
    /// avoids `#[serde(untagged)]` which postcard does not support.
    ///
    /// **Size comparison (GPIO read command):**
    /// - JSON: ~80 bytes
    /// - Postcard: ~15 bytes (~5× smaller)
    pub struct PostcardCodec;

    impl Codec for PostcardCodec {
        fn parse_command<'a>(bytes: &'a [u8]) -> Result<Command<'a>, &'static str> {
            postcard::from_bytes(bytes).map_err(|_| ERROR_MALFORMED_JSON)
        }

        fn serialize_response(resp: &Response<'_>, buf: &mut [u8]) -> Result<usize, &'static str> {
            let bin = BinaryResponse::from_response(resp);
            postcard::to_slice(&bin, buf)
                .map(|s| s.len())
                .map_err(|_| ERROR_MSG_TOO_LARGE)
        }
    }

    /// Encode a `Command` to postcard binary format into `buf`.
    ///
    /// Exposed for testing and for clients that need to construct binary frames.
    /// Returns the number of bytes written.
    pub fn encode_command_postcard(
        cmd: &Command<'_>,
        buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        postcard::to_slice(cmd, buf)
            .map(|s| s.len())
            .map_err(|_| ERROR_MSG_TOO_LARGE)
    }

    /// Compact binary representation of a [`Response`].
    ///
    /// [`ResponseData`] uses `#[serde(untagged)]` which postcard does not support
    /// (postcard requires a fixed-size discriminant for enum variants). This type
    /// mirrors the same data with a regular externally-tagged enum.
    #[derive(Serialize)]
    struct BinaryResponse<'a> {
        id: &'a str,
        ok: bool,
        error: Option<&'a str>,
        data: Option<BinaryData>,
    }

    /// Postcard-compatible mirror of [`ResponseData`].
    ///
    /// Uses a standard enum discriminant byte instead of `#[serde(untagged)]`.
    #[allow(clippy::large_enum_variant)]
    #[derive(Serialize)]
    enum BinaryData {
        GpioRead(u8),
        /// `(raw, voltage_bits)` — voltage stored as `f32::to_bits()` to avoid
        /// any f32 formatting complexity; the client reconstructs with `f32::from_bits()`.
        AdcRead(u16, u32),
        /// `celsius_bits` — temperature as `f32::to_bits()`.
        AdcTemp(u32),
        /// Raw byte payload (UART / SPI / I2C transfers).
        Bytes(heapless::Vec<u8, MAX_PAYLOAD_LEN>),
        /// Firmware version string.
        Version(heapless::String<16>),
        /// Config report (non-performance-critical; not decoded by binary clients).
        Config,
    }

    impl<'a> BinaryResponse<'a> {
        fn from_response(resp: &'a Response<'_>) -> Self {
            let data = resp.data.as_ref().map(|d| match d {
                ResponseData::GpioRead { value } => BinaryData::GpioRead(*value),
                ResponseData::AdcRead { raw, voltage } => {
                    BinaryData::AdcRead(*raw, voltage.to_bits())
                }
                ResponseData::AdcTemp { celsius } => BinaryData::AdcTemp(celsius.to_bits()),
                ResponseData::Bytes { bytes } => BinaryData::Bytes(bytes.0.clone()),
                ResponseData::Version { version } => BinaryData::Version(version.clone()),
                ResponseData::Config { .. } => BinaryData::Config,
            });
            BinaryResponse { id: resp.id, ok: resp.ok, error: resp.error, data }
        }
    }
}
