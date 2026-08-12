use crowd_core::authoring::{
    compile_authorable_project, migrate_project_v1, AuthoringDiagnosticCode, CostRegionKindV2,
    CostRegionV2, GroupBottleneckPolicyV2, GroupKindV2, GroupV2, LaneV2, QueueV2,
};
use crowd_core::behavior::{BehaviorGraphV1, BehaviorNodeV1};
use crowd_core::project::{compile_project, ProjectIrV1};

fn reference_project() -> ProjectIrV1 {
    serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap()
}

#[test]
fn migration_preserves_the_v1_simulation_hash_and_assigns_a_bounded_graph() {
    let v1 = reference_project();
    let v1_hash = compile_project(&v1).unwrap().source_hash();
    let v2 = migrate_project_v1(v1);
    let compiled = compile_authorable_project(&v2).unwrap();

    assert_eq!(compiled.base().source_hash(), v1_hash);
    assert_eq!(
        compiled
            .behavior_program("commuter_v1")
            .unwrap()
            .node_count(),
        1
    );
    assert_eq!(compiled.population_graph("commuters"), Some("commuter_v1"));
}

#[test]
fn graph_semantic_errors_name_the_node_and_corrective_action() {
    let mut project = migrate_project_v1(reference_project());
    project.behavior_graphs = vec![BehaviorGraphV1 {
        id: "bad_graph".to_string(),
        entry_id: "go".to_string(),
        nodes: vec![BehaviorNodeV1::Navigate {
            id: "go".to_string(),
            destination_id: "missing_exit".to_string(),
        }],
    }];
    project.population_behaviors[0].graph_id = "bad_graph".to_string();

    let errors = compile_authorable_project(&project).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.code == AuthoringDiagnosticCode::MissingSemanticReference
            && error.entity_id == "graph:bad_graph/node:go"
            && error.message.contains("choose an existing destination")
    }));
}

#[test]
fn queue_lane_and_cost_region_references_are_validated() {
    let mut project = migrate_project_v1(reference_project());
    project.semantics.queues.push(QueueV2 {
        id: "ticket_queue".to_string(),
        portal_id: "missing_portal".to_string(),
        slots: vec![[10.0, 10.0]],
        admission_capacity: 1,
    });
    project.semantics.lanes.push(LaneV2 {
        id: "short_lane".to_string(),
        points: vec![[0.0, 0.0]],
        strength_millionths: 500_000,
    });
    project.semantics.cost_regions.push(CostRegionV2 {
        id: "danger".to_string(),
        walkable_id: "missing_walkable".to_string(),
        bounds: crowd_core::project::Bounds2IrV1 {
            min: [4.0, 4.0],
            max: [3.0, 3.0],
        },
        kind: CostRegionKindV2::Danger,
        weight_millionths: 1_000_000,
    });

    let errors = compile_authorable_project(&project).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.code == AuthoringDiagnosticCode::MissingSemanticReference
            && error.entity_id == "queue:ticket_queue"
    }));
    assert!(errors.iter().any(|error| {
        error.code == AuthoringDiagnosticCode::InvalidSemanticGeometry
            && error.entity_id == "lane:short_lane"
    }));
    assert!(errors.iter().any(|error| {
        error.code == AuthoringDiagnosticCode::MissingSemanticReference
            && error.entity_id == "region:danger"
    }));
}

#[test]
fn group_validation_rejects_unknown_members_and_nonmember_leaders() {
    let mut project = migrate_project_v1(reference_project());
    let known = compile_project(&project.base).unwrap().agent_spawns()[0]
        .agent_id
        .0;
    project.groups.push(GroupV2 {
        id: "family".to_string(),
        kind: GroupKindV2::Family,
        member_agent_ids: vec![known, u64::MAX],
        leader_agent_id: Some(known + 1),
        shared_destination_id: "east_exit".to_string(),
        max_separation_millimeters: 2_500,
        bottleneck_policy: GroupBottleneckPolicyV2::Individual,
    });

    let errors = compile_authorable_project(&project).unwrap_err();
    assert!(errors.iter().any(|error| {
        error.code == AuthoringDiagnosticCode::UnknownAgent && error.entity_id == "group:family"
    }));
    assert!(errors.iter().any(|error| {
        error.code == AuthoringDiagnosticCode::InvalidGroupLeader
            && error.entity_id == "group:family"
    }));
}

#[test]
fn v1_to_v2_migration_matches_the_checked_golden_contract() {
    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/reference/migrations/project-v1-to-v2-golden.json"
    ))
    .unwrap();
    let migrated = migrate_project_v1(reference_project());
    let compiled = compile_authorable_project(&migrated).unwrap();
    assert_eq!(
        migrated.schema_version as u64,
        golden["expected_schema_version"].as_u64().unwrap()
    );
    assert_eq!(
        compiled.base().source_hash_hex(),
        golden["expected_base_source_hash"].as_str().unwrap()
    );
    assert_eq!(
        migrated.behavior_graphs[0].id,
        golden["expected_graph_id"].as_str().unwrap()
    );
    assert_eq!(
        migrated.behavior_graphs[0].entry_id,
        golden["expected_graph_entry"].as_str().unwrap()
    );
    assert_eq!(
        compiled.population_graph("commuters"),
        golden["expected_population_graphs"]["commuters"].as_str()
    );
}
