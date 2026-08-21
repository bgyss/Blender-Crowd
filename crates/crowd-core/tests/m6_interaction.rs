use crowd_core::interaction::{
    deterministic_paired_clip, ContactLabelV1, InteractionMotionV1, InteractionRequestV1,
    INTERACTION_MOTION_SCHEMA_VERSION, INTERACTION_REQUEST_SCHEMA_VERSION,
};

fn fixture_request() -> InteractionRequestV1 {
    serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-request-v1.json"
    ))
    .expect("interaction request fixture")
}

fn fixture_motion() -> InteractionMotionV1 {
    serde_json::from_str(include_str!(
        "../../../assets/reference/m6/interaction-motion-v1.json"
    ))
    .expect("interaction motion fixture")
}

#[test]
fn strict_request_and_motion_fixtures_validate() {
    let request = fixture_request();
    let motion = fixture_motion();

    assert_eq!(request.schema_version, INTERACTION_REQUEST_SCHEMA_VERSION);
    assert_eq!(motion.schema_version, INTERACTION_MOTION_SCHEMA_VERSION);
    request.validate().expect("request is valid");
    motion
        .validate_against(&request)
        .expect("authored motion is valid");
}

#[test]
fn paired_clip_output_is_identical_for_repeated_strict_requests() {
    let request = fixture_request();
    let first = deterministic_paired_clip(&request).expect("paired clip request");
    let second = deterministic_paired_clip(&request).expect("paired clip request");

    assert_eq!(first, second);
    assert_eq!(first.provenance.backend, "authored-paired-clip");
    assert_eq!(first.provenance.model_hash, None);
}

#[test]
fn paired_clip_contains_the_declared_required_contact_and_no_forbidden_contact() {
    let request = fixture_request();
    let motion = deterministic_paired_clip(&request).expect("paired clip request");

    assert!(motion
        .contacts
        .iter()
        .any(|contact| contact.label == ContactLabelV1::Touch));
    assert!(!motion
        .contacts
        .iter()
        .any(|contact| contact.label == ContactLabelV1::Forbidden));
}

#[test]
fn interaction_motion_round_trips_without_losing_provenance() {
    let request = fixture_request();
    let motion = deterministic_paired_clip(&request).expect("paired clip request");
    let json = serde_json::to_string(&motion).expect("serialize motion");
    let restored: InteractionMotionV1 = serde_json::from_str(&json).expect("restore motion");

    assert_eq!(restored, motion);
    assert_eq!(restored.provenance.seed, request.seed);
}
