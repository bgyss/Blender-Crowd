use crowd_core::interaction::{
    ContactLabelV1, InteractionIssueCode, InteractionMotionV1, InteractionRequestV1,
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
fn motion_rejects_root_teleportation_and_forbidden_contact() {
    let request = fixture_request();
    let mut motion = fixture_motion();
    motion.participants[0].root_samples[1].translation = [100.0, 0.0, 0.0];
    motion
        .contacts
        .push(crowd_core::interaction::MotionContactV1 {
            contact_id: "separate-7-9".to_owned(),
            label: ContactLabelV1::Forbidden,
            owner_agent_id: 7,
            other_agent_id: 9,
            tick: 20,
            distance_m: 0.0,
        });

    let errors = motion.validate_against(&request).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == InteractionIssueCode::RootDeviation));
    assert!(errors
        .iter()
        .any(|error| error.code == InteractionIssueCode::ForbiddenContact));
}

#[test]
fn request_rejects_duplicate_participants_and_reversed_ranges() {
    let mut request = fixture_request();
    request.participants[1].agent_id = request.participants[0].agent_id;
    request.tick_end = request.tick_start - 1;

    let errors = request.validate().unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == InteractionIssueCode::DuplicateParticipant));
    assert!(errors
        .iter()
        .any(|error| error.code == InteractionIssueCode::InvalidTickRange));
}

#[test]
fn motion_rejects_a_missing_required_contact() {
    let request = fixture_request();
    let mut motion = fixture_motion();
    motion.contacts.clear();

    let errors = motion.validate_against(&request).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.code == InteractionIssueCode::RequiredContactMissing));
}
