use crowd_cache::{
    content_hash, decode_chunk, encode_chunk, payload_checksum, CodecError, Frame, FrameRecord,
    PositionEncoding, CHUNK_HEADER_BYTES,
};
use proptest::prelude::*;

fn record(agent_id: u64, position: [f32; 2]) -> FrameRecord {
    FrameRecord {
        agent_id,
        position,
        orientation: 0.25,
        scale: 1.05,
        population_id: 3,
        variant_id: 7,
        clip_id: 2,
        phase: 0.75,
        playback_rate: 1.1,
        behavior_state: 4,
        decision_reason: 9,
        destination_id: 11,
        velocity: [1.25, -0.5],
        visible: true,
        render_tier: 1,
    }
}

#[test]
fn crc32c_matches_the_standard_check_value() {
    assert_eq!(payload_checksum(b"123456789"), 0xe306_9283);
}

#[test]
fn millimeter_positions_round_trip_with_half_millimeter_error() {
    let frames = vec![Frame {
        records: vec![record(7, [12.3454, -8.7656])],
    }];

    let encoded = encode_chunk(4, &frames, PositionEncoding::MillimeterI32).unwrap();
    let decoded = decode_chunk(&encoded.bytes).unwrap();
    let got = &decoded.frames[0].records[0];

    assert_eq!(got.agent_id, 7);
    assert!((got.position[0] - 12.3454).abs() <= 0.0005);
    assert!((got.position[1] + 8.7656).abs() <= 0.0005);
    assert_eq!(encoded.position_error_bound, 0.0005);
}

#[test]
fn a_payload_bit_flip_is_rejected() {
    let frames = vec![Frame {
        records: vec![record(1, [1.0, 2.0])],
    }];
    let mut encoded = encode_chunk(0, &frames, PositionEncoding::F32).unwrap();
    *encoded.bytes.last_mut().unwrap() ^= 1;

    assert!(matches!(
        decode_chunk(&encoded.bytes),
        Err(CodecError::ChecksumMismatch { .. })
    ));
}

#[test]
fn affine_encoding_preserves_a_constant_position_axis() {
    let frames = vec![Frame {
        records: vec![record(1, [5.0, -3.0]), record(2, [5.0, -3.0])],
    }];

    let encoded = encode_chunk(0, &frames, PositionEncoding::AffineI16).unwrap();
    let decoded = decode_chunk(&encoded.bytes).unwrap();

    assert_eq!(encoded.position_error_bound, 0.0);
    assert_eq!(decoded.frames[0].records[0].position, [5.0, -3.0]);
    assert_eq!(decoded.frames[0].records[1].position, [5.0, -3.0]);
}

#[test]
fn every_discrete_and_continuous_channel_round_trips_in_f32_mode() {
    let expected = record(u64::MAX - 1, [-4.5, 8.25]);
    let frames = vec![Frame {
        records: vec![expected.clone()],
    }];

    let decoded = decode_chunk(
        &encode_chunk(17, &frames, PositionEncoding::F32)
            .unwrap()
            .bytes,
    )
    .unwrap();

    assert_eq!(decoded.tick_start, 17);
    assert_eq!(decoded.frames[0].records[0], expected);
}

#[test]
fn non_finite_values_are_rejected_before_encoding() {
    let mut bad = record(1, [f32::NAN, 0.0]);
    let frames = vec![Frame {
        records: vec![bad.clone()],
    }];
    assert_eq!(
        encode_chunk(0, &frames, PositionEncoding::F32),
        Err(CodecError::NonFiniteValue)
    );

    bad.position = [0.0, 0.0];
    bad.velocity[1] = f32::INFINITY;
    let frames = vec![Frame { records: vec![bad] }];
    assert_eq!(
        encode_chunk(0, &frames, PositionEncoding::F32),
        Err(CodecError::NonFiniteValue)
    );
}

#[test]
fn frames_with_different_agent_counts_are_rejected() {
    let frames = vec![
        Frame {
            records: vec![record(1, [0.0, 0.0])],
        },
        Frame {
            records: vec![record(1, [0.0, 0.0]), record(2, [1.0, 0.0])],
        },
    ];

    assert!(matches!(
        encode_chunk(0, &frames, PositionEncoding::F32),
        Err(CodecError::InconsistentAgentCount {
            frame: 1,
            expected: 1,
            found: 2
        })
    ));
}

#[test]
fn malformed_headers_are_rejected_before_payload_decode() {
    let frames = vec![Frame {
        records: vec![record(1, [0.0, 0.0])],
    }];
    let encoded = encode_chunk(0, &frames, PositionEncoding::F32).unwrap();

    assert!(matches!(
        decode_chunk(&encoded.bytes[..CHUNK_HEADER_BYTES - 1]),
        Err(CodecError::Truncated { .. })
    ));

    let mut wrong_magic = encoded.bytes.clone();
    wrong_magic[0] = b'X';
    assert_eq!(decode_chunk(&wrong_magic), Err(CodecError::BadMagic));

    let mut wrong_length = encoded.bytes;
    wrong_length[36..44].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        decode_chunk(&wrong_length),
        Err(CodecError::LengthOverflow | CodecError::PayloadLengthMismatch { .. })
    ));
}

#[test]
fn blake3_content_hash_matches_the_standard_empty_digest() {
    assert_eq!(
        hex(&content_hash(b"")),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn position_codecs_respect_their_declared_error_bound(
        positions in prop::collection::vec((-10_000.0f32..10_000.0, -10_000.0f32..10_000.0), 1..32)
    ) {
        let records: Vec<_> = positions
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| record(index as u64 + 1, [x, y]))
            .collect();
        let frames = vec![Frame { records }];

        for encoding in [
            PositionEncoding::F32,
            PositionEncoding::MillimeterI32,
            PositionEncoding::AffineI16,
        ] {
            let encoded = encode_chunk(0, &frames, encoding).unwrap();
            let decoded = decode_chunk(&encoded.bytes).unwrap();
            for (source, restored) in frames[0].records.iter().zip(&decoded.frames[0].records) {
                prop_assert_eq!(source.agent_id, restored.agent_id);
                for axis in 0..2 {
                    let error = (source.position[axis] - restored.position[axis]).abs();
                    prop_assert!(
                        error <= encoded.position_error_bound + 0.001,
                        "{encoding:?} axis {axis} error {error} exceeded {}",
                        encoded.position_error_bound
                    );
                }
            }
        }
    }
}
