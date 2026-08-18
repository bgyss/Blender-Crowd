use crowd_core::motion::{
    compute_motion_warp, FootContactV1, MotionClipV1, MotionDatabaseV1, MotionMatcher,
    MotionQueryV1, MotionSampleV1,
};
use crowd_core::phases::{animate_with_motion_matcher, AnimateConfig, JOG_CLIP_ID};
use crowd_core::units::Vec2;
use crowd_core::world::{AgentSpawn, World, NO_ROUTE};
use crowd_core::AgentId;

fn database() -> MotionDatabaseV1 {
    MotionDatabaseV1::new(
        "reference-motion",
        vec![
            MotionClipV1::new(
                "walk-b",
                "reference-b",
                vec![MotionSampleV1::new(
                    [1000, 0],
                    [1000, 0],
                    FootContactV1::LeftFoot,
                    0,
                )],
            ),
            MotionClipV1::new(
                "walk-a",
                "reference-a",
                vec![MotionSampleV1::new(
                    [1000, 0],
                    [1000, 0],
                    FootContactV1::LeftFoot,
                    0,
                )],
            ),
            MotionClipV1::new(
                "turn",
                "reference-turn",
                vec![MotionSampleV1::new(
                    [0, 0],
                    [0, 1000],
                    FootContactV1::None,
                    250_000,
                )],
            ),
        ],
    )
    .unwrap()
}

#[test]
fn motion_match_uses_stable_clip_id_tie_breaking_and_contact_constraints() {
    let matcher = MotionMatcher::new(database());
    let query = MotionQueryV1 {
        desired_velocity_millimeters_per_second: [1000, 0],
        desired_slope_millionths: 0,
        required_contact: Some(FootContactV1::LeftFoot),
        fallback_clip_id: "turn".to_owned(),
        future_positions_millimeters: Vec::new(),
        future_velocities_millimeters_per_second: Vec::new(),
    };
    let result = matcher.select(&query).unwrap();
    assert_eq!(result.clip_id, "walk-a");
    assert!(!result.used_fallback);
}

#[test]
fn motion_match_reports_a_deterministic_fallback_when_no_contact_candidate_exists() {
    let matcher = MotionMatcher::new(database());
    let query = MotionQueryV1 {
        desired_velocity_millimeters_per_second: [0, 1000],
        desired_slope_millionths: 250_000,
        required_contact: Some(FootContactV1::RightFoot),
        fallback_clip_id: "turn".to_owned(),
        future_positions_millimeters: Vec::new(),
        future_velocities_millimeters_per_second: Vec::new(),
    };
    let result = matcher.select(&query).unwrap();
    assert_eq!(result.clip_id, "turn");
    assert!(result.used_fallback);
    assert!(result.diagnostic.contains("no feasible"));
}

#[test]
fn future_trajectory_features_break_a_first_pose_tie_deterministically() {
    let database = MotionDatabaseV1::new(
        "future",
        vec![
            MotionClipV1::new(
                "straight",
                "reference",
                vec![
                    MotionSampleV1::at(0, [0, 0], [1000, 0], FootContactV1::None, 0),
                    MotionSampleV1::at(1, [1000, 0], [1000, 0], FootContactV1::None, 0),
                ],
            ),
            MotionClipV1::new(
                "turn-late",
                "reference",
                vec![
                    MotionSampleV1::at(0, [0, 0], [1000, 0], FootContactV1::None, 0),
                    MotionSampleV1::at(1, [700, 700], [700, 700], FootContactV1::None, 0),
                ],
            ),
        ],
    )
    .unwrap();
    let matcher = MotionMatcher::new(database);
    let result = matcher
        .select(&MotionQueryV1 {
            desired_velocity_millimeters_per_second: [1000, 0],
            desired_slope_millionths: 0,
            required_contact: None,
            fallback_clip_id: "straight".to_owned(),
            future_positions_millimeters: vec![[1000, 0]],
            future_velocities_millimeters_per_second: vec![[1000, 0]],
        })
        .unwrap();
    assert_eq!(result.clip_id, "straight");
}

#[test]
fn stride_and_turn_warp_is_bounded_and_reports_infeasible_zero_motion() {
    let warp = compute_motion_warp([1000, 0], [1500, 0], 1_500_000).unwrap();
    assert_eq!(warp.stride_scale_millionths, 1_500_000);
    assert!(warp.feasible);
    assert_eq!(compute_motion_warp([0, 0], [1000, 0], 2_000_000), None);
}

#[test]
fn promoted_motion_matcher_can_change_the_clip_choice_without_changing_root_motion() {
    let database = MotionDatabaseV1::new(
        "promoted",
        vec![MotionClipV1::new(
            "jog-a",
            "reference-jog",
            vec![MotionSampleV1::new(
                [0, 0],
                [1350, 0],
                FootContactV1::None,
                0,
            )],
        )],
    )
    .unwrap();
    let matcher = MotionMatcher::new(database);
    let mut world = World::new();
    world
        .spawn(
            AgentSpawn {
                agent_id: AgentId(7),
                population_id: 0,
                position: Vec2::ZERO,
                yaw: 0.0,
                radius: 0.3,
                max_speed: 2.0,
                preferred_speed: 1.35,
                route: NO_ROUTE,
                destination: 0,
            },
            0,
        )
        .unwrap();
    world.next_vel_x[0] = 1.35;
    world.next_pos_x[0] = 0.045;
    animate_with_motion_matcher(&mut world, &AnimateConfig::default(), &matcher);
    assert_eq!(world.clip_id[0], JOG_CLIP_ID);
    assert_eq!(
        world.next_pos_x[0], 0.045,
        "matcher may not teleport root motion"
    );
}
