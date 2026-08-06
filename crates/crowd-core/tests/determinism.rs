//! The determinism contract, contract section 9.4 `Strict` mode.
//!
//! The claim is bitwise-identical output for the same binary on the same
//! machine. Cross-machine identity is not claimed.

use std::collections::BTreeMap;

use crowd_core::avoidance::{AnticipatorySolver, AvoidanceSolver, OrcaSolver, SampledVelocitySolver};
use crowd_core::ids::AgentId;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::World;

const SOLVER_NAMES: [&str; 3] = ["sampled_velocity", "orca", "anticipatory"];

fn boxed_solver(name: &str) -> Box<dyn AvoidanceSolver> {
    match name {
        "sampled_velocity" => Box::new(SampledVelocitySolver::default()),
        "orca" => Box::new(OrcaSolver::default()),
        "anticipatory" => Box::new(AnticipatorySolver::default()),
        other => panic!("unknown solver: {other}"),
    }
}

fn simulate(solver_name: &str, scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
    let scene = scenes::build(scene_name, agents, seed)
        .expect("known scene")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(scene, boxed_solver(solver_name), SimConfig::default());
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
    for solver_name in SOLVER_NAMES {
        for name in scenes::SCENE_NAMES {
            let a = simulate(solver_name, name, 200, 2026, 300);
            let b = simulate(solver_name, name, 200, 2026, 300);
            assert_eq!(a.state_hash(), b.state_hash(), "{solver_name}/{name} diverged");
            assert_eq!(
                state_by_id(a.world()),
                state_by_id(b.world()),
                "{solver_name}/{name}"
            );
        }
    }
}

#[test]
fn state_hashes_agree_at_every_tick() {
    for solver_name in SOLVER_NAMES {
        // An end-state comparison can hide a divergence that later reconverges.
        let scene = |seed| {
            scenes::build("bottleneck", 150, seed)
                .unwrap()
                .compile()
                .unwrap()
        };
        let mut a = Simulation::new(scene(7), boxed_solver(solver_name), SimConfig::default());
        let mut b = Simulation::new(scene(7), boxed_solver(solver_name), SimConfig::default());
        for tick in 0..400 {
            a.step();
            b.step();
            assert_eq!(
                a.state_hash(),
                b.state_hash(),
                "{solver_name} diverged at tick {tick}"
            );
        }
    }
}

#[test]
fn permuting_spawn_region_order_does_not_change_results() {
    for solver_name in SOLVER_NAMES {
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
            boxed_solver(solver_name),
            SimConfig::default(),
        );
        let mut b = Simulation::new(
            reversed.compile().unwrap(),
            boxed_solver(solver_name),
            SimConfig::default(),
        );
        a.run(300);
        b.run(300);

        assert_eq!(
            state_by_id(a.world()),
            state_by_id(b.world()),
            "{solver_name}: results depended on spawn region ordering"
        );
    }
}

#[test]
fn adding_one_agent_does_not_change_existing_agents_attributes() {
    for solver_name in SOLVER_NAMES {
        // Contract section 4.2. Trajectories legitimately differ once the extra
        // agent interacts, so this compares derived attributes at spawn.
        let small = simulate(solver_name, "crossing", 100, 5, 1);
        let large = simulate(solver_name, "crossing", 101, 5, 1);

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
                "{solver_name}: agent {id:?} was reshuffled by adding another agent"
            );
        }
    }
}

#[test]
fn changing_the_seed_changes_the_outcome() {
    for solver_name in SOLVER_NAMES {
        // Guards against a determinism implementation so aggressive it ignores the
        // seed entirely, which would pass every other test in this file.
        let a = simulate(solver_name, "crossing", 200, 1, 200);
        let b = simulate(solver_name, "crossing", 200, 2, 200);
        assert_ne!(a.state_hash(), b.state_hash(), "{solver_name}");
    }
}

#[test]
fn no_spawn_errors_occur_in_any_scene() {
    for name in scenes::SCENE_NAMES {
        let sim = simulate("sampled_velocity", name, 500, 3, 100);
        assert!(
            sim.spawn_errors().is_empty(),
            "{name}: {:?}",
            sim.spawn_errors()
        );
    }
}
