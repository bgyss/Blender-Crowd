use crowd_core::assets::{ClipMetadataV1, ContactIntervalV1};
use crowd_core::presentation::{
    project_presentation_pose, TerrainPlaneV1, TerrainPresentationError,
};
use crowd_core::units::Vec2;

fn walk_clip() -> ClipMetadataV1 {
    ClipMetadataV1 {
        id: "walk".into(),
        retarget_profile_id: "humanoid".into(),
        duration_ticks: 30,
        loop_start_tick: 0,
        loop_end_tick: 29,
        average_root_speed_mmps: 1_350,
        left_foot_contacts: vec![ContactIntervalV1 { start: 0, end: 8 }],
        right_foot_contacts: vec![ContactIntervalV1 { start: 15, end: 23 }],
    }
}

#[test]
fn terrain_projection_preserves_simulation_xy_and_exposes_contact_locks() {
    let pose = project_presentation_pose(
        Vec2::new(4.0, 3.0),
        &TerrainPlaneV1 {
            origin_height_m: 1.0,
            x_rise_per_meter: 0.1,
            y_rise_per_meter: -0.2,
        },
        &walk_clip(),
        3,
        30.0,
    )
    .unwrap();
    assert_eq!(pose.simulation_position, Vec2::new(4.0, 3.0));
    assert_eq!(&pose.display_position[..2], &[4.0, 3.0]);
    assert!((pose.display_position[2] - 0.8).abs() < 1e-5);
    assert!(pose.left_foot_locked);
    assert!(!pose.right_foot_locked);
    assert!(pose.slope_degrees > 0.0);
}

#[test]
fn terrain_presentation_rejects_slopes_beyond_the_authored_limit() {
    let error = project_presentation_pose(
        Vec2::ZERO,
        &TerrainPlaneV1 {
            origin_height_m: 0.0,
            x_rise_per_meter: 2.0,
            y_rise_per_meter: 0.0,
        },
        &walk_clip(),
        0,
        30.0,
    )
    .unwrap_err();
    assert_eq!(error, TerrainPresentationError::SlopeLimitExceeded);
}
