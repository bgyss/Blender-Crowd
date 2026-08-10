use std::fs;
use std::path::Path;

use crowd_cache::{
    compose_frame, content_hash, AgentStatic, BakeSpec, CacheReader, CacheWriter, ChannelDef,
    Frame, FrameRecord, OverrideLayerV1, OverrideOperation, PositionEncoding, ScalarType,
    TransformOverride,
};
use tempfile::tempdir;

const TARGET_ID: u64 = 0x8000_0001_0000_0002;
const OTHER_ID: u64 = 42;

fn frame(tick: u64) -> Frame {
    Frame {
        records: vec![
            FrameRecord {
                agent_id: TARGET_ID,
                position: [tick as f32, 10.0],
                visible: true,
                ..FrameRecord::default()
            },
            FrameRecord {
                agent_id: OTHER_ID,
                position: [-5.0, tick as f32],
                visible: true,
                ..FrameRecord::default()
            },
        ],
    }
}

fn write_cache(path: &Path) {
    let mut writer = CacheWriter::create(
        path,
        BakeSpec {
            engine_version: "test".into(),
            project_id: "override-test".into(),
            source_hash: "00".repeat(32),
            tick_start: 0,
            tick_end: 90,
            ticks_per_second: 30,
            agent_count: 2,
            channels: vec![ChannelDef {
                name: "position".into(),
                scalar_type: ScalarType::F32,
                arity: 2,
                quantization_error: Some(0.0),
            }],
            chunk_ticks: 30,
            position_encoding: PositionEncoding::F32,
        },
    )
    .unwrap();
    writer
        .write_agents(&[
            AgentStatic {
                agent_id: TARGET_ID,
                population_id: 1,
                archetype_id: 1,
                variant_id: 1,
                base_scale: 1.0,
                spawn_ordinal: 0,
            },
            AgentStatic {
                agent_id: OTHER_ID,
                population_id: 1,
                archetype_id: 1,
                variant_id: 1,
                base_scale: 1.0,
                spawn_ordinal: 1,
            },
        ])
        .unwrap();
    for tick in 0..=90 {
        writer.push_tick(tick, frame(tick)).unwrap();
    }
    writer.finish().unwrap();
}

fn cache_hash(path: &Path) -> [u8; 32] {
    let reader = CacheReader::open_complete(path).unwrap();
    let mut bytes = fs::read(path.join("manifest.json")).unwrap();
    bytes.extend(fs::read(path.join(&reader.manifest().agents.path)).unwrap());
    for chunk in &reader.manifest().chunks {
        bytes.extend(fs::read(path.join(&chunk.path)).unwrap());
    }
    content_hash(&bytes)
}

fn additive_layer() -> OverrideLayerV1 {
    OverrideLayerV1 {
        schema_version: 1,
        layer_id: "hero-pin".into(),
        author: "M1 test".into(),
        created_at: "2026-08-10T00:00:00Z".into(),
        priority: 10,
        enabled: true,
        target_agent_id: TARGET_ID,
        tick_start: 30,
        tick_end: 60,
        operation: OverrideOperation::Additive,
        samples: (30..=60)
            .map(|tick| TransformOverride {
                tick,
                translation: [1.0, -2.0, 0.5],
            })
            .collect(),
    }
}

#[test]
fn one_agent_override_is_sparse_reversible_and_base_immutable() {
    let temp = tempdir().unwrap();
    let cache_path = temp.path().join("cache");
    write_cache(&cache_path);
    let base_hash = cache_hash(&cache_path);
    let reader = CacheReader::open_complete(&cache_path).unwrap();
    let layer = additive_layer();

    for tick in [0, 29, 30, 45, 60, 61, 90] {
        let base = reader.read_tick(tick).unwrap();
        let composed = compose_frame(&base, tick, std::slice::from_ref(&layer)).unwrap();
        let target = composed
            .records
            .iter()
            .find(|record| record.agent_id == TARGET_ID)
            .unwrap();
        let other = composed
            .records
            .iter()
            .find(|record| record.agent_id == OTHER_ID)
            .unwrap();
        let expected_target = if (30..=60).contains(&tick) {
            [tick as f32 + 1.0, 8.0, 0.5]
        } else {
            [tick as f32, 10.0, 0.0]
        };
        assert_eq!(target.position, expected_target, "target at tick {tick}");
        assert_eq!(other.position, [-5.0, tick as f32, 0.0]);
        assert_eq!(base.records[0].position, [tick as f32, 10.0]);
    }

    let mut disabled = layer;
    disabled.enabled = false;
    let base = reader.read_tick(45).unwrap();
    let restored = compose_frame(&base, 45, &[disabled]).unwrap();
    assert_eq!(restored.records[0].position, [45.0, 10.0, 0.0]);
    assert_eq!(cache_hash(&cache_path), base_hash);
}

#[test]
fn layers_compose_in_priority_then_logical_id_order() {
    let base = frame(30);
    let absolute = OverrideLayerV1 {
        layer_id: "b-absolute".into(),
        priority: 20,
        operation: OverrideOperation::Absolute,
        samples: vec![TransformOverride {
            tick: 30,
            translation: [100.0, 200.0, 3.0],
        }],
        ..additive_layer()
    };
    let after = OverrideLayerV1 {
        layer_id: "a-after".into(),
        priority: 30,
        samples: vec![TransformOverride {
            tick: 30,
            translation: [2.0, 4.0, 6.0],
        }],
        ..additive_layer()
    };
    let composed = compose_frame(&base, 30, &[after, absolute, additive_layer()]).unwrap();
    assert_eq!(composed.records[0].position, [102.0, 204.0, 9.0]);
}

#[test]
fn checked_override_fixture_shape_validates_against_the_schema() {
    let schema_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/override-layer-v1.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let instance = serde_json::to_value(additive_layer()).unwrap();
    if let Err(error) = validator.validate(&instance) {
        panic!("override did not validate: {error}");
    }
}
