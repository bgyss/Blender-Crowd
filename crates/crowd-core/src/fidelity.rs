//! Deterministic simulation and presentation fidelity scheduling.
//!
//! This is deliberately a policy layer: it may change how frequently an
//! agent is evaluated or presented, but never its identity or authoritative
//! root-motion storage.  That separation is what lets M5 scale work preserve
//! the Cache v1 and M4 layer contracts.

use serde::{Deserialize, Serialize};

use crate::ids::{mix64, AgentId};
use crate::units::Vec2;

/// Background S2 agents are refreshed every other tick. At 30 Hz this bounds
/// stale avoidance state to 66.7 ms while retaining a large reduction from
/// S1's every-tick work. The per-agent phase below distributes it evenly.
pub const S2_UPDATE_INTERVAL_TICKS: u64 = 2;

/// Distant S3 agents re-select their clip every eighth tick — 266.7 ms at
/// 30 Hz. They are drawn as proxies or aggregates at that range, so a gait
/// change is not readable; their clip phase still advances every tick from
/// root displacement, so they do not slide.
pub const S3_ANIMATION_INTERVAL_TICKS: u64 = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum SimulationTier {
    /// Hero: full graph, navigation, and avoidance every tick.
    #[default]
    S0 = 0,
    /// Midground: reduced decision/perception frequency.
    S1 = 1,
    /// Background: shared-path / coarse behavior representation.
    S2 = 2,
    /// Distant: cache or flow-only representation.
    S3 = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum RenderTier {
    /// Full character.
    #[default]
    R0 = 0,
    /// Reduced character.
    R1 = 1,
    /// Instanced mesh / baked deformation.
    R2 = 2,
    /// Card or impostor.
    R3 = 3,
    /// Aggregate-only representation; no individual draw requirement.
    R4 = 4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FidelityPolicy {
    pub camera: Vec2,
    /// Agents inside this radius promote immediately; demotion happens only
    /// after crossing `near_exit`, which prevents camera-edge flicker.
    pub near_enter: f32,
    pub near_exit: f32,
    pub mid_enter: f32,
    pub mid_exit: f32,
    pub far_enter: f32,
    pub far_exit: f32,
    /// When set, assign this share of stable IDs to background S2/R2. This
    /// makes a scale benchmark's declared mix explicit and repeatable instead
    /// of depending on an arbitrary benchmark camera position.
    pub background_permyriad: Option<u16>,
}

impl Default for FidelityPolicy {
    fn default() -> Self {
        Self {
            camera: Vec2::ZERO,
            near_enter: 12.0,
            near_exit: 15.0,
            mid_enter: 35.0,
            mid_exit: 42.0,
            far_enter: 80.0,
            far_exit: 96.0,
            background_permyriad: None,
        }
    }
}

impl FidelityPolicy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self
            .background_permyriad
            .is_some_and(|share| share > 10_000)
        {
            return Err("background_permyriad must not exceed 10000");
        }
        if !(self.near_enter <= self.near_exit
            && self.near_exit <= self.mid_enter
            && self.mid_enter <= self.mid_exit
            && self.mid_exit <= self.far_enter
            && self.far_enter <= self.far_exit)
        {
            return Err("fidelity radii must be ordered enter <= exit <= next enter");
        }
        Ok(())
    }

    pub fn m5_10k_profile() -> Self {
        Self {
            background_permyriad: Some(9_000),
            ..Self::default()
        }
    }

    /// Stable background assignment for a declared percentage profile.
    ///
    /// This must use the agent ID, not an emission ordinal local to one spawn
    /// region. A lane-based fixture has many small regions; applying a 90%
    /// cutoff to each region-local ordinal classified every agent as S2 while
    /// the report still claimed a 90/10 split.
    pub fn is_background_id(&self, id: AgentId) -> Option<bool> {
        self.background_permyriad
            .map(|background| mix64(id.0) % 10_000 < u64::from(background))
    }

    /// Spread background work over the S2 cadence by stable ID.
    ///
    /// A global phase makes every S2 agent perceive and steer in lockstep.
    /// Dense lanes amplify that into repeated collective braking and turns.
    /// A fixed per-agent phase preserves the same per-agent work budget while
    /// avoiding a synchronized response.
    pub fn s2_update_due(id: AgentId, tick: u64) -> bool {
        let phase = mix64(id.0 ^ 0x9e37_79b9_7f4a_7c15) % S2_UPDATE_INTERVAL_TICKS;
        tick % S2_UPDATE_INTERVAL_TICKS == phase
    }

    /// Whether this agent's presentation state is re-evaluated on this tick.
    ///
    /// Camera-focused agents (S0/S1) are re-evaluated every tick because a
    /// viewer reads their gait directly. Background agents are re-evaluated on
    /// a stable-ID-staggered cadence, which is what makes animation evaluation
    /// cost scale with focus rather than with population.
    ///
    /// This schedules only the *classification* — state, clip choice, playback
    /// rate. Clip phase keeps advancing from actual root displacement every
    /// tick for every agent, so a background agent's feet still track the
    /// ground it covers between evaluations. The M5 contract permits animation
    /// scheduling to change evaluation cost, never cached root trajectories,
    /// and root motion is owned by `integrate` and never written here.
    pub fn animation_due(tier: SimulationTier, id: AgentId, tick: u64) -> bool {
        let interval = match tier {
            SimulationTier::S0 | SimulationTier::S1 => 1,
            SimulationTier::S2 => S2_UPDATE_INTERVAL_TICKS,
            SimulationTier::S3 => S3_ANIMATION_INTERVAL_TICKS,
        };
        if interval <= 1 {
            return true;
        }
        // A separate mixing constant from `s2_update_due`, so an agent's
        // animation refresh does not land on the same tick as its steering
        // refresh and concentrate both costs on one frame.
        let phase = mix64(id.0 ^ 0x5851_f42d_4c95_7f2d) % interval;
        tick % interval == phase
    }

    pub fn target(&self, position: Vec2, current: SimulationTier) -> SimulationTier {
        let distance = position.distance_squared(self.camera).sqrt();
        match current {
            SimulationTier::S0 if distance <= self.near_exit => SimulationTier::S0,
            SimulationTier::S1 if distance <= self.mid_exit && distance >= self.near_enter => {
                SimulationTier::S1
            }
            SimulationTier::S2 if distance <= self.far_exit && distance >= self.mid_enter => {
                SimulationTier::S2
            }
            SimulationTier::S3 if distance >= self.far_enter => SimulationTier::S3,
            _ if distance <= self.near_enter => SimulationTier::S0,
            _ if distance <= self.mid_enter => SimulationTier::S1,
            _ if distance <= self.far_enter => SimulationTier::S2,
            _ => SimulationTier::S3,
        }
    }
}

/// A deterministic pin: artist intent wins over camera policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FidelityPin {
    pub agent_id: AgentId,
    pub simulation: SimulationTier,
    pub render: RenderTier,
}

pub fn render_for(simulation: SimulationTier) -> RenderTier {
    match simulation {
        SimulationTier::S0 => RenderTier::R0,
        SimulationTier::S1 => RenderTier::R1,
        SimulationTier::S2 => RenderTier::R2,
        SimulationTier::S3 => RenderTier::R3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteresis_prevents_camera_edge_flapping() {
        let policy = FidelityPolicy::default();
        assert_eq!(
            policy.target(Vec2::new(14.0, 0.0), SimulationTier::S0),
            SimulationTier::S0
        );
        assert_eq!(
            policy.target(Vec2::new(14.0, 0.0), SimulationTier::S1),
            SimulationTier::S1
        );
        assert_eq!(
            policy.target(Vec2::new(16.0, 0.0), SimulationTier::S0),
            SimulationTier::S1
        );
    }

    #[test]
    fn policy_rejects_overlapping_hysteresis_bands() {
        let policy = FidelityPolicy {
            near_exit: 40.0,
            ..FidelityPolicy::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn profile_assignment_is_derived_from_stable_id_not_spawn_order() {
        let policy = FidelityPolicy::m5_10k_profile();
        let id = AgentId(0x1234_5678_9abc_def0);
        assert_eq!(policy.is_background_id(id), policy.is_background_id(id));
        assert!(policy.is_background_id(id).is_some());
    }

    #[test]
    fn s2_update_cadence_is_stable_and_runs_once_per_two_ticks() {
        let id = AgentId(0x0123_4567_89ab_cdef);
        let due: Vec<_> = (0..8)
            .filter(|tick| FidelityPolicy::s2_update_due(id, *tick))
            .collect();
        assert_eq!(due.len(), 4);
        assert_eq!(due[1] - due[0], 2);
        assert_eq!(due[2] - due[1], 2);
        assert_eq!(due[3] - due[2], 2);
    }
}
