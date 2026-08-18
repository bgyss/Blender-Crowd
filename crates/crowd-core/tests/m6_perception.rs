use crowd_core::arena::{Neighbor, NeighborArena};
use crowd_core::geometry::Segment;
use crowd_core::ids::AgentId;
use crowd_core::perception::{
    ObservationChannelV1, PerceptionConfigV1, PerceptionEngine, PerceptionValueV1,
};
use crowd_core::units::Vec2;
use crowd_core::world::{AgentSpawn, World, NO_ROUTE};

fn world_with_agents() -> World {
    let mut world = World::new();
    for (agent_id, position) in [
        (1, Vec2::new(0.0, 0.0)),
        (2, Vec2::new(4.0, 0.0)),
        (3, Vec2::new(0.0, 3.0)),
    ] {
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(agent_id),
                    population_id: 0,
                    position,
                    yaw: 0.0,
                    radius: 0.3,
                    max_speed: 1.8,
                    preferred_speed: 1.35,
                    route: NO_ROUTE,
                    destination: 0,
                },
                0,
            )
            .unwrap();
    }
    world
}

fn neighbors_in_stable_and_reverse_order(reverse: bool) -> NeighborArena {
    let mut arena = NeighborArena::new();
    arena.begin(3);
    let mut neighbors = vec![
        Neighbor {
            slot: 1,
            dist_sq: 16.0,
        },
        Neighbor {
            slot: 2,
            dist_sq: 9.0,
        },
    ];
    if reverse {
        neighbors.reverse();
    }
    arena.push(0, &neighbors);
    arena.push(
        1,
        &[Neighbor {
            slot: 0,
            dist_sq: 16.0,
        }],
    );
    arena.push(
        2,
        &[Neighbor {
            slot: 0,
            dist_sq: 9.0,
        }],
    );
    arena
}

fn value(
    snapshot: &crowd_core::perception::PerceptionSnapshotV1,
    channel: ObservationChannelV1,
) -> Vec<PerceptionValueV1> {
    snapshot
        .observations
        .iter()
        .filter(|observation| observation.channel == channel)
        .map(|observation| observation.value.clone())
        .collect()
}

#[test]
fn perception_is_stable_when_neighbor_input_order_changes() {
    let world = world_with_agents();
    let mut first_engine = PerceptionEngine::new(PerceptionConfigV1::default());
    let mut second_engine = PerceptionEngine::new(PerceptionConfigV1::default());
    let first = first_engine.observe(&world, &neighbors_in_stable_and_reverse_order(false), 12);
    let second = second_engine.observe(&world, &neighbors_in_stable_and_reverse_order(true), 12);

    assert_eq!(first, second);
}

#[test]
fn occluded_agents_are_not_reported_as_visible_but_touch_hearing_and_semantics_remain_typed() {
    let world = world_with_agents();
    let config = PerceptionConfigV1 {
        vision_half_angle_rad: 1.0,
        ..PerceptionConfigV1::default()
    };
    let mut engine = PerceptionEngine::new(config);
    engine.set_occluders(vec![Segment::new(
        Vec2::new(2.0, -1.0),
        Vec2::new(2.0, 1.0),
    )]);
    engine.set_group_members("pair", vec![AgentId(1), AgentId(2)]);
    engine.set_friendship(AgentId(1), AgentId(2), true);
    engine.set_touch_event(AgentId(1), AgentId(2));
    engine.set_hearing_event(AgentId(1), "door-open");
    engine.set_semantic_distance_millionths(AgentId(1), "seat", 1_250_000);

    let snapshots = engine.observe(&world, &neighbors_in_stable_and_reverse_order(false), 12);
    let snapshot = &snapshots[&AgentId(1)];
    assert!(value(snapshot, ObservationChannelV1::VisionAgent).is_empty());
    assert_eq!(
        value(snapshot, ObservationChannelV1::Hearing),
        vec![PerceptionValueV1::Text("door-open".to_owned())]
    );
    assert_eq!(
        value(snapshot, ObservationChannelV1::Touch),
        vec![PerceptionValueV1::Agent(AgentId(2))]
    );
    assert_eq!(
        value(snapshot, ObservationChannelV1::SemanticDistance),
        vec![PerceptionValueV1::NumberI32(1_250_000)]
    );
    assert_eq!(
        value(snapshot, ObservationChannelV1::GroupExtent),
        vec![PerceptionValueV1::NumberI32(4_000_000)]
    );
}

#[test]
fn observation_budget_is_explicit_and_memory_expires_deterministically() {
    let world = world_with_agents();
    let config = PerceptionConfigV1 {
        observation_budget: 2,
        ..PerceptionConfigV1::default()
    };
    let mut engine = PerceptionEngine::new(config);
    engine.remember(
        AgentId(1),
        "last_action",
        PerceptionValueV1::Text("wave".to_owned()),
        13,
    );

    let snapshots = engine.observe(&world, &neighbors_in_stable_and_reverse_order(false), 12);
    assert_eq!(snapshots[&AgentId(1)].observations.len(), 2);
    assert!(snapshots[&AgentId(1)].degraded_evidence.is_some());
    assert_eq!(snapshots[&AgentId(1)].memory.len(), 1);

    let later = engine.observe(&world, &neighbors_in_stable_and_reverse_order(false), 14);
    assert!(later[&AgentId(1)].memory.is_empty());
}

#[test]
fn perception_snapshots_are_ordered_by_stable_agent_id() {
    let world = world_with_agents();
    let mut engine = PerceptionEngine::new(PerceptionConfigV1::default());
    let snapshots = engine.observe(&world, &neighbors_in_stable_and_reverse_order(false), 12);
    let ids: Vec<_> = snapshots.keys().copied().collect();
    assert_eq!(ids, vec![AgentId(1), AgentId(2), AgentId(3)]);
    assert_eq!(snapshots[&AgentId(1)].agent_id, AgentId(1));
}
