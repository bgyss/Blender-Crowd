//! Cache manifest types and structural invariants.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Incomplete,
    Canceled,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarType {
    F32,
    I32,
    U8,
    U16,
    U32,
    U64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelDef {
    pub name: String,
    pub scalar_type: ScalarType,
    pub arity: u8,
    pub quantization_error: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkDef {
    pub path: String,
    pub tick_start: u64,
    pub tick_end: u64,
    pub byte_len: u64,
    pub checksum: u32,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDef {
    pub path: String,
    pub byte_len: u64,
    pub checksum: u32,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheManifestV1 {
    pub schema_version: u32,
    pub engine_version: String,
    pub project_id: String,
    pub source_hash: String,
    pub tick_start: u64,
    pub tick_end: u64,
    pub ticks_per_second: u32,
    pub agent_count: u32,
    pub channels: Vec<ChannelDef>,
    pub agents: FileDef,
    #[serde(default)]
    pub behavior_events: Option<FileDef>,
    pub chunks: Vec<ChunkDef>,
    pub status: CacheStatus,
    pub cancellation_reason: Option<String>,
    pub last_complete_tick: Option<u64>,
}

impl CacheManifestV1 {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != CACHE_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedVersion(self.schema_version));
        }

        if self.status == CacheStatus::Complete {
            if !self.agents.complete {
                return Err(ManifestError::IncompleteAgentTable);
            }
            if self
                .behavior_events
                .as_ref()
                .is_some_and(|events| !events.complete)
            {
                return Err(ManifestError::IncompleteBehaviorEvents);
            }
            if let Some((index, _)) = self
                .chunks
                .iter()
                .enumerate()
                .find(|(_, chunk)| !chunk.complete)
            {
                return Err(ManifestError::IncompleteChunk { index });
            }
        }

        if self.status == CacheStatus::Canceled
            && !self.chunks.is_empty()
            && self.last_complete_tick.is_none()
        {
            return Err(ManifestError::MissingLastCompleteTick);
        }
        if self.status == CacheStatus::Canceled
            && self
                .cancellation_reason
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(ManifestError::MissingCancellationReason);
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    UnsupportedVersion(u32),
    IncompleteAgentTable,
    IncompleteBehaviorEvents,
    IncompleteChunk { index: usize },
    MissingLastCompleteTick,
    MissingCancellationReason,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported cache manifest version {version}")
            }
            Self::IncompleteAgentTable => write!(f, "complete cache has no complete agent table"),
            Self::IncompleteBehaviorEvents => {
                write!(f, "complete cache has incomplete behavior events")
            }
            Self::IncompleteChunk { index } => {
                write!(f, "complete cache contains incomplete chunk {index}")
            }
            Self::MissingLastCompleteTick => {
                write!(f, "canceled cache has no last complete tick")
            }
            Self::MissingCancellationReason => write!(f, "canceled cache has no reason"),
        }
    }
}

impl Error for ManifestError {}
