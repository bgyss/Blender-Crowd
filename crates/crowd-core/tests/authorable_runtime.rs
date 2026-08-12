use crowd_core::authoring::{
    compile_authorable_project, migrate_project_v1, GroupBottleneckPolicyV2, GroupKindV2, GroupV2,
    QueueV2,
};
use crowd_core::behavior::{BehaviorGraphV1, BehaviorNodeV1};
use crowd_core::{compile_concourse, ProjectIrV1, SampledVelocitySolver, SimConfig, Simulation};

#[test]
fn compiled_graph_runs_inside_the_fixed_step_decide_phase_and_is_traceable() {
    let mut base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    base.populations[0].count = 1;
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![BehaviorGraphV1 {
        id: "hold_test".to_string(),
        entry_id: "hold".to_string(),
        nodes: vec![BehaviorNodeV1::HoldPosition {
            id: "hold".to_string(),
        }],
    }];
    project.population_behaviors[0].graph_id = "hold_test".to_string();
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let controller = compiled.runtime_controller();
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(controller);

    simulation.step();
    let agent_id = simulation.world().agent_id[0];
    assert_eq!(
        simulation.world().desired_velocity(0),
        crowd_core::units::Vec2::ZERO
    );
    let trace = simulation.behavior_trace(agent_id).unwrap();
    assert_eq!(trace.decisive_node.as_deref(), Some("hold"));
    assert_eq!(trace.visited_nodes, vec!["hold".to_string()]);
}

#[test]
fn queue_graph_action_reserves_a_live_slot_deterministically() {
    let mut base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    base.populations[0].count = 1;
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![BehaviorGraphV1 {
        id: "queue_test".to_string(),
        entry_id: "queue".to_string(),
        nodes: vec![BehaviorNodeV1::Queue {
            id: "queue".to_string(),
            queue_id: "east_queue".to_string(),
        }],
    }];
    project.population_behaviors[0].graph_id = "queue_test".to_string();
    project.semantics.queues = vec![QueueV2 {
        id: "east_queue".to_string(),
        portal_id: "east_gate".to_string(),
        slots: vec![[50.0, 10.0]],
        admission_capacity: 1,
    }];
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(compiled.runtime_controller());

    simulation.step();
    let agent_id = simulation.world().agent_id[0];
    assert_eq!(
        simulation
            .authorable_queue_status("east_queue", agent_id)
            .unwrap(),
        crowd_core::social::QueueStatus::Admitted { slot: 0 }
    );
    assert_eq!(
        simulation
            .behavior_trace(agent_id)
            .unwrap()
            .decisive_node
            .as_deref(),
        Some("queue")
    );
}

#[test]
fn live_graph_and_queue_decisions_emit_ordered_persistent_evidence() {
    let mut base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    base.populations[0].count = 1;
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![BehaviorGraphV1 {
        id: "queue_test".to_string(),
        entry_id: "queue".to_string(),
        nodes: vec![BehaviorNodeV1::Queue {
            id: "queue".to_string(),
            queue_id: "east_queue".to_string(),
        }],
    }];
    project.population_behaviors[0].graph_id = "queue_test".to_string();
    project.semantics.queues = vec![QueueV2 {
        id: "east_queue".to_string(),
        portal_id: "east_gate".to_string(),
        slots: vec![[50.0, 10.0]],
        admission_capacity: 1,
    }];
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(compiled.runtime_controller());

    simulation.step();
    let evidence = simulation.drain_behavior_events();
    assert_eq!(evidence.len(), 3);
    assert_eq!(
        evidence[0].kind,
        crowd_core::BehaviorRuntimeEventKind::Decision
    );
    assert_eq!(evidence[0].graph_id.as_deref(), Some("queue_test"));
    assert_eq!(evidence[0].decisive_node.as_deref(), Some("queue"));
    assert_eq!(
        evidence[1].kind,
        crowd_core::BehaviorRuntimeEventKind::QueueRequested
    );
    assert_eq!(
        evidence[2].kind,
        crowd_core::BehaviorRuntimeEventKind::QueueAdmitted
    );
    assert!(simulation.drain_behavior_events().is_empty());
}

#[test]
fn leader_first_group_serializes_members_through_a_live_queue() {
    let mut base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    base.populations[0].count = 2;
    base.populations[0].emission_interval_ticks = 1;
    base.semantics.spawns[1].start_tick = 0;
    let ids: Vec<_> = crowd_core::compile_project(&base)
        .unwrap()
        .agent_spawns()
        .iter()
        .map(|agent| agent.agent_id.0)
        .collect();
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![BehaviorGraphV1 {
        id: "queue_test".to_string(),
        entry_id: "queue".to_string(),
        nodes: vec![BehaviorNodeV1::Queue {
            id: "queue".to_string(),
            queue_id: "east_queue".to_string(),
        }],
    }];
    project.population_behaviors[0].graph_id = "queue_test".to_string();
    project.semantics.queues = vec![QueueV2 {
        id: "east_queue".to_string(),
        portal_id: "east_gate".to_string(),
        slots: vec![[50.0, 10.0]],
        admission_capacity: 1,
    }];
    project.groups = vec![GroupV2 {
        id: "pair".to_string(),
        kind: GroupKindV2::Couple,
        member_agent_ids: ids.clone(),
        leader_agent_id: Some(ids[1]),
        shared_destination_id: "east_exit".to_string(),
        max_separation_millimeters: 2_000,
        bottleneck_policy: GroupBottleneckPolicyV2::LeaderFirst,
    }];
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(compiled.runtime_controller());

    simulation.step();
    assert_eq!(
        simulation.authorable_queue_status("east_queue", crowd_core::AgentId(ids[1])),
        Some(crowd_core::social::QueueStatus::Admitted { slot: 0 })
    );
    assert_eq!(
        simulation.authorable_queue_status("east_queue", crowd_core::AgentId(ids[0])),
        Some(crowd_core::social::QueueStatus::Absent)
    );
}

#[test]
fn group_constraints_are_evaluated_in_the_live_steering_phase() {
    let mut base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    base.populations[0].count = 2;
    base.populations[0].emission_interval_ticks = 1;
    base.semantics.spawns[1].start_tick = 0;
    let ids: Vec<_> = crowd_core::compile_project(&base)
        .unwrap()
        .agent_spawns()
        .iter()
        .map(|agent| agent.agent_id.0)
        .collect();
    let mut project = migrate_project_v1(base);
    project.groups = vec![GroupV2 {
        id: "pair".to_string(),
        kind: GroupKindV2::Couple,
        member_agent_ids: ids,
        leader_agent_id: None,
        shared_destination_id: "east_exit".to_string(),
        max_separation_millimeters: 500,
        bottleneck_policy: GroupBottleneckPolicyV2::Individual,
    }];
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(compiled.runtime_controller());

    simulation.step();
    let report = simulation.authorable_group_report("pair").unwrap();
    assert_eq!(report.missing_members, 0);
    assert!(report.maximum_separation_m >= 0.0);
    let shared_destination = compiled.base().destination_index("east_exit").unwrap() as u16;
    assert!(
        simulation
            .world()
            .destination
            .iter()
            .all(|destination| *destination == shared_destination),
        "the authored group goal must override individual destination choices"
    );
}
