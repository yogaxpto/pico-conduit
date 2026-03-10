use pico_socketeer::board::{TCP_PORT, WS_PORT};
use pico_socketeer::protocol::{ERROR_MSG_TOO_LARGE, ERROR_WEBSOCKET_HANDSHAKE, MAX_MSG_LEN};
use pico_socketeer::ws::{
    OPCODE_PONG, compute_accept_key, encode_pong_frame, encode_text_frame,
    encode_text_frame_header, parse_frame_header, unmask,
};

// ----- WS_PORT constant tests -----

#[test]
fn ws_port_is_4243() {
    assert_eq!(WS_PORT, 4243);
}

#[test]
fn ws_port_differs_from_tcp() {
    assert_ne!(WS_PORT, TCP_PORT);
}

// ----- WebSocket frame header encoding -----

#[test]
fn ws_text_frame_header_small_payload() {
    let mut out = [0u8; 4];
    let n = encode_text_frame_header(100, &mut out).unwrap();
    assert_eq!(n, 2);
    assert_eq!(out[0], 0x81); // FIN=1, opcode=text
    assert_eq!(out[1], 100);
}

#[test]
fn ws_text_frame_header_medium_payload() {
    let mut out = [0u8; 4];
    let n = encode_text_frame_header(200, &mut out).unwrap();
    assert_eq!(n, 4);
    assert_eq!(out[0], 0x81);
    assert_eq!(out[1], 126);
    let len = ((out[2] as usize) << 8) | (out[3] as usize);
    assert_eq!(len, 200);
}

// ----- Unmask round-trip -----

#[test]
fn ws_unmask_round_trip() {
    let original = b"Hello, WebSocket!";
    let mask_key = [0x37, 0xFA, 0x21, 0x3D];
    let mut data = original.to_vec();
    unmask(&mut data, mask_key);
    // After first mask, data should differ from original
    assert_ne!(&data[..], &original[..]);
    // After second mask (unmask), data should match original
    unmask(&mut data, mask_key);
    assert_eq!(&data[..], &original[..]);
}

#[test]
fn ws_unmask_empty_payload() {
    let mut data: [u8; 0] = [];
    unmask(&mut data, [0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(data.len(), 0);
}

// ----- Frame size boundary tests -----

#[test]
fn ws_frame_exactly_max_msg_len() {
    let mut out = [0u8; MAX_MSG_LEN + 4];
    let payload = [b'x'; MAX_MSG_LEN];
    let result = encode_text_frame(&payload, &mut out);
    assert!(result.is_ok(), "1024-byte payload should fit");
    let total = result.unwrap();
    // 4-byte header (126 extended) + 1024 payload
    assert_eq!(total, 4 + MAX_MSG_LEN);
}

#[test]
fn ws_frame_over_max_msg_len_rejected() {
    let mut out = [0u8; MAX_MSG_LEN + 10];
    let payload = [b'x'; MAX_MSG_LEN + 1];
    let result = encode_text_frame(&payload, &mut out);
    assert_eq!(result, Err(ERROR_MSG_TOO_LARGE));
}

// ----- Error constant -----

#[test]
fn ws_handshake_error_is_static_str() {
    assert_eq!(ERROR_WEBSOCKET_HANDSHAKE, "ws_handshake_failed");
}

// ----- Pong frame encoding -----

#[test]
fn ws_pong_frame_encodes_correctly() {
    let mut out = [0u8; 16];
    let n = encode_pong_frame(b"ping", &mut out).unwrap();
    assert_eq!(n, 6); // 2 header + 4 payload
    assert_eq!(out[0], 0x80 | OPCODE_PONG);
    assert_eq!(out[1], 4);
    assert_eq!(&out[2..6], b"ping");
}

// ----- Parse frame header -----

#[test]
fn ws_parse_masked_text_frame_header() {
    // FIN=1, opcode=1 (text), MASK=1, len=5, mask_key=[1,2,3,4]
    let raw = [0x81, 0x85, 0x01, 0x02, 0x03, 0x04];
    let hdr = parse_frame_header(&raw).unwrap();
    assert_eq!(hdr.opcode, 0x1);
    assert!(hdr.masked);
    assert_eq!(hdr.payload_len, 5);
    assert_eq!(hdr.mask_key, [1, 2, 3, 4]);
    assert_eq!(hdr.header_len, 6); // 2 + 4 (mask key)
}

// ----- SHA-1 / accept key (RFC 6455 section 4.2.2 test vector) -----

#[test]
fn ws_compute_accept_key_rfc6455_vector() {
    // RFC 6455 section 4.2.2 example:
    // Client key: "dGhlIHNhbXBsZSBub25jZQ=="
    // Expected accept: "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    let client_key = b"dGhlIHNhbXBsZSBub25jZQ==";
    let mut out = [0u8; 28];
    let n = compute_accept_key(client_key, &mut out);
    let accept = core::str::from_utf8(&out[..n]).unwrap();
    assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}
