//! Compile the checked project IR into the fixed M1 concourse runtime.

use std::collections::BTreeMap;

use crate::commuter::{
    ProjectRuntimeData, RuntimeAgentSpec, RuntimeAnimationSettings, TimedPortalInput,
};
use crate::geometry::Segment;
use crate::ids::{hash_combine, hash_str};
use crate::nav::{CrossingAxis, NavMeshDef};
use crate::project::{CompiledProject, Diagnostic, DiagnosticCode, PortalAxisIrV1, ProjectIrV1};
use crate::rng::{Purpose, StableRng};
use crate::route::WaypointGraph;
use crate::scene::{CompiledScene, Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

// The reference population is intentionally emitted as a sustained flow.
// Bursting hundreds of agents into each platform before the first commuters
// can clear creates an artificial packed-start deadlock that no doorway-width
// or duration increase fixes.
const AGENTS_PER_SPAWN_PER_TICK: u32 = 1;

pub fn compile_concourse(project: &CompiledProject) -> Result<CompiledScene, Vec<Diagnostic>> {
    let ir = project.ir();
    let bounds = project_bounds(ir);
    let mut walls = outer_walls(bounds);
    for portal in &ir.semantics.portals {
        append_portal_divider(
            &mut walls,
            bounds,
            Vec2::new(portal.center[0], portal.center[1]),
            portal.width_m,
            portal.axis,
        );
    }
    for blocked in &ir.semantics.blocked {
        walls.extend(rectangle_walls(to_aabb(blocked.bounds)));
    }

    let destinations: Vec<Destination> = ir
        .semantics
        .destinations
        .iter()
        .map(|destination| Destination {
            name: destination.id.clone(),
            node: 0,
        })
        .collect();
    let nav_destinations: Vec<Vec2> = ir
        .semantics
        .destinations
        .iter()
        .map(|destination| Vec2::new(destination.point[0], destination.point[1]))
        .collect();

    let mut specs_by_spawn = vec![Vec::new(); ir.semantics.spawns.len()];
    let stable_seed = hash_combine(ir.seed, hash_str(&ir.project_id));
    for agent in project.agent_spawns() {
        let Some(specs) = specs_by_spawn.get_mut(agent.spawn_source_id as usize) else {
            continue;
        };
        let capacity = directional_capacity(ir, agent);
        let mut destination_rng =
            StableRng::for_agent(stable_seed, agent.agent_id, Purpose::DestinationPosition);
        specs.push(RuntimeAgentSpec {
            agent_id: agent.agent_id,
            population_id: agent.population_id,
            spawn_ordinal: agent.spawn_ordinal,
            destination_id: agent.destination_id,
            destination_point: Vec2::new(
                destination_rng.range_f32(capacity.min[0], capacity.max[0]),
                destination_rng.range_f32(capacity.min[1], capacity.max[1]),
            ),
            destination_bounds: to_aabb(capacity),
            archetype_id: agent.archetype_id,
            variant_id: agent.appearance_id,
            radius_m: agent.radius_m,
            preferred_speed_mps: agent.preferred_speed_mps,
            scale: agent.scale,
        });
    }
    for specs in &mut specs_by_spawn {
        specs.sort_by_key(|spec| (spec.population_id, spec.spawn_ordinal, spec.agent_id));
    }

    let spawns: Vec<SpawnRegion> = ir
        .semantics
        .spawns
        .iter()
        .enumerate()
        .map(|(index, spawn)| {
            let specs = &specs_by_spawn[index];
            SpawnRegion {
                id: index as u16,
                population_id: specs.first().map_or(0, |spec| spec.population_id as u16),
                area: to_aabb(spawn.bounds),
                count: specs.len() as u32,
                per_tick: AGENTS_PER_SPAWN_PER_TICK,
                destination: specs.first().map_or(0, |spec| spec.destination_id as u16),
            }
        })
        .collect();

    let populations: Vec<PopulationParams> = ir
        .populations
        .iter()
        .map(|population| PopulationParams {
            radius_min: population.radius_m.min,
            radius_max: population.radius_m.max,
            speed_mean: population.preferred_speed_mps.mean,
            speed_stddev: population.preferred_speed_mps.stddev,
            max_speed_factor: 1.5,
        })
        .collect();
    let named_portals = ir
        .semantics
        .portals
        .iter()
        .map(|portal| {
            (
                portal.id.clone(),
                Vec2::new(portal.center[0], portal.center[1]),
                match portal.axis {
                    PortalAxisIrV1::EastWest => CrossingAxis::EastWest,
                    PortalAxisIrV1::NorthSouth => CrossingAxis::NorthSouth,
                },
            )
        })
        .collect();
    let duration_frames = i64::from(ir.clock.frame_end) - i64::from(ir.clock.frame_start) + 1;
    let duration_ticks = (duration_frames.max(1) as u64 * u64::from(ir.clock.ticks_per_second))
        .div_ceil(u64::from(ir.clock.frames_per_second));

    let scene = SceneDef {
        name: "m1_reference_concourse".to_string(),
        bounds,
        walls,
        waypoints: WaypointGraph::new(),
        destinations,
        spawns,
        populations,
        project_seed: stable_seed,
        ticks_per_second: ir.clock.ticks_per_second,
        duration_ticks,
        nav: Some(NavMeshDef {
            tile_size: ir.settings.navigation.tile_size_m,
            agent_radius: ir.settings.navigation.agent_radius_m,
            cost_areas: Vec::new(),
            named_portals,
        }),
        nav_destinations,
    };
    let mut compiled = scene.compile().map_err(|errors| {
        errors
            .into_iter()
            .enumerate()
            .map(|(ordinal, error)| {
                Diagnostic::error(
                    DiagnosticCode::SceneCompile,
                    format!("concourse:{ordinal:04}"),
                    format!("scene compilation failed: {error:?}"),
                )
            })
            .collect::<Vec<_>>()
    })?;

    let portal_indices: BTreeMap<&str, u32> = ir
        .semantics
        .portals
        .iter()
        .enumerate()
        .map(|(index, portal)| (portal.id.as_str(), index as u32))
        .collect();
    let mut timed_portal_events: Vec<TimedPortalInput> = ir
        .portal_events
        .iter()
        .enumerate()
        .map(|(authored_ordinal, event)| TimedPortalInput {
            tick: event.tick,
            portal_id: event.portal_id.clone(),
            portal_index: portal_indices[&event.portal_id.as_str()],
            authored_ordinal: authored_ordinal as u32,
            open: event.open,
        })
        .collect();
    timed_portal_events
        .sort_by_key(|event| (event.tick, event.portal_index, event.authored_ordinal));
    let initially_closed_portals = ir
        .semantics
        .portals
        .iter()
        .filter(|portal| !portal.initially_open)
        .map(|portal| portal.id.clone())
        .collect();
    compiled.attach_project_runtime(
        project.source_hash(),
        ProjectRuntimeData {
            agent_specs_by_spawn: specs_by_spawn,
            timed_portal_events,
            initially_closed_portals,
            spawn_interval_ticks: ir.populations[0].emission_interval_ticks,
            spawn_start_ticks: ir
                .semantics
                .spawns
                .iter()
                .map(|spawn| spawn.start_tick)
                .collect(),
            animation: RuntimeAnimationSettings {
                jog_threshold_mps: ir.settings.animation.jog_threshold_mps,
            },
        },
    );
    Ok(compiled)
}

fn directional_capacity(
    ir: &ProjectIrV1,
    agent: &crate::project::CompiledAgentSpawn,
) -> crate::project::Bounds2IrV1 {
    let spawn = &ir.semantics.spawns[agent.spawn_source_id as usize];
    let destination = &ir.semantics.destinations[agent.destination_id as usize];
    let mut capacity = destination.capacity_bounds;
    if spawn.walkable_id == destination.walkable_id {
        return capacity;
    }

    let spawn_center_x = (spawn.bounds.min[0] + spawn.bounds.max[0]) * 0.5;
    let lane_mid = (capacity.min[1] + capacity.max[1]) * 0.5;
    let lane_separator = 0.5;
    if spawn_center_x < destination.point[0] {
        // Eastbound commuters keep the south lane.
        capacity.max[1] = lane_mid - lane_separator;
    } else {
        // Westbound commuters keep the north lane.
        capacity.min[1] = lane_mid + lane_separator;
    }
    capacity
}

fn project_bounds(ir: &ProjectIrV1) -> Aabb {
    let first = ir
        .semantics
        .walkable
        .first()
        .expect("validated project has walkable geometry");
    let mut bounds = to_aabb(first.bounds);
    for walkable in &ir.semantics.walkable[1..] {
        let item = to_aabb(walkable.bounds);
        bounds.min.x = bounds.min.x.min(item.min.x);
        bounds.min.y = bounds.min.y.min(item.min.y);
        bounds.max.x = bounds.max.x.max(item.max.x);
        bounds.max.y = bounds.max.y.max(item.max.y);
    }
    bounds
}

fn to_aabb(bounds: crate::project::Bounds2IrV1) -> Aabb {
    Aabb::new(
        Vec2::new(bounds.min[0], bounds.min[1]),
        Vec2::new(bounds.max[0], bounds.max[1]),
    )
}

fn outer_walls(bounds: Aabb) -> Vec<Segment> {
    vec![
        Segment::new(bounds.min, Vec2::new(bounds.max.x, bounds.min.y)),
        Segment::new(Vec2::new(bounds.max.x, bounds.min.y), bounds.max),
        Segment::new(bounds.max, Vec2::new(bounds.min.x, bounds.max.y)),
        Segment::new(Vec2::new(bounds.min.x, bounds.max.y), bounds.min),
    ]
}

fn rectangle_walls(bounds: Aabb) -> Vec<Segment> {
    outer_walls(bounds)
}

fn append_portal_divider(
    walls: &mut Vec<Segment>,
    bounds: Aabb,
    center: Vec2,
    width: f32,
    axis: PortalAxisIrV1,
) {
    let half = width * 0.5;
    match axis {
        PortalAxisIrV1::EastWest => {
            walls.push(Segment::new(
                Vec2::new(center.x, bounds.min.y),
                Vec2::new(center.x, center.y - half),
            ));
            walls.push(Segment::new(
                Vec2::new(center.x, center.y + half),
                Vec2::new(center.x, bounds.max.y),
            ));
        }
        PortalAxisIrV1::NorthSouth => {
            walls.push(Segment::new(
                Vec2::new(bounds.min.x, center.y),
                Vec2::new(center.x - half, center.y),
            ));
            walls.push(Segment::new(
                Vec2::new(center.x + half, center.y),
                Vec2::new(bounds.max.x, center.y),
            ));
        }
    }
}
