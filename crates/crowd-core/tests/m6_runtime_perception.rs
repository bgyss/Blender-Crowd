use crowd_core::authoring::{compile_authorable_project, migrate_project_v1};
use crowd_core::behavior::{BehaviorGraphV1, BehaviorNodeV1};
use crowd_core::perception::PerceptionEngine;
use crowd_core::{compile_concourse, ProjectIrV1, SampledVelocitySolver, SimConfig, Simulation};

#[test]
fn live_runtime_feeds_typed_hearing_into_the_traceable_behavior_decision() {
    let mut base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    base.populations[0].count = 1;
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![BehaviorGraphV1 {
        id: "hearing_test".to_owned(),
        entry_id: "heard".to_owned(),
        nodes: vec![
            BehaviorNodeV1::Event {
                id: "heard".to_owned(),
                event_type: "danger".to_owned(),
                child: "hold".to_owned(),
            },
            BehaviorNodeV1::HoldPosition {
                id: "hold".to_owned(),
            },
        ],
    }];
    project.population_behaviors[0].graph_id = "hearing_test".to_owned();
    let compiled = compile_authorable_project(&project).unwrap();
    let scene = compile_concourse(compiled.base()).unwrap();
    let agent_id = crowd_core::compile_project(&project.base)
        .unwrap()
        .agent_spawns()[0]
        .agent_id;
    let mut perception = PerceptionEngine::default();
    perception.set_hearing_event(agent_id, "danger");
    let mut controller = compiled.runtime_controller();
    controller.set_perception_engine(perception);
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(controller);

    simulation.step();

    let trace = simulation.behavior_trace(agent_id).unwrap();
    assert_eq!(trace.decisive_node.as_deref(), Some("hold"));
    assert!(trace
        .perception_channels
        .iter()
        .any(|channel| channel == "Hearing"));
}
