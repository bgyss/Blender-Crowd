//! Streaming, tick-major trace writer.
//!
//! The writer never buffers more than one tick's worth of records: crowd
//! simulations can run for tens of thousands of ticks, and holding the whole
//! trace in memory before writing would defeat the point of a file format
//! meant to decouple simulation from playback. Records are appended as soon
//! as `write_tick` receives them, one tick's records immediately after the
//! previous tick's, with no per-tick framing — the reader recovers tick
//! boundaries purely from `agent_count`, which is why every tick must carry
//! exactly that many records.
//!
//! `tick_count` is the one header field that cannot be known up front: the
//! caller may stream an unbounded number of ticks before calling `finish`.
//! So `create` writes a zeroed placeholder and `finish` seeks back to patch
//! it in place once the true count is known, rather than requiring the
//! caller to pre-declare a tick count it may not have.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use crate::{AgentRecord, Header, TraceError};

/// Writes trace v0. The tick count is unknown until writing ends, so the
/// header is written with a placeholder and patched by `finish`.
pub struct TraceWriter<W: Write + Seek> {
    inner: W,
    agent_count: u32,
    ticks_written: u64,
}

impl TraceWriter<BufWriter<File>> {
    /// Create a new trace file at `path`, writing a placeholder header.
    ///
    /// Buffered: unbuffered per-record writes would issue a syscall per
    /// agent per tick, which dominates wall-clock time for a crowd-sized
    /// trace far more than the extra copy a `BufWriter` costs.
    pub fn create(
        path: &Path,
        agent_count: u32,
        ticks_per_second: u32,
        world_to_meter: f32,
    ) -> Result<Self, TraceError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = BufWriter::new(File::create(path)?);
        Self::new(file, agent_count, ticks_per_second, world_to_meter)
    }
}

impl<W: Write + Seek> TraceWriter<W> {
    /// Build a writer over any `Write + Seek` sink. Generic over `W` so
    /// tests can exercise the format against an in-memory `Cursor` without
    /// touching disk, while `create` is the on-disk entry point real
    /// callers use.
    pub fn new(
        mut inner: W,
        agent_count: u32,
        ticks_per_second: u32,
        world_to_meter: f32,
    ) -> Result<Self, TraceError> {
        let placeholder = Header {
            tick_count: 0,
            agent_count,
            ticks_per_second,
            world_to_meter,
        };
        inner.write_all(&placeholder.encode())?;
        Ok(Self {
            inner,
            agent_count,
            ticks_written: 0,
        })
    }

    /// Append one tick's records. `records.len()` must equal the
    /// `agent_count` fixed at `create` time: the reader has no other way to
    /// find tick boundaries, so a short or long tick would silently corrupt
    /// every tick after it.
    pub fn write_tick(&mut self, records: &[AgentRecord]) -> Result<(), TraceError> {
        if records.len() != self.agent_count as usize {
            return Err(TraceError::AgentCountMismatch {
                expected: self.agent_count,
                found: records.len(),
            });
        }
        for record in records {
            self.inner.write_all(&record.encode())?;
        }
        self.ticks_written += 1;
        Ok(())
    }

    /// Patch the header's tick count and return it.
    ///
    /// The patch lands at byte offset 12, matching the `tick_count` field
    /// pinned in `header.rs` (magic 0..8, version 8..12, tick_count
    /// 12..20). That offset is load-bearing: it is exercised directly, byte
    /// for byte, by
    /// `finish_patches_tick_count_at_the_documented_offset` in
    /// `tests/round_trip.rs`.
    pub fn finish(mut self) -> Result<u64, TraceError> {
        self.inner.flush()?;
        self.inner.seek(SeekFrom::Start(12))?;
        self.inner.write_all(&self.ticks_written.to_le_bytes())?;
        self.inner.flush()?;
        Ok(self.ticks_written)
    }
}
