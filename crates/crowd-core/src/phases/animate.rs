//! Tick phase: authoritative M1 locomotion clip and phase state.

use crate::commuter::{CommuterState, DecisionReason};
use crate::units::Vec2;
use crate::world::World;

pub const IDLE_CLIP_ID: u16 = 0;
pub const WALK_CLIP_ID: u16 = 1;
pub const JOG_CLIP_ID: u16 = 2;

#[derive(Clone, Copy, Debug)]
pub struct AnimateConfig {
    pub jog_threshold_mps: f32,
    pub stationary_threshold_mps: f32,
    pub walk_stride_m: f32,
    pub jog_stride_m: f32,
}

impl Default for AnimateConfig {
    fn default() -> Self {
        Self {
            jog_threshold_mps: 1.8,
            stationary_threshold_mps: 0.05,
            walk_stride_m: 1.4,
            jog_stride_m: 1.9,
        }
    }
}

/// Update only commuter/animation columns from the staged integrated state.
///
/// Position and orientation remain owned by `integrate`; this phase reads the
/// staged values to select a clip and advances phase by traveled distance.
pub fn animate(world: &mut World, config: &AnimateConfig) {
    for slot in 0..world.len() {
        let state = if world.arrived[slot] {
            CommuterState::Arrived
        } else if world.unrouted[slot] {
            CommuterState::Blocked
        } else {
            CommuterState::Travel
        };
        world.commuter_state[slot] = state;

        match state {
            CommuterState::Arrived => {
                world.decision_reason[slot] = DecisionReason::DestinationReached;
            }
            CommuterState::Blocked => {
                if world.decision_reason[slot] != DecisionReason::PortalClosedReplan {
                    world.decision_reason[slot] = DecisionReason::NoRoute;
                }
            }
            CommuterState::Travel => {
                if matches!(
                    world.decision_reason[slot],
                    DecisionReason::None | DecisionReason::InitialDestination
                ) {
                    world.decision_reason[slot] = DecisionReason::FollowCorridor;
                }
            }
            CommuterState::Unspawned => {}
        }

        let velocity = Vec2::new(world.next_vel_x[slot], world.next_vel_y[slot]);
        let speed = velocity.length();
        let clip_id = if state != CommuterState::Travel || speed < config.stationary_threshold_mps {
            IDLE_CLIP_ID
        } else if speed >= config.jog_threshold_mps {
            JOG_CLIP_ID
        } else {
            WALK_CLIP_ID
        };
        world.clip_id[slot] = clip_id;

        if clip_id == IDLE_CLIP_ID {
            world.playback_rate[slot] = 1.0;
            continue;
        }

        let before = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        let after = Vec2::new(world.next_pos_x[slot], world.next_pos_y[slot]);
        let distance = before.distance_squared(after).sqrt();
        let stride = if clip_id == JOG_CLIP_ID {
            config.jog_stride_m
        } else {
            config.walk_stride_m
        };
        world.clip_phase[slot] = (world.clip_phase[slot] + distance / stride).rem_euclid(1.0);
        let nominal_speed = if clip_id == JOG_CLIP_ID {
            config.jog_threshold_mps
        } else {
            world.preferred_speed[slot].max(config.stationary_threshold_mps)
        };
        world.playback_rate[slot] = (speed / nominal_speed).clamp(0.5, 2.0);
    }
}
