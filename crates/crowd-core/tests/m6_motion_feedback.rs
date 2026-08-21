use crowd_core::motion::{
    FootContactV1, FootLockWindowV1, MotionCorrectionV1, MotionFeedbackV1, TerrainConstraintV1,
};

#[test]
fn motion_feedback_accepts_a_feasible_trajectory_and_reports_metrics() {
    let feedback =
        MotionFeedbackV1::evaluate([1000, 0], [980, 20], 0, 10_000, 20_000, 100_000, 100_000);
    assert!(feedback.feasible);
    assert_eq!(feedback.correction, MotionCorrectionV1::None);
    assert_eq!(feedback.root_deviation_millionths, 28_284);
}

#[test]
fn motion_feedback_rejects_teleport_or_foot_slide_with_a_clip_fallback() {
    let feedback =
        MotionFeedbackV1::evaluate([1000, 0], [5000, 0], 0, 400_000, 250_000, 100_000, 100_000);
    assert!(!feedback.feasible);
    assert_eq!(feedback.correction, MotionCorrectionV1::FallbackClip);
    assert!(feedback.foot_slide_millionths > 100_000);
}

#[test]
fn terrain_and_foot_lock_constraints_are_bounded_and_explicit() {
    let terrain = TerrainConstraintV1 {
        max_slope_millionths: 300_000,
        ground_height_millimeters: 0,
    };
    terrain.validate().unwrap();
    assert!(terrain.accepts_slope(250_000));
    assert!(!terrain.accepts_slope(350_000));

    let lock = FootLockWindowV1 {
        foot: FootContactV1::LeftFoot,
        tick_start: 10,
        tick_end: 20,
        position_millimeters: [100, 200, 0],
    };
    lock.validate().unwrap();
    assert!(lock.contains_tick(15));
    assert!(!lock.contains_tick(21));
}
