use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::SampledVelocitySolver;
use crowd_trace::TraceReader;

#[test]
fn emitted_trace_matches_the_simulation() {
    let scene = scenes::build("crossing", 20, 2026)
        .expect("scene exists")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );

    let dir = std::env::temp_dir().join("crowd-bench-trace-emit");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("emit.crowdtrace");

    // The header declares the scene's total spawn count, not however many
    // agents happen to occupy `world` at the moment `write_trace` is called
    // (spawning is staggered, so that would be 0 before the first tick).
    let agent_count = sim.scene().total_agents() as usize;
    let ticks = crowd_bench::trace_out::write_trace(&mut sim, &path, 25).expect("write");
    assert_eq!(ticks, 25);

    let mut reader = TraceReader::open(&path).expect("open");
    assert_eq!(reader.header().agent_count as usize, agent_count);
    assert_eq!(reader.header().tick_count, 25);

    // The final tick in the trace must equal the simulation's final state.
    let mut out = Vec::new();
    reader.read_tick(24, &mut out).expect("read");
    let world = sim.world();
    assert_eq!(out.len(), agent_count);
    // Occupied slots carry the simulation's real state.
    for (slot, record) in out.iter().enumerate().take(world.len()) {
        assert_eq!(record.agent_id, world.agent_id[slot].0);
        assert_eq!(record.position[0], world.pos_x[slot]);
        assert_eq!(record.position[1], world.pos_y[slot]);
        assert_eq!(record.orientation, world.yaw[slot]);
    }
    // By tick 24 every spawn region has finished staggering agents in, so
    // there should be no padded slots left (all agents have spawned).
    for record in out.iter().skip(world.len()) {
        assert_eq!(record.flags, 0);
    }
    std::fs::remove_file(&path).ok();
}

#[test]
fn unspawned_slots_are_written_as_empty_records() {
    let scene = scenes::build("crossing", 20, 2026)
        .expect("scene exists")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );

    let dir = std::env::temp_dir().join("crowd-bench-trace-emit");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("unspawned.crowdtrace");

    let total_agents = sim.scene().total_agents() as usize;
    let ticks = crowd_bench::trace_out::write_trace(&mut sim, &path, 25).expect("write");
    assert_eq!(ticks, 25);

    let mut reader = TraceReader::open(&path).expect("open");
    assert_eq!(reader.header().agent_count as usize, total_agents);

    // Tick 0: the "crossing" scene spawns only 8 of its 20 agents on the
    // first tick (two regions, `per_tick: 4` each), so most slots are still
    // unoccupied and must be written as empty records.
    let mut tick0 = Vec::new();
    reader.read_tick(0, &mut tick0).expect("read tick 0");
    assert_eq!(tick0.len(), total_agents);
    let occupied_at_tick0 = tick0.iter().filter(|r| r.flags != 0).count();
    assert!(
        occupied_at_tick0 < total_agents,
        "expected some slots still unspawned at tick 0"
    );
    for record in tick0.iter().filter(|r| r.flags == 0) {
        assert_eq!(record.agent_id, 0);
        assert_eq!(record.position, [0.0, 0.0]);
        assert_eq!(record.orientation, 0.0);
    }

    // By the final tick every spawn region has finished, so every slot
    // holds a real agent with a nonzero flag.
    let mut last_tick = Vec::new();
    reader.read_tick(24, &mut last_tick).expect("read tick 24");
    assert_eq!(last_tick.len(), total_agents);
    for record in &last_tick {
        assert_ne!(
            record.flags, 0,
            "expected every slot to hold an agent by tick 24"
        );
    }

    std::fs::remove_file(&path).ok();
}
