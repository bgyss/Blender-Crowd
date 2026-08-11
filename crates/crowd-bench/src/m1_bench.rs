//! Strict M1 reference-concourse bake and cache comparison.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crowd_cache::{
    content_hash, AgentStatic, BakeSpec, CacheReader, CacheStatus, CacheWriter, ChannelDef, Frame,
    FrameRecord, RecoveryInspector, ScalarType, CACHE_V1_DEFAULTS,
};
use crowd_core::{
    compile_concourse, compile_project, AgentId, AgentSnapshot, CommuterState, CompiledAgentSpawn,
    DecisionReason, Phase, ProjectIrV1, SampledVelocitySolver, SimConfig, Simulation, Vec2,
    NO_ROUTE,
};
use serde::{Deserialize, Serialize};

const REFERENCE_JSON: &str = include_str!("../../../assets/reference/concourse-project-v1.json");
const REQUIRED_FRAME_CHANNELS: [&str; 15] = [
    "agent_id",
    "position",
    "orientation",
    "scale",
    "population_id",
    "variant_id",
    "clip_id",
    "phase",
    "playback_rate",
    "behavior_state",
    "decision_reason",
    "destination_id",
    "velocity",
    "visible",
    "render_tier",
];

#[derive(Clone, Debug)]
pub struct StrictBakeOptions {
    pub cache_path: PathBuf,
}

impl StrictBakeOptions {
    pub fn reference(cache_path: PathBuf) -> Self {
        Self { cache_path }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalRerouteReport {
    pub portal_id: String,
    pub close_tick: u64,
    pub reopen_tick: u64,
    pub routes_using_portal_before_close: usize,
    pub unrelated_routes_before_close: usize,
    pub invalidated_routes: usize,
    pub unrelated_routes_unchanged: bool,
    pub all_invalidated_routes_recovered_by_reopen: bool,
    pub recovered_by_tick: Option<u64>,
    pub accepted: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseTiming {
    pub phase: String,
    pub nanoseconds: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrictBakeReport {
    pub schema_version: u32,
    pub cache_path: String,
    pub project_id: String,
    pub source_hash: String,
    pub tick_start: u64,
    pub tick_end: u64,
    pub agent_count: u32,
    pub unique_agent_ids: usize,
    pub static_digest: String,
    pub discrete_digest: String,
    pub destination_completion: f32,
    pub agents_arrived: usize,
    pub static_boundary_escapes: u64,
    pub portal_reroute: PortalRerouteReport,
    pub required_channels_missing: Vec<String>,
    pub position_quantization_bound_m: f32,
    pub simulation_duration_ns: u64,
    pub cache_write_duration_ns: u64,
    pub sequential_cache_read_duration_ns: u64,
    pub cache_size_bytes: u64,
    pub phase_timings: Vec<PhaseTiming>,
    pub selected_agent_id: u64,
    pub selected_agent_tick: u64,
    pub selected_agent_evidence_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrictComparisonReport {
    pub first_cache: String,
    pub second_cache: String,
    pub static_channels_equal: bool,
    pub discrete_channels_equal: bool,
    pub continuous_channels_equal_except_position: bool,
    pub max_position_delta_m: f32,
    pub declared_position_bound_m: f32,
    pub accepted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalStateEvidence {
    pub portal_id: String,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecisionTraceV1 {
    pub schema_version: u32,
    pub agent_id: u64,
    pub tick: u64,
    pub position: [f32; 2],
    pub desired_velocity: [f32; 2],
    pub solved_velocity: [f32; 2],
    pub corridor_portal_ids: Vec<u32>,
    pub corridor_points: Vec<[f32; 2]>,
    pub next_target: Option<[f32; 2]>,
    pub destination_id: u32,
    pub path_status: String,
    pub commuter_state_code: u16,
    pub commuter_state: String,
    pub clip_id: u16,
    pub clip_phase: f32,
    pub playback_rate: f32,
    pub relevant_portals: Vec<PortalStateEvidence>,
    pub decision_code: u16,
    pub decision_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceValidationReport {
    pub project_id: String,
    pub source_hash: String,
    pub agent_count: usize,
    pub unique_agent_ids: usize,
    pub spawn_count: usize,
    pub destination_count: usize,
    pub portal_count: usize,
    pub archetype_count: usize,
    pub appearance_count: usize,
    pub valid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancellationReport {
    pub cache_path: String,
    pub status: String,
    pub last_complete_tick: Option<u64>,
    pub valid_chunk_count: usize,
    pub complete_reader_rejected: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletionProbeReport {
    pub ticks: u64,
    pub agents: usize,
    pub arrived: usize,
    pub traveling: usize,
    pub blocked: usize,
    pub completion: f32,
    pub moving_agents: usize,
    pub mean_speed_mps: f32,
    pub mean_distance_to_destination_m: f32,
    pub by_destination: Vec<DestinationProbe>,
    pub simulation_duration_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DestinationProbe {
    pub destination_id: u32,
    pub agents: usize,
    pub arrived: usize,
    pub mean_distance_m: f32,
}

pub fn validate_reference() -> Result<ReferenceValidationReport, String> {
    let project = reference_project()?;
    let ids: HashSet<_> = project
        .agent_spawns()
        .iter()
        .map(|agent| agent.agent_id)
        .collect();
    let ir = project.ir();
    let report = ReferenceValidationReport {
        project_id: ir.project_id.clone(),
        source_hash: project.source_hash_hex(),
        agent_count: project.agent_spawns().len(),
        unique_agent_ids: ids.len(),
        spawn_count: ir.semantics.spawns.len(),
        destination_count: ir.semantics.destinations.len(),
        portal_count: ir.semantics.portals.len(),
        archetype_count: ir.populations.iter().map(|p| p.archetypes.len()).sum(),
        appearance_count: ir.populations.iter().map(|p| p.appearances.len()).sum(),
        valid: project.agent_spawns().len() == 1_000
            && ids.len() == 1_000
            && ir.semantics.spawns.len() == 2
            && ir.semantics.destinations.len() == 3
            && ir.semantics.portals.len() == 2,
    };
    Ok(report)
}

pub fn probe_reference_completion(
    ticks: u64,
    agent_count: u32,
) -> Result<CompletionProbeReport, String> {
    let project = reference_project_with_agents(agent_count)?;
    let scene = compile_concourse(&project).map_err(format_diagnostics)?;
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let started = Instant::now();
    simulation.run(ticks);
    let simulation_duration_ns = elapsed_ns(started);
    let arrived = simulation
        .world()
        .commuter_state
        .iter()
        .filter(|state| **state == CommuterState::Arrived)
        .count();
    let blocked = simulation
        .world()
        .commuter_state
        .iter()
        .filter(|state| **state == CommuterState::Blocked)
        .count();
    let traveling = simulation.world().len() - arrived - blocked;
    let speeds: Vec<f32> = (0..simulation.world().len())
        .map(|slot| simulation.world().velocity(slot as u32).length())
        .collect();
    let distances: Vec<f32> = (0..simulation.world().len())
        .map(|slot| {
            simulation
                .scene()
                .destination_position(simulation.world().destination[slot])
                .map_or(0.0, |destination| {
                    simulation
                        .world()
                        .position(slot as u32)
                        .distance_squared(destination)
                        .sqrt()
                })
        })
        .collect();
    let by_destination = (0..simulation.scene().destinations.len())
        .map(|destination_id| {
            let slots: Vec<_> = (0..simulation.world().len())
                .filter(|slot| simulation.world().destination[*slot] as usize == destination_id)
                .collect();
            DestinationProbe {
                destination_id: destination_id as u32,
                agents: slots.len(),
                arrived: slots
                    .iter()
                    .filter(|slot| simulation.world().arrived[**slot])
                    .count(),
                mean_distance_m: slots.iter().map(|slot| distances[*slot]).sum::<f32>()
                    / slots.len().max(1) as f32,
            }
        })
        .collect();
    Ok(CompletionProbeReport {
        ticks,
        agents: simulation.world().len(),
        arrived,
        traveling,
        blocked,
        completion: arrived as f32 / project.agent_spawns().len() as f32,
        moving_agents: speeds.iter().filter(|speed| **speed > 0.05).count(),
        mean_speed_mps: speeds.iter().sum::<f32>() / speeds.len().max(1) as f32,
        mean_distance_to_destination_m: distances.iter().sum::<f32>()
            / distances.len().max(1) as f32,
        by_destination,
        simulation_duration_ns,
    })
}

pub fn bake_reference_strict(options: &StrictBakeOptions) -> Result<StrictBakeReport, String> {
    let project = reference_project()?;
    let scene = compile_concourse(&project).map_err(format_diagnostics)?;
    let tick_end = scene.duration_ticks.saturating_sub(1);
    let static_agents = cache_agents(project.agent_spawns());
    let unique_agent_ids = static_agents
        .iter()
        .map(|agent| agent.agent_id)
        .collect::<HashSet<_>>()
        .len();
    let static_digest = hex_hash(&static_digest_bytes(&static_agents));
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut writer = CacheWriter::create(&options.cache_path, bake_spec(&project, tick_end))
        .map_err(|error| error.to_string())?;
    writer
        .write_agents(&static_agents)
        .map_err(|error| error.to_string())?;

    let blocked_bounds: Vec<_> = project
        .ir()
        .semantics
        .blocked
        .iter()
        .map(|blocked| blocked.bounds)
        .collect();
    let mut previous_positions = vec![None; project.agent_spawns().len()];
    let close_event = project
        .ir()
        .portal_events
        .iter()
        .find(|event| !event.open)
        .ok_or_else(|| "reference project has no portal-close event".to_string())?;
    let reopen_event = project
        .ir()
        .portal_events
        .iter()
        .find(|event| event.open && event.portal_id == close_event.portal_id)
        .ok_or_else(|| "reference project has no matching portal-reopen event".to_string())?;
    let close_tick = close_event.tick;
    let reopen_tick = reopen_event.tick;
    let portal_id = close_event.portal_id.clone();
    let mut route_before = Vec::new();
    let mut affected_before = Vec::new();
    let mut selected_trace = None;
    let mut boundary_escapes = 0u64;
    let mut simulation_duration_ns = 0u64;
    let mut cache_write_duration_ns = 0u64;
    let mut invalidated_routes_at_close = 0usize;
    let mut unrelated_routes_unchanged_at_close = false;
    let mut recovered_by_tick = None;

    for cache_tick in 0..=tick_end {
        if simulation.clock().tick() == close_tick {
            let portals = simulation
                .nav()
                .ok_or_else(|| "reference scene has no navigation graph".to_string())?
                .portals_named(&portal_id)
                .to_vec();
            route_before = simulation.world().route.clone();
            affected_before = (0..simulation.world().len())
                .map(|slot| simulation.route_crosses_any(slot as u32, &portals))
                .collect();
            if let Some(slot) = affected_before.iter().position(|affected| *affected) {
                let id = simulation.world().agent_id[slot];
                selected_trace = Some(decision_trace(
                    &simulation,
                    project.ir(),
                    id,
                    cache_tick.saturating_sub(1),
                )?);
            }
        }
        let started = Instant::now();
        simulation.step();
        simulation_duration_ns = simulation_duration_ns.saturating_add(elapsed_ns(started));

        if cache_tick == close_tick {
            invalidated_routes_at_close = affected_before
                .iter()
                .copied()
                .enumerate()
                .filter(|(slot, affected)| {
                    *affected
                        && simulation.world().decision_reason[*slot]
                            == DecisionReason::PortalClosedReplan
                })
                .count();
            unrelated_routes_unchanged_at_close = affected_before
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, affected)| !*affected)
                .all(|(slot, _)| {
                    route_before[slot] == NO_ROUTE
                        || simulation.world().route[slot] == route_before[slot]
                });
        }
        if cache_tick >= reopen_tick
            && recovered_by_tick.is_none()
            && !affected_before.is_empty()
            && affected_before
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, affected)| *affected)
                .all(|(slot, _)| {
                    simulation.world().arrived[slot] || simulation.world().route[slot] != NO_ROUTE
                })
        {
            recovered_by_tick = Some(cache_tick);
        }

        boundary_escapes = boundary_escapes.saturating_add(scan_static_boundaries(
            &simulation,
            &blocked_bounds,
            &mut previous_positions,
        ));
        let frame = cache_frame(&simulation, project.agent_spawns());
        let started = Instant::now();
        writer
            .push_tick(cache_tick, frame)
            .map_err(|error| error.to_string())?;
        cache_write_duration_ns = cache_write_duration_ns.saturating_add(elapsed_ns(started));
    }
    let started = Instant::now();
    let manifest = writer.finish().map_err(|error| error.to_string())?;
    cache_write_duration_ns = cache_write_duration_ns.saturating_add(elapsed_ns(started));

    let portal_reroute = summarize_portal_reroute(
        &portal_id,
        close_tick,
        reopen_tick,
        &affected_before,
        invalidated_routes_at_close,
        unrelated_routes_unchanged_at_close,
        recovered_by_tick,
    );
    let selected_trace = selected_trace
        .ok_or_else(|| format!("no selected route used {portal_id} before its close event"))?;

    let debug_dir = options.cache_path.join("debug");
    fs::create_dir_all(&debug_dir).map_err(|error| error.to_string())?;
    let evidence_path = debug_dir.join("selected-agent.json");
    write_json(&evidence_path, &selected_trace)?;

    let agents_arrived = simulation
        .world()
        .arrived
        .iter()
        .filter(|arrived| **arrived)
        .count();
    let phase_timings = Phase::ALL
        .iter()
        .map(|phase| PhaseTiming {
            phase: phase.name().to_string(),
            nanoseconds: simulation.metrics().phase_nanos(*phase),
        })
        .collect();
    drop(simulation);

    let started = Instant::now();
    let reader =
        CacheReader::open_complete(&options.cache_path).map_err(|error| error.to_string())?;
    let frames = reader
        .read_all_frames()
        .map_err(|error| error.to_string())?;
    let sequential_cache_read_duration_ns = elapsed_ns(started);
    let discrete_digest = hex_hash(&discrete_digest_bytes(reader.agents(), &frames));
    let present_channels: HashSet<_> = manifest
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    let required_channels_missing = REQUIRED_FRAME_CHANNELS
        .iter()
        .filter(|name| !present_channels.contains(**name))
        .map(|name| (*name).to_string())
        .collect();
    let position_quantization_bound_m = manifest
        .channels
        .iter()
        .find(|channel| channel.name == "position")
        .and_then(|channel| channel.quantization_error)
        .unwrap_or(f32::INFINITY);

    let report = StrictBakeReport {
        schema_version: 1,
        cache_path: options.cache_path.display().to_string(),
        project_id: project.ir().project_id.clone(),
        source_hash: project.source_hash_hex(),
        tick_start: 0,
        tick_end,
        agent_count: project.agent_spawns().len() as u32,
        unique_agent_ids,
        static_digest,
        discrete_digest,
        destination_completion: agents_arrived as f32 / project.agent_spawns().len() as f32,
        agents_arrived,
        static_boundary_escapes: boundary_escapes,
        portal_reroute,
        required_channels_missing,
        position_quantization_bound_m,
        simulation_duration_ns,
        cache_write_duration_ns,
        sequential_cache_read_duration_ns,
        cache_size_bytes: directory_size(&options.cache_path)?,
        phase_timings,
        selected_agent_id: selected_trace.agent_id,
        selected_agent_tick: selected_trace.tick,
        selected_agent_evidence_path: evidence_path.display().to_string(),
    };
    write_json(&options.cache_path.join("metrics.json"), &report)?;
    Ok(report)
}

pub fn compare_strict_bakes(
    first: &StrictBakeReport,
    second: &StrictBakeReport,
) -> Result<StrictComparisonReport, String> {
    compare_cache_paths(Path::new(&first.cache_path), Path::new(&second.cache_path))
}

pub fn compare_cache_paths(
    first_path: &Path,
    second_path: &Path,
) -> Result<StrictComparisonReport, String> {
    let first = CacheReader::open_complete(first_path).map_err(|error| error.to_string())?;
    let second = CacheReader::open_complete(second_path).map_err(|error| error.to_string())?;
    let first_frames = first.read_all_frames().map_err(|error| error.to_string())?;
    let second_frames = second
        .read_all_frames()
        .map_err(|error| error.to_string())?;
    if first_frames.len() != second_frames.len() {
        return Err(format!(
            "cache frame counts differ: {} versus {}",
            first_frames.len(),
            second_frames.len()
        ));
    }

    let static_channels_equal = first.agents() == second.agents();
    let mut discrete_channels_equal = true;
    let mut continuous_channels_equal_except_position = true;
    let mut max_position_delta_m = 0.0f32;
    for (first_frame, second_frame) in first_frames.iter().zip(&second_frames) {
        if first_frame.records.len() != second_frame.records.len() {
            return Err("cache agent counts differ within a frame".to_string());
        }
        for (a, b) in first_frame.records.iter().zip(&second_frame.records) {
            discrete_channels_equal &= discrete_fields_equal(a, b);
            continuous_channels_equal_except_position &= continuous_fields_equal(a, b);
            for axis in 0..2 {
                max_position_delta_m =
                    max_position_delta_m.max((a.position[axis] - b.position[axis]).abs());
            }
        }
    }
    let declared_position_bound_m = first
        .manifest()
        .channels
        .iter()
        .chain(&second.manifest().channels)
        .filter(|channel| channel.name == "position")
        .filter_map(|channel| channel.quantization_error)
        .fold(0.0f32, f32::max);
    let accepted = static_channels_equal
        && discrete_channels_equal
        && continuous_channels_equal_except_position
        && max_position_delta_m <= declared_position_bound_m
        && declared_position_bound_m <= 0.001;
    Ok(StrictComparisonReport {
        first_cache: first_path.display().to_string(),
        second_cache: second_path.display().to_string(),
        static_channels_equal,
        discrete_channels_equal,
        continuous_channels_equal_except_position,
        max_position_delta_m,
        declared_position_bound_m,
        accepted,
    })
}

pub fn cancel_reference_bake(
    cache_path: &Path,
    cancel_after_ticks: u64,
) -> Result<CancellationReport, String> {
    let project = reference_project()?;
    let scene = compile_concourse(&project).map_err(format_diagnostics)?;
    let tick_end = scene.duration_ticks.saturating_sub(1);
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut writer = CacheWriter::create(cache_path, bake_spec(&project, tick_end))
        .map_err(|error| error.to_string())?;
    writer
        .write_agents(&cache_agents(project.agent_spawns()))
        .map_err(|error| error.to_string())?;
    for tick in 0..cancel_after_ticks.min(tick_end + 1) {
        simulation.step();
        writer
            .push_tick(tick, cache_frame(&simulation, project.agent_spawns()))
            .map_err(|error| error.to_string())?;
    }
    writer
        .cancel("M1 cancellation acceptance probe")
        .map_err(|error| error.to_string())?;
    let recovery = RecoveryInspector::open(cache_path).map_err(|error| error.to_string())?;
    Ok(CancellationReport {
        cache_path: cache_path.display().to_string(),
        status: cache_status_name(recovery.status).to_string(),
        last_complete_tick: recovery.last_complete_tick,
        valid_chunk_count: recovery.valid_chunk_count,
        complete_reader_rejected: CacheReader::open_complete(cache_path).is_err(),
    })
}

pub fn read_selected_trace(cache_path: &Path) -> Result<DecisionTraceV1, String> {
    let path = cache_path.join("debug/selected-agent.json");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn reference_project() -> Result<crowd_core::CompiledProject, String> {
    reference_project_with_agents(1_000)
}

fn reference_project_with_agents(agent_count: u32) -> Result<crowd_core::CompiledProject, String> {
    let mut ir: ProjectIrV1 =
        serde_json::from_str(REFERENCE_JSON).map_err(|error| error.to_string())?;
    ir.populations[0].count = agent_count;
    compile_project(&ir).map_err(format_diagnostics)
}

fn format_diagnostics(diagnostics: Vec<crowd_core::Diagnostic>) -> String {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            format!(
                "E_{:?} {}: {}",
                diagnostic.code, diagnostic.entity_id, diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn cache_agents(spawns: &[CompiledAgentSpawn]) -> Vec<AgentStatic> {
    spawns
        .iter()
        .map(|spawn| AgentStatic {
            agent_id: spawn.agent_id.0,
            population_id: spawn.population_id,
            archetype_id: spawn.archetype_id,
            variant_id: spawn.appearance_id,
            base_scale: spawn.scale,
            spawn_ordinal: spawn.spawn_ordinal,
        })
        .collect()
}

fn bake_spec(project: &crowd_core::CompiledProject, tick_end: u64) -> BakeSpec {
    BakeSpec {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        project_id: project.ir().project_id.clone(),
        source_hash: project.source_hash_hex(),
        tick_start: 0,
        tick_end,
        ticks_per_second: project.ir().clock.ticks_per_second,
        agent_count: project.agent_spawns().len() as u32,
        channels: cache_channels(),
        chunk_ticks: CACHE_V1_DEFAULTS.chunk_ticks,
        position_encoding: CACHE_V1_DEFAULTS.position_encoding,
    }
}

fn cache_channels() -> Vec<ChannelDef> {
    vec![
        channel("agent_id", ScalarType::U64, 1, None),
        channel("position", ScalarType::F32, 2, Some(0.0)),
        channel("orientation", ScalarType::F32, 1, None),
        channel("scale", ScalarType::F32, 1, None),
        channel("population_id", ScalarType::U32, 1, None),
        channel("variant_id", ScalarType::U32, 1, None),
        channel("clip_id", ScalarType::U16, 1, None),
        channel("phase", ScalarType::F32, 1, None),
        channel("playback_rate", ScalarType::F32, 1, None),
        channel("behavior_state", ScalarType::U16, 1, None),
        channel("decision_reason", ScalarType::U16, 1, None),
        channel("destination_id", ScalarType::U32, 1, None),
        channel("velocity", ScalarType::F32, 2, None),
        channel("visible", ScalarType::U8, 1, None),
        channel("render_tier", ScalarType::U8, 1, None),
    ]
}

fn channel(
    name: &str,
    scalar_type: ScalarType,
    arity: u8,
    quantization_error: Option<f32>,
) -> ChannelDef {
    ChannelDef {
        name: name.to_string(),
        scalar_type,
        arity,
        quantization_error,
    }
}

fn cache_frame(simulation: &Simulation, spawns: &[CompiledAgentSpawn]) -> Frame {
    Frame {
        records: spawns
            .iter()
            .map(|spawn| cache_record(simulation.query_agent(spawn.agent_id), spawn))
            .collect(),
    }
}

fn cache_record(snapshot: Option<AgentSnapshot>, spawn: &CompiledAgentSpawn) -> FrameRecord {
    let Some(snapshot) = snapshot else {
        return FrameRecord {
            agent_id: spawn.agent_id.0,
            scale: spawn.scale,
            population_id: spawn.population_id,
            variant_id: spawn.appearance_id,
            behavior_state: CommuterState::Unspawned as u16,
            decision_reason: DecisionReason::None as u16,
            destination_id: spawn.destination_id,
            playback_rate: 1.0,
            visible: false,
            ..FrameRecord::default()
        };
    };
    FrameRecord {
        agent_id: snapshot.agent_id.0,
        position: [snapshot.position.x, snapshot.position.y],
        orientation: snapshot.orientation,
        scale: snapshot.scale,
        population_id: snapshot.population_id,
        variant_id: snapshot.variant_id,
        clip_id: snapshot.clip_state.clip_id,
        phase: snapshot.clip_state.phase,
        playback_rate: snapshot.clip_state.playback_rate,
        behavior_state: snapshot.commuter_state as u16,
        decision_reason: snapshot.decision_reason as u16,
        destination_id: snapshot.destination_id,
        velocity: [snapshot.velocity.x, snapshot.velocity.y],
        visible: snapshot.visible,
        render_tier: snapshot.render_tier,
    }
}

fn decision_trace(
    simulation: &Simulation,
    ir: &ProjectIrV1,
    id: AgentId,
    tick: u64,
) -> Result<DecisionTraceV1, String> {
    let snapshot = simulation
        .query_agent(id)
        .ok_or_else(|| format!("selected agent {} is not spawned", id.0))?;
    let corridor_points = simulation
        .route_points_for_agent(id)
        .unwrap_or_default()
        .into_iter()
        .map(|point| [point.x, point.y])
        .collect();
    let next_target = simulation
        .next_route_target(id)
        .map(|point| [point.x, point.y]);
    let relevant_portals = ir
        .semantics
        .portals
        .iter()
        .map(|portal| {
            Ok(PortalStateEvidence {
                portal_id: portal.id.clone(),
                open: simulation
                    .named_portal_is_open(&portal.id)
                    .map_err(|error| format!("portal inspection failed: {error:?}"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(DecisionTraceV1 {
        schema_version: 1,
        agent_id: id.0,
        tick,
        position: [snapshot.position.x, snapshot.position.y],
        desired_velocity: [snapshot.desired_velocity.x, snapshot.desired_velocity.y],
        solved_velocity: [snapshot.velocity.x, snapshot.velocity.y],
        corridor_portal_ids: simulation.route_portal_ids(id).unwrap_or_default(),
        corridor_points,
        next_target,
        destination_id: snapshot.destination_id,
        path_status: if snapshot.commuter_state == CommuterState::Arrived {
            "arrived"
        } else if simulation
            .world()
            .slot_of(id)
            .is_some_and(|slot| simulation.world().route[slot as usize] == NO_ROUTE)
        {
            "unrouted"
        } else {
            "routed"
        }
        .to_string(),
        commuter_state_code: snapshot.commuter_state as u16,
        commuter_state: commuter_state_name(snapshot.commuter_state).to_string(),
        clip_id: snapshot.clip_state.clip_id,
        clip_phase: snapshot.clip_state.phase,
        playback_rate: snapshot.clip_state.playback_rate,
        relevant_portals,
        decision_code: snapshot.decision_reason as u16,
        decision_reason: snapshot.decision_reason.text().to_string(),
    })
}

fn summarize_portal_reroute(
    portal_id: &str,
    close_tick: u64,
    reopen_tick: u64,
    affected_before: &[bool],
    invalidated_routes: usize,
    unrelated_routes_unchanged: bool,
    recovered_by_tick: Option<u64>,
) -> PortalRerouteReport {
    let routes_using_portal_before_close = affected_before.iter().filter(|used| **used).count();
    let unrelated_routes_before_close = affected_before.len() - routes_using_portal_before_close;
    let all_invalidated_routes_recovered_by_reopen =
        recovered_by_tick.is_some_and(|tick| tick <= reopen_tick + 120);
    let accepted = routes_using_portal_before_close > 0
        && unrelated_routes_before_close > 0
        && invalidated_routes == routes_using_portal_before_close
        && unrelated_routes_unchanged
        && all_invalidated_routes_recovered_by_reopen;
    PortalRerouteReport {
        portal_id: portal_id.to_string(),
        close_tick,
        reopen_tick,
        routes_using_portal_before_close,
        unrelated_routes_before_close,
        invalidated_routes,
        unrelated_routes_unchanged,
        all_invalidated_routes_recovered_by_reopen,
        recovered_by_tick,
        accepted,
    }
}

fn scan_static_boundaries(
    simulation: &Simulation,
    blocked: &[crowd_core::project::Bounds2IrV1],
    previous: &mut [Option<Vec2>],
) -> u64 {
    let mut escapes = 0u64;
    for (slot, previous_position) in previous
        .iter_mut()
        .enumerate()
        .take(simulation.world().len())
    {
        let position = simulation.world().position(slot as u32);
        let outside = !simulation.scene().bounds.contains(position);
        let inside_blocked = blocked.iter().any(|bounds| {
            position.x > bounds.min[0]
                && position.x < bounds.max[0]
                && position.y > bounds.min[1]
                && position.y < bounds.max[1]
        });
        let crossed_wall = previous_position.is_some_and(|before| {
            simulation
                .scene()
                .walls
                .iter()
                .any(|wall| strictly_crosses(before, position, wall.a, wall.b))
        });
        if outside || inside_blocked || crossed_wall {
            escapes += 1;
        }
        *previous_position = Some(position);
    }
    escapes
}

fn strictly_crosses(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).x * (r - p).y - (q - p).y * (r - p).x;
    let ab_c = side(a, b, c);
    let ab_d = side(a, b, d);
    let cd_a = side(c, d, a);
    let cd_b = side(c, d, b);
    ab_c * ab_d < 0.0 && cd_a * cd_b < 0.0
}

fn static_digest_bytes(agents: &[AgentStatic]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(agents.len() * 28);
    for agent in agents {
        bytes.extend_from_slice(&agent.agent_id.to_le_bytes());
        bytes.extend_from_slice(&agent.population_id.to_le_bytes());
        bytes.extend_from_slice(&agent.archetype_id.to_le_bytes());
        bytes.extend_from_slice(&agent.variant_id.to_le_bytes());
        bytes.extend_from_slice(&agent.base_scale.to_bits().to_le_bytes());
        bytes.extend_from_slice(&agent.spawn_ordinal.to_le_bytes());
    }
    bytes
}

fn discrete_digest_bytes(agents: &[AgentStatic], frames: &[Frame]) -> Vec<u8> {
    let mut bytes = static_digest_bytes(agents);
    for frame in frames {
        for record in &frame.records {
            bytes.extend_from_slice(&record.agent_id.to_le_bytes());
            bytes.extend_from_slice(&record.population_id.to_le_bytes());
            bytes.extend_from_slice(&record.variant_id.to_le_bytes());
            bytes.extend_from_slice(&record.clip_id.to_le_bytes());
            bytes.extend_from_slice(&record.behavior_state.to_le_bytes());
            bytes.extend_from_slice(&record.decision_reason.to_le_bytes());
            bytes.extend_from_slice(&record.destination_id.to_le_bytes());
            bytes.push(u8::from(record.visible));
            bytes.push(record.render_tier);
        }
    }
    bytes
}

fn discrete_fields_equal(a: &FrameRecord, b: &FrameRecord) -> bool {
    a.agent_id == b.agent_id
        && a.population_id == b.population_id
        && a.variant_id == b.variant_id
        && a.clip_id == b.clip_id
        && a.behavior_state == b.behavior_state
        && a.decision_reason == b.decision_reason
        && a.destination_id == b.destination_id
        && a.visible == b.visible
        && a.render_tier == b.render_tier
}

fn continuous_fields_equal(a: &FrameRecord, b: &FrameRecord) -> bool {
    a.orientation.to_bits() == b.orientation.to_bits()
        && a.scale.to_bits() == b.scale.to_bits()
        && a.phase.to_bits() == b.phase.to_bits()
        && a.playback_rate.to_bits() == b.playback_rate.to_bits()
        && a.velocity[0].to_bits() == b.velocity[0].to_bits()
        && a.velocity[1].to_bits() == b.velocity[1].to_bits()
}

fn hex_hash(bytes: &[u8]) -> String {
    content_hash(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn commuter_state_name(state: CommuterState) -> &'static str {
    match state {
        CommuterState::Unspawned => "unspawned",
        CommuterState::Travel => "travel",
        CommuterState::Arrived => "arrived",
        CommuterState::Blocked => "blocked",
    }
}

fn cache_status_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Incomplete => "incomplete",
        CacheStatus::Canceled => "canceled",
        CacheStatus::Complete => "complete",
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, format!("{json}\n")).map_err(|error| error.to_string())
}

fn directory_size(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        total = total.saturating_add(if metadata.is_dir() {
            directory_size(&entry.path())?
        } else {
            metadata.len()
        });
    }
    Ok(total)
}
