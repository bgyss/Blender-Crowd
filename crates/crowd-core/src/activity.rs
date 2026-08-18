//! Deterministic finite-resource activities and reservations.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const ACTIVITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityResourceKindV1 {
    Seat,
    Door,
    HandoffPoint,
    ConversationSpace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedGoalV1 {
    pub key: String,
    pub target_millionths: u32,
    pub decay_per_tick: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityResourceBindingV1 {
    pub id: String,
    pub kind: ActivityResourceKindV1,
    pub capacity: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedActivityV1 {
    pub action_id: String,
    pub participant_roles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityPlanV1 {
    pub id: String,
    pub windows: Vec<(u64, u64)>,
    pub needs: Vec<NeedGoalV1>,
    pub resources: Vec<ActivityResourceBindingV1>,
    pub paired_action: Option<PairedActivityV1>,
    pub capacity: usize,
    pub failure_policy: ActivityFailurePolicyV1,
}

impl ActivityPlanV1 {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.id.is_empty() {
            errors.push("activity ID must be non-empty".to_owned());
        }
        if self.windows.is_empty() || self.windows.iter().any(|(start, end)| start > end) {
            errors.push("activity window declarations are invalid".to_owned());
        }
        if self.capacity == 0 {
            errors.push("activity capacity must be positive".to_owned());
        }
        let mut need_ids = std::collections::BTreeSet::new();
        for need in &self.needs {
            if need.key.is_empty() || need.target_millionths > 1_000_000 {
                errors.push(format!("activity need {} is invalid", need.key));
            }
            if !need_ids.insert(need.key.as_str()) {
                errors.push(format!("activity need {} is duplicated", need.key));
            }
        }
        let mut resource_ids = std::collections::BTreeSet::new();
        for resource in &self.resources {
            if resource.id.is_empty() || resource.capacity == 0 {
                errors.push(format!(
                    "activity resource {} has invalid capacity",
                    resource.id
                ));
            }
            if !resource_ids.insert(resource.id.as_str()) {
                errors.push(format!("activity resource {} is duplicated", resource.id));
            }
        }
        if let Some(paired) = &self.paired_action {
            let mut roles = std::collections::BTreeSet::new();
            if paired.action_id.is_empty() || paired.participant_roles.len() < 2 {
                errors.push("paired activity requires an action and at least two roles".to_owned());
            }
            for role in &paired.participant_roles {
                if role.is_empty() || !roles.insert(role.as_str()) {
                    errors.push("paired activity roles must be non-empty and unique".to_owned());
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn is_active(&self, tick: u64) -> bool {
        self.windows
            .iter()
            .any(|(start, end)| (*start..=*end).contains(&tick))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityFailurePolicyV1 {
    Retry,
    Release,
    Fallback,
    Fail,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityWindowV1 {
    pub start_tick: u64,
    pub end_tick: u64,
    pub priority: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityScheduleV1 {
    pub schema_version: u32,
    pub id: String,
    /// Inclusive, deterministically ordered activity windows.
    pub windows: Vec<ActivityWindowV1>,
    pub resources: Vec<String>,
    #[serde(default)]
    pub needs: Vec<NeedGoalV1>,
    #[serde(default)]
    pub paired_action: Option<PairedActivityV1>,
    #[serde(default = "default_activity_capacity")]
    pub capacity: usize,
    pub failure_policy: ActivityFailurePolicyV1,
}

impl ActivityScheduleV1 {
    pub fn validate(&self) -> Result<(), ActivityError> {
        if self.schema_version != ACTIVITY_SCHEMA_VERSION {
            return Err(ActivityError::UnsupportedVersion(self.schema_version));
        }
        if self.id.is_empty()
            || self.windows.is_empty()
            || self.resources.is_empty()
            || self.capacity == 0
        {
            return Err(ActivityError::InvalidSchedule(
                "id, windows, resources, and positive capacity are required".to_owned(),
            ));
        }
        if self
            .windows
            .iter()
            .any(|window| window.start_tick > window.end_tick)
        {
            return Err(ActivityError::InvalidSchedule(
                "activity window start must not be after end".to_owned(),
            ));
        }
        if self.resources.iter().any(String::is_empty) {
            return Err(ActivityError::InvalidSchedule(
                "activity resources must be non-empty".to_owned(),
            ));
        }
        let plan = ActivityPlanV1 {
            id: self.id.clone(),
            windows: self
                .windows
                .iter()
                .map(|window| (window.start_tick, window.end_tick))
                .collect(),
            needs: self.needs.clone(),
            resources: Vec::new(),
            paired_action: self.paired_action.clone(),
            capacity: self.capacity,
            failure_policy: self.failure_policy,
        };
        if let Err(plan_errors) = plan.validate() {
            return Err(ActivityError::InvalidSchedule(plan_errors.join("; ")));
        }
        Ok(())
    }

    pub fn is_active(&self, tick: u64) -> bool {
        self.windows
            .iter()
            .any(|window| (window.start_tick..=window.end_tick).contains(&tick))
    }
}

fn default_activity_capacity() -> usize {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceV1 {
    pub id: String,
    pub capacity: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityRequestV1 {
    pub agent_id: u64,
    pub resource_id: String,
    pub priority: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReservationStatusV1 {
    Granted,
    Waiting { ordinal: usize },
    Released,
    Failed { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReservationResultV1 {
    pub agent_id: u64,
    pub resource_id: String,
    pub status: ReservationStatusV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityError {
    UnsupportedVersion(u32),
    InvalidSchedule(String),
    EmptyResource,
    DuplicateResource(String),
    ZeroCapacity(String),
}

impl fmt::Display for ActivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported activity schema version {version}")
            }
            Self::InvalidSchedule(message) => write!(f, "invalid activity schedule: {message}"),
            Self::EmptyResource => write!(f, "resource IDs must be non-empty"),
            Self::DuplicateResource(id) => write!(f, "resource {id} is declared more than once"),
            Self::ZeroCapacity(id) => write!(f, "resource {id} must have positive capacity"),
        }
    }
}

impl Error for ActivityError {}

#[derive(Clone, Debug)]
struct WaitingEntry {
    agent_id: u64,
    priority: i32,
}

#[derive(Clone, Debug)]
struct ResourceState {
    capacity: usize,
    owners: BTreeSet<u64>,
    waiting: BTreeMap<u64, WaitingEntry>,
}

#[derive(Clone, Debug)]
pub struct ReservationRuntimeV1 {
    resources: BTreeMap<String, ResourceState>,
}

impl ReservationRuntimeV1 {
    pub fn new(resources: Vec<ResourceV1>) -> Result<Self, ActivityError> {
        let mut states = BTreeMap::new();
        for resource in resources {
            if resource.id.is_empty() {
                return Err(ActivityError::EmptyResource);
            }
            if resource.capacity == 0 {
                return Err(ActivityError::ZeroCapacity(resource.id));
            }
            if states
                .insert(
                    resource.id.clone(),
                    ResourceState {
                        capacity: resource.capacity,
                        owners: BTreeSet::new(),
                        waiting: BTreeMap::new(),
                    },
                )
                .is_some()
            {
                return Err(ActivityError::DuplicateResource(resource.id));
            }
        }
        Ok(Self { resources: states })
    }

    pub fn request_batch(&mut self, requests: &[ActivityRequestV1]) -> Vec<ReservationResultV1> {
        let mut grouped: BTreeMap<String, Vec<ActivityRequestV1>> = BTreeMap::new();
        for request in requests {
            grouped
                .entry(request.resource_id.clone())
                .or_default()
                .push(request.clone());
        }
        let mut results = Vec::new();
        for (resource_id, mut requests) in grouped {
            let Some(state) = self.resources.get_mut(&resource_id) else {
                results.extend(requests.into_iter().map(|request| ReservationResultV1 {
                    agent_id: request.agent_id,
                    resource_id: resource_id.clone(),
                    status: ReservationStatusV1::Failed {
                        reason: "unknown resource".to_owned(),
                    },
                }));
                continue;
            };
            requests.sort_by(|left, right| {
                right
                    .priority
                    .cmp(&left.priority)
                    .then_with(|| left.agent_id.cmp(&right.agent_id))
            });
            requests.dedup_by_key(|request| request.agent_id);
            for request in &requests {
                if !state.owners.contains(&request.agent_id) {
                    state
                        .waiting
                        .entry(request.agent_id)
                        .or_insert_with(|| WaitingEntry {
                            agent_id: request.agent_id,
                            priority: request.priority,
                        });
                }
            }
            promote_waiters(state);
            for request in requests {
                results.push(ReservationResultV1 {
                    agent_id: request.agent_id,
                    resource_id: resource_id.clone(),
                    status: status_for(state, request.agent_id),
                });
            }
        }
        results.sort_by(|left, right| {
            left.agent_id
                .cmp(&right.agent_id)
                .then_with(|| left.resource_id.cmp(&right.resource_id))
        });
        results
    }

    pub fn status(&self, agent_id: u64, resource_id: &str) -> ReservationStatusV1 {
        self.resources.get(resource_id).map_or_else(
            || ReservationStatusV1::Failed {
                reason: "unknown resource".to_owned(),
            },
            |state| status_for(state, agent_id),
        )
    }

    pub fn release(&mut self, agent_id: u64, resource_id: &str) -> bool {
        let Some(state) = self.resources.get_mut(resource_id) else {
            return false;
        };
        let released = state.owners.remove(&agent_id);
        state.waiting.remove(&agent_id);
        if released {
            promote_waiters(state);
        }
        released
    }

    pub fn owners(&self, resource_id: &str) -> Vec<u64> {
        self.resources
            .get(resource_id)
            .map(|state| state.owners.iter().copied().collect())
            .unwrap_or_default()
    }
}

fn promote_waiters(state: &mut ResourceState) {
    while state.owners.len() < state.capacity {
        let next = state
            .waiting
            .values()
            .min_by(|left, right| {
                right
                    .priority
                    .cmp(&left.priority)
                    .then_with(|| left.agent_id.cmp(&right.agent_id))
            })
            .map(|entry| entry.agent_id);
        let Some(agent_id) = next else {
            break;
        };
        state.waiting.remove(&agent_id);
        state.owners.insert(agent_id);
    }
}

fn status_for(state: &ResourceState, agent_id: u64) -> ReservationStatusV1 {
    if state.owners.contains(&agent_id) {
        return ReservationStatusV1::Granted;
    }
    let mut waiting: Vec<_> = state.waiting.values().collect();
    waiting.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.agent_id.cmp(&right.agent_id))
    });
    waiting
        .iter()
        .position(|entry| entry.agent_id == agent_id)
        .map_or(ReservationStatusV1::Released, |index| {
            ReservationStatusV1::Waiting { ordinal: index + 1 }
        })
}
