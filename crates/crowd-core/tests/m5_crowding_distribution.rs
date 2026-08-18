//! Diagnostic: how crowded do agents actually get, and does it change with
//! population?
//!
//! This exists to decide whether the crowding speed term is the right lever on
//! the 100K stall result at all. `density_adjusted_preferred` scales preferred
//! speed by `1/(1 + 0.18 * crowding)`, so a floor of `f` only ever binds above
//! `crowding > (1/f - 1) / 0.18` neighbours:
//!
//! | floor | binds above |
//! | ---: | ---: |
//! | 0.70 | 2.4 |
//! | 0.50 | 5.6 |
//! | 0.35 | 10.3 |
//! | 0.25 | 16.7 |
//!
//! The floor sweep (`m5_density_floor_sweep`) measured floors of 0.35 and
//! below as *bit-identical* to no floor at 1,000 and 10,000 agents on
//! `m5_city_flow`, which says crowding rarely reaches ~10 there. That leaves
//! the load-bearing question unanswered, because the failing gate is at
//! 100,000 — if crowding is no higher there either, then a floor low enough to
//! leave 1K and 10K untouched is a no-op at every scale and cannot be the fix.
//!
//! So this probe measures the distribution directly at both populations,
//! counting neighbours inside the same clearance the solver uses, and — as the
//! solver does — ignoring agents that have already arrived. There is also a
//! hard ceiling on the answer: `PerceiveConfig::budget` keeps only the nearest
//! 16 neighbours, so the density term saturates there no matter how dense the
//! crowd really is. It runs a
//! bounded prefix rather than a full gate run: the 100K route is ~52,000 ticks
//! and congestion is well developed long before that, so 20,000 ticks answers
//! the question in about half an hour instead of three.
//!
//! ```sh
//! cargo test --release -p crowd-core --test m5_crowding_distribution -- --ignored --nocapture
//! ```

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::grid::UniformGrid;
use crowd_core::phases::perceive::PerceiveConfig;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::units::Vec2;
use crowd_core::FidelityPolicy;

/// Bucket edges, chosen to straddle the floors the sweep tried.
const BUCKETS: [u32; 7] = [0, 1, 3, 6, 10, 17, 25];

/// Clearance the crowding count uses, matching `SampledVelocitySolver`'s
/// default `personal_space`. Read from the solver so the two cannot drift.
fn personal_space() -> f32 {
    SampledVelocitySolver::default().personal_space
}

struct Distribution {
    agents: u32,
    samples: u64,
    /// Agent-samples whose crowding fell in each bucket.
    histogram: [u64; BUCKETS.len()],
    max_crowding: u32,
    /// Share of samples at or above each floor's binding point.
    share_above_10: f64,
    share_above_5: f64,
    /// Crowding is capped by what the solver can see: perception keeps only
    /// the `budget` nearest non-arrived neighbours, so the density term can
    /// never observe more than that many however dense the crowd truly is.
    perception_budget: usize,
}

fn measure(agents: u32, ticks: u64, sample_every: u64) -> Distribution {
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

    let clearance = personal_space();
    let mut histogram = [0u64; BUCKETS.len()];
    let mut samples = 0u64;
    let mut max_crowding = 0u32;
    let mut above_10 = 0u64;
    let mut above_5 = 0u64;

    for tick in 0..ticks {
        sim.step();
        if tick % sample_every != 0 {
            continue;
        }

        let world = sim.world();
        let bounds = sim.scene().bounds;
        // Rebuilt per sampled tick rather than reusing the simulation's own
        // index: this counts neighbours inside `personal_space`, which is a
        // different query than the perception radius the solver's arena holds.
        let mut grid = UniformGrid::new(bounds, 5.0);
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut near = Vec::new();

        for slot in 0..world.len() {
            if world.arrived[slot] || world.unrouted[slot] {
                continue;
            }
            let position = world.position(slot as u32);
            let radius = world.radius[slot];
            // Query generously, then apply the exact per-pair clearance below:
            // the grid query takes one radius for all candidates, but the
            // solver's test uses each pair's own combined radius.
            near.clear();
            grid.query(position, clearance + radius + 1.0, &mut near);
            let mut crowding = 0u32;
            for other in near.iter().map(|o| *o as usize) {
                if other == slot {
                    continue;
                }
                // Perception drops arrived agents (see `phases::perceive`), so
                // counting them here would measure a crowd the solver never
                // sees. Agents park on their destination node and stay there,
                // so a destination accumulates a dense pile that is invisible
                // to steering; an earlier version of this probe counted those
                // piles and reported crowding of 73 where the solver saw a
                // handful.
                if world.arrived[other] {
                    continue;
                }
                let limit = radius + world.radius[other] + clearance;
                let delta: Vec2 = world.position(other as u32) - position;
                if delta.length_squared() < limit * limit {
                    crowding += 1;
                }
            }

            samples += 1;
            max_crowding = max_crowding.max(crowding);
            if crowding > 10 {
                above_10 += 1;
            }
            if crowding > 5 {
                above_5 += 1;
            }
            let bucket = BUCKETS
                .iter()
                .rposition(|edge| crowding >= *edge)
                .unwrap_or(0);
            histogram[bucket] += 1;
        }
    }

    let total = samples.max(1) as f64;
    Distribution {
        agents,
        samples,
        histogram,
        max_crowding,
        share_above_10: above_10 as f64 / total,
        share_above_5: above_5 as f64 / total,
        perception_budget: PerceiveConfig::default().budget,
    }
}

fn report(d: &Distribution) {
    println!(
        "\n{} agents | {} agent-samples | max crowding {}",
        d.agents, d.samples, d.max_crowding
    );
    for (i, edge) in BUCKETS.iter().enumerate() {
        let upper = BUCKETS
            .get(i + 1)
            .map(|next| format!("{}", next - 1))
            .unwrap_or_else(|| "+".to_string());
        let share = d.histogram[i] as f64 / d.samples.max(1) as f64;
        println!(
            "  crowding {edge:>2}-{upper:<3} {:>12} {:>8.4}%",
            d.histogram[i],
            share * 100.0
        );
    }
    println!(
        "  share above 5 (floor 0.50 binds): {:.6}%\n  share above 10 (floor 0.35 binds): {:.6}%\n           perception budget (hard cap on what the density term can see): {}",
        d.share_above_5 * 100.0,
        d.share_above_10 * 100.0,
        d.perception_budget
    );
}

/// Does local crowding actually get worse at 100K, or is the extra stalling
/// entirely a longer-route effect?
#[test]
#[ignore = "~30 minutes at 100K; run by name when deciding on the density term"]
fn crowding_distribution_is_compared_across_scales() {
    let mut distributions = Vec::new();
    for agents in [10_000u32, 100_000] {
        // Every 250th tick: crowding is strongly autocorrelated tick to tick,
        // so denser sampling would multiply the cost without adding
        // independent information.
        let d = measure(agents, 20_000, 250);
        report(&d);
        distributions.push(d);
    }

    // Not an assertion about which way the answer should come out — only that
    // the probe observed a live crowd at both scales, so a flat histogram
    // means "crowding is low" rather than "nothing was measured".
    for d in &distributions {
        assert!(
            d.samples > 0,
            "{} agents produced no samples; the probe measured nothing",
            d.agents
        );
    }
}
