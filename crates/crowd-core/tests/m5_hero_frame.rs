//! Dump one frame of the 100K scale fixture, for a figure that is honest about
//! how many agents are actually on screen.
//!
//! The Blender scale proof inspects the addon's reference scene, which emits
//! over time — 1,200 of 100,000 agents were present at the frame it captured.
//! That is the right evidence for "the population is not expanded into
//! per-agent scene objects" and the wrong image for "here are 100,000 agents".
//!
//! `m5_city_flow` is different: emission finishes within a handful of ticks and
//! the earliest arrivals are ~42,000 ticks away, so there is a long window
//! where essentially the whole population is en route at once. This dumps one
//! tick from inside that window, plus the occupancy curve that proves the tick
//! was chosen for concurrency rather than for flattery.
//!
//! ```sh
//! M5_HERO_AGENTS=100000 M5_HERO_TICK=10000 \
//!   cargo test --release -p crowd-core --test m5_hero_frame -- --ignored --nocapture
//! ```

use std::io::Write;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::FidelityPolicy;

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[test]
#[ignore = "minutes at 100K; run by name to regenerate the hero frame"]
fn dump_a_frame_where_the_population_is_actually_concurrent() {
    let agents = env_u64("M5_HERO_AGENTS", 100_000) as u32;
    // Several ticks in one run: the simulation is the cost, a dump is not, and
    // the two opposing lane blocks start at opposite ends of the scene and only
    // overlap in x around the middle of the journey.
    let ticks: Vec<u64> = std::env::var("M5_HERO_TICKS")
        .unwrap_or_else(|_| "10000".to_string())
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    let target = *ticks.iter().max().expect("at least one tick");
    let stem = std::env::var("M5_HERO_OUT").unwrap_or_else(|_| "hero-frame".to_string());

    let scene = scenes::build("m5_city_flow", agents, 2026)
        .expect("m5_city_flow is the declared M5 scale fixture")
        .compile()
        .expect("the fixture must compile");
    let bounds = scene.bounds;
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig {
            fidelity: Some(FidelityPolicy::m5_10k_profile()),
            ..SimConfig::default()
        },
    );

    println!("bounds {:?} .. {:?}", bounds.min, bounds.max);
    println!("{:>8} {:>10} {:>8} {:>26}", "tick", "present", "share", "x span of each direction");
    for tick in 1..=target {
        sim.step();
        let dump = ticks.contains(&tick);
        if tick % 2500 == 0 || dump {
            let w = sim.world();
            let present = (0..w.len()).filter(|s| !w.arrived[*s] && !w.unrouted[*s]).count();
            println!("{:>8} {:>10} {:>7.1}%", tick, present, 100.0 * present as f64 / agents as f64);
        }
        if !dump {
            continue;
        }

        let w = sim.world();
        let out = format!("{stem}-{tick}.csv");
        let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("cannot write frame"));
        writeln!(f, "x,y,tier,speed").unwrap();
        let mut present = 0u64;
        for slot in 0..w.len() {
            if w.arrived[slot] || w.unrouted[slot] {
                continue;
            }
            present += 1;
            let p = w.position(slot as u32);
            let v = w.velocity(slot as u32);
            writeln!(f, "{:.3},{:.3},{},{:.3}", p.x, p.y, w.simulation_tier[slot] as usize, v.length()).unwrap();
        }
        drop(f);
        println!("   -> {out}: {present} agents ({:.1}%)", 100.0 * present as f64 / agents as f64);
        assert!(
            present as f64 / agents as f64 > 0.95,
            "only {present} of {agents} agents present at tick {tick}; not a concurrent frame"
        );
    }
}
