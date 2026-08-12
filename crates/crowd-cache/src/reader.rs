//! Complete-cache reader and incomplete-cache recovery inspection.

use std::fs;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use crate::agents::decode_agents;
use crate::behavior_events::BehaviorEventLogV1;
use crate::writer::safe_join;
use crate::{
    decode_chunk, payload_checksum, AgentStatic, BehaviorEventV1, CacheError, CacheManifestV1,
    CacheStatus, ChunkDef, FileDef, Frame,
};

pub struct CacheReader {
    root: PathBuf,
    manifest: CacheManifestV1,
    agents: Vec<AgentStatic>,
}

impl CacheReader {
    pub fn open_complete(root: &Path) -> Result<Self, CacheError> {
        let manifest = read_manifest(root)?;
        manifest.validate().map_err(CacheError::Manifest)?;
        if manifest.status != CacheStatus::Complete {
            return Err(CacheError::NotComplete(manifest.status));
        }

        let agent_path = safe_join(root, &manifest.agents.path)?;
        let agent_bytes = validate_file(&agent_path, &manifest.agents)?;
        let agents = decode_agents(&agent_bytes).map_err(|error| CacheError::AgentTable {
            path: agent_path,
            message: error.to_string(),
        })?;
        if agents.len() != manifest.agent_count as usize {
            return Err(CacheError::AgentCountMismatch {
                expected: manifest.agent_count as usize,
                found: agents.len(),
            });
        }

        let mut expected_tick = manifest.tick_start;
        for chunk in &manifest.chunks {
            if chunk.tick_start != expected_tick {
                return Err(CacheError::NonSequentialTick {
                    expected: expected_tick,
                    found: chunk.tick_start,
                });
            }
            let path = safe_join(root, &chunk.path)?;
            let bytes = validate_chunk_file(&path, chunk)?;
            let decoded = decode_chunk(&bytes).map_err(|source| CacheError::Codec {
                path: path.clone(),
                source,
            })?;
            if decoded.tick_start != chunk.tick_start
                || decoded.frames.len() as u64 != chunk.tick_end - chunk.tick_start + 1
            {
                return Err(CacheError::Codec {
                    path,
                    source: crate::CodecError::RecordCountMismatch {
                        declared: decoded.frames.len() as u64,
                        expected: chunk.tick_end - chunk.tick_start + 1,
                    },
                });
            }
            if decoded
                .frames
                .iter()
                .any(|frame| frame.records.len() != manifest.agent_count as usize)
            {
                return Err(CacheError::AgentCountMismatch {
                    expected: manifest.agent_count as usize,
                    found: decoded
                        .frames
                        .first()
                        .map_or(0, |frame| frame.records.len()),
                });
            }
            expected_tick = chunk.tick_end + 1;
        }
        if expected_tick != manifest.tick_end + 1 {
            return Err(CacheError::IncompleteBake {
                expected_last_tick: manifest.tick_end,
                found_last_tick: expected_tick.checked_sub(1),
            });
        }
        if let Some(definition) = &manifest.behavior_events {
            let path = safe_join(root, &definition.path)?;
            let bytes = validate_file(&path, definition)?;
            decode_behavior_events(&path, &bytes, &manifest, &agents)?;
        }

        Ok(Self {
            root: root.to_owned(),
            manifest,
            agents,
        })
    }

    pub fn manifest(&self) -> &CacheManifestV1 {
        &self.manifest
    }

    pub fn agents(&self) -> &[AgentStatic] {
        &self.agents
    }

    pub fn read_tick(&self, tick: u64) -> Result<Frame, CacheError> {
        if tick < self.manifest.tick_start || tick > self.manifest.tick_end {
            return Err(CacheError::TickOutOfRange {
                requested: tick,
                start: self.manifest.tick_start,
                end: self.manifest.tick_end,
            });
        }
        let chunk = self
            .manifest
            .chunks
            .iter()
            .find(|chunk| tick >= chunk.tick_start && tick <= chunk.tick_end)
            .ok_or(CacheError::TickOutOfRange {
                requested: tick,
                start: self.manifest.tick_start,
                end: self.manifest.tick_end,
            })?;
        let path = safe_join(&self.root, &chunk.path)?;
        let bytes = validate_chunk_file(&path, chunk)?;
        let decoded = decode_chunk(&bytes).map_err(|source| CacheError::Codec { path, source })?;
        Ok(decoded.frames[(tick - chunk.tick_start) as usize].clone())
    }

    /// Decode every frame sequentially, loading each chunk exactly once.
    ///
    /// Acceptance and render-preflight scans must not repeatedly decode the
    /// same chunk through `read_tick`; this path preserves cache order while
    /// keeping work proportional to the cache size.
    pub fn read_all_frames(&self) -> Result<Vec<Frame>, CacheError> {
        let mut frames =
            Vec::with_capacity((self.manifest.tick_end - self.manifest.tick_start + 1) as usize);
        for chunk in &self.manifest.chunks {
            let path = safe_join(&self.root, &chunk.path)?;
            let bytes = validate_chunk_file(&path, chunk)?;
            let decoded = decode_chunk(&bytes).map_err(|source| CacheError::Codec {
                path: path.clone(),
                source,
            })?;
            frames.extend(decoded.frames);
        }
        Ok(frames)
    }

    /// Read the optional M2 authored-behavior evidence channel. M1 caches
    /// intentionally return an empty log.
    pub fn read_behavior_events(&self) -> Result<Vec<BehaviorEventV1>, CacheError> {
        let Some(definition) = &self.manifest.behavior_events else {
            return Ok(Vec::new());
        };
        let path = safe_join(&self.root, &definition.path)?;
        let bytes = validate_file(&path, definition)?;
        Ok(decode_behavior_events(&path, &bytes, &self.manifest, &self.agents)?.events)
    }
}

fn decode_behavior_events(
    path: &Path,
    bytes: &[u8],
    manifest: &CacheManifestV1,
    agents: &[AgentStatic],
) -> Result<BehaviorEventLogV1, CacheError> {
    let log: BehaviorEventLogV1 =
        serde_json::from_slice(bytes).map_err(|error| CacheError::Json {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    let ids = agents
        .iter()
        .map(|agent| agent.agent_id)
        .collect::<Vec<_>>();
    log.validate(manifest.tick_start, manifest.tick_end, &ids)
        .map_err(|message| CacheError::Json {
            path: path.to_owned(),
            message,
        })?;
    Ok(log)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryReport {
    pub status: CacheStatus,
    pub cancellation_reason: Option<String>,
    pub last_complete_tick: Option<u64>,
    pub readable_tick_range: Option<RangeInclusive<u64>>,
    pub valid_chunk_count: usize,
}

pub struct RecoveryInspector;

impl RecoveryInspector {
    pub fn open(root: &Path) -> Result<RecoveryReport, CacheError> {
        let manifest = read_manifest(root)?;
        manifest.validate().map_err(CacheError::Manifest)?;

        let mut expected_tick = manifest.tick_start;
        let mut valid_chunk_count = 0usize;
        for chunk in &manifest.chunks {
            if !chunk.complete || chunk.tick_start != expected_tick {
                break;
            }
            let path = safe_join(root, &chunk.path)?;
            let Ok(bytes) = validate_chunk_file(&path, chunk) else {
                break;
            };
            let Ok(decoded) = decode_chunk(&bytes) else {
                break;
            };
            if decoded.tick_start != chunk.tick_start
                || decoded.frames.len() as u64 != chunk.tick_end - chunk.tick_start + 1
            {
                break;
            }
            valid_chunk_count += 1;
            expected_tick = chunk.tick_end + 1;
        }
        let readable_tick_range = if valid_chunk_count == 0 {
            None
        } else {
            Some(manifest.tick_start..=expected_tick - 1)
        };
        Ok(RecoveryReport {
            status: manifest.status,
            cancellation_reason: manifest.cancellation_reason,
            last_complete_tick: manifest.last_complete_tick,
            readable_tick_range,
            valid_chunk_count,
        })
    }
}

fn read_manifest(root: &Path) -> Result<CacheManifestV1, CacheError> {
    let path = root.join("manifest.json");
    let bytes = fs::read(&path).map_err(|error| CacheError::io(&path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| CacheError::Json {
        path,
        message: error.to_string(),
    })
}

fn validate_file(path: &Path, def: &FileDef) -> Result<Vec<u8>, CacheError> {
    if !path.exists() {
        return Err(CacheError::MissingFile(path.to_owned()));
    }
    let bytes = fs::read(path).map_err(|error| CacheError::io(path, error))?;
    if bytes.len() as u64 != def.byte_len {
        return Err(CacheError::FileLength {
            path: path.to_owned(),
            expected: def.byte_len,
            found: bytes.len() as u64,
        });
    }
    let found = payload_checksum(&bytes);
    if found != def.checksum {
        return Err(CacheError::FileChecksum {
            path: path.to_owned(),
            expected: def.checksum,
            found,
        });
    }
    Ok(bytes)
}

fn validate_chunk_file(path: &Path, def: &ChunkDef) -> Result<Vec<u8>, CacheError> {
    validate_file(
        path,
        &FileDef {
            path: def.path.clone(),
            byte_len: def.byte_len,
            checksum: def.checksum,
            complete: def.complete,
        },
    )
}
