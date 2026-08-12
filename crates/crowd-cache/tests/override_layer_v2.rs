use crowd_cache::{
    compose_frame_v2, Frame, FrameRecord, LocalResimulationRecordV2, OverrideEditV2,
    OverrideLayerV2, OverrideOperation, TransformOverride,
};

fn frame() -> Frame {
    Frame {
        records: vec![
            FrameRecord {
                agent_id: 10,
                position: [1.0, 2.0],
                playback_rate: 1.0,
                variant_id: 3,
                clip_id: 4,
                destination_id: 5,
                visible: true,
                render_tier: 2,
                ..FrameRecord::default()
            },
            FrameRecord {
                agent_id: 20,
                position: [7.0, 8.0],
                visible: true,
                ..FrameRecord::default()
            },
        ],
    }
}

fn layer() -> OverrideLayerV2 {
    OverrideLayerV2 {
        schema_version: 2,
        layer_id: "shot-fixes".to_string(),
        author: "artist".to_string(),
        created_at: "2026-08-11T00:00:00Z".to_string(),
        priority: 10,
        enabled: true,
        target_agent_id: 10,
        edits: vec![
            OverrideEditV2::Visibility {
                tick_start: 4,
                tick_end: 8,
                visible: false,
            },
            OverrideEditV2::Transform {
                tick_start: 4,
                tick_end: 8,
                operation: OverrideOperation::Additive,
                samples: vec![TransformOverride {
                    tick: 6,
                    translation: [10.0, 20.0, 3.0],
                }],
            },
            OverrideEditV2::Timing {
                tick_start: 4,
                tick_end: 8,
                offset_ticks: -2,
            },
            OverrideEditV2::Speed {
                tick_start: 4,
                tick_end: 8,
                multiplier_millionths: 1_500_000,
            },
            OverrideEditV2::Appearance {
                tick_start: 4,
                tick_end: 8,
                variant_id: 30,
            },
            OverrideEditV2::Animation {
                tick_start: 4,
                tick_end: 8,
                clip_id: 40,
                phase_millionths: 250_000,
            },
            OverrideEditV2::Goal {
                tick_start: 4,
                tick_end: 8,
                destination_id: 50,
            },
            OverrideEditV2::Hero {
                tick_start: 4,
                tick_end: 8,
                render_tier: 0,
            },
        ],
        local_resimulation: Some(LocalResimulationRecordV2 {
            affected_agent_ids: vec![10],
            tick_start: 4,
            tick_end: 8,
            source_base_hash: "ab".repeat(32),
        }),
    }
}

#[test]
fn all_sparse_edits_change_only_the_target_and_leave_base_immutable() {
    let base = frame();
    let before = base.clone();
    let composed = compose_frame_v2(&base, 6, &[layer()]).unwrap();
    let target = &composed.records[0];
    assert_eq!(target.position, [11.0, 22.0, 3.0]);
    assert!(!target.visible);
    assert_eq!(target.time_offset_ticks, -2);
    assert_eq!(target.playback_rate, 1.5);
    assert_eq!(target.variant_id, 30);
    assert_eq!(target.clip_id, 40);
    assert_eq!(target.phase, 0.25);
    assert_eq!(target.destination_id, 50);
    assert_eq!(target.render_tier, 0);
    assert_eq!(composed.records[1].position, [7.0, 8.0, 0.0]);
    assert_eq!(base, before);
    assert!(composed.conflicts.is_empty());
}

#[test]
fn overlapping_edits_report_the_channel_and_both_layers() {
    let first = layer();
    let mut second = layer();
    second.layer_id = "director-fixes".to_string();
    second.priority = 20;
    second.edits = vec![OverrideEditV2::Appearance {
        tick_start: 6,
        tick_end: 10,
        variant_id: 99,
    }];
    let composed = compose_frame_v2(&frame(), 6, &[second, first]).unwrap();
    assert_eq!(composed.records[0].variant_id, 99);
    assert_eq!(composed.conflicts.len(), 1);
    assert_eq!(composed.conflicts[0].channel, "appearance");
    assert_eq!(composed.conflicts[0].earlier_layer_id, "shot-fixes");
    assert_eq!(composed.conflicts[0].later_layer_id, "director-fixes");
}

#[test]
fn local_resimulation_must_name_its_ids_range_and_base_hash() {
    let mut invalid = layer();
    invalid
        .local_resimulation
        .as_mut()
        .unwrap()
        .affected_agent_ids
        .clear();
    let error = compose_frame_v2(&frame(), 6, &[invalid]).unwrap_err();
    assert!(error.to_string().contains("local resimulation"));
}
