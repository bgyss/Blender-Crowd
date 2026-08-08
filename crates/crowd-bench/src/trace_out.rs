//! Emit a trace v0 file from a running simulation.
//!
//! This is the producer half of the Blender bridge: it is how a headless
//! simulation result reaches Blender with no live simulation session.

use std::path::Path;

use crowd_core::sim::Simulation;
use crowd_core::units::{DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
use crowd_trace::{AgentRecord, TraceError, TraceWriter, FLAG_ACTIVE, FLAG_ARRIVED};

/// An all-default record: `agent_id: 0`, `position: [0.0, 0.0]`,
/// `orientation: 0.0`, `flags: 0`. Marks a slot with no spawned agent yet.
/// See the `flags == 0` documentation on `FLAG_ACTIVE`/`FLAG_ARRIVED` in
/// `crowd_trace::record`.
const EMPTY_SLOT: AgentRecord = AgentRecord {
    agent_id: 0,
    position: [0.0, 0.0],
    orientation: 0.0,
    flags: 0,
    clip_index: 0,
    phase: 0.0,
    playback_rate: 1.0,
    render_tier: 0,
};

/// Step `sim` for `ticks` ticks, writing one trace record per agent per tick.
///
/// The header declares `sim.scene().total_agents()` records per tick — the
/// scene's total spawn count, fixed for the whole run — not `world.len()`,
/// which only reaches that count once every spawn region has finished
/// staggering its agents in. Slots not yet occupied are padded with
/// [`EMPTY_SLOT`]. Records are emitted in world slot order, which is the
/// world's own stable order (spawns only append; nothing is ever removed).
/// Returns the number of ticks written.
pub fn write_trace(sim: &mut Simulation, path: &Path, ticks: u64) -> Result<u64, TraceError> {
    let agent_count = sim.scene().total_agents();
    let mut writer =
        TraceWriter::create(path, agent_count, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER)?;

    let mut batch: Vec<AgentRecord> = Vec::with_capacity(agent_count as usize);
    for _ in 0..ticks {
        sim.step();
        batch.clear();
        let world = sim.world();
        for slot in 0..world.len() {
            batch.push(AgentRecord {
                agent_id: world.agent_id[slot].0,
                position: [world.pos_x[slot], world.pos_y[slot]],
                orientation: world.yaw[slot],
                flags: if world.arrived[slot] {
                    FLAG_ARRIVED
                } else {
                    FLAG_ACTIVE
                },
                // Stubs: no animation system exists yet. See the slice design.
                clip_index: 0,
                phase: 0.0,
                playback_rate: 1.0,
                render_tier: 0,
            });
        }
        batch.resize(agent_count as usize, EMPTY_SLOT);
        writer.write_tick(&batch)?;
    }

    writer.finish()
}
