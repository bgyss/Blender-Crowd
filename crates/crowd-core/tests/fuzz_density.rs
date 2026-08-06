//! Randomised density stress, contract section 15.1: checked for NaN, escape,
//! and deadlock.

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};

fn stress(scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
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

#[test]
fn no_agent_state_goes_non_finite_under_density() {
    for seed in 0..8u64 {
        for scene in scenes::SCENE_NAMES {
            let sim = stress(scene, 800, seed, 400);
            for slot in 0..sim.world().len() {
                let position = sim.world().position(slot as u32);
                let velocity = sim.world().velocity(slot as u32);
                assert!(
                    position.is_finite() && velocity.is_finite(),
                    "{scene} seed {seed} slot {slot} went non-finite"
                );
                assert!(sim.world().yaw[slot].is_finite());
            }
        }
    }
}

#[test]
fn no_agent_escapes_far_beyond_the_scene_bounds() {
    // A small margin is legitimate: the wall push-out resolves penetration
    // against the nearest surface, which can nudge an agent just outside.
    const MARGIN: f32 = 2.0;
    for seed in 0..4u64 {
        for scene in scenes::SCENE_NAMES {
            let sim = stress(scene, 800, seed, 400);
            let bounds = sim.scene().bounds.expanded(MARGIN);
            for slot in 0..sim.world().len() {
                let position = sim.world().position(slot as u32);
                assert!(
                    bounds.contains(position),
                    "{scene} seed {seed} slot {slot} escaped to {position:?}"
                );
            }
        }
    }
}

#[test]
fn speeds_never_exceed_the_per_agent_maximum() {
    for scene in scenes::SCENE_NAMES {
        let sim = stress(scene, 500, 11, 300);
        for slot in 0..sim.world().len() {
            let speed = sim.world().velocity(slot as u32).length();
            assert!(
                speed <= sim.world().max_speed[slot] + 1e-3,
                "{scene} slot {slot} exceeded max speed: {speed}"
            );
        }
    }
}

#[test]
fn the_crowd_does_not_deadlock_wholesale() {
    // Not a quality threshold: this asserts only that the simulation is still
    // making progress, which is the difference between a slow crowd and a
    // frozen one. Real quality bars come from measured baselines.
    //
    // Requires a *fraction* of the unfinished population to be moving. The
    // earlier `moving > 0` form passed with 399 of 400 agents frozen solid —
    // exactly the state it was meant to catch.
    const MIN_MOVING_FRACTION: f32 = 0.1;

    for scene in scenes::SCENE_NAMES {
        let sim = stress(scene, 400, 3, 900);
        let unfinished: Vec<usize> = (0..sim.world().len())
            .filter(|slot| !sim.world().arrived[*slot] && !sim.world().unrouted[*slot])
            .collect();
        if unfinished.len() <= 10 {
            continue;
        }
        let moving = unfinished
            .iter()
            .filter(|slot| sim.world().velocity(**slot as u32).length() > 0.05)
            .count();
        let fraction = moving as f32 / unfinished.len() as f32;
        assert!(
            fraction >= MIN_MOVING_FRACTION,
            "{scene}: only {moving}/{} unfinished agents moving ({:.0}%)",
            unfinished.len(),
            fraction * 100.0
        );
    }
}
