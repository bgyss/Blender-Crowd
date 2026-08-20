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

struct CommandResult {
    status: ExitStatus,
    stderr: String,
    bytes: Option<Vec<u8>>,
    report: Option<Value>,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_fixture(fixture: &Path) -> RunResult {
    let motion_report = repository_root().join("docs/benchmarks/2026-08-18-m6-cmu-motion.json");
    let result = run_command(fixture, &motion_report);
    let bytes = result
        .bytes
        .unwrap_or_else(|| panic!("runner did not write report; stderr={}", result.stderr));
    let report = result.report.unwrap();
    RunResult {
        status: result.status,
        bytes,
        report,
    }
}

fn run_command(fixture: &Path, motion_report: &Path) -> CommandResult {
    let root = repository_root();
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("report.json");
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
    let bytes = fs::read(&output_path).ok();
    let report = bytes
        .as_deref()
        .map(|bytes| serde_json::from_slice(bytes).unwrap());
    CommandResult {
        status: output.status,
        stderr: String::from_utf8(output.stderr).unwrap(),
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

fn checked_fixture_value() -> Value {
    serde_json::from_slice(&fs::read(checked_fixture()).unwrap()).unwrap()
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn report_schema() -> Value {
    serde_json::from_slice(
        &fs::read(repository_root().join("schemas/m6-acceptance-scenes-v1.schema.json")).unwrap(),
    )
    .unwrap()
}

fn assert_schema_valid(report: &Value) {
    jsonschema::validator_for(&report_schema())
        .unwrap()
        .validate(report)
        .unwrap();
}

fn assert_scene_passes(id: &str) {
    let run = run_fixture(&checked_fixture());
    assert!(run.status.success());
    let scene = scene(&run.report, id);
    assert_eq!(scene["passed"], true, "{id}: {:?}", scene["reasons"]);
    assert_eq!(scene["hard_safety_passed"], true);
    assert!(matches!(
        scene["isolation_status"].as_str(),
        Some("measured") | Some("not_applicable")
    ));
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
    assert!(metrics["runtime_motion_fallbacks"].is_u64());
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
        selection["baseline"]["content_hash"],
        selection["baseline"]["database_hash"]
    );
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
        first.report["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|scene| scene["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        SCENE_IDS
    );

    assert_schema_valid(&first.report);
}

#[test]
fn a_schema_valid_failed_criterion_writes_a_report_and_exits_one() {
    let mut fixture = checked_fixture_value();
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

    assert_schema_valid(&run.report);
}

#[test]
fn source_selection_rejects_spoofed_report_database_provenance_and_threshold_bytes() {
    let root = repository_root();
    let directory = tempfile::tempdir().unwrap();
    let cases = ["cmu_report", "database", "provenance", "thresholds"];

    for case in cases {
        let mut fixture = checked_fixture_value();
        let mut motion_report_path = root.join("docs/benchmarks/2026-08-18-m6-cmu-motion.json");
        match case {
            "cmu_report" => {
                let mut report: Value =
                    serde_json::from_slice(&fs::read(&motion_report_path).unwrap()).unwrap();
                report["database_id"] = Value::from("spoofed-cmu");
                motion_report_path = directory.path().join("spoofed-cmu.json");
                write_json(&motion_report_path, &report);
            }
            "database" => {
                let source = root.join("assets/reference/m6/motion-database-input-v1.json");
                let mut value: Value = serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
                value["database_id"] = Value::from("spoofed-reference-motion");
                let path = directory.path().join("spoofed-database.json");
                write_json(&path, &value);
                fixture["motion_baseline"]["database_path"] = Value::from(path.to_str().unwrap());
            }
            "provenance" => {
                let source = root.join("assets/reference/m6/motion-provenance-v1.json");
                let mut value: Value = serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
                value["content_hash"] = Value::from("f".repeat(64));
                let path = directory.path().join("spoofed-provenance.json");
                write_json(&path, &value);
                fixture["motion_baseline"]["provenance_path"] = Value::from(path.to_str().unwrap());
            }
            "thresholds" => {
                let source = root.join("assets/reference/m6/motion-thresholds-v1.json");
                let mut value: Value = serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
                value["threshold_id"] = Value::from("spoofed-thresholds");
                let path = directory.path().join("spoofed-thresholds.json");
                write_json(&path, &value);
                fixture["motion_baseline"]["thresholds_path"] = Value::from(path.to_str().unwrap());
            }
            _ => unreachable!(),
        }
        let fixture_path = directory.path().join(format!("fixture-{case}.json"));
        write_json(&fixture_path, &fixture);
        let result = run_command(&fixture_path, &motion_report_path);
        assert_eq!(result.status.code(), Some(2), "{case}: {}", result.stderr);
        assert!(result.report.is_none(), "{case} wrote an accepted report");
        assert!(
            result.stderr.contains("pinned") || result.stderr.contains("digest"),
            "{case}: {}",
            result.stderr
        );
    }
}

#[test]
fn seed_population_and_tick_mutations_change_executed_state() {
    let baseline = run_fixture(&checked_fixture());
    let baseline_scene = scene(&baseline.report, "scheduled_cafe");
    let directory = tempfile::tempdir().unwrap();

    let mut seed_fixture = checked_fixture_value();
    seed_fixture["scenes"][0]["seed"] = Value::from(44_001);
    let seed_path = directory.path().join("seed.json");
    write_json(&seed_path, &seed_fixture);
    let seeded = run_fixture(&seed_path);
    assert_ne!(
        scene(&seeded.report, "scheduled_cafe")["metrics"]["scene_specific"]["initial_state_hash"],
        baseline_scene["metrics"]["scene_specific"]["initial_state_hash"]
    );

    let mut population_fixture = checked_fixture_value();
    population_fixture["scenes"][0]["agent_count"] = Value::from(4);
    let population_path = directory.path().join("population.json");
    write_json(&population_path, &population_fixture);
    let population = run_fixture(&population_path);
    let population_scene = scene(&population.report, "scheduled_cafe");
    assert_eq!(population_scene["agent_count"], 4);
    assert_eq!(
        population_scene["metrics"]["scene_specific"]["executed_agent_count"],
        4
    );
    assert_ne!(
        population_scene["metrics"]["scene_specific"]["motion_evaluated_agent_ticks"],
        baseline_scene["metrics"]["scene_specific"]["motion_evaluated_agent_ticks"]
    );

    let mut tick_fixture = checked_fixture_value();
    tick_fixture["scenes"][0]["tick_end"] = Value::from(119);
    let tick_path = directory.path().join("ticks.json");
    write_json(&tick_path, &tick_fixture);
    let ticked = run_fixture(&tick_path);
    assert_eq!(
        scene(&ticked.report, "scheduled_cafe")["metrics"]["scene_specific"]["executed_ticks"],
        120
    );
    assert_ne!(
        scene(&ticked.report, "scheduled_cafe")["metrics"]["scene_specific"]["final_state_hash"],
        baseline_scene["metrics"]["scene_specific"]["final_state_hash"]
    );
}

#[test]
fn motion_slide_is_measured_from_executed_tick_displacement() {
    let mut fixture = checked_fixture_value();
    fixture["scenes"][0]["motion"]["desired_velocity_millimeters_per_second"] =
        serde_json::json!([0, 0]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("zero-velocity.json");
    write_json(&path, &fixture);

    let run = run_fixture(&path);
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(
        scene(&run.report, "scheduled_cafe")["metrics"]["foot_slide_millimeters"],
        34,
        "1000 mm/s executed against a zero request over one 30 Hz tick must measure 34 mm"
    );
    assert_schema_valid(&run.report);
}

#[test]
fn required_contact_selects_and_executes_the_matching_sample_phase() {
    let baseline = run_fixture(&checked_fixture());
    assert!(baseline.status.success());
    let baseline_scene = scene(&baseline.report, "scheduled_cafe");
    let mut fixture = checked_fixture_value();
    fixture["scenes"][0]["motion"]["required_contact"] = Value::from("right_foot");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("right-foot.json");
    write_json(&path, &fixture);

    let run = run_fixture(&path);
    assert!(run.status.success());
    let mutated = scene(&run.report, "scheduled_cafe");
    assert_eq!(
        mutated["metrics"]["observed_contacts"],
        mutated["metrics"]["required_contacts"]
    );
    assert_ne!(
        mutated["metrics"]["scene_specific"]["final_state_hash"],
        baseline_scene["metrics"]["scene_specific"]["final_state_hash"],
        "changing the required contact must execute the corresponding sample phase"
    );
}

#[test]
fn scheduled_cafe_layer_targets_follow_post_release_promotion() {
    let run = run_fixture(&checked_fixture());
    assert!(run.status.success());
    let evidence = &scene(&run.report, "scheduled_cafe")["metrics"]["scene_specific"];
    assert!(
        evidence["released_agent_id"].is_u64(),
        "cafe evidence must expose the released owner"
    );
    assert!(
        evidence["promoted_agent_id"].is_u64(),
        "cafe evidence must expose the waiting agent promoted after release"
    );
    assert!(
        evidence["layer_target_agent_ids"].is_array(),
        "cafe evidence must expose the targets used by the applied layer"
    );
    let released = evidence["released_agent_id"].as_u64().unwrap();
    let promoted = evidence["promoted_agent_id"].as_u64().unwrap();
    let layer_targets = evidence["layer_target_agent_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap())
        .collect::<Vec<_>>();

    assert!(layer_targets.contains(&promoted));
    assert!(!layer_targets.contains(&released));
    assert_eq!(layer_targets.len(), 2);
}

#[test]
fn isolation_uses_full_executed_state_and_is_explicitly_not_applicable_without_an_operation() {
    let run = run_fixture(&checked_fixture());
    assert!(run.status.success());
    for id in [
        "scheduled_cafe",
        "paired_handoff",
        "ragdoll_recovery",
        "mixed_tier_diagnostics",
    ] {
        let scene = scene(&run.report, id);
        assert_eq!(scene["isolation_status"], "measured", "{id}");
        assert_eq!(scene["base_cache_mutations"], 0, "{id}");
        assert_eq!(scene["unrelated_agent_mutations"], 0, "{id}");
        assert!(
            scene["metrics"]["scene_specific"]["target_agent_mutations"]
                .as_u64()
                .unwrap()
                > 0,
            "{id} did not execute a target operation"
        );
        assert_eq!(
            scene["metrics"]["scene_specific"]["executed_agent_count"], scene["agent_count"],
            "{id} isolation did not cover the full scene"
        );
    }
    for id in ["family_split_regroup", "terrain_motion_feedback"] {
        let scene = scene(&run.report, id);
        assert_eq!(scene["isolation_status"], "not_applicable", "{id}");
        assert!(scene["base_cache_mutations"].is_null(), "{id}");
        assert!(scene["unrelated_agent_mutations"].is_null(), "{id}");
        assert!(scene["isolation_reason"]
            .as_str()
            .unwrap()
            .contains("no promoted"));
    }
}

#[test]
fn paired_handoff_rejects_artifacts_bound_to_a_different_executed_base() {
    let root = repository_root();
    let directory = tempfile::tempdir().unwrap();
    let mismatched_hash = "f".repeat(64);

    let request_source = root.join("assets/reference/m6/interaction-request-v1.json");
    let mut request: Value = serde_json::from_slice(&fs::read(request_source).unwrap()).unwrap();
    request["provenance"]["base_cache_hash"] = Value::from(mismatched_hash.clone());
    let request_path = directory
        .path()
        .join("mismatch-interaction-request-v1.json");
    write_json(&request_path, &request);

    let layer_source = root.join("assets/reference/m6/interaction-animation-layer-v1.json");
    let mut layer: Value = serde_json::from_slice(&fs::read(layer_source).unwrap()).unwrap();
    layer["base_cache_hash"] = Value::from(mismatched_hash);
    let layer_path = directory
        .path()
        .join("mismatch-interaction-animation-layer-v1.json");
    write_json(&layer_path, &layer);

    let mut fixture = checked_fixture_value();
    fixture["scenes"][3]["source_paths"][0] = Value::from(request_path.to_str().unwrap());
    fixture["scenes"][3]["source_paths"][2] = Value::from(layer_path.to_str().unwrap());
    let fixture_path = directory.path().join("mismatched-base-fixture.json");
    write_json(&fixture_path, &fixture);

    let result = run_command(
        &fixture_path,
        &root.join("docs/benchmarks/2026-08-18-m6-cmu-motion.json"),
    );
    assert_eq!(result.status.code(), Some(2), "{}", result.stderr);
    assert!(result.report.is_none());
    assert!(
        result.stderr.contains("executed full base cache"),
        "{}",
        result.stderr
    );
}

#[test]
fn paired_handoff_valid_operation_reports_executed_base_binding_and_full_state_isolation() {
    let request: Value = serde_json::from_slice(
        &fs::read(repository_root().join("assets/reference/m6/interaction-request-v1.json"))
            .unwrap(),
    )
    .unwrap();
    let run = run_fixture(&checked_fixture());
    assert!(run.status.success());
    let paired = scene(&run.report, "paired_handoff");
    let evidence = &paired["metrics"]["scene_specific"];

    assert_eq!(
        evidence["base_cache_hash"], request["provenance"]["base_cache_hash"],
        "the valid request must identify the executed full base"
    );
    assert_eq!(paired["base_cache_mutations"], 0);
    assert_eq!(paired["unrelated_agent_mutations"], 0);
    assert_eq!(evidence["executed_agent_count"], paired["agent_count"]);
    assert_eq!(evidence["target_agent_mutations"], 2);
}

#[test]
fn declared_sources_must_be_consumed_and_scene_specific_sources_drive_execution() {
    let directory = tempfile::tempdir().unwrap();
    let mut fixture = checked_fixture_value();
    fixture["scenes"][0]["source_paths"]
        .as_array_mut()
        .unwrap()
        .push(Value::from("assets/reference/m6/contact-v1.json"));
    let path = directory.path().join("extra-source.json");
    write_json(&path, &fixture);
    let result = run_command(
        &path,
        &repository_root().join("docs/benchmarks/2026-08-18-m6-cmu-motion.json"),
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(
        result.stderr.contains("declared but not consumed"),
        "{}",
        result.stderr
    );

    let run = run_fixture(&checked_fixture());
    assert!(
        scene(&run.report, "paired_handoff")["metrics"]["scene_specific"]["applied_layer_edits"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        scene(&run.report, "ragdoll_recovery")["metrics"]["scene_specific"]
            ["hero_boundary_validated"],
        1
    );
    assert_eq!(
        scene(&run.report, "mixed_tier_diagnostics")["metrics"]["scene_specific"]
            ["validated_interaction_contacts"],
        1
    );
}

#[test]
fn schema_requires_typed_common_and_per_scene_evidence_and_canonical_names() {
    let run = run_fixture(&checked_fixture());
    let validator = jsonschema::validator_for(&report_schema()).unwrap();

    let mut missing_base = run.report.clone();
    missing_base["scenes"][0]
        .as_object_mut()
        .unwrap()
        .remove("base_cache_mutations");
    assert!(validator.validate(&missing_base).is_err());

    let mut missing_fallback = run.report.clone();
    missing_fallback["scenes"][0]["metrics"]
        .as_object_mut()
        .unwrap()
        .remove("runtime_motion_fallbacks");
    assert!(validator.validate(&missing_fallback).is_err());

    let mut missing_scene_fact = run.report.clone();
    missing_scene_fact["scenes"][0]["metrics"]["scene_specific"]
        .as_object_mut()
        .unwrap()
        .remove("granted_reservations");
    assert!(validator.validate(&missing_scene_fact).is_err());

    let mut misnamed = run.report.clone();
    misnamed["scenes"][0]["id"] = Value::from("scheduled_coffee");
    assert!(validator.validate(&misnamed).is_err());
}

#[test]
fn permissive_configured_max_cannot_hide_a_hard_safety_failure() {
    let mut fixture = checked_fixture_value();
    fixture["scenes"][0]["tick_start"] = Value::from(121);
    fixture["scenes"][0]["tick_end"] = Value::from(121);
    fixture["scenes"][0]["criteria"]["max_safety_violations"] = Value::from(100);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("hard-failure.json");
    write_json(&path, &fixture);

    let run = run_fixture(&path);
    assert_eq!(run.status.code(), Some(1));
    let failed = scene(&run.report, "scheduled_cafe");
    assert_eq!(failed["hard_safety_passed"], false);
    assert!(failed["metrics"]["safety_violations"].as_u64().unwrap() > 0);
    assert!(failed["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains("hard safety")));
    assert_schema_valid(&run.report);
}
