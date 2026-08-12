//! Atomic cache publication and cancellation.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::agents::{encode_agents, AgentTableError};
use crate::behavior_events::BehaviorEventLogV1;
use crate::{
    encode_chunk, payload_checksum, AgentStatic, BehaviorEventV1, CacheError, CacheManifestV1,
    CacheReader, CacheStatus, ChannelDef, ChunkDef, FileDef, Frame, PositionEncoding,
    CACHE_SCHEMA_VERSION,
};

#[derive(Clone, Debug)]
pub struct BakeSpec {
    pub engine_version: String,
    pub project_id: String,
    pub source_hash: String,
    pub tick_start: u64,
    pub tick_end: u64,
    pub ticks_per_second: u32,
    pub agent_count: u32,
    pub channels: Vec<ChannelDef>,
    pub chunk_ticks: u32,
    pub position_encoding: PositionEncoding,
}

#[derive(Clone, Default)]
pub struct CancelToken {
    canceled: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

pub struct CacheWriter {
    target: PathBuf,
    spec: BakeSpec,
    manifest: CacheManifestV1,
    agent_ids: Option<Vec<u64>>,
    pending: Vec<Frame>,
    next_tick: u64,
}

impl CacheWriter {
    pub fn create(target: &Path, spec: BakeSpec) -> Result<Self, CacheError> {
        if target.exists()
            && (!target.is_dir()
                || fs::read_dir(target)
                    .map_err(|error| CacheError::io(target, error))?
                    .next()
                    .is_some())
        {
            return Err(CacheError::AlreadyExists(target.to_owned()));
        }
        if spec.chunk_ticks == 0 {
            return Err(CacheError::InvalidBakeSpec("chunk_ticks must be positive"));
        }
        if spec.ticks_per_second == 0 {
            return Err(CacheError::InvalidBakeSpec(
                "ticks_per_second must be positive",
            ));
        }
        if spec.tick_end < spec.tick_start {
            return Err(CacheError::InvalidBakeSpec(
                "tick_end must not precede tick_start",
            ));
        }
        fs::create_dir_all(target.join("frames")).map_err(|error| CacheError::io(target, error))?;
        let manifest = CacheManifestV1 {
            schema_version: CACHE_SCHEMA_VERSION,
            engine_version: spec.engine_version.clone(),
            project_id: spec.project_id.clone(),
            source_hash: spec.source_hash.clone(),
            tick_start: spec.tick_start,
            tick_end: spec.tick_end,
            ticks_per_second: spec.ticks_per_second,
            agent_count: spec.agent_count,
            channels: spec.channels.clone(),
            agents: FileDef {
                path: "agents.bin".to_owned(),
                byte_len: 0,
                checksum: 0,
                complete: false,
            },
            behavior_events: None,
            chunks: Vec::new(),
            status: CacheStatus::Incomplete,
            cancellation_reason: None,
            last_complete_tick: None,
        };
        write_manifest_atomic(target, &manifest)?;
        Ok(Self {
            target: target.to_owned(),
            next_tick: spec.tick_start,
            spec,
            manifest,
            agent_ids: None,
            pending: Vec::new(),
        })
    }

    pub fn write_agents(&mut self, agents: &[AgentStatic]) -> Result<(), CacheError> {
        if agents.len() != self.spec.agent_count as usize {
            return Err(CacheError::AgentCountMismatch {
                expected: self.spec.agent_count as usize,
                found: agents.len(),
            });
        }
        let mut ids = HashSet::with_capacity(agents.len());
        for agent in agents {
            if !ids.insert(agent.agent_id) {
                return Err(CacheError::DuplicateAgentId(agent.agent_id));
            }
        }
        let bytes = encode_agents(agents).map_err(|error| match error {
            AgentTableError::DuplicateAgentId(id) => CacheError::DuplicateAgentId(id),
            other => CacheError::AgentTable {
                path: self.target.join("agents.bin"),
                message: other.to_string(),
            },
        })?;
        let path = self.target.join("agents.bin");
        atomic_write(&path, &bytes)?;
        self.manifest.agents.byte_len = bytes.len() as u64;
        self.manifest.agents.checksum = payload_checksum(&bytes);
        self.manifest.agents.complete = true;
        self.agent_ids = Some(agents.iter().map(|agent| agent.agent_id).collect());
        write_manifest_atomic(&self.target, &self.manifest)
    }

    pub fn push_tick(&mut self, tick: u64, frame: Frame) -> Result<(), CacheError> {
        if !self.manifest.agents.complete {
            return Err(CacheError::MissingAgentTable);
        }
        if tick != self.next_tick {
            return Err(CacheError::NonSequentialTick {
                expected: self.next_tick,
                found: tick,
            });
        }
        if tick > self.spec.tick_end {
            return Err(CacheError::TickOutOfRange {
                requested: tick,
                start: self.spec.tick_start,
                end: self.spec.tick_end,
            });
        }
        if frame.records.len() != self.spec.agent_count as usize {
            return Err(CacheError::AgentCountMismatch {
                expected: self.spec.agent_count as usize,
                found: frame.records.len(),
            });
        }
        let agent_ids = self
            .agent_ids
            .as_ref()
            .ok_or(CacheError::MissingAgentTable)?;
        if let Some((slot, (record, expected))) = frame
            .records
            .iter()
            .zip(agent_ids)
            .enumerate()
            .find(|(_, (record, expected))| record.agent_id != **expected)
        {
            return Err(CacheError::AgentIdMismatch {
                slot,
                expected: *expected,
                found: record.agent_id,
            });
        }
        self.pending.push(frame);
        self.next_tick += 1;
        if self.pending.len() == self.spec.chunk_ticks as usize {
            self.flush_pending()?;
        }
        Ok(())
    }

    /// Persist the complete ordered decision/event log before publishing the cache.
    pub fn write_behavior_events(&mut self, events: &[BehaviorEventV1]) -> Result<(), CacheError> {
        let log = BehaviorEventLogV1::new(events.to_vec());
        // Ordering is checked first so producer errors are deterministic.
        let ids = self.agent_ids.as_deref().unwrap_or_default();
        log.validate(self.spec.tick_start, self.spec.tick_end, ids)
            .map_err(CacheError::BehaviorEvents)?;
        if self.agent_ids.is_none() {
            return Err(CacheError::MissingAgentTable);
        }
        let relative = "events/behavior-v1.json";
        let path = safe_join(&self.target, relative)?;
        let mut bytes = serde_json::to_vec_pretty(&log).map_err(|error| CacheError::Json {
            path: path.clone(),
            message: error.to_string(),
        })?;
        bytes.push(b'\n');
        atomic_write(&path, &bytes)?;
        self.manifest.behavior_events = Some(FileDef {
            path: relative.to_owned(),
            byte_len: bytes.len() as u64,
            checksum: payload_checksum(&bytes),
            complete: true,
        });
        write_manifest_atomic(&self.target, &self.manifest)
    }

    pub fn finish(mut self) -> Result<CacheManifestV1, CacheError> {
        let found_last_tick = self.next_tick.checked_sub(1);
        if found_last_tick != Some(self.spec.tick_end) {
            return Err(CacheError::IncompleteBake {
                expected_last_tick: self.spec.tick_end,
                found_last_tick,
            });
        }
        self.flush_pending()?;
        self.manifest.status = CacheStatus::Complete;
        self.manifest.cancellation_reason = None;
        self.manifest.last_complete_tick = Some(self.spec.tick_end);
        self.manifest.validate().map_err(CacheError::Manifest)?;
        write_manifest_atomic(&self.target, &self.manifest)?;
        CacheReader::open_complete(&self.target)?;
        Ok(self.manifest)
    }

    pub fn cancel(mut self, reason: &str) -> Result<CacheManifestV1, CacheError> {
        self.flush_pending()?;
        self.manifest.status = CacheStatus::Canceled;
        self.manifest.cancellation_reason = Some(reason.to_owned());
        self.manifest.last_complete_tick = self.next_tick.checked_sub(1);
        self.manifest.validate().map_err(CacheError::Manifest)?;
        write_manifest_atomic(&self.target, &self.manifest)?;
        Ok(self.manifest)
    }

    fn flush_pending(&mut self) -> Result<(), CacheError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let tick_count = self.pending.len() as u64;
        let tick_start = self.next_tick - tick_count;
        let tick_end = self.next_tick - 1;
        let encoded = encode_chunk(tick_start, &self.pending, self.spec.position_encoding)
            .map_err(|source| CacheError::Codec {
                path: self.target.join("frames"),
                source,
            })?;
        if let Some(position) = self
            .manifest
            .channels
            .iter_mut()
            .find(|channel| channel.name == "position")
        {
            position.quantization_error = Some(
                position
                    .quantization_error
                    .unwrap_or(0.0)
                    .max(encoded.position_error_bound),
            );
        }
        let relative = format!("frames/{tick_start:06}-{tick_end:06}.chunk");
        let path = safe_join(&self.target, &relative)?;
        atomic_write(&path, &encoded.bytes)?;
        self.manifest.chunks.push(ChunkDef {
            path: relative,
            tick_start,
            tick_end,
            byte_len: encoded.bytes.len() as u64,
            checksum: payload_checksum(&encoded.bytes),
            complete: true,
        });
        self.pending.clear();
        self.manifest.last_complete_tick = Some(tick_end);
        write_manifest_atomic(&self.target, &self.manifest)
    }
}

pub(crate) fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, CacheError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(CacheError::UnsafeRelativePath(relative.to_owned()));
    }
    Ok(root.join(path))
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), CacheError> {
    let parent = path
        .parent()
        .ok_or_else(|| CacheError::UnsafeRelativePath(path.display().to_string()))?;
    fs::create_dir_all(parent).map_err(|error| CacheError::io(parent, error))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| CacheError::UnsafeRelativePath(path.display().to_string()))?;
    let temporary = path.with_file_name(format!("{}.tmp", file_name.to_string_lossy()));
    let mut file = File::create(&temporary).map_err(|error| CacheError::io(&temporary, error))?;
    file.write_all(bytes)
        .map_err(|error| CacheError::io(&temporary, error))?;
    file.sync_all()
        .map_err(|error| CacheError::io(&temporary, error))?;
    fs::rename(&temporary, path).map_err(|error| CacheError::io(path, error))
}

fn write_manifest_atomic(root: &Path, manifest: &CacheManifestV1) -> Result<(), CacheError> {
    let path = root.join("manifest.json");
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|error| CacheError::Json {
        path: path.clone(),
        message: error.to_string(),
    })?;
    bytes.push(b'\n');
    atomic_write(&path, &bytes)
}
