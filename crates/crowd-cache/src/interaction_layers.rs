//! Sparse, removable animation layers for promoted M6 interaction groups.
//!
//! These layers are adjacent to the immutable base cache. Composition changes a
//! cloned frame only; it never rewrites the base frame or expands the target
//! set beyond the stable IDs declared by the layer.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Frame, FrameRecord};

pub const INTERACTION_LAYER_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationEditV1 {
    pub agent_id: u64,
    pub tick: u64,
    pub clip_id: u16,
    pub phase_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackClipV1 {
    pub clip_set_id: String,
    pub clip_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationLayerV1 {
    pub schema_version: u32,
    pub layer_id: String,
    pub interaction_id: String,
    pub base_cache_hash: String,
    pub target_agent_ids: Vec<u64>,
    pub tick_start: u64,
    pub tick_end: u64,
    pub priority: i32,
    pub enabled: bool,
    pub provenance: String,
    pub edits: Vec<AnimationEditV1>,
    pub fallback: FallbackClipV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InteractionLayerError {
    UnsupportedVersion(u32),
    EmptyField(&'static str),
    InvalidHash,
    EmptyTargets,
    DuplicateTarget(u64),
    InvalidTickRange,
    EmptyEdits,
    EditOutsideTarget(u64),
    EditOutsideRange(u64),
    DuplicateEdit(u64, u64),
    InvalidPhase(u32),
    MissingFallback,
    CrossCache {
        layer: String,
        expected: String,
        found: String,
    },
    UnknownAgent(u64),
}

impl fmt::Display for InteractionLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported interaction layer version {version}")
            }
            Self::EmptyField(field) => {
                write!(f, "interaction layer field {field} must be non-empty")
            }
            Self::InvalidHash => write!(
                f,
                "interaction layer base cache hash must be 64 lowercase hex characters"
            ),
            Self::EmptyTargets => {
                write!(f, "interaction layer must target at least one stable agent")
            }
            Self::DuplicateTarget(agent_id) => write!(
                f,
                "interaction layer targets agent {agent_id} more than once"
            ),
            Self::InvalidTickRange => {
                write!(f, "interaction layer tick_start must not be after tick_end")
            }
            Self::EmptyEdits => write!(f, "interaction layer must contain at least one edit"),
            Self::EditOutsideTarget(agent_id) => {
                write!(f, "interaction edit targets undeclared agent {agent_id}")
            }
            Self::EditOutsideRange(tick) => write!(
                f,
                "interaction edit tick {tick} is outside the layer interval"
            ),
            Self::DuplicateEdit(agent_id, tick) => write!(
                f,
                "interaction edit for agent {agent_id} at tick {tick} is duplicated"
            ),
            Self::InvalidPhase(phase) => write!(
                f,
                "interaction phase {phase} is greater than one millionth scale"
            ),
            Self::MissingFallback => {
                write!(f, "interaction layer must declare a deterministic fallback")
            }
            Self::CrossCache {
                layer,
                expected,
                found,
            } => write!(
                f,
                "layer {layer} belongs to another base cache: expected {expected}, found {found}"
            ),
            Self::UnknownAgent(agent_id) => write!(
                f,
                "interaction layer references agent {agent_id} missing from the base frame"
            ),
        }
    }
}

impl Error for InteractionLayerError {}

impl AnimationLayerV1 {
    pub fn validate(&self) -> Result<(), InteractionLayerError> {
        if self.schema_version != INTERACTION_LAYER_SCHEMA_VERSION {
            return Err(InteractionLayerError::UnsupportedVersion(
                self.schema_version,
            ));
        }
        for (field, value) in [
            ("layer_id", self.layer_id.as_str()),
            ("interaction_id", self.interaction_id.as_str()),
            ("provenance", self.provenance.as_str()),
        ] {
            if value.is_empty() {
                return Err(InteractionLayerError::EmptyField(field));
            }
        }
        if !is_hash(&self.base_cache_hash) {
            return Err(InteractionLayerError::InvalidHash);
        }
        if self.target_agent_ids.is_empty() {
            return Err(InteractionLayerError::EmptyTargets);
        }
        let mut targets = BTreeSet::new();
        for agent_id in &self.target_agent_ids {
            if !targets.insert(*agent_id) {
                return Err(InteractionLayerError::DuplicateTarget(*agent_id));
            }
        }
        if self.tick_start > self.tick_end {
            return Err(InteractionLayerError::InvalidTickRange);
        }
        if self.edits.is_empty() {
            return Err(InteractionLayerError::EmptyEdits);
        }
        let mut edits = BTreeSet::new();
        for edit in &self.edits {
            if !targets.contains(&edit.agent_id) {
                return Err(InteractionLayerError::EditOutsideTarget(edit.agent_id));
            }
            if edit.tick < self.tick_start || edit.tick > self.tick_end {
                return Err(InteractionLayerError::EditOutsideRange(edit.tick));
            }
            if !edits.insert((edit.agent_id, edit.tick)) {
                return Err(InteractionLayerError::DuplicateEdit(
                    edit.agent_id,
                    edit.tick,
                ));
            }
            if edit.phase_millionths > 1_000_000 {
                return Err(InteractionLayerError::InvalidPhase(edit.phase_millionths));
            }
        }
        if self.fallback.clip_set_id.is_empty()
            || self.fallback.clip_id.is_empty()
            || self.fallback.reason.is_empty()
        {
            return Err(InteractionLayerError::MissingFallback);
        }
        Ok(())
    }
}

pub fn compose_interaction_frame_v1(
    base: &Frame,
    tick: u64,
    base_cache_hash: &str,
    layers: &[AnimationLayerV1],
) -> Result<Frame, InteractionLayerError> {
    let mut active: Vec<&AnimationLayerV1> = Vec::new();
    for layer in layers {
        layer.validate()?;
        if layer.base_cache_hash != base_cache_hash {
            return Err(InteractionLayerError::CrossCache {
                layer: layer.layer_id.clone(),
                expected: base_cache_hash.to_owned(),
                found: layer.base_cache_hash.clone(),
            });
        }
        if layer.enabled && tick >= layer.tick_start && tick <= layer.tick_end {
            active.push(layer);
        }
    }
    active.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.layer_id.cmp(&right.layer_id))
    });

    let mut result = base.clone();
    for layer in active {
        for edit in layer.edits.iter().filter(|edit| edit.tick == tick) {
            let Some(record) = result
                .records
                .iter_mut()
                .find(|record| record.agent_id == edit.agent_id)
            else {
                return Err(InteractionLayerError::UnknownAgent(edit.agent_id));
            };
            apply_animation_edit(record, edit);
        }
    }
    Ok(result)
}

fn apply_animation_edit(record: &mut FrameRecord, edit: &AnimationEditV1) {
    record.clip_id = edit.clip_id;
    record.phase = edit.phase_millionths as f32 / 1_000_000.0;
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
