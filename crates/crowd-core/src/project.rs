//! Versioned crowd-project authoring IR and deterministic compilation.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::ids::{derive_agent_id, hash_combine, hash_str, AgentId};
use crate::rng::{Purpose, StableRng};
use crate::scene::MIN_PREFERRED_SPEED;

pub const PROJECT_IR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectIrV1 {
    pub schema_version: u32,
    pub project_id: String,
    pub seed: u64,
    pub units: UnitsIrV1,
    pub clock: ClockIrV1,
    pub mode: DeterminismModeIrV1,
    pub commuter_program: String,
    pub archetypes: Vec<ArchetypeIrV1>,
    pub appearances: Vec<AppearanceIrV1>,
    pub populations: Vec<PopulationIrV1>,
    pub semantics: SemanticIrV1,
    pub portal_events: Vec<TimedPortalEventV1>,
    pub settings: ProjectSettingsIrV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitsIrV1 {
    pub length: String,
    pub angle: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockIrV1 {
    pub ticks_per_second: u32,
    pub frame_start: i32,
    pub frame_end: i32,
    pub frames_per_second: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismModeIrV1 {
    Strict,
    Fast,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchetypeIrV1 {
    pub id: String,
    pub asset_id: String,
    pub clip_set_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceIrV1 {
    pub id: String,
    pub material_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedRefIrV1 {
    pub id: String,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RangeIrV1 {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalIrV1 {
    pub mean: f32,
    pub stddev: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationIrV1 {
    pub id: String,
    pub count: u32,
    pub spawn_source_ids: Vec<String>,
    pub destinations: Vec<WeightedRefIrV1>,
    pub archetypes: Vec<WeightedRefIrV1>,
    pub appearances: Vec<WeightedRefIrV1>,
    pub radius_m: RangeIrV1,
    pub preferred_speed_mps: NormalIrV1,
    pub scale: RangeIrV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds2IrV1 {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkableIrV1 {
    pub id: String,
    pub bounds: Bounds2IrV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockedIrV1 {
    pub id: String,
    pub walkable_id: String,
    pub bounds: Bounds2IrV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnIrV1 {
    pub id: String,
    pub walkable_id: String,
    pub bounds: Bounds2IrV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestinationIrV1 {
    pub id: String,
    pub walkable_id: String,
    pub point: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortalAxisIrV1 {
    EastWest,
    NorthSouth,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortalIrV1 {
    pub id: String,
    pub from_walkable_id: String,
    pub to_walkable_id: String,
    pub center: [f32; 2],
    pub width_m: f32,
    pub axis: PortalAxisIrV1,
    pub bidirectional: bool,
    pub initially_open: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticIrV1 {
    pub walkable: Vec<WalkableIrV1>,
    pub blocked: Vec<BlockedIrV1>,
    pub spawns: Vec<SpawnIrV1>,
    pub destinations: Vec<DestinationIrV1>,
    pub portals: Vec<PortalIrV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedPortalEventV1 {
    pub tick: u64,
    pub portal_id: String,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSettingsIrV1 {
    pub navigation: NavigationSettingsIrV1,
    pub avoidance: AvoidanceSettingsIrV1,
    pub animation: AnimationSettingsIrV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationSettingsIrV1 {
    pub tile_size_m: f32,
    pub agent_radius_m: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvoidanceSettingsIrV1 {
    pub solver: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationSettingsIrV1 {
    pub locomotion_set_id: String,
    pub jog_threshold_mps: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticCode {
    UnsupportedVersion,
    InvalidProjectId,
    InvalidUnits,
    InvalidClock,
    DuplicateId,
    InvalidCount,
    InvalidRange,
    MissingSpawn,
    MissingDestination,
    MissingWalkable,
    MissingBlocked,
    MissingPortal,
    MissingArchetype,
    MissingAppearance,
    UnreachableDestination,
    InvalidWeights,
    ContradictoryPortalEvent,
    UnsupportedProgram,
    UnsupportedSetting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub entity_id: String,
    pub message: String,
}

impl Diagnostic {
    fn error(
        code: DiagnosticCode,
        entity_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code,
            entity_id: entity_id.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledAgentSpawn {
    pub agent_id: AgentId,
    pub population_id: u32,
    pub spawn_source_id: u32,
    pub spawn_ordinal: u32,
    pub destination_id: u32,
    pub archetype_id: u32,
    pub appearance_id: u32,
    pub radius_m: f32,
    pub preferred_speed_mps: f32,
    pub scale: f32,
}

#[derive(Clone, Debug)]
pub struct CompiledProject {
    ir: ProjectIrV1,
    source_hash: [u8; 32],
    agent_spawns: Vec<CompiledAgentSpawn>,
    population_indices: BTreeMap<String, u32>,
    spawn_indices: BTreeMap<String, u32>,
    destination_indices: BTreeMap<String, u32>,
    archetype_indices: BTreeMap<String, u32>,
    appearance_indices: BTreeMap<String, u32>,
}

impl CompiledProject {
    pub fn ir(&self) -> &ProjectIrV1 {
        &self.ir
    }

    pub fn source_hash(&self) -> [u8; 32] {
        self.source_hash
    }

    pub fn source_hash_hex(&self) -> String {
        self.source_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn agent_spawns(&self) -> &[CompiledAgentSpawn] {
        &self.agent_spawns
    }

    pub fn population_index(&self, id: &str) -> Option<u32> {
        self.population_indices.get(id).copied()
    }

    pub fn spawn_index(&self, id: &str) -> Option<u32> {
        self.spawn_indices.get(id).copied()
    }

    pub fn destination_index(&self, id: &str) -> Option<u32> {
        self.destination_indices.get(id).copied()
    }

    pub fn archetype_index(&self, id: &str) -> Option<u32> {
        self.archetype_indices.get(id).copied()
    }

    pub fn appearance_index(&self, id: &str) -> Option<u32> {
        self.appearance_indices.get(id).copied()
    }
}

pub fn compile_project(project: &ProjectIrV1) -> Result<CompiledProject, Vec<Diagnostic>> {
    let mut diagnostics = validate_project(project);
    sort_diagnostics(&mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let ir = normalized_project(project);
    let canonical = serde_json::to_vec(&ir).expect("validated project serializes");
    let source_hash = *blake3::hash(&canonical).as_bytes();
    let population_indices = compact_indices(ir.populations.iter().map(|item| item.id.as_str()));
    let spawn_indices = compact_indices(ir.semantics.spawns.iter().map(|item| item.id.as_str()));
    let destination_indices = compact_indices(
        ir.semantics
            .destinations
            .iter()
            .map(|item| item.id.as_str()),
    );
    let archetype_indices = compact_indices(ir.archetypes.iter().map(|item| item.id.as_str()));
    let appearance_indices = compact_indices(ir.appearances.iter().map(|item| item.id.as_str()));
    let stable_seed = hash_combine(ir.seed, hash_str(&ir.project_id));
    let mut agent_spawns = Vec::with_capacity(
        ir.populations
            .iter()
            .map(|population| population.count as usize)
            .sum(),
    );

    for population in &ir.populations {
        let population_index = population_indices[&population.id];
        for ordinal in 0..population.count {
            let source_count = population.spawn_source_ids.len();
            let spawn_ref = &population.spawn_source_ids[ordinal as usize % source_count];
            let spawn_index = spawn_indices[spawn_ref];
            let spawn_ordinal = ordinal / source_count as u32;
            let agent_id = derive_agent_id(
                stable_seed,
                population_index as u16,
                spawn_index as u16,
                spawn_ordinal,
            );
            let destination = choose_weighted(
                stable_seed,
                agent_id,
                Purpose::DestinationChoice,
                &population.destinations,
            );
            let archetype = choose_weighted(
                stable_seed,
                agent_id,
                Purpose::ArchetypeChoice,
                &population.archetypes,
            );
            let appearance = choose_weighted(
                stable_seed,
                agent_id,
                Purpose::AppearanceChoice,
                &population.appearances,
            );
            let mut radius_rng = StableRng::for_agent(stable_seed, agent_id, Purpose::Radius);
            let mut speed_rng =
                StableRng::for_agent(stable_seed, agent_id, Purpose::PreferredSpeed);
            let mut scale_rng = StableRng::for_agent(stable_seed, agent_id, Purpose::Scale);
            agent_spawns.push(CompiledAgentSpawn {
                agent_id,
                population_id: population_index,
                spawn_source_id: spawn_index,
                spawn_ordinal,
                destination_id: destination_indices[&destination.id],
                archetype_id: archetype_indices[&archetype.id],
                appearance_id: appearance_indices[&appearance.id],
                radius_m: radius_rng.range_f32(population.radius_m.min, population.radius_m.max),
                preferred_speed_mps: speed_rng
                    .normal_f32(
                        population.preferred_speed_mps.mean,
                        population.preferred_speed_mps.stddev,
                    )
                    .clamp(MIN_PREFERRED_SPEED, 4.0),
                scale: scale_rng.range_f32(population.scale.min, population.scale.max),
            });
        }
    }

    Ok(CompiledProject {
        ir,
        source_hash,
        agent_spawns,
        population_indices,
        spawn_indices,
        destination_indices,
        archetype_indices,
        appearance_indices,
    })
}

pub fn canonical_project_json(project: &ProjectIrV1) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = validate_project(project);
    sort_diagnostics(&mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(serde_json::to_string(&normalized_project(project))
        .expect("validated project serializes to canonical JSON"))
}

fn validate_project(project: &ProjectIrV1) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let project_entity = format!("project:{}", project.project_id);
    if project.schema_version != PROJECT_IR_SCHEMA_VERSION {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnsupportedVersion,
            &project_entity,
            format!("unsupported project schema {}", project.schema_version),
        ));
    }
    if !valid_uuid(&project.project_id) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidProjectId,
            &project_entity,
            "project_id must be a canonical hyphenated UUID",
        ));
    }
    if project.units.length != "meters" || project.units.angle != "radians" {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidUnits,
            &project_entity,
            "units must be meters and radians",
        ));
    }
    if project.clock.ticks_per_second == 0
        || project.clock.frames_per_second == 0
        || project.clock.frame_end < project.clock.frame_start
    {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidClock,
            &project_entity,
            "clock rate and inclusive frame range must be valid",
        ));
    }
    if project.commuter_program != "commuter_v1" {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnsupportedProgram,
            &project_entity,
            "M1 supports commuter_v1 only",
        ));
    }
    if project.settings.avoidance.solver != "sampled_velocity"
        || !positive(project.settings.navigation.tile_size_m)
        || !positive(project.settings.navigation.agent_radius_m)
        || !positive(project.settings.animation.jog_threshold_mps)
        || project.settings.animation.locomotion_set_id.is_empty()
    {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::UnsupportedSetting,
            &project_entity,
            "navigation, sampled-velocity avoidance, and animation settings are invalid",
        ));
    }

    duplicate_ids(
        "archetype",
        project.archetypes.iter().map(|item| item.id.as_str()),
        &mut diagnostics,
    );
    duplicate_ids(
        "appearance",
        project.appearances.iter().map(|item| item.id.as_str()),
        &mut diagnostics,
    );
    duplicate_ids(
        "population",
        project.populations.iter().map(|item| item.id.as_str()),
        &mut diagnostics,
    );
    validate_semantics(project, &mut diagnostics);

    let spawn_ids: BTreeSet<&str> = project
        .semantics
        .spawns
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let destination_ids: BTreeSet<&str> = project
        .semantics
        .destinations
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let archetype_ids: BTreeSet<&str> = project
        .archetypes
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let appearance_ids: BTreeSet<&str> = project
        .appearances
        .iter()
        .map(|item| item.id.as_str())
        .collect();

    for population in &project.populations {
        let entity = format!("population:{}", population.id);
        if population.count == 0 {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidCount,
                &entity,
                "population count must be positive",
            ));
        }
        if population.spawn_source_ids.is_empty()
            || population
                .spawn_source_ids
                .iter()
                .any(|id| !spawn_ids.contains(id.as_str()))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingSpawn,
                &entity,
                "population references a missing spawn source",
            ));
        }
        if population.destinations.is_empty()
            || population
                .destinations
                .iter()
                .any(|item| !destination_ids.contains(item.id.as_str()))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingDestination,
                &entity,
                "population references a missing destination",
            ));
        }
        if !valid_weights(&population.archetypes) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidWeights,
                &entity,
                "archetype weights must have a positive finite total",
            ));
        } else if population
            .archetypes
            .iter()
            .any(|item| !archetype_ids.contains(item.id.as_str()))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingArchetype,
                &entity,
                "population references a missing archetype",
            ));
        }
        if !valid_weights(&population.appearances) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidWeights,
                &entity,
                "appearance weights must have a positive finite total",
            ));
        } else if population
            .appearances
            .iter()
            .any(|item| !appearance_ids.contains(item.id.as_str()))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingAppearance,
                &entity,
                "population references a missing appearance",
            ));
        }
        if !valid_weights(&population.destinations) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidWeights,
                &entity,
                "destination weights must have a positive finite total",
            ));
        }
        if !valid_range(population.radius_m, true)
            || population.preferred_speed_mps.mean < MIN_PREFERRED_SPEED
            || !population.preferred_speed_mps.mean.is_finite()
            || !population.preferred_speed_mps.stddev.is_finite()
            || population.preferred_speed_mps.stddev < 0.0
            || !valid_range(population.scale, true)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidRange,
                &entity,
                "radius, speed, or scale distribution is invalid",
            ));
        }
    }

    validate_reachability(project, &mut diagnostics);
    diagnostics
}

fn validate_semantics(project: &ProjectIrV1, diagnostics: &mut Vec<Diagnostic>) {
    let semantics = &project.semantics;
    let mut semantic_ids = BTreeSet::new();
    for (kind, id) in semantics
        .walkable
        .iter()
        .map(|item| ("walkable", item.id.as_str()))
        .chain(
            semantics
                .blocked
                .iter()
                .map(|item| ("blocked", item.id.as_str())),
        )
        .chain(
            semantics
                .spawns
                .iter()
                .map(|item| ("spawn", item.id.as_str())),
        )
        .chain(
            semantics
                .destinations
                .iter()
                .map(|item| ("destination", item.id.as_str())),
        )
        .chain(
            semantics
                .portals
                .iter()
                .map(|item| ("portal", item.id.as_str())),
        )
    {
        if id.is_empty() || !semantic_ids.insert(id) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateId,
                format!("{kind}:{id}"),
                "semantic IDs must be non-empty and globally unique",
            ));
        }
    }
    if semantics.walkable.is_empty() {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::MissingWalkable,
            format!("project:{}", project.project_id),
            "project has no walkable domain",
        ));
    }

    let walkable_ids: BTreeSet<&str> = semantics
        .walkable
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let walkable_bounds: BTreeMap<&str, Bounds2IrV1> = semantics
        .walkable
        .iter()
        .map(|item| (item.id.as_str(), item.bounds))
        .collect();
    for walkable in &semantics.walkable {
        validate_bounds(
            &format!("walkable:{}", walkable.id),
            walkable.bounds,
            diagnostics,
        );
    }
    for blocked in &semantics.blocked {
        validate_bounds(
            &format!("blocked:{}", blocked.id),
            blocked.bounds,
            diagnostics,
        );
        missing_walkable(
            &format!("blocked:{}", blocked.id),
            &blocked.walkable_id,
            &walkable_ids,
            diagnostics,
        );
        if walkable_bounds
            .get(blocked.walkable_id.as_str())
            .is_some_and(|walkable| !contains_bounds(*walkable, blocked.bounds))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidRange,
                format!("blocked:{}", blocked.id),
                "blocked bounds must stay inside their walkable region",
            ));
        }
    }
    for spawn in &semantics.spawns {
        validate_bounds(&format!("spawn:{}", spawn.id), spawn.bounds, diagnostics);
        missing_walkable(
            &format!("spawn:{}", spawn.id),
            &spawn.walkable_id,
            &walkable_ids,
            diagnostics,
        );
        if walkable_bounds
            .get(spawn.walkable_id.as_str())
            .is_some_and(|walkable| !contains_bounds(*walkable, spawn.bounds))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidRange,
                format!("spawn:{}", spawn.id),
                "spawn bounds must stay inside their walkable region",
            ));
        }
    }
    for destination in &semantics.destinations {
        if !finite_point(destination.point) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidRange,
                format!("destination:{}", destination.id),
                "destination point must be finite",
            ));
        }
        missing_walkable(
            &format!("destination:{}", destination.id),
            &destination.walkable_id,
            &walkable_ids,
            diagnostics,
        );
        if walkable_bounds
            .get(destination.walkable_id.as_str())
            .is_some_and(|walkable| !contains_point(*walkable, destination.point))
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidRange,
                format!("destination:{}", destination.id),
                "destination must stay inside its walkable region",
            ));
        }
    }
    for portal in &semantics.portals {
        let entity = format!("portal:{}", portal.id);
        if !finite_point(portal.center) || !positive(portal.width_m) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidRange,
                &entity,
                "portal center and width must be valid",
            ));
        }
        for walkable_id in [&portal.from_walkable_id, &portal.to_walkable_id] {
            missing_walkable(&entity, walkable_id, &walkable_ids, diagnostics);
        }
    }

    let portal_ids: BTreeSet<&str> = semantics
        .portals
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let mut event_states = BTreeMap::new();
    for event in &project.portal_events {
        let entity = format!("portal_event:{}@{}", event.portal_id, event.tick);
        if !portal_ids.contains(event.portal_id.as_str()) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::MissingPortal,
                &entity,
                "portal event references a missing portal",
            ));
        }
        let key = (event.tick, event.portal_id.as_str());
        if let Some(previous) = event_states.insert(key, event.open) {
            if previous != event.open {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::ContradictoryPortalEvent,
                    &entity,
                    "portal has contradictory states at one tick",
                ));
            }
        }
    }
}

fn validate_reachability(project: &ProjectIrV1, diagnostics: &mut Vec<Diagnostic>) {
    let region_indices = compact_indices(
        project
            .semantics
            .walkable
            .iter()
            .map(|item| item.id.as_str()),
    );
    let mut adjacency = vec![Vec::new(); region_indices.len()];
    for portal in &project.semantics.portals {
        if !portal.initially_open {
            continue;
        }
        let (Some(&from), Some(&to)) = (
            region_indices.get(&portal.from_walkable_id),
            region_indices.get(&portal.to_walkable_id),
        ) else {
            continue;
        };
        adjacency[from as usize].push(to);
        if portal.bidirectional {
            adjacency[to as usize].push(from);
        }
    }
    let spawn_regions: BTreeMap<&str, &str> = project
        .semantics
        .spawns
        .iter()
        .map(|item| (item.id.as_str(), item.walkable_id.as_str()))
        .collect();
    let destination_regions: BTreeMap<&str, &str> = project
        .semantics
        .destinations
        .iter()
        .map(|item| (item.id.as_str(), item.walkable_id.as_str()))
        .collect();

    for population in &project.populations {
        let pairs = population.spawn_source_ids.iter().flat_map(|spawn| {
            population
                .destinations
                .iter()
                .map(move |destination| (spawn.as_str(), destination.id.as_str()))
        });
        let mut unreachable = false;
        for (spawn, destination) in pairs {
            let Some(spawn_region) = spawn_regions.get(spawn) else {
                continue;
            };
            let Some(destination_region) = destination_regions.get(destination) else {
                continue;
            };
            let (Some(&from), Some(&to)) = (
                region_indices.get(*spawn_region),
                region_indices.get(*destination_region),
            ) else {
                continue;
            };
            if !reachable(&adjacency, from, to) {
                unreachable = true;
                break;
            }
        }
        if unreachable {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::UnreachableDestination,
                format!("population:{}", population.id),
                "an assigned destination is unreachable from an assigned spawn",
            ));
        }
    }
}

fn reachable(adjacency: &[Vec<u32>], from: u32, to: u32) -> bool {
    let mut queue = VecDeque::from([from]);
    let mut visited = vec![false; adjacency.len()];
    while let Some(node) = queue.pop_front() {
        if node == to {
            return true;
        }
        if visited[node as usize] {
            continue;
        }
        visited[node as usize] = true;
        queue.extend(
            adjacency[node as usize]
                .iter()
                .copied()
                .filter(|next| !visited[*next as usize]),
        );
    }
    false
}

fn normalized_project(project: &ProjectIrV1) -> ProjectIrV1 {
    let mut project = project.clone();
    project.archetypes.sort_by(|a, b| a.id.cmp(&b.id));
    project.appearances.sort_by(|a, b| a.id.cmp(&b.id));
    project.populations.sort_by(|a, b| a.id.cmp(&b.id));
    for population in &mut project.populations {
        population.spawn_source_ids.sort();
        population.spawn_source_ids.dedup();
        population.archetypes.sort_by(|a, b| a.id.cmp(&b.id));
        population.appearances.sort_by(|a, b| a.id.cmp(&b.id));
        population.destinations.sort_by(|a, b| a.id.cmp(&b.id));
    }
    project.semantics.walkable.sort_by(|a, b| a.id.cmp(&b.id));
    project.semantics.blocked.sort_by(|a, b| a.id.cmp(&b.id));
    project.semantics.spawns.sort_by(|a, b| a.id.cmp(&b.id));
    project
        .semantics
        .destinations
        .sort_by(|a, b| a.id.cmp(&b.id));
    project.semantics.portals.sort_by(|a, b| a.id.cmp(&b.id));
    project.portal_events.sort_by(|a, b| {
        (a.tick, a.portal_id.as_str(), a.open).cmp(&(b.tick, b.portal_id.as_str(), b.open))
    });
    project
}

fn choose_weighted(
    seed: u64,
    agent_id: AgentId,
    purpose: Purpose,
    choices: &[WeightedRefIrV1],
) -> &WeightedRefIrV1 {
    let total: f32 = choices.iter().map(|choice| choice.weight).sum();
    let mut rng = StableRng::for_agent(seed, agent_id, purpose);
    let mut threshold = rng.next_f32_unit() * total;
    for choice in choices {
        if threshold < choice.weight {
            return choice;
        }
        threshold -= choice.weight;
    }
    choices.last().expect("validated choices are non-empty")
}

fn compact_indices<'a>(ids: impl Iterator<Item = &'a str>) -> BTreeMap<String, u32> {
    let mut ids: Vec<&str> = ids.collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .enumerate()
        .map(|(index, id)| (id.to_string(), index as u32))
        .collect()
}

fn duplicate_ids<'a>(
    kind: &str,
    ids: impl Iterator<Item = &'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.is_empty() || !seen.insert(id) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateId,
                format!("{kind}:{id}"),
                format!("{kind} IDs must be non-empty and unique"),
            ));
        }
    }
}

fn valid_weights(choices: &[WeightedRefIrV1]) -> bool {
    let total = choices.iter().map(|choice| choice.weight).sum::<f32>();
    !choices.is_empty()
        && total.is_finite()
        && total > 0.0
        && choices
            .iter()
            .all(|choice| choice.weight.is_finite() && choice.weight >= 0.0)
}

fn valid_range(range: RangeIrV1, positive_only: bool) -> bool {
    range.min.is_finite()
        && range.max.is_finite()
        && range.max >= range.min
        && (!positive_only || range.min > 0.0)
}

fn validate_bounds(entity: &str, bounds: Bounds2IrV1, diagnostics: &mut Vec<Diagnostic>) {
    if !finite_point(bounds.min)
        || !finite_point(bounds.max)
        || bounds.max[0] <= bounds.min[0]
        || bounds.max[1] <= bounds.min[1]
    {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::InvalidRange,
            entity,
            "bounds must be finite and have positive area",
        ));
    }
}

fn missing_walkable(
    entity: &str,
    walkable_id: &str,
    walkable_ids: &BTreeSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !walkable_ids.contains(walkable_id) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::MissingWalkable,
            entity,
            format!("missing walkable region {walkable_id}"),
        ));
    }
}

fn finite_point(point: [f32; 2]) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

fn contains_point(bounds: Bounds2IrV1, point: [f32; 2]) -> bool {
    point[0] >= bounds.min[0]
        && point[0] <= bounds.max[0]
        && point[1] >= bounds.min[1]
        && point[1] <= bounds.max[1]
}

fn contains_bounds(outer: Bounds2IrV1, inner: Bounds2IrV1) -> bool {
    contains_point(outer, inner.min) && contains_point(outer, inner.max)
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn sort_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.sort_by(|a, b| {
        (&a.severity, &a.code, &a.entity_id, &a.message).cmp(&(
            &b.severity,
            &b.code,
            &b.entity_id,
            &b.message,
        ))
    });
    diagnostics.dedup();
}
