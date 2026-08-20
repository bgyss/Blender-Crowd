//! Deterministic M6 mixed-tier fixture and evidence-only performance lane.
//!
//! This is deliberately not the general crowd simulation. It composes the
//! checked M5 90/10 distribution with M6 runtime authorities in a fixed 10K
//! fixture so phase cost, evidence degradation, fallback, and safety remain
//! independently inspectable.

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::time::Instant;

use crowd_cache::content_hash;
use crowd_core::activity::{
    ActivityRequestV1, ReservationRuntimeV1, ReservationStatusV1, ResourceV1,
};
use crowd_core::arena::NeighborArena;
use crowd_core::blackboard::{
    BlackboardChannelV1, BlackboardStateV1, BlackboardTypeV1, BlackboardValueV1,
};
use crowd_core::fidelity::{FidelityPolicy, RenderTier, SimulationTier};
use crowd_core::formation::FormationV1;
use crowd_core::ids::AgentId;
use crowd_core::interaction::{
    InteractionGroupStatusV1, InteractionRequestV1, InteractionSchedulerV1,
};
use crowd_core::motion::{
    FootContactV1, MotionClipV1, MotionDatabaseV1, MotionMatcher, MotionQueryV1, MotionSampleV1,
};
use crowd_core::perception::{PerceptionConfigV1, PerceptionEngine};
use crowd_core::units::Vec2;
use crowd_core::world::{AgentSpawn, World, NO_ROUTE};
use serde::{Deserialize, Serialize};

pub const PHASE_NAMES: [&str; 6] = [
    "activity",
    "brain",
    "group",
    "interaction",
    "motion",
    "perception",
];

const FIXTURE_ID: &str = "m6-mixed-tier-10k-v1";
const AGENT_COUNT: u32 = 10_000;
const TICKS: u32 = 30;
const S0_COUNT: u32 = 10;
const S1_COUNT: u32 = 990;
const S2_COUNT: u32 = 9_000;
const PROMOTED_COUNT: u32 = S0_COUNT + S1_COUNT;
const MIN_TICKS_PER_SECOND: f64 = 10.0;
const CACHE_RECORD_BYTES: usize = 16;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixedTierFixture {
    pub schema_version: u32,
    pub fixture_id: String,
    pub agent_count: u32,
    pub ticks: u32,
    pub seed: u64,
    pub tier_counts: BTreeMap<String, u32>,
    pub debug_evidence: BTreeMap<String, String>,
    pub min_ticks_per_second: f64,
}

impl MixedTierFixture {
    pub fn checked_10k() -> Self {
        Self {
            schema_version: 1,
            fixture_id: FIXTURE_ID.to_owned(),
            agent_count: AGENT_COUNT,
            ticks: TICKS,
            seed: 2026,
            tier_counts: BTreeMap::from([
                ("S0".to_owned(), S0_COUNT),
                ("S1".to_owned(), S1_COUNT),
                ("S2".to_owned(), S2_COUNT),
            ]),
            debug_evidence: BTreeMap::from([
                ("S0".to_owned(), "full".to_owned()),
                ("S1".to_owned(), "reduced".to_owned()),
                ("S2".to_owned(), "aggregate_only".to_owned()),
            ]),
            min_ticks_per_second: MIN_TICKS_PER_SECOND,
        }
    }

    pub fn promoted_agent_count(&self) -> u32 {
        self.tier_counts.get("S0").copied().unwrap_or(0)
            + self.tier_counts.get("S1").copied().unwrap_or(0)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.fixture_id != FIXTURE_ID
            || self.agent_count != AGENT_COUNT
            || self.ticks != TICKS
            || self.tier_counts.get("S0") != Some(&S0_COUNT)
            || self.tier_counts.get("S1") != Some(&S1_COUNT)
            || self.tier_counts.get("S2") != Some(&S2_COUNT)
            || self.promoted_agent_count() != PROMOTED_COUNT
            || self.debug_evidence.get("S0").map(String::as_str) != Some("full")
            || self.debug_evidence.get("S1").map(String::as_str) != Some("reduced")
            || self.debug_evidence.get("S2").map(String::as_str) != Some("aggregate_only")
            || self.min_ticks_per_second != MIN_TICKS_PER_SECOND
        {
            return Err(
                "mixed-tier fixture must retain the checked 10/990/9000 contract".to_owned(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseTiming {
    pub phase: String,
    pub nanos: u64,
    pub operations: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FallbackAccounting {
    pub phase: String,
    pub count: u64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierEvidence {
    pub tier: String,
    pub agent_count: u32,
    pub evidence_level: String,
    pub individual_records: u32,
    pub aggregate_records: u32,
    pub unavailable_evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotionSourceEvidence {
    pub candidate_id: String,
    pub candidate_joint_limit_violations: u64,
    pub joint_limit_violation_limit: u64,
    pub candidate_decision: String,
    pub accepted_database_id: String,
    pub accepted_database_path: String,
    pub accepted_provenance_path: String,
    pub accepted_license_id: String,
    pub accepted_decision: String,
    pub database_hash: String,
    pub provenance_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixedTierReport {
    pub schema_version: u32,
    pub fixture_id: String,
    pub seed: u64,
    pub agent_count: u32,
    pub ticks: u32,
    pub tier_counts: BTreeMap<String, u32>,
    pub promoted_agent_count: u32,
    pub phase_timings: Vec<PhaseTiming>,
    pub elapsed_nanos: u64,
    pub phase_nanos: u64,
    pub overhead_nanos: u64,
    pub ticks_per_second: f64,
    pub min_ticks_per_second: f64,
    pub working_set_bytes: u64,
    pub working_set_method: String,
    pub cache_payload_bytes: u64,
    pub cache_records: u64,
    pub cache_record_bytes: u32,
    pub deterministic_replay_hash: String,
    pub final_state_hash: String,
    pub cache_payload_hash: String,
    pub motion_source: MotionSourceEvidence,
    pub tier_evidence: Vec<TierEvidence>,
    pub fallbacks: Vec<FallbackAccounting>,
    pub hard_safety_failures: u64,
    pub unrelated_agent_mutations: u64,
    pub passed: bool,
    pub failure_reasons: Vec<String>,
    pub unsupported_claims: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct AgentState {
    agent_id: u32,
    tier: u8,
    render_tier: u8,
    activity_state: u8,
    group_state: u8,
    clip_id: u16,
    phase_millionths: u32,
    interaction_state: u8,
}

#[derive(Default)]
struct TimingAccumulator {
    nanos: BTreeMap<&'static str, u64>,
    operations: BTreeMap<&'static str, u64>,
}

impl TimingAccumulator {
    fn record(&mut self, phase: &'static str, started: Instant, operations: u64) {
        *self.nanos.entry(phase).or_default() += started.elapsed().as_nanos() as u64;
        *self.operations.entry(phase).or_default() += operations;
    }

    fn report(&self) -> Vec<PhaseTiming> {
        PHASE_NAMES
            .iter()
            .map(|phase| PhaseTiming {
                phase: (*phase).to_owned(),
                nanos: self.nanos.get(phase).copied().unwrap_or(0),
                operations: self.operations.get(phase).copied().unwrap_or(0),
            })
            .collect()
    }
}

pub fn run_fixture(fixture: &MixedTierFixture) -> Result<MixedTierReport, String> {
    fixture.validate()?;
    let mut state = build_state();
    let mut promoted_world = build_promoted_world()?;
    let mut neighbors = NeighborArena::new();
    let mut perception = PerceptionEngine::new(PerceptionConfigV1::default());
    perception.set_group_members("hero-pair", vec![AgentId(7), AgentId(9)]);
    let mut blackboards = build_blackboards()?;
    let formation: FormationV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/m6/formation-v1.json"
    ))
    .map_err(|error| format!("formation fixture: {error}"))?;
    formation.validate().map_err(|error| error.to_string())?;
    let (matcher, motion_source) = build_motion_matcher()?;
    let motion_query = MotionQueryV1 {
        desired_velocity_millimeters_per_second: [1_000, 0],
        desired_slope_millionths: 0,
        required_contact: None,
        fallback_clip_id: "walk-reference".to_owned(),
        future_positions_millimeters: vec![[1_000, 0]],
        future_velocities_millimeters_per_second: vec![[1_000, 0]],
    };
    let interaction_request: InteractionRequestV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-request-v1.json"
    ))
    .map_err(|error| format!("interaction fixture: {error}"))?;
    interaction_request
        .validate()
        .map_err(|issues| format!("interaction fixture: {issues:?}"))?;

    let mut timings = TimingAccumulator::default();
    let mut hard_safety_failures = 0u64;
    let mut motion_fallbacks = 0u64;
    let mut activity_fallbacks = 0u64;
    let mut interaction_fallbacks = 0u64;
    let mut unrelated_agent_mutations = 0u64;
    let mut cache_payload = Vec::with_capacity(
        fixture.agent_count as usize * fixture.ticks as usize * CACHE_RECORD_BYTES,
    );
    let total_started = Instant::now();

    for tick in 0..fixture.ticks {
        let started = Instant::now();
        neighbors.begin(promoted_world.len());
        for slot in 0..promoted_world.len() {
            neighbors.push(slot, &[]);
        }
        let snapshots = perception.observe(&promoted_world, &neighbors, u64::from(tick));
        hard_safety_failures += u64::from(snapshots.len() != PROMOTED_COUNT as usize);
        timings.record("perception", started, PROMOTED_COUNT as u64);

        let started = Instant::now();
        let mut brain_operations = 0u64;
        for (index, blackboard) in blackboards.iter_mut().enumerate() {
            let urgency = ((tick as usize + index) % 1_000) as i32;
            if blackboard
                .set("urgency", BlackboardValueV1::NumberI32(urgency))
                .is_err()
            {
                hard_safety_failures += 1;
            }
            let _ = blackboard.drain_changes();
            state[index].activity_state = u8::from(urgency >= 500);
            brain_operations += 1;
        }
        timings.record("brain", started, brain_operations);

        let started = Instant::now();
        let requests = (0..100u64)
            .map(|agent_id| ActivityRequestV1 {
                agent_id,
                resource_id: "fixture-resource".to_owned(),
                priority: 10,
            })
            .collect::<Vec<_>>();
        let mut activity = ReservationRuntimeV1::new(vec![ResourceV1 {
            id: "fixture-resource".to_owned(),
            capacity: 100,
        }])
        .map_err(|error| error.to_string())?;
        let results = activity.request_batch(&requests);
        activity_fallbacks += results
            .iter()
            .filter(|result| matches!(result.status, ReservationStatusV1::Failed { .. }))
            .count() as u64;
        let owners = activity.owners("fixture-resource");
        hard_safety_failures += u64::from(
            owners.len() != 100 || owners.iter().copied().collect::<BTreeSet<_>>().len() != 100,
        );
        for owner in owners {
            state[owner as usize].activity_state = 2;
        }
        timings.record("activity", started, results.len() as u64);

        let started = Instant::now();
        let positions = BTreeMap::from([
            (AgentId(7), Vec2::new(0.0, 0.0)),
            (AgentId(9), Vec2::new(-1.0, 0.0)),
            (AgentId(11), Vec2::new(1.0, 0.0)),
        ]);
        let group_report = formation.evaluate(&positions, &[]);
        hard_safety_failures += u64::from(group_report.missing_members != 0 || group_report.split);
        for agent_id in [7usize, 9, 11] {
            state[agent_id].group_state = 1;
        }
        timings.record("group", started, formation.roles.len() as u64);

        let started = Instant::now();
        let mut motion_operations = 0u64;
        for agent in &mut state {
            let promoted = agent.agent_id < PROMOTED_COUNT;
            let background_due = !promoted
                && FidelityPolicy::s2_update_due(
                    AgentId(u64::from(agent.agent_id)),
                    u64::from(tick),
                );
            if promoted {
                let selected = matcher
                    .select(&motion_query)
                    .map_err(|error| error.to_string())?;
                motion_fallbacks += u64::from(selected.used_fallback);
                agent.clip_id = if selected.clip_id == "walk-reference" {
                    1
                } else {
                    2
                };
                motion_operations += 1;
            } else if background_due {
                agent.clip_id = 1;
                motion_operations += 1;
            }
            agent.phase_millionths =
                (agent.phase_millionths + 33_333 + agent.agent_id % 7) % 1_000_000;
        }
        timings.record("motion", started, motion_operations);

        let started = Instant::now();
        let before_interaction = state
            .iter()
            .map(|agent| agent.interaction_state)
            .collect::<Vec<_>>();
        let mut scheduler = InteractionSchedulerV1::new(1);
        scheduler.enqueue(interaction_request.clone())?;
        let promoted = scheduler.promote_next();
        let locked = [7u64, 9]
            .iter()
            .all(|agent_id| scheduler.active_group_for(*agent_id) == promoted.as_deref());
        let completed = scheduler.complete(&interaction_request.request_id);
        if promoted.as_deref() == Some(interaction_request.request_id.as_str())
            && locked
            && completed == Some(InteractionGroupStatusV1::Completed)
        {
            state[7].interaction_state = 1;
            state[9].interaction_state = 1;
        } else {
            hard_safety_failures += 1;
            interaction_fallbacks += 1;
        }
        unrelated_agent_mutations += state
            .iter()
            .zip(before_interaction)
            .filter(|(agent, before)| {
                ![7, 9].contains(&agent.agent_id) && agent.interaction_state != *before
            })
            .count() as u64;
        timings.record("interaction", started, 1);

        append_cache_tick(&mut cache_payload, tick, &state);
        for (slot, agent) in state.iter().enumerate().take(promoted_world.len()) {
            promoted_world.clip_id[slot] = agent.clip_id;
            promoted_world.clip_phase[slot] = agent.phase_millionths as f32 / 1_000_000.0;
        }
    }

    let elapsed_nanos = total_started.elapsed().as_nanos() as u64;
    let phase_timings = timings.report();
    let phase_nanos = phase_timings.iter().map(|timing| timing.nanos).sum::<u64>();
    let cache_records = u64::from(fixture.agent_count) * u64::from(fixture.ticks);
    if cache_payload.len() as u64 != cache_records * CACHE_RECORD_BYTES as u64 {
        hard_safety_failures += 1;
    }
    let final_state_bytes = serde_json::to_vec(&state).map_err(|error| error.to_string())?;
    let final_state_hash = hex_hash(&final_state_bytes);
    let cache_payload_hash = hex_hash(&cache_payload);
    let fallbacks = vec![
        FallbackAccounting {
            phase: "activity".to_owned(),
            count: activity_fallbacks,
            reason: "unknown or unavailable reserved resource".to_owned(),
        },
        FallbackAccounting {
            phase: "brain".to_owned(),
            count: 0,
            reason: "typed blackboard write rejected".to_owned(),
        },
        FallbackAccounting {
            phase: "group".to_owned(),
            count: 0,
            reason: "formation incomplete or outside the checked cohesion bound".to_owned(),
        },
        FallbackAccounting {
            phase: "interaction".to_owned(),
            count: interaction_fallbacks,
            reason: "atomic promotion or completion failed".to_owned(),
        },
        FallbackAccounting {
            phase: "motion".to_owned(),
            count: motion_fallbacks,
            reason: "no feasible promoted motion candidate".to_owned(),
        },
        FallbackAccounting {
            phase: "perception".to_owned(),
            count: 0,
            reason: "promoted perception snapshot unavailable".to_owned(),
        },
    ];
    let deterministic_replay_hash = hex_hash(
        &serde_json::to_vec(&(
            fixture,
            &final_state_hash,
            &cache_payload_hash,
            &motion_source,
            &fallbacks,
            hard_safety_failures,
            unrelated_agent_mutations,
        ))
        .map_err(|error| error.to_string())?,
    );
    let ticks_per_second = if elapsed_nanos == 0 {
        f64::INFINITY
    } else {
        f64::from(fixture.ticks) * 1_000_000_000.0 / elapsed_nanos as f64
    };
    let working_set_bytes = state.capacity() * size_of::<AgentState>()
        + cache_payload.capacity()
        + blackboards.capacity() * size_of::<BlackboardStateV1>()
        + promoted_world.agent_id.capacity() * size_of::<AgentId>();
    let tier_evidence = vec![
        TierEvidence {
            tier: "S0".to_owned(),
            agent_count: S0_COUNT,
            evidence_level: "full".to_owned(),
            individual_records: S0_COUNT,
            aggregate_records: 0,
            unavailable_evidence: Vec::new(),
        },
        TierEvidence {
            tier: "S1".to_owned(),
            agent_count: S1_COUNT,
            evidence_level: "reduced".to_owned(),
            individual_records: S1_COUNT,
            aggregate_records: 0,
            unavailable_evidence: vec!["full per-node trace".to_owned()],
        },
        TierEvidence {
            tier: "S2".to_owned(),
            agent_count: S2_COUNT,
            evidence_level: "aggregate_only".to_owned(),
            individual_records: 0,
            aggregate_records: 1,
            unavailable_evidence: vec![
                "individual perception snapshots".to_owned(),
                "individual brain traces".to_owned(),
                "individual interaction diagnostics".to_owned(),
            ],
        },
    ];
    let mut failure_reasons = Vec::new();
    if ticks_per_second < fixture.min_ticks_per_second {
        failure_reasons.push(format!(
            "throughput {ticks_per_second:.3} is below {:.3} ticks/s",
            fixture.min_ticks_per_second
        ));
    }
    if hard_safety_failures != 0 {
        failure_reasons.push(format!("{hard_safety_failures} hard-safety failures"));
    }
    if unrelated_agent_mutations != 0 {
        failure_reasons.push(format!(
            "{unrelated_agent_mutations} unrelated-agent interaction mutations"
        ));
    }
    if phase_timings
        .iter()
        .any(|timing| timing.nanos == 0 || timing.operations == 0)
    {
        failure_reasons.push("one or more required phases lacked measured work".to_owned());
    }
    let passed = failure_reasons.is_empty();
    Ok(MixedTierReport {
        schema_version: 1,
        fixture_id: fixture.fixture_id.clone(),
        seed: fixture.seed,
        agent_count: fixture.agent_count,
        ticks: fixture.ticks,
        tier_counts: fixture.tier_counts.clone(),
        promoted_agent_count: fixture.promoted_agent_count(),
        phase_timings,
        elapsed_nanos,
        phase_nanos,
        overhead_nanos: elapsed_nanos.saturating_sub(phase_nanos),
        ticks_per_second,
        min_ticks_per_second: fixture.min_ticks_per_second,
        working_set_bytes: working_set_bytes as u64,
        working_set_method:
            "owned Vec capacities for fixture state, cache payload, blackboards, and promoted IDs"
                .to_owned(),
        cache_payload_bytes: cache_payload.len() as u64,
        cache_records,
        cache_record_bytes: CACHE_RECORD_BYTES as u32,
        deterministic_replay_hash,
        final_state_hash,
        cache_payload_hash,
        motion_source,
        tier_evidence,
        fallbacks,
        hard_safety_failures,
        unrelated_agent_mutations,
        passed,
        failure_reasons,
        unsupported_claims: vec![
            "This fixed fixture is not a general production performance claim.".to_owned(),
            "No GPU, Blender cloth, hair, rigid-body, or neural solver throughput is measured."
                .to_owned(),
            "S2 evidence is aggregate-only; absent individual traces are not inferred.".to_owned(),
        ],
    })
}

fn build_state() -> Vec<AgentState> {
    (0..AGENT_COUNT)
        .map(|agent_id| {
            let (tier, render_tier) = if agent_id < S0_COUNT {
                (SimulationTier::S0, RenderTier::R0)
            } else if agent_id < PROMOTED_COUNT {
                (SimulationTier::S1, RenderTier::R1)
            } else {
                (SimulationTier::S2, RenderTier::R2)
            };
            AgentState {
                agent_id,
                tier: tier as u8,
                render_tier: render_tier as u8,
                activity_state: 0,
                group_state: 0,
                clip_id: 0,
                phase_millionths: 0,
                interaction_state: 0,
            }
        })
        .collect()
}

fn build_promoted_world() -> Result<World, String> {
    let mut world = World::new();
    for agent_id in 0..PROMOTED_COUNT {
        let slot = world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(u64::from(agent_id)),
                    population_id: 0,
                    position: Vec2::new((agent_id % 50) as f32 * 2.0, (agent_id / 50) as f32 * 2.0),
                    yaw: 0.0,
                    radius: 0.3,
                    max_speed: 1.8,
                    preferred_speed: 1.0,
                    route: NO_ROUTE,
                    destination: 0,
                },
                0,
            )
            .map_err(|error| format!("promoted spawn failed: {error:?}"))?;
        let index = slot as usize;
        if agent_id < S0_COUNT {
            world.simulation_tier[index] = SimulationTier::S0;
            world.render_fidelity_tier[index] = RenderTier::R0;
        } else {
            world.simulation_tier[index] = SimulationTier::S1;
            world.render_fidelity_tier[index] = RenderTier::R1;
        }
    }
    Ok(world)
}

fn build_blackboards() -> Result<Vec<BlackboardStateV1>, String> {
    (0..PROMOTED_COUNT)
        .map(|_| {
            BlackboardStateV1::new(vec![BlackboardChannelV1::new(
                "urgency",
                BlackboardTypeV1::NumberI32,
                BlackboardValueV1::NumberI32(0),
            )])
            .map_err(|error| error.to_string())
        })
        .collect()
}

#[derive(Deserialize)]
struct MotionInput {
    database_id: String,
    clips: Vec<MotionInputClip>,
}

#[derive(Deserialize)]
struct MotionInputClip {
    id: String,
    samples: Vec<MotionInputSample>,
}

#[derive(Deserialize)]
struct MotionInputSample {
    tick: u64,
    velocity_millimeters_per_second: [i32; 2],
    contact: FootContactV1,
    slope_millionths: i32,
}

fn build_motion_matcher() -> Result<(MotionMatcher, MotionSourceEvidence), String> {
    const DATABASE_PATH: &str = "assets/reference/m6/motion-database-input-v1.json";
    const PROVENANCE_PATH: &str = "assets/reference/m6/motion-provenance-v1.json";
    const DATABASE_BYTES: &[u8] =
        include_bytes!("../../../assets/reference/m6/motion-database-input-v1.json");
    const PROVENANCE_BYTES: &[u8] =
        include_bytes!("../../../assets/reference/m6/motion-provenance-v1.json");
    const CMU_BYTES: &[u8] =
        include_bytes!("../../../docs/benchmarks/2026-08-18-m6-cmu-motion.json");
    const THRESHOLD_BYTES: &[u8] =
        include_bytes!("../../../assets/reference/m6/motion-thresholds-v1.json");

    let input: MotionInput = serde_json::from_slice(DATABASE_BYTES)
        .map_err(|error| format!("accepted motion database: {error}"))?;
    let provenance: serde_json::Value = serde_json::from_slice(PROVENANCE_BYTES)
        .map_err(|error| format!("accepted motion provenance: {error}"))?;
    let cmu: serde_json::Value = serde_json::from_slice(CMU_BYTES)
        .map_err(|error| format!("CMU candidate report: {error}"))?;
    let thresholds: serde_json::Value = serde_json::from_slice(THRESHOLD_BYTES)
        .map_err(|error| format!("motion thresholds: {error}"))?;
    let database_hash = hex_hash(DATABASE_BYTES);
    let provenance_hash = hex_hash(PROVENANCE_BYTES);
    if provenance["content_hash"].as_str() != Some(database_hash.as_str())
        || provenance["license_id"].as_str() != Some("CC0-1.0")
        || provenance["redistribution_allowed"].as_bool() != Some(true)
    {
        return Err("accepted motion database does not match its pinned CC0 provenance".to_owned());
    }
    let candidate_joint_limit_violations = cmu["hard_limit_observations"]["joint_limit_violations"]
        .as_u64()
        .ok_or("CMU report lacks measured joint-limit violations")?;
    let joint_limit_violation_limit = thresholds["hard_limits"]["joint_limit_violations"]["limit"]
        .as_u64()
        .ok_or("motion thresholds lack the joint-limit hard limit")?;
    if candidate_joint_limit_violations <= joint_limit_violation_limit {
        return Err("CMU candidate was not rejected by the unchanged hard limit".to_owned());
    }
    let clips = input
        .clips
        .into_iter()
        .map(|clip| {
            MotionClipV1::new(
                clip.id,
                "checked CC0 source: assets/reference/m6/motion-provenance-v1.json",
                clip.samples
                    .into_iter()
                    .map(|sample| {
                        MotionSampleV1::at(
                            sample.tick,
                            [
                                sample.velocity_millimeters_per_second[0] * sample.tick as i32,
                                sample.velocity_millimeters_per_second[1] * sample.tick as i32,
                            ],
                            sample.velocity_millimeters_per_second,
                            sample.contact,
                            sample.slope_millionths,
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    let accepted_database_id = input.database_id;
    let database = MotionDatabaseV1::new(accepted_database_id.clone(), clips)
        .map_err(|error| error.to_string())?;
    Ok((
        MotionMatcher::new(database),
        MotionSourceEvidence {
            candidate_id: cmu["database_id"].as_str().unwrap_or("unknown").to_owned(),
            candidate_joint_limit_violations,
            joint_limit_violation_limit,
            candidate_decision: "rejected".to_owned(),
            accepted_database_id,
            accepted_database_path: DATABASE_PATH.to_owned(),
            accepted_provenance_path: PROVENANCE_PATH.to_owned(),
            accepted_license_id: "CC0-1.0".to_owned(),
            accepted_decision: "accepted".to_owned(),
            database_hash,
            provenance_hash,
        },
    ))
}

fn append_cache_tick(payload: &mut Vec<u8>, tick: u32, state: &[AgentState]) {
    for agent in state {
        payload.extend_from_slice(&agent.agent_id.to_le_bytes());
        payload.extend_from_slice(&tick.to_le_bytes());
        payload.push(agent.tier);
        payload.push(agent.render_tier);
        payload.extend_from_slice(&agent.clip_id.to_le_bytes());
        payload.extend_from_slice(&agent.phase_millionths.to_le_bytes());
    }
}

fn hex_hash(bytes: &[u8]) -> String {
    content_hash(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
