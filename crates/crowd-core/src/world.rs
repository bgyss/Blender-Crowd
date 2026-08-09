//! Structure-of-arrays agent state.
//!
//! Contract section 5.2. Each hot field is its own `Vec` indexed by dense
//! slot; a stable-ID-to-slot table keeps IDs stable while slots stay dense.
//! Slot order is derived from stable IDs, so iteration order is deterministic
//! by construction.
//!
//! Only fields this slice writes exist. `group_id`, `fidelity_tier`,
//! `blackboard_handle`, and the animation columns from the contract are
//! omitted because nothing would write them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{hash_combine, AgentId};
use crate::units::Vec2;

/// A handle into the route arena. `NO_ROUTE` means "no path assigned".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteHandle(pub u32);

pub const NO_ROUTE: RouteHandle = RouteHandle(u32::MAX);

/// Why the avoidance solver produced the velocity it did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolverStatus {
    /// No neighbor or wall constrained the choice.
    #[default]
    Free,
    /// A constraint moved the agent off its preferred velocity.
    Avoiding,
    /// No candidate was feasible; the agent slowed or stopped.
    Braking,
}

/// Everything needed to introduce one agent.
#[derive(Clone, Copy, Debug)]
pub struct AgentSpawn {
    pub agent_id: AgentId,
    pub population_id: u16,
    pub position: Vec2,
    pub yaw: f32,
    pub radius: f32,
    pub max_speed: f32,
    pub preferred_speed: f32,
    pub route: RouteHandle,
    pub destination: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnError {
    /// Contract section 10.3 makes this a bake-blocking condition.
    DuplicateAgentId(AgentId),
}

/// Dense structure-of-arrays agent storage.
#[derive(Clone, Debug, Default)]
pub struct World {
    // Identity.
    pub agent_id: Vec<AgentId>,
    pub population_id: Vec<u16>,
    pub spawn_tick: Vec<u64>,

    // Kinematic.
    pub pos_x: Vec<f32>,
    pub pos_y: Vec<f32>,
    pub yaw: Vec<f32>,
    pub vel_x: Vec<f32>,
    pub vel_y: Vec<f32>,
    pub radius: Vec<f32>,
    pub max_speed: Vec<f32>,
    pub preferred_speed: Vec<f32>,

    // Navigation.
    pub route: Vec<RouteHandle>,
    pub route_index: Vec<u16>,
    pub destination: Vec<u16>,
    pub arrived: Vec<bool>,
    /// Set when an agent has no usable route at all.
    ///
    /// Kept distinct from `arrived` because both stop the agent, but they mean
    /// opposite things: one is a destination reached, the other is a
    /// navigation failure. Sharing a flag would let routing failures be
    /// counted as destination completions in the headline metric.
    pub unrouted: Vec<bool>,

    // Staging. Written by steer, consumed by integrate.
    pub des_vel_x: Vec<f32>,
    pub des_vel_y: Vec<f32>,
    pub next_pos_x: Vec<f32>,
    pub next_pos_y: Vec<f32>,
    pub next_vel_x: Vec<f32>,
    pub next_vel_y: Vec<f32>,
    pub next_yaw: Vec<f32>,

    // Debug.
    pub solver_status: Vec<SolverStatus>,
    pub stall_ticks: Vec<u16>,

    slot_of_id: BTreeMap<AgentId, u32>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.agent_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.agent_id.is_empty()
    }

    pub fn slot_of(&self, id: AgentId) -> Option<u32> {
        self.slot_of_id.get(&id).copied()
    }

    pub fn position(&self, slot: u32) -> Vec2 {
        Vec2::new(self.pos_x[slot as usize], self.pos_y[slot as usize])
    }

    pub fn velocity(&self, slot: u32) -> Vec2 {
        Vec2::new(self.vel_x[slot as usize], self.vel_y[slot as usize])
    }

    pub fn desired_velocity(&self, slot: u32) -> Vec2 {
        Vec2::new(self.des_vel_x[slot as usize], self.des_vel_y[slot as usize])
    }

    pub fn spawn(&mut self, spawn: AgentSpawn, tick: u64) -> Result<u32, SpawnError> {
        if self.slot_of_id.contains_key(&spawn.agent_id) {
            return Err(SpawnError::DuplicateAgentId(spawn.agent_id));
        }
        let slot = self.agent_id.len() as u32;

        self.agent_id.push(spawn.agent_id);
        self.population_id.push(spawn.population_id);
        self.spawn_tick.push(tick);

        self.pos_x.push(spawn.position.x);
        self.pos_y.push(spawn.position.y);
        self.yaw.push(spawn.yaw);
        self.vel_x.push(0.0);
        self.vel_y.push(0.0);
        self.radius.push(spawn.radius);
        self.max_speed.push(spawn.max_speed);
        self.preferred_speed.push(spawn.preferred_speed);

        self.route.push(spawn.route);
        self.route_index.push(0);
        self.destination.push(spawn.destination);
        self.arrived.push(false);
        self.unrouted.push(false);

        self.des_vel_x.push(0.0);
        self.des_vel_y.push(0.0);
        self.next_pos_x.push(spawn.position.x);
        self.next_pos_y.push(spawn.position.y);
        self.next_vel_x.push(0.0);
        self.next_vel_y.push(0.0);
        self.next_yaw.push(spawn.yaw);

        self.solver_status.push(SolverStatus::Free);
        self.stall_ticks.push(0);

        self.slot_of_id.insert(spawn.agent_id, slot);
        Ok(slot)
    }

    /// Publish staged next-state into current state.
    ///
    /// Called once at the end of a tick. Until this runs, every phase reads a
    /// consistent snapshot of the previous tick, which is what makes results
    /// independent of iteration order.
    pub fn commit(&mut self) {
        // `copy_from_slice` panics on a length mismatch, and the tick loop is
        // supposed to be infallible. `spawn` is the only mutator that keeps
        // the columns in step, but phases hold `&mut World` and could push to
        // one column alone; this catches that in tests rather than in a bake.
        debug_assert!(
            self.next_pos_x.len() == self.len()
                && self.next_pos_y.len() == self.len()
                && self.next_vel_x.len() == self.len()
                && self.next_vel_y.len() == self.len()
                && self.next_yaw.len() == self.len(),
            "staged columns drifted out of step with the agent count"
        );
        self.pos_x.copy_from_slice(&self.next_pos_x);
        self.pos_y.copy_from_slice(&self.next_pos_y);
        self.vel_x.copy_from_slice(&self.next_vel_x);
        self.vel_y.copy_from_slice(&self.next_vel_y);
        self.yaw.copy_from_slice(&self.next_yaw);
    }

    /// A bitwise digest of all authoritative agent state.
    ///
    /// # What is deliberately omitted, and why that is safe
    ///
    /// `population_id`, `radius`, `max_speed`, `preferred_speed`,
    /// `destination`, `spawn_tick`, `solver_status`, `stall_ticks` and
    /// `unrouted` are excluded. Every one is either fixed at spawn or derived
    /// from state already hashed, so including them would add nothing a
    /// divergence could hide behind.
    ///
    /// `route` is **included**, unlike the fields above: the tiled-navmesh
    /// plan phase can reassign it mid-run (a portal close invalidates and
    /// reroutes a corridor), so it is no longer fixed at spawn for every
    /// scene. Hashing the raw handle index is enough — it is deterministic
    /// given deterministic `RouteArena` push order, so two runs that assign
    /// different routes to the same agent diverge here immediately, rather
    /// than only once the resulting steering difference shows up in position.
    ///
    /// **This invariant is load-bearing.** If a later change makes any
    /// currently-excluded field mutable within the tick loop, it must be
    /// added here — otherwise the determinism tests keep passing while
    /// silently ignoring the field that diverged.
    ///
    /// Hashes float *bits*, not values, so the determinism tests compare
    /// exactly rather than within a tolerance.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xa5a5_5a5a_dead_beef;
        for slot in 0..self.len() {
            h = hash_combine(h, self.agent_id[slot].0);
            h = hash_combine(h, canonical_bits(self.pos_x[slot]));
            h = hash_combine(h, canonical_bits(self.pos_y[slot]));
            h = hash_combine(h, canonical_bits(self.vel_x[slot]));
            h = hash_combine(h, canonical_bits(self.vel_y[slot]));
            h = hash_combine(h, canonical_bits(self.yaw[slot]));
            h = hash_combine(h, self.route[slot].0 as u64);
            h = hash_combine(h, self.route_index[slot] as u64);
            h = hash_combine(h, self.arrived[slot] as u64);
        }
        h
    }
}

/// Float bits, with negative zero folded onto positive zero.
///
/// `-0.0 == 0.0` is true, but their bit patterns differ. Without this fold, a
/// velocity component that cancelled to `-0.0` on one path and `0.0` on
/// another would report a determinism failure for two states that are
/// numerically identical — a false alarm on the test the whole slice is built
/// to trust. Every other value, NaN included, keeps its exact bits: a real
/// divergence must still be caught.
fn canonical_bits(value: f32) -> u64 {
    if value == 0.0 {
        0
    } else {
        value.to_bits() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    fn spawn_at(id: u64, position: Vec2) -> AgentSpawn {
        AgentSpawn {
            agent_id: AgentId(id),
            population_id: 0,
            position,
            yaw: 0.0,
            radius: 0.3,
            max_speed: 1.8,
            preferred_speed: 1.35,
            route: NO_ROUTE,
            destination: 0,
        }
    }

    #[test]
    fn new_world_is_empty() {
        let world = World::new();
        assert_eq!(world.len(), 0);
        assert!(world.is_empty());
    }

    #[test]
    fn spawn_appends_a_slot_and_records_state() {
        let mut world = World::new();
        let slot = world.spawn(spawn_at(1, Vec2::new(2.0, 3.0)), 0).unwrap();
        assert_eq!(slot, 0);
        assert_eq!(world.len(), 1);
        assert_eq!(world.position(0), Vec2::new(2.0, 3.0));
        assert_eq!(world.agent_id[0], AgentId(1));
        assert_eq!(world.spawn_tick[0], 0);
        assert_eq!(world.velocity(0), Vec2::ZERO);
    }

    #[test]
    fn spawn_rejects_duplicate_agent_ids() {
        let mut world = World::new();
        world.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        let err = world.spawn(spawn_at(1, Vec2::ZERO), 1).unwrap_err();
        assert_eq!(err, SpawnError::DuplicateAgentId(AgentId(1)));
    }

    #[test]
    fn slot_of_round_trips_stable_ids() {
        let mut world = World::new();
        world.spawn(spawn_at(10, Vec2::ZERO), 0).unwrap();
        world.spawn(spawn_at(20, Vec2::ZERO), 0).unwrap();
        assert_eq!(world.slot_of(AgentId(20)), Some(1));
        assert_eq!(world.slot_of(AgentId(30)), None);
    }

    #[test]
    fn all_columns_stay_the_same_length() {
        let mut world = World::new();
        for i in 0..25 {
            world.spawn(spawn_at(i, Vec2::ZERO), 0).unwrap();
        }
        let n = world.len();
        assert_eq!(world.pos_x.len(), n);
        assert_eq!(world.pos_y.len(), n);
        assert_eq!(world.vel_x.len(), n);
        assert_eq!(world.next_pos_x.len(), n);
        assert_eq!(world.solver_status.len(), n);
        assert_eq!(world.stall_ticks.len(), n);
        assert_eq!(world.route_index.len(), n);
    }

    #[test]
    fn commit_moves_next_state_into_current() {
        let mut world = World::new();
        world.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        world.next_pos_x[0] = 5.0;
        world.next_pos_y[0] = 6.0;
        world.next_vel_x[0] = 1.0;
        world.next_vel_y[0] = 0.0;
        world.next_yaw[0] = 0.5;
        world.commit();
        assert_eq!(world.position(0), Vec2::new(5.0, 6.0));
        assert_eq!(world.velocity(0), Vec2::new(1.0, 0.0));
        assert_eq!(world.yaw[0], 0.5);
    }

    #[test]
    fn state_hash_changes_when_state_changes() {
        let mut world = World::new();
        world.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        let before = world.state_hash();
        world.next_pos_x[0] = 1.0;
        world.commit();
        assert_ne!(world.state_hash(), before);
    }

    #[test]
    fn state_hash_ignores_the_sign_of_zero() {
        // -0.0 == 0.0 numerically but differs bitwise. Without folding, a
        // component that cancelled to -0.0 on one path would look like a
        // determinism failure against a state that is numerically identical.
        let mut positive = World::new();
        let mut negative = World::new();
        positive.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        negative.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        negative.next_vel_x[0] = -0.0;
        negative.next_pos_y[0] = -0.0;
        negative.commit();
        assert_eq!(positive.state_hash(), negative.state_hash());
    }

    #[test]
    fn state_hash_is_identical_for_identical_state() {
        let mut a = World::new();
        let mut b = World::new();
        for i in 0..10 {
            a.spawn(spawn_at(i, Vec2::new(i as f32, 0.0)), 0).unwrap();
            b.spawn(spawn_at(i, Vec2::new(i as f32, 0.0)), 0).unwrap();
        }
        assert_eq!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn state_hash_changes_when_the_route_handle_changes() {
        let mut a = World::new();
        let mut b = World::new();
        a.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        b.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        b.route[0] = RouteHandle(7);
        assert_ne!(
            a.state_hash(),
            b.state_hash(),
            "a mid-run route reassignment must be visible to the determinism hash"
        );
    }
}
