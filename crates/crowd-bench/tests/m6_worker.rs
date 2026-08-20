use std::process::Command;

use crowd_cache::AnimationLayerV1;
use crowd_core::interaction::{InteractionMotionV1, InteractionRequestV1};
use tempfile::tempdir;

#[test]
fn local_worker_round_trips_the_request_through_the_deterministic_baseline() {
    let binary = env!("CARGO_BIN_EXE_m6-interaction-worker");
    let directory = tempdir().unwrap();
    let request_path = directory.path().join("request.json");
    let output_path = directory.path().join("motion.json");
    std::fs::write(
        &request_path,
        include_str!("../../../assets/reference/m6/interaction-request-v1.json"),
    )
    .unwrap();

    let output = Command::new(binary)
        .args([
            "--request",
            request_path.to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "worker failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let request: InteractionRequestV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-request-v1.json"
    ))
    .unwrap();
    let motion: InteractionMotionV1 =
        serde_json::from_str(&std::fs::read_to_string(output_path).unwrap()).unwrap();
    motion.validate_against(&request).unwrap();
    assert_eq!(motion.provenance.backend, "authored-paired-clip");
}

#[test]
fn local_worker_rejects_unknown_arguments_without_writing_output() {
    let binary = env!("CARGO_BIN_EXE_m6-interaction-worker");
    let output = Command::new(binary).arg("--unknown").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown argument"));
}

#[test]
fn local_worker_can_emit_the_validated_sparse_animation_layer() {
    let binary = env!("CARGO_BIN_EXE_m6-interaction-worker");
    let directory = tempdir().unwrap();
    let request_path = directory.path().join("request.json");
    let motion_path = directory.path().join("motion.json");
    let layer_path = directory.path().join("layer.json");
    std::fs::write(
        &request_path,
        include_str!("../../../assets/reference/m6/interaction-request-v1.json"),
    )
    .unwrap();
    let output = Command::new(binary)
        .args([
            "--request",
            request_path.to_str().unwrap(),
            "--out",
            motion_path.to_str().unwrap(),
            "--layer-out",
            layer_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "worker failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let layer: AnimationLayerV1 =
        serde_json::from_str(&std::fs::read_to_string(layer_path).unwrap()).unwrap();
    let request: InteractionRequestV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-request-v1.json"
    ))
    .unwrap();
    layer.validate().unwrap();
    assert_eq!(layer.base_cache_hash, request.provenance.base_cache_hash);
    assert_eq!(layer.target_agent_ids, vec![7, 9]);
}
