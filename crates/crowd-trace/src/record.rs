//! One agent's state at one tick, packed.
//!
//! Packed rather than padded: the format is written and read by explicit
//! offset arithmetic on both sides of the FFI boundary, so a compiler's
//! alignment choices must never enter into it.

use crate::TraceError;

/// Agent is simulating.
pub const FLAG_ACTIVE: u32 = 1 << 0;
/// Agent has reached its destination.
pub const FLAG_ARRIVED: u32 = 1 << 1;

/// Packed record size: 8 id + 8 position + 4 orientation + 4 flags
/// + 2 clip + 4 phase + 4 rate + 1 tier = 35.
pub const RECORD_BYTES: usize = 35;

/// One agent at one tick.
///
/// `clip_index`, `phase`, `playback_rate`, and `render_tier` are stubs: no
/// animation system exists to populate them yet. They are carried at full
/// width anyway so the reader, the numpy buffer path, and the Geometry Nodes
/// attribute plumbing are proven for a representative mix of integer and
/// float channels while the format is still cheap to change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentRecord {
    pub agent_id: u64,
    pub position: [f32; 2],
    pub orientation: f32,
    pub flags: u32,
    pub clip_index: u16,
    pub phase: f32,
    pub playback_rate: f32,
    pub render_tier: u8,
}

impl AgentRecord {
    pub fn encode(&self) -> [u8; RECORD_BYTES] {
        let mut out = [0u8; RECORD_BYTES];
        out[0..8].copy_from_slice(&self.agent_id.to_le_bytes());
        out[8..12].copy_from_slice(&self.position[0].to_le_bytes());
        out[12..16].copy_from_slice(&self.position[1].to_le_bytes());
        out[16..20].copy_from_slice(&self.orientation.to_le_bytes());
        out[20..24].copy_from_slice(&self.flags.to_le_bytes());
        out[24..26].copy_from_slice(&self.clip_index.to_le_bytes());
        out[26..30].copy_from_slice(&self.phase.to_le_bytes());
        out[30..34].copy_from_slice(&self.playback_rate.to_le_bytes());
        out[34] = self.render_tier;
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<AgentRecord, TraceError> {
        if bytes.len() < RECORD_BYTES {
            return Err(TraceError::Truncated {
                expected: RECORD_BYTES,
                found: bytes.len(),
            });
        }
        Ok(AgentRecord {
            agent_id: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            position: [
                f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
                f32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            ],
            orientation: f32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            flags: u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            clip_index: u16::from_le_bytes(bytes[24..26].try_into().unwrap()),
            phase: f32::from_le_bytes(bytes[26..30].try_into().unwrap()),
            playback_rate: f32::from_le_bytes(bytes[30..34].try_into().unwrap()),
            render_tier: bytes[34],
        })
    }
}
