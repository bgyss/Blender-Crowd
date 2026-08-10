//! Versioned, recoverable crowd cache.

mod agents;
mod checksum;
mod codec;
mod defaults;
mod error;
mod manifest;
mod reader;
mod writer;

pub use agents::AgentStatic;

pub use checksum::{content_hash, payload_checksum};
pub use codec::{
    decode_chunk, encode_chunk, CodecError, DecodedChunk, EncodedChunk, Frame, FrameRecord,
    PositionEncoding, CHUNK_HEADER_BYTES,
};
pub use defaults::{CacheDefaults, CACHE_V1_DEFAULTS};
pub use error::CacheError;
pub use manifest::{
    CacheManifestV1, CacheStatus, ChannelDef, ChunkDef, FileDef, ManifestError, ScalarType,
    CACHE_SCHEMA_VERSION,
};
pub use reader::{CacheReader, RecoveryInspector, RecoveryReport};
pub use writer::{BakeSpec, CacheWriter, CancelToken};
