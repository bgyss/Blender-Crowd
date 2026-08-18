use crowd_cache::{
    compose_interaction_frame_v1, AnimationEditV1, AnimationLayerV1, FallbackClipV1, Frame,
    FrameRecord, INTERACTION_LAYER_SCHEMA_VERSION,
};
use jsonschema::validator_for;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn base() -> Frame {
    Frame {
        records: vec![
            FrameRecord {
                agent_id: 7,
                clip_id: 1,
                phase: 0.1,
                ..FrameRecord::default()
            },
            FrameRecord {
                agent_id: 9,
                clip_id: 2,
                phase: 0.2,
                ..FrameRecord::default()
            },
            FrameRecord {
                agent_id: 11,
                clip_id: 3,
                phase: 0.3,
                ..FrameRecord::default()
            },
        ],
    }
}

fn layer(base_cache_hash: &str) -> AnimationLayerV1 {
    AnimationLayerV1 {
        schema_version: INTERACTION_LAYER_SCHEMA_VERSION,
        layer_id: "interaction-pair-7-9".to_owned(),
        interaction_id: "request-pair-7-9".to_owned(),
        base_cache_hash: base_cache_hash.to_owned(),
        target_agent_ids: vec![7, 9],
        tick_start: 10,
        tick_end: 20,
        priority: 10,
        enabled: true,
        provenance: "authored-paired-clip-v1".to_owned(),
        edits: vec![
            AnimationEditV1 {
                agent_id: 7,
                tick: 15,
                clip_id: 42,
                phase_millionths: 500_000,
            },
            AnimationEditV1 {
                agent_id: 9,
                tick: 15,
                clip_id: 43,
                phase_millionths: 500_000,
            },
        ],
        fallback: FallbackClipV1 {
            clip_set_id: "pedestrian_basic".to_owned(),
            clip_id: "walk".to_owned(),
            reason: "validation failure".to_owned(),
        },
    }
}

#[test]
fn interaction_layer_changes_only_target_agents_and_preserves_base() {
    let original = base();
    let composed = compose_interaction_frame_v1(&original, 15, HASH, &[layer(HASH)]).unwrap();

    assert_eq!(original.records[0].clip_id, 1);
    assert_eq!(original.records[2].clip_id, 3);
    assert_eq!(composed.records[0].clip_id, 42);
    assert_eq!(composed.records[1].clip_id, 43);
    assert_eq!(composed.records[2], original.records[2]);
}

#[test]
fn interaction_layer_rejects_cross_cache_attachment() {
    let error = compose_interaction_frame_v1(
        &base(),
        15,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        &[layer(HASH)],
    )
    .unwrap_err();
    assert!(error.to_string().contains("another base cache"));
}

#[test]
fn disabled_or_out_of_range_layers_leave_the_frame_unchanged() {
    let mut disabled = layer(HASH);
    disabled.enabled = false;
    let unchanged = compose_interaction_frame_v1(&base(), 15, HASH, &[disabled]).unwrap();
    assert_eq!(unchanged, base());

    let unchanged = compose_interaction_frame_v1(&base(), 30, HASH, &[layer(HASH)]).unwrap();
    assert_eq!(unchanged, base());
}

#[test]
fn checked_animation_layer_fixture_matches_schema_and_rust_type() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("schemas/interaction-animation-layer-v1.schema.json"))
            .unwrap(),
    )
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-animation-layer-v1.json"
    ))
    .unwrap();
    validator_for(&schema).unwrap().validate(&fixture).unwrap();
    let layer: AnimationLayerV1 = serde_json::from_value(fixture).unwrap();
    layer.validate().unwrap();
}
