pub mod codec;
pub mod commands;
pub mod error;
pub mod events;
pub mod protocol;
pub mod ring_buffer;

// Re-export common symbols for convenience
pub use codec::ProtocolCodec;
pub use commands::{GuestCommand, GuestResponse};
pub use error::ProtocolError;
pub use events::{
    ButtonAction, FileOp, FileSystemEvent, ImageFormat, MemoryQueryType, MouseButton, NetProtocol,
    NetworkEvent, ProcessEvent, ProcessEventType, RegistryEvent, RegistryOp, TelemetryEvent,
    TelemetrySource, UiElementNode,
};
pub use protocol::{
    Frame, FrameHeader, FLAG_COMPRESSED, FLAG_ERROR, FLAG_URGENT, HEADER_SIZE, MAX_PAYLOAD_SIZE,
    PROTOCOL_MAGIC, PROTOCOL_VERSION,
};
pub use ring_buffer::RingBuffer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_header_roundtrip() {
        let payload = b"Hello, OmniScope Host!";
        let sequence_id = 1042;
        let trace_id = 0xDEADBEEFCAFE;
        let flags = FLAG_URGENT;

        let header = FrameHeader::new(sequence_id, trace_id, flags, payload)
            .expect("Header creation should succeed");

        assert_eq!(header.magic, PROTOCOL_MAGIC);
        assert_eq!(header.version, PROTOCOL_VERSION);
        assert_eq!(header.sequence_id, sequence_id);
        assert_eq!(header.trace_id, trace_id);
        assert_eq!(header.payload_len, payload.len() as u32);
        assert_eq!(header.flags, flags);

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), HEADER_SIZE);

        let decoded = FrameHeader::from_bytes(&bytes)
            .expect("Decoding header should succeed");

        assert_eq!(header, decoded);
    }

    #[test]
    fn test_frame_header_invalid_magic() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(b"BAD!");
        let err = FrameHeader::from_bytes(&bytes).unwrap_err();
        match err {
            ProtocolError::InvalidMagic(m) => assert_eq!(&m, b"BAD!"),
            other => panic!("Expected InvalidMagic, got {:?}", other),
        }
    }

    #[test]
    fn test_frame_header_unsupported_version() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&PROTOCOL_MAGIC);
        bytes[4] = 99; // Invalid version
        let err = FrameHeader::from_bytes(&bytes).unwrap_err();
        match err {
            ProtocolError::UnsupportedVersion(v) => assert_eq!(v, 99),
            other => panic!("Expected UnsupportedVersion, got {:?}", other),
        }
    }

    #[test]
    fn test_command_frame_roundtrip() {
        let cmd = GuestCommand::LaunchProcess {
            executable_path: "C:\\TargetApp\\run.exe".to_string(),
            arguments: vec!["--debug".to_string(), "--headless".to_string()],
            working_dir: Some("C:\\TargetApp".to_string()),
            suspend_on_entry: true,
        };

        let frame = Frame::new_command(42, 1001, &cmd).expect("Frame creation should succeed");
        let encoded_bytes = frame.encode_to_bytes();

        let decoded_frame = Frame::decode_from_bytes(&encoded_bytes)
            .expect("Decoding complete frame should succeed");

        assert_eq!(decoded_frame.header.sequence_id, 42);
        assert_eq!(decoded_frame.header.trace_id, 1001);

        let decoded_cmd = decoded_frame.decode_command()
            .expect("Decoding command payload should succeed");

        assert_eq!(cmd, decoded_cmd);
    }

    #[test]
    fn test_response_frame_roundtrip() {
        let resp = GuestResponse::TelemetrySlice {
            source: TelemetrySource::FileSystem,
            events: vec![
                TelemetryEvent::FileSystem(FileSystemEvent {
                    timestamp_ns: 123456789,
                    pid: 5040,
                    operation: FileOp::Write,
                    file_path: "C:\\Users\\Test\\AppData\\Local\\config.json".to_string(),
                    bytes_transferred: Some(512),
                }),
            ],
            total_captured: 1,
            dropped_events: 0,
        };

        let frame = Frame::new_response(77, 2002, &resp).expect("Frame creation should succeed");
        let encoded_bytes = frame.encode_to_bytes();

        let decoded_frame = Frame::decode_from_bytes(&encoded_bytes)
            .expect("Decoding complete frame should succeed");

        let decoded_resp = decoded_frame.decode_response()
            .expect("Decoding response payload should succeed");

        assert_eq!(resp, decoded_resp);
    }

    #[test]
    fn test_corrupted_crc_detection() {
        let cmd = GuestCommand::Ping { timestamp_ms: 55555 };
        let frame = Frame::new_command(1, 1, &cmd).unwrap();
        let mut encoded = frame.encode_to_bytes();

        // Corrupt a byte in the payload
        let last_idx = encoded.len() - 1;
        encoded[last_idx] ^= 0xFF;

        let err = Frame::decode_from_bytes(&encoded).unwrap_err();
        match err {
            ProtocolError::CrcMismatch { .. } => (), // Success: detected corruption
            other => panic!("Expected CrcMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_streaming_decode_from_slice() {
        let cmd1 = GuestCommand::Ping { timestamp_ms: 100 };
        let cmd2 = GuestCommand::TerminateProcess { pid: 999 };

        let frame1 = Frame::new_command(1, 10, &cmd1).unwrap();
        let frame2 = Frame::new_command(2, 20, &cmd2).unwrap();

        let mut stream = frame1.encode_to_bytes();
        stream.extend_from_slice(&frame2.encode_to_bytes());

        // Decode first frame
        let (decoded1, consumed1) = Frame::decode_from_slice(&stream).expect("First frame decode");
        assert_eq!(decoded1.decode_command().unwrap(), cmd1);

        // Decode second frame from remaining slice
        let (decoded2, consumed2) = Frame::decode_from_slice(&stream[consumed1..]).expect("Second frame decode");
        assert_eq!(decoded2.decode_command().unwrap(), cmd2);
        assert_eq!(consumed1 + consumed2, stream.len());
    }

    #[test]
    fn test_incomplete_frame_handling() {
        let cmd = GuestCommand::Ping { timestamp_ms: 1234 };
        let frame = Frame::new_command(1, 1, &cmd).unwrap();
        let encoded = frame.encode_to_bytes();

        // Truncate to half of header
        let partial_header = &encoded[..16];
        match Frame::decode_from_slice(partial_header) {
            Err(ProtocolError::IncompleteFrame { needed, available }) => {
                assert_eq!(needed, HEADER_SIZE);
                assert_eq!(available, 16);
            }
            other => panic!("Expected IncompleteFrame, got {:?}", other),
        }

        // Truncate halfway through payload
        let partial_payload = &encoded[..HEADER_SIZE + 2];
        match Frame::decode_from_slice(partial_payload) {
            Err(ProtocolError::IncompleteFrame { needed, available }) => {
                assert_eq!(needed, encoded.len());
                assert_eq!(available, HEADER_SIZE + 2);
            }
            other => panic!("Expected IncompleteFrame, got {:?}", other),
        }
    }

    #[test]
    fn test_ring_buffer_overwrite_and_drop_tracking() {
        let ring = RingBuffer::<u32>::new(3);

        ring.push_overwrite(10);
        ring.push_overwrite(20);
        ring.push_overwrite(30);

        assert_eq!(ring.len(), 3);
        assert_eq!(ring.total_pushed(), 3);
        assert_eq!(ring.dropped_count(), 0);
        assert_eq!(ring.get_latest(5), vec![10, 20, 30]);

        // Push 4th element: 10 should be dropped
        ring.push_overwrite(40);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.total_pushed(), 4);
        assert_eq!(ring.dropped_count(), 1);
        assert_eq!(ring.get_latest(5), vec![20, 30, 40]);

        // Push 5th element: 20 should be dropped
        ring.push_overwrite(50);
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.total_pushed(), 5);
        assert_eq!(ring.dropped_count(), 2);
        assert_eq!(ring.get_latest(5), vec![30, 40, 50]);
    }

    #[test]
    fn test_ring_buffer_selective_slice_query() {
        let ring = RingBuffer::<ProcessEvent>::new(10);

        // Simulate events for PID 100 and PID 200
        for i in 1..=5 {
            ring.push_overwrite(ProcessEvent {
                timestamp_ns: i * 1000,
                event_type: ProcessEventType::ThreadCreate,
                pid: 100,
                parent_pid: None,
                image_name: "target.exe".to_string(),
                command_line: None,
                dll_path: None,
            });
            ring.push_overwrite(ProcessEvent {
                timestamp_ns: i * 1000 + 500,
                event_type: ProcessEventType::ThreadCreate,
                pid: 200,
                parent_pid: None,
                image_name: "other.exe".to_string(),
                command_line: None,
                dll_path: None,
            });
        }

        // Query only events for PID 100, limit 3
        let pid100_events = ring.get_slice(|e| e.pid == 100, 3);
        assert_eq!(pid100_events.len(), 3);
        for e in &pid100_events {
            assert_eq!(e.pid, 100);
        }
        // Verify chronological ordering
        assert!(pid100_events[0].timestamp_ns < pid100_events[1].timestamp_ns);
        assert!(pid100_events[1].timestamp_ns < pid100_events[2].timestamp_ns);
    }
}
