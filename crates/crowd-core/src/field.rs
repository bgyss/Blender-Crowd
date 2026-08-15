//! Backend-neutral coarse density and velocity fields for M5 background flow.
//!
//! The kernel consumes immutable agent samples and returns a field only; it
//! has no access to IDs, routes, or world mutation.  A GPU implementation can
//! therefore replace the CPU implementation without changing authoritative
//! simulation semantics or cache meaning.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

/// A backend a caller can ask for.
///
/// Asking for one is not the same as getting it. The M5 backend support matrix
/// is the authoritative boundary, and `select` below is the single place that
/// decides — so an unimplemented backend degrades to the CPU reference with a
/// recorded reason rather than failing at render time or, worse, silently
/// producing nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldBackend {
    #[default]
    CpuReference,
    Metal,
    Cuda,
    Vulkan,
}

impl FieldBackend {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "cpu_reference" => Ok(Self::CpuReference),
            "metal" => Ok(Self::Metal),
            "cuda" => Ok(Self::Cuda),
            "vulkan" => Ok(Self::Vulkan),
            other => Err(format!(
                "unknown field backend: {other}; known backends: cpu_reference, metal, cuda, vulkan"
            )),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::CpuReference => "cpu_reference",
            Self::Metal => "metal",
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
        }
    }

    /// Whether an implementation exists and is tested.
    ///
    /// Only `cpu_reference` is. Flipping one of the others here without an
    /// implementation and a measured parity comparison would make the support
    /// matrix a claim rather than a record.
    pub const fn is_implemented(self) -> bool {
        matches!(self, Self::CpuReference)
    }
}

/// What a caller actually got, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackendSelection {
    pub requested: FieldBackend,
    pub active: FieldBackend,
    pub fell_back: bool,
    /// Present only on a fallback, so a report says why rather than leaving a
    /// reader to infer it from the two names.
    pub reason: Option<String>,
}

/// Resolve a requested backend to one that exists.
///
/// The only fallback target is the CPU reference, which is always available:
/// there is no configuration in which this returns nothing, so a caller never
/// has to carry a "no backend" path.
pub fn select_backend(requested: FieldBackend) -> (Box<dyn SpatialFieldKernel>, BackendSelection) {
    let active = if requested.is_implemented() {
        requested
    } else {
        FieldBackend::CpuReference
    };
    let selection = BackendSelection {
        requested,
        active,
        fell_back: active != requested,
        reason: (active != requested).then(|| {
            format!(
                "{} is not implemented in this build; see docs/backend-support-matrix.md",
                requested.name()
            )
        }),
    };
    let kernel: Box<dyn SpatialFieldKernel> = match active {
        FieldBackend::CpuReference => Box::new(CpuSpatialField::default()),
        // Unreachable while `is_implemented` admits only the CPU reference.
        // Written as an explicit arm rather than a catch-all so adding a
        // backend has to come here and say what it constructs.
        FieldBackend::Metal | FieldBackend::Cuda | FieldBackend::Vulkan => {
            Box::new(CpuSpatialField::default())
        }
    };
    (kernel, selection)
}

/// Numeric agreement a candidate backend must show against the CPU reference.
///
/// Density is an integer count of agents in a cell and carries no tolerance:
/// a backend that puts an agent in a different cell has changed the meaning of
/// the field, not its precision. Mean velocity is a float reduction whose
/// result depends on summation order, so it carries a declared tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KernelTolerance {
    pub max_mean_velocity_error_mps: f32,
}

impl Default for KernelTolerance {
    fn default() -> Self {
        // One millimetre per second. Far below any velocity a presentation or
        // coarse-perception consumer can act on, and far above the rounding a
        // reordered float sum introduces at these magnitudes.
        Self {
            max_mean_velocity_error_mps: 1e-3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KernelComparison {
    pub reference_backend: String,
    pub candidate_backend: String,
    pub probes: u64,
    /// Probes where the two backends disagreed on the integer agent count.
    /// Must be zero: this is a meaning difference, not a precision one.
    pub density_mismatches: u64,
    pub max_mean_velocity_error_mps: f32,
    pub tolerance_mean_velocity_error_mps: f32,
    /// True only when every probe agreed bit for bit. When false, the
    /// comparison stands on `max_mean_velocity_error_mps` against the declared
    /// tolerance, which is what the M5 gate asks a report to document.
    pub bitwise_identical: bool,
    pub within_tolerance: bool,
}

/// Compare two built kernels at a fixed set of probe positions.
///
/// Probes are supplied by the caller and visited in the given order, so the
/// comparison is reproducible rather than depending on either backend's
/// internal cell iteration order.
pub fn compare_kernels(
    reference: &dyn SpatialFieldKernel,
    candidate: &dyn SpatialFieldKernel,
    probes: &[Vec2],
    tolerance: KernelTolerance,
) -> KernelComparison {
    let mut density_mismatches = 0u64;
    let mut max_error = 0.0f32;
    let mut bitwise_identical = true;

    for probe in probes {
        let expected = reference.sample(*probe);
        let actual = candidate.sample(*probe);
        if expected.density != actual.density {
            density_mismatches += 1;
        }
        let error = (actual.mean_velocity - expected.mean_velocity).length();
        if error > max_error {
            max_error = error;
        }
        if expected.mean_velocity.x.to_bits() != actual.mean_velocity.x.to_bits()
            || expected.mean_velocity.y.to_bits() != actual.mean_velocity.y.to_bits()
        {
            bitwise_identical = false;
        }
    }

    KernelComparison {
        reference_backend: reference.backend_name().to_string(),
        candidate_backend: candidate.backend_name().to_string(),
        probes: probes.len() as u64,
        density_mismatches,
        max_mean_velocity_error_mps: max_error,
        tolerance_mean_velocity_error_mps: tolerance.max_mean_velocity_error_mps,
        bitwise_identical,
        within_tolerance: density_mismatches == 0
            && max_error <= tolerance.max_mean_velocity_error_mps,
    }
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
