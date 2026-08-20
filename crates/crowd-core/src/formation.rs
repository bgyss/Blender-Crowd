//! Stable group roles, formations, and readable split/intrusion reports.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::AgentId;
use crate::units::Vec2;

pub const FORMATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormationRoleV1 {
    pub agent_id: AgentId,
    pub role: String,
    pub offset_millimeters: [i32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormationSplitPolicyV1 {
    HoldLeader,
    Regroup,
    ContinueIndividual,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormationV1 {
    pub schema_version: u32,
    pub id: String,
    pub leader_agent_id: AgentId,
    pub roles: Vec<FormationRoleV1>,
    pub max_separation_millimeters: u32,
    pub split_policy: FormationSplitPolicyV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormationError {
    UnsupportedVersion(u32),
    EmptyId,
    EmptyRoles,
    DuplicateMember(AgentId),
    MissingLeader(AgentId),
    EmptyRole(AgentId),
    InvalidSeparation,
}

impl fmt::Display for FormationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported formation schema version {version}")
            }
            Self::EmptyId => write!(f, "formation ID must be non-empty"),
            Self::EmptyRoles => write!(f, "formation requires at least one role"),
            Self::DuplicateMember(agent_id) => {
                write!(f, "formation member {agent_id:?} appears more than once")
            }
            Self::MissingLeader(agent_id) => {
                write!(f, "formation leader {agent_id:?} is not a role member")
            }
            Self::EmptyRole(agent_id) => {
                write!(f, "formation role for {agent_id:?} must be non-empty")
            }
            Self::InvalidSeparation => write!(f, "formation separation must be positive"),
        }
    }
}

impl Error for FormationError {}

#[derive(Clone, Debug, PartialEq)]
pub struct FormationReportV1 {
    pub split: bool,
    pub missing_members: usize,
    pub maximum_separation_m: f32,
    pub farthest_member: Option<AgentId>,
    pub intruder_agent_ids: Vec<AgentId>,
}

impl FormationV1 {
    pub fn new(
        id: impl Into<String>,
        leader_agent_id: AgentId,
        roles: Vec<FormationRoleV1>,
        max_separation_millimeters: u32,
        split_policy: FormationSplitPolicyV1,
    ) -> Result<Self, FormationError> {
        let formation = Self {
            schema_version: FORMATION_SCHEMA_VERSION,
            id: id.into(),
            leader_agent_id,
            roles,
            max_separation_millimeters,
            split_policy,
        };
        formation.validate()?;
        Ok(formation)
    }

    pub fn validate(&self) -> Result<(), FormationError> {
        if self.schema_version != FORMATION_SCHEMA_VERSION {
            return Err(FormationError::UnsupportedVersion(self.schema_version));
        }
        if self.id.is_empty() {
            return Err(FormationError::EmptyId);
        }
        if self.roles.is_empty() {
            return Err(FormationError::EmptyRoles);
        }
        if self.max_separation_millimeters == 0 {
            return Err(FormationError::InvalidSeparation);
        }
        let mut members = BTreeSet::new();
        for role in &self.roles {
            if role.role.is_empty() {
                return Err(FormationError::EmptyRole(role.agent_id));
            }
            if !members.insert(role.agent_id) {
                return Err(FormationError::DuplicateMember(role.agent_id));
            }
        }
        if !members.contains(&self.leader_agent_id) {
            return Err(FormationError::MissingLeader(self.leader_agent_id));
        }
        Ok(())
    }

    pub fn offset_for(&self, agent_id: AgentId) -> Option<Vec2> {
        self.roles
            .iter()
            .find(|role| role.agent_id == agent_id)
            .map(|role| {
                Vec2::new(
                    role.offset_millimeters[0] as f32 / 1_000.0,
                    role.offset_millimeters[1] as f32 / 1_000.0,
                )
            })
    }

    pub fn evaluate(
        &self,
        positions: &BTreeMap<AgentId, Vec2>,
        candidates: &[(AgentId, Vec2)],
    ) -> FormationReportV1 {
        let leader_position = positions.get(&self.leader_agent_id).copied();
        let mut missing_members = 0;
        let mut maximum_separation_m = 0.0;
        let mut farthest_member = None;
        if let Some(leader_position) = leader_position {
            for role in &self.roles {
                let Some(position) = positions.get(&role.agent_id).copied() else {
                    missing_members += 1;
                    continue;
                };
                let separation = leader_position.distance_squared(position).sqrt();
                if separation > maximum_separation_m {
                    maximum_separation_m = separation;
                    farthest_member = Some(role.agent_id);
                }
            }
        } else {
            missing_members = self.roles.len();
        }
        let member_positions: Vec<_> = self
            .roles
            .iter()
            .filter_map(|role| positions.get(&role.agent_id).copied())
            .collect();
        let mut intruders: Vec<_> = candidates
            .iter()
            .filter(|(agent_id, _)| !self.roles.iter().any(|role| role.agent_id == *agent_id))
            .filter(|(_, position)| {
                member_positions
                    .iter()
                    .any(|member| member.distance_squared(*position).sqrt() <= 1.0)
            })
            .map(|(agent_id, _)| *agent_id)
            .collect();
        intruders.sort_unstable();
        intruders.dedup();
        FormationReportV1 {
            split: missing_members > 0
                || maximum_separation_m > self.max_separation_millimeters as f32 / 1_000.0,
            missing_members,
            maximum_separation_m,
            farthest_member,
            intruder_agent_ids: intruders,
        }
    }

    pub fn cohesion_velocity(
        &self,
        agent_id: AgentId,
        positions: &BTreeMap<AgentId, Vec2>,
        max_correction_mps: f32,
    ) -> Vec2 {
        let Some(current) = positions.get(&agent_id).copied() else {
            return Vec2::ZERO;
        };
        let Some(leader) = positions.get(&self.leader_agent_id).copied() else {
            return Vec2::ZERO;
        };
        let Some(offset) = self.offset_for(agent_id) else {
            return Vec2::ZERO;
        };
        (leader + offset - current).clamp_length(max_correction_mps)
    }
}
