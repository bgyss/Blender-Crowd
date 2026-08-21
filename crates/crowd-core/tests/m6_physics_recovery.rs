use crowd_core::physics::{
    recovery_phase, validate_transition, FailurePolicyV1, HeroIntegrationBoundaryV1,
    PhysicsTransitionV1, RecoveryPhaseV1, RigidBodyLayerV1,
};

fn transition() -> PhysicsTransitionV1 {
    serde_json::from_str(include_str!(
        "../../../assets/reference/m6/physics-transition-v1.json"
    ))
    .unwrap()
}

#[test]
fn physics_transition_requires_explicit_owner_cache_recovery_and_failure_policy() {
    let transition = transition();
    validate_transition(&transition).unwrap();
    assert_eq!(transition.failure_policy, FailurePolicyV1::Fallback);
}

#[test]
fn invalid_physics_transition_is_rejected_before_layer_composition() {
    let mut transition = transition();
    transition.solver.clear();
    transition.recovery.clear();
    transition.tick_end = transition.tick_start - 1;
    let errors = validate_transition(&transition).unwrap_err();
    assert!(errors.iter().any(|error| error.contains("solver")));
    assert!(errors.iter().any(|error| error.contains("recovery")));
    assert!(errors.iter().any(|error| error.contains("tick")));
}

#[test]
fn hero_solver_declaration_is_optional_and_names_its_support_boundaries() {
    let boundary = HeroIntegrationBoundaryV1 {
        integration_id: "hero-cloth-7".to_owned(),
        solver: "blender-cloth".to_owned(),
        cache_policy: "adjacent-layer".to_owned(),
        supported_render_tiers: vec!["hero".to_owned()],
        failure_policy: "fallback-to-cached-body".to_owned(),
    };
    boundary.validate().unwrap();
}

#[test]
fn recovery_phase_is_deterministic_and_rigid_body_ownership_is_explicit() {
    let transition = transition();
    assert_eq!(recovery_phase(&transition, 20, 3), RecoveryPhaseV1::Impact);
    assert_eq!(
        recovery_phase(&transition, 21, 3),
        RecoveryPhaseV1::Stabilize
    );
    assert_eq!(recovery_phase(&transition, 23, 3), RecoveryPhaseV1::Resume);
    assert_eq!(recovery_phase(&transition, 100, 3), RecoveryPhaseV1::Resume);

    let rigid_body = RigidBodyLayerV1 {
        layer_id: "rigid-hero-7".to_owned(),
        owner_agent_ids: vec![7],
        solver: "deterministic-kinematic-reference".to_owned(),
        collision_masks: vec!["crowd".to_owned(), "ground".to_owned()],
        recovery_transition_id: transition.transition_id,
    };
    rigid_body.validate().unwrap();
}
