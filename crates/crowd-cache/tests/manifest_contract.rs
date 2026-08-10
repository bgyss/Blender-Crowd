use crowd_cache::{CacheManifestV1, CacheStatus, ChunkDef, ManifestError};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn manifest(status: CacheStatus) -> CacheManifestV1 {
    CacheManifestV1 {
        schema_version: 1,
        engine_version: "0.1.0".to_owned(),
        project_id: "project:test".to_owned(),
        source_hash: "00".repeat(32),
        tick_start: 0,
        tick_end: 119,
        ticks_per_second: 30,
        agent_count: 1_000,
        channels: Vec::new(),
        chunks: vec![ChunkDef {
            path: "frames/000000-000059.chunk".to_owned(),
            tick_start: 0,
            tick_end: 59,
            byte_len: 4_096,
            checksum: 0xe306_9283,
            complete: true,
        }],
        status,
        last_complete_tick: None,
    }
}

#[test]
fn complete_manifest_rejects_an_unfinished_declared_chunk() {
    let mut manifest = manifest(CacheStatus::Complete);
    manifest.chunks[0].complete = false;

    assert_eq!(
        manifest.validate(),
        Err(ManifestError::IncompleteChunk { index: 0 })
    );
}

#[test]
fn canceled_manifest_requires_the_last_complete_tick() {
    let manifest = manifest(CacheStatus::Canceled);

    assert_eq!(
        manifest.validate(),
        Err(ManifestError::MissingLastCompleteTick)
    );
}

#[test]
fn canceled_manifest_accepts_a_recovery_boundary() {
    let mut manifest = manifest(CacheStatus::Canceled);
    manifest.last_complete_tick = Some(59);

    assert_eq!(manifest.validate(), Ok(()));
}

#[test]
fn unsupported_manifest_version_is_rejected() {
    let mut manifest = manifest(CacheStatus::Incomplete);
    manifest.schema_version = 2;

    assert_eq!(
        manifest.validate(),
        Err(ManifestError::UnsupportedVersion(2))
    );
}

#[test]
fn emitted_manifest_validates_against_the_checked_schema() {
    let schema_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/cache-manifest-v1.schema.json");
    let schema_text = fs::read_to_string(&schema_path).expect("checked cache manifest schema");
    let schema: serde_json::Value = serde_json::from_str(&schema_text).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let instance = serde_json::to_value(manifest(CacheStatus::Incomplete)).unwrap();

    if let Err(error) = validator.validate(&instance) {
        panic!("manifest did not validate: {error}");
    }

    let keys: BTreeSet<_> = instance
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "agent_count",
            "channels",
            "chunks",
            "engine_version",
            "last_complete_tick",
            "project_id",
            "schema_version",
            "source_hash",
            "status",
            "tick_end",
            "tick_start",
            "ticks_per_second",
        ])
    );
}
