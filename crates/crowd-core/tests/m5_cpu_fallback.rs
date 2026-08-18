//! M5 10K gate item 5: CPU fallback produces contract-compatible output, with
//! a documented numeric tolerance where bitwise parity is not demonstrated.
//!
//! No GPU backend is implemented (see `docs/backend-support-matrix.md`), so
//! there are two things to prove today:
//!
//! 1. Requesting an unimplemented backend degrades to `cpu_reference` and says
//!    so, rather than failing or silently producing an empty field.
//! 2. The comparison harness that a future GPU backend must pass actually
//!    discriminates. That is checked against a second CPU implementation whose
//!    arithmetic is deliberately reordered — the same class of difference a
//!    GPU reduction introduces — and against one that is deliberately wrong.
//!
//! Without (2) the harness would be an untested promise, and the first GPU
//! backend would be measured by a comparison nobody had ever seen fail.

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::field::{
    compare_kernels, select_backend, CpuSpatialField, FieldBackend, FieldConfig, FieldSample,
    FieldValue, KernelTolerance, SpatialFieldKernel,
};
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::units::Vec2;
use crowd_core::FidelityPolicy;

/// Field samples and probe positions taken from a real background population
/// rather than a synthetic grid, so the comparison sees the clustering and
/// velocity spread the kernel actually runs against.
fn city_flow_samples() -> (Vec<FieldSample>, Vec<Vec2>) {
    let scene = scenes::build("m5_city_flow", 600, 2026)
        .expect("m5_city_flow is the declared M5 scale fixture")
        .compile()
        .expect("the fixture must compile");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig {
            fidelity: Some(FidelityPolicy::m5_10k_profile()),
            ..SimConfig::default()
        },
    );
    sim.run(600);

    let world = sim.world();
    let samples: Vec<FieldSample> = (0..world.len())
        .map(|slot| FieldSample {
            position: world.position(slot as u32),
            velocity: world.velocity(slot as u32),
        })
        .collect();
    // Probe every agent position plus a lattice, so empty cells are covered
    // too: a backend that returns a default for a populated cell and one that
    // returns garbage for an empty cell are both faults.
    let mut probes: Vec<Vec2> = samples.iter().map(|sample| sample.position).collect();
    for x in -30..30 {
        for y in -10..10 {
            probes.push(Vec2::new(x as f32 * 4.0, y as f32 * 4.0));
        }
    }
    (samples, probes)
}

/// A second CPU implementation that accumulates each cell's velocity sum in a
/// different order, which is how a parallel or GPU reduction differs from the
/// sequential reference: same result mathematically, not the same float.
#[derive(Default)]
struct ReorderedCpuField {
    cell_size_m: f32,
    cells: std::collections::BTreeMap<(i32, i32), (u32, Vec<Vec2>)>,
}

impl SpatialFieldKernel for ReorderedCpuField {
    fn backend_name(&self) -> &'static str {
        "cpu_reordered_parity_fixture"
    }

    fn build(&mut self, samples: &[FieldSample], config: FieldConfig) -> Result<(), &'static str> {
        config.validate()?;
        self.cell_size_m = config.cell_size_m;
        self.cells.clear();
        for sample in samples {
            if !sample.position.is_finite() || !sample.velocity.is_finite() {
                return Err("field samples must be finite");
            }
            let key = (
                (sample.position.x / self.cell_size_m).floor() as i32,
                (sample.position.y / self.cell_size_m).floor() as i32,
            );
            let entry = self.cells.entry(key).or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(sample.velocity);
        }
        // Pairwise summation: a balanced reduction tree instead of a running
        // total, which is what a GPU work-group reduction does.
        for (_, velocities) in self.cells.values_mut() {
            let mut level = velocities.clone();
            while level.len() > 1 {
                let mut next = Vec::with_capacity(level.len().div_ceil(2));
                for pair in level.chunks(2) {
                    next.push(match pair {
                        [a, b] => *a + *b,
                        [a] => *a,
                        _ => unreachable!("chunks(2) yields one or two elements"),
                    });
                }
                level = next;
            }
            *velocities = level;
        }
        Ok(())
    }

    fn sample(&self, position: Vec2) -> FieldValue {
        if !position.is_finite() || self.cell_size_m <= 0.0 {
            return FieldValue::default();
        }
        let key = (
            (position.x / self.cell_size_m).floor() as i32,
            (position.y / self.cell_size_m).floor() as i32,
        );
        match self.cells.get(&key) {
            None => FieldValue::default(),
            Some((density, sum)) => FieldValue {
                density: *density,
                mean_velocity: sum.first().copied().unwrap_or(Vec2::ZERO) * (1.0 / *density as f32),
            },
        }
    }
}

/// A backend that is wrong in the way a broken GPU kernel is wrong: plausible
/// output, off by more than the declared tolerance.
#[derive(Default)]
struct DriftingCpuField {
    inner: CpuSpatialField,
}

impl SpatialFieldKernel for DriftingCpuField {
    fn backend_name(&self) -> &'static str {
        "cpu_drifting_negative_fixture"
    }

    fn build(&mut self, samples: &[FieldSample], config: FieldConfig) -> Result<(), &'static str> {
        self.inner.build(samples, config)
    }

    fn sample(&self, position: Vec2) -> FieldValue {
        let mut value = self.inner.sample(position);
        value.mean_velocity = value.mean_velocity + Vec2::new(0.01, 0.0);
        value
    }
}

fn build_reference(samples: &[FieldSample]) -> CpuSpatialField {
    let mut field = CpuSpatialField::default();
    field
        .build(samples, FieldConfig::default())
        .expect("the reference kernel must accept a finite population");
    field
}

#[test]
fn an_unimplemented_backend_falls_back_to_the_cpu_reference() {
    for requested in [
        FieldBackend::Metal,
        FieldBackend::Cuda,
        FieldBackend::Vulkan,
    ] {
        let (kernel, selection) = select_backend(requested);
        assert_eq!(selection.requested, requested);
        assert_eq!(selection.active, FieldBackend::CpuReference);
        assert!(selection.fell_back);
        assert!(
            selection
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains(requested.name())),
            "a fallback must name the backend it could not provide"
        );
        assert_eq!(kernel.backend_name(), "cpu_reference");
    }
}

#[test]
fn the_cpu_reference_is_never_reported_as_a_fallback() {
    let (kernel, selection) = select_backend(FieldBackend::CpuReference);
    assert_eq!(selection.active, FieldBackend::CpuReference);
    assert!(!selection.fell_back);
    assert_eq!(selection.reason, None);
    assert_eq!(kernel.backend_name(), "cpu_reference");
}

#[test]
fn a_fallback_kernel_still_produces_contract_compatible_output() {
    // The point of the fallback is that the caller gets a usable field, not
    // merely a non-error. Build through the fallback path and require it to
    // agree with the reference exactly — it is the same implementation, so
    // anything less would mean the selection path corrupted something.
    let (samples, probes) = city_flow_samples();
    let reference = build_reference(&samples);

    let (mut fallback, selection) = select_backend(FieldBackend::Metal);
    assert!(selection.fell_back);
    fallback
        .build(&samples, FieldConfig::default())
        .expect("the fallback kernel must accept the same population");

    let comparison = compare_kernels(
        &reference,
        fallback.as_ref(),
        &probes,
        KernelTolerance::default(),
    );
    assert_eq!(comparison.density_mismatches, 0);
    assert!(comparison.bitwise_identical, "{comparison:?}");
    assert!(comparison.within_tolerance, "{comparison:?}");
}

#[test]
fn a_reordered_reduction_agrees_within_the_declared_tolerance() {
    // This is the documented-tolerance case the gate asks about: a backend
    // that is mathematically equivalent but not bitwise identical still
    // passes, and the comparison records both facts.
    let (samples, probes) = city_flow_samples();
    // A coarser cell than the 2 m default, deliberately: at 2 m most cells
    // hold a single agent, the two reductions collapse to the same one-term
    // sum, and the comparison agrees bitwise — proving nothing about the
    // tolerance path. At 8 m the cells hold enough agents for the summation
    // order to actually matter.
    let config = FieldConfig { cell_size_m: 8.0 };
    let mut reference = CpuSpatialField::default();
    reference
        .build(&samples, config)
        .expect("the reference kernel must accept a finite population");
    let mut candidate = ReorderedCpuField::default();
    candidate
        .build(&samples, config)
        .expect("the parity fixture must accept the same population");

    let comparison = compare_kernels(&reference, &candidate, &probes, KernelTolerance::default());
    assert_eq!(
        comparison.density_mismatches, 0,
        "a reordered sum must not move an agent between cells"
    );
    assert!(
        !comparison.bitwise_identical,
        "the fixture must actually diverge, or this proves nothing about tolerance: {comparison:?}"
    );
    assert!(
        comparison.within_tolerance,
        "reordered reduction exceeded the declared tolerance: {comparison:?}"
    );
    assert!(
        comparison.max_mean_velocity_error_mps <= comparison.tolerance_mean_velocity_error_mps,
        "{comparison:?}"
    );
}

#[test]
fn a_backend_outside_the_tolerance_is_rejected() {
    // Guards the two tests above: without this, a comparison that always
    // passed would look like parity evidence.
    let (samples, probes) = city_flow_samples();
    let reference = build_reference(&samples);
    let mut candidate = DriftingCpuField::default();
    candidate
        .build(&samples, FieldConfig::default())
        .expect("the negative fixture must accept the same population");

    let comparison = compare_kernels(&reference, &candidate, &probes, KernelTolerance::default());
    assert!(
        !comparison.within_tolerance,
        "a backend drifting 0.01 m/s must not pass a 0.001 m/s tolerance: {comparison:?}"
    );
    assert!(!comparison.bitwise_identical);
}

#[test]
fn a_backend_that_moves_agents_between_cells_is_rejected_regardless_of_tolerance() {
    // Density is a meaning difference, so it fails even when the velocity
    // error is zero and the tolerance is enormous.
    let (samples, probes) = city_flow_samples();
    let reference = build_reference(&samples);

    let mut shifted = CpuSpatialField::default();
    let moved: Vec<FieldSample> = samples
        .iter()
        .map(|sample| FieldSample {
            position: sample.position + Vec2::new(1.0, 0.0),
            velocity: sample.velocity,
        })
        .collect();
    shifted
        .build(&moved, FieldConfig::default())
        .expect("the shifted fixture must accept a finite population");

    let comparison = compare_kernels(
        &reference,
        &shifted,
        &probes,
        KernelTolerance {
            max_mean_velocity_error_mps: 1e6,
        },
    );
    assert!(comparison.density_mismatches > 0);
    assert!(!comparison.within_tolerance, "{comparison:?}");
}
