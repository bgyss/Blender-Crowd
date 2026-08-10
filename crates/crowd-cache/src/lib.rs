//! Versioned, recoverable crowd cache.

mod checksum;
mod codec;
mod manifest;

pub use checksum::{content_hash, payload_checksum};
pub use codec::{
    decode_chunk, encode_chunk, CodecError, DecodedChunk, EncodedChunk, Frame, FrameRecord,
    PositionEncoding, CHUNK_HEADER_BYTES,
};
pub use manifest::{
    CacheManifestV1, CacheStatus, ChannelDef, ChunkDef, ManifestError, ScalarType,
    CACHE_SCHEMA_VERSION,
};
