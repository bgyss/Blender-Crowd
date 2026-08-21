use std::collections::BTreeMap;

use crowd_core::formation::{FormationRoleV1, FormationSplitPolicyV1, FormationV1};
use crowd_core::ids::AgentId;
use crowd_core::units::Vec2;

fn formation() -> FormationV1 {
    FormationV1::new(
        "family",
        AgentId(1),
        vec![
            FormationRoleV1 {
                agent_id: AgentId(1),
                role: "leader".to_owned(),
                offset_millimeters: [0, 0],
            },
            FormationRoleV1 {
                agent_id: AgentId(2),
                role: "left".to_owned(),
                offset_millimeters: [-1000, 0],
            },
            FormationRoleV1 {
                agent_id: AgentId(3),
                role: "right".to_owned(),
                offset_millimeters: [1000, 0],
            },
        ],
        3_000,
        FormationSplitPolicyV1::Regroup,
    )
    .unwrap()
}

#[test]
fn formation_reports_missing_members_and_intrusions_without_iteration_order_dependence() {
    let formation = formation();
    let positions = BTreeMap::from([
        (AgentId(1), Vec2::new(0.0, 0.0)),
        (AgentId(2), Vec2::new(-1.0, 0.0)),
        (AgentId(3), Vec2::new(8.0, 0.0)),
    ]);
    let report = formation.evaluate(&positions, &[(AgentId(99), Vec2::new(0.2, 0.0))]);
    assert!(report.split);
    assert_eq!(report.missing_members, 0);
    assert_eq!(report.farthest_member, Some(AgentId(3)));
    assert_eq!(report.intruder_agent_ids, vec![AgentId(99)]);
}

#[test]
fn formation_offsets_and_cohesion_are_bounded_and_role_addressable() {
    let formation = formation();
    assert_eq!(formation.offset_for(AgentId(2)), Some(Vec2::new(-1.0, 0.0)));
    assert_eq!(formation.offset_for(AgentId(77)), None);
    let positions = BTreeMap::from([
        (AgentId(1), Vec2::new(0.0, 0.0)),
        (AgentId(2), Vec2::new(-5.0, 0.0)),
        (AgentId(3), Vec2::new(1.0, 0.0)),
    ]);
    let correction = formation.cohesion_velocity(AgentId(2), &positions, 0.75);
    assert!(correction.x > 0.0);
    assert!(correction.length() <= 0.75 + 1e-6);
}
