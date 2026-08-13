//! Backend-neutral coarse density and velocity fields for M5 background flow.
//!
//! The kernel consumes immutable agent samples and returns a field only; it
//! has no access to IDs, routes, or world mutation.  A GPU implementation can
//! therefore replace the CPU implementation without changing authoritative
//! simulation semantics or cache meaning.

use std::collections::BTreeMap;

use crate::units::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldSample {
    pub position: Vec2,
    pub velocity: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldValue {
    /// Agents represented by the cell; consumers must treat this as an
    /// aggregate rather than a substitute for identity-aware contacts.
    pub density: u32,
    pub mean_velocity: Vec2,
}

impl Default for FieldValue {
    fn default() -> Self {
        Self {
            density: 0,
            mean_velocity: Vec2::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldConfig {
    pub cell_size_m: f32,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self { cell_size_m: 2.0 }
    }
}

impl FieldConfig {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.cell_size_m.is_finite() || self.cell_size_m <= 0.0 {
            return Err("field cell_size_m must be finite and positive");
        }
        Ok(())
    }
}

/// Contract shared by deterministic CPU and optional GPU field kernels.
/// Implementations may differ numerically only within a caller-declared
/// tolerance; they must never mutate identity or root motion.
pub trait SpatialFieldKernel {
    fn backend_name(&self) -> &'static str;
    fn build(&mut self, samples: &[FieldSample], config: FieldConfig) -> Result<(), &'static str>;
    fn sample(&self, position: Vec2) -> FieldValue;
}

#[derive(Clone, Debug, Default)]
pub struct CpuSpatialField {
    cell_size_m: f32,
    cells: BTreeMap<(i32, i32), FieldValue>,
}

impl CpuSpatialField {
    fn key(&self, position: Vec2) -> (i32, i32) {
        (
            (position.x / self.cell_size_m).floor() as i32,
            (position.y / self.cell_size_m).floor() as i32,
        )
    }
}

impl SpatialFieldKernel for CpuSpatialField {
    fn backend_name(&self) -> &'static str {
        "cpu_reference"
    }

    fn build(&mut self, samples: &[FieldSample], config: FieldConfig) -> Result<(), &'static str> {
        config.validate()?;
        self.cell_size_m = config.cell_size_m;
        self.cells.clear();
        // Integer accumulation order is stable independent of hash-map order;
        // sum floats in caller-provided stable-ID slot order.
        for sample in samples {
            if !sample.position.is_finite() || !sample.velocity.is_finite() {
                return Err("field samples must be finite");
            }
            let key = self.key(sample.position);
            let value = self.cells.entry(key).or_default();
            value.density += 1;
            value.mean_velocity = value.mean_velocity + sample.velocity;
        }
        for value in self.cells.values_mut() {
            value.mean_velocity = value.mean_velocity * (1.0 / value.density as f32);
        }
        Ok(())
    }

    fn sample(&self, position: Vec2) -> FieldValue {
        if !position.is_finite() || self.cell_size_m <= 0.0 {
            return FieldValue::default();
        }
        self.cells
            .get(&self.key(position))
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_field_reports_density_and_mean_velocity() {
        let mut field = CpuSpatialField::default();
        field
            .build(
                &[
                    FieldSample {
                        position: Vec2::new(0.1, 0.1),
                        velocity: Vec2::new(1.0, 0.0),
                    },
                    FieldSample {
                        position: Vec2::new(1.9, 1.9),
                        velocity: Vec2::new(3.0, 2.0),
                    },
                ],
                FieldConfig::default(),
            )
            .unwrap();
        assert_eq!(field.backend_name(), "cpu_reference");
        assert_eq!(
            field.sample(Vec2::new(0.5, 0.5)),
            FieldValue {
                density: 2,
                mean_velocity: Vec2::new(2.0, 1.0)
            }
        );
        assert_eq!(field.sample(Vec2::new(4.0, 4.0)), FieldValue::default());
    }

    #[test]
    fn field_rejects_bad_inputs_without_publishing_partial_results() {
        let mut field = CpuSpatialField::default();
        assert!(field.build(&[], FieldConfig { cell_size_m: 0.0 }).is_err());
        assert!(field
            .build(
                &[FieldSample {
                    position: Vec2::new(f32::NAN, 0.0),
                    velocity: Vec2::ZERO
                }],
                FieldConfig::default()
            )
            .is_err());
        assert_eq!(field.sample(Vec2::ZERO), FieldValue::default());
    }
}
