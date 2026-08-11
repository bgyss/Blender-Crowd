//! Deterministic M2 queue admission and lightweight group constraints.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::AgentId;
use crate::units::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStatus {
    Absent,
    Waiting { ordinal: u32 },
    Admitted { slot: u32 },
}

#[derive(Clone, Debug)]
pub struct QueueRuntime {
    id: String,
    slot_count: usize,
    admission_capacity: usize,
    admitted_this_tick: usize,
    admitted: Vec<AgentId>,
    waiting: BTreeSet<AgentId>,
    throughput: u64,
}

impl QueueRuntime {
    pub fn new(
        id: impl Into<String>,
        slot_count: usize,
        admission_capacity: usize,
    ) -> Result<Self, &'static str> {
        let id = id.into();
        if id.is_empty() || slot_count == 0 || admission_capacity == 0 {
            return Err("queue needs an ID, slots, and positive admission capacity");
        }
        Ok(Self {
            id,
            slot_count,
            admission_capacity,
            admitted_this_tick: 0,
            admitted: Vec::new(),
            waiting: BTreeSet::new(),
            throughput: 0,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn request_batch(&mut self, agents: &[AgentId]) {
        for agent in agents {
            if !self.admitted.contains(agent) {
                self.waiting.insert(*agent);
            }
        }
        self.fill_available_slots();
    }

    pub fn advance_tick(&mut self) {
        self.admitted_this_tick = 0;
        self.fill_available_slots();
    }

    pub fn release(&mut self, agent: AgentId) -> bool {
        let Some(index) = self.admitted.iter().position(|item| *item == agent) else {
            return false;
        };
        self.admitted.remove(index);
        self.throughput += 1;
        true
    }

    pub fn throughput(&self) -> u64 {
        self.throughput
    }

    pub fn status(&self, agent: AgentId) -> QueueStatus {
        if let Some(slot) = self.admitted.iter().position(|item| *item == agent) {
            return QueueStatus::Admitted { slot: slot as u32 };
        }
        if let Some(ordinal) = self.waiting.iter().position(|item| *item == agent) {
            return QueueStatus::Waiting {
                ordinal: ordinal as u32,
            };
        }
        QueueStatus::Absent
    }

    pub fn assignments(&self) -> BTreeMap<AgentId, QueueStatus> {
        self.admitted
            .iter()
            .chain(&self.waiting)
            .map(|agent| (*agent, self.status(*agent)))
            .collect()
    }

    fn fill_available_slots(&mut self) {
        while self.admitted.len() < self.slot_count
            && self.admitted_this_tick < self.admission_capacity
        {
            let Some(agent) = self.waiting.pop_first() else {
                break;
            };
            self.admitted.push(agent);
            self.admitted_this_tick += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupReport {
    pub split: bool,
    pub maximum_separation_m: f32,
    pub farthest_member: Option<AgentId>,
    pub missing_members: u32,
}

#[derive(Clone, Debug)]
pub struct GroupConstraint {
    id: String,
    members: Vec<AgentId>,
    leader: AgentId,
    max_separation_m: f32,
    max_correction_mps: f32,
}

impl GroupConstraint {
    pub fn new(
        id: impl Into<String>,
        mut members: Vec<AgentId>,
        leader: AgentId,
        max_separation_m: f32,
        max_correction_mps: f32,
    ) -> Result<Self, &'static str> {
        let id = id.into();
        members.sort_unstable();
        members.dedup();
        if id.is_empty()
            || members.len() < 2
            || !members.contains(&leader)
            || !max_separation_m.is_finite()
            || max_separation_m <= 0.0
            || !max_correction_mps.is_finite()
            || max_correction_mps <= 0.0
        {
            return Err("group needs unique members, a member leader, and positive limits");
        }
        Ok(Self {
            id,
            members,
            leader,
            max_separation_m,
            max_correction_mps,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn evaluate(&self, positions: &BTreeMap<AgentId, Vec2>) -> GroupReport {
        let Some(leader_position) = positions.get(&self.leader).copied() else {
            return GroupReport {
                split: true,
                maximum_separation_m: 0.0,
                farthest_member: None,
                missing_members: self.members.len() as u32,
            };
        };
        let mut maximum = 0.0f32;
        let mut farthest = None;
        let mut missing = 0;
        for member in &self.members {
            let Some(position) = positions.get(member) else {
                missing += 1;
                continue;
            };
            let separation = (*position - leader_position).length();
            if separation > maximum {
                maximum = separation;
                farthest = Some(*member);
            }
        }
        GroupReport {
            split: missing > 0 || maximum > self.max_separation_m,
            maximum_separation_m: maximum,
            farthest_member: farthest,
            missing_members: missing,
        }
    }

    pub fn cohesion_velocity(&self, member: AgentId, positions: &BTreeMap<AgentId, Vec2>) -> Vec2 {
        if member == self.leader || !self.members.contains(&member) {
            return Vec2::ZERO;
        }
        let (Some(leader), Some(position)) = (positions.get(&self.leader), positions.get(&member))
        else {
            return Vec2::ZERO;
        };
        let offset = *leader - *position;
        let distance = offset.length();
        if distance <= self.max_separation_m {
            Vec2::ZERO
        } else {
            offset.normalize_or_zero()
                * (distance - self.max_separation_m).min(self.max_correction_mps)
        }
    }
}
