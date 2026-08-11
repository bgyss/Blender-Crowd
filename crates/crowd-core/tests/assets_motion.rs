use std::collections::BTreeMap;

use crowd_core::assets::{
    validate_asset_library, AssetDiagnosticCode, AssetLibraryV1, ClipMetadataV1, ContactIntervalV1,
    RetargetProfileV1, VariationChoice, VariationProfileV1, WeightedAssetV1,
};
use crowd_core::ids::AgentId;

fn valid_library() -> AssetLibraryV1 {
    AssetLibraryV1 {
        retarget_profiles: vec![RetargetProfileV1 {
            id: "humanoid".to_string(),
            source_rig_id: "rig_a".to_string(),
            root_bone: "root".to_string(),
            forward_axis: "-Y".to_string(),
            scale_millimeters: 1_000,
            bone_map: BTreeMap::from([
                ("hips".to_string(), "pelvis".to_string()),
                ("left_foot".to_string(), "foot.L".to_string()),
                ("right_foot".to_string(), "foot.R".to_string()),
            ]),
        }],
        clips: vec![ClipMetadataV1 {
            id: "walk".to_string(),
            retarget_profile_id: "humanoid".to_string(),
            duration_ticks: 30,
            loop_start_tick: 0,
            loop_end_tick: 29,
            average_root_speed_mmps: 1_350,
            left_foot_contacts: vec![ContactIntervalV1 { start: 0, end: 8 }],
            right_foot_contacts: vec![ContactIntervalV1 { start: 15, end: 23 }],
        }],
        variations: vec![VariationProfileV1 {
            id: "commuters".to_string(),
            bodies: vec![
                WeightedAssetV1::new("body_a", 1),
                WeightedAssetV1::new("body_b", 3),
            ],
            clothing: vec![WeightedAssetV1::new("coat", 1)],
            materials: vec![
                WeightedAssetV1::new("blue", 1),
                WeightedAssetV1::new("red", 1),
            ],
            props: vec![
                WeightedAssetV1::new("none", 3),
                WeightedAssetV1::new("bag", 1),
            ],
            clips: vec![WeightedAssetV1::new("walk", 1)],
        }],
    }
}

#[test]
fn custom_character_errors_identify_the_profile_and_correction() {
    let mut library = valid_library();
    library.retarget_profiles[0].bone_map.remove("left_foot");
    let errors = validate_asset_library(&library).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.code == AssetDiagnosticCode::MissingBone
            && error.entity_id == "retarget:humanoid"
            && error.message.contains("left_foot")
    }));
}

#[test]
fn clip_loop_and_contacts_must_fit_the_clip_duration() {
    let mut library = valid_library();
    library.clips[0].left_foot_contacts[0].end = 31;
    let errors = validate_asset_library(&library).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.code == AssetDiagnosticCode::InvalidFootContact && error.entity_id == "clip:walk"
    }));
}

#[test]
fn weighted_variation_is_stable_and_individually_overrideable() {
    let library = valid_library();
    let compiled = validate_asset_library(&library).unwrap();
    let agent = AgentId(0x1234_5678_9abc_def0);
    let selected_a = compiled.select("commuters", 2026, agent).unwrap();
    let selected_b = compiled.select("commuters", 2026, agent).unwrap();
    assert_eq!(selected_a, selected_b);

    let overridden = selected_a.clone().with_material("hero_gold");
    assert_eq!(overridden.material, "hero_gold");
    assert_eq!(
        overridden,
        VariationChoice {
            material: "hero_gold".to_string(),
            ..selected_a
        }
    );
}
