//! Versioned, recoverable crowd cache.

mod manifest;

pub use manifest::{
    CacheManifestV1, CacheStatus, ChannelDef, ChunkDef, ManifestError, ScalarType,
    CACHE_SCHEMA_VERSION,
};
