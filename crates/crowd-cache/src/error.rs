use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::{CacheStatus, CodecError, ManifestError};

#[derive(Debug)]
pub enum CacheError {
    AlreadyExists(PathBuf),
    InvalidBakeSpec(&'static str),
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        message: String,
    },
    Manifest(ManifestError),
    NotComplete(CacheStatus),
    MissingAgentTable,
    AgentCountMismatch {
        expected: usize,
        found: usize,
    },
    AgentIdMismatch {
        slot: usize,
        expected: u64,
        found: u64,
    },
    DuplicateAgentId(u64),
    AgentTable {
        path: PathBuf,
        message: String,
    },
    MissingFile(PathBuf),
    FileLength {
        path: PathBuf,
        expected: u64,
        found: u64,
    },
    FileChecksum {
        path: PathBuf,
        expected: u32,
        found: u32,
    },
    Codec {
        path: PathBuf,
        source: CodecError,
    },
    NonSequentialTick {
        expected: u64,
        found: u64,
    },
    TickOutOfRange {
        requested: u64,
        start: u64,
        end: u64,
    },
    IncompleteBake {
        expected_last_tick: u64,
        found_last_tick: Option<u64>,
    },
    UnsafeRelativePath(String),
}

impl CacheError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(path) => write!(f, "cache path already exists: {}", path.display()),
            Self::InvalidBakeSpec(message) => write!(f, "invalid bake specification: {message}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Json { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Manifest(source) => write!(f, "invalid cache manifest: {source}"),
            Self::NotComplete(status) => write!(f, "cache is not complete: {status:?}"),
            Self::MissingAgentTable => write!(f, "cache agent table has not been written"),
            Self::AgentCountMismatch { expected, found } => {
                write!(f, "expected {expected} agents, found {found}")
            }
            Self::AgentIdMismatch {
                slot,
                expected,
                found,
            } => write!(
                f,
                "agent slot {slot} has stable ID {found}; expected {expected}"
            ),
            Self::DuplicateAgentId(id) => write!(f, "duplicate stable agent ID {id}"),
            Self::AgentTable { path, message } => write!(f, "{}: {message}", path.display()),
            Self::MissingFile(path) => write!(f, "missing cache file {}", path.display()),
            Self::FileLength {
                path,
                expected,
                found,
            } => write!(
                f,
                "{} has {found} bytes; manifest declares {expected}",
                path.display()
            ),
            Self::FileChecksum {
                path,
                expected,
                found,
            } => write!(
                f,
                "{} checksum {found:#010x}; manifest declares {expected:#010x}",
                path.display()
            ),
            Self::Codec { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NonSequentialTick { expected, found } => {
                write!(f, "expected tick {expected}, found tick {found}")
            }
            Self::TickOutOfRange {
                requested,
                start,
                end,
            } => {
                write!(f, "tick {requested} is outside {start}..={end}")
            }
            Self::IncompleteBake {
                expected_last_tick,
                found_last_tick,
            } => write!(
                f,
                "bake ended at {found_last_tick:?}; expected tick {expected_last_tick}"
            ),
            Self::UnsafeRelativePath(path) => write!(f, "unsafe cache-relative path {path}"),
        }
    }
}

impl Error for CacheError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Codec { source, .. } => Some(source),
            _ => None,
        }
    }
}
