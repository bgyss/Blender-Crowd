use crowd_core::project::{canonical_project_json, compile_project, DiagnosticCode, ProjectIrV1};

fn reference_project(count: u32) -> ProjectIrV1 {
    let mut project: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .expect("reference project JSON");
    project.populations[0].count = count;
    project
}

#[test]
fn adding_one_agent_does_not_reshuffle_existing_choices() {
    let project_100 = reference_project(100);
    let project_101 = reference_project(101);
    let a = compile_project(&project_100).unwrap();
    let b = compile_project(&project_101).unwrap();
    assert_eq!(&a.agent_spawns()[..100], &b.agent_spawns()[..100]);
}

#[test]
fn diagnostics_are_stably_ordered_and_name_the_entity() {
    let mut project = reference_project(10);
    project.populations[0].archetypes.clear();
    project.semantics.destinations.clear();
    let errors = compile_project(&project).unwrap_err();
    assert_eq!(
        errors
            .iter()
            .map(|diagnostic| (&diagnostic.code, diagnostic.entity_id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (&DiagnosticCode::MissingDestination, "population:commuters"),
            (&DiagnosticCode::InvalidWeights, "population:commuters"),
        ]
    );
}

#[test]
fn authoring_array_permutations_do_not_change_compilation() {
    let original = reference_project(100);
    let mut permuted = original.clone();
    permuted.archetypes.reverse();
    permuted.appearances.reverse();
    permuted.populations[0].spawn_source_ids.reverse();
    permuted.populations[0].destinations.reverse();
    permuted.populations[0].archetypes.reverse();
    permuted.populations[0].appearances.reverse();
    permuted.semantics.walkable.reverse();
    permuted.semantics.blocked.reverse();
    permuted.semantics.spawns.reverse();
    permuted.semantics.destinations.reverse();
    permuted.semantics.portals.reverse();
    permuted.portal_events.reverse();

    let original = compile_project(&original).unwrap();
    let permuted = compile_project(&permuted).unwrap();
    assert_eq!(original.source_hash(), permuted.source_hash());
    assert_eq!(original.agent_spawns(), permuted.agent_spawns());
}

#[test]
fn duplicate_logical_ids_are_rejected() {
    let mut project = reference_project(10);
    project.archetypes.push(project.archetypes[0].clone());
    let errors = compile_project(&project).unwrap_err();
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::DuplicateId
            && diagnostic.entity_id == "archetype:adult_a"
    }));
}

#[test]
fn invalid_units_are_rejected() {
    let mut project = reference_project(10);
    project.units.length = "centimeters".to_string();
    let errors = compile_project(&project).unwrap_err();
    assert_eq!(errors[0].code, DiagnosticCode::InvalidUnits);
}

#[test]
fn a_zero_weight_total_is_rejected() {
    let mut project = reference_project(10);
    for archetype in &mut project.populations[0].archetypes {
        archetype.weight = 0.0;
    }
    let errors = compile_project(&project).unwrap_err();
    assert!(errors
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidWeights));
}

#[test]
fn unreachable_initial_topology_is_rejected() {
    let mut project = reference_project(10);
    project
        .semantics
        .portals
        .iter_mut()
        .find(|portal| portal.id == "east_gate")
        .unwrap()
        .initially_open = false;
    let errors = compile_project(&project).unwrap_err();
    assert!(errors
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::UnreachableDestination));
}

#[test]
fn contradictory_portal_events_are_rejected() {
    let mut project = reference_project(10);
    project
        .portal_events
        .push(crowd_core::project::TimedPortalEventV1 {
            tick: 600,
            portal_id: "east_gate".to_string(),
            open: true,
        });
    let errors = compile_project(&project).unwrap_err();
    assert!(errors
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::ContradictoryPortalEvent));
}

#[test]
fn reference_project_has_a_pinned_canonical_hash() {
    let project = reference_project(1_000);
    let canonical = canonical_project_json(&project).unwrap();
    let hash = blake3::hash(canonical.as_bytes()).to_hex().to_string();
    assert_eq!(
        hash,
        "cfeb0ae7bb4ae1c651e7d3f6614453dad6d1d34b808ff42292cba3af5927fb74"
    );
    assert_eq!(compile_project(&project).unwrap().source_hash_hex(), hash);
}

#[test]
fn reference_project_validates_against_the_checked_schema() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("schemas/project-ir-v1.schema.json"))
            .expect("checked project schema"),
    )
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    jsonschema::validator_for(&schema)
        .expect("valid schema")
        .validate(&fixture)
        .expect("reference project matches schema");
}

#[test]
fn unknown_root_and_nested_fields_are_rejected_by_serde() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();

    let mut root_unknown = fixture.clone();
    root_unknown["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ProjectIrV1>(root_unknown).is_err());

    let mut nested_unknown = fixture;
    nested_unknown["populations"][0]["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ProjectIrV1>(nested_unknown).is_err());
}

#[test]
fn malformed_project_identity_is_rejected_at_compile_time() {
    let mut project = reference_project(10);
    project.project_id = "not-a-uuid".to_string();
    let errors = compile_project(&project).unwrap_err();
    assert_eq!(errors[0].code, DiagnosticCode::InvalidProjectId);
}

#[test]
fn spawn_and_destination_must_stay_inside_their_walkable_regions() {
    let mut project = reference_project(10);
    project
        .semantics
        .spawns
        .iter_mut()
        .find(|spawn| spawn.id == "east_platform")
        .unwrap()
        .bounds
        .max[0] = 61.0;
    project
        .semantics
        .destinations
        .iter_mut()
        .find(|destination| destination.id == "west_exit")
        .unwrap()
        .point[0] = -1.0;
    let errors = compile_project(&project).unwrap_err();
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidRange
            && diagnostic.entity_id == "spawn:east_platform"
    }));
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidRange
            && diagnostic.entity_id == "destination:west_exit"
    }));
}

#[test]
fn the_reference_population_has_unique_stable_ids_and_uses_all_variants() {
    let compiled = compile_project(&reference_project(1_000)).unwrap();
    let ids: std::collections::BTreeSet<_> = compiled
        .agent_spawns()
        .iter()
        .map(|spawn| spawn.agent_id)
        .collect();
    let archetypes: std::collections::BTreeSet<_> = compiled
        .agent_spawns()
        .iter()
        .map(|spawn| spawn.archetype_id)
        .collect();
    let appearances: std::collections::BTreeSet<_> = compiled
        .agent_spawns()
        .iter()
        .map(|spawn| spawn.appearance_id)
        .collect();
    assert_eq!(ids.len(), 1_000);
    assert_eq!(archetypes.len(), 3);
    assert_eq!(appearances.len(), 4);
}

#[test]
fn spawn_ordinals_are_local_to_each_stable_spawn_source() {
    let compiled = compile_project(&reference_project(6)).unwrap();
    let ordinals: Vec<_> = compiled
        .agent_spawns()
        .iter()
        .map(|spawn| (spawn.spawn_source_id, spawn.spawn_ordinal))
        .collect();
    assert_eq!(
        ordinals,
        vec![(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)]
    );
}
