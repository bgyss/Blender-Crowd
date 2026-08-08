use crowd_trace::{Header, TraceError, FORMAT_VERSION, HEADER_BYTES};

#[test]
fn header_round_trips() {
    let h = Header {
        tick_count: 1234,
        agent_count: 1000,
        ticks_per_second: 30,
        world_to_meter: 1.0,
    };
    let bytes = h.encode();
    assert_eq!(bytes.len(), HEADER_BYTES);
    let back = Header::decode(&bytes).expect("decode");
    assert_eq!(back.tick_count, 1234);
    assert_eq!(back.agent_count, 1000);
    assert_eq!(back.ticks_per_second, 30);
    assert_eq!(back.world_to_meter, 1.0);
}

#[test]
fn header_rejects_bad_magic() {
    let mut bytes = Header {
        tick_count: 1,
        agent_count: 1,
        ticks_per_second: 30,
        world_to_meter: 1.0,
    }
    .encode();
    bytes[0] = b'X';
    assert!(matches!(Header::decode(&bytes), Err(TraceError::BadMagic)));
}

#[test]
fn header_rejects_future_version() {
    let mut bytes = Header {
        tick_count: 1,
        agent_count: 1,
        ticks_per_second: 30,
        world_to_meter: 1.0,
    }
    .encode();
    bytes[8..12].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    match Header::decode(&bytes) {
        Err(TraceError::UnsupportedVersion { found, expected }) => {
            assert_eq!(found, FORMAT_VERSION + 1);
            assert_eq!(expected, FORMAT_VERSION);
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn header_rejects_short_buffer() {
    let bytes = [0u8; 4];
    assert!(matches!(
        Header::decode(&bytes),
        Err(TraceError::Truncated { .. })
    ));
}
