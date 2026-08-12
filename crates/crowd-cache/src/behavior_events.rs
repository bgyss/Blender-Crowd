//! Ordered, versioned explanations for authored behavior decisions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const BEHAVIOR_EVENTS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorEventKindV1 {
    Decision,
    QueueRequested,
    QueueAdmitted,
    QueueReleased,
    GroupSplit,
    GroupRegrouped,
    PortalReroute,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorEventV1 {
    pub tick: u64,
    pub agent_id: u64,
    pub kind: BehaviorEventKindV1,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decisive_node: Option<String>,
}

/// Incrementally retains the cache-side event evidence for a bake.
///
/// The runtime may emit a decision for every agent on every tick. Keeping
/// unchanged decisions out of the accumulator bounds its memory use by state
/// transitions while retaining every queue and group lifecycle event.
#[derive(Default)]
pub struct BehaviorEventCompactor {
    events: Vec<BehaviorEventV1>,
    last_decisions: BTreeMap<u64, (String, Option<String>, Option<String>)>,
}

impl BehaviorEventCompactor {
    pub fn push(&mut self, event: BehaviorEventV1) {
        if event.kind == BehaviorEventKindV1::Decision {
            let signature = (
                event.detail.clone(),
                event.graph_id.clone(),
                event.decisive_node.clone(),
            );
            let prior = self
                .last_decisions
                .insert(event.agent_id, signature.clone());
            if prior.as_ref() == Some(&signature) {
                return;
            }
        }
        self.events.push(event);
    }

    pub fn into_events(self) -> Vec<BehaviorEventV1> {
        self.events
    }
}

impl BehaviorEventV1 {
    pub fn new(
        tick: u64,
        agent_id: u64,
        kind: BehaviorEventKindV1,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            tick,
            agent_id,
            kind,
            detail: detail.into(),
            graph_id: None,
            decisive_node: None,
        }
    }

    pub fn decision(
        tick: u64,
        agent_id: u64,
        graph_id: impl Into<String>,
        decisive_node: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            tick,
            agent_id,
            kind: BehaviorEventKindV1::Decision,
            detail: detail.into(),
            graph_id: Some(graph_id.into()),
            decisive_node: Some(decisive_node.into()),
        }
    }
}

/// Keep the first and every changed decision for each agent while preserving
/// all lifecycle events. This makes a long bake's cache-side explanation
/// proportional to authored state transitions instead of simulation ticks.
pub fn compact_behavior_events(events: Vec<BehaviorEventV1>) -> Vec<BehaviorEventV1> {
    let mut compactor = BehaviorEventCompactor::default();
    for event in events {
        compactor.push(event);
    }
    compactor.into_events()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BehaviorEventLogV1 {
    pub schema_version: u32,
    pub events: Vec<BehaviorEventV1>,
}

impl BehaviorEventLogV1 {
    pub(crate) fn new(events: Vec<BehaviorEventV1>) -> Self {
        Self {
            schema_version: BEHAVIOR_EVENTS_SCHEMA_VERSION,
            events,
        }
    }

    pub(crate) fn validate(
        &self,
        tick_start: u64,
        tick_end: u64,
        agent_ids: &[u64],
    ) -> Result<(), String> {
        if self.schema_version != BEHAVIOR_EVENTS_SCHEMA_VERSION {
            return Err(format!(
                "unsupported behavior events version {}",
                self.schema_version
            ));
        }
        let mut previous = None;
        for event in &self.events {
            if event.detail.trim().is_empty() {
                return Err("behavior events require a non-empty detail".to_owned());
            }
            let key = (event.tick, event.agent_id, event.kind);
            if previous.is_some_and(|last| key < last) {
                return Err(
                    "behavior events must be ordered by tick, agent ID, and kind".to_owned(),
                );
            }
            previous = Some(key);
            if event.tick < tick_start || event.tick > tick_end {
                return Err(format!(
                    "behavior events tick {} is outside {}..={}",
                    event.tick, tick_start, tick_end
                ));
            }
            if !agent_ids.contains(&event.agent_id) {
                return Err(format!(
                    "behavior events references unknown agent {}",
                    event.agent_id
                ));
            }
        }
        Ok(())
    }
}
