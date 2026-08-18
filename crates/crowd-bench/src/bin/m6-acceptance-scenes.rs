//! Integrated deterministic M6 reference-scene runner.
//!
//! This binary is orchestration only. Activity admission, formation analysis,
//! motion matching/feedback, interaction validation/scheduling, cache-layer
//! isolation, fidelity scheduling, and physics recovery all remain owned by
//! their existing runtimes.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crowd_cache::{
    compose_interaction_frame_v1, content_hash, simulate_physics_handoff_v1, AnimationEditV1,
    AnimationLayerV1, FallbackClipV1, Frame, FrameRecord, PhysicsHandoffSpecV1,
    INTERACTION_LAYER_SCHEMA_VERSION,
};
use crowd_core::activity::{
    ActivityRequestV1, ActivityScheduleV1, ReservationRuntimeV1, ReservationStatusV1, ResourceV1,
};
use crowd_core::fidelity::{render_for, FidelityPolicy, RenderTier, SimulationTier};
use crowd_core::formation::FormationV1;
use crowd_core::interaction::{
    deterministic_paired_clip, InteractionGroupStatusV1, InteractionMotionV1, InteractionRequestV1,
    InteractionSchedulerV1,
};
use crowd_core::motion::{
    FootContactV1, FootLockWindowV1, MotionClipV1, MotionDatabaseV1, MotionFeedbackV1,
    MotionMatcher, MotionQueryV1, MotionSampleV1, TerrainConstraintV1,
};
use crowd_core::physics::{
    recovery_phase, validate_transition, HeroIntegrationBoundaryV1, PhysicsTransitionV1,
    RecoveryPhaseV1,
};
use crowd_core::{derive_agent_id, AgentId, Purpose, StableRng, Vec2};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const REPORT_SCHEMA_VERSION: u32 = 1;
const TICKS_PER_SECOND: i64 = 30;
const PINNED_CMU_REPORT_HASH: &str =
    "b05e9bce668fab8ef11fe55e6b396841fd23a7d2373e8281e9c654280cb5f2f9";
const PINNED_DATABASE_HASH: &str =
    "c687fede242e359fb7b94e91e1c17a44ddacd01963697f2e5f4e687c01998e08";
const PINNED_PROVENANCE_HASH: &str =
    "60d1bf5aa98f66ab1a37096876140a53b6bb6d63e03f8a83a5ed7370c895340d";
const PINNED_THRESHOLDS_HASH: &str =
    "993bb4897305524e359943820fdae24f347e8cc025429f604fbdc628b76de154";
const PINNED_CMU_DATABASE_ID: &str = "cmu-mocap-subjects-35-36-m6-v1";
const PINNED_CMU_MANIFEST_ID: &str = "cmu-mocap-subjects-35-36-m6-v1";
const PINNED_CMU_SOURCE_HASH: &str =
    "a75af4c0e86aa930f8618e082aa89dad820300dcecd469a8e5ff8c221ec91424";
const PINNED_DATABASE_ID: &str = "reference-humanoid-motion";
const PINNED_RETARGET_PROFILE_ID: &str = "reference-humanoid";
const PINNED_PROVENANCE_ASSET_ID: &str = "reference-walk-metadata";
const PINNED_PROVENANCE_SOURCE_URI: &str =
    "repo://assets/reference/m6/motion-database-input-v1.json";
const PINNED_THRESHOLD_ID: &str = "m6-cmu-motion-2026-08-18";
const PINNED_CMU_OBSERVATIONS: u64 = 3_587;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceFixture {
    schema_version: u32,
    fixture_id: String,
    motion_baseline: MotionBaselineFixture,
    scenes: Vec<SceneFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionBaselineFixture {
    database_path: String,
    provenance_path: String,
    thresholds_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFixture {
    id: String,
    seed: u64,
    tick_start: u64,
    tick_end: u64,
    agent_count: u64,
    source_paths: Vec<String>,
    motion: SceneMotionFixture,
    criteria: SceneCriteria,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneMotionFixture {
    desired_velocity_millimeters_per_second: [i32; 2],
    desired_slope_millionths: i32,
    required_contact: Option<FootContactV1>,
    foot_slide_millimeters: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneCriteria {
    max_trajectory_fit_millimeters: u32,
    max_foot_slide_millimeters: u32,
    minimum_contact_precision_millionths: u32,
    max_safety_violations: u32,
    max_fallback_count: u32,
    max_unrelated_agent_mutations: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionDatabaseInput {
    schema_version: u32,
    database_id: String,
    retarget_profile_id: String,
    source_provenance: String,
    clips: Vec<MotionClipInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionClipInput {
    id: String,
    samples: Vec<MotionSampleInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionSampleInput {
    tick: u64,
    velocity_millimeters_per_second: [i32; 2],
    contact: FootContactV1,
    slope_millionths: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionProvenanceInput {
    schema_version: u32,
    asset_id: String,
    source_uri: String,
    content_hash: String,
    license_id: String,
    redistribution_allowed: bool,
    terms_reference: String,
    checkpoint_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MotionThresholdInput {
    schema_version: u32,
    threshold_id: String,
    baseline_report: String,
    source_manifest: String,
    hard_limits: MotionHardLimitsInput,
}

#[derive(Debug, Deserialize)]
struct MotionHardLimitsInput {
    joint_limit_violations: HardLimitInput,
}

#[derive(Debug, Deserialize)]
struct HardLimitInput {
    limit: u64,
    evidence_status: String,
    baseline: u64,
}

#[derive(Debug, Deserialize)]
struct CmuMotionReportInput {
    schema_version: u32,
    database_id: String,
    source_manifest_id: String,
    source_hash: String,
    hard_limit_evidence: CmuHardLimitEvidenceInput,
    quality_metrics: CmuQualityMetricsInput,
}

#[derive(Debug, Deserialize)]
struct CmuHardLimitEvidenceInput {
    joint_limit_violations: CmuMeasuredObservationInput,
}

#[derive(Debug, Deserialize)]
struct CmuMeasuredObservationInput {
    observed: u64,
    status: String,
}

#[derive(Debug, Deserialize)]
struct CmuQualityMetricsInput {
    joint_limit_violations: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TerrainMotionInput {
    schema_version: u32,
    scene_id: String,
    terrain_id: String,
    max_slope_millionths: i32,
    ground_height_millimeters: i32,
    foot_locks: Vec<FootLockInput>,
    navigation_feedback: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FootLockInput {
    foot: FootContactV1,
    tick_start: u64,
    tick_end: u64,
    position_millimeters: [i32; 3],
}

impl FootLockInput {
    fn runtime(&self) -> FootLockWindowV1 {
        FootLockWindowV1 {
            foot: self.foot,
            tick_start: self.tick_start,
            tick_end: self.tick_end,
            position_millimeters: self.position_millimeters,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MixedTierInput {
    schema_version: u32,
    scene_id: String,
    base_cache_hash: String,
    tiers: Vec<MixedTierEntry>,
    promoted_interaction_groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MixedTierEntry {
    simulation_tier: String,
    render_tier: String,
    agent_count: u64,
    diagnostics: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AcceptanceReport {
    schema_version: u32,
    fixture_id: String,
    hash_algorithm: &'static str,
    fixture_hash: String,
    motion_report_hash: String,
    motion_source_selection: MotionSourceSelection,
    scenes: Vec<SceneReport>,
    deterministic_replay_hash: String,
    hard_safety_passed: bool,
    fallback_count: u32,
    unrelated_agent_mutations: u32,
    passed: bool,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MotionSourceSelection {
    external_candidate: ExternalCandidateReport,
    baseline: BaselineSelectionReport,
}

#[derive(Debug, Serialize)]
struct ExternalCandidateReport {
    id: String,
    report_path: String,
    report_hash: String,
    status: &'static str,
    joint_limit_violations: HardLimitObservation,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HardLimitObservation {
    observed: u64,
    limit: u64,
}

#[derive(Debug, Serialize)]
struct BaselineSelectionReport {
    id: String,
    database_path: String,
    provenance_path: String,
    database_hash: String,
    provenance_hash: String,
    content_hash: String,
    license_id: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct SceneReport {
    id: String,
    seed: u64,
    fixture_hash: String,
    source_hashes: BTreeMap<String, String>,
    tick_start: u64,
    tick_end: u64,
    agent_count: u64,
    promoted_group_count: u32,
    deterministic_replay_hash: String,
    hard_safety_passed: bool,
    isolation_status: &'static str,
    isolation_reason: Option<String>,
    base_cache_mutations: Option<u32>,
    metrics: SceneMetrics,
    fallback_count: u32,
    unrelated_agent_mutations: Option<u32>,
    passed: bool,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SceneMetrics {
    trajectory_fit_millimeters: u32,
    foot_slide_millimeters: u32,
    required_contacts: u32,
    observed_contacts: u32,
    contact_precision_millionths: u32,
    safety_violations: u32,
    runtime_motion_fallbacks: u32,
    scene_specific: SceneSpecificEvidence,
}

#[derive(Debug, Serialize)]
struct ExecutionEvidence {
    executed_ticks: u64,
    executed_agent_count: u64,
    motion_evaluated_agent_ticks: u64,
    initial_state_hash: String,
    final_state_hash: String,
    target_agent_mutations: u32,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SceneSpecificEvidence {
    ScheduledCafe {
        #[serde(flatten)]
        execution: ExecutionEvidence,
        granted_reservations: u64,
        waiting_before_release: u64,
        promoted_after_release: u64,
        double_ownership_violations: u64,
    },
    FamilySplitRegroup {
        #[serde(flatten)]
        execution: ExecutionEvidence,
        split_samples: u64,
        regrouped_samples: u64,
        intrusion_samples: u64,
        maximum_split_separation_millimeters: u64,
    },
    TerrainMotionFeedback {
        #[serde(flatten)]
        execution: ExecutionEvidence,
        terrain_constraints_accepted: u64,
        foot_locks_satisfied: u64,
        navigation_feedback_events: u64,
    },
    PairedHandoff {
        #[serde(flatten)]
        execution: ExecutionEvidence,
        participants_locked_atomically: u64,
        required_interaction_contacts: u64,
        completed_interactions: u64,
        applied_layer_edits: u64,
    },
    RagdollRecovery {
        #[serde(flatten)]
        execution: ExecutionEvidence,
        physics_cache_samples: u64,
        impact_phase_ticks: u64,
        stabilize_phase_ticks: u64,
        resume_phase_ticks: u64,
        floor_contact_samples: u64,
        hero_boundary_validated: u64,
    },
    MixedTierDiagnostics {
        #[serde(flatten)]
        execution: ExecutionEvidence,
        full_diagnostic_channels: u64,
        reduced_diagnostic_channels: u64,
        aggregate_diagnostic_channels: u64,
        scheduled_animation_evaluations: u64,
        validated_interaction_contacts: u64,
        promoted_interaction_groups: u64,
    },
}

enum DomainSpecificData {
    ScheduledCafe {
        granted_reservations: u64,
        waiting_before_release: u64,
        promoted_after_release: u64,
        double_ownership_violations: u64,
    },
    FamilySplitRegroup {
        split_samples: u64,
        regrouped_samples: u64,
        intrusion_samples: u64,
        maximum_split_separation_millimeters: u64,
    },
    TerrainMotionFeedback {
        terrain_constraints_accepted: u64,
        foot_locks_satisfied: u64,
        navigation_feedback_events: u64,
    },
    PairedHandoff {
        participants_locked_atomically: u64,
        required_interaction_contacts: u64,
        completed_interactions: u64,
        applied_layer_edits: u64,
    },
    RagdollRecovery {
        physics_cache_samples: u64,
        impact_phase_ticks: u64,
        stabilize_phase_ticks: u64,
        resume_phase_ticks: u64,
        floor_contact_samples: u64,
        hero_boundary_validated: u64,
    },
    MixedTierDiagnostics {
        full_diagnostic_channels: u64,
        reduced_diagnostic_channels: u64,
        aggregate_diagnostic_channels: u64,
        scheduled_animation_evaluations: u64,
        validated_interaction_contacts: u64,
        promoted_interaction_groups: u64,
    },
}

struct DomainEvidence {
    promoted_group_count: u32,
    required_contacts: u32,
    observed_contacts: u32,
    safety_violations: u32,
    target_agent_ids: Vec<u64>,
    isolation_applicable: bool,
    isolation_reason: Option<String>,
    specific: DomainSpecificData,
    state: ExecutedSceneState,
}

struct MotionEvidence {
    trajectory_fit_millimeters: u32,
    foot_slide_millimeters: u32,
    required_contacts: u32,
    observed_contacts: u32,
    runtime_fallbacks: u32,
    safety_violations: u32,
    evaluated_agent_ticks: u64,
}

struct IsolationEvidence {
    status: &'static str,
    reason: Option<String>,
    base_cache_mutations: Option<u32>,
    unrelated_agent_mutations: Option<u32>,
    target_agent_mutations: u32,
}

struct ExecutedSceneState {
    seed: u64,
    tick_start: u64,
    tick_end: u64,
    initial_state_hash: String,
    base_snapshot: Frame,
    base: Frame,
    composed: Frame,
    motion: MotionEvidence,
}

struct SourceLedger {
    scene_id: String,
    declared: BTreeMap<String, PathBuf>,
    consumed: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct StateDigestRecord {
    agent_id: u64,
    position: [u32; 2],
    orientation: u32,
    scale: u32,
    population_id: u32,
    variant_id: u32,
    clip_id: u16,
    phase: u32,
    playback_rate: u32,
    behavior_state: u16,
    decision_reason: u16,
    destination_id: u32,
    velocity: [u32; 2],
    visible: bool,
    render_tier: u8,
}

fn main() -> ExitCode {
    match parse_args().and_then(|(fixture, motion_report, out)| {
        let report = run(&fixture, &motion_report)?;
        write_report(&out, &report)?;
        Ok(report.passed)
    }) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn parse_args() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let mut fixture = None;
    let mut motion_report = None;
    let mut out = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {argument}"))?;
        match argument.as_str() {
            "--fixture" => fixture = Some(PathBuf::from(value)),
            "--motion-report" => motion_report = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    match (fixture, motion_report, out) {
        (Some(fixture), Some(motion_report), Some(out)) => Ok((fixture, motion_report, out)),
        _ => Err(
            "usage: m6-acceptance-scenes --fixture FIXTURE --motion-report REPORT --out OUT"
                .to_owned(),
        ),
    }
}

fn run(fixture_path: &Path, motion_report_path: &Path) -> Result<AcceptanceReport, String> {
    let fixture_bytes = read_bytes(fixture_path)?;
    let fixture: AcceptanceFixture = parse_bytes(fixture_path, &fixture_bytes)?;
    validate_fixture(&fixture)?;
    let fixture_hash = hash_bytes(&fixture_bytes);

    let database_path = Path::new(&fixture.motion_baseline.database_path);
    let provenance_path = Path::new(&fixture.motion_baseline.provenance_path);
    let thresholds_path = Path::new(&fixture.motion_baseline.thresholds_path);
    let database_bytes = read_bytes(database_path)?;
    let provenance_bytes = read_bytes(provenance_path)?;
    let thresholds_bytes = read_bytes(thresholds_path)?;
    let motion_report_bytes = read_bytes(motion_report_path)?;
    let database_hash = hash_bytes(&database_bytes);
    let provenance_hash = hash_bytes(&provenance_bytes);
    let thresholds_hash = hash_bytes(&thresholds_bytes);
    let motion_report_hash = hash_bytes(&motion_report_bytes);
    verify_pinned_digest("CC0 motion database", &database_hash, PINNED_DATABASE_HASH)?;
    verify_pinned_digest(
        "CC0 motion provenance",
        &provenance_hash,
        PINNED_PROVENANCE_HASH,
    )?;
    verify_pinned_digest(
        "CMU threshold contract",
        &thresholds_hash,
        PINNED_THRESHOLDS_HASH,
    )?;
    verify_pinned_digest("CMU report", &motion_report_hash, PINNED_CMU_REPORT_HASH)?;
    let database_input: MotionDatabaseInput = parse_bytes(database_path, &database_bytes)?;
    let provenance: MotionProvenanceInput = parse_bytes(provenance_path, &provenance_bytes)?;
    let thresholds: MotionThresholdInput = parse_bytes(thresholds_path, &thresholds_bytes)?;
    let motion_report: CmuMotionReportInput =
        parse_bytes(motion_report_path, &motion_report_bytes)?;
    validate_motion_inputs(
        &database_input,
        &provenance,
        &thresholds,
        &motion_report,
        &database_hash,
    )?;
    let database = build_motion_database(&database_input)?;

    let observed = motion_report.quality_metrics.joint_limit_violations;
    let limit = thresholds.hard_limits.joint_limit_violations.limit;
    if observed <= limit {
        return Err(format!(
            "CMU candidate must remain rejected for the measured hard-zero joint-limit failure; observed {observed}, limit {limit}"
        ));
    }

    let shared_sources = BTreeMap::from([
        (path_text(database_path), database_hash.clone()),
        (path_text(provenance_path), provenance_hash.clone()),
        (path_text(thresholds_path), thresholds_hash),
        (path_text(motion_report_path), motion_report_hash.clone()),
    ]);
    let selection = MotionSourceSelection {
        external_candidate: ExternalCandidateReport {
            id: motion_report.database_id,
            report_path: path_text(motion_report_path),
            report_hash: motion_report_hash.clone(),
            status: "rejected",
            joint_limit_violations: HardLimitObservation { observed, limit },
            reasons: vec![format!(
                "measured joint-limit violations {observed} exceed the unchanged hard-zero limit {limit}"
            )],
        },
        baseline: BaselineSelectionReport {
            id: database_input.database_id.clone(),
            database_path: path_text(database_path),
            provenance_path: path_text(provenance_path),
            database_hash,
            provenance_hash,
            content_hash: provenance.content_hash.clone(),
            license_id: provenance.license_id.clone(),
            status: "accepted",
        },
    };

    let mut scenes = Vec::with_capacity(fixture.scenes.len());
    for scene in &fixture.scenes {
        scenes.push(run_scene(scene, &database, &fixture_hash, &shared_sources)?);
    }

    let hard_safety_passed = scenes.iter().all(|scene| scene.hard_safety_passed);
    let fallback_count = scenes.iter().map(|scene| scene.fallback_count).sum();
    let unrelated_agent_mutations = scenes
        .iter()
        .map(|scene| scene.unrelated_agent_mutations.unwrap_or(0))
        .sum();
    let passed = scenes.iter().all(|scene| scene.passed) && hard_safety_passed;
    let deterministic_replay_hash = hash_serializable(&(
        REPORT_SCHEMA_VERSION,
        &fixture.fixture_id,
        &fixture_hash,
        &motion_report_hash,
        scenes
            .iter()
            .map(|scene| (&scene.id, &scene.deterministic_replay_hash))
            .collect::<Vec<_>>(),
        fallback_count,
        unrelated_agent_mutations,
    ))?;
    let reasons = if passed {
        vec![
            "all six deterministic reference scenes passed".to_owned(),
            format!(
                "CMU candidate rejected at {observed} joint-limit violations against hard-zero; checked CC0 baseline used"
            ),
        ]
    } else {
        scenes
            .iter()
            .filter(|scene| !scene.passed)
            .map(|scene| format!("scene {} failed: {}", scene.id, scene.reasons.join("; ")))
            .collect()
    };

    Ok(AcceptanceReport {
        schema_version: REPORT_SCHEMA_VERSION,
        fixture_id: fixture.fixture_id,
        hash_algorithm: "blake3",
        fixture_hash,
        motion_report_hash,
        motion_source_selection: selection,
        scenes,
        deterministic_replay_hash,
        hard_safety_passed,
        fallback_count,
        unrelated_agent_mutations,
        passed,
        reasons,
    })
}

fn validate_fixture(fixture: &AcceptanceFixture) -> Result<(), String> {
    if fixture.schema_version != REPORT_SCHEMA_VERSION || fixture.fixture_id.is_empty() {
        return Err("acceptance fixture requires schema version 1 and a non-empty ID".to_owned());
    }
    let expected = [
        "scheduled_cafe",
        "family_split_regroup",
        "terrain_motion_feedback",
        "paired_handoff",
        "ragdoll_recovery",
        "mixed_tier_diagnostics",
    ];
    let actual = fixture
        .scenes
        .iter()
        .map(|scene| scene.id.as_str())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(format!(
            "acceptance fixture scenes must be present in canonical order: {expected:?}"
        ));
    }
    for scene in &fixture.scenes {
        let unique_sources = scene.source_paths.iter().collect::<BTreeSet<_>>();
        if scene.tick_start > scene.tick_end
            || scene.agent_count == 0
            || scene.agent_count > u64::from(u32::MAX)
            || scene.source_paths.is_empty()
            || unique_sources.len() != scene.source_paths.len()
            || scene.criteria.minimum_contact_precision_millionths > 1_000_000
        {
            return Err(format!(
                "scene {} has an invalid acceptance declaration",
                scene.id
            ));
        }
    }
    Ok(())
}

fn validate_motion_inputs(
    database: &MotionDatabaseInput,
    provenance: &MotionProvenanceInput,
    thresholds: &MotionThresholdInput,
    report: &CmuMotionReportInput,
    database_hash: &str,
) -> Result<(), String> {
    if database.schema_version != 1
        || database.database_id != PINNED_DATABASE_ID
        || database.retarget_profile_id != PINNED_RETARGET_PROFILE_ID
        || database.source_provenance != "Blender Crowd redistributable reference metadata"
        || database.clips.len() != 2
        || database.clips[0].id != "walk-reference"
        || database.clips[1].id != "jog-reference"
    {
        return Err("pinned CC0 authored motion database identity is invalid".to_owned());
    }
    if provenance.schema_version != 1
        || provenance.asset_id != PINNED_PROVENANCE_ASSET_ID
        || provenance.source_uri != PINNED_PROVENANCE_SOURCE_URI
        || provenance.content_hash != database_hash
        || provenance.license_id != "CC0-1.0"
        || !provenance.redistribution_allowed
        || provenance.terms_reference != "docs/m6-motion-data-policy.md"
        || provenance.checkpoint_hash.is_some()
    {
        return Err(
            "pinned motion provenance must identify and hash the checked redistributable CC0 database"
                .to_owned(),
        );
    }
    if thresholds.schema_version != 1
        || thresholds.threshold_id != PINNED_THRESHOLD_ID
        || thresholds.baseline_report != "docs/benchmarks/2026-08-18-m6-cmu-motion.json"
        || thresholds.source_manifest != "assets/reference/m6/cmu-motion-source-v1.json"
        || thresholds.hard_limits.joint_limit_violations.limit != 0
        || thresholds
            .hard_limits
            .joint_limit_violations
            .evidence_status
            != "measured"
        || thresholds.hard_limits.joint_limit_violations.baseline != PINNED_CMU_OBSERVATIONS
        || report.schema_version != 1
        || report.database_id != PINNED_CMU_DATABASE_ID
        || report.source_manifest_id != PINNED_CMU_MANIFEST_ID
        || report.source_hash != PINNED_CMU_SOURCE_HASH
        || report.quality_metrics.joint_limit_violations != PINNED_CMU_OBSERVATIONS
        || report.hard_limit_evidence.joint_limit_violations.observed != PINNED_CMU_OBSERVATIONS
        || report.hard_limit_evidence.joint_limit_violations.status != "measured"
    {
        return Err(
            "pinned CMU report/threshold identity or hard-zero relationship is invalid".to_owned(),
        );
    }
    Ok(())
}

fn verify_pinned_digest(label: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{label} digest does not match pinned bytes: expected {expected}, found {actual}"
        ))
    }
}

fn build_motion_database(input: &MotionDatabaseInput) -> Result<MotionDatabaseV1, String> {
    let clips = input
        .clips
        .iter()
        .map(|clip| {
            let mut position = [0i64, 0i64];
            let mut previous_tick = None;
            let samples = clip
                .samples
                .iter()
                .map(|sample| {
                    if let Some(previous) = previous_tick {
                        let delta =
                            i64::try_from(sample.tick.saturating_sub(previous)).unwrap_or(i64::MAX);
                        for (axis, position_component) in position.iter_mut().enumerate() {
                            *position_component = position_component.saturating_add(
                                i64::from(sample.velocity_millimeters_per_second[axis])
                                    .saturating_mul(delta)
                                    / TICKS_PER_SECOND,
                            );
                        }
                    }
                    previous_tick = Some(sample.tick);
                    MotionSampleV1::at(
                        sample.tick,
                        [position[0] as i32, position[1] as i32],
                        sample.velocity_millimeters_per_second,
                        sample.contact,
                        sample.slope_millionths,
                    )
                })
                .collect();
            MotionClipV1::new(&clip.id, &input.source_provenance, samples)
        })
        .collect();
    MotionDatabaseV1::new(&input.database_id, clips).map_err(|error| error.to_string())
}

fn run_scene(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    fixture_hash: &str,
    shared_sources: &BTreeMap<String, String>,
) -> Result<SceneReport, String> {
    let mut ledger = SourceLedger::new(scene)?;
    let domain = match scene.id.as_str() {
        "scheduled_cafe" => run_scheduled_cafe(scene, database, &mut ledger)?,
        "family_split_regroup" => run_family_split_regroup(scene, database, &mut ledger)?,
        "terrain_motion_feedback" => run_terrain_motion_feedback(scene, database, &mut ledger)?,
        "paired_handoff" => run_paired_handoff(scene, database, &mut ledger)?,
        "ragdoll_recovery" => run_ragdoll_recovery(scene, database, &mut ledger)?,
        "mixed_tier_diagnostics" => run_mixed_tier_diagnostics(scene, database, &mut ledger)?,
        other => return Err(format!("unsupported M6 scene {other}")),
    };
    ledger.consume_shared(shared_sources);
    let source_hashes = ledger.finish()?;
    let isolation = measure_isolation(&domain)?;
    let motion = &domain.state.motion;

    let required_contacts = domain.required_contacts + motion.required_contacts;
    let observed_contacts = domain.observed_contacts + motion.observed_contacts;
    let contact_precision_millionths = if required_contacts == 0 {
        1_000_000
    } else {
        observed_contacts.min(required_contacts) * 1_000_000 / required_contacts
    };
    let safety_violations = domain.safety_violations
        + motion.safety_violations
        + isolation.base_cache_mutations.unwrap_or(0)
        + isolation.unrelated_agent_mutations.unwrap_or(0)
        + u32::from(isolation.status == "measured" && isolation.target_agent_mutations == 0);
    let fallback_count = 1 + motion.runtime_fallbacks;
    let hard_safety_passed = safety_violations == 0;
    let execution = ExecutionEvidence {
        executed_ticks: domain
            .state
            .tick_end
            .saturating_sub(domain.state.tick_start)
            .saturating_add(1),
        executed_agent_count: domain.state.base.records.len() as u64,
        motion_evaluated_agent_ticks: motion.evaluated_agent_ticks,
        initial_state_hash: domain.state.initial_state_hash.clone(),
        final_state_hash: frame_state_hash(&domain.state.composed)?,
        target_agent_mutations: isolation.target_agent_mutations,
    };
    let metrics = SceneMetrics {
        trajectory_fit_millimeters: motion.trajectory_fit_millimeters,
        foot_slide_millimeters: motion.foot_slide_millimeters,
        required_contacts,
        observed_contacts,
        contact_precision_millionths,
        safety_violations,
        runtime_motion_fallbacks: motion.runtime_fallbacks,
        scene_specific: domain.specific.into_report(execution),
    };

    let mut reasons = Vec::new();
    if !hard_safety_passed {
        reasons.push(format!(
            "hard safety failed with {safety_violations} violation(s)"
        ));
    }
    if metrics.trajectory_fit_millimeters > scene.criteria.max_trajectory_fit_millimeters {
        reasons.push(format!(
            "trajectory fit {} mm exceeds {} mm",
            metrics.trajectory_fit_millimeters, scene.criteria.max_trajectory_fit_millimeters
        ));
    }
    if metrics.foot_slide_millimeters > scene.criteria.max_foot_slide_millimeters {
        reasons.push(format!(
            "foot slide {} mm exceeds {} mm",
            metrics.foot_slide_millimeters, scene.criteria.max_foot_slide_millimeters
        ));
    }
    if metrics.contact_precision_millionths < scene.criteria.minimum_contact_precision_millionths {
        reasons.push(format!(
            "contact precision {} is below {}",
            metrics.contact_precision_millionths,
            scene.criteria.minimum_contact_precision_millionths
        ));
    }
    if metrics.safety_violations > scene.criteria.max_safety_violations {
        reasons.push(format!(
            "safety violations {} exceed {}",
            metrics.safety_violations, scene.criteria.max_safety_violations
        ));
    }
    if fallback_count > scene.criteria.max_fallback_count {
        reasons.push(format!(
            "fallback count {fallback_count} exceeds {}",
            scene.criteria.max_fallback_count
        ));
    }
    if isolation.unrelated_agent_mutations.unwrap_or(0)
        > scene.criteria.max_unrelated_agent_mutations
    {
        reasons.push(format!(
            "unrelated-agent mutations {} exceed {}",
            isolation.unrelated_agent_mutations.unwrap_or(0),
            scene.criteria.max_unrelated_agent_mutations
        ));
    }
    let passed = reasons.is_empty() && hard_safety_passed;
    if passed {
        reasons.push(
            "all scene criteria passed; rejected CMU candidate fell back to checked CC0 motion"
                .to_owned(),
        );
    }

    let deterministic_replay_hash = hash_serializable(&(
        &scene.id,
        domain.state.seed,
        fixture_hash,
        &source_hashes,
        domain.state.tick_start,
        domain.state.tick_end,
        domain.state.base.records.len(),
        domain.promoted_group_count,
        &metrics,
        fallback_count,
        isolation.base_cache_mutations,
        isolation.unrelated_agent_mutations,
    ))?;

    Ok(SceneReport {
        id: scene.id.clone(),
        seed: domain.state.seed,
        fixture_hash: fixture_hash.to_owned(),
        source_hashes,
        tick_start: domain.state.tick_start,
        tick_end: domain.state.tick_end,
        agent_count: domain.state.base.records.len() as u64,
        promoted_group_count: domain.promoted_group_count,
        deterministic_replay_hash,
        hard_safety_passed,
        isolation_status: isolation.status,
        isolation_reason: isolation.reason,
        base_cache_mutations: isolation.base_cache_mutations,
        metrics,
        fallback_count,
        unrelated_agent_mutations: isolation.unrelated_agent_mutations,
        passed,
        reasons,
    })
}

fn build_executed_state(
    scene: &SceneFixture,
    required_agent_ids: &[u64],
    database: &MotionDatabaseV1,
) -> Result<ExecutedSceneState, String> {
    let mut agent_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for agent_id in required_agent_ids {
        if seen.insert(*agent_id) {
            agent_ids.push(*agent_id);
        }
    }
    if agent_ids.len() > scene.agent_count as usize {
        return Err(format!(
            "scene {} population {} cannot contain {} required runtime agents",
            scene.id,
            scene.agent_count,
            agent_ids.len()
        ));
    }
    let mut ordinal = 0u32;
    while agent_ids.len() < scene.agent_count as usize {
        let candidate = derive_agent_id(scene.seed, 12, 1, ordinal).0;
        ordinal = ordinal.saturating_add(1);
        if seen.insert(candidate) {
            agent_ids.push(candidate);
        }
    }
    let mut frame = Frame {
        records: agent_ids
            .iter()
            .map(|agent_id| {
                let id = AgentId(*agent_id);
                let mut position_rng = StableRng::for_agent(scene.seed, id, Purpose::SpawnPosition);
                let mut appearance_rng =
                    StableRng::for_agent(scene.seed, id, Purpose::AppearanceChoice);
                FrameRecord {
                    agent_id: *agent_id,
                    position: [
                        position_rng.range_f32(40.0, 80.0),
                        position_rng.range_f32(40.0, 80.0),
                    ],
                    scale: 1.0,
                    clip_id: appearance_rng.range_u32(1, 3) as u16,
                    phase: appearance_rng.next_f32_unit(),
                    playback_rate: 1.0,
                    visible: true,
                    ..FrameRecord::default()
                }
            })
            .collect(),
    };
    let initial_state_hash = frame_state_hash(&frame)?;
    let motion = execute_motion(scene, database, &mut frame)?;
    let base_snapshot = frame.clone();
    Ok(ExecutedSceneState {
        seed: scene.seed,
        tick_start: scene.tick_start,
        tick_end: scene.tick_end,
        initial_state_hash,
        base_snapshot,
        base: frame.clone(),
        composed: frame,
        motion,
    })
}

fn execute_motion(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    frame: &mut Frame,
) -> Result<MotionEvidence, String> {
    let matcher = MotionMatcher::new(database.clone());
    let fallback_clip_id = database
        .clips
        .first()
        .ok_or_else(|| "motion database has no fallback clip".to_owned())?
        .id
        .clone();
    let mut trajectory_fit_millimeters = 0;
    let mut required_contacts = 0u32;
    let mut observed_contacts = 0u32;
    let mut runtime_fallbacks = 0u32;
    let mut safety_violations = 0u32;
    let mut evaluated_agent_ticks = 0u64;
    for tick in scene.tick_start..=scene.tick_end {
        for record in &mut frame.records {
            let mut speed_rng = StableRng::for_agent(
                scene.seed ^ tick,
                AgentId(record.agent_id),
                Purpose::PreferredSpeed,
            );
            let speed_offset = speed_rng.range_u32(0, 11) as i32 - 5;
            let desired = [
                scene.motion.desired_velocity_millimeters_per_second[0] + speed_offset,
                scene.motion.desired_velocity_millimeters_per_second[1],
            ];
            let selected = matcher
                .select(&MotionQueryV1 {
                    desired_velocity_millimeters_per_second: desired,
                    desired_slope_millionths: scene.motion.desired_slope_millionths,
                    required_contact: scene.motion.required_contact,
                    fallback_clip_id: fallback_clip_id.clone(),
                    future_positions_millimeters: Vec::new(),
                    future_velocities_millimeters_per_second: Vec::new(),
                })
                .map_err(|error| error.to_string())?;
            let clip = database
                .clips
                .iter()
                .find(|clip| clip.id == selected.clip_id)
                .ok_or_else(|| format!("selected clip {} is missing", selected.clip_id))?;
            let sample = clip
                .samples
                .first()
                .ok_or_else(|| format!("selected clip {} is empty", selected.clip_id))?;
            let feedback = MotionFeedbackV1::evaluate(
                desired,
                sample.velocity_millimeters_per_second,
                scene.motion.desired_slope_millionths,
                sample.slope_millionths,
                scene.motion.foot_slide_millimeters.saturating_mul(1_000),
                scene
                    .criteria
                    .max_trajectory_fit_millimeters
                    .saturating_mul(1_000),
                scene
                    .criteria
                    .max_foot_slide_millimeters
                    .saturating_mul(1_000),
            );
            trajectory_fit_millimeters =
                trajectory_fit_millimeters.max(feedback.root_deviation_millionths / 1_000);
            safety_violations += u32::from(!feedback.feasible);
            runtime_fallbacks += u32::from(selected.used_fallback);
            if let Some(contact) = scene.motion.required_contact {
                required_contacts = required_contacts.saturating_add(1);
                observed_contacts = observed_contacts.saturating_add(u32::from(
                    clip.samples.iter().any(|sample| sample.contact == contact),
                ));
            }
            record.clip_id = if selected.clip_id == "jog-reference" {
                2
            } else {
                1
            };
            record.velocity = [
                sample.velocity_millimeters_per_second[0] as f32 / 1_000.0,
                sample.velocity_millimeters_per_second[1] as f32 / 1_000.0,
            ];
            for axis in 0..2 {
                record.position[axis] += record.velocity[axis] / TICKS_PER_SECOND as f32;
            }
            record.phase = (record.phase + record.playback_rate / TICKS_PER_SECOND as f32).fract();
            evaluated_agent_ticks = evaluated_agent_ticks.saturating_add(1);
        }
    }
    Ok(MotionEvidence {
        trajectory_fit_millimeters,
        foot_slide_millimeters: scene.motion.foot_slide_millimeters,
        required_contacts,
        observed_contacts,
        runtime_fallbacks,
        safety_violations,
        evaluated_agent_ticks,
    })
}

impl DomainSpecificData {
    fn into_report(self, execution: ExecutionEvidence) -> SceneSpecificEvidence {
        match self {
            Self::ScheduledCafe {
                granted_reservations,
                waiting_before_release,
                promoted_after_release,
                double_ownership_violations,
            } => SceneSpecificEvidence::ScheduledCafe {
                execution,
                granted_reservations,
                waiting_before_release,
                promoted_after_release,
                double_ownership_violations,
            },
            Self::FamilySplitRegroup {
                split_samples,
                regrouped_samples,
                intrusion_samples,
                maximum_split_separation_millimeters,
            } => SceneSpecificEvidence::FamilySplitRegroup {
                execution,
                split_samples,
                regrouped_samples,
                intrusion_samples,
                maximum_split_separation_millimeters,
            },
            Self::TerrainMotionFeedback {
                terrain_constraints_accepted,
                foot_locks_satisfied,
                navigation_feedback_events,
            } => SceneSpecificEvidence::TerrainMotionFeedback {
                execution,
                terrain_constraints_accepted,
                foot_locks_satisfied,
                navigation_feedback_events,
            },
            Self::PairedHandoff {
                participants_locked_atomically,
                required_interaction_contacts,
                completed_interactions,
                applied_layer_edits,
            } => SceneSpecificEvidence::PairedHandoff {
                execution,
                participants_locked_atomically,
                required_interaction_contacts,
                completed_interactions,
                applied_layer_edits,
            },
            Self::RagdollRecovery {
                physics_cache_samples,
                impact_phase_ticks,
                stabilize_phase_ticks,
                resume_phase_ticks,
                floor_contact_samples,
                hero_boundary_validated,
            } => SceneSpecificEvidence::RagdollRecovery {
                execution,
                physics_cache_samples,
                impact_phase_ticks,
                stabilize_phase_ticks,
                resume_phase_ticks,
                floor_contact_samples,
                hero_boundary_validated,
            },
            Self::MixedTierDiagnostics {
                full_diagnostic_channels,
                reduced_diagnostic_channels,
                aggregate_diagnostic_channels,
                scheduled_animation_evaluations,
                validated_interaction_contacts,
                promoted_interaction_groups,
            } => SceneSpecificEvidence::MixedTierDiagnostics {
                execution,
                full_diagnostic_channels,
                reduced_diagnostic_channels,
                aggregate_diagnostic_channels,
                scheduled_animation_evaluations,
                validated_interaction_contacts,
                promoted_interaction_groups,
            },
        }
    }
}

impl SourceLedger {
    fn new(scene: &SceneFixture) -> Result<Self, String> {
        let mut declared = BTreeMap::new();
        for source in &scene.source_paths {
            let path = PathBuf::from(source);
            let key = path_text(&path);
            if declared.insert(key.clone(), path).is_some() {
                return Err(format!("scene {} duplicates source {key}", scene.id));
            }
        }
        Ok(Self {
            scene_id: scene.id.clone(),
            declared,
            consumed: BTreeMap::new(),
        })
    }

    fn consume<T: DeserializeOwned>(&mut self, suffix: &str) -> Result<T, String> {
        let matches = self
            .declared
            .iter()
            .filter(|(key, _)| key.ends_with(suffix))
            .map(|(key, path)| (key.clone(), path.clone()))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "scene {} requires exactly one declared source ending in {suffix}",
                self.scene_id
            ));
        }
        let (key, path) = &matches[0];
        let bytes = read_bytes(path)?;
        self.consumed.insert(key.clone(), hash_bytes(&bytes));
        parse_bytes(path, &bytes)
    }

    fn consume_shared(&mut self, sources: &BTreeMap<String, String>) {
        self.consumed.extend(
            sources
                .iter()
                .map(|(key, hash)| (key.clone(), hash.clone())),
        );
    }

    fn finish(self) -> Result<BTreeMap<String, String>, String> {
        let unconsumed = self
            .declared
            .keys()
            .filter(|key| !self.consumed.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>();
        if unconsumed.is_empty() {
            Ok(self.consumed)
        } else {
            Err(format!(
                "scene {} declared but not consumed sources: {}",
                self.scene_id,
                unconsumed.join(", ")
            ))
        }
    }
}

fn frame_state_hash(frame: &Frame) -> Result<String, String> {
    let records = frame
        .records
        .iter()
        .map(|record| StateDigestRecord {
            agent_id: record.agent_id,
            position: [record.position[0].to_bits(), record.position[1].to_bits()],
            orientation: record.orientation.to_bits(),
            scale: record.scale.to_bits(),
            population_id: record.population_id,
            variant_id: record.variant_id,
            clip_id: record.clip_id,
            phase: record.phase.to_bits(),
            playback_rate: record.playback_rate.to_bits(),
            behavior_state: record.behavior_state,
            decision_reason: record.decision_reason,
            destination_id: record.destination_id,
            velocity: [record.velocity[0].to_bits(), record.velocity[1].to_bits()],
            visible: record.visible,
            render_tier: record.render_tier,
        })
        .collect::<Vec<_>>();
    hash_serializable(&records)
}

fn measure_isolation(domain: &DomainEvidence) -> Result<IsolationEvidence, String> {
    if !domain.isolation_applicable {
        return Ok(IsolationEvidence {
            status: "not_applicable",
            reason: domain
                .isolation_reason
                .clone()
                .or_else(|| Some("no promoted runtime operation was executed".to_owned())),
            base_cache_mutations: None,
            unrelated_agent_mutations: None,
            target_agent_mutations: 0,
        });
    }
    let targets = domain
        .target_agent_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let base_cache_mutations = frame_mutations(&domain.state.base_snapshot, &domain.state.base)?;
    let unrelated_agent_mutations =
        frame_mutations_filtered(&domain.state.base, &domain.state.composed, |agent_id| {
            !targets.contains(&agent_id)
        })?;
    let target_agent_mutations =
        frame_mutations_filtered(&domain.state.base, &domain.state.composed, |agent_id| {
            targets.contains(&agent_id)
        })?;
    Ok(IsolationEvidence {
        status: "measured",
        reason: None,
        base_cache_mutations: Some(base_cache_mutations),
        unrelated_agent_mutations: Some(unrelated_agent_mutations),
        target_agent_mutations,
    })
}

fn frame_mutations(before: &Frame, after: &Frame) -> Result<u32, String> {
    frame_mutations_filtered(before, after, |_| true)
}

fn frame_mutations_filtered(
    before: &Frame,
    after: &Frame,
    include: impl Fn(u64) -> bool,
) -> Result<u32, String> {
    if before.records.len() != after.records.len() {
        return Err("scene operation changed full-state population length".to_owned());
    }
    let after_by_id = after
        .records
        .iter()
        .map(|record| (record.agent_id, record))
        .collect::<BTreeMap<_, _>>();
    Ok(before
        .records
        .iter()
        .filter(|record| include(record.agent_id))
        .filter(|record| after_by_id.get(&record.agent_id).copied() != Some(*record))
        .count() as u32)
}

fn run_scheduled_cafe(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    ledger: &mut SourceLedger,
) -> Result<DomainEvidence, String> {
    let schedule: ActivityScheduleV1 = ledger.consume("activity-v1.json")?;
    schedule.validate().map_err(|error| error.to_string())?;
    if scene.agent_count < 3 {
        return Err("scheduled_cafe requires at least three executed agents".to_owned());
    }
    let mut state = build_executed_state(scene, &[], database)?;
    let resource_id = schedule
        .resources
        .first()
        .ok_or_else(|| "scheduled cafe has no resource".to_owned())?
        .clone();
    let mut runtime = ReservationRuntimeV1::new(vec![ResourceV1 {
        id: resource_id.clone(),
        capacity: schedule.capacity,
    }])
    .map_err(|error| error.to_string())?;
    let active_ticks = (scene.tick_start..=scene.tick_end)
        .filter(|tick| schedule.is_active(*tick))
        .count() as u64;
    let request_ids = state
        .base
        .records
        .iter()
        .take(3)
        .map(|record| record.agent_id)
        .collect::<Vec<_>>();
    let requests = request_ids
        .iter()
        .map(|agent_id| ActivityRequestV1 {
            agent_id: *agent_id,
            resource_id: resource_id.clone(),
            priority: 10,
        })
        .collect::<Vec<_>>();
    let results = if active_ticks > 0 {
        runtime.request_batch(&requests)
    } else {
        Vec::new()
    };
    let granted = results
        .iter()
        .filter(|result| result.status == ReservationStatusV1::Granted)
        .count() as u64;
    let waiting = results
        .iter()
        .filter(|result| matches!(result.status, ReservationStatusV1::Waiting { .. }))
        .count() as u64;
    let promoted_targets = runtime.owners(&resource_id);
    let first_owner = promoted_targets.first().copied();
    let released = first_owner.is_some_and(|agent_id| runtime.release(agent_id, &resource_id));
    let waiting_agent = results
        .iter()
        .find(|result| matches!(result.status, ReservationStatusV1::Waiting { .. }))
        .map(|result| result.agent_id)
        .unwrap_or(0);
    let promoted_after_release =
        u64::from(runtime.status(waiting_agent, &resource_id) == ReservationStatusV1::Granted);
    let owners = runtime.owners(&resource_id);
    let unique_owners = owners.iter().copied().collect::<BTreeSet<_>>().len();
    let promoted_group_count = u32::from(
        active_ticks > 0 && schedule.paired_action.is_some() && promoted_targets.len() >= 2,
    );
    if promoted_group_count > 0 {
        let tick = scene.tick_start + (scene.tick_end - scene.tick_start) / 2;
        let base_hash = frame_state_hash(&state.base)?;
        let layer = AnimationLayerV1 {
            schema_version: INTERACTION_LAYER_SCHEMA_VERSION,
            layer_id: "scheduled-cafe-pair".to_owned(),
            interaction_id: schedule
                .paired_action
                .as_ref()
                .map(|paired| paired.action_id.clone())
                .unwrap_or_default(),
            base_cache_hash: base_hash.clone(),
            target_agent_ids: promoted_targets.clone(),
            tick_start: scene.tick_start,
            tick_end: scene.tick_end,
            priority: 10,
            enabled: true,
            provenance: "scheduled-cafe-runtime-v1".to_owned(),
            edits: promoted_targets
                .iter()
                .enumerate()
                .map(|(index, agent_id)| AnimationEditV1 {
                    agent_id: *agent_id,
                    tick,
                    clip_id: 30 + index as u16,
                    phase_millionths: 500_000,
                })
                .collect(),
            fallback: FallbackClipV1 {
                clip_set_id: PINNED_DATABASE_ID.to_owned(),
                clip_id: "walk-reference".to_owned(),
                reason: "checked CC0 fallback".to_owned(),
            },
        };
        state.composed = compose_interaction_frame_v1(&state.base, tick, &base_hash, &[layer])
            .map_err(|error| error.to_string())?;
    }
    let safety_violations = u32::from(active_ticks == 0)
        + u32::from(granted != 2)
        + u32::from(waiting != 1)
        + u32::from(!released)
        + u32::from(promoted_after_release != 1)
        + u32::from(unique_owners != owners.len());
    Ok(DomainEvidence {
        promoted_group_count,
        required_contacts: promoted_group_count,
        observed_contacts: u32::from(promoted_group_count > 0 && granted >= 2),
        safety_violations,
        target_agent_ids: promoted_targets,
        isolation_applicable: promoted_group_count > 0,
        isolation_reason: (promoted_group_count == 0)
            .then(|| "no promoted scheduled paired activity was executed".to_owned()),
        specific: DomainSpecificData::ScheduledCafe {
            granted_reservations: granted,
            waiting_before_release: waiting,
            promoted_after_release,
            double_ownership_violations: (owners.len() - unique_owners) as u64,
        },
        state,
    })
}

fn run_family_split_regroup(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    ledger: &mut SourceLedger,
) -> Result<DomainEvidence, String> {
    let formation: FormationV1 = ledger.consume("formation-v1.json")?;
    formation.validate().map_err(|error| error.to_string())?;
    let member_ids = formation
        .roles
        .iter()
        .map(|role| role.agent_id.0)
        .collect::<Vec<_>>();
    let state = build_executed_state(scene, &member_ids, database)?;
    let members = member_ids.iter().copied().collect::<BTreeSet<_>>();
    let candidates = state
        .base
        .records
        .iter()
        .filter(|record| !members.contains(&record.agent_id))
        .map(|record| {
            (
                AgentId(record.agent_id),
                Vec2::new(record.position[0], record.position[1]),
            )
        })
        .collect::<Vec<_>>();
    let midpoint = scene.tick_start + (scene.tick_end - scene.tick_start) / 2;
    let farthest = formation
        .roles
        .last()
        .ok_or_else(|| "formation has no runtime roles".to_owned())?
        .agent_id;
    let mut split_samples = 0u64;
    let mut regrouped_samples = 0u64;
    let mut intrusion_samples = 0u64;
    let mut maximum_separation_m = 0.0f32;
    let mut split_positions_for_correction = None;
    for tick in scene.tick_start..=scene.tick_end {
        let mut positions = formation
            .roles
            .iter()
            .map(|role| {
                (
                    role.agent_id,
                    Vec2::new(
                        role.offset_millimeters[0] as f32 / 1_000.0,
                        role.offset_millimeters[1] as f32 / 1_000.0,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if tick <= midpoint {
            let split_offset = formation.max_separation_millimeters as f32 / 1_000.0 + 1.0;
            positions.insert(farthest, Vec2::new(split_offset, 0.0));
            split_positions_for_correction = Some(positions.clone());
        }
        let report = formation.evaluate(&positions, &candidates);
        split_samples += u64::from(report.split);
        regrouped_samples += u64::from(!report.split);
        intrusion_samples += u64::from(!report.intruder_agent_ids.is_empty());
        maximum_separation_m = maximum_separation_m.max(report.maximum_separation_m);
    }
    let correction = formation.cohesion_velocity(
        farthest,
        split_positions_for_correction
            .as_ref()
            .ok_or_else(|| "family scene executed no split interval".to_owned())?,
        0.75,
    );
    let safety_violations = u32::from(split_samples == 0)
        + u32::from(regrouped_samples == 0)
        + u32::from(intrusion_samples > 0)
        + u32::from(correction.length() > 0.750_001);
    Ok(DomainEvidence {
        promoted_group_count: 0,
        required_contacts: 0,
        observed_contacts: 0,
        safety_violations,
        target_agent_ids: member_ids,
        isolation_applicable: false,
        isolation_reason: Some(
            "no promoted layer/runtime operation applies to formation evaluation".to_owned(),
        ),
        specific: DomainSpecificData::FamilySplitRegroup {
            split_samples,
            regrouped_samples,
            intrusion_samples,
            maximum_split_separation_millimeters: (maximum_separation_m * 1_000.0).round() as u64,
        },
        state,
    })
}

fn run_terrain_motion_feedback(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    ledger: &mut SourceLedger,
) -> Result<DomainEvidence, String> {
    let input: TerrainMotionInput = ledger.consume("terrain-motion-v1.json")?;
    let terrain = TerrainConstraintV1 {
        max_slope_millionths: input.max_slope_millionths,
        ground_height_millimeters: input.ground_height_millimeters,
    };
    terrain.validate().map_err(str::to_owned)?;
    let foot_locks = input
        .foot_locks
        .iter()
        .map(FootLockInput::runtime)
        .collect::<Vec<_>>();
    for lock in &foot_locks {
        lock.validate().map_err(str::to_owned)?;
    }
    let state = build_executed_state(scene, &[], database)?;
    let mut terrain_constraints_accepted = 0u64;
    let mut foot_locks_satisfied = 0u64;
    let mut navigation_feedback_events = 0u64;
    for tick in scene.tick_start..=scene.tick_end {
        let accepted = terrain.accepts_slope(scene.motion.desired_slope_millionths);
        terrain_constraints_accepted += u64::from(accepted);
        for lock in &foot_locks {
            if lock.contains_tick(tick) {
                foot_locks_satisfied += 1;
                navigation_feedback_events +=
                    u64::from(accepted && input.navigation_feedback == "warp_stride");
            }
        }
    }
    let required_contacts = foot_locks
        .iter()
        .map(|lock| {
            (scene.tick_start..=scene.tick_end)
                .filter(|tick| lock.contains_tick(*tick))
                .count() as u32
        })
        .sum::<u32>();
    let safety_violations = u32::from(input.schema_version != 1)
        + u32::from(input.scene_id != "sloped-platform")
        + u32::from(input.terrain_id != "ramp-reference")
        + u32::from(input.navigation_feedback != "warp_stride")
        + u32::from(!terrain.accepts_slope(scene.motion.desired_slope_millionths))
        + u32::from(required_contacts == 0)
        + u32::from(foot_locks_satisfied != u64::from(required_contacts));
    Ok(DomainEvidence {
        promoted_group_count: 0,
        required_contacts,
        observed_contacts: foot_locks_satisfied as u32,
        safety_violations,
        target_agent_ids: Vec::new(),
        isolation_applicable: false,
        isolation_reason: Some(
            "no promoted layer/runtime operation applies to terrain feedback evaluation".to_owned(),
        ),
        specific: DomainSpecificData::TerrainMotionFeedback {
            terrain_constraints_accepted,
            foot_locks_satisfied,
            navigation_feedback_events,
        },
        state,
    })
}

fn run_paired_handoff(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    ledger: &mut SourceLedger,
) -> Result<DomainEvidence, String> {
    let request: InteractionRequestV1 = ledger.consume("interaction-request-v1.json")?;
    let checked_motion: InteractionMotionV1 = ledger.consume("interaction-motion-v1.json")?;
    let layer: AnimationLayerV1 = ledger.consume("interaction-animation-layer-v1.json")?;
    checked_motion
        .validate_against(&request)
        .map_err(|issues| format!("checked paired motion is invalid: {issues:?}"))?;
    deterministic_paired_clip(&request)
        .map_err(|issues| format!("deterministic paired clip failed: {issues:?}"))?;
    layer.validate().map_err(|error| error.to_string())?;
    let participant_ids = request
        .participants
        .iter()
        .map(|participant| participant.agent_id)
        .collect::<Vec<_>>();
    if scene.tick_start != request.tick_start
        || scene.tick_end != request.tick_end
        || layer.interaction_id != request.request_id
        || layer.base_cache_hash != request.provenance.base_cache_hash
        || layer.target_agent_ids != participant_ids
        || layer.tick_start != request.tick_start
        || layer.tick_end != request.tick_end
    {
        return Err("paired scene tick/participant/layer relationships are invalid".to_owned());
    }
    let mut state = build_executed_state(scene, &participant_ids, database)?;
    let mut scheduler = InteractionSchedulerV1::new(1);
    scheduler.enqueue(request.clone())?;
    let promoted = scheduler.promote_next();
    let atomically_locked = request
        .participants
        .iter()
        .all(|participant| scheduler.active_group_for(participant.agent_id) == promoted.as_deref());
    let completed = scheduler.complete(&request.request_id);
    let required = request
        .contact_constraints
        .iter()
        .filter(|contact| contact.required)
        .count() as u32;
    let observed = checked_motion
        .contacts
        .iter()
        .filter(|contact| {
            request.contact_constraints.iter().any(|constraint| {
                constraint.required && constraint.contact_id == contact.contact_id
            })
        })
        .count() as u32;
    let edit_ticks = layer
        .edits
        .iter()
        .map(|edit| edit.tick)
        .collect::<BTreeSet<_>>();
    for tick in edit_ticks {
        state.composed = compose_interaction_frame_v1(
            &state.composed,
            tick,
            &request.provenance.base_cache_hash,
            std::slice::from_ref(&layer),
        )
        .map_err(|error| error.to_string())?;
    }
    let applied_layer_edits = layer
        .edits
        .iter()
        .filter(|edit| (scene.tick_start..=scene.tick_end).contains(&edit.tick))
        .count() as u64;
    let safety_violations = u32::from(promoted.as_deref() != Some(request.request_id.as_str()))
        + u32::from(!atomically_locked)
        + u32::from(completed != Some(InteractionGroupStatusV1::Completed))
        + u32::from(required != observed)
        + u32::from(applied_layer_edits == 0);
    Ok(DomainEvidence {
        promoted_group_count: u32::from(promoted.is_some()),
        required_contacts: required,
        observed_contacts: observed,
        safety_violations,
        target_agent_ids: participant_ids,
        isolation_applicable: promoted.is_some(),
        isolation_reason: promoted
            .is_none()
            .then(|| "no promoted paired handoff operation was executed".to_owned()),
        specific: DomainSpecificData::PairedHandoff {
            participants_locked_atomically: u64::from(atomically_locked),
            required_interaction_contacts: u64::from(required),
            completed_interactions: u64::from(completed.is_some()),
            applied_layer_edits,
        },
        state,
    })
}

fn run_ragdoll_recovery(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    ledger: &mut SourceLedger,
) -> Result<DomainEvidence, String> {
    let transition: PhysicsTransitionV1 = ledger.consume("physics-transition-v1.json")?;
    let hero: HeroIntegrationBoundaryV1 = ledger.consume("hero-integration-v1.json")?;
    validate_transition(&transition).map_err(|errors| errors.join("; "))?;
    hero.validate().map_err(|errors| errors.join("; "))?;
    if scene.tick_start != transition.tick_start || scene.tick_end != transition.tick_end {
        return Err("ragdoll scene tick range must match the consumed transition".to_owned());
    }
    let hero_boundary_validated = u64::from(
        hero.cache_policy == "adjacent-layer"
            && hero
                .supported_render_tiers
                .iter()
                .any(|tier| tier == "hero")
            && hero.failure_policy.starts_with("fallback"),
    );
    let mut state = build_executed_state(scene, &transition.agent_ids, database)?;
    let owner = transition.agent_ids[0];
    let owner_record = state
        .base
        .records
        .iter()
        .find(|record| record.agent_id == owner)
        .ok_or_else(|| "ragdoll owner is missing from full scene state".to_owned())?;
    let samples = simulate_physics_handoff_v1(&PhysicsHandoffSpecV1 {
        tick_start: scene.tick_start,
        tick_end: scene.tick_end,
        ticks_per_second: 30,
        incoming_position: [owner_record.position[0], owner_record.position[1], 0.5],
        incoming_velocity: [
            owner_record.velocity[0] + 0.5,
            owner_record.velocity[1],
            -2.0,
        ],
        gravity_mps2: -9.81,
        floor_z: 0.0,
        restitution_millionths: 0,
        collision_masks: vec!["crowd".to_owned(), "ground".to_owned()],
    })
    .map_err(|error| error.to_string())?;
    let phases = (scene.tick_start..=scene.tick_end)
        .map(|tick| recovery_phase(&transition, tick, 3))
        .collect::<Vec<_>>();
    let impact = phases
        .iter()
        .filter(|phase| **phase == RecoveryPhaseV1::Impact)
        .count() as u64;
    let stabilize = phases
        .iter()
        .filter(|phase| **phase == RecoveryPhaseV1::Stabilize)
        .count() as u64;
    let resume = phases
        .iter()
        .filter(|phase| **phase == RecoveryPhaseV1::Resume)
        .count() as u64;
    let floor_contacts = samples
        .iter()
        .filter(|sample| sample.position[2] == 0.0)
        .count() as u32;
    let expected_samples = scene
        .tick_end
        .saturating_sub(scene.tick_start)
        .saturating_add(1);
    let promoted_group_count = u32::from(hero_boundary_validated == 1);
    if let Some(last) = samples.last() {
        for record in &mut state.composed.records {
            if transition.agent_ids.contains(&record.agent_id) {
                record.position = [last.position[0], last.position[1]];
                record.velocity = [last.velocity[0], last.velocity[1]];
                record.clip_id = 90;
            }
        }
    }
    let safety_violations = u32::from(samples.len() as u64 != expected_samples)
        + u32::from(impact == 0 || stabilize == 0 || resume == 0)
        + u32::from(floor_contacts == 0)
        + u32::from(samples.iter().any(|sample| sample.position[2] < 0.0))
        + u32::from(hero_boundary_validated == 0);
    Ok(DomainEvidence {
        promoted_group_count,
        required_contacts: 1,
        observed_contacts: u32::from(floor_contacts > 0),
        safety_violations,
        target_agent_ids: transition.agent_ids,
        isolation_applicable: promoted_group_count > 0,
        isolation_reason: (promoted_group_count == 0)
            .then(|| "no validated hero recovery operation was promoted".to_owned()),
        specific: DomainSpecificData::RagdollRecovery {
            physics_cache_samples: samples.len() as u64,
            impact_phase_ticks: impact,
            stabilize_phase_ticks: stabilize,
            resume_phase_ticks: resume,
            floor_contact_samples: u64::from(floor_contacts),
            hero_boundary_validated,
        },
        state,
    })
}

fn run_mixed_tier_diagnostics(
    scene: &SceneFixture,
    database: &MotionDatabaseV1,
    ledger: &mut SourceLedger,
) -> Result<DomainEvidence, String> {
    let input: MixedTierInput = ledger.consume("mixed-tier-v1.json")?;
    let request: InteractionRequestV1 = ledger.consume("interaction-request-v1.json")?;
    let motion: InteractionMotionV1 = ledger.consume("interaction-motion-v1.json")?;
    motion
        .validate_against(&request)
        .map_err(|issues| format!("mixed-tier interaction motion is invalid: {issues:?}"))?;
    let total_agents = input.tiers.iter().map(|tier| tier.agent_count).sum::<u64>();
    if total_agents != scene.agent_count {
        return Err(format!(
            "mixed-tier source population {total_agents} does not match executed population {}",
            scene.agent_count
        ));
    }
    if input.base_cache_hash != request.provenance.base_cache_hash
        || !input
            .promoted_interaction_groups
            .iter()
            .any(|group| group == &request.group_id)
    {
        return Err(
            "mixed-tier promotion does not match the consumed interaction request".to_owned(),
        );
    }
    let participant_ids = request
        .participants
        .iter()
        .map(|participant| participant.agent_id)
        .collect::<Vec<_>>();
    let mut state = build_executed_state(scene, &participant_ids, database)?;
    let mut safety_violations = u32::from(input.schema_version != 1)
        + u32::from(input.scene_id.is_empty())
        + u32::from(input.base_cache_hash.len() != 64);
    let mut full_diagnostics = 0u64;
    let mut reduced_diagnostics = 0u64;
    let mut aggregate_diagnostics = 0u64;
    let mut scheduled_animation_evaluations = 0u64;
    let mut record_cursor = 0usize;
    for tier in &input.tiers {
        let simulation = parse_simulation_tier(&tier.simulation_tier)?;
        let expected_render = parse_render_tier(&tier.render_tier)?;
        safety_violations += u32::from(render_for(simulation) != expected_render);
        match simulation {
            SimulationTier::S0 => full_diagnostics += tier.diagnostics.len() as u64,
            SimulationTier::S1 => reduced_diagnostics += tier.diagnostics.len() as u64,
            SimulationTier::S2 | SimulationTier::S3 => {
                aggregate_diagnostics += tier.diagnostics.len() as u64
            }
        }
        for _ in 0..tier.agent_count {
            let agent_id = AgentId(state.base.records[record_cursor].agent_id);
            for tick in scene.tick_start..=scene.tick_end {
                scheduled_animation_evaluations +=
                    u64::from(FidelityPolicy::animation_due(simulation, agent_id, tick));
            }
            record_cursor += 1;
        }
    }
    let mut scheduler = InteractionSchedulerV1::new(input.promoted_interaction_groups.len());
    scheduler.enqueue(request.clone())?;
    let promoted = scheduler.promote_next();
    let atomically_locked = request
        .participants
        .iter()
        .all(|participant| scheduler.active_group_for(participant.agent_id) == promoted.as_deref());
    let validated_interaction_contacts = motion
        .contacts
        .iter()
        .filter(|contact| (scene.tick_start..=scene.tick_end).contains(&contact.tick))
        .count() as u64;
    for participant in &motion.participants {
        let sample = participant
            .root_samples
            .iter()
            .filter(|sample| (scene.tick_start..=scene.tick_end).contains(&sample.tick))
            .max_by_key(|sample| sample.tick);
        if let Some(sample) = sample {
            let record = state
                .composed
                .records
                .iter_mut()
                .find(|record| record.agent_id == participant.agent_id)
                .ok_or_else(|| {
                    "mixed-tier participant is missing from full scene state".to_owned()
                })?;
            record.position = [sample.translation[0], sample.translation[1]];
            record.orientation = sample.yaw;
            record.clip_id = 42;
        }
    }
    let promoted_group_count = u32::from(promoted.is_some() && atomically_locked);
    safety_violations += u32::from(full_diagnostics == 0)
        + u32::from(reduced_diagnostics == 0)
        + u32::from(aggregate_diagnostics == 0)
        + u32::from(promoted_group_count == 0)
        + u32::from(validated_interaction_contacts == 0);
    Ok(DomainEvidence {
        promoted_group_count,
        required_contacts: request
            .contact_constraints
            .iter()
            .filter(|contact| contact.required)
            .count() as u32,
        observed_contacts: validated_interaction_contacts as u32,
        safety_violations,
        target_agent_ids: participant_ids,
        isolation_applicable: promoted_group_count > 0,
        isolation_reason: (promoted_group_count == 0)
            .then(|| "no mixed-tier interaction group was promoted".to_owned()),
        specific: DomainSpecificData::MixedTierDiagnostics {
            full_diagnostic_channels: full_diagnostics,
            reduced_diagnostic_channels: reduced_diagnostics,
            aggregate_diagnostic_channels: aggregate_diagnostics,
            scheduled_animation_evaluations,
            validated_interaction_contacts,
            promoted_interaction_groups: u64::from(promoted_group_count),
        },
        state,
    })
}

fn parse_simulation_tier(value: &str) -> Result<SimulationTier, String> {
    match value {
        "s0" => Ok(SimulationTier::S0),
        "s1" => Ok(SimulationTier::S1),
        "s2" => Ok(SimulationTier::S2),
        "s3" => Ok(SimulationTier::S3),
        _ => Err(format!("unknown simulation tier {value}")),
    }
}

fn parse_render_tier(value: &str) -> Result<RenderTier, String> {
    match value {
        "r0" => Ok(RenderTier::R0),
        "r1" => Ok(RenderTier::R1),
        "r2" => Ok(RenderTier::R2),
        "r3" => Ok(RenderTier::R3),
        "r4" => Ok(RenderTier::R4),
        _ => Err(format!("unknown render tier {value}")),
    }
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn parse_bytes<T: DeserializeOwned>(path: &Path, bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn hash_serializable(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| hash_bytes(&bytes))
        .map_err(|error| format!("failed to serialize replay evidence: {error}"))
}

fn hash_bytes(bytes: &[u8]) -> String {
    content_hash(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn path_text(path: &Path) -> String {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let normalized = env::current_dir()
        .ok()
        .and_then(|root| fs::canonicalize(root).ok())
        .and_then(|root| canonical.strip_prefix(root).ok())
        .unwrap_or(&canonical);
    normalized.to_string_lossy().replace('\\', "/")
}

fn write_report(path: &Path, report: &AcceptanceReport) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("output path {} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("failed to serialize report: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}
