use std::collections::{BTreeMap, BTreeSet};

use crowd_core::behavior::{
    compile_graph, BehaviorContext, BehaviorNodeV1, BehaviorVm, BehaviorVmState, UtilityOptionV1,
};
use crowd_core::blackboard::BlackboardValueV1;
use crowd_core::ids::AgentId;

#[test]
fn utility_and_interrupt_decisions_emit_explainable_trace_evidence() {
    let graph = crowd_core::behavior::BehaviorGraphV1 {
        id: "traceable_brain".to_owned(),
        entry_id: "choose".to_owned(),
        nodes: vec![
            BehaviorNodeV1::UtilitySelector {
                id: "choose".to_owned(),
                options: vec![
                    UtilityOptionV1 {
                        child: "respond".to_owned(),
                        score_key: "threat_score".to_owned(),
                    },
                    UtilityOptionV1 {
                        child: "travel".to_owned(),
                        score_key: "travel_score".to_owned(),
                    },
                ],
            },
            BehaviorNodeV1::Interrupt {
                id: "respond".to_owned(),
                condition_key: "threat_visible".to_owned(),
                child: "hold".to_owned(),
            },
            BehaviorNodeV1::HoldPosition {
                id: "hold".to_owned(),
            },
            BehaviorNodeV1::Navigate {
                id: "travel".to_owned(),
                destination_id: "exit".to_owned(),
            },
        ],
    };
    let program = compile_graph(&graph).unwrap();
    let vm = BehaviorVm::new(program, 2026);
    let context = BehaviorContext {
        tick: 12,
        agent_id: AgentId(7),
        bool_observations: BTreeMap::from([(String::from("threat_visible"), true)]),
        number_observations: BTreeMap::from([
            (String::from("threat_score"), 900_000),
            (String::from("travel_score"), 100_000),
        ]),
        typed_blackboard: BTreeMap::new(),
        events: BTreeSet::new(),
        completed_nodes: BTreeSet::new(),
    };

    let outcome = vm.decide(&mut BehaviorVmState::default(), &context);
    assert_eq!(outcome.decisive_node.as_deref(), Some("hold"));
    assert_eq!(
        outcome.utility_scores,
        vec![
            ("respond".to_owned(), 900_000),
            ("travel".to_owned(), 100_000)
        ]
    );
    assert_eq!(outcome.interrupts, vec!["respond".to_owned()]);
    assert_eq!(outcome.visited_nodes, vec!["choose", "respond", "hold"]);
}

#[test]
fn fuzzy_compare_reads_the_typed_blackboard_and_records_membership() {
    let graph = crowd_core::behavior::BehaviorGraphV1 {
        id: "fuzzy_brain".to_owned(),
        entry_id: "fuzzy".to_owned(),
        nodes: vec![
            BehaviorNodeV1::FuzzyCompare {
                id: "fuzzy".to_owned(),
                key: "need_score".to_owned(),
                lower: 0,
                upper: 100,
                threshold_millionths: 500_000,
                child: "hold".to_owned(),
            },
            BehaviorNodeV1::HoldPosition {
                id: "hold".to_owned(),
            },
        ],
    };
    let vm = BehaviorVm::new(compile_graph(&graph).unwrap(), 2026);
    let context = BehaviorContext {
        tick: 1,
        agent_id: AgentId(7),
        bool_observations: BTreeMap::new(),
        number_observations: BTreeMap::new(),
        typed_blackboard: BTreeMap::from([(
            String::from("need_score"),
            BlackboardValueV1::NumberI32(75),
        )]),
        events: BTreeSet::new(),
        completed_nodes: BTreeSet::new(),
    };
    let outcome = vm.decide(&mut BehaviorVmState::default(), &context);
    assert_eq!(outcome.decisive_node.as_deref(), Some("hold"));
    assert_eq!(
        outcome.fuzzy_scores,
        vec![("need_score".to_owned(), 750_000)]
    );
}
