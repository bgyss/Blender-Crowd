//! Release-gated M1 strict rebake acceptance.

#[path = "../../crowd-bench/src/m1_bench.rs"]
#[allow(dead_code)]
mod m1_bench;

use m1_bench::{
    bake_reference_strict, compare_strict_bakes, read_selected_trace, StrictBakeOptions,
};

#[test]
#[ignore = "release M1 acceptance"]
fn strict_reference_rebakes_reproduce_and_meet_navigation_gate() {
    let temporary = tempfile::tempdir().unwrap();
    let first = bake_reference_strict(&StrictBakeOptions::reference(
        temporary.path().join("first.crowd"),
    ))
    .unwrap();
    let second = bake_reference_strict(&StrictBakeOptions::reference(
        temporary.path().join("second.crowd"),
    ))
    .unwrap();
    let comparison = compare_strict_bakes(&first, &second).unwrap();

    assert_eq!(first.agent_count, 1_000);
    assert_eq!(first.unique_agent_ids, 1_000);
    assert_eq!(first.discrete_digest, second.discrete_digest);
    assert!(comparison.static_channels_equal);
    assert!(comparison.discrete_channels_equal);
    assert!(comparison.max_position_delta_m <= 0.001);
    assert!(first.destination_completion >= 0.95);
    assert_eq!(first.static_boundary_escapes, 0);
    assert!(first.portal_reroute.accepted);
    assert!(first.portal_reroute.unrelated_routes_unchanged);
    assert_eq!(first.required_channels_missing, Vec::<String>::new());

    let trace =
        serde_json::to_value(read_selected_trace(std::path::Path::new(&first.cache_path)).unwrap())
            .unwrap();
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/decision-trace-v1.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&trace)
        .unwrap();
}
