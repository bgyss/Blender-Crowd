//! Fixed-step simulation clock.
//!
//! An integer tick rate, a `dt` derived once from it, and a `u64` tick counter
//! are the kernel's only notion of time. No kernel code reads a wall clock,
//! thread identity, or address — those would break determinism.

use crate::units::DEFAULT_TICKS_PER_SECOND;

/// A fixed-step clock. `dt` never varies, so integration is reproducible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    ticks_per_second: u32,
    dt: f32,
    tick: u64,
}

impl Clock {
    pub fn new(ticks_per_second: u32) -> Self {
        assert!(ticks_per_second > 0, "ticks_per_second must be non-zero");
        Self {
            ticks_per_second,
            dt: 1.0 / ticks_per_second as f32,
            tick: 0,
        }
    }

    /// Seconds per tick. Constant for the clock's whole lifetime.
    pub fn dt(&self) -> f32 {
        self.dt
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn ticks_per_second(&self) -> u32 {
        self.ticks_per_second
    }

    pub fn advance(&mut self) {
        self.tick += 1;
    }

    /// Elapsed simulated seconds, computed in `f64` for reporting only.
    ///
    /// Integration uses `dt` directly; accumulating `dt` in `f32` would drift.
    pub fn time_seconds(&self) -> f64 {
        self.tick as f64 / self.ticks_per_second as f64
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new(DEFAULT_TICKS_PER_SECOND)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_clock_runs_at_thirty_hertz() {
        let clock = Clock::default();
        assert_eq!(clock.ticks_per_second(), 30);
        assert!((clock.dt() - 1.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn clock_starts_at_tick_zero() {
        assert_eq!(Clock::default().tick(), 0);
    }

    #[test]
    fn advance_increments_tick_and_time() {
        let mut clock = Clock::new(60);
        clock.advance();
        clock.advance();
        assert_eq!(clock.tick(), 2);
        assert!((clock.time_seconds() - 2.0 / 60.0).abs() < 1e-12);
    }

    #[test]
    fn dt_is_constant_across_ticks() {
        let mut clock = Clock::new(30);
        let first = clock.dt();
        for _ in 0..1000 {
            clock.advance();
        }
        assert_eq!(clock.dt(), first);
    }

    #[test]
    #[should_panic(expected = "ticks_per_second must be non-zero")]
    fn zero_rate_is_rejected() {
        Clock::new(0);
    }
}
