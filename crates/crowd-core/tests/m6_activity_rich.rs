use crowd_core::activity::{
    ActivityFailurePolicyV1, ActivityPlanV1, ActivityResourceBindingV1, ActivityResourceKindV1,
    NeedGoalV1, PairedActivityV1,
};

#[test]
fn rich_activity_plan_declares_needs_resources_paired_actions_and_capacity() {
    let plan = ActivityPlanV1 {
        id: "platform-conversation".to_owned(),
        windows: vec![(10, 100)],
        needs: vec![NeedGoalV1 {
            key: "social".to_owned(),
            target_millionths: 750_000,
            decay_per_tick: 1000,
        }],
        resources: vec![ActivityResourceBindingV1 {
            id: "conversation-seat".to_owned(),
            kind: ActivityResourceKindV1::ConversationSpace,
            capacity: 2,
        }],
        paired_action: Some(PairedActivityV1 {
            action_id: "greet-and-talk".to_owned(),
            participant_roles: vec!["speaker".to_owned(), "listener".to_owned()],
        }),
        capacity: 2,
        failure_policy: ActivityFailurePolicyV1::Fallback,
    };
    plan.validate().unwrap();
    assert!(plan.is_active(10));
    assert!(!plan.is_active(101));
}

#[test]
fn rich_activity_plan_rejects_bad_need_and_paired_role_declarations() {
    let plan = ActivityPlanV1 {
        id: "bad".to_owned(),
        windows: vec![(5, 4)],
        needs: vec![NeedGoalV1 {
            key: "".to_owned(),
            target_millionths: 1_000_001,
            decay_per_tick: 0,
        }],
        resources: vec![],
        paired_action: Some(PairedActivityV1 {
            action_id: "".to_owned(),
            participant_roles: vec!["speaker".to_owned()],
        }),
        capacity: 0,
        failure_policy: ActivityFailurePolicyV1::Fail,
    };
    let errors = plan.validate().unwrap_err();
    assert!(errors.iter().any(|error| error.contains("window")));
    assert!(errors.iter().any(|error| error.contains("need")));
    assert!(errors.iter().any(|error| error.contains("paired")));
    assert!(errors.iter().any(|error| error.contains("capacity")));
}
