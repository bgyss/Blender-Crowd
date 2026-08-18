use std::path::{Path, PathBuf};

use crowd_core::activity::ActivityScheduleV1;
use crowd_core::motion::MotionDatabaseV1;
use crowd_core::perception::PerceptionSnapshotV1;
use crowd_core::physics::HeroIntegrationBoundaryV1;
use crowd_core::physics::PhysicsTransitionV1;
use jsonschema::validator_for;
use serde_json::Value;

const FIXTURES: &[(&str, &str)] = &[
    (
        "schemas/perception-v1.schema.json",
        "assets/reference/m6/perception-v1.json",
    ),
    (
        "schemas/brain-v1.schema.json",
        "assets/reference/m6/brain-v1.json",
    ),
    (
        "schemas/activity-v1.schema.json",
        "assets/reference/m6/activity-v1.json",
    ),
    (
        "schemas/trajectory-v1.schema.json",
        "assets/reference/m6/trajectory-v1.json",
    ),
    (
        "schemas/contact-v1.schema.json",
        "assets/reference/m6/contact-v1.json",
    ),
    (
        "schemas/interaction-request-v1.schema.json",
        "assets/reference/m6/interaction-request-v1.json",
    ),
    (
        "schemas/interaction-motion-v1.schema.json",
        "assets/reference/m6/interaction-motion-v1.json",
    ),
    (
        "schemas/interaction-animation-layer-v1.schema.json",
        "assets/reference/m6/interaction-animation-layer-v1.json",
    ),
    (
        "schemas/physics-transition-v1.schema.json",
        "assets/reference/m6/physics-transition-v1.json",
    ),
    (
        "schemas/retarget-profile-v1.schema.json",
        "assets/reference/m6/retarget-profile-v1.json",
    ),
    (
        "schemas/formation-v1.schema.json",
        "assets/reference/m6/formation-v1.json",
    ),
    (
        "schemas/terrain-motion-v1.schema.json",
        "assets/reference/m6/terrain-motion-v1.json",
    ),
    (
        "schemas/mixed-tier-v1.schema.json",
        "assets/reference/m6/mixed-tier-v1.json",
    ),
    (
        "schemas/hero-integration-v1.schema.json",
        "assets/reference/m6/hero-integration-v1.json",
    ),
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

#[test]
fn m6_fixtures_validate_against_their_versioned_schemas() {
    let root = repository_root();
    for (schema_path, fixture_path) in FIXTURES {
        let schema = read_json(&root.join(schema_path));
        let fixture = read_json(&root.join(fixture_path));
        validator_for(&schema)
            .unwrap_or_else(|error| panic!("compile {schema_path}: {error}"))
            .validate(&fixture)
            .unwrap_or_else(|error| panic!("{fixture_path} does not match {schema_path}: {error}"));
    }
}

#[test]
fn m6_interaction_schema_rejects_unknown_fields() {
    let root = repository_root();
    let schema = read_json(&root.join("schemas/interaction-request-v1.schema.json"));
    let mut fixture = read_json(&root.join("assets/reference/m6/interaction-request-v1.json"));
    fixture
        .as_object_mut()
        .expect("interaction fixture object")
        .insert("unexpected".to_owned(), Value::Null);

    let error = validator_for(&schema)
        .expect("interaction schema")
        .validate(&fixture)
        .expect_err("unknown interaction fields must be rejected");
    assert!(error.to_string().contains("unexpected"));
}

#[test]
fn m6_golden_fixtures_also_round_trip_through_rust_contract_types() {
    let root = repository_root();
    let _: PerceptionSnapshotV1 = serde_json::from_value(read_json(
        &root.join("assets/reference/m6/perception-v1.json"),
    ))
    .expect("typed perception fixture");
    let _: ActivityScheduleV1 = serde_json::from_value(read_json(
        &root.join("assets/reference/m6/activity-v1.json"),
    ))
    .expect("typed activity fixture");
    let _: MotionDatabaseV1 = serde_json::from_value(read_json(
        &root.join("assets/reference/m6/trajectory-v1.json"),
    ))
    .expect("typed trajectory fixture");
    let _: PhysicsTransitionV1 = serde_json::from_value(read_json(
        &root.join("assets/reference/m6/physics-transition-v1.json"),
    ))
    .expect("typed physics fixture");
    let _: HeroIntegrationBoundaryV1 = serde_json::from_value(read_json(
        &root.join("assets/reference/m6/hero-integration-v1.json"),
    ))
    .expect("typed hero integration fixture");
}
