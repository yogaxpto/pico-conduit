//! Minimal base64 encoder/decoder for `no_std` environments.
//!
//! Uses the standard alphabet (A–Z, a–z, 0–9, +, /) with `=` padding on encode.
//! Decode accepts input with or without padding.

const ENCODE_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Decode a single base64 ASCII byte to its 6-bit value, or 0xFF if invalid.
const fn decode_byte(b: u8) -> u8 {
    match b {
        b'A'..=b'Z' => b - b'A',
        b'a'..=b'z' => b - b'a' + 26,
        b'0'..=b'9' => b - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 0xFF,
    }
}

/// Encode `input` as base64 into `output`. Returns the number of bytes written.
///
/// # Panics
///
/// Panics if `output` is too small. Required size: `(input.len() / 3 + 1) * 4`.
pub fn encode(input: &[u8], output: &mut [u8]) -> usize {
    let mut i = 0;
    let mut o = 0;
    let len = input.len();

    // Process full 3-byte groups.
    while i + 3 <= len {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        let b2 = input[i + 2] as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        output[o] = ENCODE_TABLE[((triple >> 18) & 0x3F) as usize];
        output[o + 1] = ENCODE_TABLE[((triple >> 12) & 0x3F) as usize];
        output[o + 2] = ENCODE_TABLE[((triple >> 6) & 0x3F) as usize];
        output[o + 3] = ENCODE_TABLE[(triple & 0x3F) as usize];

        i += 3;
        o += 4;
    }

    let remaining = len - i;
    if remaining == 1 {
        let b0 = input[i] as u32;
        output[o] = ENCODE_TABLE[((b0 >> 2) & 0x3F) as usize];
        output[o + 1] = ENCODE_TABLE[((b0 << 4) & 0x3F) as usize];
        output[o + 2] = b'=';
        output[o + 3] = b'=';
        o += 4;
    } else if remaining == 2 {
        let b0 = input[i] as u32;
        let b1 = input[i + 1] as u32;
        output[o] = ENCODE_TABLE[((b0 >> 2) & 0x3F) as usize];
        output[o + 1] = ENCODE_TABLE[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize];
        output[o + 2] = ENCODE_TABLE[((b1 << 2) & 0x3F) as usize];
        output[o + 3] = b'=';
        o += 4;
    }

    o
}

/// Decode base64 `input` into `output`. Returns the number of bytes written, or `Err(())`
/// if the input contains invalid characters.
///
/// Accepts input with or without `=` padding.
///
/// # Panics
///
/// Panics if `output` is too small. Required size: `input.len() / 4 * 3 + 3`.
#[allow(clippy::result_unit_err)]
pub fn decode(input: &[u8], output: &mut [u8]) -> Result<usize, ()> {
    // Strip trailing padding.
    let len = input.len();
    let stripped = if len >= 2 && input[len - 1] == b'=' && input[len - 2] == b'=' {
        len - 2
    } else if len >= 1 && input[len - 1] == b'=' {
        len - 1
    } else {
        len
    };

    let mut i = 0;
    let mut o = 0;

    // Process full 4-character groups.
    while i + 4 <= stripped {
        let a = decode_byte(input[i]);
        let b = decode_byte(input[i + 1]);
        let c = decode_byte(input[i + 2]);
        let d = decode_byte(input[i + 3]);
        if (a | b | c | d) == 0xFF {
            return Err(());
        }
        output[o] = (a << 2) | (b >> 4);
        output[o + 1] = (b << 4) | (c >> 2);
        output[o + 2] = (c << 6) | d;
        i += 4;
        o += 3;
    }

    let remaining = stripped - i;
    if remaining == 2 {
        let a = decode_byte(input[i]);
        let b = decode_byte(input[i + 1]);
        if (a | b) == 0xFF {
            return Err(());
        }
        output[o] = (a << 2) | (b >> 4);
        o += 1;
    } else if remaining == 3 {
        let a = decode_byte(input[i]);
        let b = decode_byte(input[i + 1]);
        let c = decode_byte(input[i + 2]);
        if (a | b | c) == 0xFF {
            return Err(());
        }
        output[o] = (a << 2) | (b >> 4);
        output[o + 1] = (b << 4) | (c >> 2);
        o += 2;
    } else if remaining == 1 {
        // Single trailing character is invalid base64.
        return Err(());
    }

    Ok(o)
}
