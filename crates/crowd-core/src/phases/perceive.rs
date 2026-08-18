//! Tick phase 3: perceive.
//!
//! Collects each agent's nearest neighbors under a fixed budget. Sorting by
//! `(distance_squared, agent_id)` matters: with distance alone, two exactly
//! equidistant neighbors would be ordered by slot layout, and a budget cutoff
//! between them would silently depend on spawn history.

use crate::arena::{Neighbor, NeighborArena};
use crate::fidelity::{FidelityPolicy, SimulationTier};
use crate::grid::UniformGrid;
use crate::units::Vec2;
use crate::world::World;

/// Perception limits. Contract section 6 calls these a per-tier budget; this
/// slice has one tier, so they are global.
#[derive(Clone, Copy, Debug)]
pub struct PerceiveConfig {
    pub query_radius: f32,
    pub budget: usize,
}

impl Default for PerceiveConfig {
    fn default() -> Self {
        Self {
            query_radius: 5.0,
            budget: 16,
        }
    }
}

/// Reused buffers, so the phase does not allocate after warmup.
#[derive(Clone, Debug, Default)]
pub struct PerceiveScratch {
    candidates: Vec<u32>,
    accepted: Vec<Neighbor>,
}

pub fn perceive(
    world: &World,
    grid: &UniformGrid,
    config: &PerceiveConfig,
    scratch: &mut PerceiveScratch,
    arena: &mut NeighborArena,
) {
    perceive_with_schedule(world, grid, config, scratch, arena, |_| true);
}

/// M5 scheduled perception. S0/S1 query every tick; S2 queries every other
/// tick on a stable-ID stagger; and S3 uses its flow/cache representation
/// rather than an individual neighbor list. The arena still has one
/// deterministic empty entry for each skipped slot, so downstream phases
/// retain their stable indexing contract.
pub fn perceive_scheduled(
    world: &World,
    grid: &UniformGrid,
    config: &PerceiveConfig,
    scratch: &mut PerceiveScratch,
    arena: &mut NeighborArena,
    tick: u64,
) {
    perceive_with_schedule(world, grid, config, scratch, arena, |slot| {
        match world.simulation_tier[slot] {
            SimulationTier::S0 | SimulationTier::S1 => true,
            SimulationTier::S2 => FidelityPolicy::s2_update_due(world.agent_id[slot], tick),
            SimulationTier::S3 => false,
        }
    });
}

fn perceive_with_schedule(
    world: &World,
    grid: &UniformGrid,
    config: &PerceiveConfig,
    scratch: &mut PerceiveScratch,
    arena: &mut NeighborArena,
    should_query: impl Fn(usize) -> bool,
) {
    arena.begin(world.len());
    let radius_sq = config.query_radius * config.query_radius;

    for slot in 0..world.len() {
        if !should_query(slot) {
            arena.push_unobserved(slot);
            continue;
        }
        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        grid.query(position, config.query_radius, &mut scratch.candidates);

        scratch.accepted.clear();
        for &candidate in &scratch.candidates {
            if candidate as usize == slot {
                continue;
            }
            // An agent that reached its destination has left the scene. It
            // keeps its last position for playback, but it must stop
            // obstructing: otherwise the first arrivals park on the goal and
            // become a permanent plug that blocks everyone behind them, and
            // destination completion collapses to a few percent.
            if world.arrived[candidate as usize] {
                continue;
            }
            let other = Vec2::new(
                world.pos_x[candidate as usize],
                world.pos_y[candidate as usize],
            );
            let dist_sq = position.distance_squared(other);
            if dist_sq <= radius_sq {
                scratch.accepted.push(Neighbor {
                    slot: candidate,
                    dist_sq,
                });
            }
        }

        // `total_cmp` gives a total order over floats without NaN ambiguity;
        // the agent-ID tiebreak makes the result independent of slot layout.
        scratch.accepted.sort_unstable_by(|a, b| {
            a.dist_sq
                .total_cmp(&b.dist_sq)
                .then_with(|| world.agent_id[a.slot as usize].cmp(&world.agent_id[b.slot as usize]))
        });
        scratch.accepted.truncate(config.budget);

        arena.push(slot, &scratch.accepted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fidelity::S2_UPDATE_INTERVAL_TICKS;
    use crate::ids::AgentId;
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn world_at(points: &[Vec2]) -> World {
        let mut world = World::new();
        for (i, p) in points.iter().enumerate() {
            world
                .spawn(
                    AgentSpawn {
                        agent_id: AgentId(i as u64 + 1),
                        population_id: 0,
                        position: *p,
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

    fn perceive_world(world: &World, config: &PerceiveConfig) -> NeighborArena {
        let mut grid = UniformGrid::new(
            Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0)),
            config.query_radius,
        );
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut scratch = PerceiveScratch::default();
        let mut arena = NeighborArena::new();
        perceive(world, &grid, config, &mut scratch, &mut arena);
        arena
    }

    #[test]
    fn an_agent_never_perceives_itself() {
        let world = world_at(&[Vec2::ZERO]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert!(arena.neighbors(0).is_empty());
    }

    #[test]
    fn nearby_agents_are_perceived_reciprocally() {
        let world = world_at(&[Vec2::ZERO, Vec2::new(1.0, 0.0)]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert_eq!(arena.neighbors(0).len(), 1);
        assert_eq!(arena.neighbors(0)[0].slot, 1);
        assert_eq!(arena.neighbors(1)[0].slot, 0);
    }

    #[test]
    fn background_perception_is_deterministically_scheduled() {
        let mut world = world_at(&[Vec2::ZERO, Vec2::new(1.0, 0.0)]);
        world.simulation_tier[0] = SimulationTier::S2;
        world.simulation_tier[1] = SimulationTier::S3;
        let mut grid = UniformGrid::new(
            Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0)),
            5.0,
        );
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut scratch = PerceiveScratch::default();
        let mut arena = NeighborArena::new();
        let due_tick = (0..S2_UPDATE_INTERVAL_TICKS)
            .find(|tick| FidelityPolicy::s2_update_due(world.agent_id[0], *tick))
            .unwrap();
        let skipped_tick = (due_tick + 1) % S2_UPDATE_INTERVAL_TICKS;
        perceive_scheduled(
            &world,
            &grid,
            &PerceiveConfig::default(),
            &mut scratch,
            &mut arena,
            skipped_tick,
        );
        assert!(arena.neighbors(0).is_empty());
        perceive_scheduled(
            &world,
            &grid,
            &PerceiveConfig::default(),
            &mut scratch,
            &mut arena,
            due_tick,
        );
        assert_eq!(arena.neighbors(0).len(), 1);
        assert!(arena.neighbors(1).is_empty());
    }

    #[test]
    fn agents_beyond_the_query_radius_are_excluded() {
        let config = PerceiveConfig {
            query_radius: 2.0,
            budget: 16,
        };
        let world = world_at(&[Vec2::ZERO, Vec2::new(50.0, 0.0)]);
        let arena = perceive_world(&world, &config);
        assert!(arena.neighbors(0).is_empty());
    }

    #[test]
    fn neighbors_are_sorted_nearest_first() {
        let world = world_at(&[
            Vec2::ZERO,
            Vec2::new(3.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
        ]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        let slots: Vec<u32> = arena.neighbors(0).iter().map(|n| n.slot).collect();
        assert_eq!(slots, vec![2, 3, 1]);
    }

    #[test]
    fn the_budget_keeps_only_the_nearest_neighbors() {
        let points: Vec<Vec2> = (0..20).map(|i| Vec2::new(i as f32 * 0.2, 0.0)).collect();
        let world = world_at(&points);
        let config = PerceiveConfig {
            query_radius: 10.0,
            budget: 4,
        };
        let arena = perceive_world(&world, &config);
        assert_eq!(arena.neighbors(0).len(), 4);
        let slots: Vec<u32> = arena.neighbors(0).iter().map(|n| n.slot).collect();
        assert_eq!(slots, vec![1, 2, 3, 4]);
    }

    #[test]
    fn equidistant_neighbors_are_ordered_by_stable_id() {
        // Two neighbors at identical distance: the tie must resolve by agent
        // ID, or a budget cutoff would silently depend on slot layout.
        let world = world_at(&[Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0)]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        let ids: Vec<u64> = arena
            .neighbors(0)
            .iter()
            .map(|n| world.agent_id[n.slot as usize].0)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn an_arrived_agent_does_not_obstruct_others() {
        // Regression: agents that reach a destination park on it and never
        // move again. If they still register as neighbours, the first
        // arrivals become a permanent plug sitting on the goal and everyone
        // behind them jams -- destination completion collapsed from ~85% to
        // single digits before this was fixed.
        let mut world = world_at(&[Vec2::ZERO, Vec2::new(1.0, 0.0)]);
        world.arrived[1] = true;
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert!(
            arena.neighbors(0).is_empty(),
            "an agent that has left the scene still obstructs"
        );
        // The reverse still holds: an agent still in the scene is visible.
        world.arrived[1] = false;
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert_eq!(arena.neighbors(0).len(), 1);
    }

    #[test]
    fn perceiving_an_empty_world_is_a_no_op() {
        let world = World::new();
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert_eq!(arena.capacity(), 0);
    }
}
