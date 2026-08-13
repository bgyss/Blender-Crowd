//! M4 non-destructive crowd layout composition.
//!
//! This is deliberately a cache-side data contract.  Blender and interchange
//! adapters may present or serialize the result, but neither is allowed to
//! mutate the base cache to make a directed shot.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Frame, FrameRecord, OverrideLayerV1, OverrideOperation, TransformOverride};

pub const LAYOUT_LAYER_SCHEMA_VERSION: u32 = 1;
pub const USD_CROWD_PROFILE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKindV1 {
    BaseSimulation,
    Layout,
    AnimationFix,
    Hero,
    Physics,
    Shot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayerTargetV1 {
    pub agent_ids: Vec<u64>,
    pub tick_start: u64,
    pub tick_end: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsSampleV1 {
    pub tick: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsHandoffSpecV1 {
    pub tick_start: u64,
    pub tick_end: u64,
    pub ticks_per_second: u32,
    pub incoming_position: [f32; 3],
    pub incoming_velocity: [f32; 3],
    pub gravity_mps2: f32,
    pub floor_z: f32,
    pub restitution_millionths: u32,
    pub collision_masks: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProceduralPrototypeV1 {
    pub prototype_id: String,
    pub material_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProceduralInstanceV1 {
    pub agent_id: u64,
    pub prototype_id: String,
    pub material_id: String,
    pub position: [f32; 3],
    pub clip_id: u16,
    pub phase: f32,
    pub render_tier: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsdCrowdImportV1 {
    pub base_cache_hash: String,
    pub agent_ids: Vec<u64>,
    pub positions: Vec<[f32; 3]>,
    pub variant_ids: Vec<u32>,
}

/// Explicit bounded replacement scope for a locally recomputed trajectory.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalResimulationV1 {
    pub affected_agent_ids: Vec<u64>,
    pub tick_start: u64,
    pub tick_end: u64,
    pub source_base_hash: String,
    pub reason: String,
}

/// Bounded local kinematic re-simulation request. Its outputs are explicit
/// transform samples for a layer; the immutable base simulation is untouched.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalResimulationRequestV1 {
    pub tick_start: u64,
    pub tick_end: u64,
    pub ticks_per_second: u32,
    pub incoming_position: [f32; 3],
    pub incoming_velocity: [f32; 3],
    pub target_position: [f32; 3],
    pub max_speed_mps: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayoutEditV1 {
    Transform {
        operation: OverrideOperation,
        samples: Vec<TransformOverride>,
    },
    Visibility {
        visible: bool,
    },
    Freeze {
        position: [f32; 3],
    },
    Timing {
        offset_ticks: i64,
    },
    Speed {
        multiplier_millionths: u32,
    },
    PathGuide {
        guide_id: String,
    },
    RegionDensity {
        region_id: String,
        density_millionths: u32,
    },
    CurveRetiming {
        curve_id: String,
        offset_ticks: i64,
    },
    Goal {
        destination_id: u32,
    },
    Appearance {
        variant_id: u32,
    },
    Animation {
        clip_id: u16,
        phase_millionths: u32,
    },
    RenderTier {
        tier: u8,
    },
    Group {
        group_id: String,
    },
    PhysicsHandoff {
        collision_masks: Vec<String>,
        incoming_position: [f32; 3],
        incoming_velocity: [f32; 3],
        cached_samples: Vec<PhysicsSampleV1>,
        recovery_tick: u64,
    },
}

impl LayoutEditV1 {
    fn channel(&self) -> &'static str {
        match self {
            Self::Transform { .. } | Self::Freeze { .. } => "transform",
            Self::Visibility { .. } => "visibility",
            Self::Timing { .. } => "timing",
            Self::Speed { .. } => "speed",
            Self::PathGuide { .. } | Self::Goal { .. } => "trajectory",
            Self::RegionDensity { .. } => "density",
            Self::CurveRetiming { .. } => "timing",
            Self::Appearance { .. } => "appearance",
            Self::Animation { .. } => "animation",
            Self::RenderTier { .. } => "render_tier",
            Self::Group { .. } => "group",
            Self::PhysicsHandoff { .. } => "physics",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutLayerV1 {
    pub schema_version: u32,
    pub layer_id: String,
    pub kind: LayerKindV1,
    /// Explicit UI ordering. Lower values compose first; priority breaks ties.
    pub order: u32,
    pub priority: i32,
    pub muted: bool,
    pub solo: bool,
    pub author: String,
    pub created_at: String,
    pub base_cache_hash: String,
    pub provenance: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Set by the invalidation API when one of `dependencies` changes. A stale
    /// layer is a hard composition error until it is recomputed or muted.
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub local_resimulation: Option<LocalResimulationV1>,
    pub target: LayerTargetV1,
    pub edits: Vec<LayoutEditV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LayoutRecordV1 {
    pub agent_id: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub visible: bool,
    pub playback_rate: f32,
    pub variant_id: u32,
    pub clip_id: u16,
    pub phase: f32,
    pub destination_id: u32,
    pub render_tier: u8,
    pub time_offset_ticks: i64,
    pub frozen: bool,
    pub path_guide: Option<String>,
    pub group_id: Option<String>,
    pub physics_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LayoutConflictV1 {
    pub agent_id: u64,
    pub channel: String,
    pub earlier_layer_id: String,
    pub later_layer_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ComposedLayoutFrameV1 {
    pub records: Vec<LayoutRecordV1>,
    pub conflicts: Vec<LayoutConflictV1>,
    pub active_layer_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerInvalidationV1 {
    pub layer_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutErrorV1(pub String);
impl fmt::Display for LayoutErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for LayoutErrorV1 {}

pub fn compose_layout_frame_v1(
    base: &Frame,
    tick: u64,
    base_cache_hash: &str,
    layers: &[LayoutLayerV1],
) -> Result<ComposedLayoutFrameV1, LayoutErrorV1> {
    let mut index = BTreeMap::new();
    let mut records = Vec::with_capacity(base.records.len());
    for (position, record) in base.records.iter().enumerate() {
        if index.insert(record.agent_id, position).is_some() {
            return Err(LayoutErrorV1(format!(
                "base has duplicate agent {}",
                record.agent_id
            )));
        }
        records.push(layout_record(record));
    }
    let solo_ids: BTreeSet<_> = layers
        .iter()
        .filter(|l| l.solo && !l.muted)
        .map(|l| l.layer_id.as_str())
        .collect();
    let mut ordered: Vec<_> = layers
        .iter()
        .filter(|l| !l.muted && (solo_ids.is_empty() || solo_ids.contains(l.layer_id.as_str())))
        .collect();
    ordered.sort_by(|a, b| {
        (a.order, a.priority, a.layer_id.as_str()).cmp(&(b.order, b.priority, b.layer_id.as_str()))
    });
    let mut channels = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut active_layer_ids = Vec::new();
    for layer in ordered {
        validate_layer(layer, base_cache_hash, &index)?;
        active_layer_ids.push(layer.layer_id.clone());
        if !(layer.target.tick_start..=layer.target.tick_end).contains(&tick) {
            continue;
        }
        for agent_id in &layer.target.agent_ids {
            let record = &mut records[index[agent_id]];
            for edit in &layer.edits {
                let key = (*agent_id, edit.channel());
                if let Some(previous) = channels.insert(key, layer.layer_id.as_str()) {
                    conflicts.push(LayoutConflictV1 {
                        agent_id: *agent_id,
                        channel: edit.channel().to_owned(),
                        earlier_layer_id: previous.to_owned(),
                        later_layer_id: layer.layer_id.clone(),
                    });
                }
                apply_edit(record, edit, tick)?;
            }
        }
    }
    Ok(ComposedLayoutFrameV1 {
        records,
        conflicts,
        active_layer_ids,
    })
}

pub fn invalidate_dependents_v1(
    layers: &[LayoutLayerV1],
    changed_layer_id: &str,
) -> Vec<LayerInvalidationV1> {
    layers
        .iter()
        .filter(|layer| {
            layer
                .dependencies
                .iter()
                .any(|dependency| dependency == changed_layer_id)
        })
        .map(|layer| LayerInvalidationV1 {
            layer_id: layer.layer_id.clone(),
            reason: format!("depends on changed layer {changed_layer_id}"),
        })
        .collect()
}

/// Mark all direct dependents stale. The caller persists the returned layer
/// stack beside the cache, making invalidation durable across save/reload.
pub fn mark_dependents_stale_v1(
    layers: &mut [LayoutLayerV1],
    changed_layer_id: &str,
) -> Vec<LayerInvalidationV1> {
    let invalidated = invalidate_dependents_v1(layers, changed_layer_id);
    for item in &invalidated {
        if let Some(layer) = layers
            .iter_mut()
            .find(|layer| layer.layer_id == item.layer_id)
        {
            layer.stale = true;
        }
    }
    invalidated
}

/// Build a deterministic, inspectable physics-cache interval. The host DCC
/// only presents this cache; it is never hidden authoritative physics state.
pub fn simulate_physics_handoff_v1(
    spec: &PhysicsHandoffSpecV1,
) -> Result<Vec<PhysicsSampleV1>, LayoutErrorV1> {
    if spec.tick_start > spec.tick_end
        || spec.ticks_per_second == 0
        || spec.collision_masks.is_empty()
        || !spec.gravity_mps2.is_finite()
        || !spec.floor_z.is_finite()
        || spec.restitution_millionths > 1_000_000
        || !spec
            .incoming_position
            .iter()
            .chain(spec.incoming_velocity.iter())
            .all(|value| value.is_finite())
    {
        return Err(LayoutErrorV1(
            "invalid physics handoff specification".to_owned(),
        ));
    }
    let dt = 1.0 / spec.ticks_per_second as f32;
    let restitution = spec.restitution_millionths as f32 / 1_000_000.0;
    let mut position = spec.incoming_position;
    let mut velocity = spec.incoming_velocity;
    let mut samples = Vec::with_capacity((spec.tick_end - spec.tick_start + 1) as usize);
    for tick in spec.tick_start..=spec.tick_end {
        samples.push(PhysicsSampleV1 {
            tick,
            position,
            velocity,
        });
        velocity[2] += spec.gravity_mps2 * dt;
        for axis in 0..3 {
            position[axis] += velocity[axis] * dt;
        }
        if position[2] < spec.floor_z {
            position[2] = spec.floor_z;
            if velocity[2] < 0.0 {
                velocity[2] = -velocity[2] * restitution;
            }
        }
    }
    Ok(samples)
}

/// Produce render-time instance data without creating one scene object per
/// crowd member. Prototype/material selection is stable from the cached
/// variant ID and retains clip/phase, visibility, and render tier.
pub fn extract_procedural_instances_v1(
    records: &[LayoutRecordV1],
    prototypes: &[ProceduralPrototypeV1],
) -> Result<Vec<ProceduralInstanceV1>, LayoutErrorV1> {
    if prototypes.is_empty()
        || prototypes.iter().any(|prototype| {
            prototype.prototype_id.trim().is_empty() || prototype.material_id.trim().is_empty()
        })
    {
        return Err(LayoutErrorV1(
            "procedural extraction requires named prototypes and materials".to_owned(),
        ));
    }
    Ok(records
        .iter()
        .filter(|record| record.visible)
        .map(|record| {
            let prototype = &prototypes[record.variant_id as usize % prototypes.len()];
            ProceduralInstanceV1 {
                agent_id: record.agent_id,
                prototype_id: prototype.prototype_id.clone(),
                material_id: prototype.material_id.clone(),
                position: record.position,
                clip_id: record.clip_id,
                phase: record.phase,
                render_tier: record.render_tier,
            }
        })
        .collect())
}

/// Recompute a selected trajectory toward an authored target. The calculation
/// is bounded to the requested ticks and returns absolute samples for one M4
/// transform layer, so it is inspectable and replayable without a live session.
pub fn resimulate_local_kinematic_v1(
    request: &LocalResimulationRequestV1,
) -> Result<Vec<TransformOverride>, LayoutErrorV1> {
    if request.tick_start > request.tick_end
        || request.ticks_per_second == 0
        || !request.max_speed_mps.is_finite()
        || request.max_speed_mps <= 0.0
        || !request
            .incoming_position
            .iter()
            .chain(request.incoming_velocity.iter())
            .chain(request.target_position.iter())
            .all(|value| value.is_finite())
    {
        return Err(LayoutErrorV1(
            "invalid local resimulation request".to_owned(),
        ));
    }
    let dt = 1.0 / request.ticks_per_second as f32;
    let mut position = request.incoming_position;
    let mut velocity = request.incoming_velocity;
    let mut samples = Vec::with_capacity((request.tick_end - request.tick_start + 1) as usize);
    for tick in request.tick_start..=request.tick_end {
        samples.push(TransformOverride {
            tick,
            translation: position,
        });
        let delta: [f32; 3] =
            std::array::from_fn(|axis| request.target_position[axis] - position[axis]);
        let distance = delta
            .iter()
            .map(|component| component * component)
            .sum::<f32>()
            .sqrt();
        if distance > 0.000_001 {
            velocity = std::array::from_fn(|axis| delta[axis] / distance * request.max_speed_mps);
        }
        for axis in 0..3 {
            position[axis] += velocity[axis] * dt;
        }
    }
    Ok(samples)
}

/// Migrate the 1.0 one-agent transform override into the first M4 layout
/// layer.  Cache v1 itself remains the base representation; migration creates
/// a new adjacent layer artifact and never rewrites cache chunks.
pub fn migrate_override_layer_v1(
    source: &OverrideLayerV1,
    base_cache_hash: String,
) -> Result<LayoutLayerV1, LayoutErrorV1> {
    if source.schema_version != 1 {
        return Err(LayoutErrorV1(format!(
            "cannot migrate override {} with schema {}",
            source.layer_id, source.schema_version
        )));
    }
    if base_cache_hash.len() != 64 || !base_cache_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(LayoutErrorV1(
            "M4 migration requires the complete cache's SHA-256 source hash".to_owned(),
        ));
    }
    Ok(LayoutLayerV1 {
        schema_version: LAYOUT_LAYER_SCHEMA_VERSION,
        layer_id: format!("{}-m4", source.layer_id),
        kind: LayerKindV1::Hero,
        order: 30,
        priority: source.priority,
        muted: !source.enabled,
        solo: false,
        author: source.author.clone(),
        created_at: source.created_at.clone(),
        base_cache_hash,
        provenance: format!("migrated from override-layer-v1:{}", source.layer_id),
        dependencies: Vec::new(),
        stale: false,
        local_resimulation: None,
        target: LayerTargetV1 {
            agent_ids: vec![source.target_agent_id],
            tick_start: source.tick_start,
            tick_end: source.tick_end,
        },
        edits: vec![LayoutEditV1::Transform {
            operation: source.operation,
            samples: source.samples.clone(),
        }],
    })
}

fn layout_record(record: &FrameRecord) -> LayoutRecordV1 {
    LayoutRecordV1 {
        agent_id: record.agent_id,
        position: [record.position[0], record.position[1], 0.0],
        velocity: [record.velocity[0], record.velocity[1], 0.0],
        visible: record.visible,
        playback_rate: record.playback_rate,
        variant_id: record.variant_id,
        clip_id: record.clip_id,
        phase: record.phase,
        destination_id: record.destination_id,
        render_tier: record.render_tier,
        time_offset_ticks: 0,
        frozen: false,
        path_guide: None,
        group_id: None,
        physics_active: false,
    }
}

fn validate_layer(
    layer: &LayoutLayerV1,
    base_cache_hash: &str,
    ids: &BTreeMap<u64, usize>,
) -> Result<(), LayoutErrorV1> {
    if layer.schema_version != LAYOUT_LAYER_SCHEMA_VERSION {
        return Err(LayoutErrorV1(format!(
            "layer {} uses unsupported schema {}",
            layer.layer_id, layer.schema_version
        )));
    }
    if layer.layer_id.trim().is_empty()
        || layer.author.trim().is_empty()
        || layer.created_at.trim().is_empty()
        || layer.provenance.trim().is_empty()
    {
        return Err(LayoutErrorV1(
            "layer identity and provenance must be non-empty".to_owned(),
        ));
    }
    if layer.base_cache_hash != base_cache_hash {
        return Err(LayoutErrorV1(format!(
            "layer {} belongs to a different base cache",
            layer.layer_id
        )));
    }
    if layer.stale {
        return Err(LayoutErrorV1(format!(
            "layer {} is stale; recompute it, resolve the dependency, or mute it",
            layer.layer_id
        )));
    }
    if layer.target.agent_ids.is_empty()
        || layer.target.tick_start > layer.target.tick_end
        || layer.edits.is_empty()
    {
        return Err(LayoutErrorV1(format!(
            "layer {} has an empty target, invalid range, or no edits",
            layer.layer_id
        )));
    }
    if layer
        .target
        .agent_ids
        .iter()
        .any(|id| !ids.contains_key(id))
    {
        return Err(LayoutErrorV1(format!(
            "layer {} targets an agent absent from the base",
            layer.layer_id
        )));
    }
    for edit in &layer.edits {
        validate_edit(edit, layer)?;
    }
    if let Some(local) = &layer.local_resimulation {
        if local.affected_agent_ids.is_empty()
            || local.tick_start > local.tick_end
            || local.tick_start < layer.target.tick_start
            || local.tick_end > layer.target.tick_end
            || local.source_base_hash != base_cache_hash
            || local.reason.trim().is_empty()
            || local
                .affected_agent_ids
                .iter()
                .any(|id| !ids.contains_key(id))
        {
            return Err(LayoutErrorV1(format!(
                "layer {} has invalid local resimulation scope",
                layer.layer_id
            )));
        }
    }
    Ok(())
}

fn validate_edit(edit: &LayoutEditV1, layer: &LayoutLayerV1) -> Result<(), LayoutErrorV1> {
    match edit {
        LayoutEditV1::Transform { samples, .. }
            if samples.is_empty()
                || samples.iter().any(|sample| {
                    sample.tick < layer.target.tick_start
                        || sample.tick > layer.target.tick_end
                        || !sample.translation.iter().all(|value| value.is_finite())
                }) =>
        {
            Err(LayoutErrorV1(format!(
                "layer {} has invalid transform samples",
                layer.layer_id
            )))
        }
        LayoutEditV1::Freeze { position } if !position.iter().all(|value| value.is_finite()) => {
            Err(LayoutErrorV1(format!(
                "layer {} has invalid frozen position",
                layer.layer_id
            )))
        }
        LayoutEditV1::Speed {
            multiplier_millionths: 0,
        } => Err(LayoutErrorV1(format!(
            "layer {} has zero speed",
            layer.layer_id
        ))),
        LayoutEditV1::PathGuide { guide_id } if guide_id.trim().is_empty() => Err(LayoutErrorV1(
            format!("layer {} has empty path guide", layer.layer_id),
        )),
        LayoutEditV1::RegionDensity {
            region_id,
            density_millionths,
        } if region_id.trim().is_empty() || *density_millionths > 1_000_000 => {
            Err(LayoutErrorV1(format!(
                "layer {} has invalid region density operation",
                layer.layer_id
            )))
        }
        LayoutEditV1::CurveRetiming { curve_id, .. } if curve_id.trim().is_empty() => Err(
            LayoutErrorV1(format!("layer {} has empty curve ID", layer.layer_id)),
        ),
        LayoutEditV1::Group { group_id } if group_id.trim().is_empty() => Err(LayoutErrorV1(
            format!("layer {} has empty group", layer.layer_id),
        )),
        LayoutEditV1::Animation {
            phase_millionths, ..
        } if *phase_millionths > 1_000_000 => Err(LayoutErrorV1(format!(
            "layer {} has invalid animation phase",
            layer.layer_id
        ))),
        LayoutEditV1::PhysicsHandoff {
            collision_masks,
            incoming_position,
            incoming_velocity,
            cached_samples,
            recovery_tick,
        } if collision_masks.is_empty()
            || *recovery_tick < layer.target.tick_start
            || !incoming_position
                .iter()
                .chain(incoming_velocity)
                .all(|value| value.is_finite())
            || cached_samples.is_empty()
            || cached_samples.iter().any(|sample| {
                sample.tick < layer.target.tick_start
                    || sample.tick > layer.target.tick_end
                    || !sample
                        .position
                        .iter()
                        .chain(sample.velocity.iter())
                        .all(|value| value.is_finite())
            }) =>
        {
            Err(LayoutErrorV1(format!(
                "layer {} has invalid physics handoff",
                layer.layer_id
            )))
        }
        _ => Ok(()),
    }
}

fn apply_edit(
    record: &mut LayoutRecordV1,
    edit: &LayoutEditV1,
    tick: u64,
) -> Result<(), LayoutErrorV1> {
    match edit {
        LayoutEditV1::Transform { operation, samples } => {
            let value = sample_transform(samples, tick);
            match operation {
                OverrideOperation::Additive => {
                    for (p, d) in record.position.iter_mut().zip(value) {
                        *p += d;
                    }
                }
                OverrideOperation::Absolute => record.position = value,
            }
        }
        LayoutEditV1::Visibility { visible } => record.visible = *visible,
        LayoutEditV1::Freeze { position } => {
            record.position = *position;
            record.velocity = [0.0; 3];
            record.frozen = true;
        }
        LayoutEditV1::Timing { offset_ticks } => record.time_offset_ticks = *offset_ticks,
        LayoutEditV1::Speed {
            multiplier_millionths,
        } => record.playback_rate *= *multiplier_millionths as f32 / 1_000_000.0,
        LayoutEditV1::PathGuide { guide_id } => record.path_guide = Some(guide_id.clone()),
        LayoutEditV1::RegionDensity {
            density_millionths, ..
        } => {
            record.visible = (record.agent_id % 1_000_000) < u64::from(*density_millionths);
        }
        LayoutEditV1::CurveRetiming { offset_ticks, .. } => {
            record.time_offset_ticks = *offset_ticks
        }
        LayoutEditV1::Goal { destination_id } => record.destination_id = *destination_id,
        LayoutEditV1::Appearance { variant_id } => record.variant_id = *variant_id,
        LayoutEditV1::Animation {
            clip_id,
            phase_millionths,
        } => {
            record.clip_id = *clip_id;
            record.phase = *phase_millionths as f32 / 1_000_000.0;
        }
        LayoutEditV1::RenderTier { tier } => record.render_tier = *tier,
        LayoutEditV1::Group { group_id } => record.group_id = Some(group_id.clone()),
        LayoutEditV1::PhysicsHandoff {
            cached_samples,
            recovery_tick,
            ..
        } => {
            if tick < *recovery_tick {
                let sample = sample_physics(cached_samples, tick);
                record.position = sample.position;
                record.velocity = sample.velocity;
                record.physics_active = true;
            }
        }
    }
    Ok(())
}

fn sample_transform(samples: &[TransformOverride], tick: u64) -> [f32; 3] {
    samples
        .iter()
        .rev()
        .find(|sample| sample.tick <= tick)
        .unwrap_or(&samples[0])
        .translation
}
fn sample_physics(samples: &[PhysicsSampleV1], tick: u64) -> &PhysicsSampleV1 {
    samples
        .iter()
        .rev()
        .find(|sample| sample.tick <= tick)
        .unwrap_or(&samples[0])
}

/// Write a deliberately small, inspectable USDA profile.  It uses a point
/// instancer and custom per-instance attributes; readers that do not support a
/// custom attribute must report it rather than silently flattening it.
pub fn write_usda_crowd_profile_v1(
    records: &[LayoutRecordV1],
    source_hash: &str,
) -> Result<String, LayoutErrorV1> {
    if source_hash.len() != 64 || !source_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(LayoutErrorV1(
            "USD export requires a 64-character base cache hash".to_owned(),
        ));
    }
    let ids = records
        .iter()
        .map(|r| r.agent_id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let positions = records
        .iter()
        .map(|r| format!("({}, {}, {})", r.position[0], r.position[1], r.position[2]))
        .collect::<Vec<_>>()
        .join(", ");
    let variants = records
        .iter()
        .map(|r| r.variant_id.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("#usda 1.0\n(\n    defaultPrim = \"Crowd\"\n    metersPerUnit = 1\n    upAxis = \"Z\"\n    customLayerData = {{\n        string crowdProfile = \"BlenderCrowd/v{}\"\n        string baseCacheHash = \"{}\"\n    }}\n)\ndef Xform \"Crowd\" {{\n    def PointInstancer \"Agents\" {{\n        rel prototypes = [</Crowd/Prototypes/Agent>]\n        int64[] ids = [{}]\n        point3f[] positions = [{}]\n        int[] protoIndices = [{}]\n        custom int[] crowd:variant = [{}]\n    }}\n    def Scope \"Prototypes\" {{\n        def Xform \"Agent\" {{\n        }}\n    }}\n}}\n", USD_CROWD_PROFILE_VERSION, source_hash, ids, positions, vec!["0"; records.len()].join(", "), variants))
}

/// Read exactly the public v1 profile channels. Missing or mismatched claimed
/// channels are errors, never silent interchange degradation.
pub fn read_usda_crowd_profile_v1(source: &str) -> Result<UsdCrowdImportV1, LayoutErrorV1> {
    if !source.starts_with("#usda 1.0") || !source.contains("crowdProfile = \"BlenderCrowd/v1\"") {
        return Err(LayoutErrorV1("unsupported USD crowd profile".to_owned()));
    }
    let base_cache_hash = quoted_usd_value(source, "string baseCacheHash = ")?;
    if base_cache_hash.len() != 64 || !base_cache_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(LayoutErrorV1(
            "USD profile has invalid base cache hash".to_owned(),
        ));
    }
    let agent_ids = usd_scalar_list(source, "int64[] ids = [")?
        .into_iter()
        .map(|value| {
            value
                .parse()
                .map_err(|_| LayoutErrorV1("USD profile has invalid agent ID".to_owned()))
        })
        .collect::<Result<Vec<u64>, _>>()?;
    let positions = usd_point_list(source, "point3f[] positions = [")?;
    let variant_ids = usd_scalar_list(source, "custom int[] crowd:variant = [")?
        .into_iter()
        .map(|value| {
            value
                .parse()
                .map_err(|_| LayoutErrorV1("USD profile has invalid variant ID".to_owned()))
        })
        .collect::<Result<Vec<u32>, _>>()?;
    if agent_ids.is_empty()
        || agent_ids.len() != positions.len()
        || agent_ids.len() != variant_ids.len()
    {
        return Err(LayoutErrorV1(
            "USD profile channel lengths do not match".to_owned(),
        ));
    }
    Ok(UsdCrowdImportV1 {
        base_cache_hash,
        agent_ids,
        positions,
        variant_ids,
    })
}

fn quoted_usd_value(source: &str, prefix: &str) -> Result<String, LayoutErrorV1> {
    let rest = source
        .split_once(prefix)
        .map(|(_, rest)| rest)
        .ok_or_else(|| LayoutErrorV1(format!("USD profile is missing {prefix}")))?;
    let start = rest
        .find('"')
        .ok_or_else(|| LayoutErrorV1("USD profile has malformed string".to_owned()))?
        + 1;
    let end = rest[start..]
        .find('"')
        .ok_or_else(|| LayoutErrorV1("USD profile has unterminated string".to_owned()))?
        + start;
    Ok(rest[start..end].to_owned())
}

fn usd_scalar_list<'a>(source: &'a str, prefix: &str) -> Result<Vec<&'a str>, LayoutErrorV1> {
    let rest = source
        .split_once(prefix)
        .map(|(_, rest)| rest)
        .ok_or_else(|| LayoutErrorV1(format!("USD profile is missing {prefix}")))?;
    let end = rest
        .find(']')
        .ok_or_else(|| LayoutErrorV1("USD profile has unterminated array".to_owned()))?;
    Ok(rest[..end]
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect())
}

fn usd_point_list(source: &str, prefix: &str) -> Result<Vec<[f32; 3]>, LayoutErrorV1> {
    let rest = source
        .split_once(prefix)
        .map(|(_, rest)| rest)
        .ok_or_else(|| LayoutErrorV1(format!("USD profile is missing {prefix}")))?;
    let end = rest
        .find(']')
        .ok_or_else(|| LayoutErrorV1("USD profile has unterminated position array".to_owned()))?;
    rest[..end]
        .split(')')
        .filter_map(|item| item.split_once('(').map(|(_, point)| point))
        .map(|point| {
            let values = point
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| {
                    value
                        .parse::<f32>()
                        .map_err(|_| LayoutErrorV1("USD profile has invalid position".to_owned()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            values.try_into().map_err(|_| {
                LayoutErrorV1("USD profile positions must have three values".to_owned())
            })
        })
        .collect()
}
