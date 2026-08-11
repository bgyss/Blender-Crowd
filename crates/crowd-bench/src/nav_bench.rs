//! Runs `two_room`, closes the south door partway through, and reports the
//! reroute outcome — the CLI-facing form of M0 acceptance criterion 3.

use std::path::PathBuf;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::nav::NavDebugSnapshot;
use crowd_core::nav_scenes::{two_room, NORTH_DOOR, SOUTH_DOOR};
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::NO_ROUTE;
use serde::{Deserialize, Serialize};

use crate::svg::TrajectoryRecorder;

pub struct NavRerouteOptions {
    pub agents: u32,
    pub seed: u64,
    pub out_dir: PathBuf,
    pub svg: bool,
    /// Ticks to run before the initial routing pass is assumed complete, and
    /// again after the close, before measuring arrivals.
    pub settle_ticks: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NavRerouteReport {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub invalidated_on_close: u32,
    pub untouched_on_close: u32,
    /// Of the agents invalidated by the close, how many had a live route by
    /// the time arrivals were measured whose recorded portal sequence
    /// actually crosses a north_door portal — direct evidence the doorway
    /// was genuinely closed, not just that *some* route was assigned.
    pub crossed_north_door: u32,
    pub arrived_after_reroute: u64,
}

pub fn run_nav_reroute(options: &NavRerouteOptions) -> Result<NavRerouteReport, String> {
    let compiled = two_room(options.agents, options.seed)
        .compile()
        .map_err(|e| format!("{e:?}"))?;
    let mut sim = Simulation::new(
        compiled,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );

    let mut recorder = TrajectoryRecorder::new(10, options.agents as usize);
    for _ in 0..options.settle_ticks {
        sim.step();
        recorder.record(&sim);
    }

    let south: Vec<_> = sim
        .nav()
        .map(|nav| nav.portals_named(SOUTH_DOOR).to_vec())
        .unwrap_or_default();
    if south.is_empty() {
        return Err("south_door named no portals".to_string());
    }
    let route_before: Vec<_> = (0..sim.world().len())
        .map(|s| sim.world().route[s])
        .collect();
    sim.set_portals_open(&south, false);

    let mut invalidated_on_close = 0u32;
    let mut untouched_on_close = 0u32;
    for (slot, before) in route_before.iter().enumerate() {
        if sim.world().route[slot] == NO_ROUTE {
            invalidated_on_close += 1;
        } else {
            debug_assert_eq!(sim.world().route[slot], *before);
            untouched_on_close += 1;
        }
    }

    for _ in 0..options.settle_ticks {
        sim.step();
        recorder.record(&sim);
    }

    let north: Vec<_> = sim
        .nav()
        .map(|nav| nav.portals_named(NORTH_DOOR).to_vec())
        .unwrap_or_default();
    let mut crossed_north_door = 0u32;
    for (slot, before) in route_before.iter().enumerate() {
        let was_invalidated = sim.world().route[slot] != *before;
        if was_invalidated && sim.route_crosses_any(slot as u32, &north) {
            crossed_north_door += 1;
        }
    }

    if options.svg {
        std::fs::create_dir_all(&options.out_dir).map_err(|e| e.to_string())?;
        let snapshot = sim.nav().map(|nav| {
            NavDebugSnapshot::capture(nav, sim.world(), sim.routes(), options.agents as usize)
        });
        let svg = recorder.write_svg_with_nav(
            "two_room",
            sim.scene().bounds,
            sim.walls(),
            snapshot.as_ref(),
        );
        let path = options
            .out_dir
            .join(format!("two_room-reroute-{}.svg", options.agents));
        std::fs::write(&path, svg).map_err(|e| e.to_string())?;
    }

    Ok(NavRerouteReport {
        scene: "two_room".to_string(),
        agents: options.agents,
        seed: options.seed,
        invalidated_on_close,
        untouched_on_close,
        crossed_north_door,
        arrived_after_reroute: sim.metrics().arrived(),
    })
}
