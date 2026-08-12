//! Adapter that executes compiled M2 graphs in the fixed-step decide phase.

use std::collections::{BTreeMap, BTreeSet};

use crate::arena::NeighborArena;
use crate::authoring::GroupBottleneckPolicyV2;
use crate::behavior::{
    BehaviorAction, BehaviorContext, BehaviorVm, BehaviorVmState, DecisionOutcome,
};
use crate::ids::AgentId;
use crate::social::{GroupConstraint, GroupReport, QueueRuntime, QueueStatus};
use crate::units::Vec2;
use crate::world::{World, NO_ROUTE};

#[derive(Clone, Debug)]
pub struct RuntimeBehaviorController {
    pub(crate) by_population: BTreeMap<u16, BehaviorVm>,
    pub(crate) destination_indices: BTreeMap<String, u16>,
    graph_ids: BTreeMap<u16, String>,
    states: BTreeMap<AgentId, BehaviorVmState>,
    traces: BTreeMap<AgentId, DecisionOutcome>,
    completed_nodes: BTreeMap<AgentId, BTreeSet<String>>,
    queues: BTreeMap<String, RuntimeQueue>,
    groups: Vec<RuntimeGroup>,
    group_reports: BTreeMap<String, GroupReport>,
    group_was_split: BTreeMap<String, bool>,
    passed_group_bottlenecks: BTreeSet<(String, String, AgentId)>,
    events: Vec<BehaviorRuntimeEvent>,
}

/// An ordered, cacheable explanation emitted by the live M2 behavior runtime.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BehaviorRuntimeEventKind {
    Decision,
    QueueRequested,
    QueueAdmitted,
    QueueReleased,
    GroupSplit,
    GroupRegrouped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BehaviorRuntimeEvent {
    pub tick: u64,
    pub agent_id: AgentId,
    pub kind: BehaviorRuntimeEventKind,
    pub detail: String,
    pub graph_id: Option<String>,
    pub decisive_node: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeQueue {
    pub runtime: QueueRuntime,
    pub slots: Vec<Vec2>,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGroup {
    pub constraint: GroupConstraint,
    pub shared_destination: Option<u16>,
    pub bottleneck_policy: GroupBottleneckPolicyV2,
}

impl RuntimeBehaviorController {
    pub(crate) fn new(
        by_population: BTreeMap<u16, BehaviorVm>,
        destination_indices: BTreeMap<String, u16>,
    ) -> Self {
        Self {
            by_population,
            destination_indices,
            graph_ids: BTreeMap::new(),
            states: BTreeMap::new(),
            traces: BTreeMap::new(),
            completed_nodes: BTreeMap::new(),
            queues: BTreeMap::new(),
            groups: Vec::new(),
            group_reports: BTreeMap::new(),
            group_was_split: BTreeMap::new(),
            passed_group_bottlenecks: BTreeSet::new(),
            events: Vec::new(),
        }
    }

    pub(crate) fn with_graph_ids(mut self, graph_ids: BTreeMap<u16, String>) -> Self {
        self.graph_ids = graph_ids;
        self
    }

    pub(crate) fn with_social(
        mut self,
        queues: BTreeMap<String, RuntimeQueue>,
        groups: Vec<RuntimeGroup>,
    ) -> Self {
        self.queues = queues;
        self.groups = groups;
        self
    }

    pub fn trace(&self, agent_id: AgentId) -> Option<&DecisionOutcome> {
        self.traces.get(&agent_id)
    }

    pub fn queue_status(&self, queue_id: &str, agent_id: AgentId) -> QueueStatus {
        self.queues
            .get(queue_id)
            .map_or(QueueStatus::Absent, |queue| queue.runtime.status(agent_id))
    }

    pub fn group_report(&self, group_id: &str) -> Option<GroupReport> {
        self.group_reports.get(group_id).copied()
    }

    pub fn drain_events(&mut self) -> Vec<BehaviorRuntimeEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn apply(&mut self, world: &mut World, neighbors: &NeighborArena, tick: u64) {
        // A reserved line is a live spatial structure: its head clears only
        // after reaching the first authored slot, then every later reservation
        // advances deterministically on the next admission pass.
        for (queue_id, queue) in &mut self.queues {
            let Some(agent_id) = queue.runtime.front_agent() else {
                continue;
            };
            let Some(slot) = world.slot_of(agent_id) else {
                continue;
            };
            let position = world.position(slot);
            let target = queue.slots[0];
            let reached_head =
                (target - position).length() <= world.radius[slot as usize].max(0.25);
            if reached_head && queue.runtime.release(agent_id) {
                if let Some(trace) = self.traces.get(&agent_id) {
                    if matches!(&trace.action, Some(BehaviorAction::Queue { .. })) {
                        if let Some(node) = &trace.decisive_node {
                            self.completed_nodes
                                .entry(agent_id)
                                .or_default()
                                .insert(node.clone());
                        }
                    }
                }
                self.events.push(BehaviorRuntimeEvent {
                    tick,
                    agent_id,
                    kind: BehaviorRuntimeEventKind::QueueReleased,
                    detail: queue_id.clone(),
                    graph_id: None,
                    decisive_node: None,
                });
                for group in &self.groups {
                    if group.bottleneck_policy == GroupBottleneckPolicyV2::LeaderFirst
                        && group.constraint.members().contains(&agent_id)
                    {
                        self.passed_group_bottlenecks.insert((
                            group.constraint.id().to_string(),
                            queue_id.clone(),
                            agent_id,
                        ));
                    }
                }
            }
        }
        for queue in self.queues.values_mut() {
            queue.runtime.advance_tick();
        }
        let mut decisions = Vec::with_capacity(world.len());
        for slot in 0..world.len() {
            let population = world.population_id[slot];
            let Some(vm) = self.by_population.get(&population) else {
                continue;
            };
            let agent_id = world.agent_id[slot];
            let neighbor_count = neighbors.neighbors(slot).len() as i32;
            let context = BehaviorContext {
                tick,
                agent_id,
                bool_observations: BTreeMap::from([
                    ("nearby_agents".to_string(), neighbor_count > 0),
                    ("density_high".to_string(), neighbor_count >= 8),
                ]),
                number_observations: BTreeMap::from([
                    ("nearby_agent_count".to_string(), neighbor_count),
                    (
                        "density_score".to_string(),
                        (neighbor_count * 62_500).min(1_000_000),
                    ),
                ]),
                events: BTreeSet::new(),
                completed_nodes: self
                    .completed_nodes
                    .get(&agent_id)
                    .cloned()
                    .unwrap_or_default(),
            };
            let outcome = vm.decide(self.states.entry(agent_id).or_default(), &context);
            self.events.push(BehaviorRuntimeEvent {
                tick,
                agent_id,
                kind: BehaviorRuntimeEventKind::Decision,
                detail: format!("population:{population}"),
                graph_id: self.graph_ids.get(&population).cloned(),
                decisive_node: outcome.decisive_node.clone(),
            });
            decisions.push((slot, agent_id, outcome));
        }
        let mut queue_requests: BTreeMap<String, Vec<AgentId>> = BTreeMap::new();
        for (_, agent_id, outcome) in &decisions {
            if let Some(BehaviorAction::Queue { queue_id }) = &outcome.action {
                if self.group_bottleneck_allows(*agent_id, queue_id) {
                    queue_requests
                        .entry(queue_id.clone())
                        .or_default()
                        .push(*agent_id);
                }
            }
        }
        for (queue_id, agents) in queue_requests {
            if let Some(queue) = self.queues.get_mut(&queue_id) {
                let prior: BTreeMap<_, _> = agents
                    .iter()
                    .map(|agent| (*agent, queue.runtime.status(*agent)))
                    .collect();
                queue.runtime.request_batch(&agents);
                for agent in agents {
                    let before = prior[&agent];
                    let after = queue.runtime.status(agent);
                    if matches!(before, QueueStatus::Absent) {
                        self.events.push(BehaviorRuntimeEvent {
                            tick,
                            agent_id: agent,
                            kind: BehaviorRuntimeEventKind::QueueRequested,
                            detail: queue_id.clone(),
                            graph_id: None,
                            decisive_node: None,
                        });
                    }
                    if !matches!(before, QueueStatus::Admitted { .. })
                        && matches!(after, QueueStatus::Admitted { .. })
                    {
                        self.events.push(BehaviorRuntimeEvent {
                            tick,
                            agent_id: agent,
                            kind: BehaviorRuntimeEventKind::QueueAdmitted,
                            detail: queue_id.clone(),
                            graph_id: None,
                            decisive_node: None,
                        });
                    }
                }
            }
        }
        for (slot, agent_id, outcome) in decisions {
            match &outcome.action {
                Some(BehaviorAction::HoldPosition | BehaviorAction::Wait { .. }) => {
                    world.des_vel_x[slot] = 0.0;
                    world.des_vel_y[slot] = 0.0;
                }
                Some(BehaviorAction::Queue { queue_id }) => {
                    if let Some(queue) = self.queues.get(queue_id) {
                        if let Some(slot_index) = queue.runtime.assigned_slot(agent_id) {
                            let target = queue.slots[slot_index];
                            let offset = target - world.position(slot as u32);
                            if offset.length() > world.radius[slot].max(0.25) {
                                let velocity =
                                    offset.normalize_or_zero() * world.preferred_speed[slot];
                                world.des_vel_x[slot] = velocity.x;
                                world.des_vel_y[slot] = velocity.y;
                            } else {
                                world.des_vel_x[slot] = 0.0;
                                world.des_vel_y[slot] = 0.0;
                            }
                        } else {
                            world.des_vel_x[slot] = 0.0;
                            world.des_vel_y[slot] = 0.0;
                        }
                    }
                }
                Some(BehaviorAction::Navigate { destination_id })
                    if destination_id != "__assigned_destination" =>
                {
                    if let Some(destination) = self.destination_indices.get(destination_id).copied()
                    {
                        if world.destination[slot] != destination {
                            world.destination[slot] = destination;
                            world.route[slot] = NO_ROUTE;
                            world.arrived[slot] = false;
                            world.unrouted[slot] = false;
                        }
                    }
                }
                Some(BehaviorAction::FollowLane { .. } | BehaviorAction::Navigate { .. })
                | None => {}
            }
            self.traces.insert(agent_id, outcome);
        }
        let positions: BTreeMap<_, _> = (0..world.len())
            .map(|slot| (world.agent_id[slot], world.position(slot as u32)))
            .collect();
        for group in &self.groups {
            if let Some(destination) = group.shared_destination {
                for member in group.constraint.members() {
                    let Some(slot) = world.slot_of(*member) else {
                        continue;
                    };
                    let slot = slot as usize;
                    if world.destination[slot] != destination {
                        world.destination[slot] = destination;
                        world.route[slot] = NO_ROUTE;
                        world.arrived[slot] = false;
                        world.unrouted[slot] = false;
                    }
                }
            }
            let report = group.constraint.evaluate(&positions);
            self.group_reports
                .insert(group.constraint.id().to_string(), report);
            let was_split = self
                .group_was_split
                .insert(group.constraint.id().to_string(), report.split);
            if was_split.map_or(report.split, |split| split != report.split) {
                self.events.push(BehaviorRuntimeEvent {
                    tick,
                    agent_id: report
                        .farthest_member
                        .unwrap_or_else(|| group.constraint.leader()),
                    kind: if report.split {
                        BehaviorRuntimeEventKind::GroupSplit
                    } else {
                        BehaviorRuntimeEventKind::GroupRegrouped
                    },
                    detail: group.constraint.id().to_string(),
                    graph_id: None,
                    decisive_node: None,
                });
            }
            for agent in positions.keys() {
                let correction = group.constraint.cohesion_velocity(*agent, &positions);
                if correction == Vec2::ZERO {
                    continue;
                }
                let Some(slot) = world.slot_of(*agent) else {
                    continue;
                };
                let slot = slot as usize;
                let desired = Vec2::new(world.des_vel_x[slot], world.des_vel_y[slot]) + correction;
                let clamped = if desired.length() > world.max_speed[slot] {
                    desired.normalize_or_zero() * world.max_speed[slot]
                } else {
                    desired
                };
                world.des_vel_x[slot] = clamped.x;
                world.des_vel_y[slot] = clamped.y;
            }
        }
        self.events.sort_by(|left, right| {
            (left.tick, left.agent_id, &left.kind).cmp(&(right.tick, right.agent_id, &right.kind))
        });
    }

    /// A leader-first group owns one queue reservation at a time. The next
    /// group member becomes eligible only after the prior member reaches and
    /// releases the head slot; individual groups retain ordinary admission.
    fn group_bottleneck_allows(&self, agent_id: AgentId, queue_id: &str) -> bool {
        for group in &self.groups {
            if group.bottleneck_policy != GroupBottleneckPolicyV2::LeaderFirst
                || !group.constraint.members().contains(&agent_id)
            {
                continue;
            }
            let mut ordered = vec![group.constraint.leader()];
            ordered.extend(
                group
                    .constraint
                    .members()
                    .iter()
                    .copied()
                    .filter(|member| *member != group.constraint.leader()),
            );
            let next = ordered.into_iter().find(|member| {
                !self.passed_group_bottlenecks.contains(&(
                    group.constraint.id().to_string(),
                    queue_id.to_string(),
                    *member,
                ))
            });
            if next != Some(agent_id) {
                return false;
            }
        }
        true
    }
}
