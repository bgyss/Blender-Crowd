//! Deterministic trajectory feature database and motion matching foundation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const TRAJECTORY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FootContactV1 {
    None,
    LeftFoot,
    RightFoot,
    BothFeet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainConstraintV1 {
    pub max_slope_millionths: i32,
    pub ground_height_millimeters: i32,
}

impl TerrainConstraintV1 {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.max_slope_millionths < 0 || self.max_slope_millionths > 1_000_000 {
            Err("terrain slope must be between 0 and 1000000 millionths")
        } else {
            Ok(())
        }
    }

    pub fn accepts_slope(&self, slope_millionths: i32) -> bool {
        slope_millionths.abs() <= self.max_slope_millionths
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FootLockWindowV1 {
    pub foot: FootContactV1,
    pub tick_start: u64,
    pub tick_end: u64,
    pub position_millimeters: [i32; 3],
}

impl FootLockWindowV1 {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.foot == FootContactV1::None {
            return Err("foot lock requires a named foot contact");
        }
        if self.tick_start > self.tick_end {
            return Err("foot lock tick_start must not be after tick_end");
        }
        Ok(())
    }

    pub fn contains_tick(&self, tick: u64) -> bool {
        (self.tick_start..=self.tick_end).contains(&tick)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionSampleV1 {
    pub tick: u64,
    pub position_millimeters: [i32; 2],
    pub velocity_millimeters_per_second: [i32; 2],
    pub contact: FootContactV1,
    pub slope_millionths: i32,
}

impl MotionSampleV1 {
    pub fn new(
        position_millimeters: [i32; 2],
        velocity_millimeters_per_second: [i32; 2],
        contact: FootContactV1,
        slope_millionths: i32,
    ) -> Self {
        Self {
            tick: 0,
            position_millimeters,
            velocity_millimeters_per_second,
            contact,
            slope_millionths,
        }
    }

    pub fn at(
        tick: u64,
        position_millimeters: [i32; 2],
        velocity_millimeters_per_second: [i32; 2],
        contact: FootContactV1,
        slope_millionths: i32,
    ) -> Self {
        Self {
            tick,
            position_millimeters,
            velocity_millimeters_per_second,
            contact,
            slope_millionths,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionClipV1 {
    pub id: String,
    pub provenance: String,
    pub samples: Vec<MotionSampleV1>,
}

impl MotionClipV1 {
    pub fn new(
        id: impl Into<String>,
        provenance: impl Into<String>,
        samples: Vec<MotionSampleV1>,
    ) -> Self {
        Self {
            id: id.into(),
            provenance: provenance.into(),
            samples,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionDatabaseV1 {
    pub schema_version: u32,
    pub database_id: String,
    pub clips: Vec<MotionClipV1>,
}

impl MotionDatabaseV1 {
    pub fn new(
        database_id: impl Into<String>,
        clips: Vec<MotionClipV1>,
    ) -> Result<Self, MotionError> {
        let database = Self {
            schema_version: TRAJECTORY_SCHEMA_VERSION,
            database_id: database_id.into(),
            clips,
        };
        database.validate()?;
        Ok(database)
    }

    pub fn validate(&self) -> Result<(), MotionError> {
        if self.schema_version != TRAJECTORY_SCHEMA_VERSION {
            return Err(MotionError::UnsupportedVersion(self.schema_version));
        }
        if self.database_id.is_empty() || self.clips.is_empty() {
            return Err(MotionError::InvalidDatabase(
                "database ID and clips are required",
            ));
        }
        let mut ids = BTreeSet::new();
        for clip in &self.clips {
            if clip.id.is_empty() || clip.provenance.is_empty() || clip.samples.is_empty() {
                return Err(MotionError::InvalidClip(clip.id.clone()));
            }
            if !ids.insert(clip.id.as_str()) {
                return Err(MotionError::DuplicateClip(clip.id.clone()));
            }
            if clip
                .samples
                .windows(2)
                .any(|pair| pair[0].tick >= pair[1].tick)
            {
                return Err(MotionError::InvalidClip(clip.id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MotionQueryV1 {
    pub desired_velocity_millimeters_per_second: [i32; 2],
    pub desired_slope_millionths: i32,
    pub required_contact: Option<FootContactV1>,
    pub fallback_clip_id: String,
    pub future_positions_millimeters: Vec<[i32; 2]>,
    pub future_velocities_millimeters_per_second: Vec<[i32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MotionMatchResultV1 {
    pub clip_id: String,
    pub score_millionths: u64,
    pub used_fallback: bool,
    pub diagnostic: String,
}

#[derive(Clone, Debug)]
pub struct MotionMatcher {
    database: MotionDatabaseV1,
}

impl MotionMatcher {
    pub fn new(database: MotionDatabaseV1) -> Self {
        Self { database }
    }

    pub fn select(&self, query: &MotionQueryV1) -> Result<MotionMatchResultV1, MotionError> {
        let mut candidates = self
            .database
            .clips
            .iter()
            .filter_map(|clip| {
                let sample = clip.samples.first()?;
                if query.required_contact.is_some_and(|contact| {
                    !clip.samples.iter().any(|sample| sample.contact == contact)
                }) {
                    return None;
                }
                let velocity_error = squared_distance(
                    sample.velocity_millimeters_per_second,
                    query.desired_velocity_millimeters_per_second,
                );
                let slope_error = i64::from(
                    (sample.slope_millionths - query.desired_slope_millionths).unsigned_abs(),
                );
                let future_velocity_error = query
                    .future_velocities_millimeters_per_second
                    .iter()
                    .enumerate()
                    .map(|(index, desired)| {
                        clip.samples
                            .get(index + 1)
                            .or_else(|| clip.samples.last())
                            .map_or(0, |actual| {
                                squared_distance(actual.velocity_millimeters_per_second, *desired)
                            })
                    })
                    .sum::<i64>();
                let future_position_error = query
                    .future_positions_millimeters
                    .iter()
                    .enumerate()
                    .map(|(index, desired)| {
                        clip.samples
                            .get(index + 1)
                            .or_else(|| clip.samples.last())
                            .map_or(0, |actual| {
                                squared_distance(actual.position_millimeters, *desired)
                            })
                    })
                    .sum::<i64>();
                Some((
                    velocity_error
                        .saturating_add(slope_error.saturating_mul(slope_error))
                        .saturating_add(future_velocity_error)
                        .saturating_add(future_position_error),
                    clip,
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.id.cmp(&right.1.id))
        });
        if let Some((score, clip)) = candidates.first() {
            return Ok(MotionMatchResultV1 {
                clip_id: clip.id.clone(),
                score_millionths: *score as u64,
                used_fallback: false,
                diagnostic: "feasible trajectory match".to_owned(),
            });
        }
        if self
            .database
            .clips
            .iter()
            .any(|clip| clip.id == query.fallback_clip_id)
        {
            return Ok(MotionMatchResultV1 {
                clip_id: query.fallback_clip_id.clone(),
                score_millionths: 0,
                used_fallback: true,
                diagnostic: "no feasible motion candidate; deterministic fallback selected"
                    .to_owned(),
            });
        }
        Err(MotionError::MissingFallback(query.fallback_clip_id.clone()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionCorrectionV1 {
    None,
    WarpStride,
    AdjustTurn,
    FallbackClip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionWarpV1 {
    pub stride_scale_millionths: u32,
    pub turn_delta_millionths: i32,
    pub feasible: bool,
}

pub fn compute_motion_warp(
    clip_velocity_millimeters_per_second: [i32; 2],
    desired_velocity_millimeters_per_second: [i32; 2],
    max_stride_scale_millionths: u32,
) -> Option<MotionWarpV1> {
    let clip_length =
        (squared_distance(clip_velocity_millimeters_per_second, [0, 0]) as f32).sqrt();
    let desired_length =
        (squared_distance(desired_velocity_millimeters_per_second, [0, 0]) as f32).sqrt();
    if clip_length <= f32::EPSILON || desired_length <= f32::EPSILON {
        return None;
    }
    let scale = ((desired_length / clip_length) * 1_000_000.0).round() as u32;
    let clip_yaw = (clip_velocity_millimeters_per_second[1] as f32)
        .atan2(clip_velocity_millimeters_per_second[0] as f32);
    let desired_yaw = (desired_velocity_millimeters_per_second[1] as f32)
        .atan2(desired_velocity_millimeters_per_second[0] as f32);
    let mut turn = desired_yaw - clip_yaw;
    while turn > std::f32::consts::PI {
        turn -= std::f32::consts::TAU;
    }
    while turn <= -std::f32::consts::PI {
        turn += std::f32::consts::TAU;
    }
    Some(MotionWarpV1 {
        stride_scale_millionths: scale,
        turn_delta_millionths: (turn * 1_000_000.0).round() as i32,
        feasible: scale > 0 && scale <= max_stride_scale_millionths,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MotionFeedbackV1 {
    pub feasible: bool,
    pub root_deviation_millionths: u32,
    pub foot_slide_millionths: u32,
    pub slope_error_millionths: u32,
    pub correction: MotionCorrectionV1,
}

impl MotionFeedbackV1 {
    pub fn evaluate(
        requested_velocity: [i32; 2],
        actual_velocity: [i32; 2],
        requested_slope_millionths: i32,
        actual_slope_millionths: i32,
        foot_slide_millionths: u32,
        max_root_deviation_millionths: u32,
        max_foot_slide_millionths: u32,
    ) -> Self {
        let root_deviation_millionths =
            ((squared_distance(requested_velocity, actual_velocity) as f64).sqrt() * 1_000.0)
                .round() as u32;
        let slope_error_millionths = requested_slope_millionths.abs_diff(actual_slope_millionths);
        let feasible = root_deviation_millionths <= max_root_deviation_millionths
            && foot_slide_millionths <= max_foot_slide_millionths
            && slope_error_millionths <= max_root_deviation_millionths;
        let correction = if feasible {
            MotionCorrectionV1::None
        } else if foot_slide_millionths <= max_foot_slide_millionths
            && root_deviation_millionths <= max_root_deviation_millionths
        {
            MotionCorrectionV1::AdjustTurn
        } else {
            MotionCorrectionV1::FallbackClip
        };
        Self {
            feasible,
            root_deviation_millionths,
            foot_slide_millionths,
            slope_error_millionths,
            correction,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MotionError {
    UnsupportedVersion(u32),
    InvalidDatabase(&'static str),
    InvalidClip(String),
    DuplicateClip(String),
    MissingFallback(String),
}

impl fmt::Display for MotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported trajectory schema version {version}")
            }
            Self::InvalidDatabase(message) => write!(f, "invalid motion database: {message}"),
            Self::InvalidClip(id) => write!(f, "invalid motion clip {id}"),
            Self::DuplicateClip(id) => write!(f, "motion clip {id} is duplicated"),
            Self::MissingFallback(id) => write!(f, "motion fallback clip {id} is missing"),
        }
    }
}

impl Error for MotionError {}

fn squared_distance(left: [i32; 2], right: [i32; 2]) -> i64 {
    let x = i64::from(left[0]) - i64::from(right[0]);
    let y = i64::from(left[1]) - i64::from(right[1]);
    x.saturating_mul(x).saturating_add(y.saturating_mul(y))
}
