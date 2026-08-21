//! Optional physics/recovery ownership and hero-solver declarations.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const PHYSICS_TRANSITION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicyV1 {
    Fallback,
    Reject,
    Hold,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsTransitionV1 {
    pub schema_version: u32,
    pub transition_id: String,
    pub agent_ids: Vec<u64>,
    pub tick_start: u64,
    pub tick_end: u64,
    pub solver: String,
    pub cache_hash: String,
    pub recovery: String,
    pub failure_policy: FailurePolicyV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeroIntegrationBoundaryV1 {
    pub integration_id: String,
    pub solver: String,
    pub cache_policy: String,
    pub supported_render_tiers: Vec<String>,
    pub failure_policy: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryPhaseV1 {
    Impact,
    Stabilize,
    Resume,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigidBodyLayerV1 {
    pub layer_id: String,
    pub owner_agent_ids: Vec<u64>,
    pub solver: String,
    pub collision_masks: Vec<String>,
    pub recovery_transition_id: String,
}

impl RigidBodyLayerV1 {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.layer_id.is_empty() {
            errors.push("rigid-body layer ID must be non-empty".to_owned());
        }
        if self.owner_agent_ids.is_empty() {
            errors.push("rigid-body layer must declare owner agents".to_owned());
        }
        if self.solver.is_empty() {
            errors.push("rigid-body layer solver must be declared".to_owned());
        }
        if self.collision_masks.is_empty() || self.collision_masks.iter().any(String::is_empty) {
            errors.push("rigid-body layer collision masks must be declared".to_owned());
        }
        if self.recovery_transition_id.is_empty() {
            errors.push("rigid-body layer must declare a recovery transition".to_owned());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

pub fn recovery_phase(
    transition: &PhysicsTransitionV1,
    tick: u64,
    stabilize_ticks: u64,
) -> RecoveryPhaseV1 {
    if tick <= transition.tick_start {
        RecoveryPhaseV1::Impact
    } else if tick < transition.tick_start.saturating_add(stabilize_ticks) {
        RecoveryPhaseV1::Stabilize
    } else {
        RecoveryPhaseV1::Resume
    }
}

pub fn validate_transition(transition: &PhysicsTransitionV1) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if transition.schema_version != PHYSICS_TRANSITION_SCHEMA_VERSION {
        errors.push(format!(
            "unsupported physics transition version {}",
            transition.schema_version
        ));
    }
    if transition.transition_id.is_empty() {
        errors.push("transition ID must be non-empty".to_owned());
    }
    if transition.agent_ids.is_empty() {
        errors.push("physics transition must target at least one agent".to_owned());
    }
    let mut ids = BTreeSet::new();
    if transition
        .agent_ids
        .iter()
        .any(|agent_id| !ids.insert(agent_id))
    {
        errors.push("physics transition agent IDs must be unique".to_owned());
    }
    if transition.tick_start > transition.tick_end {
        errors.push("physics transition tick range is invalid".to_owned());
    }
    if transition.solver.is_empty() {
        errors.push("physics transition solver must be declared".to_owned());
    }
    if transition.recovery.is_empty() {
        errors.push("physics transition recovery must be declared".to_owned());
    }
    if !is_hash(&transition.cache_hash) {
        errors.push("physics transition cache hash must be 64 lowercase hex characters".to_owned());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

impl HeroIntegrationBoundaryV1 {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.integration_id.is_empty() {
            errors.push("hero integration ID must be non-empty".to_owned());
        }
        if self.solver.is_empty() {
            errors.push("hero solver must be declared".to_owned());
        }
        if self.cache_policy.is_empty() {
            errors.push("hero cache policy must be declared".to_owned());
        }
        if self.supported_render_tiers.is_empty()
            || self.supported_render_tiers.iter().any(String::is_empty)
        {
            errors.push("hero integration must declare supported render tiers".to_owned());
        }
        if self.failure_policy.is_empty() {
            errors.push("hero failure policy must be declared".to_owned());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
