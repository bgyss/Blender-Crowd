//! Full-duration 1K M2 authorable reference acceptance.

use std::fs;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crowd_core::authoring::{
    compile_authorable_project, migrate_project_v1, GroupBottleneckPolicyV2, GroupKindV2, GroupV2,
};
use crowd_core::behavior::BehaviorGraphV1;
use crowd_core::{
    compile_concourse, compile_project, ProjectIrV1, SampledVelocitySolver, SimConfig, Simulation,
};
use serde::Serialize;

#[derive(Serialize)]
struct M2ReferenceReport {
    schema_version: u32,
    generated_unix_seconds: u64,
    agent_count: usize,
    ticks: u64,
    decision_events: u64,
    queue_requested_events: u64,
    queue_admitted_events: u64,
    queue_released_events: u64,
    group_split_events: u64,
    group_regrouped_events: u64,
    group_reported: bool,
    simulation_duration_ns: u64,
    runtime_evidence_accepted: bool,
    /// Blender reproduction and visual fixtures have separate required gates.
    m2_milestone_accepted: bool,
}

/// Run with `scripts/m2-reference-acceptance.sh`; this is intentionally not a
/// normal unit test because it exercises the full 10,000-tick shot at 1K.
#[test]
#[ignore = "M2 1K full-duration reference acceptance"]
fn authorable_reference_1000_agents_emits_queue_group_and_decision_evidence() {
    let base: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    let stable_ids = compile_project(&base)
        .unwrap()
        .agent_spawns()
        .iter()
        .take(2)
        .map(|agent| agent.agent_id.0)
        .collect();
    let mut project = migrate_project_v1(base);
    project.behavior_graphs = vec![serde_json::from_str::<BehaviorGraphV1>(include_str!(
        "../../../assets/reference/graphs/leave-concourse-v1.json"
    ))
    .unwrap()];
    project.population_behaviors[0].graph_id = "leave_concourse".to_string();
    let semantics: serde_json::Value = serde_json::from_str(include_str!(
        "../../../addon/blender_crowd/reference/concourse-authoring-v2.json"
    ))
    .unwrap();
    project.semantics.queues = serde_json::from_value(semantics["queues"].clone()).unwrap();
    project.semantics.lanes = serde_json::from_value(semantics["lanes"].clone()).unwrap();
    project.semantics.cost_regions =
        serde_json::from_value(semantics["cost_regions"].clone()).unwrap();
    project.groups = vec![GroupV2 {
        id: "reference_pair".to_string(),
        kind: GroupKindV2::Couple,
        member_agent_ids: stable_ids,
        leader_agent_id: None,
        shared_destination_id: "east_exit".to_string(),
        max_separation_millimeters: 2_000,
        bottleneck_policy: GroupBottleneckPolicyV2::LeaderFirst,
    }];

    let compiled = compile_authorable_project(&project).unwrap();
    assert_eq!(compiled.base().agent_spawns().len(), 1_000);
    let scene = compile_concourse(compiled.base()).unwrap();
    let ticks = std::env::var("M2_REFERENCE_TICKS")
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .expect("M2_REFERENCE_TICKS is a positive integer")
        })
        .unwrap_or(scene.duration_ticks);
    assert!(ticks > 0 && ticks <= scene.duration_ticks);
    let mut simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    simulation.enable_authorable_behavior(compiled.runtime_controller());

    let started = Instant::now();
    let mut decision_events = 0;
    let mut queue_requested_events = 0;
    let mut queue_admitted_events = 0;
    let mut queue_released_events = 0;
    let mut group_split_events = 0;
    let mut group_regrouped_events = 0;
    for _ in 0..ticks {
        simulation.step();
        for event in simulation.drain_behavior_events() {
            match event.kind {
                crowd_core::BehaviorRuntimeEventKind::Decision => decision_events += 1,
                crowd_core::BehaviorRuntimeEventKind::QueueRequested => queue_requested_events += 1,
                crowd_core::BehaviorRuntimeEventKind::QueueAdmitted => queue_admitted_events += 1,
                crowd_core::BehaviorRuntimeEventKind::QueueReleased => queue_released_events += 1,
                crowd_core::BehaviorRuntimeEventKind::GroupSplit => group_split_events += 1,
                crowd_core::BehaviorRuntimeEventKind::GroupRegrouped => group_regrouped_events += 1,
                crowd_core::BehaviorRuntimeEventKind::ActivityGranted
                | crowd_core::BehaviorRuntimeEventKind::ActivityWaiting
                | crowd_core::BehaviorRuntimeEventKind::ActivityReleased
                | crowd_core::BehaviorRuntimeEventKind::ActivityFailed => {}
            }
        }
    }
    let group_reported = simulation
        .authorable_group_report("reference_pair")
        .is_some();
    let report = M2ReferenceReport {
        schema_version: 1,
        generated_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        agent_count: compiled.base().agent_spawns().len(),
        ticks,
        decision_events,
        queue_requested_events,
        queue_admitted_events,
        queue_released_events,
        group_split_events,
        group_regrouped_events,
        group_reported,
        simulation_duration_ns: started.elapsed().as_nanos() as u64,
        runtime_evidence_accepted: decision_events > 0
            && queue_requested_events > 0
            && queue_admitted_events > 0
            && queue_released_events > 0
            && group_reported,
        m2_milestone_accepted: false,
    };
    if let Ok(path) = std::env::var("M2_REFERENCE_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    assert!(
        report.runtime_evidence_accepted,
        "M2 runtime evidence report: {}",
        serde_json::to_string(&report).unwrap()
    );
}
