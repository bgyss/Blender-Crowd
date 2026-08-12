use crowd_core::behavior::{compile_graph, BehaviorGraphV1, BehaviorNodeV1, GraphDiagnosticCode};

fn graph(nodes: Vec<BehaviorNodeV1>, entry_id: &str) -> BehaviorGraphV1 {
    BehaviorGraphV1 {
        id: "leave_concourse".to_string(),
        entry_id: entry_id.to_string(),
        nodes,
    }
}

#[test]
fn compiles_a_typed_selector_with_terminal_navigation() {
    let program = compile_graph(&graph(
        vec![
            BehaviorNodeV1::Selector {
                id: "choose_exit".to_string(),
                children: vec!["north_exit".to_string(), "south_exit".to_string()],
            },
            BehaviorNodeV1::Navigate {
                id: "north_exit".to_string(),
                destination_id: "north_exit".to_string(),
            },
            BehaviorNodeV1::Navigate {
                id: "south_exit".to_string(),
                destination_id: "south_exit".to_string(),
            },
        ],
        "choose_exit",
    ))
    .expect("well-formed graph must compile");

    assert_eq!(program.id(), "leave_concourse");
    assert_eq!(program.entry_index(), 0);
    assert_eq!(program.node_count(), 3);
}

#[test]
fn rejects_cycles_with_the_offending_node_and_a_corrective_action() {
    let errors = compile_graph(&graph(
        vec![
            BehaviorNodeV1::Sequence {
                id: "loop".to_string(),
                children: vec!["loop".to_string()],
            },
            BehaviorNodeV1::Navigate {
                id: "exit".to_string(),
                destination_id: "exit".to_string(),
            },
        ],
        "loop",
    ))
    .expect_err("cycles would make the authorable graph an unbounded language");

    assert!(errors.iter().any(|error| {
        error.code == GraphDiagnosticCode::Cycle
            && error.node_id == "loop"
            && error.message.contains("remove the cycle")
    }));
}

#[test]
fn rejects_a_composite_that_references_a_missing_child() {
    let errors = compile_graph(&graph(
        vec![BehaviorNodeV1::Sequence {
            id: "main".to_string(),
            children: vec!["missing".to_string()],
        }],
        "main",
    ))
    .expect_err("a dangling child cannot be executed or explained");

    assert_eq!(errors[0].code, GraphDiagnosticCode::MissingNode);
    assert_eq!(errors[0].node_id, "main");
    assert!(errors[0].message.contains("missing"));
}

#[test]
fn rejects_nodes_not_reachable_from_the_entry() {
    let errors = compile_graph(&graph(
        vec![
            BehaviorNodeV1::Navigate {
                id: "main".to_string(),
                destination_id: "exit".to_string(),
            },
            BehaviorNodeV1::Wait {
                id: "orphan".to_string(),
                ticks: 30,
            },
        ],
        "main",
    ))
    .expect_err("unreachable nodes cannot provide an inspectable program");

    assert!(errors.iter().any(|error| {
        error.code == GraphDiagnosticCode::UnreachableNode && error.node_id == "orphan"
    }));
}

#[test]
fn checked_graph_preset_is_validated_by_the_versioned_schema_and_compiler() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("schemas/behavior-graph-v1.schema.json"))
            .expect("checked behavior schema"),
    )
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/reference/graphs/leave-concourse-v1.json"
    ))
    .expect("golden graph fixture");
    jsonschema::validator_for(&schema)
        .expect("valid schema")
        .validate(&fixture)
        .expect("golden graph matches schema");
    let graph: BehaviorGraphV1 = serde_json::from_value(fixture).unwrap();
    assert_eq!(compile_graph(&graph).unwrap().node_count(), 6);
}
