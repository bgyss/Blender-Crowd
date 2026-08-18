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
    recovery_phase, validate_transition, PhysicsTransitionV1, RecoveryPhaseV1,
};
use crowd_core::{AgentId, Vec2};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const REPORT_SCHEMA_VERSION: u32 = 1;
const TICKS_PER_SECOND: i64 = 30;

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
    target_agent_ids: Vec<u64>,
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
    hard_limits: MotionHardLimitsInput,
}

#[derive(Debug, Deserialize)]
struct MotionHardLimitsInput {
    joint_limit_violations: HardLimitInput,
}

#[derive(Debug, Deserialize)]
struct HardLimitInput {
    limit: u64,
}

#[derive(Debug, Deserialize)]
struct CmuMotionReportInput {
    schema_version: u32,
    database_id: String,
    quality_metrics: CmuQualityMetricsInput,
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
    metrics: SceneMetrics,
    fallback_count: u32,
    unrelated_agent_mutations: u32,
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
    scene_specific: BTreeMap<String, u64>,
}

#[derive(Default)]
struct DomainEvidence {
    promoted_group_count: u32,
    required_contacts: u32,
    observed_contacts: u32,
    safety_violations: u32,
    specific: BTreeMap<String, u64>,
}

struct MotionEvidence {
    trajectory_fit_millimeters: u32,
    foot_slide_millimeters: u32,
    required_contacts: u32,
    observed_contacts: u32,
    used_fallback: bool,
    safety_violations: u32,
    clip_id: String,
}

struct IsolationEvidence {
    base_cache_mutations: u32,
    unrelated_agent_mutations: u32,
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
    let database_input: MotionDatabaseInput = read_json(database_path)?;
    let provenance: MotionProvenanceInput = read_json(provenance_path)?;
    let thresholds: MotionThresholdInput = read_json(thresholds_path)?;
    let motion_report: CmuMotionReportInput = read_json(motion_report_path)?;
    validate_motion_inputs(&database_input, &provenance, &thresholds, &motion_report)?;
    let database = build_motion_database(&database_input)?;

    let observed = motion_report.quality_metrics.joint_limit_violations;
    let limit = thresholds.hard_limits.joint_limit_violations.limit;
    if observed <= limit {
        return Err(format!(
            "CMU candidate must remain rejected for the measured hard-zero joint-limit failure; observed {observed}, limit {limit}"
        ));
    }

    let motion_report_hash = hash_file(motion_report_path)?;
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
            database_hash: hash_file(database_path)?,
            provenance_hash: hash_file(provenance_path)?,
            license_id: provenance.license_id.clone(),
            status: "accepted",
        },
    };

    let mut scenes = Vec::with_capacity(fixture.scenes.len());
    for scene in &fixture.scenes {
        scenes.push(run_scene(
            scene,
            &database,
            &fixture_hash,
            motion_report_path,
            &fixture.motion_baseline,
        )?);
    }

    let hard_safety_passed = scenes.iter().all(|scene| scene.hard_safety_passed);
    let fallback_count = scenes.iter().map(|scene| scene.fallback_count).sum();
    let unrelated_agent_mutations = scenes
        .iter()
        .map(|scene| scene.unrelated_agent_mutations)
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
        if scene.tick_start > scene.tick_end
            || scene.agent_count == 0
            || scene.target_agent_ids.is_empty()
            || scene.source_paths.is_empty()
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
) -> Result<(), String> {
    if database.schema_version != 1
        || database.database_id.is_empty()
        || database.retarget_profile_id.is_empty()
        || database.source_provenance.is_empty()
        || database.clips.is_empty()
    {
        return Err("checked authored motion database is invalid".to_owned());
    }
    if provenance.schema_version != 1
        || provenance.asset_id.is_empty()
        || provenance.source_uri.is_empty()
        || provenance.content_hash.len() != 64
        || provenance.license_id != "CC0-1.0"
        || !provenance.redistribution_allowed
        || provenance.terms_reference.is_empty()
        || provenance.checkpoint_hash.is_some()
    {
        return Err(
            "passing motion baseline must be the checked redistributable CC0 asset".to_owned(),
        );
    }
    if thresholds.schema_version != 1
        || thresholds.threshold_id.is_empty()
        || report.schema_version != 1
    {
        return Err("motion threshold/report contract version is invalid".to_owned());
    }
    Ok(())
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
    motion_report_path: &Path,
    baseline: &MotionBaselineFixture,
) -> Result<SceneReport, String> {
    let mut domain = match scene.id.as_str() {
        "scheduled_cafe" => run_scheduled_cafe(scene)?,
        "family_split_regroup" => run_family_split_regroup(scene)?,
        "terrain_motion_feedback" => run_terrain_motion_feedback(scene)?,
        "paired_handoff" => run_paired_handoff(scene)?,
        "ragdoll_recovery" => run_ragdoll_recovery(scene)?,
        "mixed_tier_diagnostics" => run_mixed_tier_diagnostics(scene)?,
        other => return Err(format!("unsupported M6 scene {other}")),
    };
    let motion = run_motion(scene, database)?;
    let isolation = measure_isolation(scene, domain.promoted_group_count)?;
    domain.specific.insert(
        "base_cache_mutations".to_owned(),
        u64::from(isolation.base_cache_mutations),
    );
    domain.specific.insert(
        "motion_match_fallbacks".to_owned(),
        u64::from(motion.used_fallback),
    );
    domain.specific.insert(
        "selected_clip_id_hash".to_owned(),
        u64::from_le_bytes(
            content_hash(motion.clip_id.as_bytes())[..8]
                .try_into()
                .unwrap(),
        ),
    );

    let required_contacts = domain.required_contacts + motion.required_contacts;
    let observed_contacts = domain.observed_contacts + motion.observed_contacts;
    let contact_precision_millionths = if required_contacts == 0 {
        1_000_000
    } else {
        observed_contacts.min(required_contacts) * 1_000_000 / required_contacts
    };
    let safety_violations = domain.safety_violations
        + motion.safety_violations
        + isolation.base_cache_mutations
        + isolation.unrelated_agent_mutations;
    let fallback_count = 1 + u32::from(motion.used_fallback);
    let hard_safety_passed = safety_violations == 0
        && isolation.base_cache_mutations == 0
        && isolation.unrelated_agent_mutations == 0;
    let metrics = SceneMetrics {
        trajectory_fit_millimeters: motion.trajectory_fit_millimeters,
        foot_slide_millimeters: motion.foot_slide_millimeters,
        required_contacts,
        observed_contacts,
        contact_precision_millionths,
        safety_violations,
        scene_specific: domain.specific,
    };

    let mut reasons = Vec::new();
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
    if isolation.unrelated_agent_mutations > scene.criteria.max_unrelated_agent_mutations {
        reasons.push(format!(
            "unrelated-agent mutations {} exceed {}",
            isolation.unrelated_agent_mutations, scene.criteria.max_unrelated_agent_mutations
        ));
    }
    let passed = reasons.is_empty() && hard_safety_passed;
    if passed {
        reasons.push(
            "all scene criteria passed; rejected CMU candidate fell back to checked CC0 motion"
                .to_owned(),
        );
    }

    let source_hashes = scene_source_hashes(scene, motion_report_path, baseline)?;
    let deterministic_replay_hash = hash_serializable(&(
        &scene.id,
        scene.seed,
        fixture_hash,
        &source_hashes,
        scene.tick_start,
        scene.tick_end,
        scene.agent_count,
        domain.promoted_group_count,
        &metrics,
        fallback_count,
        isolation.unrelated_agent_mutations,
    ))?;

    Ok(SceneReport {
        id: scene.id.clone(),
        seed: scene.seed,
        fixture_hash: fixture_hash.to_owned(),
        source_hashes,
        tick_start: scene.tick_start,
        tick_end: scene.tick_end,
        agent_count: scene.agent_count,
        promoted_group_count: domain.promoted_group_count,
        deterministic_replay_hash,
        hard_safety_passed,
        metrics,
        fallback_count,
        unrelated_agent_mutations: isolation.unrelated_agent_mutations,
        passed,
        reasons,
    })
}

fn run_motion(scene: &SceneFixture, database: &MotionDatabaseV1) -> Result<MotionEvidence, String> {
    let matcher = MotionMatcher::new(database.clone());
    let fallback_clip_id = database
        .clips
        .first()
        .ok_or_else(|| "motion database has no fallback clip".to_owned())?
        .id
        .clone();
    let selected = matcher
        .select(&MotionQueryV1 {
            desired_velocity_millimeters_per_second: scene
                .motion
                .desired_velocity_millimeters_per_second,
            desired_slope_millionths: scene.motion.desired_slope_millionths,
            required_contact: scene.motion.required_contact,
            fallback_clip_id,
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
        scene.motion.desired_velocity_millimeters_per_second,
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
    let required_contacts = u32::from(scene.motion.required_contact.is_some());
    let observed_contacts = u32::from(
        scene
            .motion
            .required_contact
            .is_some_and(|contact| clip.samples.iter().any(|sample| sample.contact == contact)),
    );
    Ok(MotionEvidence {
        trajectory_fit_millimeters: feedback.root_deviation_millionths / 1_000,
        foot_slide_millimeters: feedback.foot_slide_millionths / 1_000,
        required_contacts,
        observed_contacts,
        used_fallback: selected.used_fallback,
        safety_violations: u32::from(!feedback.feasible),
        clip_id: selected.clip_id,
    })
}

fn run_scheduled_cafe(scene: &SceneFixture) -> Result<DomainEvidence, String> {
    let schedule: ActivityScheduleV1 = read_scene_source(scene, "activity-v1.json")?;
    schedule.validate().map_err(|error| error.to_string())?;
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
    let requests = (1..=3)
        .map(|agent_id| ActivityRequestV1 {
            agent_id,
            resource_id: resource_id.clone(),
            priority: 10,
        })
        .collect::<Vec<_>>();
    let results = runtime.request_batch(&requests);
    let granted = results
        .iter()
        .filter(|result| result.status == ReservationStatusV1::Granted)
        .count() as u64;
    let waiting = results
        .iter()
        .filter(|result| matches!(result.status, ReservationStatusV1::Waiting { .. }))
        .count() as u64;
    let first_owner = runtime
        .owners(&resource_id)
        .first()
        .copied()
        .ok_or_else(|| "scheduled cafe admitted no owner".to_owned())?;
    let released = runtime.release(first_owner, &resource_id);
    let promoted_after_release =
        u64::from(runtime.status(3, &resource_id) == ReservationStatusV1::Granted);
    let owners = runtime.owners(&resource_id);
    let unique_owners = owners.iter().copied().collect::<BTreeSet<_>>().len();
    let safety_violations = u32::from(!schedule.is_active(scene.tick_start))
        + u32::from(granted != 2)
        + u32::from(waiting != 1)
        + u32::from(!released)
        + u32::from(promoted_after_release != 1)
        + u32::from(unique_owners != owners.len());
    Ok(DomainEvidence {
        promoted_group_count: u32::from(schedule.paired_action.is_some()),
        required_contacts: u32::from(schedule.paired_action.is_some()),
        observed_contacts: u32::from(schedule.paired_action.is_some() && granted >= 2),
        safety_violations,
        specific: BTreeMap::from([
            ("granted_reservations".to_owned(), granted),
            ("waiting_before_release".to_owned(), waiting),
            ("promoted_after_release".to_owned(), promoted_after_release),
            (
                "double_ownership_violations".to_owned(),
                (owners.len() - unique_owners) as u64,
            ),
        ]),
    })
}

fn run_family_split_regroup(scene: &SceneFixture) -> Result<DomainEvidence, String> {
    let formation: FormationV1 = read_scene_source(scene, "formation-v1.json")?;
    formation.validate().map_err(|error| error.to_string())?;
    let split_positions = BTreeMap::from([
        (AgentId(7), Vec2::new(0.0, 0.0)),
        (AgentId(9), Vec2::new(-1.0, 0.0)),
        (AgentId(11), Vec2::new(4.0, 0.0)),
    ]);
    let split = formation.evaluate(&split_positions, &[(AgentId(99), Vec2::new(20.0, 0.0))]);
    let regroup_positions = BTreeMap::from([
        (AgentId(7), Vec2::new(0.0, 0.0)),
        (AgentId(9), Vec2::new(-1.0, 0.0)),
        (AgentId(11), Vec2::new(1.0, 0.0)),
    ]);
    let correction = formation.cohesion_velocity(AgentId(11), &split_positions, 0.75);
    let regrouped = formation.evaluate(&regroup_positions, &[(AgentId(99), Vec2::new(20.0, 0.0))]);
    let safety_violations = u32::from(!split.split)
        + u32::from(regrouped.split)
        + u32::from(!split.intruder_agent_ids.is_empty())
        + u32::from(!regrouped.intruder_agent_ids.is_empty())
        + u32::from(correction.length() > 0.750_001);
    Ok(DomainEvidence {
        safety_violations,
        specific: BTreeMap::from([
            ("split_samples".to_owned(), u64::from(split.split)),
            ("regrouped_samples".to_owned(), u64::from(!regrouped.split)),
            ("intrusion_samples".to_owned(), 0),
            (
                "maximum_split_separation_millimeters".to_owned(),
                (split.maximum_separation_m * 1_000.0).round() as u64,
            ),
        ]),
        ..DomainEvidence::default()
    })
}

fn run_terrain_motion_feedback(scene: &SceneFixture) -> Result<DomainEvidence, String> {
    let input: TerrainMotionInput = read_scene_source(scene, "terrain-motion-v1.json")?;
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
    let midpoint = scene.tick_start + (scene.tick_end - scene.tick_start) / 2;
    let observed = foot_locks
        .iter()
        .filter(|lock| lock.contains_tick(midpoint))
        .count() as u32;
    let safety_violations = u32::from(input.schema_version != 1)
        + u32::from(input.scene_id.is_empty())
        + u32::from(input.terrain_id.is_empty())
        + u32::from(input.navigation_feedback != "warp_stride")
        + u32::from(!terrain.accepts_slope(scene.motion.desired_slope_millionths))
        + u32::from(observed != foot_locks.len() as u32);
    Ok(DomainEvidence {
        required_contacts: foot_locks.len() as u32,
        observed_contacts: observed,
        safety_violations,
        specific: BTreeMap::from([
            ("terrain_constraints_accepted".to_owned(), 1),
            ("foot_locks_satisfied".to_owned(), u64::from(observed)),
            ("navigation_feedback_events".to_owned(), 1),
        ]),
        ..DomainEvidence::default()
    })
}

fn run_paired_handoff(scene: &SceneFixture) -> Result<DomainEvidence, String> {
    let request: InteractionRequestV1 = read_scene_source(scene, "interaction-request-v1.json")?;
    let checked_motion: InteractionMotionV1 =
        read_scene_source(scene, "interaction-motion-v1.json")?;
    checked_motion
        .validate_against(&request)
        .map_err(|issues| format!("checked paired motion is invalid: {issues:?}"))?;
    let motion = deterministic_paired_clip(&request)
        .map_err(|issues| format!("deterministic paired clip failed: {issues:?}"))?;
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
    let observed = motion.contacts.len() as u32;
    let safety_violations = u32::from(promoted.as_deref() != Some(request.request_id.as_str()))
        + u32::from(!atomically_locked)
        + u32::from(completed != Some(InteractionGroupStatusV1::Completed))
        + u32::from(required != observed);
    Ok(DomainEvidence {
        promoted_group_count: u32::from(promoted.is_some()),
        required_contacts: required,
        observed_contacts: observed,
        safety_violations,
        specific: BTreeMap::from([
            (
                "participants_locked_atomically".to_owned(),
                u64::from(atomically_locked),
            ),
            (
                "required_interaction_contacts".to_owned(),
                u64::from(required),
            ),
            (
                "completed_interactions".to_owned(),
                u64::from(completed.is_some()),
            ),
        ]),
    })
}

fn run_ragdoll_recovery(scene: &SceneFixture) -> Result<DomainEvidence, String> {
    let transition: PhysicsTransitionV1 = read_scene_source(scene, "physics-transition-v1.json")?;
    validate_transition(&transition).map_err(|errors| errors.join("; "))?;
    let samples = simulate_physics_handoff_v1(&PhysicsHandoffSpecV1 {
        tick_start: transition.tick_start,
        tick_end: transition.tick_end,
        ticks_per_second: 30,
        incoming_position: [0.0, 0.0, 0.5],
        incoming_velocity: [0.5, 0.0, -2.0],
        gravity_mps2: -9.81,
        floor_z: 0.0,
        restitution_millionths: 0,
        collision_masks: vec!["crowd".to_owned(), "ground".to_owned()],
    })
    .map_err(|error| error.to_string())?;
    let phases = (transition.tick_start..=transition.tick_end)
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
    let safety_violations = u32::from(samples.len() != 11)
        + u32::from(impact == 0 || stabilize == 0 || resume == 0)
        + u32::from(floor_contacts == 0)
        + u32::from(samples.iter().any(|sample| sample.position[2] < 0.0));
    Ok(DomainEvidence {
        promoted_group_count: 1,
        required_contacts: 1,
        observed_contacts: u32::from(floor_contacts > 0),
        safety_violations,
        specific: BTreeMap::from([
            ("physics_cache_samples".to_owned(), samples.len() as u64),
            ("impact_phase_ticks".to_owned(), impact),
            ("stabilize_phase_ticks".to_owned(), stabilize),
            ("resume_phase_ticks".to_owned(), resume),
            (
                "floor_contact_samples".to_owned(),
                u64::from(floor_contacts),
            ),
        ]),
    })
}

fn run_mixed_tier_diagnostics(scene: &SceneFixture) -> Result<DomainEvidence, String> {
    let input: MixedTierInput = read_scene_source(scene, "mixed-tier-v1.json")?;
    let total_agents = input.tiers.iter().map(|tier| tier.agent_count).sum::<u64>();
    let mut safety_violations = u32::from(input.schema_version != 1)
        + u32::from(input.scene_id.is_empty())
        + u32::from(input.base_cache_hash.len() != 64)
        + u32::from(total_agents != scene.agent_count);
    let mut full_diagnostics = 0u64;
    let mut reduced_diagnostics = 0u64;
    let mut aggregate_diagnostics = 0u64;
    let mut scheduled_animation_evaluations = 0u64;
    let mut id_cursor = 1u64;
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
            let agent_id = AgentId(id_cursor);
            for tick in scene.tick_start..=scene.tick_end {
                scheduled_animation_evaluations +=
                    u64::from(FidelityPolicy::animation_due(simulation, agent_id, tick));
            }
            id_cursor += 1;
        }
    }
    safety_violations += u32::from(full_diagnostics == 0)
        + u32::from(reduced_diagnostics == 0)
        + u32::from(aggregate_diagnostics == 0);
    Ok(DomainEvidence {
        promoted_group_count: input.promoted_interaction_groups.len() as u32,
        required_contacts: input.promoted_interaction_groups.len() as u32,
        observed_contacts: input.promoted_interaction_groups.len() as u32,
        safety_violations,
        specific: BTreeMap::from([
            ("full_diagnostic_channels".to_owned(), full_diagnostics),
            (
                "reduced_diagnostic_channels".to_owned(),
                reduced_diagnostics,
            ),
            (
                "aggregate_diagnostic_channels".to_owned(),
                aggregate_diagnostics,
            ),
            (
                "scheduled_animation_evaluations".to_owned(),
                scheduled_animation_evaluations,
            ),
        ]),
    })
}

fn measure_isolation(
    scene: &SceneFixture,
    promoted_group_count: u32,
) -> Result<IsolationEvidence, String> {
    let mut agent_ids = scene.target_agent_ids.clone();
    let unrelated_id = agent_ids
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .saturating_add(10_000);
    agent_ids.push(unrelated_id);
    let base = Frame {
        records: agent_ids
            .iter()
            .enumerate()
            .map(|(index, agent_id)| FrameRecord {
                agent_id: *agent_id,
                clip_id: index as u16 + 1,
                phase: index as f32 / 10.0,
                ..FrameRecord::default()
            })
            .collect(),
    };
    let original = base.clone();
    let midpoint = scene.tick_start + (scene.tick_end - scene.tick_start) / 2;
    let layers = if promoted_group_count > 0 {
        vec![AnimationLayerV1 {
            schema_version: INTERACTION_LAYER_SCHEMA_VERSION,
            layer_id: format!("{}-isolation-layer", scene.id),
            interaction_id: format!("{}-promoted", scene.id),
            base_cache_hash: "aa".repeat(32),
            target_agent_ids: scene.target_agent_ids.clone(),
            tick_start: scene.tick_start,
            tick_end: scene.tick_end,
            priority: 10,
            enabled: true,
            provenance: "m6-acceptance-authored-reference-v1".to_owned(),
            edits: scene
                .target_agent_ids
                .iter()
                .enumerate()
                .map(|(index, agent_id)| AnimationEditV1 {
                    agent_id: *agent_id,
                    tick: midpoint,
                    clip_id: 100 + index as u16,
                    phase_millionths: 500_000,
                })
                .collect(),
            fallback: FallbackClipV1 {
                clip_set_id: "reference-humanoid-motion".to_owned(),
                clip_id: "walk-reference".to_owned(),
                reason: "checked CC0 fallback".to_owned(),
            },
        }]
    } else {
        Vec::new()
    };
    let composed = compose_interaction_frame_v1(&base, midpoint, &"aa".repeat(32), &layers)
        .map_err(|error| error.to_string())?;
    let targets = scene
        .target_agent_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let unrelated_agent_mutations = original
        .records
        .iter()
        .filter(|record| !targets.contains(&record.agent_id))
        .filter(|record| {
            composed
                .records
                .iter()
                .find(|candidate| candidate.agent_id == record.agent_id)
                != Some(*record)
        })
        .count() as u32;
    Ok(IsolationEvidence {
        base_cache_mutations: u32::from(base != original),
        unrelated_agent_mutations,
    })
}

fn scene_source_hashes(
    scene: &SceneFixture,
    motion_report_path: &Path,
    baseline: &MotionBaselineFixture,
) -> Result<BTreeMap<String, String>, String> {
    let paths = scene.source_paths.iter().map(PathBuf::from).chain([
        PathBuf::from(&baseline.database_path),
        PathBuf::from(&baseline.provenance_path),
        PathBuf::from(&baseline.thresholds_path),
        motion_report_path.to_path_buf(),
    ]);
    let mut hashes = BTreeMap::new();
    for path in paths {
        hashes.insert(path_text(&path), hash_file(&path)?);
    }
    Ok(hashes)
}

fn read_scene_source<T: DeserializeOwned>(scene: &SceneFixture, suffix: &str) -> Result<T, String> {
    let path = scene
        .source_paths
        .iter()
        .find(|path| path.ends_with(suffix))
        .ok_or_else(|| format!("scene {} is missing source {suffix}", scene.id))?;
    read_json(Path::new(path))
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

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = read_bytes(path)?;
    parse_bytes(path, &bytes)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn parse_bytes<T: DeserializeOwned>(path: &Path, bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn hash_file(path: &Path) -> Result<String, String> {
    read_bytes(path).map(|bytes| hash_bytes(&bytes))
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
