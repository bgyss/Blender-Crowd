//! Versioned, recoverable crowd cache.

mod agents;
mod behavior_events;
mod checksum;
mod codec;
mod defaults;
mod error;
mod manifest;
mod override_layer;
mod reader;
mod writer;

pub use agents::AgentStatic;
pub use behavior_events::{BehaviorEventKindV1, BehaviorEventV1, BEHAVIOR_EVENTS_SCHEMA_VERSION};

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
pub use override_layer::{
    compose_frame, compose_frame_v2, validate_layers, ComposedFrame, ComposedFrameV2,
    ComposedRecord, ComposedRecordV2, LocalResimulationRecordV2, OverrideConflictV2,
    OverrideEditV2, OverrideError, OverrideLayerV1, OverrideLayerV2, OverrideOperation,
    OverrideV2Error, TransformOverride, OVERRIDE_SCHEMA_VERSION,
};
pub use reader::{CacheReader, RecoveryInspector, RecoveryReport};
pub use writer::{BakeSpec, CacheWriter, CancelToken};
