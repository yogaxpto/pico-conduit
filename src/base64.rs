//! Minimal base64 encoder/decoder for `no_std` environments.
//!
//! Uses the standard alphabet (A–Z, a–z, 0–9, +, /) with `=` padding on encode.
//! Decode accepts input with or without padding.

const ENCODE_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// 256-byte lookup table: maps ASCII byte → 6-bit value (0–63), or 0xFF if invalid.
/// Lives in `.rodata` (flash). Replaces a 5-branch match with a single indexed load.
const DECODE_TABLE: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut i = 0u8;
    loop {
        t[(b'A' + i) as usize] = i;
        if i == 25 {
            break;
        }
        i += 1;
    }
    i = 0;
    loop {
        t[(b'a' + i) as usize] = i + 26;
        if i == 25 {
            break;
        }
        i += 1;
    }
    i = 0;
    loop {
        t[(b'0' + i) as usize] = i + 52;
        if i == 9 {
            break;
        }
        i += 1;
    }
    t[b'+' as usize] = 62;
    t[b'/' as usize] = 63;
    t
};

/// Encode `input` as base64 into `output`. Returns the number of bytes written.
///
/// # Panics
///
/// Panics if `output` is too small. Required size: `(input.len() / 3 + 1) * 4`.
pub fn encode(input: &[u8], output: &mut [u8]) -> usize {
    let mut o = 0;
    let chunks = input.chunks_exact(3);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let triple = u32::from(chunk[0]) << 16 | u32::from(chunk[1]) << 8 | u32::from(chunk[2]);
        output[o] = ENCODE_TABLE[((triple >> 18) & 0x3F) as usize];
        output[o + 1] = ENCODE_TABLE[((triple >> 12) & 0x3F) as usize];
        output[o + 2] = ENCODE_TABLE[((triple >> 6) & 0x3F) as usize];
        output[o + 3] = ENCODE_TABLE[(triple & 0x3F) as usize];
        o += 4;
    }

    match remainder.len() {
        1 => {
            let b0 = u32::from(remainder[0]);
            output[o] = ENCODE_TABLE[((b0 >> 2) & 0x3F) as usize];
            output[o + 1] = ENCODE_TABLE[((b0 << 4) & 0x3F) as usize];
            output[o + 2] = b'=';
            output[o + 3] = b'=';
            o += 4;
        }
        2 => {
            let b0 = u32::from(remainder[0]);
            let b1 = u32::from(remainder[1]);
            output[o] = ENCODE_TABLE[((b0 >> 2) & 0x3F) as usize];
            output[o + 1] = ENCODE_TABLE[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize];
            output[o + 2] = ENCODE_TABLE[((b1 << 2) & 0x3F) as usize];
            output[o + 3] = b'=';
            o += 4;
        }
        _ => {}
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
#[allow(clippy::result_unit_err, clippy::many_single_char_names)]
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

    let data = &input[..stripped];
    let chunks = data.chunks_exact(4);
    let remainder = chunks.remainder();
    let mut o = 0;

    for chunk in chunks {
        let a = DECODE_TABLE[chunk[0] as usize];
        let b = DECODE_TABLE[chunk[1] as usize];
        let c = DECODE_TABLE[chunk[2] as usize];
        let d = DECODE_TABLE[chunk[3] as usize];
        if (a | b | c | d) & 0x80 != 0 {
            return Err(());
        }
        output[o] = (a << 2) | (b >> 4);
        output[o + 1] = (b << 4) | (c >> 2);
        output[o + 2] = (c << 6) | d;
        o += 3;
    }

    match remainder.len() {
        2 => {
            let a = DECODE_TABLE[remainder[0] as usize];
            let b = DECODE_TABLE[remainder[1] as usize];
            if (a | b) & 0x80 != 0 {
                return Err(());
            }
            output[o] = (a << 2) | (b >> 4);
            o += 1;
        }
        3 => {
            let a = DECODE_TABLE[remainder[0] as usize];
            let b = DECODE_TABLE[remainder[1] as usize];
            let c = DECODE_TABLE[remainder[2] as usize];
            if (a | b | c) & 0x80 != 0 {
                return Err(());
            }
            output[o] = (a << 2) | (b >> 4);
            output[o + 1] = (b << 4) | (c >> 2);
            o += 2;
        }
        1 => {
            // Single trailing character is invalid base64.
            return Err(());
        }
        _ => {}
    }

    Ok(o)
}
