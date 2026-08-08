//! Random-access trace reader.
//!
//! Blender's playback is scrubbable, not sequential: an artist can drag the
//! timeline to any frame, so the reader must be able to jump straight to an
//! arbitrary tick rather than only stream forward. Because every tick is a
//! fixed `agent_count * RECORD_BYTES` stride laid out immediately after a
//! fixed-size header, that jump is a single seek — no index, chunk table,
//! or scan is needed, which is exactly the "no chunking" simplicity trace
//! v0 is committed to for now.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::{AgentRecord, Header, TraceError, HEADER_BYTES, RECORD_BYTES};

/// Reads trace v0. Holds an open file and its parsed header, nothing else.
pub struct TraceReader {
    inner: BufReader<File>,
    header: Header,
    // Reused across `read_tick` calls so scrubbing through a trace does not
    // allocate a fresh buffer per frame.
    scratch: Vec<u8>,
}

impl TraceReader {
    /// Open `path`, decode its header, and validate the magic and format
    /// version up front. A version mismatch fails here rather than
    /// resurfacing later as garbage records — `Header::decode` treats it as
    /// a hard error, never a silent accept.
    pub fn open(path: &Path) -> Result<Self, TraceError> {
        let mut inner = BufReader::new(File::open(path)?);
        let mut head = [0u8; HEADER_BYTES];
        inner.read_exact(&mut head).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                TraceError::Truncated {
                    expected: HEADER_BYTES,
                    found: 0,
                }
            } else {
                TraceError::Io(e)
            }
        })?;
        let header = Header::decode(&head)?;
        let scratch = vec![0u8; header.agent_count as usize * RECORD_BYTES];
        Ok(Self {
            inner,
            header,
            scratch,
        })
    }

    pub fn header(&self) -> Header {
        self.header
    }

    /// Read one tick into `out`, replacing its contents.
    ///
    /// Reads the tick's raw bytes into `scratch` in one call, then decodes
    /// each record from a `chunks_exact(RECORD_BYTES)` window rather than
    /// hand-computed byte offsets. `AgentRecord::decode` only checks that
    /// its input is *at least* `RECORD_BYTES` long, so handing it anything
    /// other than an exact 35-byte window (e.g. from manual index
    /// arithmetic gone wrong) could silently decode misaligned data and
    /// still "succeed". `chunks_exact` makes that class of bug impossible
    /// by construction.
    pub fn read_tick(&mut self, tick: u64, out: &mut Vec<AgentRecord>) -> Result<(), TraceError> {
        if tick >= self.header.tick_count {
            return Err(TraceError::TickOutOfRange {
                requested: tick,
                tick_count: self.header.tick_count,
            });
        }
        let stride = self.header.agent_count as u64 * RECORD_BYTES as u64;
        let offset = HEADER_BYTES as u64 + tick * stride;
        self.inner.seek(SeekFrom::Start(offset))?;
        self.inner.read_exact(&mut self.scratch).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                TraceError::Truncated {
                    expected: self.scratch.len(),
                    found: 0,
                }
            } else {
                TraceError::Io(e)
            }
        })?;
        out.clear();
        out.reserve(self.header.agent_count as usize);
        for chunk in self.scratch.chunks_exact(RECORD_BYTES) {
            out.push(AgentRecord::decode(chunk)?);
        }
        Ok(())
    }
}
