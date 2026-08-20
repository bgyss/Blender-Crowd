use std::collections::{BTreeMap, BTreeSet};

use crowd_core::behavior::{
    compile_graph, BehaviorAction, BehaviorContext, BehaviorGraphV1, BehaviorNodeV1, BehaviorVm,
    BehaviorVmState,
};
use crowd_core::ids::AgentId;

#[test]
fn reusable_action_node_emits_a_stable_named_action_without_runtime_callbacks() {
    let graph = BehaviorGraphV1 {
        id: "action-library-graph".to_owned(),
        entry_id: "greet".to_owned(),
        nodes: vec![BehaviorNodeV1::Action {
            id: "greet".to_owned(),
            action_id: "greet-and-talk".to_owned(),
        }],
    };
    let vm = BehaviorVm::new(compile_graph(&graph).unwrap(), 2026);
    let outcome = vm.decide(
        &mut BehaviorVmState::default(),
        &BehaviorContext {
            tick: 10,
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
        Some(BehaviorAction::Action {
            action_id: "greet-and-talk".to_owned()
        })
    );
    assert_eq!(outcome.decisive_node.as_deref(), Some("greet"));
}
