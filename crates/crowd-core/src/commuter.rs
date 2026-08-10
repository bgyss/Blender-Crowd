//! Fixed M1 commuter state and cache-facing snapshots.

use serde::{Deserialize, Serialize};

use crate::ids::AgentId;
use crate::units::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u16)]
pub enum CommuterState {
    #[default]
    Unspawned = 0,
    Travel = 1,
    Arrived = 2,
    Blocked = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u16)]
pub enum DecisionReason {
    #[default]
    None = 0,
    InitialDestination = 1,
    FollowCorridor = 2,
    PortalClosedReplan = 3,
    PortalReopened = 4,
    DestinationReached = 5,
    NoRoute = 6,
}

impl DecisionReason {
    pub const fn text(self) -> &'static str {
        match self {
            Self::None => "no decision recorded",
            Self::InitialDestination => "initial destination assigned",
            Self::FollowCorridor => "following planned corridor",
            Self::PortalClosedReplan => "named portal closed; corridor invalidated",
            Self::PortalReopened => "named portal reopened",
            Self::DestinationReached => "destination reached",
            Self::NoRoute => "no route to destination",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipState {
    pub clip_id: u16,
    pub phase: f32,
    pub playback_rate: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub agent_id: AgentId,
    pub population_id: u32,
    pub archetype_id: u32,
    pub variant_id: u32,
    pub spawn_ordinal: u32,
    pub position: Vec2,
    pub orientation: f32,
    pub scale: f32,
    pub velocity: Vec2,
    pub desired_velocity: Vec2,
    pub destination_id: u32,
    pub commuter_state: CommuterState,
    pub decision_reason: DecisionReason,
    pub clip_state: ClipState,
    pub visible: bool,
    pub render_tier: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameSnapshot {
    pub tick: u64,
    pub agents: Vec<AgentSnapshot>,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeAgentSpec {
    pub agent_id: AgentId,
    pub population_id: u32,
    pub spawn_ordinal: u32,
    pub destination_id: u32,
    pub archetype_id: u32,
    pub variant_id: u32,
    pub radius_m: f32,
    pub preferred_speed_mps: f32,
    pub scale: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedPortalInput {
    pub tick: u64,
    pub portal_id: String,
    pub portal_index: u32,
    pub authored_ordinal: u32,
    pub open: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeAnimationSettings {
    pub jog_threshold_mps: f32,
}

impl Default for RuntimeAnimationSettings {
    fn default() -> Self {
        Self {
            jog_threshold_mps: 1.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortalControlError {
    MissingNavigation,
    UnknownPortal(String),
}
