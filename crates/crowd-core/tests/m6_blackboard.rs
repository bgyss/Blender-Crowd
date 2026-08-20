use crowd_core::blackboard::{
    fuzzy_compare, fuzzy_membership, BlackboardChannelV1, BlackboardStateV1, BlackboardTypeV1,
    BlackboardValueV1, FuzzyComparisonV1,
};
use crowd_core::ids::AgentId;

fn schema() -> Vec<BlackboardChannelV1> {
    vec![
        BlackboardChannelV1::new(
            "threat_visible",
            BlackboardTypeV1::Bool,
            BlackboardValueV1::Bool(false),
        ),
        BlackboardChannelV1::new(
            "need_score",
            BlackboardTypeV1::NumberI32,
            BlackboardValueV1::NumberI32(0),
        ),
        BlackboardChannelV1::new(
            "attention_target",
            BlackboardTypeV1::AgentId,
            BlackboardValueV1::AgentId(AgentId(0)),
        ),
    ]
}

#[test]
fn typed_blackboard_rejects_undeclared_and_wrong_type_writes() {
    let mut blackboard = BlackboardStateV1::new(schema()).unwrap();
    blackboard
        .set("threat_visible", BlackboardValueV1::Bool(true))
        .unwrap();
    assert!(blackboard
        .set("missing", BlackboardValueV1::Bool(true))
        .unwrap_err()
        .to_string()
        .contains("undeclared"));
    assert!(blackboard
        .set("need_score", BlackboardValueV1::Bool(true))
        .unwrap_err()
        .to_string()
        .contains("expects"));
}

#[test]
fn blackboard_changes_are_ordered_and_only_record_real_changes() {
    let mut blackboard = BlackboardStateV1::new(schema()).unwrap();
    blackboard
        .set("need_score", BlackboardValueV1::NumberI32(100))
        .unwrap();
    blackboard
        .set("threat_visible", BlackboardValueV1::Bool(true))
        .unwrap();
    blackboard
        .set("threat_visible", BlackboardValueV1::Bool(true))
        .unwrap();
    let changes = blackboard.drain_changes();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].key, "need_score");
    assert_eq!(changes[1].key, "threat_visible");
    assert_eq!(
        blackboard.get("need_score"),
        Some(&BlackboardValueV1::NumberI32(100))
    );
}

#[test]
fn fuzzy_membership_and_comparison_are_fixed_point_and_deterministic() {
    assert_eq!(fuzzy_membership(0, 0, 100), 0);
    assert_eq!(fuzzy_membership(50, 0, 100), 500_000);
    assert_eq!(fuzzy_membership(100, 0, 100), 1_000_000);
    assert_eq!(fuzzy_membership(150, 0, 100), 1_000_000);
    assert!(fuzzy_compare(75, FuzzyComparisonV1::GreaterThan, 50));
    assert!(!fuzzy_compare(25, FuzzyComparisonV1::GreaterThan, 50));
    assert!(fuzzy_compare(
        50,
        FuzzyComparisonV1::BetweenInclusive(25, 75),
        0
    ));
}
