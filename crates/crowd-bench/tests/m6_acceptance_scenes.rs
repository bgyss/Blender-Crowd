use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use serde_json::Value;

const SCENE_IDS: [&str; 6] = [
    "scheduled_cafe",
    "family_split_regroup",
    "terrain_motion_feedback",
    "paired_handoff",
    "ragdoll_recovery",
    "mixed_tier_diagnostics",
];

struct RunResult {
    status: ExitStatus,
    bytes: Vec<u8>,
    report: Value,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_fixture(fixture: &Path) -> RunResult {
    let root = repository_root();
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("report.json");
    let motion_report = root.join("docs/benchmarks/2026-08-18-m6-cmu-motion.json");
    let output = Command::new(env!("CARGO_BIN_EXE_m6-acceptance-scenes"))
        .current_dir(&root)
        .args([
            "--fixture",
            fixture.to_str().unwrap(),
            "--motion-report",
            motion_report.to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let bytes = fs::read(&output_path).unwrap_or_else(|error| {
        panic!(
            "runner did not write report: {error}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let report = serde_json::from_slice(&bytes).unwrap();
    RunResult {
        status: output.status,
        bytes,
        report,
    }
}

fn checked_fixture() -> PathBuf {
    repository_root().join("assets/reference/m6/acceptance-scenes-v1.json")
}

fn scene<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scene| scene["id"] == id)
        .unwrap_or_else(|| panic!("missing scene {id}"))
}

fn assert_scene_passes(id: &str) {
    let run = run_fixture(&checked_fixture());
    assert!(run.status.success());
    let scene = scene(&run.report, id);
    assert_eq!(scene["passed"], true, "{id}: {:?}", scene["reasons"]);
    assert_eq!(scene["hard_safety_passed"], true);
    assert_eq!(scene["unrelated_agent_mutations"], 0);
    assert!(scene["agent_count"].as_u64().unwrap() > 0);
    assert!(scene["tick_end"].as_u64().unwrap() >= scene["tick_start"].as_u64().unwrap());
    assert_eq!(
        scene["deterministic_replay_hash"].as_str().unwrap().len(),
        64
    );
    assert!(scene["source_hashes"]
        .as_object()
        .unwrap()
        .values()
        .all(|hash| hash.as_str().is_some_and(|hash| hash.len() == 64)));

    let metrics = &scene["metrics"];
    assert!(metrics["trajectory_fit_millimeters"].is_u64());
    assert!(metrics["required_contacts"].is_u64());
    assert!(metrics["observed_contacts"].is_u64());
    assert!(metrics["contact_precision_millionths"].is_u64());
    assert_eq!(metrics["safety_violations"], 0);
    assert!(!metrics["scene_specific"].as_object().unwrap().is_empty());
}

#[test]
fn scheduled_cafe_passes_resource_and_motion_criteria() {
    assert_scene_passes("scheduled_cafe");
}

#[test]
fn family_split_regroup_passes_group_and_isolation_criteria() {
    assert_scene_passes("family_split_regroup");
}

#[test]
fn terrain_motion_feedback_passes_trajectory_contact_and_safety_criteria() {
    assert_scene_passes("terrain_motion_feedback");
}

#[test]
fn paired_handoff_passes_contact_promotion_and_isolation_criteria() {
    assert_scene_passes("paired_handoff");
}

#[test]
fn ragdoll_recovery_passes_physics_recovery_and_isolation_criteria() {
    assert_scene_passes("ragdoll_recovery");
}

#[test]
fn mixed_tier_diagnostics_passes_tier_evidence_and_isolation_criteria() {
    assert_scene_passes("mixed_tier_diagnostics");
}

#[test]
fn rejected_cmu_candidate_falls_back_to_the_checked_cc0_baseline() {
    let run = run_fixture(&checked_fixture());
    assert!(run.status.success());
    let selection = &run.report["motion_source_selection"];
    assert_eq!(selection["external_candidate"]["status"], "rejected");
    assert_eq!(
        selection["external_candidate"]["joint_limit_violations"]["observed"],
        3_587
    );
    assert_eq!(
        selection["external_candidate"]["joint_limit_violations"]["limit"],
        0
    );
    assert_eq!(selection["baseline"]["status"], "accepted");
    assert_eq!(selection["baseline"]["license_id"], "CC0-1.0");
    assert_eq!(
        selection["baseline"]["database_hash"],
        "c687fede242e359fb7b94e91e1c17a44ddacd01963697f2e5f4e687c01998e08"
    );
    assert_eq!(
        selection["external_candidate"]["report_hash"],
        "b05e9bce668fab8ef11fe55e6b396841fd23a7d2373e8281e9c654280cb5f2f9"
    );
    assert!(selection["external_candidate"]["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("hard-zero")));
}

#[test]
fn integrated_scenes_are_schema_valid_repeatable_and_preserve_unrelated_agents() {
    let first = run_fixture(&checked_fixture());
    let second = run_fixture(&checked_fixture());
    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.report["schema_version"], 1);
    assert_eq!(first.report["passed"], true);
    assert_eq!(first.report["hard_safety_passed"], true);
    assert_eq!(first.report["unrelated_agent_mutations"], 0);
    assert_eq!(first.report["scenes"].as_array().unwrap().len(), 6);
    assert_eq!(
        first.report["deterministic_replay_hash"],
        second.report["deterministic_replay_hash"]
    );
    assert_eq!(
        first.report["deterministic_replay_hash"],
        "620e086637a7df18e78c13347e26a3452078bc7fed1d8f65d79d0e81fbc569b8",
        "replay identity must not depend on absolute versus repository-relative path spelling"
    );
    assert_eq!(
        first.report["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|scene| scene["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        SCENE_IDS
    );

    let schema: Value = serde_json::from_slice(
        &fs::read(repository_root().join("schemas/m6-acceptance-scenes-v1.schema.json")).unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&first.report)
        .unwrap();
}

#[test]
fn a_schema_valid_failed_criterion_writes_a_report_and_exits_one() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(checked_fixture()).unwrap()).unwrap();
    fixture["scenes"][2]["criteria"]["max_trajectory_fit_millimeters"] = Value::from(0);
    let directory = tempfile::tempdir().unwrap();
    let fixture_path = directory.path().join("failing-fixture.json");
    fs::write(&fixture_path, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();

    let run = run_fixture(&fixture_path);
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(run.report["passed"], false);
    assert_eq!(
        scene(&run.report, "terrain_motion_feedback")["passed"],
        false
    );

    let schema: Value = serde_json::from_slice(
        &fs::read(repository_root().join("schemas/m6-acceptance-scenes-v1.schema.json")).unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&run.report)
        .unwrap();
}
