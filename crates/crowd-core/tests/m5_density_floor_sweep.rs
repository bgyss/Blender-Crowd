//! Sweep: how does the crowding speed floor trade throughput against contact?
//!
//! `density_adjusted_preferred` scales preferred speed by
//! `1/(1 + density_speed_factor * crowding)`. That decays without bound, so a
//! jammed agent's preferred speed approaches zero exactly when moving is what
//! would relieve the jam. The 100K gate run measured the consequence: stall
//! episodes lasted 2.2x as long as at 10K (143.1 against 65.1 ticks) while the
//! blocking *rate* per metre travelled barely moved.
//!
//! `min_density_speed_fraction` floors that multiplier. The floor is a real
//! trade, not a free win: a higher floor keeps jams dissolving but pushes
//! agents harder into their neighbours, so contact gets worse. This sweep
//! measures both sides at once so the chosen value is a reviewed point on a
//! measured curve rather than a guess.
//!
//! Run it when changing the floor, and record the table in the benchmark note
//! that justifies the new value:
//!
//! ```sh
//! cargo test --release -p crowd-core --test m5_density_floor_sweep -- --ignored --nocapture
//! ```

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::FidelityPolicy;

/// Candidate floors. `0.0` reproduces the unbounded decay the 100K run used,
/// so the sweep always contains its own control.
const FLOORS: [f32; 6] = [0.0, 0.15, 0.25, 0.35, 0.5, 0.7];

struct Row {
    floor: f32,
    completion_rate: f32,
    median_travel_seconds: f32,
    stall_episodes: u64,
    stall_episodes_per_agent_km: f32,
    stall_agent_ticks_per_agent_tick: f32,
    ticks_per_stall_episode: f32,
    deep_penetration_rate: f32,
    mean_penetration_depth_fraction: f32,
    max_penetration_depth: f32,
    penetration_rate: f32,
}

fn measure(agents: u32, ticks: u64, floor: f32) -> Row {
    let scene = scenes::build("m5_city_flow", agents, 2026)
        .expect("m5_city_flow is the declared M5 scale fixture")
        .compile()
        .expect("the fixture must compile");
    let solver = SampledVelocitySolver {
        min_density_speed_fraction: floor,
        ..SampledVelocitySolver::default()
    };
    let mut sim = Simulation::new(
        scene,
        Box::new(solver),
        SimConfig {
            fidelity: Some(FidelityPolicy::m5_10k_profile()),
            ..SimConfig::default()
        },
    );

    for _ in 0..ticks {
        sim.step();
    }

    let summary = sim.metrics().summarize(sim.world(), sim.scene(), 0.0, 0);
    // Read the whole-population figures: the floor is a solver-wide change and
    // the tiers only differ in update cadence, so a per-tier split would add
    // rows without adding information about the trade being made here.
    let stall_ticks_per_episode = if summary.stall_episodes > 0 {
        summary.stall_agent_ticks as f32 / summary.stall_episodes as f32
    } else {
        0.0
    };
    let agent_ticks: u64 = summary.per_tier.iter().map(|t| t.agent_ticks).sum();
    let exposure = agent_ticks.max(1) as f32;
    let distance_km = (summary.distance_travelled_m / 1000.0).max(1e-9);

    Row {
        floor,
        completion_rate: summary.completion_rate,
        median_travel_seconds: summary.median_travel_seconds,
        stall_episodes: summary.stall_episodes,
        stall_episodes_per_agent_km: (summary.stall_episodes as f64 / distance_km) as f32,
        stall_agent_ticks_per_agent_tick: summary.stall_agent_ticks as f32 / exposure,
        ticks_per_stall_episode: stall_ticks_per_episode,
        deep_penetration_rate: summary.deep_penetration_agent_ticks as f32 / exposure,
        mean_penetration_depth_fraction: (summary.penetration_depth_fraction_sum / exposure as f64)
            as f32,
        max_penetration_depth: summary.max_penetration_depth,
        penetration_rate: summary.penetration_agent_ticks as f32 / exposure,
    }
}

fn header() {
    println!(
        "{:>5} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>10} {:>10} {:>9} {:>10}",
        "floor",
        "complete",
        "median_s",
        "stalls",
        "st/km",
        "st_rate",
        "ticks/ep",
        "deep_rate",
        "mean_sev",
        "max_pen",
        "pen_rate",
    );
}

fn print_row(row: &Row) {
    println!(
        "{:>5.2} {:>7.4} {:>9.1} {:>9} {:>9.3} {:>9.6} {:>10.1} {:>10.3e} {:>10.3e} {:>9.6} {:>10.3e}",
        row.floor,
        row.completion_rate,
        row.median_travel_seconds,
        row.stall_episodes,
        row.stall_episodes_per_agent_km,
        row.stall_agent_ticks_per_agent_tick,
        row.ticks_per_stall_episode,
        row.deep_penetration_rate,
        row.mean_penetration_depth_fraction,
        row.max_penetration_depth,
        row.penetration_rate,
    );
}

/// The sweep itself. Not an assertion about which floor is right — that is a
/// reviewed judgement — only the measurement that judgement has to rest on.
#[test]
#[ignore = "minutes per floor; run by name when retuning the density floor"]
fn floor_trades_jam_persistence_against_contact() {
    for agents in [1_000u32, 10_000] {
        // Long enough for the crowd to clear at both scales: the 10K gate runs
        // 45,000 ticks, and a truncated run would credit a floor for stalls it
        // simply never reached.
        let ticks = if agents == 1_000 { 20_000 } else { 45_000 };
        println!("\n=== m5_city_flow, {agents} agents, {ticks} ticks ===");
        header();
        let mut rows = Vec::new();
        for floor in FLOORS {
            let row = measure(agents, ticks, floor);
            print_row(&row);
            rows.push(row);
        }

        // The control must still be a complete run, or the comparison is
        // between a working configuration and a broken one rather than
        // between two floors.
        let control = rows
            .iter()
            .find(|r| r.floor == 0.0)
            .expect("the sweep must include the unbounded-decay control");
        assert!(
            control.completion_rate > 0.99,
            "the control run did not clear the scene ({}); the sweep's baseline is unsound",
            control.completion_rate
        );
    }
}

/// Does the floor change the simulation *at all* at a given scale?
///
/// The aggregate table above can only show that summary metrics coincide.
/// `final_state_hash` is the exact question: two runs with the same hash did
/// the same thing tick for tick. This exists because the sweep reported floors
/// of 0.35 and below as identical at 10K even though a separate probe measured
/// crowding reaching 73 there — a contradiction that had to be resolved by
/// measurement rather than by argument.
#[test]
#[ignore = "minutes per floor; run by name when the sweep shows suspicious ties"]
fn floors_that_should_bind_are_shown_to_change_the_run() {
    let agents = 10_000u32;
    let ticks = 45_000u64;
    let mut hashes = Vec::new();
    for floor in [0.0f32, 0.35, 0.5, 0.7] {
        let scene = scenes::build("m5_city_flow", agents, 2026)
            .expect("m5_city_flow is the declared M5 scale fixture")
            .compile()
            .expect("the fixture must compile");
        let mut sim = Simulation::new(
            scene,
            Box::new(SampledVelocitySolver {
                min_density_speed_fraction: floor,
                ..SampledVelocitySolver::default()
            }),
            SimConfig {
                fidelity: Some(FidelityPolicy::m5_10k_profile()),
                ..SimConfig::default()
            },
        );
        for _ in 0..ticks {
            sim.step();
        }
        let hash = sim.state_hash();
        println!("floor {floor:>4.2} -> final_state_hash {hash}");
        hashes.push((floor, hash));
    }

    let control = hashes[0].1;
    for (floor, hash) in &hashes[1..] {
        println!(
            "floor {floor:>4.2}: {}",
            if *hash == control {
                "IDENTICAL to no floor - the floor never binds at this scale"
            } else {
                "differs from no floor - the floor binds"
            }
        );
    }
}
