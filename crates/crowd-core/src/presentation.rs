//! Deterministic terrain and foot-contact presentation derived from cache truth.
//!
//! This module never feeds a corrected position back into simulation.  Blender
//! may use the returned display pose for terrain projection and foot locking,
//! while cache XY remains the authoritative trajectory.

use crate::assets::ClipMetadataV1;
use crate::units::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainPlaneV1 {
    pub origin_height_m: f32,
    pub x_rise_per_meter: f32,
    pub y_rise_per_meter: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresentationPose {
    pub simulation_position: Vec2,
    pub display_position: [f32; 3],
    pub terrain_normal: [f32; 3],
    pub slope_degrees: f32,
    pub pitch_radians: f32,
    pub roll_radians: f32,
    pub left_foot_locked: bool,
    pub right_foot_locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainPresentationError {
    InvalidTerrain,
    InvalidSlopeLimit,
    SlopeLimitExceeded,
}

/// Produce a display-only terrain pose and contact locks for one cached frame.
pub fn project_presentation_pose(
    simulation_position: Vec2,
    terrain: &TerrainPlaneV1,
    clip: &ClipMetadataV1,
    clip_tick: u32,
    maximum_slope_degrees: f32,
) -> Result<PresentationPose, TerrainPresentationError> {
    if !terrain.origin_height_m.is_finite()
        || !terrain.x_rise_per_meter.is_finite()
        || !terrain.y_rise_per_meter.is_finite()
    {
        return Err(TerrainPresentationError::InvalidTerrain);
    }
    if !maximum_slope_degrees.is_finite() || !(0.0..=89.9).contains(&maximum_slope_degrees) {
        return Err(TerrainPresentationError::InvalidSlopeLimit);
    }
    let gradient_length = terrain.x_rise_per_meter.hypot(terrain.y_rise_per_meter);
    let slope_degrees = gradient_length.atan().to_degrees();
    if slope_degrees > maximum_slope_degrees {
        return Err(TerrainPresentationError::SlopeLimitExceeded);
    }
    let height = terrain.origin_height_m
        + terrain.x_rise_per_meter * simulation_position.x
        + terrain.y_rise_per_meter * simulation_position.y;
    let normal_length = (1.0 + gradient_length * gradient_length).sqrt();
    let normal = [
        -terrain.x_rise_per_meter / normal_length,
        -terrain.y_rise_per_meter / normal_length,
        1.0 / normal_length,
    ];
    let tick = if clip.duration_ticks == 0 {
        0
    } else {
        clip_tick % clip.duration_ticks
    };
    let in_contact = |intervals: &[crate::assets::ContactIntervalV1]| {
        intervals
            .iter()
            .any(|interval| tick >= interval.start && tick <= interval.end)
    };
    Ok(PresentationPose {
        simulation_position,
        display_position: [simulation_position.x, simulation_position.y, height],
        terrain_normal: normal,
        slope_degrees,
        // Rotations are display orientation offsets, not changes to cache yaw.
        pitch_radians: terrain.y_rise_per_meter.atan(),
        roll_radians: -terrain.x_rise_per_meter.atan(),
        left_foot_locked: in_contact(&clip.left_foot_contacts),
        right_foot_locked: in_contact(&clip.right_foot_contacts),
    })
}
