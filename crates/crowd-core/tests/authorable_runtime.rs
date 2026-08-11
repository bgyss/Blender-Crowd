use crowd_core::authoring::{compile_authorable_project, migrate_project_v1};
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
