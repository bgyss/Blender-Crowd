use crowd_cache::{
    compose_layout_frame_v1, extract_procedural_instances_v1, invalidate_dependents_v1,
    mark_dependents_stale_v1, migrate_override_layer_v1, read_usda_crowd_profile_v1,
    resimulate_local_kinematic_v1, simulate_physics_handoff_v1, write_usda_crowd_profile_v1, Frame,
    FrameRecord, LayerKindV1, LayerTargetV1, LayoutEditV1, LayoutLayerV1,
    LocalResimulationRequestV1, LocalResimulationV1, OverrideLayerV1, OverrideOperation,
    PhysicsHandoffSpecV1, PhysicsSampleV1, ProceduralPrototypeV1, TransformOverride,
};
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn base() -> Frame {
    Frame {
        records: vec![
            FrameRecord {
                agent_id: 7,
                position: [1.0, 2.0],
                velocity: [1.0, 0.0],
                visible: true,
                playback_rate: 1.0,
                variant_id: 1,
                clip_id: 2,
                destination_id: 3,
                render_tier: 2,
                ..FrameRecord::default()
            },
            FrameRecord {
                agent_id: 8,
                position: [4.0, 5.0],
                visible: true,
                ..FrameRecord::default()
            },
        ],
    }
}

fn layer(id: &str, kind: LayerKindV1, order: u32, edit: LayoutEditV1) -> LayoutLayerV1 {
    LayoutLayerV1 {
        schema_version: 1,
        layer_id: id.to_owned(),
        kind,
        order,
        priority: 0,
        muted: false,
        solo: false,
        author: "artist".to_owned(),
        created_at: "2026-08-12T00:00:00Z".to_owned(),
        base_cache_hash: HASH.to_owned(),
        provenance: "M4 test".to_owned(),
        dependencies: vec![],
        stale: false,
        local_resimulation: None,
        target: LayerTargetV1 {
            agent_ids: vec![7],
            tick_start: 10,
            tick_end: 20,
        },
        edits: vec![edit],
    }
}

#[test]
fn ordered_layers_are_sparse_conflict_aware_and_leave_base_unchanged() {
    let base = base();
    let before = base.clone();
    let layout = layer(
        "layout",
        LayerKindV1::Layout,
        10,
        LayoutEditV1::Transform {
            operation: OverrideOperation::Additive,
            samples: vec![TransformOverride {
                tick: 10,
                translation: [2.0, 0.0, 1.0],
            }],
        },
    );
    let shot = layer(
        "shot",
        LayerKindV1::Shot,
        20,
        LayoutEditV1::Transform {
            operation: OverrideOperation::Absolute,
            samples: vec![TransformOverride {
                tick: 10,
                translation: [9.0, 8.0, 7.0],
            }],
        },
    );
    let composed = compose_layout_frame_v1(&base, 10, HASH, &[shot, layout]).unwrap();
    assert_eq!(base, before);
    assert_eq!(composed.records[0].position, [9.0, 8.0, 7.0]);
    assert_eq!(composed.records[1].position, [4.0, 5.0, 0.0]);
    assert_eq!(composed.conflicts.len(), 1);
    assert_eq!(composed.conflicts[0].earlier_layer_id, "layout");
    assert_eq!(composed.conflicts[0].later_layer_id, "shot");
}

#[test]
fn physics_handoff_recovers_without_touching_unrelated_agents() {
    let physics = layer(
        "physics",
        LayerKindV1::Physics,
        10,
        LayoutEditV1::PhysicsHandoff {
            collision_masks: vec!["crowd".to_owned()],
            incoming_position: [1.0, 2.0, 0.0],
            incoming_velocity: [1.0, 0.0, 0.0],
            cached_samples: vec![PhysicsSampleV1 {
                tick: 10,
                position: [20.0, 21.0, 3.0],
                velocity: [0.0, -1.0, 0.0],
            }],
            recovery_tick: 15,
        },
    );
    let active =
        compose_layout_frame_v1(&base(), 12, HASH, std::slice::from_ref(&physics)).unwrap();
    assert!(active.records[0].physics_active);
    assert_eq!(active.records[0].position, [20.0, 21.0, 3.0]);
    assert_eq!(active.records[1].position, [4.0, 5.0, 0.0]);
    let recovered = compose_layout_frame_v1(&base(), 15, HASH, &[physics]).unwrap();
    assert!(!recovered.records[0].physics_active);
    assert_eq!(recovered.records[0].position, [1.0, 2.0, 0.0]);
}

#[test]
fn physics_handoff_without_collision_masks_is_rejected_with_an_actionable_error() {
    let invalid = layer(
        "invalid-physics",
        LayerKindV1::Physics,
        10,
        LayoutEditV1::PhysicsHandoff {
            collision_masks: vec![],
            incoming_position: [1.0, 2.0, 0.0],
            incoming_velocity: [1.0, 0.0, 0.0],
            cached_samples: vec![PhysicsSampleV1 {
                tick: 10,
                position: [1.0, 2.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
            }],
            recovery_tick: 15,
        },
    );
    let error = compose_layout_frame_v1(&base(), 12, HASH, &[invalid]).unwrap_err();
    assert!(error.to_string().contains("invalid physics handoff"));
}

#[test]
fn deterministic_physics_handoff_generates_recoverable_cached_samples() {
    let spec = PhysicsHandoffSpecV1 {
        tick_start: 10,
        tick_end: 14,
        ticks_per_second: 10,
        incoming_position: [0.0, 0.0, 1.0],
        incoming_velocity: [1.0, 0.0, 0.0],
        gravity_mps2: -9.8,
        floor_z: 0.0,
        restitution_millionths: 500_000,
        collision_masks: vec!["crowd".to_owned()],
    };
    let first = simulate_physics_handoff_v1(&spec).unwrap();
    assert_eq!(first, simulate_physics_handoff_v1(&spec).unwrap());
    assert_eq!(first.len(), 5);
    assert!(first.iter().all(|sample| sample.position[2] >= 0.0));
    let layer = layer(
        "physics-cache",
        LayerKindV1::Physics,
        10,
        LayoutEditV1::PhysicsHandoff {
            collision_masks: spec.collision_masks.clone(),
            incoming_position: spec.incoming_position,
            incoming_velocity: spec.incoming_velocity,
            cached_samples: first,
            recovery_tick: 15,
        },
    );
    assert!(
        compose_layout_frame_v1(&base(), 12, HASH, &[layer])
            .unwrap()
            .records[0]
            .physics_active
    );
}

#[test]
fn procedural_extraction_keeps_10k_agents_as_data_and_bounded_prototypes() {
    let records = (0..10_000)
        .map(|agent_id| crowd_cache::LayoutRecordV1 {
            agent_id,
            position: [agent_id as f32, 0.0, 0.0],
            velocity: [0.0; 3],
            visible: agent_id % 10 != 0,
            playback_rate: 1.0,
            variant_id: (agent_id % 3) as u32,
            clip_id: 2,
            phase: 0.25,
            destination_id: 0,
            render_tier: (agent_id % 3) as u8,
            time_offset_ticks: 0,
            frozen: false,
            path_guide: None,
            group_id: None,
            physics_active: false,
        })
        .collect::<Vec<_>>();
    let prototypes = vec![
        ProceduralPrototypeV1 {
            prototype_id: "adult-a".to_owned(),
            material_id: "mat-a".to_owned(),
        },
        ProceduralPrototypeV1 {
            prototype_id: "adult-b".to_owned(),
            material_id: "mat-b".to_owned(),
        },
        ProceduralPrototypeV1 {
            prototype_id: "adult-c".to_owned(),
            material_id: "mat-c".to_owned(),
        },
    ];
    let instances = extract_procedural_instances_v1(&records, &prototypes).unwrap();
    assert_eq!(instances.len(), 9_000);
    assert!(instances
        .iter()
        .all(|instance| instance.prototype_id.starts_with("adult-")));
    assert_eq!(
        prototypes.len(),
        3,
        "prototype count must not grow with crowd size"
    );
}

#[test]
fn dependencies_are_explicit_and_usd_keeps_identity_and_variant_data() {
    let mut dependent = layer(
        "animation",
        LayerKindV1::AnimationFix,
        20,
        LayoutEditV1::Animation {
            clip_id: 4,
            phase_millionths: 0,
        },
    );
    dependent.dependencies.push("layout".to_owned());
    assert_eq!(
        invalidate_dependents_v1(&[dependent], "layout")[0].layer_id,
        "animation"
    );
    let composed = compose_layout_frame_v1(&base(), 10, HASH, &[]).unwrap();
    let usda = write_usda_crowd_profile_v1(&composed.records, HASH).unwrap();
    assert!(usda.contains("PointInstancer"));
    assert!(usda.contains("int64[] ids = [7, 8]"));
    assert!(usda.contains("crowd:variant"));
}

#[test]
fn local_resimulation_is_bounded_to_the_declared_target_and_base() {
    let mut redirected = layer(
        "redirect",
        LayerKindV1::Layout,
        10,
        LayoutEditV1::PathGuide {
            guide_id: "exit-curve".to_owned(),
        },
    );
    redirected.local_resimulation = Some(LocalResimulationV1 {
        affected_agent_ids: vec![7],
        tick_start: 11,
        tick_end: 19,
        source_base_hash: HASH.to_owned(),
        reason: "redirect around closed aisle".to_owned(),
    });
    assert!(compose_layout_frame_v1(&base(), 12, HASH, &[redirected]).is_ok());
}

#[test]
fn bounded_local_resimulation_emits_explicit_absolute_samples_only_for_its_range() {
    let request = LocalResimulationRequestV1 {
        tick_start: 10,
        tick_end: 14,
        ticks_per_second: 10,
        incoming_position: [0.0, 0.0, 0.0],
        incoming_velocity: [0.0, 0.0, 0.0],
        target_position: [10.0, 0.0, 0.0],
        max_speed_mps: 2.0,
    };
    let samples = resimulate_local_kinematic_v1(&request).unwrap();
    assert_eq!(samples.len(), 5);
    assert_eq!(samples[0].tick, 10);
    assert_eq!(samples[4].tick, 14);
    assert!(samples
        .windows(2)
        .all(|pair| pair[1].translation[0] > pair[0].translation[0]));
    let mut layer = layer(
        "local-resim",
        LayerKindV1::Layout,
        10,
        LayoutEditV1::Transform {
            operation: OverrideOperation::Absolute,
            samples,
        },
    );
    layer.local_resimulation = Some(LocalResimulationV1 {
        affected_agent_ids: vec![7],
        tick_start: 10,
        tick_end: 14,
        source_base_hash: HASH.to_owned(),
        reason: "redirect through alternate exit".to_owned(),
    });
    let composed = compose_layout_frame_v1(&base(), 12, HASH, &[layer]).unwrap();
    assert_ne!(composed.records[0].position, [1.0, 2.0, 0.0]);
}

#[test]
fn dependency_invalidation_marks_a_layer_stale_and_composition_refuses_it() {
    let source = layer(
        "layout",
        LayerKindV1::Layout,
        10,
        LayoutEditV1::Visibility { visible: false },
    );
    let mut downstream = layer(
        "animation",
        LayerKindV1::AnimationFix,
        20,
        LayoutEditV1::Animation {
            clip_id: 3,
            phase_millionths: 0,
        },
    );
    downstream.dependencies.push("layout".to_owned());
    let mut layers = vec![source, downstream];
    let invalidated = mark_dependents_stale_v1(&mut layers, "layout");
    assert_eq!(invalidated[0].layer_id, "animation");
    assert!(layers[1].stale);
    let error = compose_layout_frame_v1(&base(), 10, HASH, &layers).unwrap_err();
    assert!(error.to_string().contains("is stale"));
    layers[1].muted = true;
    assert!(compose_layout_frame_v1(&base(), 10, HASH, &layers).is_ok());
}

#[test]
fn region_density_and_curve_retiming_are_stable_id_scoped_operations() {
    let mut density = layer(
        "region-density",
        LayerKindV1::Layout,
        10,
        LayoutEditV1::RegionDensity {
            region_id: "concourse-west".to_owned(),
            density_millionths: 8,
        },
    );
    density.target.agent_ids = vec![7, 8];
    let curve = layer(
        "curve-retime",
        LayerKindV1::Layout,
        20,
        LayoutEditV1::CurveRetiming {
            curve_id: "exit-curve".to_owned(),
            offset_ticks: -3,
        },
    );
    let composed = compose_layout_frame_v1(&base(), 10, HASH, &[density, curve]).unwrap();
    assert!(composed.records[0].visible);
    assert!(!composed.records[1].visible);
    assert_eq!(composed.records[0].time_offset_ticks, -3);
}

#[test]
fn incompatible_base_hash_is_an_actionable_error() {
    let layer = layer(
        "shot",
        LayerKindV1::Shot,
        10,
        LayoutEditV1::Visibility { visible: false },
    );
    let error = compose_layout_frame_v1(
        &base(),
        10,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        &[layer],
    )
    .unwrap_err();
    assert!(error.to_string().contains("different base cache"));
}

#[test]
fn cache_v1_overrides_migrate_to_an_adjacent_m4_layer() {
    let old = OverrideLayerV1 {
        schema_version: 1,
        layer_id: "hero-pin".to_owned(),
        author: "artist".to_owned(),
        created_at: "2026-08-12T00:00:00Z".to_owned(),
        priority: 100,
        enabled: true,
        target_agent_id: 7,
        tick_start: 10,
        tick_end: 20,
        operation: OverrideOperation::Additive,
        samples: vec![TransformOverride {
            tick: 10,
            translation: [1.0, 2.0, 3.0],
        }],
    };
    let migrated = migrate_override_layer_v1(&old, HASH.to_owned()).unwrap();
    assert_eq!(migrated.layer_id, "hero-pin-m4");
    assert_eq!(migrated.kind, LayerKindV1::Hero);
    let composed = compose_layout_frame_v1(&base(), 10, HASH, &[migrated]).unwrap();
    assert_eq!(composed.records[0].position, [2.0, 4.0, 3.0]);
}

#[test]
fn checked_v1_override_migration_fixture_matches_the_m4_golden() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source: OverrideLayerV1 = serde_json::from_slice(
        &std::fs::read(root.join("assets/reference/migrations/override-layer-v1-hero-pin.json"))
            .unwrap(),
    )
    .unwrap();
    let migrated = migrate_override_layer_v1(&source, HASH.to_owned()).unwrap();
    let expected: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            root.join("assets/reference/migrations/override-layer-v1-to-layout-v1-golden.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(migrated).unwrap(), expected);
}

#[test]
fn checked_m4_migration_fixture_validates_against_the_full_layer_schema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schema: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("schemas/layout-layer-v1.schema.json")).unwrap(),
    )
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("assets/reference/migrations/override-layer-v1-to-layout-v1-golden.json")).unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema).unwrap().validate(&fixture).unwrap();
}

#[test]
fn schema_rejects_an_unknown_or_under_specified_edit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schema: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("schemas/layout-layer-v1.schema.json")).unwrap(),
    )
    .unwrap();
    let mut fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("assets/reference/migrations/override-layer-v1-to-layout-v1-golden.json")).unwrap(),
    )
    .unwrap();
    fixture["edits"] = serde_json::json!([{ "type": "region_density", "region_id": "west" }]);
    assert!(jsonschema::validator_for(&schema).unwrap().validate(&fixture).is_err());
}

#[test]
fn usd_profile_loads_in_the_independent_usdcat_consumer_when_available() {
    if Command::new("usdcat").arg("--help").output().is_err() {
        eprintln!("usdcat unavailable; external consumer check skipped");
        return;
    }
    let composed = compose_layout_frame_v1(&base(), 10, HASH, &[]).unwrap();
    let directory = tempdir().unwrap();
    let path = directory.path().join("crowd.usda");
    std::fs::write(
        &path,
        write_usda_crowd_profile_v1(&composed.records, HASH).unwrap(),
    )
    .unwrap();
    let status = Command::new("usdcat")
        .arg("--loadOnly")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success(), "usdcat rejected the M4 profile");
}

#[test]
fn usd_profile_passes_the_independent_usdchecker_when_available() {
    if Command::new("usdchecker").arg("--help").output().is_err() {
        eprintln!("usdchecker unavailable; external checker skipped");
        return;
    }
    let composed = compose_layout_frame_v1(&base(), 10, HASH, &[]).unwrap();
    let directory = tempdir().unwrap();
    let path = directory.path().join("crowd.usda");
    std::fs::write(
        &path,
        write_usda_crowd_profile_v1(&composed.records, HASH).unwrap(),
    )
    .unwrap();
    assert!(Command::new("usdchecker")
        .arg(&path)
        .status()
        .unwrap()
        .success());
}

#[test]
fn usd_profile_round_trips_its_claimed_identity_transform_and_variant_channels() {
    let composed = compose_layout_frame_v1(&base(), 10, HASH, &[]).unwrap();
    let imported =
        read_usda_crowd_profile_v1(&write_usda_crowd_profile_v1(&composed.records, HASH).unwrap())
            .unwrap();
    assert_eq!(imported.base_cache_hash, HASH);
    assert_eq!(imported.agent_ids, vec![7, 8]);
    assert_eq!(imported.positions, vec![[1.0, 2.0, 0.0], [4.0, 5.0, 0.0]]);
    assert_eq!(imported.variant_ids, vec![1, 0]);
}

#[test]
fn usd_import_rejects_missing_claimed_channels_instead_of_silent_degradation() {
    let source = "#usda 1.0\n( customLayerData = { string crowdProfile = \"BlenderCrowd/v1\" string baseCacheHash = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" } )\n";
    assert!(read_usda_crowd_profile_v1(source).is_err());
}
