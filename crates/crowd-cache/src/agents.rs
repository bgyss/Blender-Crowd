//! Static per-agent table stored once per cache.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::payload_checksum;

const AGENT_MAGIC: [u8; 8] = *b"BCAGT\0\x01\0";
const AGENT_VERSION: u16 = 1;
const LITTLE_ENDIAN: u8 = 1;
const HEADER_BYTES: usize = 32;
const RECORD_BYTES: usize = 28;

#[derive(Clone, Debug, PartialEq)]
pub struct AgentStatic {
    pub agent_id: u64,
    pub population_id: u32,
    pub archetype_id: u32,
    pub variant_id: u32,
    pub base_scale: f32,
    pub spawn_ordinal: u32,
}

pub(crate) fn encode_agents(agents: &[AgentStatic]) -> Result<Vec<u8>, AgentTableError> {
    let count = u32::try_from(agents.len()).map_err(|_| AgentTableError::LengthOverflow)?;
    let mut seen = HashSet::with_capacity(agents.len());
    let payload_len = agents
        .len()
        .checked_mul(RECORD_BYTES)
        .ok_or(AgentTableError::LengthOverflow)?;
    let mut payload = Vec::with_capacity(payload_len);
    for agent in agents {
        if !seen.insert(agent.agent_id) {
            return Err(AgentTableError::DuplicateAgentId(agent.agent_id));
        }
        if !agent.base_scale.is_finite() {
            return Err(AgentTableError::NonFiniteScale(agent.agent_id));
        }
        payload.extend_from_slice(&agent.agent_id.to_le_bytes());
        payload.extend_from_slice(&agent.population_id.to_le_bytes());
        payload.extend_from_slice(&agent.archetype_id.to_le_bytes());
        payload.extend_from_slice(&agent.variant_id.to_le_bytes());
        payload.extend_from_slice(&agent.base_scale.to_le_bytes());
        payload.extend_from_slice(&agent.spawn_ordinal.to_le_bytes());
    }
    let checksum = payload_checksum(&payload);
    let mut bytes = Vec::with_capacity(HEADER_BYTES + payload.len());
    bytes.extend_from_slice(&AGENT_MAGIC);
    bytes.push(LITTLE_ENDIAN);
    bytes.push(0);
    bytes.extend_from_slice(&AGENT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&(payload_len as u64).to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(bytes.len(), HEADER_BYTES);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub(crate) fn decode_agents(bytes: &[u8]) -> Result<Vec<AgentStatic>, AgentTableError> {
    if bytes.len() < HEADER_BYTES {
        return Err(AgentTableError::Truncated);
    }
    if bytes[0..8] != AGENT_MAGIC {
        return Err(AgentTableError::BadMagic);
    }
    if bytes[8] != LITTLE_ENDIAN {
        return Err(AgentTableError::WrongEndian(bytes[8]));
    }
    let version = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
    if version != AGENT_VERSION {
        return Err(AgentTableError::UnsupportedVersion(version));
    }
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let declared_len_u64 = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
    let declared_len =
        usize::try_from(declared_len_u64).map_err(|_| AgentTableError::LengthOverflow)?;
    let expected_len = count
        .checked_mul(RECORD_BYTES)
        .ok_or(AgentTableError::LengthOverflow)?;
    let payload = &bytes[HEADER_BYTES..];
    if declared_len != expected_len || payload.len() != expected_len {
        return Err(AgentTableError::LengthMismatch);
    }
    let expected_checksum = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let found_checksum = payload_checksum(payload);
    if found_checksum != expected_checksum {
        return Err(AgentTableError::ChecksumMismatch);
    }

    let mut agents = Vec::with_capacity(count);
    let mut seen = HashSet::with_capacity(count);
    for record in payload.chunks_exact(RECORD_BYTES) {
        let agent = AgentStatic {
            agent_id: u64::from_le_bytes(record[0..8].try_into().unwrap()),
            population_id: u32::from_le_bytes(record[8..12].try_into().unwrap()),
            archetype_id: u32::from_le_bytes(record[12..16].try_into().unwrap()),
            variant_id: u32::from_le_bytes(record[16..20].try_into().unwrap()),
            base_scale: f32::from_le_bytes(record[20..24].try_into().unwrap()),
            spawn_ordinal: u32::from_le_bytes(record[24..28].try_into().unwrap()),
        };
        if !agent.base_scale.is_finite() {
            return Err(AgentTableError::NonFiniteScale(agent.agent_id));
        }
        if !seen.insert(agent.agent_id) {
            return Err(AgentTableError::DuplicateAgentId(agent.agent_id));
        }
        agents.push(agent);
    }
    Ok(agents)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentTableError {
    BadMagic,
    WrongEndian(u8),
    UnsupportedVersion(u16),
    Truncated,
    LengthOverflow,
    LengthMismatch,
    ChecksumMismatch,
    DuplicateAgentId(u64),
    NonFiniteScale(u64),
}

impl fmt::Display for AgentTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for AgentTableError {}
