//! Deterministic M6 mixed-tier fixture and evidence-only performance lane.
//!
//! This is deliberately not the general crowd simulation. It composes the
//! checked M5 90/10 distribution with M6 runtime authorities in a fixed 10K
//! fixture so phase cost, evidence degradation, fallback, and safety remain
//! independently inspectable.

use std::collections::{BTreeMap, BTreeSet};
use std::mem::{size_of, size_of_val};
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
use crowd_core::formation::{FormationRoleV1, FormationSplitPolicyV1, FormationV1};
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
const CACHE_RECORD_BYTES: usize = 59;
const ACTIVITY_RESOURCE_CAPACITY: usize = 10;

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
    pub phase_operations: BTreeMap<String, u64>,
    pub cache_records: u64,
    pub fallbacks: u64,
    pub hard_safety_failures: u64,
    pub unrelated_agent_mutations: u64,
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
    pub tier_counts_source: String,
    pub promoted_agent_count: u32,
    pub phase_timings: Vec<PhaseTiming>,
    pub elapsed_nanos: u64,
    pub phase_nanos: u64,
    pub overhead_nanos: u64,
    pub ticks_per_second: f64,
    pub min_ticks_per_second: f64,
    pub working_set_bytes: u64,
    pub working_set_method: String,
    pub working_set_components: BTreeMap<String, u64>,
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
    perception_operations: u32,
    brain_operations: u32,
    activity_operations: u32,
    group_operations: u32,
    motion_operations: u32,
    interaction_operations: u32,
    fallbacks: u32,
    hard_safety_failures: u32,
    unrelated_agent_mutations: u32,
    cache_records: u32,
}

struct GroupFixture {
    formation: FormationV1,
    positions: BTreeMap<AgentId, Vec2>,
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
    let mut world = build_world(&state)?;
    let mut neighbors = NeighborArena::new();
    let mut perception = PerceptionEngine::new(PerceptionConfigV1::default());
    perception.set_group_members("hero-pair", vec![AgentId(7), AgentId(9)]);
    let mut blackboards = build_blackboards()?;
    let groups = build_group_fixtures()?;
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
    let interaction_requests = build_interaction_requests(&interaction_request)?;
    let activity_resources = (0..fixture.agent_count as usize / ACTIVITY_RESOURCE_CAPACITY)
        .map(|index| ResourceV1 {
            id: format!("fixture-resource-{index}"),
            capacity: ACTIVITY_RESOURCE_CAPACITY,
        })
        .collect::<Vec<_>>();
    let activity_requests = (0..u64::from(fixture.agent_count))
        .map(|agent_id| ActivityRequestV1 {
            agent_id,
            resource_id: format!(
                "fixture-resource-{}",
                agent_id as usize / ACTIVITY_RESOURCE_CAPACITY
            ),
            priority: 10,
        })
        .collect::<Vec<_>>();

    let mut timings = TimingAccumulator::default();
    let mut hard_safety_failures = 0u64;
    let mut motion_fallbacks = 0u64;
    let mut activity_fallbacks = 0u64;
    let mut brain_fallbacks = 0u64;
    let mut group_fallbacks = 0u64;
    let mut perception_fallbacks = 0u64;
    let mut interaction_fallbacks = 0u64;
    let mut unrelated_agent_mutations = 0u64;
    let mut tier_cache_records = BTreeMap::<String, u64>::new();
    let mut cache_payload = Vec::with_capacity(
        fixture.agent_count as usize * fixture.ticks as usize * CACHE_RECORD_BYTES,
    );
    let total_started = Instant::now();

    for tick in 0..fixture.ticks {
        let started = Instant::now();
        neighbors.begin(world.len());
        for slot in 0..world.len() {
            neighbors.push(slot, &[]);
        }
        let snapshots = perception.observe(&world, &neighbors, u64::from(tick));
        for agent_id in snapshots.keys() {
            let index = agent_id.0 as usize;
            if state
                .get(index)
                .is_some_and(|agent| u64::from(agent.agent_id) == agent_id.0)
            {
                state[index].perception_operations += 1;
            } else {
                hard_safety_failures += 1;
            }
        }
        for agent in &mut state {
            if agent.perception_operations != tick + 1 {
                perception_fallbacks += 1;
                hard_safety_failures += 1;
                agent.fallbacks += 1;
                agent.hard_safety_failures += 1;
            }
        }
        timings.record("perception", started, snapshots.len() as u64);

        let started = Instant::now();
        let mut brain_operations = 0u64;
        for (index, blackboard) in blackboards.iter_mut().enumerate() {
            let urgency = ((tick as usize + index) % 1_000) as i32;
            if blackboard
                .set("urgency", BlackboardValueV1::NumberI32(urgency))
                .is_err()
            {
                hard_safety_failures += 1;
                brain_fallbacks += 1;
                state[index].fallbacks += 1;
                state[index].hard_safety_failures += 1;
                continue;
            }
            let _ = blackboard.drain_changes();
            state[index].activity_state = u8::from(urgency >= 500);
            state[index].brain_operations += 1;
            brain_operations += 1;
        }
        timings.record("brain", started, brain_operations);

        let started = Instant::now();
        let mut activity = ReservationRuntimeV1::new(activity_resources.clone())
            .map_err(|error| error.to_string())?;
        let results = activity.request_batch(&activity_requests);
        for result in &results {
            let index = result.agent_id as usize;
            if matches!(result.status, ReservationStatusV1::Granted) {
                state[index].activity_state = 2;
                state[index].activity_operations += 1;
            } else {
                activity_fallbacks += 1;
                hard_safety_failures += 1;
                state[index].fallbacks += 1;
                state[index].hard_safety_failures += 1;
            }
        }
        let owners = activity_resources
            .iter()
            .flat_map(|resource| activity.owners(&resource.id))
            .collect::<Vec<_>>();
        hard_safety_failures += u64::from(
            owners.len() != fixture.agent_count as usize
                || owners.iter().copied().collect::<BTreeSet<_>>().len()
                    != fixture.agent_count as usize,
        );
        timings.record("activity", started, results.len() as u64);

        let started = Instant::now();
        let mut group_operations = 0u64;
        for group in &groups {
            let group_report = group.formation.evaluate(&group.positions, &[]);
            let failed = group_report.missing_members != 0 || group_report.split;
            for role in &group.formation.roles {
                let index = role.agent_id.0 as usize;
                state[index].group_operations += 1;
                group_operations += 1;
                if failed {
                    group_fallbacks += 1;
                    hard_safety_failures += 1;
                    state[index].fallbacks += 1;
                    state[index].hard_safety_failures += 1;
                } else {
                    state[index].group_state = 1;
                }
            }
        }
        timings.record("group", started, group_operations);

        let started = Instant::now();
        let mut motion_operations = 0u64;
        for agent in &mut state {
            let promoted = agent.agent_id < PROMOTED_COUNT;
            let background_due = !promoted
                && FidelityPolicy::s2_update_due(
                    AgentId(u64::from(agent.agent_id)),
                    u64::from(tick),
                );
            if promoted || background_due {
                let selected = matcher
                    .select(&motion_query)
                    .map_err(|error| error.to_string())?;
                motion_fallbacks += u64::from(selected.used_fallback);
                agent.fallbacks += u32::from(selected.used_fallback);
                agent.clip_id = if selected.clip_id == "walk-reference" {
                    1
                } else {
                    2
                };
                agent.motion_operations += 1;
                motion_operations += 1;
            }
            agent.phase_millionths =
                (agent.phase_millionths + 33_333 + agent.agent_id % 7) % 1_000_000;
        }
        timings.record("motion", started, motion_operations);

        let started = Instant::now();
        let mut interaction_operations = 0u64;
        let before_interaction = state
            .iter()
            .map(|agent| agent.interaction_state)
            .collect::<Vec<_>>();
        let mut expected_interaction_mutations = BTreeSet::new();
        for request in interaction_requests
            .iter()
            .enumerate()
            .filter(|(index, _)| *index % fixture.ticks as usize == tick as usize)
            .map(|(_, request)| request)
        {
            let mut scheduler = InteractionSchedulerV1::new(1);
            scheduler.enqueue(request.clone())?;
            let promoted = scheduler.promote_next();
            let locked = request.participants.iter().all(|participant| {
                scheduler.active_group_for(participant.agent_id) == promoted.as_deref()
            });
            let completed = scheduler.complete(&request.request_id);
            let released = request
                .participants
                .iter()
                .all(|participant| scheduler.active_group_for(participant.agent_id).is_none());
            if promoted.as_deref() == Some(request.request_id.as_str())
                && locked
                && completed == Some(InteractionGroupStatusV1::Completed)
                && released
            {
                for participant in &request.participants {
                    let agent = &mut state[participant.agent_id as usize];
                    agent.interaction_state = 1;
                    agent.interaction_operations += 1;
                    expected_interaction_mutations.insert(participant.agent_id);
                    interaction_operations += 1;
                }
            } else {
                interaction_fallbacks += 1;
                hard_safety_failures += 1;
                for participant in &request.participants {
                    let agent = &mut state[participant.agent_id as usize];
                    agent.fallbacks += 1;
                    agent.hard_safety_failures += 1;
                }
            }
        }
        for (index, (before, agent)) in before_interaction.iter().zip(&mut state).enumerate() {
            if *before != agent.interaction_state
                && !expected_interaction_mutations.contains(&u64::from(agent.agent_id))
            {
                debug_assert_eq!(index, agent.agent_id as usize);
                unrelated_agent_mutations += 1;
                agent.unrelated_agent_mutations += 1;
            }
        }
        timings.record("interaction", started, interaction_operations);

        append_cache_tick(
            &mut cache_payload,
            tick,
            &mut state,
            &mut tier_cache_records,
        );
        for (slot, agent) in state.iter().enumerate() {
            world.clip_id[slot] = agent.clip_id;
            world.clip_phase[slot] = agent.phase_millionths as f32 / 1_000_000.0;
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
            count: brain_fallbacks,
            reason: "typed blackboard write rejected".to_owned(),
        },
        FallbackAccounting {
            phase: "group".to_owned(),
            count: group_fallbacks,
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
            count: perception_fallbacks,
            reason: "authoritative perception snapshot unavailable".to_owned(),
        },
    ];
    let tier_counts = derive_tier_counts(&state);
    if tier_counts != fixture.tier_counts {
        hard_safety_failures += 1;
    }
    let tier_evidence = derive_tier_evidence(&state, fixture.ticks, &tier_cache_records);
    for evidence in &tier_evidence {
        let expected_agents = fixture
            .tier_counts
            .get(&evidence.tier)
            .copied()
            .unwrap_or(0);
        if evidence.agent_count != expected_agents
            || evidence.cache_records != u64::from(expected_agents) * u64::from(fixture.ticks)
            || PHASE_NAMES
                .iter()
                .any(|phase| evidence.phase_operations.get(*phase).copied().unwrap_or(0) == 0)
        {
            hard_safety_failures += 1;
        }
    }
    for timing in &phase_timings {
        let tier_operations = tier_evidence
            .iter()
            .map(|evidence| {
                evidence
                    .phase_operations
                    .get(&timing.phase)
                    .copied()
                    .unwrap_or(0)
            })
            .sum::<u64>();
        if timing.operations != tier_operations {
            hard_safety_failures += 1;
        }
    }
    let deterministic_replay_hash = hex_hash(
        &serde_json::to_vec(&(
            fixture,
            &tier_counts,
            &tier_evidence,
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
    let working_set_components = BTreeMap::from([
        (
            "activity_inputs".to_owned(),
            activity_input_bytes(&activity_resources, &activity_requests) as u64,
        ),
        (
            "agent_state".to_owned(),
            (state.capacity() * size_of::<AgentState>()) as u64,
        ),
        (
            "blackboards".to_owned(),
            (blackboards.capacity() * size_of::<BlackboardStateV1>()) as u64,
        ),
        ("cache_payload".to_owned(), cache_payload.capacity() as u64),
        (
            "group_runtime".to_owned(),
            group_runtime_bytes(&groups) as u64,
        ),
        (
            "interaction_requests".to_owned(),
            interaction_request_bytes(&interaction_requests) as u64,
        ),
        ("world".to_owned(), world_capacity_bytes(&world) as u64),
    ]);
    let working_set_bytes = working_set_components.values().sum::<u64>();
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
        tier_counts: tier_counts.clone(),
        tier_counts_source: "runtime_state".to_owned(),
        promoted_agent_count: tier_counts.get("S0").copied().unwrap_or(0)
            + tier_counts.get("S1").copied().unwrap_or(0),
        phase_timings,
        elapsed_nanos,
        phase_nanos,
        overhead_nanos: elapsed_nanos.saturating_sub(phase_nanos),
        ticks_per_second,
        min_ticks_per_second: fixture.min_ticks_per_second,
        working_set_bytes,
        working_set_method:
            "owned-capacity lower bound derived from the full 10K runtime state, world, blackboards, group/interaction inputs, and deterministic per-agent fixture evidence payload"
                .to_owned(),
        working_set_components,
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
                perception_operations: 0,
                brain_operations: 0,
                activity_operations: 0,
                group_operations: 0,
                motion_operations: 0,
                interaction_operations: 0,
                fallbacks: 0,
                hard_safety_failures: 0,
                unrelated_agent_mutations: 0,
                cache_records: 0,
            }
        })
        .collect()
}

fn build_world(state: &[AgentState]) -> Result<World, String> {
    let mut world = World::new();
    for agent in state {
        let agent_id = agent.agent_id;
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
            .map_err(|error| format!("mixed-tier spawn failed: {error:?}"))?;
        let index = slot as usize;
        world.simulation_tier[index] = match agent.tier {
            value if value == SimulationTier::S0 as u8 => SimulationTier::S0,
            value if value == SimulationTier::S1 as u8 => SimulationTier::S1,
            _ => SimulationTier::S2,
        };
        world.render_fidelity_tier[index] = match agent.render_tier {
            value if value == RenderTier::R0 as u8 => RenderTier::R0,
            value if value == RenderTier::R1 as u8 => RenderTier::R1,
            _ => RenderTier::R2,
        };
        world.render_tier[index] = agent.render_tier;
    }
    Ok(world)
}

fn build_blackboards() -> Result<Vec<BlackboardStateV1>, String> {
    (0..AGENT_COUNT)
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

fn build_group_fixtures() -> Result<Vec<GroupFixture>, String> {
    (0..AGENT_COUNT)
        .step_by(3)
        .enumerate()
        .map(|(group_index, first)| {
            let ids = (first..(first + 3).min(AGENT_COUNT))
                .map(|agent_id| AgentId(u64::from(agent_id)))
                .collect::<Vec<_>>();
            let offsets = [[0, 0], [-1_000, 0], [1_000, 0]];
            let roles = ids
                .iter()
                .enumerate()
                .map(|(index, agent_id)| FormationRoleV1 {
                    agent_id: *agent_id,
                    role: match index {
                        0 => "leader",
                        1 => "left",
                        _ => "right",
                    }
                    .to_owned(),
                    offset_millimeters: offsets[index],
                })
                .collect::<Vec<_>>();
            let positions = ids
                .iter()
                .enumerate()
                .map(|(index, agent_id)| {
                    (
                        *agent_id,
                        Vec2::new(
                            offsets[index][0] as f32 / 1_000.0,
                            offsets[index][1] as f32 / 1_000.0,
                        ),
                    )
                })
                .collect();
            let formation = FormationV1::new(
                format!("mixed-tier-group-{group_index}"),
                ids[0],
                roles,
                3_000,
                FormationSplitPolicyV1::Regroup,
            )
            .map_err(|error| error.to_string())?;
            Ok(GroupFixture {
                formation,
                positions,
            })
        })
        .collect()
}

fn build_interaction_requests(
    template: &InteractionRequestV1,
) -> Result<Vec<InteractionRequestV1>, String> {
    (0..AGENT_COUNT)
        .step_by(2)
        .enumerate()
        .map(|(group_index, first)| {
            let participant_ids = [u64::from(first), u64::from(first + 1)];
            let mut request = template.clone();
            request.request_id = format!("mixed-tier-pair-{group_index}");
            request.group_id = format!("mixed-tier-group-{group_index}");
            for (participant, agent_id) in request.participants.iter_mut().zip(participant_ids) {
                participant.agent_id = agent_id;
            }
            for (constraint, agent_id) in request.root_constraints.iter_mut().zip(participant_ids) {
                constraint.agent_id = agent_id;
            }
            for (contact_index, constraint) in request.contact_constraints.iter_mut().enumerate() {
                constraint.contact_id = format!("mixed-tier-contact-{group_index}-{contact_index}");
                constraint.owner_agent_id = participant_ids[0];
                constraint.other_agent_id = participant_ids[1];
            }
            request
                .validate()
                .map_err(|issues| {
                    issues
                        .into_iter()
                        .map(|issue| issue.message)
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .map(|_| request)
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

fn derive_tier_counts(state: &[AgentState]) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for agent in state {
        *counts.entry(tier_name(agent.tier).to_owned()).or_default() += 1;
    }
    counts
}

fn derive_tier_evidence(
    state: &[AgentState],
    ticks: u32,
    tier_cache_records: &BTreeMap<String, u64>,
) -> Vec<TierEvidence> {
    [
        ("S0", "full", Vec::new()),
        ("S1", "reduced", vec!["full per-node trace".to_owned()]),
        (
            "S2",
            "aggregate_only",
            vec![
                "individual perception snapshots".to_owned(),
                "individual brain traces".to_owned(),
                "individual interaction diagnostics".to_owned(),
            ],
        ),
    ]
    .into_iter()
    .map(|(tier, evidence_level, unavailable_evidence)| {
        let agents = state
            .iter()
            .filter(|agent| tier_name(agent.tier) == tier)
            .collect::<Vec<_>>();
        let phase_operations = BTreeMap::from([
            (
                "activity".to_owned(),
                agents
                    .iter()
                    .map(|agent| u64::from(agent.activity_operations))
                    .sum(),
            ),
            (
                "brain".to_owned(),
                agents
                    .iter()
                    .map(|agent| u64::from(agent.brain_operations))
                    .sum(),
            ),
            (
                "group".to_owned(),
                agents
                    .iter()
                    .map(|agent| u64::from(agent.group_operations))
                    .sum(),
            ),
            (
                "interaction".to_owned(),
                agents
                    .iter()
                    .map(|agent| u64::from(agent.interaction_operations))
                    .sum(),
            ),
            (
                "motion".to_owned(),
                agents
                    .iter()
                    .map(|agent| u64::from(agent.motion_operations))
                    .sum(),
            ),
            (
                "perception".to_owned(),
                agents
                    .iter()
                    .map(|agent| u64::from(agent.perception_operations))
                    .sum(),
            ),
        ]);
        let agent_count = agents.len() as u32;
        TierEvidence {
            tier: tier.to_owned(),
            agent_count,
            evidence_level: evidence_level.to_owned(),
            individual_records: if tier == "S2" { 0 } else { agent_count },
            aggregate_records: if tier == "S2" { ticks } else { 0 },
            phase_operations,
            cache_records: tier_cache_records.get(tier).copied().unwrap_or(0),
            fallbacks: agents.iter().map(|agent| u64::from(agent.fallbacks)).sum(),
            hard_safety_failures: agents
                .iter()
                .map(|agent| u64::from(agent.hard_safety_failures))
                .sum(),
            unrelated_agent_mutations: agents
                .iter()
                .map(|agent| u64::from(agent.unrelated_agent_mutations))
                .sum(),
            unavailable_evidence,
        }
    })
    .collect()
}

fn tier_name(tier: u8) -> &'static str {
    match tier {
        value if value == SimulationTier::S0 as u8 => "S0",
        value if value == SimulationTier::S1 as u8 => "S1",
        _ => "S2",
    }
}

fn append_cache_tick(
    payload: &mut Vec<u8>,
    tick: u32,
    state: &mut [AgentState],
    tier_cache_records: &mut BTreeMap<String, u64>,
) {
    for agent in state {
        agent.cache_records += 1;
        *tier_cache_records
            .entry(tier_name(agent.tier).to_owned())
            .or_default() += 1;
        let record_start = payload.len();
        payload.extend_from_slice(&agent.agent_id.to_le_bytes());
        payload.extend_from_slice(&tick.to_le_bytes());
        payload.push(agent.tier);
        payload.push(agent.render_tier);
        payload.push(agent.activity_state);
        payload.push(agent.group_state);
        payload.push(agent.interaction_state);
        payload.extend_from_slice(&agent.clip_id.to_le_bytes());
        payload.extend_from_slice(&agent.phase_millionths.to_le_bytes());
        for value in [
            agent.perception_operations,
            agent.brain_operations,
            agent.activity_operations,
            agent.group_operations,
            agent.motion_operations,
            agent.interaction_operations,
            agent.fallbacks,
            agent.hard_safety_failures,
            agent.unrelated_agent_mutations,
            agent.cache_records,
        ] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        debug_assert_eq!(payload.len() - record_start, CACHE_RECORD_BYTES);
    }
}

fn group_runtime_bytes(groups: &[GroupFixture]) -> usize {
    size_of_val(groups)
        + groups
            .iter()
            .map(|group| {
                group.formation.id.capacity()
                    + group.formation.roles.capacity() * size_of::<FormationRoleV1>()
                    + group
                        .formation
                        .roles
                        .iter()
                        .map(|role| role.role.capacity())
                        .sum::<usize>()
                    + group.positions.len() * size_of::<(AgentId, Vec2)>()
            })
            .sum::<usize>()
}

fn interaction_request_bytes(requests: &[InteractionRequestV1]) -> usize {
    size_of_val(requests)
        + requests
            .iter()
            .map(|request| serde_json::to_vec(request).map_or(0, |bytes| bytes.len()))
            .sum::<usize>()
}

fn activity_input_bytes(resources: &[ResourceV1], requests: &[ActivityRequestV1]) -> usize {
    size_of_val(resources)
        + resources
            .iter()
            .map(|resource| resource.id.capacity())
            .sum::<usize>()
        + size_of_val(requests)
        + requests
            .iter()
            .map(|request| request.resource_id.capacity())
            .sum::<usize>()
}

fn vec_capacity_bytes<T>(values: &Vec<T>) -> usize {
    values.capacity() * size_of::<T>()
}

fn world_capacity_bytes(world: &World) -> usize {
    vec_capacity_bytes(&world.agent_id)
        + vec_capacity_bytes(&world.population_id)
        + vec_capacity_bytes(&world.spawn_tick)
        + vec_capacity_bytes(&world.archetype_id)
        + vec_capacity_bytes(&world.variant_id)
        + vec_capacity_bytes(&world.spawn_ordinal)
        + vec_capacity_bytes(&world.scale)
        + vec_capacity_bytes(&world.pos_x)
        + vec_capacity_bytes(&world.pos_y)
        + vec_capacity_bytes(&world.yaw)
        + vec_capacity_bytes(&world.vel_x)
        + vec_capacity_bytes(&world.vel_y)
        + vec_capacity_bytes(&world.radius)
        + vec_capacity_bytes(&world.max_speed)
        + vec_capacity_bytes(&world.preferred_speed)
        + vec_capacity_bytes(&world.route)
        + vec_capacity_bytes(&world.route_index)
        + vec_capacity_bytes(&world.destination)
        + vec_capacity_bytes(&world.custom_destination)
        + vec_capacity_bytes(&world.destination_x)
        + vec_capacity_bytes(&world.destination_y)
        + vec_capacity_bytes(&world.custom_destination_bounds)
        + vec_capacity_bytes(&world.destination_min_x)
        + vec_capacity_bytes(&world.destination_min_y)
        + vec_capacity_bytes(&world.destination_max_x)
        + vec_capacity_bytes(&world.destination_max_y)
        + vec_capacity_bytes(&world.arrived)
        + vec_capacity_bytes(&world.unrouted)
        + vec_capacity_bytes(&world.commuter_state)
        + vec_capacity_bytes(&world.decision_reason)
        + vec_capacity_bytes(&world.clip_id)
        + vec_capacity_bytes(&world.clip_phase)
        + vec_capacity_bytes(&world.playback_rate)
        + vec_capacity_bytes(&world.visible)
        + vec_capacity_bytes(&world.simulation_tier)
        + vec_capacity_bytes(&world.render_fidelity_tier)
        + vec_capacity_bytes(&world.render_tier)
        + vec_capacity_bytes(&world.des_vel_x)
        + vec_capacity_bytes(&world.des_vel_y)
        + vec_capacity_bytes(&world.scheduled_target_vel_x)
        + vec_capacity_bytes(&world.scheduled_target_vel_y)
        + vec_capacity_bytes(&world.next_pos_x)
        + vec_capacity_bytes(&world.next_pos_y)
        + vec_capacity_bytes(&world.next_vel_x)
        + vec_capacity_bytes(&world.next_vel_y)
        + vec_capacity_bytes(&world.next_yaw)
        + vec_capacity_bytes(&world.solver_status)
        + vec_capacity_bytes(&world.stall_ticks)
}

fn hex_hash(bytes: &[u8]) -> String {
    content_hash(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
