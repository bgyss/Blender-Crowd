use std::collections::{BTreeMap, BTreeSet};

use crowd_core::behavior::{
    compile_graph, BehaviorAction, BehaviorContext, BehaviorGraphV1, BehaviorNodeV1, BehaviorVm,
    BehaviorVmState, StateBranchV1, UtilityOptionV1,
};
use crowd_core::ids::AgentId;

fn compile(nodes: Vec<BehaviorNodeV1>, entry_id: &str) -> BehaviorVm {
    let program = compile_graph(&BehaviorGraphV1 {
        id: "runtime_test".to_string(),
        entry_id: entry_id.to_string(),
        nodes,
    })
    .unwrap();
    BehaviorVm::new(program, 2026)
}

fn context(tick: u64) -> BehaviorContext {
    BehaviorContext {
        tick,
        agent_id: AgentId(42),
        bool_observations: BTreeMap::new(),
        number_observations: BTreeMap::new(),
        typed_blackboard: BTreeMap::new(),
        events: BTreeSet::new(),
        completed_nodes: BTreeSet::new(),
    }
}

#[test]
fn utility_selector_chooses_the_highest_score_with_stable_ties() {
    let vm = compile(
        vec![
            BehaviorNodeV1::UtilitySelector {
                id: "choose_goal".to_string(),
                options: vec![
                    UtilityOptionV1 {
                        child: "exit".to_string(),
                        score_key: "exit_score".to_string(),
                    },
                    UtilityOptionV1 {
                        child: "interest".to_string(),
                        score_key: "interest_score".to_string(),
                    },
                ],
            },
            BehaviorNodeV1::Navigate {
                id: "exit".to_string(),
                destination_id: "exit".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "interest".to_string(),
                destination_id: "kiosk".to_string(),
            },
        ],
        "choose_goal",
    );
    let mut input = context(1);
    input
        .number_observations
        .insert("exit_score".to_string(), 250_000);
    input
        .number_observations
        .insert("interest_score".to_string(), 800_000);

    let outcome = vm.decide(&mut BehaviorVmState::default(), &input);
    assert_eq!(
        outcome.action,
        Some(BehaviorAction::Navigate {
            destination_id: "kiosk".to_string()
        })
    );
    assert_eq!(
        outcome.number_observations,
        vec![
            ("exit_score".to_string(), 250_000),
            ("interest_score".to_string(), 800_000),
        ]
    );
}

#[test]
fn state_switch_routes_the_declared_finite_state() {
    let vm = compile(
        vec![
            BehaviorNodeV1::StateSwitch {
                id: "mode".to_string(),
                state_key: "travel_mode".to_string(),
                branches: vec![
                    StateBranchV1 {
                        value: 0,
                        child: "travel".to_string(),
                    },
                    StateBranchV1 {
                        value: 1,
                        child: "regroup".to_string(),
                    },
                ],
                fallback: "travel".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "travel".to_string(),
                destination_id: "exit".to_string(),
            },
            BehaviorNodeV1::HoldPosition {
                id: "regroup".to_string(),
            },
        ],
        "mode",
    );
    let mut input = context(1);
    input
        .number_observations
        .insert("travel_mode".to_string(), 1);

    assert_eq!(
        vm.decide(&mut BehaviorVmState::default(), &input).action,
        Some(BehaviorAction::HoldPosition)
    );
}

#[test]
fn sequence_advances_only_after_the_current_action_completes() {
    let vm = compile(
        vec![
            BehaviorNodeV1::Sequence {
                id: "exit_flow".to_string(),
                children: vec!["join_queue".to_string(), "leave".to_string()],
            },
            BehaviorNodeV1::Queue {
                id: "join_queue".to_string(),
                queue_id: "gate_queue".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "leave".to_string(),
                destination_id: "exit".to_string(),
            },
        ],
        "exit_flow",
    );
    let mut state = BehaviorVmState::default();

    let first = vm.decide(&mut state, &context(10));
    assert_eq!(
        first.action,
        Some(BehaviorAction::Queue {
            queue_id: "gate_queue".to_string()
        })
    );

    let mut after_queue = context(11);
    after_queue.completed_nodes.insert("join_queue".to_string());
    let second = vm.decide(&mut state, &after_queue);
    assert_eq!(
        second.action,
        Some(BehaviorAction::Navigate {
            destination_id: "exit".to_string()
        })
    );
}

#[test]
fn interrupt_uses_the_typed_observation_and_explains_the_decisive_node() {
    let vm = compile(
        vec![
            BehaviorNodeV1::Selector {
                id: "root".to_string(),
                children: vec!["danger_interrupt".to_string(), "normal".to_string()],
            },
            BehaviorNodeV1::Interrupt {
                id: "danger_interrupt".to_string(),
                condition_key: "danger_nearby".to_string(),
                child: "flee".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "flee".to_string(),
                destination_id: "safe_exit".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "normal".to_string(),
                destination_id: "platform".to_string(),
            },
        ],
        "root",
    );
    let mut state = BehaviorVmState::default();
    let mut input = context(30);
    input
        .bool_observations
        .insert("danger_nearby".to_string(), true);

    let outcome = vm.decide(&mut state, &input);
    assert_eq!(outcome.decisive_node, Some("flee".to_string()));
    assert_eq!(
        outcome.observations,
        vec![("danger_nearby".to_string(), true)]
    );
    assert_eq!(
        outcome.action,
        Some(BehaviorAction::Navigate {
            destination_id: "safe_exit".to_string()
        })
    );
}

#[test]
fn probability_is_stable_for_agent_tick_and_node() {
    let vm = compile(
        vec![
            BehaviorNodeV1::Selector {
                id: "root".to_string(),
                children: vec!["optional".to_string(), "fallback".to_string()],
            },
            BehaviorNodeV1::Probability {
                id: "optional".to_string(),
                probability_millionths: 500_000,
                child: "interest".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "interest".to_string(),
                destination_id: "kiosk".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "fallback".to_string(),
                destination_id: "exit".to_string(),
            },
        ],
        "root",
    );
    let input = context(77);
    let a = vm.decide(&mut BehaviorVmState::default(), &input);
    let b = vm.decide(&mut BehaviorVmState::default(), &input);
    assert_eq!(a, b);
}

#[test]
fn timer_does_not_release_its_child_before_the_declared_duration() {
    let vm = compile(
        vec![
            BehaviorNodeV1::Timer {
                id: "pause".to_string(),
                ticks: 3,
                child: "leave".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "leave".to_string(),
                destination_id: "exit".to_string(),
            },
        ],
        "pause",
    );
    let mut state = BehaviorVmState::default();

    assert_eq!(vm.decide(&mut state, &context(20)).action, None);
    assert_eq!(vm.decide(&mut state, &context(22)).action, None);
    assert_eq!(
        vm.decide(&mut state, &context(23)).action,
        Some(BehaviorAction::Navigate {
            destination_id: "exit".to_string()
        })
    );
}
