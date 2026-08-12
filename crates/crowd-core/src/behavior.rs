//! Typed, bounded behavior-graph authoring IR and deterministic compiler.
//!
//! The graph is deliberately data, not callbacks: it is safe to serialize,
//! validate before a bake, and later lower to a hot-loop bytecode program.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::ids::{hash_combine, hash_str, AgentId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorGraphV1 {
    pub id: String,
    pub entry_id: String,
    pub nodes: Vec<BehaviorNodeV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UtilityOptionV1 {
    pub child: String,
    pub score_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateBranchV1 {
    pub value: i32,
    pub child: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BehaviorNodeV1 {
    Selector {
        id: String,
        children: Vec<String>,
    },
    Sequence {
        id: String,
        children: Vec<String>,
    },
    Fallback {
        id: String,
        children: Vec<String>,
    },
    UtilitySelector {
        id: String,
        options: Vec<UtilityOptionV1>,
    },
    StateSwitch {
        id: String,
        state_key: String,
        branches: Vec<StateBranchV1>,
        fallback: String,
    },
    Interrupt {
        id: String,
        condition_key: String,
        child: String,
    },
    Timer {
        id: String,
        ticks: u32,
        child: String,
    },
    Probability {
        id: String,
        probability_millionths: u32,
        child: String,
    },
    Event {
        id: String,
        event_type: String,
        child: String,
    },
    BlackboardCompare {
        id: String,
        key: String,
        child: String,
    },
    Navigate {
        id: String,
        destination_id: String,
    },
    Wait {
        id: String,
        ticks: u32,
    },
    Queue {
        id: String,
        queue_id: String,
    },
    FollowLane {
        id: String,
        lane_id: String,
    },
    HoldPosition {
        id: String,
    },
}

impl BehaviorNodeV1 {
    pub fn id(&self) -> &str {
        match self {
            Self::Selector { id, .. }
            | Self::Sequence { id, .. }
            | Self::Fallback { id, .. }
            | Self::UtilitySelector { id, .. }
            | Self::StateSwitch { id, .. }
            | Self::Interrupt { id, .. }
            | Self::Timer { id, .. }
            | Self::Probability { id, .. }
            | Self::Event { id, .. }
            | Self::BlackboardCompare { id, .. }
            | Self::Navigate { id, .. }
            | Self::Wait { id, .. }
            | Self::Queue { id, .. }
            | Self::FollowLane { id, .. }
            | Self::HoldPosition { id } => id,
        }
    }

    fn children(&self) -> Vec<&str> {
        match self {
            Self::Selector { children, .. }
            | Self::Sequence { children, .. }
            | Self::Fallback { children, .. } => children.iter().map(String::as_str).collect(),
            Self::UtilitySelector { options, .. } => {
                options.iter().map(|option| option.child.as_str()).collect()
            }
            Self::StateSwitch {
                branches, fallback, ..
            } => branches
                .iter()
                .map(|branch| branch.child.as_str())
                .chain(std::iter::once(fallback.as_str()))
                .collect(),
            Self::Interrupt { child, .. }
            | Self::Timer { child, .. }
            | Self::Probability { child, .. }
            | Self::Event { child, .. }
            | Self::BlackboardCompare { child, .. } => vec![child],
            Self::Navigate { .. }
            | Self::Wait { .. }
            | Self::Queue { .. }
            | Self::FollowLane { .. }
            | Self::HoldPosition { .. } => Vec::new(),
        }
    }

    fn validate_parameters(&self) -> Option<&'static str> {
        match self {
            Self::Selector { children, .. }
            | Self::Sequence { children, .. }
            | Self::Fallback { children, .. }
                if children.is_empty() =>
            {
                Some("add at least one child")
            }
            Self::UtilitySelector { options, .. }
                if options.is_empty()
                    || options
                        .iter()
                        .any(|option| option.child.is_empty() || option.score_key.is_empty()) =>
            {
                Some("add at least one child with a numeric score key")
            }
            Self::StateSwitch {
                state_key,
                branches,
                fallback,
                ..
            } if state_key.is_empty() || branches.is_empty() || fallback.is_empty() => {
                Some("set a state key, at least one branch, and a fallback")
            }
            Self::Timer { ticks: 0, .. } | Self::Wait { ticks: 0, .. } => {
                Some("set ticks to a positive value")
            }
            Self::Probability {
                probability_millionths,
                ..
            } if *probability_millionths > 1_000_000 => {
                Some("set probability_millionths between 0 and 1000000")
            }
            Self::Interrupt { condition_key, .. } if condition_key.is_empty() => {
                Some("set a blackboard condition key")
            }
            Self::Event { event_type, .. } if event_type.is_empty() => Some("set an event type"),
            Self::BlackboardCompare { key, .. } if key.is_empty() => Some("set a blackboard key"),
            Self::Navigate { destination_id, .. } if destination_id.is_empty() => {
                Some("set a destination ID")
            }
            Self::Queue { queue_id, .. } if queue_id.is_empty() => Some("set a queue ID"),
            Self::FollowLane { lane_id, .. } if lane_id.is_empty() => Some("set a lane ID"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphDiagnosticCode {
    EmptyGraph,
    DuplicateNode,
    MissingEntry,
    MissingNode,
    Cycle,
    UnreachableNode,
    InvalidNode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphDiagnostic {
    pub code: GraphDiagnosticCode,
    pub node_id: String,
    pub message: String,
}

impl GraphDiagnostic {
    fn error(
        code: GraphDiagnosticCode,
        node_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            node_id: node_id.into(),
            message: message.into(),
        }
    }
}

/// A normalized, index-addressable graph. Its node order is stable by node ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BehaviorProgram {
    id: String,
    entry_index: u32,
    nodes: Vec<BehaviorNodeV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BehaviorAction {
    Navigate { destination_id: String },
    Wait { ticks: u32 },
    Queue { queue_id: String },
    FollowLane { lane_id: String },
    HoldPosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BehaviorContext {
    pub tick: u64,
    pub agent_id: AgentId,
    pub bool_observations: BTreeMap<String, bool>,
    /// Deterministic fixed-point numeric observations (one unit = 1e-6).
    pub number_observations: BTreeMap<String, i32>,
    pub events: BTreeSet<String>,
    /// Node IDs whose action completed since the preceding decision.
    pub completed_nodes: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BehaviorVmState {
    timer_started_at: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionOutcome {
    pub action: Option<BehaviorAction>,
    pub decisive_node: Option<String>,
    pub visited_nodes: Vec<String>,
    pub observations: Vec<(String, bool)>,
    pub number_observations: Vec<(String, i32)>,
}

#[derive(Clone, Debug)]
pub struct BehaviorVm {
    program: BehaviorProgram,
    global_seed: u64,
    node_indices: BTreeMap<String, u32>,
}

impl BehaviorVm {
    pub fn new(program: BehaviorProgram, global_seed: u64) -> Self {
        let node_indices = program
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id().to_string(), index as u32))
            .collect();
        Self {
            program,
            global_seed,
            node_indices,
        }
    }

    pub fn decide(
        &self,
        state: &mut BehaviorVmState,
        context: &BehaviorContext,
    ) -> DecisionOutcome {
        let mut outcome = DecisionOutcome::default();
        outcome.action = self.evaluate(self.program.entry_index, state, context, &mut outcome);
        outcome
    }

    fn evaluate(
        &self,
        index: u32,
        state: &mut BehaviorVmState,
        context: &BehaviorContext,
        outcome: &mut DecisionOutcome,
    ) -> Option<BehaviorAction> {
        let node = &self.program.nodes[index as usize];
        outcome.visited_nodes.push(node.id().to_string());
        match node {
            BehaviorNodeV1::Selector { children, .. }
            | BehaviorNodeV1::Fallback { children, .. } => children
                .iter()
                .find_map(|child| self.evaluate(self.node_indices[child], state, context, outcome)),
            BehaviorNodeV1::Sequence { children, .. } => children
                .iter()
                .find(|child| !context.completed_nodes.contains(child.as_str()))
                .and_then(|child| self.evaluate(self.node_indices[child], state, context, outcome)),
            BehaviorNodeV1::UtilitySelector { options, .. } => {
                let mut scored: Vec<_> = options
                    .iter()
                    .map(|option| {
                        let score = context
                            .number_observations
                            .get(&option.score_key)
                            .copied()
                            .unwrap_or(0);
                        outcome
                            .number_observations
                            .push((option.score_key.clone(), score));
                        (score, option.child.as_str())
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
                scored.first().and_then(|(_, child)| {
                    self.evaluate(self.node_indices[*child], state, context, outcome)
                })
            }
            BehaviorNodeV1::StateSwitch {
                state_key,
                branches,
                fallback,
                ..
            } => {
                let value = context
                    .number_observations
                    .get(state_key)
                    .copied()
                    .unwrap_or(0);
                outcome.number_observations.push((state_key.clone(), value));
                let child = branches
                    .iter()
                    .find(|branch| branch.value == value)
                    .map(|branch| branch.child.as_str())
                    .unwrap_or(fallback);
                self.evaluate(self.node_indices[child], state, context, outcome)
            }
            BehaviorNodeV1::Interrupt {
                condition_key,
                child,
                ..
            }
            | BehaviorNodeV1::BlackboardCompare {
                key: condition_key,
                child,
                ..
            } => {
                let value = context
                    .bool_observations
                    .get(condition_key)
                    .copied()
                    .unwrap_or(false);
                if !outcome
                    .observations
                    .iter()
                    .any(|(key, _)| key == condition_key)
                {
                    outcome.observations.push((condition_key.clone(), value));
                }
                value
                    .then(|| self.evaluate(self.node_indices[child], state, context, outcome))
                    .flatten()
            }
            BehaviorNodeV1::Timer {
                id, ticks, child, ..
            } => {
                let started = state
                    .timer_started_at
                    .entry(id.clone())
                    .or_insert(context.tick);
                (context.tick.saturating_sub(*started) >= u64::from(*ticks))
                    .then(|| self.evaluate(self.node_indices[child], state, context, outcome))
                    .flatten()
            }
            BehaviorNodeV1::Probability {
                id,
                probability_millionths,
                child,
            } => {
                let mut sample = hash_combine(self.global_seed, context.agent_id.0);
                sample = hash_combine(sample, context.tick);
                sample = hash_combine(sample, hash_str(id));
                (sample % 1_000_000 < u64::from(*probability_millionths))
                    .then(|| self.evaluate(self.node_indices[child], state, context, outcome))
                    .flatten()
            }
            BehaviorNodeV1::Event {
                event_type, child, ..
            } => context
                .events
                .contains(event_type)
                .then(|| self.evaluate(self.node_indices[child], state, context, outcome))
                .flatten(),
            BehaviorNodeV1::Navigate { id, destination_id } => {
                outcome.decisive_node = Some(id.clone());
                Some(BehaviorAction::Navigate {
                    destination_id: destination_id.clone(),
                })
            }
            BehaviorNodeV1::Wait { id, ticks } => {
                outcome.decisive_node = Some(id.clone());
                Some(BehaviorAction::Wait { ticks: *ticks })
            }
            BehaviorNodeV1::Queue { id, queue_id } => {
                outcome.decisive_node = Some(id.clone());
                Some(BehaviorAction::Queue {
                    queue_id: queue_id.clone(),
                })
            }
            BehaviorNodeV1::FollowLane { id, lane_id } => {
                outcome.decisive_node = Some(id.clone());
                Some(BehaviorAction::FollowLane {
                    lane_id: lane_id.clone(),
                })
            }
            BehaviorNodeV1::HoldPosition { id } => {
                outcome.decisive_node = Some(id.clone());
                Some(BehaviorAction::HoldPosition)
            }
        }
    }
}

impl BehaviorProgram {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn entry_index(&self) -> u32 {
        self.entry_index
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn nodes(&self) -> &[BehaviorNodeV1] {
        &self.nodes
    }
}

pub fn compile_graph(graph: &BehaviorGraphV1) -> Result<BehaviorProgram, Vec<GraphDiagnostic>> {
    let mut diagnostics = Vec::new();
    if graph.id.is_empty() || graph.nodes.is_empty() {
        diagnostics.push(GraphDiagnostic::error(
            GraphDiagnosticCode::EmptyGraph,
            graph.id.clone(),
            "add a graph ID and at least one node",
        ));
    }

    let mut by_id = BTreeMap::new();
    for node in &graph.nodes {
        if node.id().is_empty() {
            diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::InvalidNode,
                "<unnamed>",
                "give every node a stable ID",
            ));
            continue;
        }
        if by_id.insert(node.id(), node).is_some() {
            diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::DuplicateNode,
                node.id(),
                "rename the duplicate node ID",
            ));
        }
        if let Some(action) = node.validate_parameters() {
            diagnostics.push(GraphDiagnostic::error(
                GraphDiagnosticCode::InvalidNode,
                node.id(),
                action,
            ));
        }
    }

    if !by_id.contains_key(graph.entry_id.as_str()) {
        diagnostics.push(GraphDiagnostic::error(
            GraphDiagnosticCode::MissingEntry,
            graph.entry_id.clone(),
            "choose an existing entry node",
        ));
    }
    for node in by_id.values() {
        for child in node.children() {
            if !by_id.contains_key(child) {
                diagnostics.push(GraphDiagnostic::error(
                    GraphDiagnosticCode::MissingNode,
                    node.id(),
                    format!("connect to an existing node instead of '{child}'"),
                ));
            }
        }
    }

    if diagnostics.is_empty() {
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        visit(
            graph.entry_id.as_str(),
            &by_id,
            &mut visiting,
            &mut visited,
            &mut diagnostics,
        );
        for id in by_id.keys() {
            if !visited.contains(id) {
                diagnostics.push(GraphDiagnostic::error(
                    GraphDiagnosticCode::UnreachableNode,
                    *id,
                    "connect this node to the entry or remove it",
                ));
            }
        }
    }

    diagnostics
        .sort_by(|a, b| (&a.node_id, a.code, &a.message).cmp(&(&b.node_id, b.code, &b.message)));
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let nodes: Vec<_> = by_id.into_values().cloned().collect();
    let entry_index = nodes
        .binary_search_by(|node| node.id().cmp(graph.entry_id.as_str()))
        .expect("validated entry is present") as u32;
    Ok(BehaviorProgram {
        id: graph.id.clone(),
        entry_index,
        nodes,
    })
}

fn visit<'a>(
    id: &'a str,
    by_id: &BTreeMap<&'a str, &'a BehaviorNodeV1>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    if visited.contains(id) {
        return;
    }
    if !visiting.insert(id) {
        diagnostics.push(GraphDiagnostic::error(
            GraphDiagnosticCode::Cycle,
            id,
            "remove the cycle; behavior graphs cannot loop",
        ));
        return;
    }
    for child in by_id[id].children() {
        visit(child, by_id, visiting, visited, diagnostics);
    }
    visiting.remove(id);
    visited.insert(id);
}
