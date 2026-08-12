use std::collections::BTreeMap;

use crowd_core::ids::AgentId;
use crowd_core::social::{GroupConstraint, QueueRuntime, QueueStatus};
use crowd_core::units::Vec2;

#[test]
fn queue_admission_is_stable_by_agent_id_not_request_order() {
    let mut a = QueueRuntime::new("gate", 2, 2).unwrap();
    let mut b = QueueRuntime::new("gate", 2, 2).unwrap();
    a.request_batch(&[AgentId(3), AgentId(1), AgentId(2)]);
    b.request_batch(&[AgentId(2), AgentId(3), AgentId(1)]);

    assert_eq!(a.assignments(), b.assignments());
    assert_eq!(a.status(AgentId(1)), QueueStatus::Admitted { slot: 0 });
    assert_eq!(a.status(AgentId(2)), QueueStatus::Admitted { slot: 1 });
    assert_eq!(a.status(AgentId(3)), QueueStatus::Waiting { ordinal: 0 });
}

#[test]
fn queue_capacity_limits_new_admissions_and_advances_in_order() {
    let mut queue = QueueRuntime::new("gate", 3, 1).unwrap();
    queue.request_batch(&[AgentId(30), AgentId(10), AgentId(20)]);
    assert_eq!(queue.status(AgentId(10)), QueueStatus::Admitted { slot: 0 });
    assert_eq!(queue.front_agent(), Some(AgentId(10)));
    assert_eq!(
        queue.status(AgentId(20)),
        QueueStatus::Waiting { ordinal: 0 }
    );

    queue.advance_tick();
    assert_eq!(queue.status(AgentId(20)), QueueStatus::Admitted { slot: 1 });
    queue.release(AgentId(10));
    assert_eq!(queue.status(AgentId(20)), QueueStatus::Admitted { slot: 0 });
    assert_eq!(queue.front_agent(), Some(AgentId(20)));
    assert_eq!(queue.throughput(), 1);
}

#[test]
fn queue_exposes_the_reserved_slot_for_live_steering() {
    let mut queue = QueueRuntime::new("gate", 2, 2).unwrap();
    queue.request_batch(&[AgentId(2), AgentId(1)]);
    assert_eq!(queue.assigned_slot(AgentId(1)), Some(0));
    assert_eq!(queue.assigned_slot(AgentId(2)), Some(1));
    assert_eq!(queue.assigned_slot(AgentId(99)), None);
}

#[test]
fn group_constraint_reports_splits_and_produces_bounded_cohesion() {
    let group = GroupConstraint::new(
        "family",
        vec![AgentId(1), AgentId(2), AgentId(3)],
        AgentId(1),
        2.0,
        0.75,
    )
    .unwrap();
    let positions = BTreeMap::from([
        (AgentId(1), Vec2::new(0.0, 0.0)),
        (AgentId(2), Vec2::new(1.0, 0.0)),
        (AgentId(3), Vec2::new(5.0, 0.0)),
    ]);

    let report = group.evaluate(&positions);
    assert!(report.split);
    assert_eq!(report.farthest_member, Some(AgentId(3)));
    assert!((report.maximum_separation_m - 5.0).abs() < 1e-6);
    let correction = group.cohesion_velocity(AgentId(3), &positions);
    assert!(correction.x < 0.0);
    assert!(correction.length() <= 0.75 + 1e-6);
}

#[test]
fn group_members_inside_the_declared_limit_need_no_correction() {
    let group = GroupConstraint::new(
        "couple",
        vec![AgentId(7), AgentId(8)],
        AgentId(7),
        2.0,
        0.75,
    )
    .unwrap();
    let positions = BTreeMap::from([
        (AgentId(7), Vec2::new(0.0, 0.0)),
        (AgentId(8), Vec2::new(1.0, 0.0)),
    ]);
    assert_eq!(group.cohesion_velocity(AgentId(8), &positions), Vec2::ZERO);
    assert!(!group.evaluate(&positions).split);
}
