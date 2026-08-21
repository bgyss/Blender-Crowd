//! Executable Rust example for the versioned M6 extension boundary.

use crowd_core::extensions::{
    ExtensionChannelV1, ExtensionManifestV1, ExtensionValidationError, EXTENSION_SCHEMA_VERSION,
};
use serde_json::{json, Value};

const CHANNEL_VERSION: u32 = 1;
const COST_BUDGET_MILLIONTHS: u32 = 100_000;

fn manifest() -> ExtensionManifestV1 {
    ExtensionManifestV1::new(
        "studio-look-at-rust",
        vec![ExtensionChannelV1 {
            name: "look_at".to_owned(),
            version: CHANNEL_VERSION,
            inputs: vec!["attention_target".to_owned()],
            outputs: vec!["gaze_offset".to_owned()],
            cost_budget_millionths: COST_BUDGET_MILLIONTHS,
            deterministic: true,
            failure_isolated: true,
        }],
    )
    .expect("the checked example manifest is valid")
}

fn record(case: &str, status: &str, reason: Option<&str>, value: Value) -> Value {
    json!({
        "case": case,
        "status": status,
        "reason": reason,
        "schema_version": EXTENSION_SCHEMA_VERSION,
        "channel_version": CHANNEL_VERSION,
        "inputs": ["attention_target"],
        "outputs": ["gaze_offset"],
        "cost_budget_millionths": COST_BUDGET_MILLIONTHS,
        "deterministic": true,
        "failure_isolated": true,
        "value": value,
    })
}

fn emit(value: Value) {
    println!(
        "{}",
        serde_json::to_string(&value).expect("example record must serialize")
    );
}

fn main() {
    let declared = manifest();

    declared
        .validate_call("look_at", &["attention_target"], 50_000)
        .expect("accepted call must satisfy the declared boundary");
    emit(record(
        "accepted_call",
        "accepted",
        None,
        json!({"gaze_offset": [0, 0, 0]}),
    ));

    let over_budget = declared.validate_call("look_at", &["attention_target"], 100_001);
    assert_eq!(
        over_budget,
        Err(ExtensionValidationError::CostBudgetExceeded)
    );
    emit(record(
        "over_budget_call",
        "fallback",
        Some("cost_budget_exceeded"),
        json!({"gaze_offset": [0, 0, 0]}),
    ));

    let undeclared = declared.validate_call("look_at", &["private_state"], 50_000);
    assert_eq!(
        undeclared,
        Err(ExtensionValidationError::UndeclaredInput(
            "private_state".to_owned()
        ))
    );
    emit(record(
        "undeclared_channel_call",
        "rejected",
        Some("undeclared_input"),
        Value::Null,
    ));

    let mut incompatible = declared;
    incompatible.schema_version = EXTENSION_SCHEMA_VERSION + 1;
    let mismatch = incompatible.validate_call("look_at", &["attention_target"], 50_000);
    assert_eq!(
        mismatch,
        Err(ExtensionValidationError::UnsupportedVersion(
            EXTENSION_SCHEMA_VERSION + 1
        ))
    );
    emit(record(
        "version_mismatch_call",
        "rejected",
        Some("unsupported_version"),
        Value::Null,
    ));
}
