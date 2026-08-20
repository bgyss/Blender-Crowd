use crowd_core::interaction::{
    InteractionGroupStatusV1, InteractionRequestV1, InteractionSchedulerV1,
};

fn request(id: &str, agents: &[u64]) -> InteractionRequestV1 {
    let mut request: InteractionRequestV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-request-v1.json"
    ))
    .unwrap();
    request.request_id = id.to_owned();
    request.group_id = format!("group-{id}");
    request.participants = agents
        .iter()
        .enumerate()
        .map(
            |(index, agent_id)| crowd_core::interaction::InteractionParticipantV1 {
                agent_id: *agent_id,
                role: if index == 0 { "initiator" } else { "responder" }.to_owned(),
                retarget_profile_id: "reference-humanoid".to_owned(),
            },
        )
        .collect();
    request.root_constraints = agents
        .iter()
        .map(|agent_id| crowd_core::interaction::RootConstraintV1 {
            agent_id: *agent_id,
            samples: vec![
                crowd_core::interaction::RootSampleV1 {
                    tick: 10,
                    position: [0.0, 0.0, 0.0],
                    yaw: 0.0,
                },
                crowd_core::interaction::RootSampleV1 {
                    tick: 20,
                    position: [0.0, 0.0, 0.0],
                    yaw: 0.0,
                },
            ],
        })
        .collect();
    request.contact_constraints.clear();
    request
}

#[test]
fn scheduler_promotes_a_group_atomically_and_locks_both_participants() {
    let mut scheduler = InteractionSchedulerV1::new(1);
    let first = request("first", &[7, 9]);
    scheduler.enqueue(first.clone()).unwrap();
    assert_eq!(scheduler.promote_next(), Some("first".to_owned()));
    assert_eq!(
        scheduler.status("first"),
        Some(InteractionGroupStatusV1::Promoted)
    );
    assert_eq!(scheduler.active_group_for(7), Some("first"));
    assert_eq!(scheduler.active_group_for(9), Some("first"));
}

#[test]
fn scheduler_rejects_partial_overlap_and_falls_back_without_releasing_the_other_group() {
    let mut scheduler = InteractionSchedulerV1::new(1);
    scheduler.enqueue(request("first", &[7, 9])).unwrap();
    scheduler.promote_next();
    assert!(scheduler.enqueue(request("overlap", &[9, 11])).is_err());
    assert_eq!(
        scheduler.fail("first", "worker unavailable"),
        Some(InteractionGroupStatusV1::Fallback)
    );
    assert_eq!(scheduler.active_group_for(7), None);
    assert_eq!(scheduler.active_group_for(9), None);
    assert_eq!(
        scheduler.status("first"),
        Some(InteractionGroupStatusV1::Fallback)
    );
}

#[test]
fn scheduler_capacity_and_queue_order_are_stable_by_request_id() {
    let mut scheduler = InteractionSchedulerV1::new(1);
    scheduler.enqueue(request("zeta", &[1, 2])).unwrap();
    scheduler.enqueue(request("alpha", &[3, 4])).unwrap();
    assert_eq!(scheduler.promote_next(), Some("alpha".to_owned()));
    assert_eq!(
        scheduler.status("zeta"),
        Some(InteractionGroupStatusV1::Queued)
    );
}
