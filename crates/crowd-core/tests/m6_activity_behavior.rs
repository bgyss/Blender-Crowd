use std::collections::{BTreeMap, BTreeSet};

use crowd_core::activity::{ReservationRuntimeV1, ResourceV1};
use crowd_core::authoring::{compile_authorable_project, migrate_project_v1};
use crowd_core::behavior::{
    compile_graph, BehaviorAction, BehaviorContext, BehaviorGraphV1, BehaviorNodeV1, BehaviorVm,
    BehaviorVmState,
};
use crowd_core::ids::AgentId;
use crowd_core::{compile_concourse, ProjectIrV1, SampledVelocitySolver, SimConfig, Simulation};

#[test]
fn authored_reserve_action_is_a_traceable_typed_behavior_terminal() {
    let graph = BehaviorGraphV1 {
        id: "reserve-seat".to_owned(),
        entry_id: "reserve".to_owned(),
        nodes: vec![BehaviorNodeV1::Reserve {
            id: "reserve".to_owned(),
            resource_id: "seat-a".to_owned(),
            priority: 20,
        }],
    };
    let vm = BehaviorVm::new(compile_graph(&graph).unwrap(), 2026);
    let outcome = vm.decide(
        &mut BehaviorVmState::default(),
        &BehaviorContext {
            tick: 1,
            agent_id: AgentId(7),
            bool_observations: BTreeMap::new(),
            number_observations: BTreeMap::new(),
            typed_blackboard: BTreeMap::new(),
            events: BTreeSet::new(),
            completed_nodes: BTreeSet::new(),
        },
    );
    assert_eq!(
        outcome.action,
        Some(BehaviorAction::Reserve {
            resource_id: "seat-a".to_owned(),
            priority: 20,
        })
    );
}

#[test]
fn activity_runtime_can_be_attached_with_a_finite_resource_definition() {
    let runtime = ReservationRuntimeV1::new(vec![ResourceV1 {
        id: "seat-a".to_owned(),
        capacity: 1,
    }])
    .unwrap();
    assert_eq!(runtime.owners("seat-a"), Vec::<u64>::new());
}

#[test]
fn live_reserve_actions_admit_one_agent_and_trace_the_waiting_agent() {
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
        .map(|agent| agent.agent_id)
        .collect();
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![crowd_core::behavior::BehaviorGraphV1 {
        id: "reserve_graph".to_owned(),
        entry_id: "reserve".to_owned(),
        nodes: vec![BehaviorNodeV1::Reserve {
            id: "reserve".to_owned(),
            resource_id: "seat-a".to_owned(),
            priority: 10,
        }],
    }];
    project.population_behaviors[0].graph_id = "reserve_graph".to_owned();
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let mut controller = compiled.runtime_controller();
    controller
        .set_activity_resources(vec![ResourceV1 {
            id: "seat-a".to_owned(),
            capacity: 1,
        }])
        .unwrap();
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(controller);

    simulation.step();

    assert_eq!(
        simulation.authorable_activity_status(ids[0].0, "seat-a"),
        Some(crowd_core::activity::ReservationStatusV1::Granted)
    );
    assert_eq!(
        simulation.authorable_activity_status(ids[1].0, "seat-a"),
        Some(crowd_core::activity::ReservationStatusV1::Waiting { ordinal: 1 })
    );
    assert!(simulation
        .drain_behavior_events()
        .iter()
        .any(|event| event.kind == crowd_core::BehaviorRuntimeEventKind::ActivityWaiting));
}
