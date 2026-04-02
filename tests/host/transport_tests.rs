use pico_conduit::protocol::{ERROR_MALFORMED_JSON, ERROR_MSG_TOO_LARGE, MAX_MSG_LEN};
use pico_conduit::router::{DeviceState, dispatch, validate_route};
use pico_conduit::transport::{Transport, TransportError};

// ----- TransportError enum tests -----

#[test]
fn transport_error_disconnected_is_distinct() {
    let err = TransportError::Disconnected;
    assert!(matches!(err, TransportError::Disconnected));
    assert!(!matches!(err, TransportError::Timeout));
    assert!(!matches!(err, TransportError::Protocol(_)));
}

#[test]
fn transport_error_protocol_carries_error_code() {
    let err = TransportError::Protocol(ERROR_MSG_TOO_LARGE);
    match err {
        TransportError::Protocol(code) => assert_eq!(code, ERROR_MSG_TOO_LARGE),
        _ => panic!("expected Protocol variant"),
    }
}

#[test]
fn transport_error_timeout_is_distinct() {
    let err = TransportError::Timeout;
    assert!(matches!(err, TransportError::Timeout));
    assert!(!matches!(err, TransportError::Disconnected));
    assert!(!matches!(err, TransportError::Protocol(_)));
}

// ----- MockTransport -----

/// Test-only transport that returns canned frames from `read_frame`
/// and collects written frames.
struct MockTransport {
    /// Canned results to return from `read_frame`, consumed in order.
    reads: Vec<Result<Vec<u8>, TransportError>>,
    /// Index into `reads`.
    read_idx: usize,
    /// Frames written via `write_frame`.
    written: Vec<Vec<u8>>,
}

impl MockTransport {
    fn from_frames(frames: &[&[u8]]) -> Self {
        Self {
            reads: frames.iter().map(|f| Ok(f.to_vec())).collect(),
            read_idx: 0,
            written: Vec::new(),
        }
    }

    fn from_results(results: Vec<Result<Vec<u8>, TransportError>>) -> Self {
        Self {
            reads: results,
            read_idx: 0,
            written: Vec::new(),
        }
    }
}

impl Transport for MockTransport {
    async fn read_frame<'b>(&mut self, buf: &'b mut [u8]) -> Result<&'b [u8], TransportError> {
        if self.read_idx >= self.reads.len() {
            return Err(TransportError::Disconnected);
        }
        let idx = self.read_idx;
        self.read_idx += 1;
        match &self.reads[idx] {
            Ok(data) => {
                let len = data.len();
                buf[..len].copy_from_slice(data);
                Ok(&buf[..len])
            }
            Err(TransportError::Disconnected) => Err(TransportError::Disconnected),
            Err(TransportError::Timeout) => Err(TransportError::Timeout),
            Err(TransportError::Protocol(code)) => Err(TransportError::Protocol(code)),
        }
    }

    async fn write_frame(&mut self, data: &[u8]) -> Result<(), TransportError> {
        self.written.push(data.to_vec());
        Ok(())
    }
}

// ----- handle_client helper -----

/// Run handle_client with a MockTransport and return the transport after completion.
async fn run_handle_client(transport: &mut MockTransport) -> &mut MockTransport {
    let ssid = heapless::String::<32>::new();
    let ip = heapless::String::<16>::new();
    handle_client_generic(transport, &ssid, &ip).await;
    transport
}

/// Mirrors the embedded `handle_client` logic for host testing.
/// This is the transport-generic command loop extracted from net.rs.
async fn handle_client_generic<T: Transport>(
    transport: &mut T,
    config_ssid: &heapless::String<32>,
    config_ip: &heapless::String<16>,
) {
    use pico_conduit::protocol::{Response, parse_command, serialize_response};

    let mut frame_buf = [0u8; MAX_MSG_LEN];
    let mut resp_buf = [0u8; MAX_MSG_LEN];
    let mut device_state = DeviceState {
        config_ssid: config_ssid.clone(),
        config_ip: config_ip.clone(),
        config_connected: true,
        ..DeviceState::default()
    };

    loop {
        let frame = match transport.read_frame(&mut frame_buf).await {
            Ok(frame) => frame,
            Err(TransportError::Protocol(err_code)) => {
                let resp = Response::error("", err_code);
                if let Ok(n) = serialize_response(&resp, &mut resp_buf) {
                    let _ = transport.write_frame(&resp_buf[..n]).await;
                }
                continue;
            }
            Err(TransportError::Disconnected | TransportError::Timeout) => {
                return;
            }
        };

        let response = match parse_command(frame) {
            Err(err_code) => Response::error("", err_code),
            Ok(cmd) => match validate_route(&cmd) {
                Err(r) => r,
                Ok(route) => dispatch(&cmd, route, &mut device_state),
            },
        };

        if let Ok(n) = serialize_response(&response, &mut resp_buf) {
            if transport.write_frame(&resp_buf[..n]).await.is_err() {
                return;
            }
        }

        if device_state.pending_reboot {
            return;
        }
    }
}

// ----- MockTransport tests with handle_client -----

#[test]
fn mock_transport_single_command_roundtrip() {
    pollster_block(async {
        let cmd = br#"{"version":1,"id":"t1","interface":"config","action":"get"}"#;
        let mut transport = MockTransport::from_frames(&[cmd]);
        run_handle_client(&mut transport).await;

        assert_eq!(transport.written.len(), 1, "expected exactly one response");
        let resp = core::str::from_utf8(&transport.written[0]).unwrap();
        assert!(
            resp.contains("\"id\":\"t1\""),
            "response missing id: {resp}"
        );
        assert!(resp.contains("\"ok\":true"), "response missing ok: {resp}");
        assert!(resp.ends_with('\n'), "response missing newline: {resp:?}");
    });
}

#[test]
fn mock_transport_disconnected_exits_cleanly() {
    pollster_block(async {
        let mut transport = MockTransport::from_results(vec![Err(TransportError::Disconnected)]);
        run_handle_client(&mut transport).await;
        assert!(transport.written.is_empty());
    });
}

#[test]
fn mock_transport_timeout_exits_cleanly() {
    pollster_block(async {
        let mut transport = MockTransport::from_results(vec![Err(TransportError::Timeout)]);
        run_handle_client(&mut transport).await;
        assert!(transport.written.is_empty());
    });
}

#[test]
fn mock_transport_protocol_error_exits() {
    pollster_block(async {
        let mut transport =
            MockTransport::from_results(vec![Err(TransportError::Protocol(ERROR_MSG_TOO_LARGE))]);
        run_handle_client(&mut transport).await;
        // Protocol error sends an error response then continues; next read returns Disconnected
        assert_eq!(transport.written.len(), 1);
        let resp = core::str::from_utf8(&transport.written[0]).unwrap();
        assert!(
            resp.contains("msg_too_large"),
            "expected msg_too_large error: {resp}"
        );
    });
}

#[test]
fn mock_transport_multiple_commands() {
    pollster_block(async {
        let cmds: &[&[u8]] = &[
            br#"{"version":1,"id":"1","interface":"config","action":"get"}"#,
            br#"{"version":1,"id":"2","interface":"config","action":"get"}"#,
            br#"{"version":1,"id":"3","interface":"config","action":"get"}"#,
        ];
        let mut transport = MockTransport::from_frames(cmds);
        run_handle_client(&mut transport).await;

        assert_eq!(transport.written.len(), 3, "expected 3 responses");
        for (i, written) in transport.written.iter().enumerate() {
            let resp = core::str::from_utf8(written).unwrap();
            let expected_id = format!("\"id\":\"{}\"", i + 1);
            assert!(
                resp.contains(&expected_id),
                "response {i} missing id: {resp}"
            );
        }
    });
}

#[test]
fn mock_transport_malformed_json_returns_error_response() {
    pollster_block(async {
        let mut transport = MockTransport::from_frames(&[b"not json at all"]);
        run_handle_client(&mut transport).await;

        assert_eq!(transport.written.len(), 1);
        let resp = core::str::from_utf8(&transport.written[0]).unwrap();
        assert!(resp.contains("\"ok\":false"), "expected ok:false: {resp}");
        assert!(
            resp.contains(ERROR_MALFORMED_JSON),
            "expected malformed_json error: {resp}"
        );
    });
}

/// Verify that 5 commands sent consecutively (without waiting for each response)
/// produce 5 ordered responses — this is the server-side correctness guarantee for
/// client-side pipelining as documented in PROTOCOL.md.
#[test]
fn pipelining_five_commands_return_five_ordered_responses() {
    pollster_block(async {
        let cmds: &[&[u8]] = &[
            br#"{"version":1,"id":"p1","interface":"system","action":"get_version"}"#,
            br#"{"version":1,"id":"p2","interface":"system","action":"get_version"}"#,
            br#"{"version":1,"id":"p3","interface":"system","action":"get_version"}"#,
            br#"{"version":1,"id":"p4","interface":"system","action":"get_version"}"#,
            br#"{"version":1,"id":"p5","interface":"system","action":"get_version"}"#,
        ];
        let mut transport = MockTransport::from_frames(cmds);
        run_handle_client(&mut transport).await;

        assert_eq!(transport.written.len(), 5, "expected exactly 5 responses");
        for (i, written) in transport.written.iter().enumerate() {
            let resp = core::str::from_utf8(written).unwrap();
            let expected_id = format!("\"id\":\"p{}\"", i + 1);
            assert!(
                resp.contains(&expected_id),
                "response {i} has wrong id (ordering broken): {resp}"
            );
            assert!(resp.contains("\"ok\":true"), "response {i} not ok: {resp}");
            assert!(
                resp.ends_with('\n'),
                "response {i} missing newline terminator"
            );
        }
    });
}

/// Verify that a malformed command in a pipeline does not abort subsequent commands.
#[test]
fn pipelining_malformed_command_does_not_abort_pipeline() {
    pollster_block(async {
        let cmds: &[&[u8]] = &[
            br#"{"version":1,"id":"q1","interface":"system","action":"get_version"}"#,
            b"this is not json",
            br#"{"version":1,"id":"q3","interface":"system","action":"get_version"}"#,
        ];
        let mut transport = MockTransport::from_frames(cmds);
        run_handle_client(&mut transport).await;

        assert_eq!(
            transport.written.len(),
            3,
            "expected 3 responses (including error)"
        );
        let r0 = core::str::from_utf8(&transport.written[0]).unwrap();
        let r1 = core::str::from_utf8(&transport.written[1]).unwrap();
        let r2 = core::str::from_utf8(&transport.written[2]).unwrap();
        assert!(
            r0.contains("\"id\":\"q1\"") && r0.contains("\"ok\":true"),
            "q1 failed: {r0}"
        );
        assert!(
            r1.contains("\"ok\":false") && r1.contains(ERROR_MALFORMED_JSON),
            "error resp: {r1}"
        );
        assert!(
            r2.contains("\"id\":\"q3\"") && r2.contains("\"ok\":true"),
            "q3 failed: {r2}"
        );
    });
}

#[test]
fn mock_transport_reboot_flag_after_response() {
    pollster_block(async {
        let cmd =
            br#"{"version":1,"id":"r1","interface":"system","action":"reboot_to_bootloader"}"#;
        let mut transport = MockTransport::from_frames(&[cmd]);
        run_handle_client(&mut transport).await;

        assert_eq!(
            transport.written.len(),
            1,
            "response should be written before returning"
        );
        let resp = core::str::from_utf8(&transport.written[0]).unwrap();
        assert!(
            resp.contains("\"id\":\"r1\""),
            "response missing id: {resp}"
        );
        assert!(resp.contains("\"ok\":true"), "expected ok:true: {resp}");
    });
}

// ----- Minimal pollster -----

/// Minimal single-threaded async block_on for host tests.
fn pollster_block<F: core::future::Future<Output = T>, T>(f: F) -> T {
    // All MockTransport futures resolve immediately (no real I/O), so a trivial
    // executor suffices. We use a simple spin-poll approach.
    let mut f = core::pin::pin!(f);
    let waker = noop_waker();
    let mut cx = core::task::Context::from_waker(&waker);
    loop {
        match f.as_mut().poll(&mut cx) {
            core::task::Poll::Ready(val) => return val,
            core::task::Poll::Pending => continue,
        }
    }
}

fn noop_waker() -> core::task::Waker {
    use core::task::{RawWaker, RawWakerVTable};
    const VTABLE: RawWakerVTable =
        RawWakerVTable::new(|p| RawWaker::new(p, &VTABLE), |_| {}, |_| {}, |_| {});
    // SAFETY: the vtable is valid and the data pointer is never dereferenced.
    unsafe { core::task::Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}
