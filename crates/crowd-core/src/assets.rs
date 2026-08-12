//! M2 character, motion, retarget, and deterministic variation contracts.

use crate::ids::{hash_combine, hash_str, AgentId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedAssetV1 {
    pub id: String,
    pub weight: u32,
}
impl WeightedAssetV1 {
    pub fn new(id: impl Into<String>, weight: u32) -> Self {
        Self {
            id: id.into(),
            weight,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariationProfileV1 {
    pub id: String,
    pub bodies: Vec<WeightedAssetV1>,
    pub clothing: Vec<WeightedAssetV1>,
    pub materials: Vec<WeightedAssetV1>,
    pub props: Vec<WeightedAssetV1>,
    pub clips: Vec<WeightedAssetV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetargetProfileV1 {
    pub id: String,
    pub source_rig_id: String,
    pub root_bone: String,
    pub forward_axis: String,
    pub scale_millimeters: u32,
    pub bone_map: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactIntervalV1 {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipMetadataV1 {
    pub id: String,
    pub retarget_profile_id: String,
    pub duration_ticks: u32,
    pub loop_start_tick: u32,
    pub loop_end_tick: u32,
    pub average_root_speed_mmps: u32,
    pub left_foot_contacts: Vec<ContactIntervalV1>,
    pub right_foot_contacts: Vec<ContactIntervalV1>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetLibraryV1 {
    pub retarget_profiles: Vec<RetargetProfileV1>,
    pub clips: Vec<ClipMetadataV1>,
    pub variations: Vec<VariationProfileV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetDiagnosticCode {
    DuplicateId,
    InvalidRetargetProfile,
    MissingBone,
    MissingRetargetProfile,
    InvalidClipLoop,
    InvalidFootContact,
    InvalidVariation,
    MissingClip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetDiagnostic {
    pub code: AssetDiagnosticCode,
    pub entity_id: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariationChoice {
    pub body: String,
    pub clothing: String,
    pub material: String,
    pub prop: String,
    pub clip: String,
}
impl VariationChoice {
    pub fn with_material(mut self, material: impl Into<String>) -> Self {
        self.material = material.into();
        self
    }
}

#[derive(Clone, Debug)]
pub struct CompiledAssetLibrary {
    variations: BTreeMap<String, VariationProfileV1>,
}
impl CompiledAssetLibrary {
    pub fn select(
        &self,
        profile_id: &str,
        global_seed: u64,
        agent_id: AgentId,
    ) -> Option<VariationChoice> {
        let profile = self.variations.get(profile_id)?;
        Some(VariationChoice {
            body: choose(&profile.bodies, global_seed, agent_id, "body")?.to_string(),
            clothing: choose(&profile.clothing, global_seed, agent_id, "clothing")?.to_string(),
            material: choose(&profile.materials, global_seed, agent_id, "material")?.to_string(),
            prop: choose(&profile.props, global_seed, agent_id, "prop")?.to_string(),
            clip: choose(&profile.clips, global_seed, agent_id, "clip")?.to_string(),
        })
    }
}

pub fn validate_asset_library(
    library: &AssetLibraryV1,
) -> Result<CompiledAssetLibrary, Vec<AssetDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();
    for (kind, id) in library
        .retarget_profiles
        .iter()
        .map(|item| ("retarget", item.id.as_str()))
        .chain(library.clips.iter().map(|item| ("clip", item.id.as_str())))
        .chain(
            library
                .variations
                .iter()
                .map(|item| ("variation", item.id.as_str())),
        )
    {
        if id.is_empty() || !ids.insert((kind, id)) {
            diagnostics.push(error(
                AssetDiagnosticCode::DuplicateId,
                format!("{kind}:{id}"),
                "give this asset contract a unique stable ID",
            ));
        }
    }
    let profile_ids: BTreeSet<_> = library
        .retarget_profiles
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    for profile in &library.retarget_profiles {
        let entity = format!("retarget:{}", profile.id);
        if profile.source_rig_id.is_empty()
            || profile.root_bone.is_empty()
            || !matches!(profile.forward_axis.as_str(), "X" | "-X" | "Y" | "-Y")
            || profile.scale_millimeters == 0
        {
            diagnostics.push(error(
                AssetDiagnosticCode::InvalidRetargetProfile,
                &entity,
                "set source rig, root bone, supported forward axis, and positive scale",
            ));
        }
        for required in ["hips", "left_foot", "right_foot"] {
            if profile
                .bone_map
                .get(required)
                .is_none_or(|bone| bone.is_empty())
            {
                diagnostics.push(error(
                    AssetDiagnosticCode::MissingBone,
                    &entity,
                    format!("map the required canonical bone '{required}'"),
                ));
            }
        }
    }
    let clip_ids: BTreeSet<_> = library.clips.iter().map(|item| item.id.as_str()).collect();
    for clip in &library.clips {
        let entity = format!("clip:{}", clip.id);
        if !profile_ids.contains(clip.retarget_profile_id.as_str()) {
            diagnostics.push(error(
                AssetDiagnosticCode::MissingRetargetProfile,
                &entity,
                "choose an existing retarget profile",
            ));
        }
        if clip.duration_ticks == 0
            || clip.loop_start_tick >= clip.loop_end_tick
            || clip.loop_end_tick >= clip.duration_ticks
            || clip.average_root_speed_mmps == 0
        {
            diagnostics.push(error(
                AssetDiagnosticCode::InvalidClipLoop,
                &entity,
                "set a positive duration, root speed, and loop range inside the clip",
            ));
        }
        if clip
            .left_foot_contacts
            .iter()
            .chain(&clip.right_foot_contacts)
            .any(|contact| contact.start > contact.end || contact.end >= clip.duration_ticks)
        {
            diagnostics.push(error(
                AssetDiagnosticCode::InvalidFootContact,
                entity,
                "keep ordered foot-contact intervals inside the clip duration",
            ));
        }
    }
    let mut variations = BTreeMap::new();
    for profile in &library.variations {
        let entity = format!("variation:{}", profile.id);
        if [
            &profile.bodies,
            &profile.clothing,
            &profile.materials,
            &profile.props,
            &profile.clips,
        ]
        .into_iter()
        .any(|items| invalid_weights(items))
        {
            diagnostics.push(error(
                AssetDiagnosticCode::InvalidVariation,
                &entity,
                "provide non-empty weighted body, clothing, material, prop, and clip choices",
            ));
        }
        for clip in &profile.clips {
            if !clip_ids.contains(clip.id.as_str()) {
                diagnostics.push(error(
                    AssetDiagnosticCode::MissingClip,
                    &entity,
                    format!("choose an existing clip instead of '{}'", clip.id),
                ));
            }
        }
        variations.insert(profile.id.clone(), profile.clone());
    }
    diagnostics.sort_by(|a, b| {
        (&a.entity_id, a.code, &a.message).cmp(&(&b.entity_id, b.code, &b.message))
    });
    if diagnostics.is_empty() {
        Ok(CompiledAssetLibrary { variations })
    } else {
        Err(diagnostics)
    }
}

fn invalid_weights(items: &[WeightedAssetV1]) -> bool {
    items.is_empty()
        || items.iter().any(|item| item.id.is_empty())
        || items.iter().map(|item| u64::from(item.weight)).sum::<u64>() == 0
}
fn choose<'a>(
    items: &'a [WeightedAssetV1],
    global_seed: u64,
    agent_id: AgentId,
    channel: &str,
) -> Option<&'a str> {
    let total: u64 = items.iter().map(|item| u64::from(item.weight)).sum();
    if total == 0 {
        return None;
    }
    let mut value = hash_combine(hash_combine(global_seed, agent_id.0), hash_str(channel)) % total;
    for item in items {
        let weight = u64::from(item.weight);
        if value < weight {
            return Some(&item.id);
        }
        value -= weight;
    }
    None
}
fn error(
    code: AssetDiagnosticCode,
    entity_id: impl Into<String>,
    message: impl Into<String>,
) -> AssetDiagnostic {
    AssetDiagnostic {
        code,
        entity_id: entity_id.into(),
        message: message.into(),
    }
}
