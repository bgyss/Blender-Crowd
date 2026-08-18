use crowd_core::action_library::{ActionDefinitionV1, ActionLibraryV1};

#[test]
fn action_library_is_stable_by_id_and_declares_cost_and_fallback() {
    let library = ActionLibraryV1::new(vec![
        ActionDefinitionV1::new("walk", "locomotion", 1000, "hold"),
        ActionDefinitionV1::new("hold", "locomotion", 500, "hold"),
    ])
    .unwrap();
    assert_eq!(library.ids(), vec!["hold", "walk"]);
    assert_eq!(library.get("walk").unwrap().fallback_id, "hold");
}

#[test]
fn action_library_rejects_duplicate_ids_and_zero_costs() {
    let error = ActionLibraryV1::new(vec![
        ActionDefinitionV1::new("walk", "locomotion", 1000, "hold"),
        ActionDefinitionV1::new("walk", "other", 0, "hold"),
    ])
    .unwrap_err();
    assert!(error.iter().any(|message| message.contains("duplicate")));
    assert!(error.iter().any(|message| message.contains("cost")));
}

#[test]
fn hundreds_of_actions_compile_into_a_stable_bounded_library() {
    let actions = (0..512)
        .map(|index| {
            let id = format!("action-{index:04}");
            ActionDefinitionV1::new(id.clone(), "test", 100, id)
        })
        .collect();
    let library = ActionLibraryV1::new(actions).unwrap();
    assert_eq!(library.len(), 512);
    assert_eq!(
        library.ids().first().map(String::as_str),
        Some("action-0000")
    );
    assert_eq!(
        library.ids().last().map(String::as_str),
        Some("action-0511")
    );
}
