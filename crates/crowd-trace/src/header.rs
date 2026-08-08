//! Fixed-size trace header.

use crate::TraceError;

/// File magic. The trailing digit is part of the magic, not the version:
/// a v1 format would use a new magic so a v0 reader cannot even open it.
pub const MAGIC: [u8; 8] = *b"CRWDTRC0";

/// Bumped whenever the record layout changes. Mismatches are a hard error.
pub const FORMAT_VERSION: u32 = 0;

/// Header size in bytes. 8 magic + 4 version + 8 ticks + 4 agents
/// + 4 rate + 4 scale = 32.
pub const HEADER_BYTES: usize = 32;

/// Everything a reader needs before it can interpret a single record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub tick_count: u64,
    pub agent_count: u32,
    pub ticks_per_second: u32,
    pub world_to_meter: f32,
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_BYTES] {
        let mut out = [0u8; HEADER_BYTES];
        out[0..8].copy_from_slice(&MAGIC);
        out[8..12].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        out[12..20].copy_from_slice(&self.tick_count.to_le_bytes());
        out[20..24].copy_from_slice(&self.agent_count.to_le_bytes());
        out[24..28].copy_from_slice(&self.ticks_per_second.to_le_bytes());
        out[28..32].copy_from_slice(&self.world_to_meter.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Header, TraceError> {
        if bytes.len() < HEADER_BYTES {
            return Err(TraceError::Truncated {
                expected: HEADER_BYTES,
                found: bytes.len(),
            });
        }
        if bytes[0..8] != MAGIC {
            return Err(TraceError::BadMagic);
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(TraceError::UnsupportedVersion {
                found: version,
                expected: FORMAT_VERSION,
            });
        }
        Ok(Header {
            tick_count: u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
            agent_count: u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            ticks_per_second: u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            world_to_meter: f32::from_le_bytes(bytes[28..32].try_into().unwrap()),
        })
    }
}
