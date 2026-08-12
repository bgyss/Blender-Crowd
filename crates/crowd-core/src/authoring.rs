//! M2 authorable-project IR layered over the accepted Project IR v1.
//!
//! The v1 payload remains byte-for-byte canonical for M1 cache provenance.
//! M2 data is compiled beside it so migration cannot silently change a proven
//! simulation input.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::assets::{validate_asset_library, AssetLibraryV1, CompiledAssetLibrary};
use crate::behavior::{compile_graph, BehaviorGraphV1, BehaviorNodeV1, BehaviorProgram};
use crate::project::{compile_project, Bounds2IrV1, CompiledProject, ProjectIrV1};
use crate::runtime_behavior::{RuntimeBehaviorController, RuntimeGroup, RuntimeQueue};
use crate::social::{GroupConstraint, QueueRuntime};
use crate::units::Vec2;

pub const AUTHORABLE_PROJECT_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorableProjectV2 {
    pub schema_version: u32,
    pub base: ProjectIrV1,
    pub behavior_graphs: Vec<BehaviorGraphV1>,
    pub population_behaviors: Vec<PopulationBehaviorV2>,
    pub semantics: AuthorableSemanticsV2,
    pub groups: Vec<GroupV2>,
    pub assets: AssetLibraryV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationBehaviorV2 {
    pub population_id: String,
    pub graph_id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorableSemanticsV2 {
    pub queues: Vec<QueueV2>,
    pub lanes: Vec<LaneV2>,
    pub cost_regions: Vec<CostRegionV2>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueV2 {
    pub id: String,
    pub portal_id: String,
    pub slots: Vec<[f32; 2]>,
    pub admission_capacity: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneV2 {
    pub id: String,
    pub points: Vec<[f32; 2]>,
    pub strength_millionths: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostRegionKindV2 {
    Interest,
    Danger,
    Preferred,
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostRegionV2 {
    pub id: String,
    pub walkable_id: String,
    pub bounds: Bounds2IrV1,
    pub kind: CostRegionKindV2,
    pub weight_millionths: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKindV2 {
    Couple,
    Family,
    LeaderFollower,
}

/// How a social group traverses a capacity-constrained authored queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBottleneckPolicyV2 {
    /// Preserve the pre-M2 per-agent queue behavior.
    #[default]
    Individual,
    /// Admit the declared leader, then each remaining member in stable ID order.
    LeaderFirst,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupV2 {
    pub id: String,
    pub kind: GroupKindV2,
    pub member_agent_ids: Vec<u64>,
    pub leader_agent_id: Option<u64>,
    pub shared_destination_id: String,
    pub max_separation_millimeters: u32,
    #[serde(default)]
    pub bottleneck_policy: GroupBottleneckPolicyV2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthoringDiagnosticCode {
    UnsupportedVersion,
    BaseProject,
    DuplicateId,
    InvalidGraph,
    MissingBehavior,
    MissingSemanticReference,
    InvalidSemanticGeometry,
    UnknownAgent,
    InvalidGroupLeader,
    InvalidGroup,
    InvalidAsset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoringDiagnostic {
    pub code: AuthoringDiagnosticCode,
    pub entity_id: String,
    pub message: String,
}

impl AuthoringDiagnostic {
    fn error(
        code: AuthoringDiagnosticCode,
        entity_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            entity_id: entity_id.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct CompiledAuthorableProject {
    base: CompiledProject,
    programs: BTreeMap<String, BehaviorProgram>,
    population_graphs: BTreeMap<String, String>,
    assets: CompiledAssetLibrary,
    queues: Vec<QueueV2>,
    groups: Vec<GroupV2>,
}

impl CompiledAuthorableProject {
    pub fn base(&self) -> &CompiledProject {
        &self.base
    }

    pub fn behavior_program(&self, id: &str) -> Option<&BehaviorProgram> {
        self.programs.get(id)
    }

    pub fn population_graph(&self, population_id: &str) -> Option<&str> {
        self.population_graphs
            .get(population_id)
            .map(String::as_str)
    }

    pub fn behavior_program_count(&self) -> usize {
        self.programs.len()
    }

    pub fn assets(&self) -> &CompiledAssetLibrary {
        &self.assets
    }

    pub fn runtime_controller(&self) -> RuntimeBehaviorController {
        let by_population = self
            .population_graphs
            .iter()
            .filter_map(|(population, graph)| {
                let index = self.base.population_index(population)? as u16;
                let program = self.programs.get(graph)?.clone();
                Some((
                    index,
                    crate::behavior::BehaviorVm::new(program, self.base.ir().seed),
                ))
            })
            .collect();
        let graph_ids = self
            .population_graphs
            .iter()
            .filter_map(|(population, graph)| {
                self.base
                    .population_index(population)
                    .and_then(|index| u16::try_from(index).ok())
                    .map(|index| (index, graph.clone()))
            })
            .collect();
        let destination_indices = self
            .base
            .ir()
            .semantics
            .destinations
            .iter()
            .filter_map(|item| {
                self.base
                    .destination_index(&item.id)
                    .and_then(|index| u16::try_from(index).ok())
                    .map(|index| (item.id.clone(), index))
            })
            .collect();
        let queues = self
            .queues
            .iter()
            .filter_map(|queue| {
                QueueRuntime::new(
                    queue.id.clone(),
                    queue.slots.len(),
                    queue.admission_capacity as usize,
                )
                .ok()
                .map(|runtime| {
                    (
                        queue.id.clone(),
                        RuntimeQueue {
                            runtime,
                            slots: queue
                                .slots
                                .iter()
                                .map(|point| Vec2::new(point[0], point[1]))
                                .collect(),
                        },
                    )
                })
            })
            .collect();
        let groups = self
            .groups
            .iter()
            .filter_map(|group| {
                let leader = group
                    .leader_agent_id
                    .or_else(|| group.member_agent_ids.iter().copied().min())?;
                GroupConstraint::new(
                    group.id.clone(),
                    group
                        .member_agent_ids
                        .iter()
                        .copied()
                        .map(crate::ids::AgentId)
                        .collect(),
                    crate::ids::AgentId(leader),
                    group.max_separation_millimeters as f32 / 1000.0,
                    1.0,
                )
                .ok()
                .map(|constraint| RuntimeGroup {
                    constraint,
                    bottleneck_policy: group.bottleneck_policy,
                    shared_destination: self
                        .base
                        .destination_index(&group.shared_destination_id)
                        .and_then(|index| u16::try_from(index).ok()),
                })
            })
            .collect();
        RuntimeBehaviorController::new(by_population, destination_indices)
            .with_graph_ids(graph_ids)
            .with_social(queues, groups)
    }
}

pub fn migrate_project_v1(base: ProjectIrV1) -> AuthorableProjectV2 {
    let population_behaviors = base
        .populations
        .iter()
        .map(|population| PopulationBehaviorV2 {
            population_id: population.id.clone(),
            graph_id: "commuter_v1".to_string(),
        })
        .collect();
    AuthorableProjectV2 {
        schema_version: AUTHORABLE_PROJECT_SCHEMA_VERSION,
        base,
        behavior_graphs: vec![BehaviorGraphV1 {
            id: "commuter_v1".to_string(),
            entry_id: "assigned_destination".to_string(),
            nodes: vec![BehaviorNodeV1::Navigate {
                id: "assigned_destination".to_string(),
                destination_id: "__assigned_destination".to_string(),
            }],
        }],
        population_behaviors,
        semantics: AuthorableSemanticsV2::default(),
        groups: Vec::new(),
        assets: AssetLibraryV1::default(),
    }
}

pub fn compile_authorable_project(
    project: &AuthorableProjectV2,
) -> Result<CompiledAuthorableProject, Vec<AuthoringDiagnostic>> {
    let mut diagnostics = Vec::new();
    if project.schema_version != AUTHORABLE_PROJECT_SCHEMA_VERSION {
        diagnostics.push(AuthoringDiagnostic::error(
            AuthoringDiagnosticCode::UnsupportedVersion,
            "project",
            format!(
                "migrate schema {} to authorable schema {}",
                project.schema_version, AUTHORABLE_PROJECT_SCHEMA_VERSION
            ),
        ));
    }
    let base = match compile_project(&project.base) {
        Ok(compiled) => compiled,
        Err(errors) => {
            diagnostics.extend(errors.into_iter().map(|error| {
                AuthoringDiagnostic::error(
                    AuthoringDiagnosticCode::BaseProject,
                    error.entity_id,
                    error.message,
                )
            }));
            sort_diagnostics(&mut diagnostics);
            return Err(diagnostics);
        }
    };

    let mut programs = BTreeMap::new();
    for graph in &project.behavior_graphs {
        match compile_graph(graph) {
            Ok(program) => {
                if programs.insert(graph.id.clone(), program).is_some() {
                    diagnostics.push(AuthoringDiagnostic::error(
                        AuthoringDiagnosticCode::DuplicateId,
                        format!("graph:{}", graph.id),
                        "rename the duplicate behavior graph",
                    ));
                }
            }
            Err(errors) => diagnostics.extend(errors.into_iter().map(|error| {
                AuthoringDiagnostic::error(
                    AuthoringDiagnosticCode::InvalidGraph,
                    format!("graph:{}/node:{}", graph.id, error.node_id),
                    error.message,
                )
            })),
        }
    }

    let population_ids: BTreeSet<_> = project
        .base
        .populations
        .iter()
        .map(|population| population.id.as_str())
        .collect();
    let mut population_graphs = BTreeMap::new();
    for assignment in &project.population_behaviors {
        let entity = format!("population:{}", assignment.population_id);
        if !population_ids.contains(assignment.population_id.as_str())
            || !programs.contains_key(&assignment.graph_id)
        {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::MissingBehavior,
                entity,
                "choose an existing population and behavior graph",
            ));
        } else if population_graphs
            .insert(
                assignment.population_id.clone(),
                assignment.graph_id.clone(),
            )
            .is_some()
        {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::DuplicateId,
                entity,
                "keep one behavior assignment per population",
            ));
        }
    }

    validate_semantics(project, &programs, &mut diagnostics);
    validate_groups(project, &base, &mut diagnostics);
    let assets = match validate_asset_library(&project.assets) {
        Ok(assets) => assets,
        Err(errors) => {
            diagnostics.extend(errors.into_iter().map(|error| {
                AuthoringDiagnostic::error(
                    AuthoringDiagnosticCode::InvalidAsset,
                    error.entity_id,
                    error.message,
                )
            }));
            validate_asset_library(&AssetLibraryV1::default())
                .expect("an empty asset library is a valid migration default")
        }
    };
    sort_diagnostics(&mut diagnostics);
    if diagnostics.is_empty() {
        Ok(CompiledAuthorableProject {
            base,
            programs,
            population_graphs,
            assets,
            queues: project.semantics.queues.clone(),
            groups: project.groups.clone(),
        })
    } else {
        Err(diagnostics)
    }
}

fn validate_semantics(
    project: &AuthorableProjectV2,
    programs: &BTreeMap<String, BehaviorProgram>,
    diagnostics: &mut Vec<AuthoringDiagnostic>,
) {
    let destination_ids: BTreeSet<_> = project
        .base
        .semantics
        .destinations
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let portal_ids: BTreeSet<_> = project
        .base
        .semantics
        .portals
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let walkable_ids: BTreeSet<_> = project
        .base
        .semantics
        .walkable
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let queue_ids: BTreeSet<_> = project
        .semantics
        .queues
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let lane_ids: BTreeSet<_> = project
        .semantics
        .lanes
        .iter()
        .map(|item| item.id.as_str())
        .collect();

    let mut all_ids = BTreeSet::new();
    for (kind, id) in project
        .semantics
        .queues
        .iter()
        .map(|item| ("queue", item.id.as_str()))
        .chain(
            project
                .semantics
                .lanes
                .iter()
                .map(|item| ("lane", item.id.as_str())),
        )
        .chain(
            project
                .semantics
                .cost_regions
                .iter()
                .map(|item| ("region", item.id.as_str())),
        )
    {
        if id.is_empty() || !all_ids.insert(id) {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::DuplicateId,
                format!("{kind}:{id}"),
                "give every M2 semantic entity a unique stable ID",
            ));
        }
    }

    for queue in &project.semantics.queues {
        let entity = format!("queue:{}", queue.id);
        if !portal_ids.contains(queue.portal_id.as_str()) {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::MissingSemanticReference,
                &entity,
                "choose an existing portal for this queue",
            ));
        }
        if queue.slots.is_empty() || queue.admission_capacity == 0 {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::InvalidSemanticGeometry,
                entity,
                "add at least one queue slot and positive admission capacity",
            ));
        }
    }
    for lane in &project.semantics.lanes {
        if lane.points.len() < 2 || lane.strength_millionths > 1_000_000 {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::InvalidSemanticGeometry,
                format!("lane:{}", lane.id),
                "add at least two points and set strength between 0 and 1000000",
            ));
        }
    }
    for region in &project.semantics.cost_regions {
        let entity = format!("region:{}", region.id);
        if !walkable_ids.contains(region.walkable_id.as_str()) {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::MissingSemanticReference,
                &entity,
                "choose an existing walkable region",
            ));
        }
        if !valid_bounds(region.bounds) || region.weight_millionths > 1_000_000 {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::InvalidSemanticGeometry,
                entity,
                "use finite ordered bounds and a weight between 0 and 1000000",
            ));
        }
    }

    for (graph_id, program) in programs {
        for node in program.nodes() {
            let (target, exists, action) = match node {
                BehaviorNodeV1::Navigate { destination_id, .. } => (
                    destination_id,
                    destination_id == "__assigned_destination"
                        || destination_ids.contains(destination_id.as_str()),
                    "choose an existing destination",
                ),
                BehaviorNodeV1::Queue { queue_id, .. } => (
                    queue_id,
                    queue_ids.contains(queue_id.as_str()),
                    "choose an existing queue",
                ),
                BehaviorNodeV1::FollowLane { lane_id, .. } => (
                    lane_id,
                    lane_ids.contains(lane_id.as_str()),
                    "choose an existing lane",
                ),
                _ => continue,
            };
            if !exists {
                diagnostics.push(AuthoringDiagnostic::error(
                    AuthoringDiagnosticCode::MissingSemanticReference,
                    format!("graph:{graph_id}/node:{}", node.id()),
                    format!("{action} instead of '{target}'"),
                ));
            }
        }
    }
}

fn validate_groups(
    project: &AuthorableProjectV2,
    base: &CompiledProject,
    diagnostics: &mut Vec<AuthoringDiagnostic>,
) {
    let agent_ids: BTreeSet<_> = base
        .agent_spawns()
        .iter()
        .map(|agent| agent.agent_id.0)
        .collect();
    let destination_ids: BTreeSet<_> = project
        .base
        .semantics
        .destinations
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    let mut group_ids = BTreeSet::new();
    let mut assigned_agents = BTreeSet::new();
    for group in &project.groups {
        let entity = format!("group:{}", group.id);
        if group.id.is_empty() || !group_ids.insert(group.id.as_str()) {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::DuplicateId,
                &entity,
                "give the group a unique stable ID",
            ));
        }
        if group.member_agent_ids.len() < 2
            || group.max_separation_millimeters == 0
            || !destination_ids.contains(group.shared_destination_id.as_str())
        {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::InvalidGroup,
                &entity,
                "add at least two members, a positive separation, and an existing destination",
            ));
        }
        if group
            .member_agent_ids
            .iter()
            .any(|id| !agent_ids.contains(id) || !assigned_agents.insert(*id))
        {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::UnknownAgent,
                &entity,
                "use known stable agent IDs and assign each agent to at most one group",
            ));
        }
        if group
            .leader_agent_id
            .is_some_and(|leader| !group.member_agent_ids.contains(&leader))
        {
            diagnostics.push(AuthoringDiagnostic::error(
                AuthoringDiagnosticCode::InvalidGroupLeader,
                entity,
                "choose a leader from this group's member IDs",
            ));
        }
    }
}

fn valid_bounds(bounds: Bounds2IrV1) -> bool {
    bounds.min.into_iter().chain(bounds.max).all(f32::is_finite)
        && bounds.min[0] < bounds.max[0]
        && bounds.min[1] < bounds.max[1]
}

fn sort_diagnostics(diagnostics: &mut [AuthoringDiagnostic]) {
    diagnostics.sort_by(|a, b| {
        (&a.entity_id, a.code, &a.message).cmp(&(&b.entity_id, b.code, &b.message))
    });
}
