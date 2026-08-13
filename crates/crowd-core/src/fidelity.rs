//! Deterministic simulation and presentation fidelity scheduling.
//!
//! This is deliberately a policy layer: it may change how frequently an
//! agent is evaluated or presented, but never its identity or authoritative
//! root-motion storage.  That separation is what lets M5 scale work preserve
//! the Cache v1 and M4 layer contracts.

use serde::{Deserialize, Serialize};

use crate::ids::AgentId;
use crate::units::Vec2;

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
}
