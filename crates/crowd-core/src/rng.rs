//! Deterministic per-agent randomness.
//!
//! No `rand` crate: its algorithms and stream values are not a stability
//! guarantee, and this project's caches and baselines depend on exact values.
//!
//! Values are keyed by `(global_seed, agent_id, purpose)` rather than drawn
//! from a shared sequence. That is what delivers contract section 4.2: because
//! each attribute has its own stream keyed by stable ID, adding an agent does
//! not reshuffle existing variants, and adding a new attribute does not shift
//! existing ones.

use crate::ids::{hash_combine, mix64, AgentId};

/// Names the attribute a random stream is for.
///
/// Add new variants at the end with fresh tag values. Never renumber an
/// existing tag: doing so silently changes every previously drawn value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Radius,
    PreferredSpeed,
    MaxSpeed,
    SpawnPosition,
    DestinationChoice,
    SpawnSourceChoice,
    ArchetypeChoice,
    AppearanceChoice,
    Scale,
}

impl Purpose {
    pub const fn tag(self) -> u64 {
        match self {
            Purpose::Radius => 1,
            Purpose::PreferredSpeed => 2,
            Purpose::MaxSpeed => 3,
            Purpose::SpawnPosition => 4,
            Purpose::DestinationChoice => 5,
            Purpose::SpawnSourceChoice => 6,
            Purpose::ArchetypeChoice => 7,
            Purpose::AppearanceChoice => 8,
            Purpose::Scale => 9,
        }
    }
}

/// A SplitMix64 generator seeded from stable inputs.
#[derive(Clone, Copy, Debug)]
pub struct StableRng {
    state: u64,
}

impl StableRng {
    pub const fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Seed a stream for one agent's one attribute.
    pub fn for_agent(global_seed: u64, agent: AgentId, purpose: Purpose) -> Self {
        let mut h = mix64(global_seed);
        h = hash_combine(h, agent.0);
        h = hash_combine(h, purpose.tag());
        Self { state: h }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        mix64(self.state)
    }

    /// Uniform in `[0, 1)`.
    ///
    /// Takes the top 24 bits so every result is exactly representable in
    /// `f32`, which keeps the mapping reproducible bit-for-bit.
    pub fn next_f32_unit(&mut self) -> f32 {
        const SCALE: f32 = 1.0 / (1u32 << 24) as f32;
        ((self.next_u64() >> 40) as u32) as f32 * SCALE
    }

    /// Uniform in `[lo, hi)` — `hi` is unreachable, because `next_f32_unit`
    /// never returns exactly 1.0.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32_unit()
    }

    /// Uniform in `[lo, hi)`. Returns `lo` when the range is empty.
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo) as u64) as u32
    }

    /// Normally distributed via Box-Muller.
    ///
    /// `u1` is nudged off exact zero because `ln(0)` is infinite, and a single
    /// infinite preferred speed would poison an entire bake.
    pub fn normal_f32(&mut self, mean: f32, stddev: f32) -> f32 {
        const TWO_PI: f32 = std::f32::consts::TAU;
        let u1 = self.next_f32_unit().max(f32::MIN_POSITIVE);
        let u2 = self.next_f32_unit();
        let magnitude = (-2.0 * u1.ln()).sqrt();
        mean + stddev * magnitude * (TWO_PI * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::derive_agent_id;

    fn radius_for(seed: u64, ordinal: u32) -> f32 {
        let id = derive_agent_id(seed, 0, 0, ordinal);
        StableRng::for_agent(seed, id, Purpose::Radius).range_f32(0.24, 0.38)
    }

    #[test]
    fn same_agent_and_purpose_always_yields_the_same_value() {
        assert_eq!(radius_for(7, 12), radius_for(7, 12));
    }

    #[test]
    fn adding_an_agent_does_not_reshuffle_existing_variants() {
        // Contract section 4.2. Draw a population of 100, then a population of
        // 101, and confirm the first 100 are untouched.
        let small: Vec<f32> = (0..100).map(|i| radius_for(7, i)).collect();
        let large: Vec<f32> = (0..101).map(|i| radius_for(7, i)).collect();
        assert_eq!(small, large[..100]);
    }

    #[test]
    fn different_purposes_produce_independent_streams() {
        let id = derive_agent_id(7, 0, 0, 3);
        let radius = StableRng::for_agent(7, id, Purpose::Radius).next_u64();
        let speed = StableRng::for_agent(7, id, Purpose::PreferredSpeed).next_u64();
        assert_ne!(radius, speed);
    }

    #[test]
    fn changing_the_global_seed_changes_values() {
        assert_ne!(radius_for(7, 12), radius_for(8, 12));
    }

    #[test]
    fn range_f32_stays_within_bounds() {
        let mut rng = StableRng::from_seed(99);
        for _ in 0..10_000 {
            let v = rng.range_f32(-2.5, 4.0);
            assert!((-2.5..=4.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn next_f32_unit_stays_in_half_open_unit_interval() {
        let mut rng = StableRng::from_seed(1);
        for _ in 0..10_000 {
            let v = rng.next_f32_unit();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn normal_f32_has_approximately_the_requested_moments() {
        let mut rng = StableRng::from_seed(5);
        let n = 100_000;
        let samples: Vec<f32> = (0..n).map(|_| rng.normal_f32(1.35, 0.18)).collect();
        let mean = samples.iter().sum::<f32>() / n as f32;
        let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n as f32;
        assert!((mean - 1.35).abs() < 0.01, "mean was {mean}");
        assert!(
            (var.sqrt() - 0.18).abs() < 0.01,
            "stddev was {}",
            var.sqrt()
        );
    }

    #[test]
    fn normal_f32_is_always_finite() {
        let mut rng = StableRng::from_seed(3);
        for _ in 0..100_000 {
            assert!(rng.normal_f32(0.0, 1.0).is_finite());
        }
    }

    #[test]
    fn range_u32_stays_within_bounds() {
        let mut rng = StableRng::from_seed(11);
        for _ in 0..10_000 {
            let v = rng.range_u32(3, 9);
            assert!((3..9).contains(&v), "out of range: {v}");
        }
    }
}
