//! Transport abstraction — decouples the command-processing loop from the underlying
//! wire protocol (TCP, WebSocket, MQTT).
//!
//! The [`Transport`] trait provides `read_frame` / `write_frame` so that `handle_client`
//! can work identically across all transports. Each implementation maps its internal
//! errors into [`TransportError`] variants.
//!
//! This module is `no_std`-compatible and compiles on both embedded and host targets.

/// Unified error type returned by all Transport implementations.
///
/// Each impl maps its internal errors into these variants so that
/// `handle_client` can branch on disconnect vs. protocol errors
/// without knowing the concrete transport type.
#[derive(Debug, PartialEq)]
pub enum TransportError {
    /// Clean disconnect or connection lost — caller should exit the
    /// client loop and wait for the next connection (TCP/WS) or
    /// reconnect to the broker (MQTT).
    Disconnected,
    /// A protocol-level error (malformed frame, handshake failure, etc.).
    /// The `&'static str` is an error code from `src/protocol.rs`.
    Protocol(&'static str),
    /// Read timed out with no data.  Treated the same as Disconnected
    /// by `handle_client` but lets transports distinguish internally.
    Timeout,
}

/// Unified transport interface for the command-processing loop.
///
/// Implementations wrap a concrete connection type (TCP socket, WebSocket, MQTT client)
/// and provide framed JSON message I/O.
pub trait Transport {
    /// Read one complete framed message into `buf`.
    /// Returns a sub-slice of `buf` containing the JSON bytes (no trailing newline).
    ///
    /// **Timeout contract:** each implementation is responsible for its own idle
    /// timeout.  TCP and WebSocket wrap reads with `embassy_time::with_timeout`
    /// (30 s, matching the current `TCP_READ_TIMEOUT`).  MQTT's `read_frame`
    /// polls `minimq`, which handles keepalive internally; a broker disconnect
    /// surfaces as `TransportError::Disconnected`.
    fn read_frame<'b>(
        &mut self,
        buf: &'b mut [u8],
    ) -> impl core::future::Future<Output = Result<&'b [u8], TransportError>>;

    /// Write one complete framed message.
    /// `data` is raw JSON bytes (no trailing newline needed by caller).
    fn write_frame(
        &mut self,
        data: &[u8],
    ) -> impl core::future::Future<Output = Result<(), TransportError>>;
}
