use crowd_core::activity::{
    ActivityFailurePolicyV1, ActivityRequestV1, ActivityScheduleV1, ActivityWindowV1,
    ReservationRuntimeV1, ReservationStatusV1, ResourceV1,
};

fn runtime() -> ReservationRuntimeV1 {
    ReservationRuntimeV1::new(vec![ResourceV1 {
        id: "seat-a".to_owned(),
        capacity: 1,
    }])
    .unwrap()
}

#[test]
fn reservation_admission_is_stable_by_priority_then_agent_id() {
    let requests = vec![
        ActivityRequestV1 {
            agent_id: 30,
            resource_id: "seat-a".to_owned(),
            priority: 10,
        },
        ActivityRequestV1 {
            agent_id: 10,
            resource_id: "seat-a".to_owned(),
            priority: 10,
        },
        ActivityRequestV1 {
            agent_id: 20,
            resource_id: "seat-a".to_owned(),
            priority: 20,
        },
    ];
    let mut first = runtime();
    let mut second = runtime();
    let first_results = first.request_batch(&requests);
    let reversed: Vec<_> = requests.into_iter().rev().collect();
    let second_results = second.request_batch(&reversed);

    assert_eq!(first_results, second_results);
    assert_eq!(
        first.status(10, "seat-a"),
        ReservationStatusV1::Waiting { ordinal: 1 }
    );
    assert_eq!(first.status(20, "seat-a"), ReservationStatusV1::Granted);
    assert_eq!(
        first.status(30, "seat-a"),
        ReservationStatusV1::Waiting { ordinal: 2 }
    );
}

#[test]
fn releasing_a_resource_advances_the_waiting_queue_without_double_ownership() {
    let mut runtime = runtime();
    runtime.request_batch(&[
        ActivityRequestV1 {
            agent_id: 1,
            resource_id: "seat-a".to_owned(),
            priority: 1,
        },
        ActivityRequestV1 {
            agent_id: 2,
            resource_id: "seat-a".to_owned(),
            priority: 1,
        },
    ]);
    assert!(runtime.release(1, "seat-a"));
    assert_eq!(runtime.status(2, "seat-a"), ReservationStatusV1::Granted);
    assert!(!runtime.release(1, "seat-a"));
    assert_eq!(runtime.owners("seat-a"), vec![2]);
}

#[test]
fn activity_schedule_requires_a_declared_window_and_failure_policy() {
    let schedule = ActivityScheduleV1 {
        schema_version: 1,
        id: "wait-for-train".to_owned(),
        windows: vec![ActivityWindowV1 {
            start_tick: 10,
            end_tick: 20,
            priority: 10,
        }],
        resources: vec!["seat-a".to_owned()],
        needs: Vec::new(),
        paired_action: None,
        capacity: 1,
        failure_policy: ActivityFailurePolicyV1::Fallback,
    };
    schedule.validate().unwrap();
    assert!(!schedule.is_active(9));
    assert!(schedule.is_active(10));
    assert!(schedule.is_active(20));
    assert!(!schedule.is_active(21));
}
