# Blender Bridge and Native Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove a Rust native module loads in a stock Blender 5.2 LTS install, reads a simulation result from disk with no live simulation, and drives 1,000 Geometry Nodes points — every step runnable headlessly.

**Architecture:** A new pure-Rust crate `crowd-trace` owns a deliberately simple binary format (trace v0). A thin PyO3 crate `crowd-blender` wraps it and decides nothing. A Blender extension in `addon/blender_crowd/` bundles that crate as an abi3 wheel and pushes per-tick data into point-cloud attributes for a Geometry Nodes asset to instance.

**Tech Stack:** Rust 1.94.1 (pinned), PyO3 with `abi3-py311`, maturin, Blender 5.2.0 LTS, bundled CPython 3.13.13, bundled numpy 2.3.4.

**Spec:** [Blender bridge and native packaging slice design](../specs/2026-08-07-blender-bridge-slice-design.md)

## Global Constraints

Every task's requirements implicitly include this section.

- Target host is **Blender 5.2 LTS only**. `blender_version_min = "5.2.0"`.
- Blender binary path on this machine: `/Applications/Blender.app/Contents/MacOS/Blender`.
- The Python module name is **`blender_crowd_native`**. Bundled wheels unpack into `extensions/.local/lib/python3.13/site-packages/`, shared by every installed extension, so a generic name like `crowd` or `core` would risk collision.
- The extension id is **`blender_crowd`**; it is installed to repo `user_default`, so its full package path is `bl_ext.user_default.blender_crowd`.
- **Use relative imports everywhere inside the addon package** (`from . import x`). Extensions are imported as `bl_ext.user_default.blender_crowd`, so absolute imports like `from blender_crowd.x import y` fail with "package not found".
- The wheel is built **abi3** via `pyo3/abi3-py311`.
- `crowd-blender` **decides nothing**: no simulation, no policy, no state beyond an open file handle and its parsed header.
- Trace v0 has **no chunking, no quantization, no checksums, no compression**. Those are cache v0 decisions and must stay open.
- Records are **packed, not padded**, stride exactly 35 bytes, little-endian throughout.
- `cargo fmt` before every commit; `cargo clippy --workspace --all-targets -- -D warnings` must be clean.
- Never claim a runner passed if it is not checked in. Performance claims require a checked-in fixture, runner, report, and recorded environment.
- A run recorded with `--svg` or `--frames` samples every tick, so its `ticks_per_second` is not a performance measurement and must not be quoted as one.

## File Structure

| Path | Responsibility |
|---|---|
| `crates/crowd-trace/Cargo.toml` | New crate manifest; depends on `crowd-core` only |
| `crates/crowd-trace/src/lib.rs` | Public surface: `Header`, `AgentRecord`, `TraceWriter`, `TraceReader`, `TraceError` |
| `crates/crowd-trace/src/header.rs` | Header encode/decode and version validation |
| `crates/crowd-trace/src/record.rs` | Packed 35-byte record encode/decode |
| `crates/crowd-trace/src/writer.rs` | `TraceWriter` — streaming tick-major writes |
| `crates/crowd-trace/src/reader.rs` | `TraceReader` — random-access tick reads |
| `crates/crowd-trace/tests/round_trip.rs` | Round trip, version mismatch, truncation |
| `crates/crowd-blender/Cargo.toml` | PyO3 cdylib, abi3 |
| `crates/crowd-blender/src/lib.rs` | PyO3 module `blender_crowd_native` |
| `crates/crowd-bench/src/main.rs` | Add `--trace` flag |
| `crates/crowd-bench/src/trace_out.rs` | Drive `TraceWriter` from a `Simulation` |
| `addon/blender_crowd/blender_manifest.toml` | Extension manifest |
| `addon/blender_crowd/__init__.py` | `register()` / `unregister()` |
| `addon/blender_crowd/trace_playback.py` | Point-cloud sync from a trace |
| `addon/blender_crowd/operators.py` | Load-trace operator |
| `scripts/build-wheel.sh` | maturin build |
| `scripts/blender-install-test.sh` | Clean install + load assertion (M0 criterion 5) |
| `scripts/blender-playback-test.sh` | 1,000-point playback + split cost (M0 criterion 6) |
| `tests/blender/test_install.py` | Runs inside Blender; asserts module origin |
| `tests/blender/test_playback.py` | Runs inside Blender; asserts IDs/positions |

---

### Task 1: `crowd-trace` header

**Files:**
- Create: `crates/crowd-trace/Cargo.toml`
- Create: `crates/crowd-trace/src/lib.rs`
- Create: `crates/crowd-trace/src/header.rs`
- Modify: `Cargo.toml:2` (workspace members)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub const MAGIC: [u8; 8] = *b"CRWDTRC0";`
  - `pub const FORMAT_VERSION: u32 = 0;`
  - `pub const HEADER_BYTES: usize = 32;`
  - `pub struct Header { pub tick_count: u64, pub agent_count: u32, pub ticks_per_second: u32, pub world_to_meter: f32 }`
  - `impl Header { pub fn encode(&self) -> [u8; HEADER_BYTES]; pub fn decode(bytes: &[u8]) -> Result<Header, TraceError>; }`
  - `pub enum TraceError { BadMagic, UnsupportedVersion { found: u32, expected: u32 }, Truncated { expected: usize, found: usize }, Io(std::io::Error) }`

- [ ] **Step 1: Add the crate to the workspace**

Modify `Cargo.toml` line 2:

```toml
members = ["crates/crowd-core", "crates/crowd-bench", "crates/crowd-trace"]
```

Create `crates/crowd-trace/Cargo.toml`:

```toml
[package]
name = "crowd-trace"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
crowd-core = { path = "../crowd-core" }
```

- [ ] **Step 2: Write the failing test**

Create `crates/crowd-trace/tests/round_trip.rs`:

```rust
use crowd_trace::{Header, TraceError, FORMAT_VERSION, HEADER_BYTES};

#[test]
fn header_round_trips() {
    let h = Header {
        tick_count: 1234,
        agent_count: 1000,
        ticks_per_second: 30,
        world_to_meter: 1.0,
    };
    let bytes = h.encode();
    assert_eq!(bytes.len(), HEADER_BYTES);
    let back = Header::decode(&bytes).expect("decode");
    assert_eq!(back.tick_count, 1234);
    assert_eq!(back.agent_count, 1000);
    assert_eq!(back.ticks_per_second, 30);
    assert_eq!(back.world_to_meter, 1.0);
}

#[test]
fn header_rejects_bad_magic() {
    let mut bytes = Header {
        tick_count: 1,
        agent_count: 1,
        ticks_per_second: 30,
        world_to_meter: 1.0,
    }
    .encode();
    bytes[0] = b'X';
    assert!(matches!(Header::decode(&bytes), Err(TraceError::BadMagic)));
}

#[test]
fn header_rejects_future_version() {
    let mut bytes = Header {
        tick_count: 1,
        agent_count: 1,
        ticks_per_second: 30,
        world_to_meter: 1.0,
    }
    .encode();
    bytes[8..12].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    match Header::decode(&bytes) {
        Err(TraceError::UnsupportedVersion { found, expected }) => {
            assert_eq!(found, FORMAT_VERSION + 1);
            assert_eq!(expected, FORMAT_VERSION);
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn header_rejects_short_buffer() {
    let bytes = [0u8; 4];
    assert!(matches!(
        Header::decode(&bytes),
        Err(TraceError::Truncated { .. })
    ));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p crowd-trace`
Expected: FAIL — `error[E0433]: failed to resolve: use of undeclared crate or module 'crowd_trace'`

- [ ] **Step 4: Write the implementation**

Create `crates/crowd-trace/src/lib.rs`:

```rust
//! Trace v0: the provisional Rust-to-Blender transport format.
//!
//! Deliberately the simplest thing that proves a cached simulation can be
//! replayed in Blender with no live simulation session. It has no chunking,
//! quantization, checksums, or compression *on purpose*: those are cache v0
//! design decisions that require their own measured review, and trace v0
//! exists so that proving the bridge does not settle them by accident.
//!
//! See `docs/superpowers/specs/2026-08-07-blender-bridge-slice-design.md`.

pub mod header;

pub use header::{Header, HEADER_BYTES, MAGIC, FORMAT_VERSION};

/// Every way reading a trace can fail.
#[derive(Debug)]
pub enum TraceError {
    BadMagic,
    UnsupportedVersion { found: u32, expected: u32 },
    Truncated { expected: usize, found: usize },
    Io(std::io::Error),
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a crowd trace file (bad magic)"),
            Self::UnsupportedVersion { found, expected } => write!(
                f,
                "trace format version {found} is not supported (expected {expected})"
            ),
            Self::Truncated { expected, found } => {
                write!(f, "trace truncated: expected {expected} bytes, found {found}")
            }
            Self::Io(e) => write!(f, "io error reading trace: {e}"),
        }
    }
}

impl std::error::Error for TraceError {}

impl From<std::io::Error> for TraceError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
```

Create `crates/crowd-trace/src/header.rs`:

```rust
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-trace`
Expected: PASS, 4 tests.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/crowd-trace
git commit -m "Add the trace v0 header and its version gate"
```

---

### Task 2: `crowd-trace` packed record

**Files:**
- Create: `crates/crowd-trace/src/record.rs`
- Modify: `crates/crowd-trace/src/lib.rs` (add `pub mod record;` and re-export)
- Modify: `crates/crowd-trace/tests/round_trip.rs` (append tests)

**Interfaces:**
- Consumes: `TraceError` from Task 1.
- Produces:
  - `pub const RECORD_BYTES: usize = 35;`
  - `pub struct AgentRecord { pub agent_id: u64, pub position: [f32; 2], pub orientation: f32, pub flags: u32, pub clip_index: u16, pub phase: f32, pub playback_rate: f32, pub render_tier: u8 }`
  - `pub const FLAG_ACTIVE: u32 = 1 << 0;` and `pub const FLAG_ARRIVED: u32 = 1 << 1;`
  - `impl AgentRecord { pub fn encode(&self) -> [u8; RECORD_BYTES]; pub fn decode(bytes: &[u8]) -> Result<AgentRecord, TraceError>; }`

- [ ] **Step 1: Write the failing test**

Append to `crates/crowd-trace/tests/round_trip.rs`:

```rust
use crowd_trace::{AgentRecord, FLAG_ARRIVED, RECORD_BYTES};

fn sample_record() -> AgentRecord {
    AgentRecord {
        agent_id: 0x0123_4567_89ab_cdef,
        position: [1.5, -2.25],
        orientation: 0.75,
        flags: FLAG_ARRIVED,
        clip_index: 3,
        phase: 0.5,
        playback_rate: 1.25,
        render_tier: 2,
    }
}

#[test]
fn record_is_packed_to_35_bytes() {
    assert_eq!(RECORD_BYTES, 35);
    assert_eq!(sample_record().encode().len(), 35);
}

#[test]
fn record_round_trips() {
    let r = sample_record();
    let back = AgentRecord::decode(&r.encode()).expect("decode");
    assert_eq!(back.agent_id, r.agent_id);
    assert_eq!(back.position, r.position);
    assert_eq!(back.orientation, r.orientation);
    assert_eq!(back.flags, r.flags);
    assert_eq!(back.clip_index, r.clip_index);
    assert_eq!(back.phase, r.phase);
    assert_eq!(back.playback_rate, r.playback_rate);
    assert_eq!(back.render_tier, r.render_tier);
}

#[test]
fn record_preserves_full_64_bit_id() {
    // Stable identity is a contract guarantee. A truncated ID is not a
    // stable ID, so the high word must survive a round trip intact.
    let mut r = sample_record();
    r.agent_id = u64::MAX - 1;
    let back = AgentRecord::decode(&r.encode()).expect("decode");
    assert_eq!(back.agent_id, u64::MAX - 1);
}

#[test]
fn record_rejects_short_buffer() {
    let bytes = [0u8; RECORD_BYTES - 1];
    assert!(AgentRecord::decode(&bytes).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-trace`
Expected: FAIL — `unresolved import 'crowd_trace::AgentRecord'`

- [ ] **Step 3: Write the implementation**

Create `crates/crowd-trace/src/record.rs`:

```rust
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
```

Modify `crates/crowd-trace/src/lib.rs` — add below `pub mod header;`:

```rust
pub mod record;
```

and extend the re-export line to:

```rust
pub use record::{AgentRecord, FLAG_ACTIVE, FLAG_ARRIVED, RECORD_BYTES};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p crowd-trace`
Expected: PASS, 8 tests.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add crates/crowd-trace
git commit -m "Add the packed trace v0 agent record"
```

---

### Task 3: `TraceWriter` and `TraceReader`

**Files:**
- Create: `crates/crowd-trace/src/writer.rs`
- Create: `crates/crowd-trace/src/reader.rs`
- Modify: `crates/crowd-trace/src/lib.rs`
- Modify: `crates/crowd-trace/tests/round_trip.rs`

**Interfaces:**
- Consumes: `Header`, `AgentRecord`, `TraceError`, `HEADER_BYTES`, `RECORD_BYTES`.
- Produces:
  - `pub struct TraceWriter<W: Write + Seek>`
  - `impl TraceWriter<std::fs::File> { pub fn create(path: &Path, agent_count: u32, ticks_per_second: u32, world_to_meter: f32) -> Result<Self, TraceError> }`
  - `impl<W: Write + Seek> TraceWriter<W> { pub fn write_tick(&mut self, records: &[AgentRecord]) -> Result<(), TraceError>; pub fn finish(self) -> Result<u64, TraceError> }`
  - `pub struct TraceReader`
  - `impl TraceReader { pub fn open(path: &Path) -> Result<Self, TraceError>; pub fn header(&self) -> Header; pub fn read_tick(&mut self, tick: u64, out: &mut Vec<AgentRecord>) -> Result<(), TraceError> }`

**Note on the writer:** `tick_count` is not known until writing finishes, so `create` writes a placeholder header and `finish` seeks back to offset 12 to patch it. `finish` returns the final tick count.

- [ ] **Step 1: Write the failing test**

Append to `crates/crowd-trace/tests/round_trip.rs`:

```rust
use crowd_trace::{TraceReader, TraceWriter};

fn record_for(tick: u64, agent: u64) -> AgentRecord {
    AgentRecord {
        agent_id: agent * 1_000_003,
        position: [tick as f32, agent as f32],
        orientation: 0.1 * tick as f32,
        flags: crowd_trace::FLAG_ACTIVE,
        clip_index: 0,
        phase: 0.0,
        playback_rate: 1.0,
        render_tier: 0,
    }
}

#[test]
fn trace_file_round_trips() {
    let dir = std::env::temp_dir().join("crowd-trace-round-trip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.crowdtrace");

    let agents = 4u32;
    let ticks = 3u64;
    let mut w = TraceWriter::create(&path, agents, 30, 1.0).unwrap();
    for tick in 0..ticks {
        let batch: Vec<_> = (0..agents as u64).map(|a| record_for(tick, a)).collect();
        w.write_tick(&batch).unwrap();
    }
    assert_eq!(w.finish().unwrap(), ticks);

    let mut r = TraceReader::open(&path).unwrap();
    let header = r.header();
    assert_eq!(header.tick_count, ticks);
    assert_eq!(header.agent_count, agents);
    assert_eq!(header.ticks_per_second, 30);

    let mut out = Vec::new();
    for tick in 0..ticks {
        r.read_tick(tick, &mut out).unwrap();
        assert_eq!(out.len(), agents as usize);
        for (a, got) in out.iter().enumerate() {
            assert_eq!(*got, record_for(tick, a as u64));
        }
    }
    std::fs::remove_file(&path).ok();
}

#[test]
fn reader_rejects_out_of_range_tick() {
    let dir = std::env::temp_dir().join("crowd-trace-range");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.crowdtrace");
    let mut w = TraceWriter::create(&path, 2, 30, 1.0).unwrap();
    w.write_tick(&[record_for(0, 0), record_for(0, 1)]).unwrap();
    w.finish().unwrap();

    let mut r = TraceReader::open(&path).unwrap();
    let mut out = Vec::new();
    assert!(r.read_tick(5, &mut out).is_err());
    std::fs::remove_file(&path).ok();
}

#[test]
fn reader_rejects_truncated_file() {
    let dir = std::env::temp_dir().join("crowd-trace-trunc");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.crowdtrace");
    let mut w = TraceWriter::create(&path, 2, 30, 1.0).unwrap();
    w.write_tick(&[record_for(0, 0), record_for(0, 1)]).unwrap();
    w.finish().unwrap();

    // Chop the last record off; the header still claims one full tick.
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - crowd_trace::RECORD_BYTES]).unwrap();

    let mut r = TraceReader::open(&path).unwrap();
    let mut out = Vec::new();
    assert!(r.read_tick(0, &mut out).is_err());
    std::fs::remove_file(&path).ok();
}

#[test]
fn writer_rejects_wrong_agent_count() {
    let dir = std::env::temp_dir().join("crowd-trace-count");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.crowdtrace");
    let mut w = TraceWriter::create(&path, 4, 30, 1.0).unwrap();
    assert!(w.write_tick(&[record_for(0, 0)]).is_err());
    std::fs::remove_file(&path).ok();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-trace`
Expected: FAIL — `unresolved import 'crowd_trace::TraceReader'`

- [ ] **Step 3: Add the `AgentCountMismatch` and `TickOutOfRange` errors**

Modify `crates/crowd-trace/src/lib.rs` — add these two variants to `TraceError`:

```rust
    AgentCountMismatch { expected: u32, found: usize },
    TickOutOfRange { requested: u64, tick_count: u64 },
```

and add their `Display` arms inside the existing `match self`:

```rust
            Self::AgentCountMismatch { expected, found } => write!(
                f,
                "tick has {found} records but the header declares {expected} agents"
            ),
            Self::TickOutOfRange { requested, tick_count } => write!(
                f,
                "tick {requested} is out of range (trace has {tick_count} ticks)"
            ),
```

- [ ] **Step 4: Write the writer**

Create `crates/crowd-trace/src/writer.rs`:

```rust
//! Streaming, tick-major trace writer.

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
    pub fn finish(mut self) -> Result<u64, TraceError> {
        self.inner.flush()?;
        self.inner.seek(SeekFrom::Start(12))?;
        self.inner.write_all(&self.ticks_written.to_le_bytes())?;
        self.inner.flush()?;
        Ok(self.ticks_written)
    }
}
```

- [ ] **Step 5: Write the reader**

Create `crates/crowd-trace/src/reader.rs`:

```rust
//! Random-access trace reader.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::{AgentRecord, Header, TraceError, HEADER_BYTES, RECORD_BYTES};

/// Reads trace v0. Holds an open file and its parsed header, nothing else.
pub struct TraceReader {
    inner: BufReader<File>,
    header: Header,
    scratch: Vec<u8>,
}

impl TraceReader {
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
```

Modify `crates/crowd-trace/src/lib.rs` — add the modules and re-exports:

```rust
pub mod reader;
pub mod writer;

pub use reader::TraceReader;
pub use writer::TraceWriter;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p crowd-trace`
Expected: PASS, 12 tests.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add crates/crowd-trace
git commit -m "Add the trace v0 writer and reader"
```

---

### Task 4: `crowd-bench --trace`

**Files:**
- Create: `crates/crowd-bench/src/trace_out.rs`
- Modify: `crates/crowd-bench/src/main.rs`
- Modify: `crates/crowd-bench/Cargo.toml`

**Interfaces:**
- Consumes: `TraceWriter`, `AgentRecord`, `FLAG_ACTIVE`, `FLAG_ARRIVED` from Task 3; `Simulation`, `World` from `crowd-core`.
- Produces: `pub fn write_trace(sim: &mut Simulation, path: &Path, ticks: u64) -> Result<u64, TraceError>` in `crowd_bench::trace_out`, and a `--trace` CLI flag.

**Verified API facts** (do not guess these):
- Scenes are built with `crowd_core::scenes::build(name: &str, agents: u32, seed: u64) -> Option<SceneDef>`, then `.compile() -> Result<CompiledScene, _>`. There is no `scenes::compile`.
- A scene's length is `sim.scene().duration_ticks: u64`; the current tick is `sim.clock().tick() -> u64`. `Simulation::run_to_completion` steps until the former is reached.
- `Args` in `main.rs` has **no** `ticks` field. Its fields are `scene`, `agents`, `seed`, `svg`, `frames`, `frame_interval`, `out`, `solver`.
- `crates/crowd-bench/src/lib.rs` does **not** exist yet; Step 4 creates it.

**Note:** Read the SoA `World` fields directly — `world.agent_id`, `world.pos_x`, `world.pos_y`, `world.yaw`, `world.arrived`. Iterate in slot order; slot order is the world's own stable order.

- [ ] **Step 1: Add the dependency**

Modify `crates/crowd-bench/Cargo.toml`, in `[dependencies]`:

```toml
crowd-trace = { path = "../crowd-trace" }
```

- [ ] **Step 2: Write the failing test**

Create `crates/crowd-bench/tests/trace_emit.rs`:

```rust
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::SampledVelocitySolver;
use crowd_trace::TraceReader;

#[test]
fn emitted_trace_matches_the_simulation() {
    let scene = scenes::build("crossing", 20, 2026)
        .expect("scene exists")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );

    let dir = std::env::temp_dir().join("crowd-bench-trace-emit");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("emit.crowdtrace");

    let agent_count = sim.world().len();
    let ticks = crowd_bench::trace_out::write_trace(&mut sim, &path, 25).expect("write");
    assert_eq!(ticks, 25);

    let mut reader = TraceReader::open(&path).expect("open");
    assert_eq!(reader.header().agent_count as usize, agent_count);
    assert_eq!(reader.header().tick_count, 25);

    // The final tick in the trace must equal the simulation's final state.
    let mut out = Vec::new();
    reader.read_tick(24, &mut out).expect("read");
    let world = sim.world();
    for (slot, record) in out.iter().enumerate() {
        assert_eq!(record.agent_id, world.agent_id[slot].0);
        assert_eq!(record.position[0], world.pos_x[slot]);
        assert_eq!(record.position[1], world.pos_y[slot]);
        assert_eq!(record.orientation, world.yaw[slot]);
    }
    std::fs::remove_file(&path).ok();
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p crowd-bench --test trace_emit`
Expected: FAIL — `crowd_bench` has no `trace_out` module (and, if `crowd-bench` has no `lib.rs`, `unresolved import 'crowd_bench'`).

- [ ] **Step 4: Expose the crate as a library**

`crates/crowd-bench` is binary-only today, so an integration test cannot reach
its modules. Add a library target alongside the existing binary. Create
`crates/crowd-bench/src/lib.rs`:

```rust
//! Benchmark harness internals, exposed as a library so integration tests
//! can drive them directly.

pub mod trace_out;
```

Then add to `crates/crowd-bench/Cargo.toml`:

```toml
[lib]
name = "crowd_bench"
path = "src/lib.rs"

[[bin]]
name = "crowd-bench"
path = "src/main.rs"
```

- [ ] **Step 5: Write the implementation**

Create `crates/crowd-bench/src/trace_out.rs`:

```rust
//! Emit a trace v0 file from a running simulation.
//!
//! This is the producer half of the Blender bridge: it is how a headless
//! simulation result reaches Blender with no live simulation session.

use std::path::Path;

use crowd_core::sim::Simulation;
use crowd_core::units::{DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
use crowd_trace::{AgentRecord, TraceError, TraceWriter, FLAG_ACTIVE, FLAG_ARRIVED};

/// Step `sim` for `ticks` ticks, writing one trace record per agent per tick.
///
/// Records are emitted in world slot order, which is the world's own stable
/// order. Returns the number of ticks written.
pub fn write_trace(sim: &mut Simulation, path: &Path, ticks: u64) -> Result<u64, TraceError> {
    let agent_count = sim.world().len() as u32;
    let mut writer = TraceWriter::create(
        path,
        agent_count,
        DEFAULT_TICKS_PER_SECOND,
        WORLD_TO_METER,
    )?;

    let mut batch: Vec<AgentRecord> = Vec::with_capacity(agent_count as usize);
    for _ in 0..ticks {
        sim.step();
        batch.clear();
        let world = sim.world();
        for slot in 0..world.len() {
            batch.push(AgentRecord {
                agent_id: world.agent_id[slot].0,
                position: [world.pos_x[slot], world.pos_y[slot]],
                orientation: world.yaw[slot],
                flags: if world.arrived[slot] {
                    FLAG_ARRIVED
                } else {
                    FLAG_ACTIVE
                },
                // Stubs: no animation system exists yet. See the slice design.
                clip_index: 0,
                phase: 0.0,
                playback_rate: 1.0,
                render_tier: 0,
            });
        }
        writer.write_tick(&batch)?;
    }

    writer.finish()
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p crowd-bench --test trace_emit`
Expected: PASS.

- [ ] **Step 7: Wire up the CLI flag**

Modify `crates/crowd-bench/src/main.rs`. Add a field to the `Args` struct
(whose fields are `scene`, `agents`, `seed`, `svg`, `frames`, `frame_interval`,
`out`, `solver`):

```rust
    trace: bool,
```

Add the flag to the `match` in `parse_args` (alongside the existing
`"--frames" => args.frames = true,` arm at line 89):

```rust
            "--trace" => args.trace = true,
```

The `Simulation` itself lives in `crates/crowd-bench/src/report.rs` — it is
built at line 231 and stepped by `sim.run_to_completion()` at line 283. Thread
the `trace` flag through to the same options struct that carries `svg` and
`frames`, then add this **in place of** the `run_to_completion()` call when
tracing is on. It must replace that call, not follow it: after the scene's
duration has elapsed there are no ticks left to record.

```rust
    if args.trace {
        let path = options
            .out
            .join(format!("{}-{}.crowdtrace", options.scene, options.agents));
        // `Args` has no tick count: scene length comes from the compiled
        // scene, exactly as `Simulation::run_to_completion` uses it. The
        // trace must be written by the same stepping loop that records it,
        // so the trace run replaces the normal run rather than following it.
        let remaining = sim
            .scene()
            .duration_ticks
            .saturating_sub(sim.clock().tick());
        let ticks = crowd_bench::trace_out::write_trace(&mut sim, &path, remaining)
            .map_err(|e| format!("writing trace: {e}"))?;
        println!("trace: {} ({ticks} ticks)", path.display());
    }
```

- [ ] **Step 8: Verify the flag end to end**

Run: `cargo run --release -p crowd-bench -- run --scene crossing --agents 1000 --trace --out benchmarks/reports`
Expected: prints a `trace:` line; the file exists and is `32 + ticks * 1000 * 35` bytes.

- [ ] **Step 9: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add crates/crowd-bench Cargo.lock
git commit -m "Emit trace v0 files from crowd-bench with --trace"
```

---

### Task 5: `crowd-blender` PyO3 module

**Files:**
- Create: `crates/crowd-blender/Cargo.toml`
- Create: `crates/crowd-blender/src/lib.rs`
- Create: `crates/crowd-blender/pyproject.toml`
- Modify: `Cargo.toml` (workspace members)
- Modify: `mise.toml` (add maturin)
- Create: `scripts/build-wheel.sh`

**Interfaces:**
- Consumes: `TraceReader`, `Header`, `AgentRecord` from Task 3.
- Produces the Python module `blender_crowd_native`:
  - `Trace(path: str)` — constructor, raises `OSError` on any trace error
  - `Trace.tick_count -> int`, `Trace.agent_count -> int`, `Trace.ticks_per_second -> int`, `Trace.world_to_meter -> float`
  - `Trace.read_tick(tick: int) -> dict[str, bytes]` returning keys `position` (3×f32 per agent, z=0), `orientation` (f32), `agent_id_lo` (i32), `agent_id_hi` (i32), `flags` (i32), `clip_index` (i32), `phase` (f32), `playback_rate` (f32), `render_tier` (i32)
  - `__version__: str`

**Note on the return type:** raw `bytes` per channel, laid out for direct `numpy.frombuffer` and `foreach_set`. `position` is emitted as 3 floats per agent because Blender's `position` point attribute is `FLOAT_VECTOR`; z is always 0.0. `agent_id` is split into two `i32` halves because Blender point attributes are 32-bit and truncating a stable ID would break the identity contract.

- [ ] **Step 1: Add maturin to the pinned toolchain**

Modify `mise.toml`, in `[tools]`:

```toml
"cargo:maturin" = "1.9.6"
```

Run: `mise install`
Expected: maturin becomes available; verify with `maturin --version`.

- [ ] **Step 2: Create the crate**

Modify root `Cargo.toml` line 2:

```toml
members = ["crates/crowd-core", "crates/crowd-bench", "crates/crowd-trace", "crates/crowd-blender"]
```

Create `crates/crowd-blender/Cargo.toml`:

```toml
[package]
name = "crowd-blender"
edition.workspace = true
version.workspace = true
license.workspace = true

[lib]
name = "blender_crowd_native"
crate-type = ["cdylib"]

[dependencies]
crowd-trace = { path = "../crowd-trace" }
pyo3 = { version = "0.27", features = ["abi3-py311", "extension-module"] }
```

Create `crates/crowd-blender/pyproject.toml`:

```toml
[build-system]
requires = ["maturin>=1.9,<2.0"]
build-backend = "maturin"

[project]
name = "blender-crowd-native"
requires-python = ">=3.11"
classifiers = ["Programming Language :: Rust"]

[tool.maturin]
module-name = "blender_crowd_native"
manifest-path = "Cargo.toml"
```

- [ ] **Step 3: Write the implementation**

Create `crates/crowd-blender/src/lib.rs`:

```rust
//! PyO3 bridge from trace v0 to Blender.
//!
//! This module decides nothing. It performs no simulation, applies no policy,
//! and holds no state beyond an open file and its parsed header. Anything
//! requiring a decision belongs in `crowd-core` or in the addon. Keeping this
//! rule is what keeps the FFI surface small enough to audit.

use std::path::PathBuf;

use crowd_trace::{AgentRecord, TraceReader};
use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

/// A read-only handle to a trace v0 file.
#[pyclass]
struct Trace {
    reader: TraceReader,
    scratch: Vec<AgentRecord>,
}

#[pymethods]
impl Trace {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let reader =
            TraceReader::open(&path).map_err(|e| PyOSError::new_err(format!("{path:?}: {e}")))?;
        Ok(Self {
            reader,
            scratch: Vec::new(),
        })
    }

    #[getter]
    fn tick_count(&self) -> u64 {
        self.reader.header().tick_count
    }

    #[getter]
    fn agent_count(&self) -> u32 {
        self.reader.header().agent_count
    }

    #[getter]
    fn ticks_per_second(&self) -> u32 {
        self.reader.header().ticks_per_second
    }

    #[getter]
    fn world_to_meter(&self) -> f32 {
        self.reader.header().world_to_meter
    }

    /// Read one tick as flat per-channel byte buffers.
    ///
    /// Buffers are shaped for `numpy.frombuffer` followed by `foreach_set`,
    /// which is the only path into Blender point attributes that avoids a
    /// per-element Python round trip.
    fn read_tick<'py>(&mut self, py: Python<'py>, tick: u64) -> PyResult<Bound<'py, PyDict>> {
        self.reader
            .read_tick(tick, &mut self.scratch)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;

        let n = self.scratch.len();
        // `position` is 3 floats per agent: Blender's `position` attribute is
        // FLOAT_VECTOR, and the simulation is planar, so z is always 0.
        let mut position = Vec::with_capacity(n * 12);
        let mut orientation = Vec::with_capacity(n * 4);
        let mut agent_id_lo = Vec::with_capacity(n * 4);
        let mut agent_id_hi = Vec::with_capacity(n * 4);
        let mut flags = Vec::with_capacity(n * 4);
        let mut clip_index = Vec::with_capacity(n * 4);
        let mut phase = Vec::with_capacity(n * 4);
        let mut playback_rate = Vec::with_capacity(n * 4);
        let mut render_tier = Vec::with_capacity(n * 4);

        for r in &self.scratch {
            position.extend_from_slice(&r.position[0].to_le_bytes());
            position.extend_from_slice(&r.position[1].to_le_bytes());
            position.extend_from_slice(&0.0f32.to_le_bytes());
            orientation.extend_from_slice(&r.orientation.to_le_bytes());
            // Split rather than narrow: a truncated stable ID is not stable.
            agent_id_lo.extend_from_slice(&((r.agent_id & 0xffff_ffff) as u32).to_le_bytes());
            agent_id_hi.extend_from_slice(&((r.agent_id >> 32) as u32).to_le_bytes());
            flags.extend_from_slice(&r.flags.to_le_bytes());
            clip_index.extend_from_slice(&(r.clip_index as u32).to_le_bytes());
            phase.extend_from_slice(&r.phase.to_le_bytes());
            playback_rate.extend_from_slice(&r.playback_rate.to_le_bytes());
            render_tier.extend_from_slice(&(r.render_tier as u32).to_le_bytes());
        }

        let out = PyDict::new(py);
        out.set_item("position", PyBytes::new(py, &position))?;
        out.set_item("orientation", PyBytes::new(py, &orientation))?;
        out.set_item("agent_id_lo", PyBytes::new(py, &agent_id_lo))?;
        out.set_item("agent_id_hi", PyBytes::new(py, &agent_id_hi))?;
        out.set_item("flags", PyBytes::new(py, &flags))?;
        out.set_item("clip_index", PyBytes::new(py, &clip_index))?;
        out.set_item("phase", PyBytes::new(py, &phase))?;
        out.set_item("playback_rate", PyBytes::new(py, &playback_rate))?;
        out.set_item("render_tier", PyBytes::new(py, &render_tier))?;
        Ok(out)
    }
}

#[pymodule]
fn blender_crowd_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<Trace>()?;
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p crowd-blender`
Expected: builds clean. (If pyo3 0.27's API differs, fix the call sites; do not change the module's public Python surface.)

- [ ] **Step 5: Write the wheel build script**

Create `scripts/build-wheel.sh`:

```bash
#!/usr/bin/env bash
# Build the native module as an abi3 wheel for the Blender extension.
#
# abi3 rather than a cp313-specific wheel: Blender 5.2 resolves an `abi3` tag
# as "any CPython 3" and lets it override the `cp3xx` tag, so one wheel keeps
# working if a future Blender ships a newer CPython. (This was broken in 4.2
# and 4.3 -- see blender issue #130561 -- and is fixed in 5.2.)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/addon/blender_crowd/wheels"

command -v maturin >/dev/null || { echo "maturin is required (mise install)" >&2; exit 1; }

# Blender's `extension build` fails with a bare Errno 2 rather than creating
# a missing output directory, so every directory is made explicitly.
mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/*.whl

maturin build \
    --release \
    --manifest-path "$REPO_ROOT/crates/crowd-blender/Cargo.toml" \
    --out "$OUT_DIR"

echo "built: $(ls "$OUT_DIR"/*.whl)"
```

- [ ] **Step 6: Build the wheel and check its filename**

```bash
chmod +x scripts/build-wheel.sh
scripts/build-wheel.sh
ls addon/blender_crowd/wheels/
```

Expected: a file named `blender_crowd_native-0.1.0-cp311-abi3-macosx_11_0_arm64.whl`. The `abi3` in the ABI tag position is the part that matters.

- [ ] **Step 7: Commit**

Create `.gitignore` entry so built wheels are not committed — append to the repo's `.gitignore`:

```
addon/blender_crowd/wheels/*.whl
```

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock mise.toml crates/crowd-blender scripts/build-wheel.sh .gitignore
git commit -m "Add the PyO3 bridge module and its abi3 wheel build"
```

---

### Task 6: The Blender extension package

**Files:**
- Create: `addon/blender_crowd/blender_manifest.toml`
- Create: `addon/blender_crowd/__init__.py`
- Create: `addon/blender_crowd/trace_playback.py`
- Create: `addon/blender_crowd/operators.py`

**Interfaces:**
- Consumes: the Python surface of `blender_crowd_native` from Task 5.
- Produces:
  - `trace_playback.TracePlayback` — `open(path)`, `sync_to_tick(tick)`, `agent_count`, `tick_count`
  - `trace_playback.ensure_point_cloud(name, agent_count) -> bpy.types.Object`
  - Operator `crowd.load_trace` with a `filepath` string property.

- [ ] **Step 1: Write the manifest**

Create `addon/blender_crowd/blender_manifest.toml`. Replace `WHEEL_FILENAME` in the `wheels` list with the actual filename produced by `scripts/build-wheel.sh`; Task 7's script rewrites this line automatically.

```toml
schema_version = "1.0.0"

id = "blender_crowd"
version = "0.1.0"
name = "Blender Crowd"
tagline = "Deterministic crowd simulation playback from a baked trace"
maintainer = "Blender Crowd contributors"
type = "add-on"

license = ["SPDX:GPL-3.0-or-later"]
blender_version_min = "5.2.0"

platforms = ["macos-arm64"]
wheels = ["./wheels/WHEEL_FILENAME"]
```

- [ ] **Step 2: Write the playback module**

Create `addon/blender_crowd/trace_playback.py`:

```python
"""Push one tick of a baked trace into a Blender point cloud.

This module is presentation only. It never simulates and never decides
anything about agent behaviour: it reads what the Rust side baked and moves
it into attributes a Geometry Nodes tree can instance.
"""

import numpy as np

import bpy

import blender_crowd_native

# Point attributes written every tick. Blender point attributes are 32-bit,
# so the 64-bit stable agent ID is carried as two halves rather than being
# narrowed -- a truncated stable ID is not a stable ID.
_INT_CHANNELS = (
    "agent_id_lo",
    "agent_id_hi",
    "flags",
    "clip_index",
    "render_tier",
)
_FLOAT_CHANNELS = ("orientation", "phase", "playback_rate")


def ensure_point_cloud(name, agent_count):
    """Return an object holding a point cloud of exactly `agent_count` points."""
    data = bpy.data.pointclouds.get(name)
    if data is None:
        data = bpy.data.pointclouds.new(name)
    # The API is `resize`, not `add`.
    data.resize(agent_count)

    for channel in _INT_CHANNELS:
        if channel not in data.attributes:
            data.attributes.new(channel, "INT", "POINT")
    for channel in _FLOAT_CHANNELS:
        if channel not in data.attributes:
            data.attributes.new(channel, "FLOAT", "POINT")

    obj = bpy.data.objects.get(name)
    if obj is None or obj.data is not data:
        obj = bpy.data.objects.new(name, data)
        bpy.context.scene.collection.objects.link(obj)
    return obj


class TracePlayback:
    """A trace file bound to a point-cloud object."""

    def __init__(self, path, object_name="crowd"):
        self._trace = blender_crowd_native.Trace(path)
        self._object = ensure_point_cloud(object_name, self._trace.agent_count)
        self._data = self._object.data

    @property
    def agent_count(self):
        return self._trace.agent_count

    @property
    def tick_count(self):
        return self._trace.tick_count

    @property
    def object(self):
        return self._object

    def sync_to_tick(self, tick):
        """Write one tick's channels into the point cloud's attributes."""
        buffers = self._trace.read_tick(tick)

        self._data.attributes["position"].data.foreach_set(
            "vector", np.frombuffer(buffers["position"], dtype=np.float32)
        )
        for channel in _FLOAT_CHANNELS:
            self._data.attributes[channel].data.foreach_set(
                "value", np.frombuffer(buffers[channel], dtype=np.float32)
            )
        for channel in _INT_CHANNELS:
            self._data.attributes[channel].data.foreach_set(
                "value", np.frombuffer(buffers[channel], dtype=np.int32)
            )
        self._data.update_tag()
```

- [ ] **Step 3: Write the operator and registration**

Create `addon/blender_crowd/operators.py`:

```python
"""Blender operators for the crowd bridge."""

import bpy
from bpy.props import StringProperty
from bpy.types import Operator

# Relative import: extensions are imported as `bl_ext.user_default.blender_crowd`,
# so an absolute `from blender_crowd.x import y` fails with "package not found".
from .trace_playback import TracePlayback

_ACTIVE = {}


class CROWD_OT_load_trace(Operator):
    bl_idname = "crowd.load_trace"
    bl_label = "Load Crowd Trace"
    bl_description = "Load a baked crowd trace and bind it to a point cloud"

    filepath: StringProperty(subtype="FILE_PATH")

    def execute(self, context):
        try:
            playback = TracePlayback(self.filepath)
        except OSError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        _ACTIVE["playback"] = playback
        playback.sync_to_tick(0)
        self.report(
            {"INFO"},
            "Loaded {} agents, {} ticks".format(
                playback.agent_count, playback.tick_count
            ),
        )
        return {"FINISHED"}


def active_playback():
    """Return the loaded playback, or None."""
    return _ACTIVE.get("playback")


_CLASSES = (CROWD_OT_load_trace,)


def register():
    for cls in _CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(_CLASSES):
        bpy.utils.unregister_class(cls)
    _ACTIVE.clear()
```

Create `addon/blender_crowd/__init__.py`:

```python
"""Blender Crowd -- deterministic crowd playback from a baked trace.

Targets Blender 5.2 LTS only. All imports inside this package are relative:
the extension is imported as `bl_ext.user_default.blender_crowd`, so absolute
imports of the package name fail.
"""

from . import operators


def register():
    operators.register()


def unregister():
    operators.unregister()
```

- [ ] **Step 4: Commit**

```bash
git add addon
git commit -m "Add the Blender Crowd extension package and trace playback"
```

---

### Task 7: Clean-install runner (M0 acceptance criterion 5)

**Files:**
- Create: `scripts/blender-install-test.sh`
- Create: `tests/blender/test_install.py`

**Interfaces:**
- Consumes: `scripts/build-wheel.sh` (Task 5), `addon/blender_crowd/` (Task 6).
- Produces: an exit-zero-on-pass runner.

- [ ] **Step 1: Write the in-Blender assertion script**

Create `tests/blender/test_install.py`:

```python
"""Assert the native module loads from a clean Blender install.

Runs inside Blender via `--python`. Exits non-zero on failure so the calling
shell script fails loudly. This automates M0 acceptance criterion 5: "Blender
loads the native module from a clean supported install with no absolute links
to a contributor environment."
"""

import os
import sys

import addon_utils

EXTENSION = "bl_ext.user_default.blender_crowd"


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def main():
    addon_utils.enable(EXTENSION, default_set=True)

    try:
        import blender_crowd_native
    except ImportError as error:
        fail("native module did not import: {}".format(error))

    origin = os.path.realpath(blender_crowd_native.__file__)
    print("module origin: {}".format(origin))
    print("module version: {}".format(blender_crowd_native.__version__))

    # It must have been installed, not picked up from the working checkout.
    if "extensions" not in origin.split(os.sep):
        fail("module did not load from the Blender extensions directory")

    repo_root = os.path.realpath(os.environ["CROWD_REPO_ROOT"])
    if origin.startswith(repo_root + os.sep):
        fail("module resolved into the source checkout at {}".format(repo_root))

    if not hasattr(blender_crowd_native, "Trace"):
        fail("native module has no Trace class")

    print("PASS: native module loaded from a clean install")


main()
```

- [ ] **Step 2: Write the shell runner**

Create `scripts/blender-install-test.sh`:

```bash
#!/usr/bin/env bash
# Build, install, and load the extension in a clean Blender 5.2 session.
#
# Automates M0 acceptance criterion 5. Every step is headless so this can run
# unattended.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
DIST_DIR="$REPO_ROOT/dist"
PKG="user_default.blender_crowd"

command -v "$BLENDER" >/dev/null 2>&1 || [ -x "$BLENDER" ] || {
    echo "Blender not found at $BLENDER (override with BLENDER=...)" >&2
    exit 1
}

"$REPO_ROOT/scripts/build-wheel.sh"

WHEEL_NAME="$(basename "$(ls "$REPO_ROOT/addon/blender_crowd/wheels/"*.whl)")"
# Keep the manifest's wheel entry in step with what was just built.
python3 - "$REPO_ROOT/addon/blender_crowd/blender_manifest.toml" "$WHEEL_NAME" <<'PY'
import re
import sys

path, wheel = sys.argv[1], sys.argv[2]
with open(path) as handle:
    text = handle.read()
text = re.sub(r'wheels = \["\./wheels/[^"]*"\]',
              'wheels = ["./wheels/{}"]'.format(wheel), text)
with open(path, "w") as handle:
    handle.write(text)
print("manifest wheel entry: {}".format(wheel))
PY

# `extension build` fails with a bare Errno 2 rather than creating this.
mkdir -p "$DIST_DIR"
rm -f "$DIST_DIR"/blender_crowd-*.zip

"$BLENDER" --command extension validate "$REPO_ROOT/addon/blender_crowd"
"$BLENDER" --command extension build \
    --source-dir "$REPO_ROOT/addon/blender_crowd" \
    --output-dir "$DIST_DIR"

ZIP="$(ls "$DIST_DIR"/blender_crowd-*.zip)"

# Remove any prior install so this is genuinely a clean-install test.
# `extension remove` takes repo.pkg_id as ONE positional, not a --repo flag.
"$BLENDER" --command extension remove "$PKG" >/dev/null 2>&1 || true

"$BLENDER" --command extension install-file --repo user_default --enable "$ZIP"

CROWD_REPO_ROOT="$REPO_ROOT" "$BLENDER" -b --python "$REPO_ROOT/tests/blender/test_install.py"

echo "install test: PASS"
```

- [ ] **Step 3: Run it**

```bash
chmod +x scripts/blender-install-test.sh
scripts/blender-install-test.sh
```

Expected: ends with `PASS: native module loaded from a clean install` then `install test: PASS`.

If it fails with a namespace-package error mentioning `__path__=_NamespacePath`, the ZIP layout is wrong: `__init__.py` and `blender_manifest.toml` must be at the archive root, not nested. Verify with:

```bash
python3 -c "import zipfile,glob; ns=zipfile.ZipFile(glob.glob('dist/blender_crowd-*.zip')[0]).namelist(); assert '__init__.py' in ns and 'blender_manifest.toml' in ns, ns"
```

- [ ] **Step 4: Commit**

```bash
git add scripts/blender-install-test.sh tests/blender/test_install.py addon/blender_crowd/blender_manifest.toml
git commit -m "Add the headless clean-install and native module load runner"
```

---

### Task 8: Playback runner (M0 acceptance criterion 6)

**Files:**
- Create: `tests/blender/test_playback.py`
- Create: `scripts/blender-playback-test.sh`

**Interfaces:**
- Consumes: `TracePlayback` (Task 6), the `--trace` flag (Task 4), the install runner (Task 7).
- Produces: an exit-zero-on-pass runner that prints simulation and playback costs separately.

- [ ] **Step 1: Write the in-Blender playback assertion**

Create `tests/blender/test_playback.py`:

```python
"""Play a 1,000-agent trace back through Geometry Nodes point attributes.

Runs inside Blender via `--python`. Automates M0 acceptance criterion 6:
Blender plays 1,000 cached point transforms with stable IDs, and simulation
and playback costs are reported separately.
"""

import os
import sys
import time

import numpy as np

import addon_utils
import bpy

EXTENSION = "bl_ext.user_default.blender_crowd"
EXPECTED_AGENTS = 1000


def fail(message):
    print("FAIL: {}".format(message))
    sys.exit(1)


def main():
    addon_utils.enable(EXTENSION, default_set=True)
    from bl_ext.user_default.blender_crowd.trace_playback import TracePlayback

    trace_path = os.environ["CROWD_TRACE_PATH"]
    playback = TracePlayback(trace_path)

    if playback.agent_count != EXPECTED_AGENTS:
        fail("expected {} agents, got {}".format(EXPECTED_AGENTS, playback.agent_count))

    data = playback.object.data
    positions = np.empty(playback.agent_count * 3, dtype=np.float32)
    ids_lo = np.empty(playback.agent_count, dtype=np.int32)
    ids_hi = np.empty(playback.agent_count, dtype=np.int32)

    playback.sync_to_tick(0)
    data.attributes["agent_id_lo"].data.foreach_get("value", ids_lo)
    data.attributes["agent_id_hi"].data.foreach_get("value", ids_hi)
    first_ids = (ids_lo.copy(), ids_hi.copy())

    # Time every tick. This measures Blender-side playback only: the
    # simulation cost is reported separately by the calling script, because
    # conflating them is exactly what M0 criterion 6 forbids.
    start = time.perf_counter()
    for tick in range(playback.tick_count):
        playback.sync_to_tick(tick)
    elapsed = time.perf_counter() - start

    # Stable IDs must not drift across playback.
    data.attributes["agent_id_lo"].data.foreach_get("value", ids_lo)
    data.attributes["agent_id_hi"].data.foreach_get("value", ids_hi)
    playback.sync_to_tick(0)
    data.attributes["agent_id_lo"].data.foreach_get("value", ids_lo)
    data.attributes["agent_id_hi"].data.foreach_get("value", ids_hi)
    if not np.array_equal(ids_lo, first_ids[0]) or not np.array_equal(ids_hi, first_ids[1]):
        fail("agent IDs changed across playback")

    # Positions must match the Rust reader exactly, not approximately.
    import blender_crowd_native

    reference = blender_crowd_native.Trace(trace_path).read_tick(0)
    expected = np.frombuffer(reference["position"], dtype=np.float32)
    data.attributes["position"].data.foreach_get("vector", positions)
    if not np.array_equal(positions, expected):
        fail("point positions do not match the Rust reader")

    per_tick_ms = (elapsed / max(playback.tick_count, 1)) * 1000.0
    print("agents: {}".format(playback.agent_count))
    print("ticks: {}".format(playback.tick_count))
    print("blender_playback_total_s: {:.4f}".format(elapsed))
    print("blender_playback_per_tick_ms: {:.4f}".format(per_tick_ms))
    print("PASS: 1,000-point playback with stable IDs")


main()
```

- [ ] **Step 2: Write the shell runner**

Create `scripts/blender-playback-test.sh`:

```bash
#!/usr/bin/env bash
# Bake a 1,000-agent trace, then play it back in Blender with the simulation
# process already exited.
#
# Automates M0 acceptance criterion 6. Simulation and playback costs are
# printed separately and must never be summed into one number.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
OUT_DIR="$REPO_ROOT/benchmarks/reports"
SCENE="${SCENE:-crossing}"
AGENTS="${AGENTS:-1000}"

mkdir -p "$OUT_DIR"

echo "== simulation =="
# Not recorded with --svg or --frames, so this run's timing is a real
# measurement rather than a sampling-inflated one.
SIM_START=$(python3 -c "import time; print(time.perf_counter())")
cargo run --release -p crowd-bench -- run \
    --scene "$SCENE" --agents "$AGENTS" --trace --out "$OUT_DIR"
SIM_END=$(python3 -c "import time; print(time.perf_counter())")
python3 -c "print('simulation_wall_s: {:.4f}'.format($SIM_END - $SIM_START))"

TRACE="$OUT_DIR/$SCENE-$AGENTS.crowdtrace"
[ -f "$TRACE" ] || { echo "trace not written to $TRACE" >&2; exit 1; }
echo "trace_bytes: $(wc -c < "$TRACE" | tr -d ' ')"

echo "== blender playback (simulation process has exited) =="
CROWD_TRACE_PATH="$TRACE" "$BLENDER" -b --python "$REPO_ROOT/tests/blender/test_playback.py"

echo "playback test: PASS"
```

- [ ] **Step 3: Run it**

```bash
chmod +x scripts/blender-playback-test.sh
scripts/blender-install-test.sh   # extension must be installed first
scripts/blender-playback-test.sh
```

Expected: ends with `PASS: 1,000-point playback with stable IDs` then `playback test: PASS`, having printed `simulation_wall_s`, `trace_bytes`, `blender_playback_total_s`, and `blender_playback_per_tick_ms` as separate numbers.

- [ ] **Step 4: Commit**

```bash
git add scripts/blender-playback-test.sh tests/blender/test_playback.py
git commit -m "Add the 1,000-point Blender playback runner with split costs"
```

---

### Task 9: Geometry Nodes instancing asset

**Files:**
- Create: `addon/blender_crowd/geometry_nodes.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `addon/blender_crowd/__init__.py`

**Interfaces:**
- Consumes: `ensure_point_cloud` from Task 6.
- Produces: `geometry_nodes.ensure_crowd_node_group() -> bpy.types.NodeTree` and `geometry_nodes.attach(obj) -> bpy.types.Modifier`.

**Note:** The node group is built in Python rather than shipped as a `.blend`, so it stays reviewable in diffs and needs no binary fixture. It instances a cone on each point and rotates it by the `orientation` attribute — enough to prove the attribute reaches Geometry Nodes, and no more. Geometry Nodes is presentation, never authoritative.

- [ ] **Step 1: Write the node group builder**

Create `addon/blender_crowd/geometry_nodes.py`:

```python
"""Build the crowd instancing node group.

Built in Python rather than shipped as a .blend so it stays reviewable in
diffs and needs no binary fixture. This layer is presentation only: it reads
attributes the Rust side baked and never decides anything.
"""

import bpy

NODE_GROUP_NAME = "CrowdInstances"
MODIFIER_NAME = "CrowdInstances"


def ensure_crowd_node_group():
    """Return the crowd instancing node group, creating it if absent."""
    existing = bpy.data.node_groups.get(NODE_GROUP_NAME)
    if existing is not None:
        return existing

    group = bpy.data.node_groups.new(NODE_GROUP_NAME, "GeometryNodeTree")
    group.interface.new_socket(
        "Geometry", in_out="INPUT", socket_type="NodeSocketGeometry"
    )
    group.interface.new_socket(
        "Geometry", in_out="OUTPUT", socket_type="NodeSocketGeometry"
    )

    nodes = group.nodes
    links = group.links

    group_in = nodes.new("NodeGroupInput")
    group_in.location = (-600, 0)
    group_out = nodes.new("NodeGroupOutput")
    group_out.location = (600, 0)

    # A cone is a stand-in for a character: it has an obvious facing
    # direction, so a wrong orientation is visible at a glance.
    cone = nodes.new("GeometryNodeMeshCone")
    cone.location = (-300, -200)
    cone.inputs["Radius Bottom"].default_value = 0.25
    cone.inputs["Depth"].default_value = 1.7

    instance = nodes.new("GeometryNodeInstanceOnPoints")
    instance.location = (0, 0)

    orientation = nodes.new("GeometryNodeInputNamedAttribute")
    orientation.location = (-300, 200)
    orientation.data_type = "FLOAT"
    orientation.inputs["Name"].default_value = "orientation"

    combine = nodes.new("ShaderNodeCombineXYZ")
    combine.location = (-150, 200)

    links.new(group_in.outputs[0], instance.inputs["Points"])
    links.new(cone.outputs["Mesh"], instance.inputs["Instance"])
    links.new(orientation.outputs["Attribute"], combine.inputs["Z"])
    links.new(combine.outputs["Vector"], instance.inputs["Rotation"])
    links.new(instance.outputs["Instances"], group_out.inputs[0])

    return group


def attach(obj):
    """Attach the crowd node group to `obj`, reusing an existing modifier."""
    modifier = obj.modifiers.get(MODIFIER_NAME)
    if modifier is None:
        modifier = obj.modifiers.new(MODIFIER_NAME, "NODES")
    modifier.node_group = ensure_crowd_node_group()
    return modifier
```

- [ ] **Step 2: Attach it when a trace loads**

Modify `addon/blender_crowd/operators.py`. Add to the imports:

```python
from . import geometry_nodes
```

and in `CROWD_OT_load_trace.execute`, immediately after `_ACTIVE["playback"] = playback`:

```python
        geometry_nodes.attach(playback.object)
```

- [ ] **Step 3: Extend the playback test to assert instancing**

Modify `tests/blender/test_playback.py`. After the position comparison and before the metrics printing, add:

```python
    from bl_ext.user_default.blender_crowd import geometry_nodes

    geometry_nodes.attach(playback.object)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = playback.object.evaluated_get(depsgraph)
    instance_count = sum(
        1 for instance in depsgraph.object_instances if instance.is_instance
    )
    print("instances: {}".format(instance_count))
    if instance_count != EXPECTED_AGENTS:
        fail("expected {} instances, got {}".format(EXPECTED_AGENTS, instance_count))
```

- [ ] **Step 4: Run the runners**

```bash
scripts/blender-install-test.sh
scripts/blender-playback-test.sh
```

Expected: prints `instances: 1000` and still ends with `playback test: PASS`.

If a node input name in Step 1 does not exist on Blender 5.2, print the real names with:

```bash
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup --python-expr \
  'import bpy; g=bpy.data.node_groups.new("t","GeometryNodeTree"); n=g.nodes.new("GeometryNodeMeshCone"); print([i.name for i in n.inputs])'
```

and correct the script to match. Do not guess.

- [ ] **Step 5: Commit**

```bash
git add addon/blender_crowd/geometry_nodes.py addon/blender_crowd/operators.py tests/blender/test_playback.py
git commit -m "Instance 1,000 baked agents through a Geometry Nodes group"
```

---

### Task 10: Contract amendment, docs, and the dated report

**Files:**
- Modify: `docs/blender-crowd-1.0.md:4`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Create: `docs/benchmarks/2026-08-07-blender-bridge.md`

- [ ] **Step 1: Amend the canonical contract's target host**

Modify `docs/blender-crowd-1.0.md` line 4. Change:

```
Target host: Blender 4.x LTS-compatible API surface
```

to:

```
Target host: Blender 5.2 LTS (bundled CPython 3.13). Widening the support
matrix is an M3 decision requiring measured evidence, not a default.
```

- [ ] **Step 2: Add copy-ready commands to README.md and AGENTS.md**

Add to the development command block in both `README.md` and `AGENTS.md` (milestone rule 8 requires exact, copy-ready commands whenever a runner is checked in):

```sh
scripts/build-wheel.sh                     # abi3 wheel -> addon/blender_crowd/wheels/
scripts/blender-install-test.sh            # clean install + native module load
scripts/blender-playback-test.sh           # 1,000-point playback, costs reported separately

cargo run --release -p crowd-bench -- run --scene crossing --agents 1000 --trace
```

Also note the Blender requirement in `README.md`'s Development section:

```
The Blender runners require Blender 5.2 LTS at
/Applications/Blender.app/Contents/MacOS/Blender (override with BLENDER=...).
```

- [ ] **Step 3: Mirror the guidance into CLAUDE.md**

Add the same command block to `CLAUDE.md`'s "Build, Test, and Development Commands" section, and add under it:

```
The addon package uses relative imports throughout: extensions are imported
as `bl_ext.user_default.blender_crowd`, so absolute imports of the package
name fail. Bundled wheels unpack into a site-packages directory shared by all
installed extensions, so the native module name `blender_crowd_native` must
stay distinctive.
```

- [ ] **Step 4: Write the dated report**

Create `docs/benchmarks/2026-08-07-blender-bridge.md`. Fill every number from an actual run — do not copy the placeholder text below without replacing it.

```markdown
# Blender bridge and native packaging — M0 item 6

Date: 2026-08-07
Milestone: [M0 — Proving grounds](../milestones/M0-proving-grounds.md)
Design: [Blender bridge slice](../superpowers/specs/2026-08-07-blender-bridge-slice-design.md)

## Environment

| | |
|---|---|
| CPU | (fill from `sysctl -n machdep.cpu.brand_string`) |
| OS | (fill from `sw_vers`) |
| Blender | 5.2.0 LTS, hash fbe6228777e7 |
| Bundled CPython | 3.13.13 |
| Rust | 1.94.1 |
| Wheel | (filename from scripts/build-wheel.sh) |

## Results

| Measure | Value |
|---|---|
| Agents | 1000 |
| Ticks | (fill) |
| Simulation wall time | (fill) |
| Trace size on disk | (fill) |
| Blender playback total | (fill) |
| Blender playback per tick | (fill) |
| Instances evaluated | 1000 |

Simulation and playback costs are listed separately and must not be summed.

## Acceptance criteria addressed

- M0 criterion 5 (clean install, no contributor-environment links):
  `scripts/blender-install-test.sh`.
- M0 criterion 6 (1,000 cached point transforms, stable IDs, separated costs):
  `scripts/blender-playback-test.sh`.

## Known limitations and unsupported claims

- macOS arm64 only. No Linux or Windows wheel was built or tested.
- Single machine. No claim is made about any other hardware.
- Trace v0 is not the cache format: it has no chunking, quantization,
  checksums, or cancellation, and cache v0 must still decide all four.
- Navigation still uses the waypoint stand-in, which M0 forbids treating as
  production navigation.
- The instanced cone is a stand-in, not a character asset. No armature
  evaluation or render cost was measured.

## Next gate

M0 items 4 (tiled navigation), 5 (cache v0), and 7 (Python/Rust facade)
remain open. M1 stays blocked until they close.
```

- [ ] **Step 5: Final verification**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/blender-install-test.sh
scripts/blender-playback-test.sh
git diff --check
```

Expected: all clean, both Blender runners PASS.

- [ ] **Step 6: Commit**

```bash
git add docs README.md AGENTS.md CLAUDE.md
git commit -m "Record the Blender bridge slice results and amend the target host"
```

---

## Plan-level definition of done

- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `scripts/blender-install-test.sh` and `scripts/blender-playback-test.sh` both pass from a clean install.
- The dated report exists with real numbers and an honest limitations section.
- `docs/blender-crowd-1.0.md` line 4 names Blender 5.2 LTS.
- `README.md`, `AGENTS.md`, and `CLAUDE.md` carry copy-ready commands for every checked-in runner.

## Stop conditions

Stop and record a failed gate if the native module cannot load from a clean install without linking to the checkout, if wheel name collisions cannot be avoided, or if 1,000-point playback is too costly to make the 1K vertical slice credible. Do not proceed to cache v0 or navigation on an unresolved bridge failure, and do not start M1 to route around it.
