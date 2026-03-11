//! WebSocket framing helpers — pure functions for frame encoding, decoding, and masking.
//!
//! This module is `no_std`-compatible and compiles on both embedded and host targets.
//! The embedded-only WebSocket server task and `WsTransport` live in `src/net.rs`.

use crate::protocol::{ERROR_MSG_TOO_LARGE, MAX_MSG_LEN};

/// WebSocket opcodes (RFC 6455 section 5.2).
pub const OPCODE_TEXT: u8 = 0x1;
pub const OPCODE_CLOSE: u8 = 0x8;
pub const OPCODE_PING: u8 = 0x9;
pub const OPCODE_PONG: u8 = 0xA;

/// Apply or remove the XOR mask on a WebSocket frame payload (RFC 6455 section 5.3).
///
/// Masking is symmetric: applying the same key twice restores the original data.
pub fn unmask(data: &mut [u8], mask_key: [u8; 4]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= mask_key[i % 4];
    }
}

/// Encode a WebSocket text frame header (server-to-client, unmasked).
///
/// Writes the frame header into `out` and returns the header length in bytes.
/// Does NOT copy the payload — caller appends payload after the header.
///
/// - Payload ≤ 125 bytes: 2-byte header (FIN=1, opcode=0x1, length)
/// - Payload 126–65535 bytes: 4-byte header (FIN=1, opcode=0x1, 126, 16-bit BE length)
#[allow(clippy::cast_possible_truncation)] // guarded by MAX_MSG_LEN (≤ 1024) and <= 125 checks
pub fn encode_text_frame_header(payload_len: usize, out: &mut [u8]) -> Result<usize, &'static str> {
    if payload_len > MAX_MSG_LEN {
        return Err(ERROR_MSG_TOO_LARGE);
    }

    if payload_len <= 125 {
        if out.len() < 2 {
            return Err(ERROR_MSG_TOO_LARGE);
        }
        out[0] = 0x81; // FIN=1, opcode=1 (text)
        out[1] = payload_len as u8;
        Ok(2)
    } else {
        if out.len() < 4 {
            return Err(ERROR_MSG_TOO_LARGE);
        }
        out[0] = 0x81;
        out[1] = 126;
        out[2] = (payload_len >> 8) as u8;
        out[3] = (payload_len & 0xFF) as u8;
        Ok(4)
    }
}

/// Encode a complete WebSocket text frame (header + payload) into `out`.
///
/// Returns total bytes written.
pub fn encode_text_frame(payload: &[u8], out: &mut [u8]) -> Result<usize, &'static str> {
    let header_len = encode_text_frame_header(payload.len(), out)?;
    let total = header_len + payload.len();
    if total > out.len() {
        return Err(ERROR_MSG_TOO_LARGE);
    }
    out[header_len..total].copy_from_slice(payload);
    Ok(total)
}

/// Parsed WebSocket frame header information.
pub struct WsFrameHeader {
    /// Total header length in bytes (2, 4, or 6/8 with mask key).
    pub header_len: usize,
    /// Payload length in bytes.
    pub payload_len: usize,
    /// WebSocket opcode (0x1=text, 0x8=close, 0x9=ping, 0xA=pong).
    pub opcode: u8,
    /// XOR mask key (valid only if `masked` is true).
    pub mask_key: [u8; 4],
    /// Whether the frame payload is masked (client-to-server frames are always masked).
    pub masked: bool,
}

/// Parse a WebSocket frame header from raw bytes.
///
/// Returns `None` if there are not enough bytes to parse the complete header.
/// Does NOT validate the opcode or consume the payload.
pub fn parse_frame_header(raw: &[u8]) -> Option<WsFrameHeader> {
    if raw.len() < 2 {
        return None;
    }

    let opcode = raw[0] & 0x0F;
    let masked = (raw[1] & 0x80) != 0;
    let payload_len_7 = (raw[1] & 0x7F) as usize;

    let (payload_len, ext_len) = if payload_len_7 <= 125 {
        (payload_len_7, 0)
    } else if payload_len_7 == 126 {
        if raw.len() < 4 {
            return None;
        }
        let len = ((raw[2] as usize) << 8) | (raw[3] as usize);
        (len, 2)
    } else {
        // 64-bit payload length — not supported (max 1024 bytes)
        return None;
    };

    let mask_len = if masked { 4 } else { 0 };
    let header_len = 2 + ext_len + mask_len;

    if raw.len() < header_len {
        return None;
    }

    let mut mask_key = [0u8; 4];
    if masked {
        let mask_start = 2 + ext_len;
        mask_key.copy_from_slice(&raw[mask_start..mask_start + 4]);
    }

    Some(WsFrameHeader {
        header_len,
        payload_len,
        opcode,
        mask_key,
        masked,
    })
}

/// Encode a WebSocket pong frame (server-to-client, unmasked).
///
/// Control frame payloads must be ≤ 125 bytes (RFC 6455 section 5.5).
#[allow(clippy::cast_possible_truncation)] // guarded by > 125 check above
pub fn encode_pong_frame(payload: &[u8], out: &mut [u8]) -> Result<usize, &'static str> {
    if payload.len() > 125 {
        return Err(ERROR_MSG_TOO_LARGE);
    }
    let total = 2 + payload.len();
    if out.len() < total {
        return Err(ERROR_MSG_TOO_LARGE);
    }
    out[0] = 0x80 | OPCODE_PONG; // FIN=1, opcode=pong
    out[1] = payload.len() as u8;
    out[2..total].copy_from_slice(payload);
    Ok(total)
}

// ── WebSocket handshake helpers ──────────────────────────────────────────────

/// WebSocket GUID for `Sec-WebSocket-Accept` computation (RFC 6455 section 4.2.2).
const WS_GUID: &[u8; 36] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Compute the `Sec-WebSocket-Accept` header value for a WebSocket upgrade handshake.
///
/// Concatenates the client's `Sec-WebSocket-Key` with the WebSocket GUID,
/// computes SHA-1, and base64-encodes the result into `out`.
/// Returns the number of bytes written to `out` (always 28).
pub fn compute_accept_key(client_key: &[u8], out: &mut [u8]) -> usize {
    let mut input = [0u8; 100]; // up to 64-byte key + 36-byte GUID
    let key_len = client_key.len().min(64);
    input[..key_len].copy_from_slice(&client_key[..key_len]);
    input[key_len..key_len + WS_GUID.len()].copy_from_slice(WS_GUID);
    let hash = sha1(&input[..key_len + WS_GUID.len()]);
    crate::base64::encode(&hash, out)
}

// ── SHA-1 (RFC 3174) ─────────────────────────────────────────────────────────
// Minimal implementation used only for the WebSocket handshake key derivation.

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let bit_len = (data.len() as u64) * 8;

    // Process complete 64-byte blocks
    let mut offset = 0;
    while offset + 64 <= data.len() {
        sha1_block(&data[offset..offset + 64], &mut h);
        offset += 64;
    }

    // Pad the final block(s)
    let remaining = data.len() - offset;
    let mut pad = [0u8; 128];
    pad[..remaining].copy_from_slice(&data[offset..]);
    pad[remaining] = 0x80;
    let pad_len = if remaining < 56 { 64 } else { 128 };
    pad[pad_len - 8..pad_len].copy_from_slice(&bit_len.to_be_bytes());

    sha1_block(&pad[..64], &mut h);
    if pad_len == 128 {
        sha1_block(&pad[64..128], &mut h);
    }

    let mut out = [0u8; 20];
    for (i, &val) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
    }
    out
}

#[allow(clippy::needless_range_loop, clippy::many_single_char_names)]
fn sha1_block(block: &[u8], h: &mut [u32; 5]) {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *h;

    for i in 0..80 {
        let (f, k) = match i {
            0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999_u32),
            20..=39 => (b ^ c ^ d, 0x6ED9_EBA1_u32),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC_u32),
            _ => (b ^ c ^ d, 0xCA62_C1D6_u32),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(w[i]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
}
