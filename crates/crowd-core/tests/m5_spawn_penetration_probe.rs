//! Diagnostic: when does the M5 scale fixture's worst overlap actually happen?
//!
//! `max_penetration_depth` is an extremum over the whole run, so it says
//! nothing about *when*. `spawn::place_clear_of_others` already documents that
//! co-located placement "accounted for every penetration event recorded in the
//! open benchmark scenes -- all of it inside the first tenth of a run", and its
//! rejection sampler gives up after a bounded number of attempts and places the
//! agent anyway. At 100K there are ten times as many draws into a spawn band of
//! the same density, so the fallback fires more often.
//!
//! **That hypothesis was tested and refuted**, which is why this file is kept.
//! Measured 2026-08-14 on the schema-v5 fixture:
//!
//! | population | overlap when emission completes | worst in first 400 ticks | worst over the full gate run |
//! | --- | ---: | ---: | ---: |
//! | 10,000 | 0.000000 m | 0.000000 m | 0.018272 m |
//! | 100,000 | 0.000000 m | 0.006531 m (tick 169) | 0.135480 m |
//!
//! The spawn placer is not the source: it emits a cleanly separated crowd at
//! both scales. The deep overlaps develop over the route, so they are
//! congestion behaviour and have to be addressed as such. Keeping this probe
//! stops the placement theory from being re-proposed without evidence.
//!
//! This test steps tick by tick and records the tick at which the running
//! maximum was last raised. It is `#[ignore]`d because the 100K case takes
//! minutes; run it by name when investigating a penetration result.
//!
//! ```sh
//! cargo test --release -p crowd-core --test m5_spawn_penetration_probe -- --ignored --nocapture
//! ```

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::FidelityPolicy;

struct Probe {
    agents: u32,
    ticks: u64,
    max_depth: f32,
    /// Tick on which the running maximum was last raised.
    max_tick: u64,
    /// Depth reached by the end of the emission burst, for comparison with the
    /// final figure: if they match, nothing after spawn made contact worse.
    depth_after_emission: f32,
    emission_ticks: u64,
}

fn probe(agents: u32, ticks: u64) -> Probe {
    let scene = scenes::build("m5_city_flow", agents, 2026)
        .expect("m5_city_flow is the declared M5 scale fixture")
        .compile()
        .expect("the fixture must compile");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig {
            fidelity: Some(FidelityPolicy::m5_10k_profile()),
            ..SimConfig::default()
        },
    );

    let mut max_depth = 0.0f32;
    let mut max_tick = 0;
    let mut emission_ticks = 0;
    let mut depth_after_emission = 0.0f32;
    let mut spawned = 0;

    for tick in 0..ticks {
        sim.step();
        let depth = sim.metrics().max_penetration_depth();
        if depth > max_depth {
            max_depth = depth;
            max_tick = tick;
        }
        if sim.world().len() > spawned {
            spawned = sim.world().len();
            emission_ticks = tick + 1;
            depth_after_emission = depth;
        }
    }

    Probe {
        agents,
        ticks,
        max_depth,
        max_tick,
        depth_after_emission,
        emission_ticks,
    }
}

fn report(probe: &Probe) {
    println!(
        "{:>7} agents | {:>5} ticks | emission complete at tick {:>3} \
         | depth at end of emission {:.6} m | max {:.6} m first reached at tick {}",
        probe.agents,
        probe.ticks,
        probe.emission_ticks,
        probe.depth_after_emission,
        probe.max_depth,
        probe.max_tick,
    );
}

/// The load-bearing question: is the scale fixture's worst overlap created by
/// steering under congestion, or by the spawn placer giving up?
#[test]
#[ignore = "minutes at 100K; run by name when investigating penetration"]
fn worst_overlap_is_attributed_to_a_tick() {
    let mut probes = Vec::new();
    for agents in [10_000u32, 100_000] {
        // Only the opening of the run: emission finishes within a handful of
        // ticks, so a few hundred is enough to separate placement from
        // steering. A depth set here that the full run never exceeds is
        // placement; one the full run exceeds later is congestion.
        let probe = probe(agents, 400);
        report(&probe);
        probes.push(probe);
    }

    let hundred_k = &probes[1];
    println!(
        "\n100K run recorded {:.6} m within the first {} ticks; \
         the full 142,302-tick gate run recorded 0.135480 m.",
        hundred_k.max_depth, hundred_k.ticks
    );
}
