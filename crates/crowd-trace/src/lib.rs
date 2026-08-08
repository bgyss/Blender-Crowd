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
pub mod record;

pub use header::{Header, FORMAT_VERSION, HEADER_BYTES, MAGIC};
pub use record::{AgentRecord, FLAG_ACTIVE, FLAG_ARRIVED, RECORD_BYTES};

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
                write!(
                    f,
                    "trace truncated: expected {expected} bytes, found {found}"
                )
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
