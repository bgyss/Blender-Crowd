//! Deterministic, typed perception snapshots for M6 brains.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::arena::NeighborArena;
use crate::geometry::Segment;
use crate::ids::AgentId;
use crate::units::Vec2;
use crate::world::World;

pub const PERCEPTION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationChannelV1 {
    VisionAgent,
    Hearing,
    Touch,
    Density,
    FlowSpeed,
    SemanticDistance,
    GroupExtent,
    NearestFriend,
    NearestThreat,
    AttentionTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PerceptionValueV1 {
    Bool(bool),
    NumberI32(i32),
    Agent(AgentId),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationV1 {
    pub channel: ObservationChannelV1,
    pub value: PerceptionValueV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntryV1 {
    pub key: String,
    pub value: PerceptionValueV1,
    pub expires_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptionSnapshotV1 {
    pub schema_version: u32,
    pub agent_id: AgentId,
    pub tick: u64,
    pub observations: Vec<ObservationV1>,
    pub memory: Vec<MemoryEntryV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_evidence: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PerceptionConfigV1 {
    pub vision_range_m: f32,
    pub vision_half_angle_rad: f32,
    pub hearing_range_m: f32,
    pub touch_range_m: f32,
    pub memory_capacity: usize,
    pub observation_budget: usize,
}

impl Default for PerceptionConfigV1 {
    fn default() -> Self {
        Self {
            vision_range_m: 8.0,
            vision_half_angle_rad: std::f32::consts::FRAC_PI_2,
            hearing_range_m: 10.0,
            touch_range_m: 0.2,
            memory_capacity: 8,
            observation_budget: 64,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PerceptionEngine {
    config: PerceptionConfigV1,
    occluders: Vec<Segment>,
    groups: BTreeMap<String, Vec<AgentId>>,
    friendship: BTreeSet<(AgentId, AgentId)>,
    threats: BTreeSet<(AgentId, AgentId)>,
    touch_events: BTreeSet<(AgentId, AgentId)>,
    hearing_events: BTreeMap<AgentId, BTreeSet<String>>,
    semantic_distances_millionths: BTreeMap<(AgentId, String), i32>,
    attention_targets: BTreeMap<AgentId, AgentId>,
    memory: BTreeMap<AgentId, BTreeMap<String, MemoryEntryV1>>,
}

impl PerceptionEngine {
    pub fn new(config: PerceptionConfigV1) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn set_occluders(&mut self, occluders: Vec<Segment>) {
        self.occluders = occluders;
    }

    pub fn set_group_members(&mut self, group_id: impl Into<String>, members: Vec<AgentId>) {
        let mut members = members;
        members.sort_unstable();
        members.dedup();
        self.groups.insert(group_id.into(), members);
    }

    pub fn set_friendship(&mut self, first: AgentId, second: AgentId, is_friend: bool) {
        let pair = ordered_pair(first, second);
        if is_friend {
            self.friendship.insert(pair);
        } else {
            self.friendship.remove(&pair);
        }
    }

    pub fn set_threat(&mut self, observer: AgentId, threat: AgentId, is_threat: bool) {
        if is_threat {
            self.threats.insert((observer, threat));
        } else {
            self.threats.remove(&(observer, threat));
        }
    }

    pub fn set_touch_event(&mut self, first: AgentId, second: AgentId) {
        self.touch_events.insert(ordered_pair(first, second));
    }

    pub fn set_hearing_event(&mut self, agent_id: AgentId, event: impl Into<String>) {
        self.hearing_events
            .entry(agent_id)
            .or_default()
            .insert(event.into());
    }

    pub fn set_semantic_distance_millionths(
        &mut self,
        agent_id: AgentId,
        semantic_id: impl Into<String>,
        distance_millionths: i32,
    ) {
        self.semantic_distances_millionths
            .insert((agent_id, semantic_id.into()), distance_millionths.max(0));
    }

    pub fn set_attention_target(&mut self, agent_id: AgentId, target: AgentId) {
        self.attention_targets.insert(agent_id, target);
    }

    pub fn remember(
        &mut self,
        agent_id: AgentId,
        key: impl Into<String>,
        value: PerceptionValueV1,
        expires_tick: u64,
    ) {
        if self.config.memory_capacity == 0 {
            return;
        }
        let entries = self.memory.entry(agent_id).or_default();
        let key = key.into();
        entries.insert(
            key.clone(),
            MemoryEntryV1 {
                key,
                value,
                expires_tick,
            },
        );
        while entries.len() > self.config.memory_capacity {
            let remove_key = entries
                .values()
                .max_by(|left, right| {
                    (left.expires_tick, left.key.as_str())
                        .cmp(&(right.expires_tick, right.key.as_str()))
                })
                .map(|entry| entry.key.clone());
            if let Some(remove_key) = remove_key {
                entries.remove(&remove_key);
            } else {
                break;
            }
        }
    }

    pub fn observe(
        &mut self,
        world: &World,
        neighbors: &NeighborArena,
        tick: u64,
    ) -> BTreeMap<AgentId, PerceptionSnapshotV1> {
        let mut slots: Vec<usize> = (0..world.len()).collect();
        slots.sort_unstable_by_key(|slot| world.agent_id[*slot]);
        let mut snapshots = BTreeMap::new();
        for slot in slots {
            let agent_id = world.agent_id[slot];
            let mut observations = Vec::new();
            let mut ordered_neighbors = neighbors.neighbors(slot).to_vec();
            ordered_neighbors
                .sort_unstable_by_key(|neighbor| world.agent_id[neighbor.slot as usize]);

            let mut flow_sum = 0.0;
            let mut friend_distances = Vec::new();
            let mut threat_distances = Vec::new();
            for neighbor in ordered_neighbors {
                let other_slot = neighbor.slot as usize;
                if other_slot >= world.len() || other_slot == slot {
                    continue;
                }
                let other_id = world.agent_id[other_slot];
                let position = world.position(slot as u32);
                let other_position = world.position(other_slot as u32);
                let distance_m = neighbor.dist_sq.max(0.0).sqrt();
                flow_sum += world.velocity(other_slot as u32).length();
                let relative = other_position - position;
                let visible = distance_m <= self.config.vision_range_m
                    && relative.length_squared() > f32::MIN_POSITIVE
                    && world::facing_dot(world.yaw[slot], relative)
                        >= self.config.vision_half_angle_rad.cos()
                    && !self.is_occluded(position, other_position);
                if visible {
                    observations.push(ObservationV1 {
                        channel: ObservationChannelV1::VisionAgent,
                        value: PerceptionValueV1::Agent(other_id),
                        source_agent_id: Some(other_id),
                        key: None,
                    });
                }
                if distance_m <= self.config.hearing_range_m {
                    // Hearing is represented by explicit events below; this
                    // branch reserves the deterministic spatial budget without
                    // inventing an event from mere proximity.
                }
                if self
                    .touch_events
                    .contains(&ordered_pair(agent_id, other_id))
                    || distance_m
                        <= world.radius[slot] + world.radius[other_slot] + self.config.touch_range_m
                {
                    observations.push(ObservationV1 {
                        channel: ObservationChannelV1::Touch,
                        value: PerceptionValueV1::Agent(other_id),
                        source_agent_id: Some(other_id),
                        key: None,
                    });
                }
                if self.friendship.contains(&ordered_pair(agent_id, other_id)) {
                    friend_distances.push((distance_m, other_id));
                }
                if self.threats.contains(&(agent_id, other_id)) {
                    threat_distances.push((distance_m, other_id));
                }
            }
            friend_distances.sort_by(|left, right| {
                left.partial_cmp(right)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.cmp(&right.1))
            });
            threat_distances.sort_by(|left, right| {
                left.partial_cmp(right)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.1.cmp(&right.1))
            });
            let nearest_friend = friend_distances.first().map(|(_, id)| *id);
            let nearest_threat = threat_distances.first().map(|(_, id)| *id);
            if let Some(target) = nearest_friend {
                observations.push(ObservationV1 {
                    channel: ObservationChannelV1::NearestFriend,
                    value: PerceptionValueV1::Agent(target),
                    source_agent_id: Some(target),
                    key: None,
                });
            }
            if let Some(target) = nearest_threat {
                observations.push(ObservationV1 {
                    channel: ObservationChannelV1::NearestThreat,
                    value: PerceptionValueV1::Agent(target),
                    source_agent_id: Some(target),
                    key: None,
                });
            }
            observations.push(ObservationV1 {
                channel: ObservationChannelV1::Density,
                value: PerceptionValueV1::NumberI32(neighbors.neighbors(slot).len() as i32),
                source_agent_id: None,
                key: None,
            });
            let neighbor_count = neighbors.neighbors(slot).len() as f32;
            observations.push(ObservationV1 {
                channel: ObservationChannelV1::FlowSpeed,
                value: PerceptionValueV1::NumberI32(if neighbor_count > 0.0 {
                    ((flow_sum / neighbor_count) * 1_000_000.0).round() as i32
                } else {
                    0
                }),
                source_agent_id: None,
                key: None,
            });
            if let Some(group) = self.group_for(agent_id) {
                let position = world.position(slot as u32);
                let extent = group
                    .iter()
                    .filter_map(|member| world.slot_of(*member))
                    .map(|member_slot| {
                        position
                            .distance_squared(world.position(member_slot))
                            .sqrt()
                    })
                    .fold(0.0, f32::max);
                observations.push(ObservationV1 {
                    channel: ObservationChannelV1::GroupExtent,
                    value: PerceptionValueV1::NumberI32((extent * 1_000_000.0).round() as i32),
                    source_agent_id: None,
                    key: None,
                });
            }
            for ((observing_agent, semantic_id), distance) in &self.semantic_distances_millionths {
                if *observing_agent == agent_id {
                    observations.push(ObservationV1 {
                        channel: ObservationChannelV1::SemanticDistance,
                        value: PerceptionValueV1::NumberI32(*distance),
                        source_agent_id: None,
                        key: Some(semantic_id.clone()),
                    });
                    let _ = semantic_id;
                }
            }
            if let Some(target) = self.attention_targets.get(&agent_id).copied() {
                observations.push(ObservationV1 {
                    channel: ObservationChannelV1::AttentionTarget,
                    value: PerceptionValueV1::Agent(target),
                    source_agent_id: Some(target),
                    key: None,
                });
            }
            if let Some(events) = self.hearing_events.get(&agent_id) {
                observations.extend(events.iter().map(|event| ObservationV1 {
                    channel: ObservationChannelV1::Hearing,
                    value: PerceptionValueV1::Text(event.clone()),
                    source_agent_id: None,
                    key: None,
                }));
            }
            observations.sort_by(|left, right| {
                (
                    left.channel,
                    left.source_agent_id,
                    left.key.as_deref(),
                    &left.value,
                )
                    .cmp(&(
                        right.channel,
                        right.source_agent_id,
                        right.key.as_deref(),
                        &right.value,
                    ))
            });
            let degraded_evidence = if observations.len() > self.config.observation_budget {
                observations.truncate(self.config.observation_budget);
                Some("observation budget reduced M6 evidence for this tier".to_owned())
            } else {
                None
            };
            self.memory
                .entry(agent_id)
                .or_default()
                .retain(|_, entry| entry.expires_tick >= tick);
            let memory = self
                .memory
                .get(&agent_id)
                .map(|entries| entries.values().cloned().collect())
                .unwrap_or_default();
            snapshots.insert(
                agent_id,
                PerceptionSnapshotV1 {
                    schema_version: PERCEPTION_SCHEMA_VERSION,
                    agent_id,
                    tick,
                    observations,
                    memory,
                    degraded_evidence,
                },
            );
        }
        snapshots
    }

    fn group_for(&self, agent_id: AgentId) -> Option<&[AgentId]> {
        self.groups
            .values()
            .find(|members| members.contains(&agent_id))
            .map(Vec::as_slice)
    }

    fn is_occluded(&self, start: Vec2, end: Vec2) -> bool {
        let sight = Segment::new(start, end);
        self.occluders
            .iter()
            .any(|occluder| segments_intersect(sight, *occluder))
    }
}

fn ordered_pair(first: AgentId, second: AgentId) -> (AgentId, AgentId) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn segments_intersect(first: Segment, second: Segment) -> bool {
    fn cross(a: Vec2, b: Vec2, c: Vec2) -> f32 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }
    fn on_segment(a: Vec2, b: Vec2, point: Vec2) -> bool {
        point.x >= a.x.min(b.x)
            && point.x <= a.x.max(b.x)
            && point.y >= a.y.min(b.y)
            && point.y <= a.y.max(b.y)
    }
    let ab_c = cross(first.a, first.b, second.a);
    let ab_d = cross(first.a, first.b, second.b);
    let cd_a = cross(second.a, second.b, first.a);
    let cd_b = cross(second.a, second.b, first.b);
    let epsilon = 1e-6;
    (ab_c.abs() <= epsilon && on_segment(first.a, first.b, second.a))
        || (ab_d.abs() <= epsilon && on_segment(first.a, first.b, second.b))
        || (cd_a.abs() <= epsilon && on_segment(second.a, second.b, first.a))
        || (cd_b.abs() <= epsilon && on_segment(second.a, second.b, first.b))
        || ((ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0))
}

mod world {
    use crate::units::Vec2;

    pub fn facing_dot(yaw: f32, relative: Vec2) -> f32 {
        Vec2::from_yaw(yaw).dot(relative.normalize_or_zero())
    }
}
