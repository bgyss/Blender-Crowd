//! Checksummed, channel-major cache frame chunks.

use std::error::Error;
use std::fmt;

use crate::payload_checksum;

const CHUNK_MAGIC: [u8; 8] = *b"BCFRM\0\x01\0";
const LITTLE_ENDIAN: u8 = 1;
const CHUNK_VERSION: u16 = 1;
pub const CHUNK_HEADER_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PositionEncoding {
    F32 = 0,
    MillimeterI32 = 1,
    AffineI16 = 2,
}

impl TryFrom<u8> for PositionEncoding {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::MillimeterI32),
            2 => Ok(Self::AffineI16),
            other => Err(CodecError::InvalidEncoding(other)),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameRecord {
    pub agent_id: u64,
    pub position: [f32; 2],
    pub orientation: f32,
    pub scale: f32,
    pub population_id: u32,
    pub variant_id: u32,
    pub clip_id: u16,
    pub phase: f32,
    pub playback_rate: f32,
    pub behavior_state: u16,
    pub decision_reason: u16,
    pub destination_id: u32,
    pub velocity: [f32; 2],
    pub visible: bool,
    pub render_tier: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub records: Vec<FrameRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodedChunk {
    pub bytes: Vec<u8>,
    pub tick_start: u64,
    pub tick_count: u32,
    pub agent_count: u32,
    pub position_encoding: PositionEncoding,
    pub position_error_bound: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedChunk {
    pub tick_start: u64,
    pub position_encoding: PositionEncoding,
    pub position_error_bound: f32,
    pub frames: Vec<Frame>,
}

pub fn encode_chunk(
    tick_start: u64,
    frames: &[Frame],
    position_encoding: PositionEncoding,
) -> Result<EncodedChunk, CodecError> {
    let tick_count = u32::try_from(frames.len()).map_err(|_| CodecError::LengthOverflow)?;
    let agent_count = frames.first().map_or(0, |frame| {
        u32::try_from(frame.records.len()).unwrap_or(u32::MAX)
    });
    if agent_count == u32::MAX {
        return Err(CodecError::LengthOverflow);
    }
    for (index, frame) in frames.iter().enumerate() {
        if frame.records.len() != agent_count as usize {
            return Err(CodecError::InconsistentAgentCount {
                frame: index,
                expected: agent_count as usize,
                found: frame.records.len(),
            });
        }
    }

    let record_count_u64 = u64::from(tick_count)
        .checked_mul(u64::from(agent_count))
        .ok_or(CodecError::LengthOverflow)?;
    let record_count = usize::try_from(record_count_u64).map_err(|_| CodecError::LengthOverflow)?;
    let records: Vec<&FrameRecord> = frames
        .iter()
        .flat_map(|frame| frame.records.iter())
        .collect();
    debug_assert_eq!(records.len(), record_count);
    validate_records(&records)?;

    let (origin, scale, position_error_bound) = position_metadata(&records, position_encoding)?;
    let position_bytes = match position_encoding {
        PositionEncoding::F32 | PositionEncoding::MillimeterI32 => 8,
        PositionEncoding::AffineI16 => 4,
    };
    let payload_capacity = record_count
        .checked_mul(52 + position_bytes)
        .ok_or(CodecError::LengthOverflow)?;
    let mut payload = Vec::with_capacity(payload_capacity);

    for record in &records {
        payload.extend_from_slice(&record.agent_id.to_le_bytes());
    }
    match position_encoding {
        PositionEncoding::F32 => {
            for record in &records {
                payload.extend_from_slice(&record.position[0].to_le_bytes());
                payload.extend_from_slice(&record.position[1].to_le_bytes());
            }
        }
        PositionEncoding::MillimeterI32 => {
            for record in &records {
                for component in record.position {
                    let millimeters = (component * 1_000.0).round();
                    if millimeters < i32::MIN as f32 || millimeters > i32::MAX as f32 {
                        return Err(CodecError::PositionOutOfRange(component));
                    }
                    payload.extend_from_slice(&(millimeters as i32).to_le_bytes());
                }
            }
        }
        PositionEncoding::AffineI16 => {
            for record in &records {
                for axis in 0..2 {
                    let quantized = if scale[axis] == 0.0 {
                        -32_767
                    } else {
                        let unsigned = ((record.position[axis] - origin[axis]) / scale[axis])
                            .round()
                            .clamp(0.0, 65_534.0) as i32;
                        unsigned - 32_767
                    };
                    payload.extend_from_slice(&(quantized as i16).to_le_bytes());
                }
            }
        }
    }
    for record in &records {
        payload.extend_from_slice(&record.orientation.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.scale.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.population_id.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.variant_id.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.clip_id.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.phase.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.playback_rate.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.behavior_state.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.decision_reason.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.destination_id.to_le_bytes());
    }
    for record in &records {
        payload.extend_from_slice(&record.velocity[0].to_le_bytes());
        payload.extend_from_slice(&record.velocity[1].to_le_bytes());
    }
    for record in &records {
        payload.push(u8::from(record.visible));
    }
    for record in &records {
        payload.push(record.render_tier);
    }

    let payload_len = u64::try_from(payload.len()).map_err(|_| CodecError::LengthOverflow)?;
    let checksum = payload_checksum(&payload);
    let mut bytes = Vec::with_capacity(CHUNK_HEADER_BYTES + payload.len());
    bytes.extend_from_slice(&CHUNK_MAGIC);
    bytes.push(LITTLE_ENDIAN);
    bytes.push(position_encoding as u8);
    bytes.extend_from_slice(&CHUNK_VERSION.to_le_bytes());
    bytes.extend_from_slice(&tick_start.to_le_bytes());
    bytes.extend_from_slice(&tick_count.to_le_bytes());
    bytes.extend_from_slice(&agent_count.to_le_bytes());
    bytes.extend_from_slice(&record_count_u64.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(&origin[0].to_le_bytes());
    bytes.extend_from_slice(&origin[1].to_le_bytes());
    bytes.extend_from_slice(&scale[0].to_le_bytes());
    bytes.extend_from_slice(&scale[1].to_le_bytes());
    debug_assert_eq!(bytes.len(), CHUNK_HEADER_BYTES);
    bytes.extend_from_slice(&payload);

    Ok(EncodedChunk {
        bytes,
        tick_start,
        tick_count,
        agent_count,
        position_encoding,
        position_error_bound,
    })
}

pub fn decode_chunk(bytes: &[u8]) -> Result<DecodedChunk, CodecError> {
    if bytes.len() < CHUNK_HEADER_BYTES {
        return Err(CodecError::Truncated {
            expected: CHUNK_HEADER_BYTES,
            found: bytes.len(),
        });
    }
    if bytes[0..8] != CHUNK_MAGIC {
        return Err(CodecError::BadMagic);
    }
    if bytes[8] != LITTLE_ENDIAN {
        return Err(CodecError::WrongEndian(bytes[8]));
    }
    let position_encoding = PositionEncoding::try_from(bytes[9])?;
    let version = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
    if version != CHUNK_VERSION {
        return Err(CodecError::UnsupportedVersion(version));
    }
    let tick_start = u64::from_le_bytes(bytes[12..20].try_into().unwrap());
    let tick_count = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    let agent_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let record_count_u64 = u64::from_le_bytes(bytes[28..36].try_into().unwrap());
    let expected_records = u64::from(tick_count)
        .checked_mul(u64::from(agent_count))
        .ok_or(CodecError::LengthOverflow)?;
    if record_count_u64 != expected_records {
        return Err(CodecError::RecordCountMismatch {
            declared: record_count_u64,
            expected: expected_records,
        });
    }
    let record_count = usize::try_from(record_count_u64).map_err(|_| CodecError::LengthOverflow)?;
    let payload_len_u64 = u64::from_le_bytes(bytes[36..44].try_into().unwrap());
    let payload_len = usize::try_from(payload_len_u64).map_err(|_| CodecError::LengthOverflow)?;
    let actual_payload_len = bytes.len() - CHUNK_HEADER_BYTES;
    if payload_len != actual_payload_len {
        return Err(CodecError::PayloadLengthMismatch {
            declared: payload_len,
            actual: actual_payload_len,
        });
    }
    let expected_checksum = u32::from_le_bytes(bytes[44..48].try_into().unwrap());
    let payload = &bytes[CHUNK_HEADER_BYTES..];
    let found_checksum = payload_checksum(payload);
    if found_checksum != expected_checksum {
        return Err(CodecError::ChecksumMismatch {
            expected: expected_checksum,
            found: found_checksum,
        });
    }
    let origin = [
        f32::from_le_bytes(bytes[48..52].try_into().unwrap()),
        f32::from_le_bytes(bytes[52..56].try_into().unwrap()),
    ];
    let scale = [
        f32::from_le_bytes(bytes[56..60].try_into().unwrap()),
        f32::from_le_bytes(bytes[60..64].try_into().unwrap()),
    ];
    if origin
        .iter()
        .chain(scale.iter())
        .any(|value| !value.is_finite())
        || scale.iter().any(|value| *value < 0.0)
    {
        return Err(CodecError::InvalidQuantizationMetadata);
    }

    let mut records = vec![FrameRecord::default(); record_count];
    let mut cursor = 0usize;
    for record in &mut records {
        record.agent_id = read_u64(payload, &mut cursor)?;
    }
    match position_encoding {
        PositionEncoding::F32 => {
            for record in &mut records {
                record.position = [
                    read_f32(payload, &mut cursor)?,
                    read_f32(payload, &mut cursor)?,
                ];
            }
        }
        PositionEncoding::MillimeterI32 => {
            for record in &mut records {
                record.position = [
                    read_i32(payload, &mut cursor)? as f32 / 1_000.0,
                    read_i32(payload, &mut cursor)? as f32 / 1_000.0,
                ];
            }
        }
        PositionEncoding::AffineI16 => {
            for record in &mut records {
                for axis in 0..2 {
                    let signed = read_i16(payload, &mut cursor)? as i32;
                    record.position[axis] = origin[axis] + (signed + 32_767) as f32 * scale[axis];
                }
            }
        }
    }
    for record in &mut records {
        record.orientation = read_f32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.scale = read_f32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.population_id = read_u32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.variant_id = read_u32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.clip_id = read_u16(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.phase = read_f32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.playback_rate = read_f32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.behavior_state = read_u16(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.decision_reason = read_u16(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.destination_id = read_u32(payload, &mut cursor)?;
    }
    for record in &mut records {
        record.velocity = [
            read_f32(payload, &mut cursor)?,
            read_f32(payload, &mut cursor)?,
        ];
    }
    for record in &mut records {
        record.visible = match read_u8(payload, &mut cursor)? {
            0 => false,
            1 => true,
            value => return Err(CodecError::InvalidBoolean(value)),
        };
    }
    for record in &mut records {
        record.render_tier = read_u8(payload, &mut cursor)?;
    }
    if cursor != payload.len() {
        return Err(CodecError::TrailingPayloadBytes(payload.len() - cursor));
    }
    validate_records(&records.iter().collect::<Vec<_>>())?;

    let mut frames = Vec::with_capacity(tick_count as usize);
    if agent_count == 0 {
        frames.resize_with(tick_count as usize, Frame::default);
    } else {
        for chunk in records.chunks_exact(agent_count as usize) {
            frames.push(Frame {
                records: chunk.to_vec(),
            });
        }
    }
    let position_error_bound = match position_encoding {
        PositionEncoding::F32 => 0.0,
        PositionEncoding::MillimeterI32 => 0.0005,
        PositionEncoding::AffineI16 => scale[0].max(scale[1]) * 0.5,
    };

    Ok(DecodedChunk {
        tick_start,
        position_encoding,
        position_error_bound,
        frames,
    })
}

fn validate_records(records: &[&FrameRecord]) -> Result<(), CodecError> {
    for record in records {
        let finite = record
            .position
            .iter()
            .chain(record.velocity.iter())
            .chain([
                &record.orientation,
                &record.scale,
                &record.phase,
                &record.playback_rate,
            ])
            .all(|value| value.is_finite());
        if !finite {
            return Err(CodecError::NonFiniteValue);
        }
    }
    Ok(())
}

fn position_metadata(
    records: &[&FrameRecord],
    encoding: PositionEncoding,
) -> Result<([f32; 2], [f32; 2], f32), CodecError> {
    if encoding != PositionEncoding::AffineI16 || records.is_empty() {
        let error = if encoding == PositionEncoding::MillimeterI32 {
            0.0005
        } else {
            0.0
        };
        return Ok(([0.0, 0.0], [1.0, 1.0], error));
    }
    let mut min = records[0].position;
    let mut max = records[0].position;
    for record in &records[1..] {
        for axis in 0..2 {
            min[axis] = min[axis].min(record.position[axis]);
            max[axis] = max[axis].max(record.position[axis]);
        }
    }
    let mut scale = [0.0f32; 2];
    for axis in 0..2 {
        let span = max[axis] - min[axis];
        if span > 0.0 {
            scale[axis] = span / 65_534.0;
        }
    }
    Ok((min, scale, scale[0].max(scale[1]) * 0.5))
}

fn read_bytes<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], CodecError> {
    let end = cursor.checked_add(N).ok_or(CodecError::LengthOverflow)?;
    let slice = bytes.get(*cursor..end).ok_or(CodecError::Truncated {
        expected: end,
        found: bytes.len(),
    })?;
    *cursor = end;
    Ok(slice.try_into().unwrap())
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, CodecError> {
    Ok(read_bytes::<1>(bytes, cursor)?[0])
}
fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, CodecError> {
    Ok(u16::from_le_bytes(read_bytes(bytes, cursor)?))
}
fn read_i16(bytes: &[u8], cursor: &mut usize) -> Result<i16, CodecError> {
    Ok(i16::from_le_bytes(read_bytes(bytes, cursor)?))
}
fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, CodecError> {
    Ok(u32::from_le_bytes(read_bytes(bytes, cursor)?))
}
fn read_i32(bytes: &[u8], cursor: &mut usize) -> Result<i32, CodecError> {
    Ok(i32::from_le_bytes(read_bytes(bytes, cursor)?))
}
fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, CodecError> {
    Ok(u64::from_le_bytes(read_bytes(bytes, cursor)?))
}
fn read_f32(bytes: &[u8], cursor: &mut usize) -> Result<f32, CodecError> {
    Ok(f32::from_le_bytes(read_bytes(bytes, cursor)?))
}

#[derive(Clone, Debug, PartialEq)]
pub enum CodecError {
    BadMagic,
    UnsupportedVersion(u16),
    WrongEndian(u8),
    InvalidEncoding(u8),
    InconsistentAgentCount {
        frame: usize,
        expected: usize,
        found: usize,
    },
    RecordCountMismatch {
        declared: u64,
        expected: u64,
    },
    PayloadLengthMismatch {
        declared: usize,
        actual: usize,
    },
    ChecksumMismatch {
        expected: u32,
        found: u32,
    },
    Truncated {
        expected: usize,
        found: usize,
    },
    LengthOverflow,
    NonFiniteValue,
    PositionOutOfRange(f32),
    InvalidQuantizationMetadata,
    InvalidBoolean(u8),
    TrailingPayloadBytes(usize),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "bad cache chunk magic"),
            Self::UnsupportedVersion(version) => write!(f, "unsupported chunk version {version}"),
            Self::WrongEndian(marker) => write!(f, "unsupported endian marker {marker}"),
            Self::InvalidEncoding(value) => write!(f, "invalid position encoding {value}"),
            Self::InconsistentAgentCount {
                frame,
                expected,
                found,
            } => write!(f, "frame {frame} has {found} agents; expected {expected}"),
            Self::RecordCountMismatch { declared, expected } => {
                write!(f, "chunk declares {declared} records; expected {expected}")
            }
            Self::PayloadLengthMismatch { declared, actual } => {
                write!(f, "chunk declares {declared} payload bytes; found {actual}")
            }
            Self::ChecksumMismatch { expected, found } => {
                write!(f, "chunk checksum {found:#010x}; expected {expected:#010x}")
            }
            Self::Truncated { expected, found } => {
                write!(
                    f,
                    "truncated chunk: expected {expected} bytes, found {found}"
                )
            }
            Self::LengthOverflow => write!(f, "cache chunk length overflow"),
            Self::NonFiniteValue => write!(f, "cache record contains a non-finite float"),
            Self::PositionOutOfRange(value) => {
                write!(f, "position {value} is outside millimeter encoding range")
            }
            Self::InvalidQuantizationMetadata => write!(f, "invalid quantization metadata"),
            Self::InvalidBoolean(value) => write!(f, "invalid encoded boolean {value}"),
            Self::TrailingPayloadBytes(count) => write!(f, "{count} trailing payload bytes"),
        }
    }
}

impl Error for CodecError {}
