use crowd_core::{
    animate, compile_concourse, compile_project, AgentId, AgentSpawn, AnimateConfig, CommuterState,
    DecisionReason, ProjectIrV1, SampledVelocitySolver, SimConfig, Simulation, Vec2, World,
    IDLE_CLIP_ID, JOG_CLIP_ID, NO_ROUTE, WALK_CLIP_ID,
};

fn project(agent_count: u32) -> crowd_core::CompiledProject {
    let mut ir: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    ir.populations[0].count = agent_count;
    compile_project(&ir).unwrap()
}

fn concourse_simulation(agent_count: u32) -> (crowd_core::CompiledProject, Simulation) {
    let project = project(agent_count);
    let scene = compile_concourse(&project).unwrap();
    let simulation = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    (project, simulation)
}

#[test]
fn reference_project_compiles_to_a_real_concourse_scene() {
    let project = project(40);
    let scene = compile_concourse(&project).unwrap();

    assert_eq!(scene.total_agents(), 40);
    assert_eq!(scene.spawns.len(), 2);
    assert!(scene.spawns.iter().all(|spawn| spawn.per_tick == 1));
    assert_eq!(scene.runtime_spawn_interval_ticks, 5);
    assert_eq!(scene.runtime_spawn_start_ticks, vec![0, 4_000]);
    assert_eq!(scene.destinations.len(), 3);
    assert!(scene.nav.as_ref().unwrap().portals_named("east_gate").len() > 10);
    assert!(scene.nav.as_ref().unwrap().portals_named("west_gate").len() > 10);
    assert_eq!(scene.timed_portal_events.len(), 2);
    let east_exit_goals: std::collections::BTreeSet<_> = scene
        .agent_specs_by_spawn
        .iter()
        .flatten()
        .filter(|spec| spec.destination_id == 1)
        .map(|spec| {
            (
                spec.destination_point.x.to_bits(),
                spec.destination_point.y.to_bits(),
            )
        })
        .collect();
    assert!(east_exit_goals.len() > 5);
    let east_to_west_uses_north_lane = scene.agent_specs_by_spawn[0]
        .iter()
        .filter(|spec| spec.destination_id == 2)
        .all(|spec| spec.destination_point.y > 10.0);
    let west_to_east_uses_south_lane = scene.agent_specs_by_spawn[1]
        .iter()
        .filter(|spec| spec.destination_id == 1)
        .all(|spec| spec.destination_point.y < 10.0);
    assert!(east_to_west_uses_north_lane);
    assert!(west_to_east_uses_south_lane);
}

#[test]
fn spawned_agents_retain_compiled_static_choices() {
    let (project, mut sim) = concourse_simulation(12);
    sim.step();

    for snapshot in sim.frame_snapshot().agents {
        let expected = project
            .agent_spawns()
            .iter()
            .find(|agent| agent.agent_id == snapshot.agent_id)
            .unwrap();
        assert_eq!(snapshot.population_id, expected.population_id);
        assert_eq!(snapshot.archetype_id, expected.archetype_id);
        assert_eq!(snapshot.variant_id, expected.appearance_id);
        assert_eq!(snapshot.spawn_ordinal, expected.spawn_ordinal);
        assert_eq!(snapshot.scale.to_bits(), expected.scale.to_bits());
        assert_eq!(snapshot.destination_id, expected.destination_id);
    }
}

#[test]
fn authored_destination_capacity_finishes_agents_without_point_convergence() {
    let (_, mut sim) = concourse_simulation(40);
    sim.run(30);
    assert!(
        sim.metrics().arrived() > 0,
        "no commuter completed an authored destination region"
    );
}

#[test]
fn a_traveling_agent_uses_distance_to_advance_walk_phase() {
    let (_, mut sim) = concourse_simulation(1);
    sim.step();
    let before = sim.frame_snapshot().agents[0].clone();
    sim.step();
    let after = sim.frame_snapshot().agents[0].clone();

    assert_eq!(after.commuter_state, CommuterState::Travel);
    assert_eq!(after.clip_state.clip_id, WALK_CLIP_ID);
    assert!(after.clip_state.phase > before.clip_state.phase);
}

#[test]
fn portal_close_records_replan_reason_only_for_affected_routes() {
    let (_, mut sim) = concourse_simulation(40);
    sim.run(30);
    let unaffected = sim.agent_ids_not_using_portal("east_gate").unwrap();
    let affected: Vec<_> = sim
        .frame_snapshot()
        .agents
        .into_iter()
        .filter(|agent| !unaffected.contains(&agent.agent_id))
        .map(|agent| agent.agent_id)
        .collect();
    assert!(
        !affected.is_empty(),
        "fixture did not route through east_gate"
    );

    let invalidated = sim.set_named_portal_open("east_gate", false).unwrap();
    assert_eq!(invalidated, affected.len());
    for id in affected {
        assert_eq!(
            sim.query_agent(id).unwrap().decision_reason,
            DecisionReason::PortalClosedReplan
        );
    }
    for id in unaffected {
        assert_ne!(
            sim.query_agent(id).unwrap().decision_reason,
            DecisionReason::PortalClosedReplan
        );
    }
}

#[test]
fn timed_portal_inputs_apply_before_that_ticks_planning() {
    let mut ir: ProjectIrV1 = serde_json::from_str(include_str!(
        "../../../assets/reference/concourse-project-v1.json"
    ))
    .unwrap();
    ir.populations[0].count = 20;
    ir.portal_events[0].tick = 5;
    ir.portal_events[1].tick = 8;
    let project = compile_project(&ir).unwrap();
    let scene = compile_concourse(&project).unwrap();
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );

    sim.run(5);
    assert!(sim.named_portal_is_open("east_gate").unwrap());
    sim.step();
    assert!(!sim.named_portal_is_open("east_gate").unwrap());
    sim.run(3);
    assert!(sim.named_portal_is_open("east_gate").unwrap());
}

fn one_agent_world() -> World {
    let mut world = World::new();
    world
        .spawn(
            AgentSpawn {
                agent_id: AgentId(7),
                population_id: 0,
                position: Vec2::ZERO,
                yaw: 0.25,
                radius: 0.3,
                max_speed: 3.0,
                preferred_speed: 1.35,
                route: NO_ROUTE,
                destination: 0,
            },
            0,
        )
        .unwrap();
    world
}

#[test]
fn animate_selects_idle_walk_and_jog_without_writing_position() {
    let mut world = one_agent_world();
    let config = AnimateConfig {
        jog_threshold_mps: 1.8,
        ..AnimateConfig::default()
    };
    let original_position = world.position(0);

    animate(&mut world, &config);
    assert_eq!(world.clip_id[0], IDLE_CLIP_ID);
    assert_eq!(world.yaw[0], 0.25);

    world.next_pos_x[0] = 0.04;
    world.next_vel_x[0] = 1.2;
    animate(&mut world, &config);
    assert_eq!(world.clip_id[0], WALK_CLIP_ID);
    assert!((0.0..1.0).contains(&world.clip_phase[0]));

    world.next_pos_x[0] = 4.0;
    world.next_vel_x[0] = 2.0;
    animate(&mut world, &config);
    assert_eq!(world.clip_id[0], JOG_CLIP_ID);
    assert!((0.0..1.0).contains(&world.clip_phase[0]));
    assert_eq!(world.position(0), original_position);

    world.arrived[0] = true;
    animate(&mut world, &config);
    assert_eq!(world.commuter_state[0], CommuterState::Arrived);
    assert_eq!(world.clip_id[0], IDLE_CLIP_ID);
}

#[test]
fn commuter_and_animation_state_participate_in_the_determinism_hash() {
    let mut baseline = one_agent_world();
    let expected = baseline.state_hash();

    baseline.commuter_state[0] = CommuterState::Blocked;
    assert_ne!(baseline.state_hash(), expected);
    baseline = one_agent_world();
    baseline.decision_reason[0] = DecisionReason::PortalClosedReplan;
    assert_ne!(baseline.state_hash(), expected);
    baseline = one_agent_world();
    baseline.clip_id[0] = JOG_CLIP_ID;
    assert_ne!(baseline.state_hash(), expected);
    baseline = one_agent_world();
    baseline.clip_phase[0] = 0.5;
    assert_ne!(baseline.state_hash(), expected);
}

#[test]
fn commuter_and_decision_codes_are_stable_cache_values() {
    assert_eq!(CommuterState::Unspawned as u16, 0);
    assert_eq!(CommuterState::Travel as u16, 1);
    assert_eq!(CommuterState::Arrived as u16, 2);
    assert_eq!(CommuterState::Blocked as u16, 3);
    assert_eq!(DecisionReason::InitialDestination as u16, 1);
    assert_eq!(DecisionReason::FollowCorridor as u16, 2);
    assert_eq!(DecisionReason::PortalClosedReplan as u16, 3);
    assert_eq!(DecisionReason::PortalReopened as u16, 4);
    assert_eq!(DecisionReason::DestinationReached as u16, 5);
    assert_eq!(DecisionReason::NoRoute as u16, 6);
    assert!(DecisionReason::PortalClosedReplan
        .text()
        .contains("corridor invalidated"));
}
