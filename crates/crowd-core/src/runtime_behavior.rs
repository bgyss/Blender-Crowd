//! Adapter that executes compiled M2 graphs in the fixed-step decide phase.

use std::collections::{BTreeMap, BTreeSet};

use crate::arena::NeighborArena;
use crate::behavior::{
    BehaviorAction, BehaviorContext, BehaviorVm, BehaviorVmState, DecisionOutcome,
};
use crate::ids::AgentId;
use crate::world::{World, NO_ROUTE};

#[derive(Clone, Debug)]
pub struct RuntimeBehaviorController {
    pub(crate) by_population: BTreeMap<u16, BehaviorVm>,
    pub(crate) destination_indices: BTreeMap<String, u16>,
    states: BTreeMap<AgentId, BehaviorVmState>,
    traces: BTreeMap<AgentId, DecisionOutcome>,
}

impl RuntimeBehaviorController {
    pub(crate) fn new(
        by_population: BTreeMap<u16, BehaviorVm>,
        destination_indices: BTreeMap<String, u16>,
    ) -> Self {
        Self {
            by_population,
            destination_indices,
            states: BTreeMap::new(),
            traces: BTreeMap::new(),
        }
    }

    pub fn trace(&self, agent_id: AgentId) -> Option<&DecisionOutcome> {
        self.traces.get(&agent_id)
    }

    pub fn apply(&mut self, world: &mut World, neighbors: &NeighborArena, tick: u64) {
        for slot in 0..world.len() {
            let population = world.population_id[slot];
            let Some(vm) = self.by_population.get(&population) else {
                continue;
            };
            let agent_id = world.agent_id[slot];
            let neighbor_count = neighbors.neighbors(slot).len() as i32;
            let context = BehaviorContext {
                tick,
                agent_id,
                bool_observations: BTreeMap::from([
                    ("nearby_agents".to_string(), neighbor_count > 0),
                    ("density_high".to_string(), neighbor_count >= 8),
                ]),
                number_observations: BTreeMap::from([
                    ("nearby_agent_count".to_string(), neighbor_count),
                    (
                        "density_score".to_string(),
                        (neighbor_count * 62_500).min(1_000_000),
                    ),
                ]),
                events: BTreeSet::new(),
                completed_nodes: BTreeSet::new(),
            };
            let outcome = vm.decide(self.states.entry(agent_id).or_default(), &context);
            match &outcome.action {
                Some(
                    BehaviorAction::HoldPosition
                    | BehaviorAction::Wait { .. }
                    | BehaviorAction::Queue { .. },
                ) => {
                    world.des_vel_x[slot] = 0.0;
                    world.des_vel_y[slot] = 0.0;
                }
                Some(BehaviorAction::Navigate { destination_id })
                    if destination_id != "__assigned_destination" =>
                {
                    if let Some(destination) = self.destination_indices.get(destination_id).copied()
                    {
                        if world.destination[slot] != destination {
                            world.destination[slot] = destination;
                            world.route[slot] = NO_ROUTE;
                            world.arrived[slot] = false;
                            world.unrouted[slot] = false;
                        }
                    }
                }
                Some(BehaviorAction::FollowLane { .. } | BehaviorAction::Navigate { .. })
                | None => {}
            }
            self.traces.insert(agent_id, outcome);
        }
    }
}
