//! M5 10K gate items 3 and 4: what a tier transition is allowed to change.
//!
//! Item 3 — a transition must not change stable IDs, pop the presentation
//! beyond the accepted tolerance, lose interaction state, or invalidate the
//! layer/route composition an agent is running under.
//!
//! Item 4 — camera/focus animation scheduling changes evaluation cost, not
//! cached root trajectories or required contacts.
//!
//! The fixture drives a camera across the scene so agents are promoted and
//! demoted repeatedly during one run, rather than asserting properties of a
//! population whose tiers never actually moved.

use std::collections::{BTreeMap, BTreeSet};

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::fidelity::{RenderTier, SimulationTier};
use crowd_core::ids::AgentId;
use crowd_core::phases::steer::SteerConfig;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::units::Vec2;
use crowd_core::{FidelityPolicy, SolverStatus, World};

const SCENE: &str = "m5_city_flow";
const AGENTS: u32 = 240;
const TICKS: u64 = 900;

fn build(fidelity: Option<FidelityPolicy>) -> Simulation {
    let scene = scenes::build(SCENE, AGENTS, 2026)
        .expect("m5_city_flow is the declared M5 scale fixture")
        .compile()
        .expect("the fixture must compile");
    Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig {
            fidelity,
            ..SimConfig::default()
        },
    )
}

/// Sweep the camera along the scene so every agent crosses several tier bands.
/// A fixed camera would leave tiers static and make every assertion below
/// vacuous.
fn sweeping_camera(tick: u64) -> FidelityPolicy {
    let phase = (tick % 300) as f32 / 300.0;
    FidelityPolicy {
        camera: Vec2::new(-60.0 + 120.0 * phase, 0.0),
        ..FidelityPolicy::default()
    }
}

#[derive(Default)]
struct Observed {
    /// Tiers each stable ID has held, so the run can prove it saw transitions.
    tiers_by_id: BTreeMap<AgentId, BTreeSet<SimulationTier>>,
    /// Stable ID seen in each slot, to catch identity being reassigned.
    id_by_slot: BTreeMap<usize, AgentId>,
    /// Route handle each ID is running, to catch a transition dropping the
    /// layer composition the agent was routed under.
    route_by_id: BTreeMap<AgentId, u32>,
    transitions: u64,
    /// Largest single-tick displacement observed on a transition tick, which
    /// is where a presentation pop would show up as a teleport.
    max_transition_step_m: f32,
}

fn observe(
    observed: &mut Observed,
    world: &World,
    previous: &BTreeMap<AgentId, (Vec2, SimulationTier)>,
) {
    for slot in 0..world.len() {
        let id = world.agent_id[slot];
        let tier = world.simulation_tier[slot];

        let seen = observed.id_by_slot.entry(slot).or_insert(id);
        assert_eq!(
            *seen, id,
            "slot {slot} changed stable identity from {seen:?} to {id:?}"
        );

        let route = observed
            .route_by_id
            .entry(id)
            .or_insert(world.route[slot].0);
        assert_eq!(
            *route, world.route[slot].0,
            "{id:?} lost the route it was composed under"
        );

        observed.tiers_by_id.entry(id).or_default().insert(tier);

        if let Some((previous_position, previous_tier)) = previous.get(&id) {
            if *previous_tier != tier {
                observed.transitions += 1;
                let step = world
                    .position(slot as u32)
                    .distance_squared(*previous_position)
                    .sqrt();
                observed.max_transition_step_m = observed.max_transition_step_m.max(step);
            }
        }
    }
}

fn snapshot(world: &World) -> BTreeMap<AgentId, (Vec2, SimulationTier)> {
    (0..world.len())
        .map(|slot| {
            (
                world.agent_id[slot],
                (world.position(slot as u32), world.simulation_tier[slot]),
            )
        })
        .collect()
}

fn run_with_sweeping_camera() -> Observed {
    let mut sim = build(Some(sweeping_camera(0)));
    let mut observed = Observed::default();
    let mut previous = BTreeMap::new();
    for tick in 0..TICKS {
        sim.set_fidelity_policy(sweeping_camera(tick));
        sim.step();
        observe(&mut observed, sim.world(), &previous);
        previous = snapshot(sim.world());
    }
    observed
}

#[test]
fn the_fixture_actually_exercises_promotions_and_demotions() {
    // Guards every other test in this file: if the camera sweep stopped
    // producing transitions, the assertions below would pass on a population
    // that never changed tier and would prove nothing.
    let observed = run_with_sweeping_camera();
    assert!(
        observed.transitions > 100,
        "only {} transitions; the fixture is not exercising the scheduler",
        observed.transitions
    );
    let multi_tier = observed
        .tiers_by_id
        .values()
        .filter(|tiers| tiers.len() > 1)
        .count();
    assert!(
        multi_tier > 50,
        "only {multi_tier} agents ever changed tier"
    );
}

#[test]
fn transitions_do_not_change_stable_ids_or_reassign_slots() {
    // The assertions live in `observe`, which runs on every tick: a violation
    // fails at the tick it happens rather than being averaged away.
    let observed = run_with_sweeping_camera();
    assert_eq!(
        observed.id_by_slot.len(),
        observed.tiers_by_id.len(),
        "slot count and distinct stable IDs must agree"
    );
}

#[test]
fn a_transition_never_teleports_an_agent() {
    // A promotion changes how often an agent is solved, never where it is.
    // At 30 Hz an agent covers well under a metre per tick, so a step beyond
    // that on a transition tick is the presentation pop the gate forbids.
    let observed = run_with_sweeping_camera();
    assert!(
        observed.max_transition_step_m < 0.5,
        "a tier transition moved an agent {:.3} m in one tick",
        observed.max_transition_step_m
    );
}

#[test]
fn interaction_state_survives_a_transition() {
    // Stall accounting is the interaction state most easily lost: a scheduler
    // that reset solver status on a tier change would silently restart every
    // braking episode, and the run would report many short stalls instead of
    // one long one.
    //
    // The property has to be stated carefully. `stall_ticks` legitimately
    // returns to zero the moment an agent stops braking, so a drop is only a
    // fault when the agent is *still* braking after the transition — that is
    // accumulated state being discarded rather than resolved.
    let mut sim = build(Some(sweeping_camera(0)));
    let mut previous: BTreeMap<AgentId, u16> = BTreeMap::new();
    let mut previous_tier: BTreeMap<AgentId, SimulationTier> = BTreeMap::new();
    let mut checked = 0u64;

    for tick in 0..TICKS {
        sim.set_fidelity_policy(sweeping_camera(tick));
        sim.step();
        let world = sim.world();
        for slot in 0..world.len() {
            let id = world.agent_id[slot];
            let tier = world.simulation_tier[slot];
            let stall = world.stall_ticks[slot];
            let braking = world.solver_status[slot] == SolverStatus::Braking;
            if let (Some(before), Some(before_tier)) = (previous.get(&id), previous_tier.get(&id)) {
                if *before_tier != tier && *before > 0 && braking {
                    assert!(
                        stall > *before,
                        "{id:?} lost stall state across a {before_tier:?} -> {tier:?} \
                         transition while still braking: {before} then {stall}"
                    );
                    checked += 1;
                }
            }
            previous.insert(id, stall);
            previous_tier.insert(id, tier);
        }
    }
    assert!(
        checked > 0,
        "no agent was mid-stall across a transition; the test proved nothing"
    );
}

#[test]
fn render_tier_always_follows_the_committed_simulation_tier() {
    // A demoted agent that kept an R0 draw would cost full-character work
    // while being simulated as background — the scale claim's failure mode.
    let mut sim = build(Some(sweeping_camera(0)));
    for tick in 0..TICKS {
        sim.set_fidelity_policy(sweeping_camera(tick));
        sim.step();
        let world = sim.world();
        for slot in 0..world.len() {
            let expected = match world.simulation_tier[slot] {
                SimulationTier::S0 => RenderTier::R0,
                SimulationTier::S1 => RenderTier::R1,
                SimulationTier::S2 => RenderTier::R2,
                SimulationTier::S3 => RenderTier::R3,
            };
            assert_eq!(
                world.render_fidelity_tier[slot], expected,
                "slot {slot} render tier drifted from its simulation tier at tick {tick}"
            );
            assert_eq!(world.render_tier[slot], expected as u8);
        }
    }
}

/// M5 gate item 4. Animation scheduling is a presentation policy, so turning
/// it on must not move a single agent.
#[test]
fn animation_scheduling_changes_evaluation_cost_not_root_motion() {
    let policy = FidelityPolicy::m5_10k_profile();

    let mut scheduled = build(Some(policy));
    scheduled.run(TICKS);

    // The same declared tier mix, with every agent re-classified every tick.
    let mut every_tick = build(Some(policy));
    every_tick.set_animation_scheduling(false);
    every_tick.run(TICKS);

    let a = scheduled.world();
    let b = every_tick.world();
    assert_eq!(
        a.len(),
        b.len(),
        "the two runs spawned different populations"
    );
    for slot in 0..a.len() {
        assert_eq!(a.agent_id[slot], b.agent_id[slot]);
        // Bitwise, not approximate: presentation scheduling has no path into
        // integration, so any difference at all is a contract violation rather
        // than a tolerance question.
        assert_eq!(
            a.pos_x[slot].to_bits(),
            b.pos_x[slot].to_bits(),
            "slot {slot} root motion diverged in x"
        );
        assert_eq!(
            a.pos_y[slot].to_bits(),
            b.pos_y[slot].to_bits(),
            "slot {slot} root motion diverged in y"
        );
        assert_eq!(a.yaw[slot].to_bits(), b.yaw[slot].to_bits());
        assert_eq!(a.vel_x[slot].to_bits(), b.vel_x[slot].to_bits());
        assert_eq!(a.vel_y[slot].to_bits(), b.vel_y[slot].to_bits());
    }

    let scheduled_metrics =
        scheduled
            .metrics()
            .summarize(scheduled.world(), scheduled.scene(), 1.0, 0);
    let s2 = scheduled_metrics
        .per_tier
        .iter()
        .find(|tier| tier.tier == "S2")
        .expect("the declared profile assigns a background tier");
    assert!(
        s2.animation_evaluation_share < 0.75,
        "background animation was not actually scheduled down: share {}",
        s2.animation_evaluation_share
    );

    let unscheduled_metrics =
        every_tick
            .metrics()
            .summarize(every_tick.world(), every_tick.scene(), 1.0, 0);
    let s2_every_tick = unscheduled_metrics
        .per_tier
        .iter()
        .find(|tier| tier.tier == "S2")
        .expect("the declared profile assigns a background tier");
    assert_eq!(
        s2_every_tick.animation_evaluation_share, 1.0,
        "the comparison run must classify every agent every tick"
    );
}

/// The background tier must actually use the coarse solver, and the choice
/// must be a pure function of the committed tier.
///
/// Asserted through observable behaviour rather than a call counter: a coarse
/// sampler resolves the same cost model at lower angular resolution, so it
/// reaches measurably different velocities while leaving foreground agents
/// untouched.
#[test]
fn the_background_solver_applies_to_background_tiers_only() {
    let policy = FidelityPolicy::m5_10k_profile();

    let mut full = build(Some(policy));
    full.run(300);

    let mut coarse = build(Some(policy));
    coarse.set_background_solver(Box::new(SampledVelocitySolver::background()));
    coarse.run(300);

    let a = full.world();
    let b = coarse.world();
    assert_eq!(a.len(), b.len());

    let mut background_changed = 0;
    let mut foreground_changed = 0;
    for slot in 0..a.len() {
        assert_eq!(a.agent_id[slot], b.agent_id[slot], "identity must not move");
        let moved = a.pos_x[slot].to_bits() != b.pos_x[slot].to_bits()
            || a.pos_y[slot].to_bits() != b.pos_y[slot].to_bits();
        match a.simulation_tier[slot] {
            SimulationTier::S2 | SimulationTier::S3 => background_changed += i32::from(moved),
            SimulationTier::S0 | SimulationTier::S1 => foreground_changed += i32::from(moved),
        }
    }
    assert!(
        background_changed > 0,
        "the coarse solver made no difference; it is not reaching the background tier"
    );
    // Foreground agents share the scene with background ones, so a few will
    // drift once their neighbours steer differently. What must not happen is
    // wholesale divergence, which would mean the coarse solver was applied to
    // them too.
    assert!(
        foreground_changed * 4 < background_changed,
        "foreground diverged nearly as much as background ({foreground_changed} vs \
         {background_changed}); the coarse solver is not tier-scoped"
    );
}

/// Repeating the run must reproduce it exactly: the coarse path must not
/// introduce any order- or timing-dependent behaviour.
#[test]
fn the_background_solver_is_deterministic() {
    let policy = FidelityPolicy::m5_10k_profile();
    let mut first = build(Some(policy));
    first.set_background_solver(Box::new(SampledVelocitySolver::background()));
    first.run(300);

    let mut second = build(Some(policy));
    second.set_background_solver(Box::new(SampledVelocitySolver::background()));
    second.run(300);

    assert_eq!(first.state_hash(), second.state_hash());
}

/// The parallel and sequential steering paths must agree bitwise.
///
/// Run the same scene at the same population both ways, selected only by
/// `parallel_min_agents`. Anything other than an exact match means the
/// threaded path is not a pure scheduling change — which would put every
/// determinism guarantee in the contract at risk, not just this scene's.
#[test]
fn parallel_and_sequential_steering_agree_bitwise() {
    const AGENTS: u32 = 4_000;

    let run = |parallel_min_agents: usize| {
        let scene = scenes::build(SCENE, AGENTS, 2026)
            .expect("fixture")
            .compile()
            .expect("fixture compiles");
        let mut sim = Simulation::new(
            scene,
            Box::new(SampledVelocitySolver::default()),
            SimConfig {
                fidelity: Some(FidelityPolicy::m5_10k_profile()),
                steer: SteerConfig {
                    parallel_min_agents,
                    ..SteerConfig::default()
                },
                ..SimConfig::default()
            },
        );
        sim.run(200);
        let world = sim.world();
        let positions: Vec<(u32, u32)> = (0..world.len())
            .map(|slot| (world.pos_x[slot].to_bits(), world.pos_y[slot].to_bits()))
            .collect();
        (sim.state_hash(), positions)
    };

    // 0 forces every agent through the threaded path; usize::MAX forces the
    // single-threaded one.
    let (parallel_hash, parallel_positions) = run(0);
    let (sequential_hash, sequential_positions) = run(usize::MAX);

    assert_eq!(
        parallel_hash, sequential_hash,
        "threaded steering changed the simulation state"
    );
    assert_eq!(
        parallel_positions, sequential_positions,
        "threaded steering moved at least one agent to a different position"
    );
}

/// Repeating a threaded run must reproduce it exactly, whatever the operating
/// system does with the workers.
#[test]
fn parallel_steering_is_reproducible_across_runs() {
    const AGENTS: u32 = 4_000;

    let hashes: Vec<u64> = (0..3)
        .map(|_| {
            let scene = scenes::build(SCENE, AGENTS, 2026)
                .expect("fixture")
                .compile()
                .expect("fixture compiles");
            let mut sim = Simulation::new(
                scene,
                Box::new(SampledVelocitySolver::default()),
                SimConfig {
                    fidelity: Some(FidelityPolicy::m5_10k_profile()),
                    steer: SteerConfig {
                        parallel_min_agents: 0,
                        ..SteerConfig::default()
                    },
                    ..SimConfig::default()
                },
            );
            sim.run(200);
            sim.state_hash()
        })
        .collect();

    assert_eq!(
        hashes[0], hashes[1],
        "threaded steering is not reproducible"
    );
    assert_eq!(hashes[1], hashes[2]);
}
