//! The determinism contract, contract section 9.4 `Strict` mode.
//!
//! The claim is bitwise-identical output for the same binary on the same
//! machine. Cross-machine identity is not claimed.

use std::collections::BTreeMap;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::ids::AgentId;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::World;

fn simulate(scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
    let scene = scenes::build(scene_name, agents, seed)
        .expect("known scene")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    sim.run(ticks);
    sim
}

/// Per-agent state keyed by stable ID, so slot layout cannot affect the
/// comparison.
#[allow(clippy::type_complexity)]
fn state_by_id(world: &World) -> BTreeMap<AgentId, (u32, u32, u32, u32, u32, u16, bool)> {
    (0..world.len())
        .map(|slot| {
            (
                world.agent_id[slot],
                (
                    world.pos_x[slot].to_bits(),
                    world.pos_y[slot].to_bits(),
                    world.vel_x[slot].to_bits(),
                    world.vel_y[slot].to_bits(),
                    world.yaw[slot].to_bits(),
                    world.route_index[slot],
                    world.arrived[slot],
                ),
            )
        })
        .collect()
}

#[test]
fn repeated_runs_are_bitwise_identical_in_every_scene() {
    for name in scenes::SCENE_NAMES {
        let a = simulate(name, 200, 2026, 300);
        let b = simulate(name, 200, 2026, 300);
        assert_eq!(a.state_hash(), b.state_hash(), "{name} diverged");
        assert_eq!(state_by_id(a.world()), state_by_id(b.world()), "{name}");
    }
}

#[test]
fn state_hashes_agree_at_every_tick() {
    // An end-state comparison can hide a divergence that later reconverges.
    let scene = |seed| {
        scenes::build("bottleneck", 150, seed)
            .unwrap()
            .compile()
            .unwrap()
    };
    let mut a = Simulation::new(
        scene(7),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut b = Simulation::new(
        scene(7),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    for tick in 0..400 {
        a.step();
        b.step();
        assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {tick}");
    }
}

#[test]
fn permuting_spawn_region_order_does_not_change_results() {
    // Reversing the spawn regions changes every agent's slot, so any result
    // that depends on iteration order will differ. Comparing by stable ID
    // isolates that from the legitimate change in slot layout.
    let mut forward = scenes::build("bidirectional_corridor", 200, 99).unwrap();
    let mut reversed = forward.clone();
    reversed.spawns.reverse();

    forward.duration_ticks = 300;
    reversed.duration_ticks = 300;

    let mut a = Simulation::new(
        forward.compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut b = Simulation::new(
        reversed.compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    a.run(300);
    b.run(300);

    assert_eq!(
        state_by_id(a.world()),
        state_by_id(b.world()),
        "results depended on spawn region ordering"
    );
}

#[test]
fn adding_one_agent_does_not_change_existing_agents_attributes() {
    // Contract section 4.2. Trajectories legitimately differ once the extra
    // agent interacts, so this compares derived attributes at spawn.
    let small = simulate("crossing", 100, 5, 1);
    let large = simulate("crossing", 101, 5, 1);

    let attributes = |sim: &Simulation| -> BTreeMap<AgentId, (u32, u32)> {
        let world = sim.world();
        (0..world.len())
            .map(|slot| {
                (
                    world.agent_id[slot],
                    (
                        world.radius[slot].to_bits(),
                        world.preferred_speed[slot].to_bits(),
                    ),
                )
            })
            .collect()
    };

    let small_attributes = attributes(&small);
    let large_attributes = attributes(&large);

    assert!(!small_attributes.is_empty(), "no agents spawned");
    for (id, expected) in &small_attributes {
        assert_eq!(
            large_attributes.get(id),
            Some(expected),
            "agent {id:?} was reshuffled by adding another agent"
        );
    }
}

#[test]
fn changing_the_seed_changes_the_outcome() {
    // Guards against a determinism implementation so aggressive it ignores the
    // seed entirely, which would pass every other test in this file.
    let a = simulate("crossing", 200, 1, 200);
    let b = simulate("crossing", 200, 2, 200);
    assert_ne!(a.state_hash(), b.state_hash());
}

#[test]
fn no_spawn_errors_occur_in_any_scene() {
    for name in scenes::SCENE_NAMES {
        let sim = simulate(name, 500, 3, 100);
        assert!(
            sim.spawn_errors().is_empty(),
            "{name}: {:?}",
            sim.spawn_errors()
        );
    }
}
