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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OverrideEditV2 {
    Visibility {
        tick_start: u64,
        tick_end: u64,
        visible: bool,
    },
    Transform {
        tick_start: u64,
        tick_end: u64,
        operation: OverrideOperation,
        samples: Vec<TransformOverride>,
    },
    Timing {
        tick_start: u64,
        tick_end: u64,
        offset_ticks: i64,
    },
    Speed {
        tick_start: u64,
        tick_end: u64,
        multiplier_millionths: u32,
    },
    Appearance {
        tick_start: u64,
        tick_end: u64,
        variant_id: u32,
    },
    Animation {
        tick_start: u64,
        tick_end: u64,
        clip_id: u16,
        phase_millionths: u32,
    },
    Goal {
        tick_start: u64,
        tick_end: u64,
        destination_id: u32,
    },
    Hero {
        tick_start: u64,
        tick_end: u64,
        render_tier: u8,
    },
}

impl OverrideEditV2 {
    fn range(&self) -> (u64, u64) {
        match self {
            Self::Visibility {
                tick_start,
                tick_end,
                ..
            }
            | Self::Transform {
                tick_start,
                tick_end,
                ..
            }
            | Self::Timing {
                tick_start,
                tick_end,
                ..
            }
            | Self::Speed {
                tick_start,
                tick_end,
                ..
            }
            | Self::Appearance {
                tick_start,
                tick_end,
                ..
            }
            | Self::Animation {
                tick_start,
                tick_end,
                ..
            }
            | Self::Goal {
                tick_start,
                tick_end,
                ..
            }
            | Self::Hero {
                tick_start,
                tick_end,
                ..
            } => (*tick_start, *tick_end),
        }
    }
    fn channel(&self) -> &'static str {
        match self {
            Self::Visibility { .. } => "visibility",
            Self::Transform { .. } => "transform",
            Self::Timing { .. } => "timing",
            Self::Speed { .. } => "speed",
            Self::Appearance { .. } => "appearance",
            Self::Animation { .. } => "animation",
            Self::Goal { .. } => "goal",
            Self::Hero { .. } => "hero",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalResimulationRecordV2 {
    pub affected_agent_ids: Vec<u64>,
    pub tick_start: u64,
    pub tick_end: u64,
    pub source_base_hash: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideLayerV2 {
    pub schema_version: u32,
    pub layer_id: String,
    pub author: String,
    pub created_at: String,
    pub priority: i32,
    pub enabled: bool,
    pub target_agent_id: u64,
    pub edits: Vec<OverrideEditV2>,
    pub local_resimulation: Option<LocalResimulationRecordV2>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComposedRecordV2 {
    pub agent_id: u64,
    pub position: [f32; 3],
    pub playback_rate: f32,
    pub variant_id: u32,
    pub clip_id: u16,
    pub phase: f32,
    pub destination_id: u32,
    pub visible: bool,
    pub render_tier: u8,
    pub time_offset_ticks: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideConflictV2 {
    pub target_agent_id: u64,
    pub channel: String,
    pub earlier_layer_id: String,
    pub later_layer_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComposedFrameV2 {
    pub records: Vec<ComposedRecordV2>,
    pub conflicts: Vec<OverrideConflictV2>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideV2Error(pub String);
impl fmt::Display for OverrideV2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for OverrideV2Error {}

pub fn compose_frame_v2(
    frame: &Frame,
    tick: u64,
    layers: &[OverrideLayerV2],
) -> Result<ComposedFrameV2, OverrideV2Error> {
    let mut index_by_id = BTreeMap::new();
    let mut records: Vec<_> = frame
        .records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            if index_by_id.insert(record.agent_id, index).is_some() {
                return Err(OverrideV2Error(format!(
                    "duplicate base agent {}",
                    record.agent_id
                )));
            }
            Ok(ComposedRecordV2 {
                agent_id: record.agent_id,
                position: [record.position[0], record.position[1], 0.0],
                playback_rate: record.playback_rate,
                variant_id: record.variant_id,
                clip_id: record.clip_id,
                phase: record.phase,
                destination_id: record.destination_id,
                visible: record.visible,
                render_tier: record.render_tier,
                time_offset_ticks: 0,
            })
        })
        .collect::<Result<_, _>>()?;
    let mut ordered: Vec<_> = layers.iter().filter(|layer| layer.enabled).collect();
    ordered
        .sort_by(|a, b| (a.priority, a.layer_id.as_str()).cmp(&(b.priority, b.layer_id.as_str())));
    let mut channels: BTreeMap<(u64, &'static str), &str> = BTreeMap::new();
    let mut conflicts = Vec::new();
    for layer in ordered {
        validate_layer_v2(layer, &index_by_id)?;
        let record = &mut records[index_by_id[&layer.target_agent_id]];
        for edit in &layer.edits {
            let (start, end) = edit.range();
            if !(start..=end).contains(&tick) {
                continue;
            }
            let key = (layer.target_agent_id, edit.channel());
            if let Some(previous) = channels.insert(key, &layer.layer_id) {
                conflicts.push(OverrideConflictV2 {
                    target_agent_id: layer.target_agent_id,
                    channel: edit.channel().to_string(),
                    earlier_layer_id: previous.to_string(),
                    later_layer_id: layer.layer_id.clone(),
                });
            }
            match edit {
                OverrideEditV2::Visibility { visible, .. } => record.visible = *visible,
                OverrideEditV2::Transform {
                    operation, samples, ..
                } => {
                    let value = sample_v2(samples, tick);
                    match operation {
                        OverrideOperation::Additive => {
                            for (v, d) in record.position.iter_mut().zip(value) {
                                *v += d;
                            }
                        }
                        OverrideOperation::Absolute => record.position = value,
                    }
                }
                OverrideEditV2::Timing { offset_ticks, .. } => {
                    record.time_offset_ticks = *offset_ticks
                }
                OverrideEditV2::Speed {
                    multiplier_millionths,
                    ..
                } => record.playback_rate *= *multiplier_millionths as f32 / 1_000_000.0,
                OverrideEditV2::Appearance { variant_id, .. } => record.variant_id = *variant_id,
                OverrideEditV2::Animation {
                    clip_id,
                    phase_millionths,
                    ..
                } => {
                    record.clip_id = *clip_id;
                    record.phase = *phase_millionths as f32 / 1_000_000.0;
                }
                OverrideEditV2::Goal { destination_id, .. } => {
                    record.destination_id = *destination_id
                }
                OverrideEditV2::Hero { render_tier, .. } => record.render_tier = *render_tier,
            }
        }
    }
    Ok(ComposedFrameV2 { records, conflicts })
}

fn validate_layer_v2(
    layer: &OverrideLayerV2,
    ids: &BTreeMap<u64, usize>,
) -> Result<(), OverrideV2Error> {
    if layer.schema_version != 2
        || layer.layer_id.trim().is_empty()
        || layer.author.trim().is_empty()
        || layer.created_at.trim().is_empty()
        || layer.edits.is_empty()
    {
        return Err(OverrideV2Error(format!(
            "override layer {} is invalid",
            layer.layer_id
        )));
    }
    if !ids.contains_key(&layer.target_agent_id) {
        return Err(OverrideV2Error(format!(
            "override layer {} targets absent agent {}",
            layer.layer_id, layer.target_agent_id
        )));
    }
    for edit in &layer.edits {
        let (start, end) = edit.range();
        if start > end {
            return Err(OverrideV2Error(format!(
                "override layer {} has invalid edit range",
                layer.layer_id
            )));
        }
        if let OverrideEditV2::Transform { samples, .. } = edit {
            if samples.is_empty()
                || samples.iter().any(|sample| {
                    sample.tick < start
                        || sample.tick > end
                        || !sample.translation.iter().all(|v| v.is_finite())
                })
            {
                return Err(OverrideV2Error(format!(
                    "override layer {} has invalid transform samples",
                    layer.layer_id
                )));
            }
        }
        if let OverrideEditV2::Speed {
            multiplier_millionths: 0,
            ..
        } = edit
        {
            return Err(OverrideV2Error(format!(
                "override layer {} has zero speed",
                layer.layer_id
            )));
        }
        if let OverrideEditV2::Animation {
            phase_millionths, ..
        } = edit
        {
            if *phase_millionths > 1_000_000 {
                return Err(OverrideV2Error(format!(
                    "override layer {} has invalid phase",
                    layer.layer_id
                )));
            }
        }
    }
    if let Some(local) = &layer.local_resimulation {
        if local.affected_agent_ids.is_empty()
            || local.tick_start > local.tick_end
            || local.source_base_hash.len() != 64
            || !local
                .source_base_hash
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(OverrideV2Error(format!(
                "override layer {} has invalid local resimulation provenance",
                layer.layer_id
            )));
        }
    }
    Ok(())
}

fn sample_v2(samples: &[TransformOverride], tick: u64) -> [f32; 3] {
    let upper = samples.partition_point(|sample| sample.tick < tick);
    if upper == 0 {
        return samples[0].translation;
    }
    if upper == samples.len() {
        return samples[samples.len() - 1].translation;
    }
    if samples[upper].tick == tick {
        return samples[upper].translation;
    }
    let before = &samples[upper - 1];
    let after = &samples[upper];
    let fraction = (tick - before.tick) as f32 / (after.tick - before.tick) as f32;
    std::array::from_fn(|axis| {
        before.translation[axis] + (after.translation[axis] - before.translation[axis]) * fraction
    })
}
