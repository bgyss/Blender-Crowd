use crowd_core::extensions::{ExtensionChannelV1, ExtensionManifestV1, ExtensionValidationError};

fn manifest() -> ExtensionManifestV1 {
    ExtensionManifestV1::new(
        "studio-actions",
        vec![ExtensionChannelV1 {
            name: "look_at".to_owned(),
            version: 1,
            inputs: vec!["attention_target".to_owned()],
            outputs: vec!["gaze_offset".to_owned()],
            cost_budget_millionths: 100_000,
            deterministic: true,
            failure_isolated: true,
        }],
    )
    .unwrap()
}

#[test]
fn extension_channel_requires_declared_inputs_cost_and_failure_isolation() {
    let manifest = manifest();
    manifest
        .validate_call("look_at", &["attention_target"], 50_000)
        .unwrap();
    assert_eq!(
        manifest.validate_call("look_at", &["undeclared"], 50_000),
        Err(ExtensionValidationError::UndeclaredInput(
            "undeclared".to_owned()
        ))
    );
    assert_eq!(
        manifest.validate_call("look_at", &["attention_target"], 200_000),
        Err(ExtensionValidationError::CostBudgetExceeded)
    );
}

#[test]
fn extension_manifest_rejects_non_deterministic_or_non_isolated_channels() {
    let mut invalid = manifest();
    invalid.channels[0].deterministic = false;
    invalid.channels[0].failure_isolated = false;
    let errors = invalid.validate().unwrap_err();
    assert!(errors.iter().any(|error| error.contains("deterministic")));
    assert!(errors.iter().any(|error| error.contains("isolated")));
}
