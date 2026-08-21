//! Versioned, recoverable crowd cache.

mod agents;
mod behavior_events;
mod checksum;
mod codec;
mod defaults;
mod error;
mod interaction_layers;
mod layout;
mod manifest;
mod override_layer;
mod reader;
mod writer;

pub use agents::AgentStatic;
pub use behavior_events::{
    compact_behavior_events, BehaviorEventCompactor, BehaviorEventKindV1, BehaviorEventV1,
    BEHAVIOR_EVENTS_SCHEMA_VERSION,
};

pub use checksum::{content_hash, payload_checksum};
pub use codec::{
    decode_chunk, encode_chunk, CodecError, DecodedChunk, EncodedChunk, Frame, FrameRecord,
    PositionEncoding, CHUNK_HEADER_BYTES,
};
pub use defaults::{CacheDefaults, CACHE_V1_DEFAULTS};
pub use error::CacheError;
pub use interaction_layers::{
    compose_interaction_frame_v1, AnimationEditV1, AnimationLayerV1, FallbackClipV1,
    InteractionLayerError, INTERACTION_LAYER_SCHEMA_VERSION,
};
pub use layout::{
    compose_layout_frame_v1, extract_procedural_instances_v1, invalidate_dependents_v1,
    mark_dependents_stale_v1, migrate_override_layer_v1, read_usda_crowd_profile_v1,
    resimulate_local_kinematic_v1, simulate_physics_handoff_v1, validate_layout_layers_v1,
    write_usda_crowd_profile_v1, ComposedLayoutFrameV1, LayerInvalidationV1, LayerKindV1,
    LayerTargetV1, LayoutConflictV1, LayoutEditV1, LayoutErrorV1, LayoutLayerV1, LayoutRecordV1,
    LocalResimulationRequestV1, LocalResimulationV1, PhysicsHandoffSpecV1, PhysicsSampleV1,
    ProceduralInstanceV1, ProceduralPrototypeV1, UsdCrowdImportV1, LAYOUT_LAYER_SCHEMA_VERSION,
    USD_CROWD_PROFILE_VERSION,
};
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
