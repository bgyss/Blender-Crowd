//! Tick phase: authoritative M1 locomotion clip and phase state.

use crate::commuter::{CommuterState, DecisionReason};
use crate::fidelity::{FidelityPolicy, SimulationTier};
use crate::ids::AgentId;
use crate::motion::{MotionMatcher, MotionQueryV1};
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

/// How much presentation work a tick actually did, per simulation tier.
///
/// This is the measurable form of the M5 claim that camera/focus animation
/// scheduling changes *evaluation cost*: `evaluated_by_tier` divided by the
/// tier's agent-ticks is the share of full classifications it paid for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimateReport {
    pub evaluated_by_tier: [u64; 4],
    pub agents_by_tier: [u64; 4],
}

/// Update only commuter/animation columns from the staged integrated state.
///
/// Position and orientation remain owned by `integrate`; this phase reads the
/// staged values to select a clip and advances phase by traveled distance.
///
/// Every agent is re-classified every tick. Use `animate_scheduled` when a
/// fidelity policy is active.
pub fn animate(world: &mut World, config: &AnimateConfig) -> AnimateReport {
    animate_inner(world, config, |_| true)
}

/// As `animate`, but re-classify each agent only on its scheduled tick.
///
/// Clip phase still advances every tick for every agent from the distance the
/// agent actually covered, so a background agent whose clip choice is stale
/// does not slide: only the choice is stale, never the motion.
pub fn animate_scheduled(world: &mut World, config: &AnimateConfig, tick: u64) -> AnimateReport {
    animate_inner(world, config, |(tier, id)| {
        FidelityPolicy::animation_due(tier, id, tick)
    })
}

/// Run the deterministic clip-state baseline, then optionally promote the
/// selected clip through the versioned trajectory matcher. The matcher can
/// only choose among declared clips; a missing or infeasible result leaves the
/// baseline clip in place.
pub fn animate_with_motion_matcher(
    world: &mut World,
    config: &AnimateConfig,
    matcher: &MotionMatcher,
) -> AnimateReport {
    let report = animate(world, config);
    apply_motion_matches(world, matcher);
    report
}

/// Apply only promoted S0 matches to the already-selected clip state. Lower
/// tiers retain the deterministic clip baseline and therefore do not pay the
/// matcher cost or inherit its diagnostics.
pub fn apply_motion_matches(world: &mut World, matcher: &MotionMatcher) {
    for slot in 0..world.len() {
        if world.simulation_tier[slot] != SimulationTier::S0 {
            continue;
        }
        let velocity = Vec2::new(world.next_vel_x[slot], world.next_vel_y[slot]);
        let query = MotionQueryV1 {
            desired_velocity_millimeters_per_second: [
                (velocity.x * 1_000.0).round() as i32,
                (velocity.y * 1_000.0).round() as i32,
            ],
            desired_slope_millionths: 0,
            required_contact: None,
            fallback_clip_id: if world.clip_id[slot] == JOG_CLIP_ID {
                "jog".to_owned()
            } else {
                "walk".to_owned()
            },
            future_positions_millimeters: Vec::new(),
            future_velocities_millimeters_per_second: Vec::new(),
        };
        let Ok(result) = matcher.select(&query) else {
            continue;
        };
        if let Some(clip_id) = runtime_clip_id(&result.clip_id) {
            world.clip_id[slot] = clip_id;
        }
    }
}

fn animate_inner(
    world: &mut World,
    config: &AnimateConfig,
    due: impl Fn((SimulationTier, AgentId)) -> bool,
) -> AnimateReport {
    let mut report = AnimateReport::default();
    for slot in 0..world.len() {
        let tier = world.simulation_tier[slot];
        report.agents_by_tier[tier as usize] += 1;
        if !due((tier, world.agent_id[slot])) {
            advance_clip_phase(world, config, slot);
            continue;
        }
        report.evaluated_by_tier[tier as usize] += 1;

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

        advance_clip_phase(world, config, slot);
        let nominal_speed = if clip_id == JOG_CLIP_ID {
            config.jog_threshold_mps
        } else {
            world.preferred_speed[slot].max(config.stationary_threshold_mps)
        };
        world.playback_rate[slot] = (speed / nominal_speed).clamp(0.5, 2.0);
    }
    report
}

/// Advance the clip cycle by the distance the agent actually covered.
///
/// Driven entirely by root displacement, so it runs every tick for every agent
/// regardless of scheduling. Skipping it for unevaluated background agents
/// would make their feet slide against the ground they are covering, which is
/// the popping artifact the M5 gate forbids.
fn advance_clip_phase(world: &mut World, config: &AnimateConfig, slot: usize) {
    if world.clip_id[slot] == IDLE_CLIP_ID {
        return;
    }
    let before = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
    let after = Vec2::new(world.next_pos_x[slot], world.next_pos_y[slot]);
    let distance = before.distance_squared(after).sqrt();
    let stride = if world.clip_id[slot] == JOG_CLIP_ID {
        config.jog_stride_m
    } else {
        config.walk_stride_m
    };
    world.clip_phase[slot] = (world.clip_phase[slot] + distance / stride).rem_euclid(1.0);
}

fn runtime_clip_id(clip_id: &str) -> Option<u16> {
    let normalized = clip_id.to_ascii_lowercase();
    if normalized.contains("jog") {
        Some(JOG_CLIP_ID)
    } else if normalized.contains("walk") {
        Some(WALK_CLIP_ID)
    } else if normalized.contains("idle") {
        Some(IDLE_CLIP_ID)
    } else {
        None
    }
}
