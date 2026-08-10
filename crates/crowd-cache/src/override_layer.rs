//! Versioned, sparse transform layers composed without mutating a base cache.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Frame;

pub const OVERRIDE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideOperation {
    Additive,
    Absolute,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransformOverride {
    pub tick: u64,
    pub translation: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideLayerV1 {
    pub schema_version: u32,
    pub layer_id: String,
    pub author: String,
    pub created_at: String,
    pub priority: i32,
    pub enabled: bool,
    pub target_agent_id: u64,
    pub tick_start: u64,
    pub tick_end: u64,
    pub operation: OverrideOperation,
    pub samples: Vec<TransformOverride>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComposedRecord {
    pub agent_id: u64,
    pub position: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComposedFrame {
    pub records: Vec<ComposedRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OverrideError {
    UnsupportedVersion {
        layer_id: String,
        found: u32,
    },
    EmptyLayerId,
    DuplicateLayerId(String),
    EmptyAuthor(String),
    EmptyTimestamp(String),
    InvalidTickRange {
        layer_id: String,
        start: u64,
        end: u64,
    },
    NoSamples(String),
    SampleOutsideRange {
        layer_id: String,
        tick: u64,
    },
    SamplesNotStrictlyOrdered(String),
    NonFiniteTranslation {
        layer_id: String,
        tick: u64,
    },
    TargetNotFound {
        layer_id: String,
        agent_id: u64,
    },
    DuplicateBaseAgent(u64),
}

impl fmt::Display for OverrideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { layer_id, found } => write!(
                formatter,
                "override layer {layer_id} uses schema {found}; expected {OVERRIDE_SCHEMA_VERSION}"
            ),
            Self::EmptyLayerId => write!(formatter, "override layer ID must not be empty"),
            Self::DuplicateLayerId(layer_id) => {
                write!(formatter, "duplicate override layer ID {layer_id}")
            }
            Self::EmptyAuthor(layer_id) => {
                write!(formatter, "override layer {layer_id} has no author")
            }
            Self::EmptyTimestamp(layer_id) => {
                write!(formatter, "override layer {layer_id} has no timestamp")
            }
            Self::InvalidTickRange {
                layer_id,
                start,
                end,
            } => write!(
                formatter,
                "override layer {layer_id} has invalid tick range {start}..={end}"
            ),
            Self::NoSamples(layer_id) => {
                write!(
                    formatter,
                    "override layer {layer_id} has no transform samples"
                )
            }
            Self::SampleOutsideRange { layer_id, tick } => write!(
                formatter,
                "override layer {layer_id} sample {tick} is outside its tick range"
            ),
            Self::SamplesNotStrictlyOrdered(layer_id) => write!(
                formatter,
                "override layer {layer_id} samples must have strictly increasing ticks"
            ),
            Self::NonFiniteTranslation { layer_id, tick } => write!(
                formatter,
                "override layer {layer_id} sample {tick} has a non-finite translation"
            ),
            Self::TargetNotFound { layer_id, agent_id } => write!(
                formatter,
                "override layer {layer_id} targets absent stable agent ID {agent_id}"
            ),
            Self::DuplicateBaseAgent(agent_id) => {
                write!(
                    formatter,
                    "base frame contains duplicate stable agent ID {agent_id}"
                )
            }
        }
    }
}

impl Error for OverrideError {}

pub fn validate_layers(frame: &Frame, layers: &[OverrideLayerV1]) -> Result<(), OverrideError> {
    let mut base_ids = BTreeMap::new();
    for (index, record) in frame.records.iter().enumerate() {
        if base_ids.insert(record.agent_id, index).is_some() {
            return Err(OverrideError::DuplicateBaseAgent(record.agent_id));
        }
    }
    let mut layer_ids = BTreeSet::new();
    for layer in layers {
        validate_layer(layer)?;
        if !layer_ids.insert(layer.layer_id.as_str()) {
            return Err(OverrideError::DuplicateLayerId(layer.layer_id.clone()));
        }
        if !base_ids.contains_key(&layer.target_agent_id) {
            return Err(OverrideError::TargetNotFound {
                layer_id: layer.layer_id.clone(),
                agent_id: layer.target_agent_id,
            });
        }
    }
    Ok(())
}

pub fn compose_frame(
    frame: &Frame,
    tick: u64,
    layers: &[OverrideLayerV1],
) -> Result<ComposedFrame, OverrideError> {
    validate_layers(frame, layers)?;
    let mut index_by_id = BTreeMap::new();
    let mut records: Vec<_> = frame
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            index_by_id.insert(record.agent_id, index);
            ComposedRecord {
                agent_id: record.agent_id,
                position: [record.position[0], record.position[1], 0.0],
            }
        })
        .collect();
    let mut ordered: Vec<_> = layers
        .iter()
        .filter(|layer| layer.enabled && (layer.tick_start..=layer.tick_end).contains(&tick))
        .collect();
    ordered.sort_by(|left, right| {
        (left.priority, left.layer_id.as_str()).cmp(&(right.priority, right.layer_id.as_str()))
    });
    for layer in ordered {
        let translation = sample_translation(layer, tick);
        let record = &mut records[index_by_id[&layer.target_agent_id]];
        match layer.operation {
            OverrideOperation::Additive => {
                for (component, offset) in record.position.iter_mut().zip(translation) {
                    *component += offset;
                }
            }
            OverrideOperation::Absolute => record.position = translation,
        }
    }
    Ok(ComposedFrame { records })
}

fn validate_layer(layer: &OverrideLayerV1) -> Result<(), OverrideError> {
    if layer.schema_version != OVERRIDE_SCHEMA_VERSION {
        return Err(OverrideError::UnsupportedVersion {
            layer_id: layer.layer_id.clone(),
            found: layer.schema_version,
        });
    }
    if layer.layer_id.trim().is_empty() {
        return Err(OverrideError::EmptyLayerId);
    }
    if layer.author.trim().is_empty() {
        return Err(OverrideError::EmptyAuthor(layer.layer_id.clone()));
    }
    if layer.created_at.trim().is_empty() {
        return Err(OverrideError::EmptyTimestamp(layer.layer_id.clone()));
    }
    if layer.tick_start > layer.tick_end {
        return Err(OverrideError::InvalidTickRange {
            layer_id: layer.layer_id.clone(),
            start: layer.tick_start,
            end: layer.tick_end,
        });
    }
    if layer.samples.is_empty() {
        return Err(OverrideError::NoSamples(layer.layer_id.clone()));
    }
    let mut previous = None;
    for sample in &layer.samples {
        if !(layer.tick_start..=layer.tick_end).contains(&sample.tick) {
            return Err(OverrideError::SampleOutsideRange {
                layer_id: layer.layer_id.clone(),
                tick: sample.tick,
            });
        }
        if previous.is_some_and(|tick| sample.tick <= tick) {
            return Err(OverrideError::SamplesNotStrictlyOrdered(
                layer.layer_id.clone(),
            ));
        }
        if !sample.translation.iter().all(|value| value.is_finite()) {
            return Err(OverrideError::NonFiniteTranslation {
                layer_id: layer.layer_id.clone(),
                tick: sample.tick,
            });
        }
        previous = Some(sample.tick);
    }
    Ok(())
}

fn sample_translation(layer: &OverrideLayerV1, tick: u64) -> [f32; 3] {
    let upper = layer.samples.partition_point(|sample| sample.tick < tick);
    if upper == 0 {
        return layer.samples[0].translation;
    }
    if upper == layer.samples.len() {
        return layer.samples[layer.samples.len() - 1].translation;
    }
    if layer.samples[upper].tick == tick {
        return layer.samples[upper].translation;
    }
    let before = &layer.samples[upper - 1];
    let after = &layer.samples[upper];
    let fraction = (tick - before.tick) as f32 / (after.tick - before.tick) as f32;
    std::array::from_fn(|axis| {
        before.translation[axis] + (after.translation[axis] - before.translation[axis]) * fraction
    })
}
