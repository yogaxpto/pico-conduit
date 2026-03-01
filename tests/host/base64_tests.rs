use pico_socketeer::base64::{decode, encode};

// ----- Encode -----

#[test]
fn encode_empty() {
    let mut buf = [0u8; 4];
    let n = encode(&[], &mut buf);
    assert_eq!(n, 0);
}

#[test]
fn encode_one_byte() {
    let mut buf = [0u8; 4];
    let n = encode(&[0x41], &mut buf); // 'A'
    assert_eq!(&buf[..n], b"QQ==");
}

#[test]
fn encode_two_bytes() {
    let mut buf = [0u8; 4];
    let n = encode(&[0x01, 0x02], &mut buf);
    assert_eq!(&buf[..n], b"AQI=");
}

#[test]
fn encode_three_bytes() {
    // 3-byte group: no padding needed
    let mut buf = [0u8; 4];
    let n = encode(&[0x48, 0x65, 0x6C], &mut buf); // "Hel"
    assert_eq!(&buf[..n], b"SGVs");
}

#[test]
fn encode_hello() {
    let mut buf = [0u8; 12];
    let n = encode(b"Hello", &mut buf);
    assert_eq!(&buf[..n], b"SGVsbG8=");
}

#[test]
fn encode_0xff() {
    let mut buf = [0u8; 4];
    let n = encode(&[0xFF], &mut buf);
    assert_eq!(&buf[..n], b"/w==");
}

// ----- Decode -----

#[test]
fn decode_empty() {
    let mut buf = [0u8; 4];
    let n = decode(b"", &mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn decode_one_byte_padded() {
    let mut buf = [0u8; 4];
    let n = decode(b"QQ==", &mut buf).unwrap();
    assert_eq!(&buf[..n], &[0x41]);
}

#[test]
fn decode_one_byte_unpadded() {
    let mut buf = [0u8; 4];
    let n = decode(b"QQ", &mut buf).unwrap();
    assert_eq!(&buf[..n], &[0x41]);
}

#[test]
fn decode_two_bytes_padded() {
    let mut buf = [0u8; 4];
    let n = decode(b"AQI=", &mut buf).unwrap();
    assert_eq!(&buf[..n], &[0x01, 0x02]);
}

#[test]
fn decode_three_bytes() {
    let mut buf = [0u8; 4];
    let n = decode(b"SGVs", &mut buf).unwrap();
    assert_eq!(&buf[..n], &[0x48, 0x65, 0x6C]);
}

#[test]
fn decode_hello() {
    let mut buf = [0u8; 8];
    let n = decode(b"SGVsbG8=", &mut buf).unwrap();
    assert_eq!(&buf[..n], b"Hello");
}

#[test]
fn decode_0xff() {
    let mut buf = [0u8; 4];
    let n = decode(b"/w==", &mut buf).unwrap();
    assert_eq!(&buf[..n], &[0xFF]);
}

// ----- Round-trip -----

#[test]
fn round_trip_all_byte_values() {
    // Encode all 256 byte values and decode back.
    let input: Vec<u8> = (0..=255).collect();
    let mut enc_buf = [0u8; 512];
    let enc_len = encode(&input, &mut enc_buf);
    let mut dec_buf = [0u8; 256];
    let dec_len = decode(&enc_buf[..enc_len], &mut dec_buf).unwrap();
    assert_eq!(&dec_buf[..dec_len], &input[..]);
}

#[test]
fn round_trip_empty() {
    let mut enc_buf = [0u8; 4];
    let enc_len = encode(&[], &mut enc_buf);
    let mut dec_buf = [0u8; 4];
    let dec_len = decode(&enc_buf[..enc_len], &mut dec_buf).unwrap();
    assert_eq!(dec_len, 0);
}

// ----- Invalid input -----

#[test]
fn decode_invalid_character() {
    let mut buf = [0u8; 4];
    assert!(decode(b"QQ!!", &mut buf).is_err());
}

#[test]
fn decode_single_trailing_char_is_invalid() {
    let mut buf = [0u8; 4];
    // A single character (not multiple of 2, 3, or 4) is invalid.
    assert!(decode(b"Q", &mut buf).is_err());
}

#[test]
fn decode_whitespace_is_invalid() {
    let mut buf = [0u8; 4];
    assert!(decode(b"QQ =", &mut buf).is_err());
}
