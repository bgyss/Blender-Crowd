# Deterministic Crowd Simulation Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a headless, deterministic Rust crowd simulation kernel with structure-of-arrays agents, a fixed tick, spatial queries, one sampled-velocity avoidance solver, five benchmark scenes, and a measured metrics report.

**Architecture:** A Cargo workspace with `crowd-core` (library: contracts, SoA world, tick phases, avoidance, scenes, metrics) and `crowd-bench` (binary: scene runner, JSON report, SVG dump, baseline check). Every tick phase is a free function reading immutable previous-state buffers and writing next-state buffers, so read/write sets are visible in signatures and a later parallel pass needs no semantic change. All randomness and identity derive from vendored hash functions keyed by stable ID, never from iteration order.

**Tech Stack:** Rust (stable 1.94.1 via rustup), `serde` + `serde_json` for reports, `proptest` as a dev-dependency only. No `rand`, no ECS, no graphics stack.

**Spec:** [Deterministic crowd simulation kernel (slice 1) — design](../specs/2026-08-04-crowd-sim-kernel-design.md)

**Parent contract:** [Blender Crowd 1.0 architecture and MVP](../../blender-crowd-1.0.md)

## Global Constraints

Every task's requirements implicitly include this section.

- **Toolchain:** stable Rust, pinned to `1.94.1` in `rust-toolchain.toml` and `mise.toml`. Do not use nightly features.
- **Linker:** this machine's `PATH` `cc` is nix `gcc`, which cannot link macOS `libSystem` on arm64. `.cargo/config.toml` MUST set `linker = "/usr/bin/clang"` for the Apple targets. Without it nothing links, including dependency-free crates. The setting is scoped per-target so Linux and Windows builds are unaffected.
- **Dev environment is not the shipped artifact.** mise, rustup, nix, and uv configure a contributor's machine. Nothing produced for Blender may depend on them at runtime. This slice ships no Blender artifact, so the rule is recorded rather than enforced here — see "Toolchain and packaging boundary" below for the constraints the future PyO3 slice inherits.
- **Edition:** `2021` for all crates.
- **Dependencies:** `crowd-core` may depend only on `serde` (feature `derive`). `crowd-bench` may additionally depend on `serde_json`. `proptest` is a dev-dependency only. Adding any other dependency requires changing this plan.
- **No `rand` crate, no external hasher.** Identity and randomness use functions vendored in `crowd-core::ids` and `crowd-core::rng`. Their exact output is a versioned contract; changing them invalidates every baseline.
- **Determinism rules, policed by tests:** no `HashMap` iteration in the tick path (use `BTreeMap` or sorted `Vec`); no value derived from addresses, thread identity, or wall-clock time; all ties broken by `AgentId`; no fast-math or float-reassociation flags.
- **Units:** meters, seconds, radians. Z-up right-handed. Pedestrian kinematics are planar in XY. Orientation is a yaw scalar about Z.
- **Formatting:** `cargo fmt` before every commit. Four-space indent is rustfmt's default; do not override.
- **Naming:** `snake_case` modules and functions, `PascalCase` types, crate package names kebab-case (`crowd-core`), directory names matching the package name.
- **No absolute quality thresholds.** Baselines are measured and checked in; the `check` command tests relative drift only.

## Toolchain and Packaging Boundary

The development environment and the redistributable Blender extension are separate concerns, and conflating them is a known way to ship a plugin that only works on the machine that built it.

**Development environment** — managed by `mise` (Rust toolchain, and Python later), which reads the same version pins the repo commits. `rustup` remains the underlying Rust installer. `nix` and `uv` are fine for a contributor's shell. None of this is committed into any build output.

**Shipped artifact** — not produced by this slice, but the constraints are recorded now because they shape decisions made here:

1. The Blender extension ZIP must contain `__init__.py` and `blender_manifest.toml` **at the archive root**, never nested under a package directory.
2. Add-on code uses **relative imports** throughout, because extensions are imported as `bl_ext.<repo>.<id>`.
3. The native module must be built against the **Blender-bundled CPython ABI** for each supported platform, not against a nix, mise, uv, or Homebrew Python.
4. The native module must have **no absolute rpaths or dynamic links into the nix store, a mise shim, or a uv venv**. This is the specific trap in this environment: a nix-toolchain build happily embeds `/nix/store/...` paths that resolve on the build machine and nowhere else. Verify with `otool -L` on macOS and `ldd` on Linux before shipping.
5. Set an explicit `MACOSX_DEPLOYMENT_TARGET` matching Blender's own floor, so the module loads on older macOS than the build host.

Consequences for this slice: `crowd-core` stays a plain `cdylib`-free library crate with no build-time environment capture beyond the rustc version string recorded in reports, and its only dependencies are pure-Rust crates that cross-compile cleanly. Task 20's `build.rs` records the rustc version as **report metadata only** and must never gate compilation on it.

---

## File Structure

```text
rust-toolchain.toml                     toolchain pin
.cargo/config.toml                      linker fix
.gitignore                              target/, benchmarks/reports/
Cargo.toml                              workspace manifest
crates/crowd-core/
  Cargo.toml
  src/lib.rs                            module wiring and re-exports
  src/units.rs                          Vec2, Aabb, unit constants
  src/clock.rs                          fixed-step Clock
  src/ids.rs                            mix64, AgentId, stable derivation
  src/rng.rs                            StableRng, Purpose streams
  src/geometry.rs                       Segment, time-to-collision math
  src/world.rs                          SoA World, slot table, commit
  src/arena.rs                          NeighborArena
  src/grid.rs                           UniformGrid, SegmentIndex
  src/route.rs                          WaypointGraph, RouteArena, following
  src/scene.rs                          SceneDef, CompiledScene, diagnostics
  src/avoidance/mod.rs                  AvoidanceSolver trait and I/O types
  src/avoidance/sampled.rs              SampledVelocitySolver
  src/phases/mod.rs                     phase re-exports
  src/phases/spawn.rs                   apply inputs
  src/phases/perceive.rs                neighbor collection
  src/phases/decide.rs                  route advance (decide + plan)
  src/phases/steer.rs                   preferred velocity + avoidance
  src/phases/integrate.rs               limits, advance, wall resolution
  src/metrics.rs                        accumulators and summary
  src/scenes.rs                         the five benchmark scenes
  src/sim.rs                            Simulation tick loop, state hash
  tests/determinism.rs                  bitwise, permutation, add-one-agent
  tests/properties.rs                   proptest suites
  tests/fuzz_density.rs                 randomized density fuzzing
crates/crowd-bench/
  Cargo.toml
  build.rs                              capture rustc version
  src/main.rs                           CLI: run, sweep, check
  src/alloc.rs                          counting global allocator
  src/report.rs                         Report, Environment, JSON output
  src/svg.rs                            trajectory SVG dump
  src/baseline.rs                       baseline load and compare
benchmarks/baselines/*.json             measured, checked in
benchmarks/reports/                     generated, git-ignored
```

---

## Task 1: Workspace, toolchain, and linker fix

Nothing else in this plan can be verified until `cargo test` runs. Confirm this task's exit condition before starting Task 2.

**Files:**
- Create: `rust-toolchain.toml`
- Create: `mise.toml`
- Create: `.cargo/config.toml`
- Create: `.gitignore`
- Create: `Cargo.toml`
- Create: `crates/crowd-core/Cargo.toml`
- Create: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: a buildable workspace; `crowd_core` crate root.

- [ ] **Step 1: Write the toolchain pins**

`rust-toolchain.toml` — read by `rustup` and by `cargo` directly:

```toml
[toolchain]
channel = "1.94.1"
components = ["rustfmt", "clippy"]
```

`mise.toml` — the contributor-facing environment. The version is duplicated deliberately: `rust-toolchain.toml` is what `cargo` honors with or without mise, so it must stand alone.

```toml
[tools]
rust = "1.94.1"

[env]
# Reports record this; it never gates compilation.
CROWD_BUILD_PROFILE = "dev"

[tasks.test]
run = "cargo test --workspace"

[tasks.fmt]
run = "cargo fmt --all"

[tasks.lint]
run = "cargo clippy --workspace --all-targets -- -D warnings"
```

If the two pins ever disagree, `rust-toolchain.toml` wins and mise is wrong; fix mise.

- [ ] **Step 2: Write the linker configuration**

`.cargo/config.toml`:

```toml
# The `cc` on PATH in this environment is nix gcc, which cannot resolve macOS
# libSystem symbols on arm64; every link fails, including for dependency-free
# crates. Point rustc at the system clang instead.
[target.aarch64-apple-darwin]
linker = "/usr/bin/clang"

[target.x86_64-apple-darwin]
linker = "/usr/bin/clang"
```

- [ ] **Step 3: Write the ignore file**

`.gitignore`:

```gitignore
/target
/benchmarks/reports
```

- [ ] **Step 4: Write the workspace manifest**

`Cargo.toml`:

```toml
[workspace]
members = ["crates/crowd-core"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "GPL-3.0-or-later"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
proptest = "1"

[profile.release]
debug = true
```

`debug = true` in release keeps symbols for profiling without changing codegen.

- [ ] **Step 5: Write the core crate manifest**

`crates/crowd-core/Cargo.toml`:

```toml
[package]
name = "crowd-core"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
```

- [ ] **Step 6: Write the crate root with a smoke test**

`crates/crowd-core/src/lib.rs`:

```rust
//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

#[cfg(test)]
mod smoke {
    #[test]
    fn workspace_builds_and_links() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 7: Run the test**

Run: `cargo test -p crowd-core`
Expected: PASS, `test smoke::workspace_builds_and_links ... ok`.

If this fails with `ld: symbol(s) not found for architecture arm64`, `.cargo/config.toml` is not being picked up — confirm it sits at the repository root, not inside `crates/`.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add rust-toolchain.toml mise.toml .cargo/config.toml .gitignore Cargo.toml crates/
git commit -m "Add Rust workspace with pinned toolchain and macOS linker fix"
```

---

## Task 2: Units and vector primitives

**Files:**
- Create: `crates/crowd-core/src/units.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Vec2 { x: f32, y: f32 }` with `new`, `ZERO`, `length`, `length_squared`, `normalize_or_zero`, `perp`, `dot`, `distance_squared`, `clamp_length`, `to_yaw`, `from_yaw`, `is_finite`, and `Add`/`Sub`/`Mul<f32>`/`Neg` operators. `Aabb { min: Vec2, max: Vec2 }` with `contains`, `center`, `size`, `expanded`. Free function `wrap_angle(angle: f32) -> f32`. Constants `DEFAULT_TICKS_PER_SECOND: u32 = 30`, `WORLD_TO_METER: f32 = 1.0`.

`wrap_angle` lives here because both the integrate phase and the metrics layer need it; duplicating it in each would be two copies of one convention that must never disagree.

- [ ] **Step 1: Write the failing test**

Append to `crates/crowd-core/src/units.rs` (create the file with just this for now):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_or_zero_handles_zero_vector() {
        assert_eq!(Vec2::ZERO.normalize_or_zero(), Vec2::ZERO);
    }

    #[test]
    fn normalize_or_zero_produces_unit_length() {
        let v = Vec2::new(3.0, 4.0).normalize_or_zero();
        assert!((v.length() - 1.0).abs() < 1e-6);
        assert!((v.x - 0.6).abs() < 1e-6);
        assert!((v.y - 0.8).abs() < 1e-6);
    }

    #[test]
    fn clamp_length_leaves_short_vectors_untouched() {
        let v = Vec2::new(1.0, 0.0);
        assert_eq!(v.clamp_length(2.0), v);
    }

    #[test]
    fn clamp_length_shortens_long_vectors() {
        let v = Vec2::new(10.0, 0.0).clamp_length(2.0);
        assert!((v.length() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn perp_is_ninety_degrees_left() {
        let v = Vec2::new(1.0, 0.0).perp();
        assert!((v.x - 0.0).abs() < 1e-6);
        assert!((v.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_contains_only_points_inside() {
        let b = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 2.0));
        assert!(b.contains(Vec2::new(1.0, 1.0)));
        assert!(!b.contains(Vec2::new(3.0, 1.0)));
    }

    #[test]
    fn wrap_angle_leaves_small_angles_alone() {
        assert!((wrap_angle(0.5) - 0.5).abs() < 1e-6);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-6);
    }

    #[test]
    fn wrap_angle_takes_the_short_way_round() {
        use std::f32::consts::PI;
        // Just past +pi must come out just past -pi, not as a near-full turn.
        let wrapped = wrap_angle(PI + 0.1);
        assert!(wrapped < 0.0, "got {wrapped}");
        assert!((wrapped + PI - 0.1).abs() < 1e-4, "got {wrapped}");
    }

    #[test]
    fn wrap_angle_output_is_always_within_half_turn() {
        use std::f32::consts::PI;
        for step in -50..50 {
            let angle = step as f32 * 0.7;
            let wrapped = wrap_angle(angle);
            assert!(wrapped > -PI - 1e-5 && wrapped <= PI + 1e-5, "got {wrapped}");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core units`
Expected: FAIL — `cannot find type Vec2 in this scope` (the module is not yet declared either).

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/units.rs`, above the `tests` module:

```rust
//! Unit and coordinate contract.
//!
//! Meters, seconds, radians. Z-up right-handed to match Blender exactly.
//! Pedestrian kinematics are planar in XY; ground height Z is resolved from
//! the environment rather than integrated. Orientation is a yaw scalar about Z.

use std::ops::{Add, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

/// Default simulation rate in ticks per second.
pub const DEFAULT_TICKS_PER_SECOND: u32 = 30;

/// Scene scale factor. This slice asserts 1.0; scaling has no test yet.
pub const WORLD_TO_METER: f32 = 1.0;

/// A planar vector in the XY ground plane, in meters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn distance_squared(self, other: Vec2) -> f32 {
        (other - self).length_squared()
    }

    /// Unit vector, or exactly zero when the input is degenerate.
    ///
    /// Returning zero rather than NaN keeps a stalled agent's state finite,
    /// which the tick loop relies on.
    pub fn normalize_or_zero(self) -> Vec2 {
        let len_sq = self.length_squared();
        if len_sq <= f32::MIN_POSITIVE {
            Vec2::ZERO
        } else {
            let inv = 1.0 / len_sq.sqrt();
            Vec2::new(self.x * inv, self.y * inv)
        }
    }

    /// Rotate 90 degrees counter-clockwise (to the agent's left, Z-up).
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    pub fn clamp_length(self, max_length: f32) -> Vec2 {
        let len_sq = self.length_squared();
        if len_sq <= max_length * max_length {
            self
        } else {
            self.normalize_or_zero() * max_length
        }
    }

    /// Yaw about Z, in radians, for the direction this vector points.
    pub fn to_yaw(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn from_yaw(yaw: f32) -> Vec2 {
        Vec2::new(yaw.cos(), yaw.sin())
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// An axis-aligned bounding box in the XY ground plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb {
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn expanded(&self, margin: f32) -> Aabb {
        Aabb::new(
            Vec2::new(self.min.x - margin, self.min.y - margin),
            Vec2::new(self.max.x + margin, self.max.y + margin),
        )
    }
}

/// Wrap an angle to `(-pi, pi]` so differences take the short way round.
///
/// Shared rather than duplicated: the integrate phase uses it to limit turn
/// rate and the metrics layer uses it to measure heading change. Two copies of
/// one convention is two chances for them to disagree about what a reversal is.
pub fn wrap_angle(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut a = angle % TAU;
    if a > PI {
        a -= TAU;
    } else if a <= -PI {
        a += TAU;
    }
    a
}
```

- [ ] **Step 4: Declare the module**

Replace the contents of `crates/crowd-core/src/lib.rs`:

```rust
//! Deterministic crowd simulation kernel.
//!
//! See `docs/superpowers/specs/2026-08-04-crowd-sim-kernel-design.md`.

pub mod units;

pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core units`
Expected: PASS, 9 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/units.rs crates/crowd-core/src/lib.rs
git commit -m "Add Vec2 and Aabb primitives for the planar unit contract"
```

---

## Task 3: Fixed-step clock

**Files:**
- Create: `crates/crowd-core/src/clock.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `crowd_core::units::DEFAULT_TICKS_PER_SECOND`.
- Produces: `Clock` with `new(ticks_per_second: u32) -> Clock`, `dt(&self) -> f32`, `tick(&self) -> u64`, `ticks_per_second(&self) -> u32`, `advance(&mut self)`, `time_seconds(&self) -> f64`, and `Default`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/clock.rs` with only:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core clock`
Expected: FAIL — `cannot find type Clock in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/clock.rs`:

```rust
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
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs`, add after `pub mod units;`:

```rust
pub mod clock;
```

and extend the re-export line to:

```rust
pub use clock::Clock;
pub use units::{wrap_angle, Aabb, Vec2, DEFAULT_TICKS_PER_SECOND, WORLD_TO_METER};
```

Later tasks only ever *add* `pub mod` and `pub use` lines here. Never drop an existing export while adding yours.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core clock`
Expected: PASS, 5 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/clock.rs crates/crowd-core/src/lib.rs
git commit -m "Add fixed-step simulation clock"
```

---

## Task 4: Stable identity

**Files:**
- Create: `crates/crowd-core/src/ids.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `mix64(z: u64) -> u64`, `hash_combine(seed: u64, value: u64) -> u64`, `AgentId(pub u64)` deriving `Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize`, `derive_agent_id(project_seed: u64, population_id: u16, spawn_source_id: u16, ordinal: u32) -> AgentId`, and `hash_str(s: &str) -> u64`.

The exact numeric output of these functions is a versioned contract. Changing a constant invalidates every checked-in baseline and every future cache.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/ids.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn mix64_output_is_pinned() {
        // These values are a contract. If this test fails after an
        // intentional change, every baseline and cache must be regenerated.
        assert_eq!(mix64(0), 0);
        assert_eq!(mix64(1), 6238072747940578789);
        assert_eq!(mix64(u64::MAX), 7256069403641682740);
    }

    #[test]
    fn derived_ids_are_unique_across_ordinals() {
        let mut seen = BTreeSet::new();
        for ordinal in 0..10_000u32 {
            assert!(seen.insert(derive_agent_id(42, 0, 0, ordinal)));
        }
    }

    #[test]
    fn derived_ids_are_unique_across_spawn_sources() {
        let a = derive_agent_id(42, 0, 0, 7);
        let b = derive_agent_id(42, 0, 1, 7);
        assert_ne!(a, b);
    }

    #[test]
    fn derived_ids_are_unique_across_populations() {
        assert_ne!(derive_agent_id(42, 0, 0, 7), derive_agent_id(42, 1, 0, 7));
    }

    #[test]
    fn derived_ids_depend_on_project_seed() {
        assert_ne!(derive_agent_id(42, 0, 0, 7), derive_agent_id(43, 0, 0, 7));
    }

    #[test]
    fn derived_ids_are_independent_of_call_order() {
        let forward: Vec<_> = (0..100).map(|i| derive_agent_id(1, 0, 0, i)).collect();
        let backward: Vec<_> = (0..100)
            .rev()
            .map(|i| derive_agent_id(1, 0, 0, i))
            .collect();
        let backward: Vec<_> = backward.into_iter().rev().collect();
        assert_eq!(forward, backward);
    }

    #[test]
    fn hash_str_is_stable_and_distinguishes_inputs() {
        assert_eq!(hash_str("platform_a"), hash_str("platform_a"));
        assert_ne!(hash_str("platform_a"), hash_str("platform_b"));
    }
}
```

The two pinned `mix64` values in the first test are placeholders you must replace with the real measured output. Step 4 tells you exactly how.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core ids`
Expected: FAIL — `cannot find function mix64 in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/ids.rs`:

```rust
//! Stable identity contract.
//!
//! Agent IDs derive from the project seed, population, spawn source, and
//! spawn ordinal, per contract section 5.1. The mixing function is vendored
//! rather than taken from an external hasher: an external hash's exact output
//! is not a stability guarantee, and if it changed, every cache and baseline
//! in the project would silently break. Vendoring makes it part of the
//! versioned contract.

use serde::{Deserialize, Serialize};

/// SplitMix64 finalizer. Strong avalanche, no dependencies, permanently fixed.
pub const fn mix64(z: u64) -> u64 {
    let mut z = z;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Fold one value into a running hash.
pub const fn hash_combine(seed: u64, value: u64) -> u64 {
    mix64(seed ^ mix64(value.wrapping_add(0x9e3779b97f4a7c15)))
}

/// FNV-1a over bytes, then finalized through `mix64`.
///
/// Used only to turn authoring names into stable numeric IDs at scene compile
/// time. Never called inside the tick loop.
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in s.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    mix64(h)
}

/// A stable 64-bit agent identifier.
///
/// Stable across rebakes when unrelated authoring data changes, and the
/// tie-breaker for every ordering decision in the kernel.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct AgentId(pub u64);

/// Derive an agent's stable ID, per contract section 5.1.
///
/// Deliberately a pure function of its inputs: it never consults a counter,
/// the world, or call order, so IDs do not shift when unrelated agents are
/// added or removed.
pub fn derive_agent_id(
    project_seed: u64,
    population_id: u16,
    spawn_source_id: u16,
    ordinal: u32,
) -> AgentId {
    let mut h = mix64(project_seed);
    h = hash_combine(h, population_id as u64);
    h = hash_combine(h, spawn_source_id as u64);
    h = hash_combine(h, ordinal as u64);
    AgentId(h)
}
```

- [ ] **Step 4: Measure and pin the `mix64` values**

The placeholders in the first test are almost certainly wrong. Print the real values:

```bash
cat > /tmp/pin_mix64.rs <<'EOF'
const fn mix64(z: u64) -> u64 {
    let mut z = z;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
fn main() {
    println!("mix64(0)        = {}", mix64(0));
    println!("mix64(1)        = {}", mix64(1));
    println!("mix64(u64::MAX) = {}", mix64(u64::MAX));
}
EOF
rustc -O -o /tmp/pin_mix64 /tmp/pin_mix64.rs && /tmp/pin_mix64
```

Copy the three printed numbers into `mix64_output_is_pinned`, replacing the placeholders.

- [ ] **Step 5: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod ids;` and `pub use ids::{derive_agent_id, AgentId};`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p crowd-core ids`
Expected: PASS, 7 tests.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/ids.rs crates/crowd-core/src/lib.rs
git commit -m "Add vendored stable agent identity derivation"
```

---

## Task 5: Stable per-agent randomness

This is the task that actually delivers contract section 4.2's promise: adding an agent must not reshuffle existing variants, and adding a new *attribute* must not shift existing ones.

**Files:**
- Create: `crates/crowd-core/src/rng.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `crowd_core::ids::{mix64, hash_combine, AgentId}`.
- Produces: `Purpose` enum with variants `Radius`, `PreferredSpeed`, `MaxSpeed`, `SpawnPosition`, `DestinationChoice` and method `tag(self) -> u64`. `StableRng` with `for_agent(global_seed: u64, agent: AgentId, purpose: Purpose) -> StableRng`, `from_seed(seed: u64) -> StableRng`, `next_u64(&mut self) -> u64`, `next_f32_unit(&mut self) -> f32`, `range_f32(&mut self, lo: f32, hi: f32) -> f32`, `normal_f32(&mut self, mean: f32, stddev: f32) -> f32`, `range_u32(&mut self, lo: u32, hi: u32) -> u32`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/rng.rs` with only:

```rust
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
        assert!((var.sqrt() - 0.18).abs() < 0.01, "stddev was {}", var.sqrt());
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core rng`
Expected: FAIL — `cannot find type StableRng in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/rng.rs`:

```rust
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
}

impl Purpose {
    pub const fn tag(self) -> u64 {
        match self {
            Purpose::Radius => 1,
            Purpose::PreferredSpeed => 2,
            Purpose::MaxSpeed => 3,
            Purpose::SpawnPosition => 4,
            Purpose::DestinationChoice => 5,
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

    /// Uniform in `[lo, hi]`.
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
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod rng;` and `pub use rng::{Purpose, StableRng};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core rng`
Expected: PASS, 9 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/rng.rs crates/crowd-core/src/lib.rs
git commit -m "Add stable per-agent random streams keyed by ID and purpose"
```

---

## Task 6: Geometry and time-to-collision

**Files:**
- Create: `crates/crowd-core/src/geometry.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `crowd_core::units::{Aabb, Vec2}`.
- Produces: `Segment { a: Vec2, b: Vec2 }` with `new`, `closest_point(&self, p: Vec2) -> Vec2`, `distance_to(&self, p: Vec2) -> f32`, `bounds(&self) -> Aabb`. Free functions `time_to_collision_disc(rel_pos: Vec2, rel_vel: Vec2, combined_radius: f32) -> Option<f32>` and `time_to_collision_segment(pos: Vec2, vel: Vec2, radius: f32, seg: &Segment, horizon: f32) -> Option<f32>`.

`time_to_collision_disc` returns `Some(0.0)` when the discs already overlap, `Some(t)` for a future collision with `t >= 0`, and `None` when they never collide.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/geometry.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    #[test]
    fn closest_point_clamps_to_the_segment_endpoints() {
        let seg = Segment::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
        assert_eq!(seg.closest_point(Vec2::new(-5.0, 1.0)), Vec2::new(0.0, 0.0));
        assert_eq!(seg.closest_point(Vec2::new(9.0, 1.0)), Vec2::new(2.0, 0.0));
        assert_eq!(seg.closest_point(Vec2::new(1.0, 3.0)), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn distance_to_measures_perpendicular_distance() {
        let seg = Segment::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0));
        assert!((seg.distance_to(Vec2::new(1.0, 3.0)) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_segment_behaves_like_a_point() {
        let seg = Segment::new(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0));
        assert_eq!(seg.closest_point(Vec2::new(4.0, 5.0)), Vec2::new(1.0, 1.0));
    }

    #[test]
    fn head_on_approach_has_a_finite_time_to_collision() {
        // Two unit-radius discs 10m apart closing at 1 m/s each: surfaces meet
        // after 8m of closure, so t = 8.0.
        let t = time_to_collision_disc(Vec2::new(10.0, 0.0), Vec2::new(-2.0, 0.0), 2.0);
        assert!((t.unwrap() - 4.0).abs() < 1e-4, "got {t:?}");
    }

    #[test]
    fn separating_discs_never_collide() {
        assert_eq!(
            time_to_collision_disc(Vec2::new(10.0, 0.0), Vec2::new(2.0, 0.0), 2.0),
            None
        );
    }

    #[test]
    fn parallel_passing_discs_never_collide() {
        assert_eq!(
            time_to_collision_disc(Vec2::new(0.0, 5.0), Vec2::new(-2.0, 0.0), 2.0),
            None
        );
    }

    #[test]
    fn already_overlapping_discs_report_zero() {
        let t = time_to_collision_disc(Vec2::new(0.5, 0.0), Vec2::new(-1.0, 0.0), 2.0);
        assert_eq!(t, Some(0.0));
    }

    #[test]
    fn stationary_discs_never_collide() {
        assert_eq!(
            time_to_collision_disc(Vec2::new(10.0, 0.0), Vec2::ZERO, 2.0),
            None
        );
    }

    #[test]
    fn agent_walking_into_a_wall_has_a_finite_time_to_collision() {
        let wall = Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0));
        let t = time_to_collision_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, &wall, 10.0);
        assert!((t.unwrap() - 4.5).abs() < 1e-2, "got {t:?}");
    }

    #[test]
    fn agent_walking_away_from_a_wall_has_no_collision() {
        let wall = Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0));
        assert_eq!(
            time_to_collision_segment(Vec2::ZERO, Vec2::new(-1.0, 0.0), 0.5, &wall, 10.0),
            None
        );
    }

    #[test]
    fn wall_collision_beyond_the_horizon_is_ignored() {
        let wall = Segment::new(Vec2::new(50.0, -5.0), Vec2::new(50.0, 5.0));
        assert_eq!(
            time_to_collision_segment(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.5, &wall, 10.0),
            None
        );
    }
}
```

Note the head-on test: `rel_vel` is the full relative velocity (`-2.0`), the gap between surfaces is `10 - 2 = 8`, so `t = 8 / 2 = 4.0`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core geometry`
Expected: FAIL — `cannot find type Segment in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/geometry.rs`:

```rust
//! Analytic static geometry and predictive collision math.
//!
//! The navmesh is deferred (contract section 6.1), so this slice represents
//! the environment as line segments. They serve two roles: constraints for the
//! avoidance solver, and hard boundaries the integrate phase resolves against.

use crate::units::{Aabb, Vec2};

/// A wall, as a line segment in the XY ground plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: Vec2,
    pub b: Vec2,
}

impl Segment {
    pub const fn new(a: Vec2, b: Vec2) -> Self {
        Self { a, b }
    }

    /// The point on the segment nearest `p`, clamped to the endpoints.
    pub fn closest_point(&self, p: Vec2) -> Vec2 {
        let ab = self.b - self.a;
        let len_sq = ab.length_squared();
        if len_sq <= f32::MIN_POSITIVE {
            return self.a;
        }
        let t = ((p - self.a).dot(ab) / len_sq).clamp(0.0, 1.0);
        self.a + ab * t
    }

    pub fn distance_to(&self, p: Vec2) -> f32 {
        (p - self.closest_point(p)).length()
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec2::new(self.a.x.min(self.b.x), self.a.y.min(self.b.y)),
            Vec2::new(self.a.x.max(self.b.x), self.a.y.max(self.b.y)),
        )
    }
}

/// Time until two moving discs touch, or `None` if they never do.
///
/// `rel_pos` is other minus self, `rel_vel` is other's velocity minus self's,
/// and `combined_radius` is the sum of both radii. Returns `Some(0.0)` when
/// they already overlap, so callers treat interpenetration as maximally urgent
/// rather than as "no collision".
pub fn time_to_collision_disc(
    rel_pos: Vec2,
    rel_vel: Vec2,
    combined_radius: f32,
) -> Option<f32> {
    let dist_sq = rel_pos.length_squared();
    let radius_sq = combined_radius * combined_radius;
    if dist_sq <= radius_sq {
        return Some(0.0);
    }

    // Solve |rel_pos + rel_vel * t| = combined_radius for the smaller root.
    let a = rel_vel.length_squared();
    if a <= f32::MIN_POSITIVE {
        return None;
    }
    let b = rel_pos.dot(rel_vel);
    if b >= 0.0 {
        return None; // Separating or parallel.
    }
    let c = dist_sq - radius_sq;
    let discriminant = b * b - a * c;
    if discriminant <= 0.0 {
        return None; // Passes by without touching.
    }
    let t = (-b - discriminant.sqrt()) / a;
    if t < 0.0 {
        None
    } else {
        Some(t)
    }
}

/// Time until a moving disc touches a static segment, within `horizon`.
///
/// Sampled rather than solved in closed form: the exact swept-capsule test is
/// three cases (two endpoints plus the edge), and at the sample counts the
/// solver uses, a short bisection is both simpler to keep correct and fast
/// enough. Determinism is unaffected because the sample count is fixed.
pub fn time_to_collision_segment(
    pos: Vec2,
    vel: Vec2,
    radius: f32,
    seg: &Segment,
    horizon: f32,
) -> Option<f32> {
    if seg.distance_to(pos) <= radius {
        return Some(0.0);
    }
    if vel.length_squared() <= f32::MIN_POSITIVE {
        return None;
    }

    const COARSE_STEPS: u32 = 8;
    const REFINE_STEPS: u32 = 12;

    let mut lo = 0.0f32;
    let mut hit = false;
    let mut hi = horizon;
    for i in 1..=COARSE_STEPS {
        let t = horizon * i as f32 / COARSE_STEPS as f32;
        if seg.distance_to(pos + vel * t) <= radius {
            hi = t;
            hit = true;
            break;
        }
        lo = t;
    }
    if !hit {
        return None;
    }

    for _ in 0..REFINE_STEPS {
        let mid = 0.5 * (lo + hi);
        if seg.distance_to(pos + vel * mid) <= radius {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod geometry;` and `pub use geometry::Segment;`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core geometry`
Expected: PASS, 11 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/geometry.rs crates/crowd-core/src/lib.rs
git commit -m "Add segment geometry and predictive time-to-collision math"
```

---

## Task 7: Structure-of-arrays world

**Files:**
- Create: `crates/crowd-core/src/world.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `crowd_core::ids::AgentId`, `crowd_core::units::Vec2`.
- Produces: `SolverStatus` enum (`Free`, `Avoiding`, `Braking`) with `Default = Free`. `AgentSpawn` struct (fields `agent_id: AgentId`, `population_id: u16`, `position: Vec2`, `yaw: f32`, `radius: f32`, `max_speed: f32`, `preferred_speed: f32`, `route: RouteHandle`, `destination: u16`). `SpawnError::DuplicateAgentId(AgentId)`. `World` with public `Vec` columns as listed in the spec, plus `new()`, `len()`, `is_empty()`, `spawn(&mut self, spawn: AgentSpawn, tick: u64) -> Result<u32, SpawnError>`, `slot_of(&self, id: AgentId) -> Option<u32>`, `position(&self, slot: u32) -> Vec2`, `velocity(&self, slot: u32) -> Vec2`, `commit(&mut self)`, `state_hash(&self) -> u64`.

`RouteHandle` is defined in Task 9. To keep tasks independently testable, define it in `world.rs` now and have Task 9 import it from here.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/world.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    fn spawn_at(id: u64, position: Vec2) -> AgentSpawn {
        AgentSpawn {
            agent_id: AgentId(id),
            population_id: 0,
            position,
            yaw: 0.0,
            radius: 0.3,
            max_speed: 1.8,
            preferred_speed: 1.35,
            route: NO_ROUTE,
            destination: 0,
        }
    }

    #[test]
    fn new_world_is_empty() {
        let world = World::new();
        assert_eq!(world.len(), 0);
        assert!(world.is_empty());
    }

    #[test]
    fn spawn_appends_a_slot_and_records_state() {
        let mut world = World::new();
        let slot = world.spawn(spawn_at(1, Vec2::new(2.0, 3.0)), 0).unwrap();
        assert_eq!(slot, 0);
        assert_eq!(world.len(), 1);
        assert_eq!(world.position(0), Vec2::new(2.0, 3.0));
        assert_eq!(world.agent_id[0], AgentId(1));
        assert_eq!(world.spawn_tick[0], 0);
        assert_eq!(world.velocity(0), Vec2::ZERO);
    }

    #[test]
    fn spawn_rejects_duplicate_agent_ids() {
        let mut world = World::new();
        world.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        let err = world.spawn(spawn_at(1, Vec2::ZERO), 1).unwrap_err();
        assert_eq!(err, SpawnError::DuplicateAgentId(AgentId(1)));
    }

    #[test]
    fn slot_of_round_trips_stable_ids() {
        let mut world = World::new();
        world.spawn(spawn_at(10, Vec2::ZERO), 0).unwrap();
        world.spawn(spawn_at(20, Vec2::ZERO), 0).unwrap();
        assert_eq!(world.slot_of(AgentId(20)), Some(1));
        assert_eq!(world.slot_of(AgentId(30)), None);
    }

    #[test]
    fn all_columns_stay_the_same_length() {
        let mut world = World::new();
        for i in 0..25 {
            world.spawn(spawn_at(i, Vec2::ZERO), 0).unwrap();
        }
        let n = world.len();
        assert_eq!(world.pos_x.len(), n);
        assert_eq!(world.pos_y.len(), n);
        assert_eq!(world.vel_x.len(), n);
        assert_eq!(world.next_pos_x.len(), n);
        assert_eq!(world.solver_status.len(), n);
        assert_eq!(world.stall_ticks.len(), n);
        assert_eq!(world.route_index.len(), n);
    }

    #[test]
    fn commit_moves_next_state_into_current() {
        let mut world = World::new();
        world.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        world.next_pos_x[0] = 5.0;
        world.next_pos_y[0] = 6.0;
        world.next_vel_x[0] = 1.0;
        world.next_vel_y[0] = 0.0;
        world.next_yaw[0] = 0.5;
        world.commit();
        assert_eq!(world.position(0), Vec2::new(5.0, 6.0));
        assert_eq!(world.velocity(0), Vec2::new(1.0, 0.0));
        assert_eq!(world.yaw[0], 0.5);
    }

    #[test]
    fn state_hash_changes_when_state_changes() {
        let mut world = World::new();
        world.spawn(spawn_at(1, Vec2::ZERO), 0).unwrap();
        let before = world.state_hash();
        world.next_pos_x[0] = 1.0;
        world.commit();
        assert_ne!(world.state_hash(), before);
    }

    #[test]
    fn state_hash_is_identical_for_identical_state() {
        let mut a = World::new();
        let mut b = World::new();
        for i in 0..10 {
            a.spawn(spawn_at(i, Vec2::new(i as f32, 0.0)), 0).unwrap();
            b.spawn(spawn_at(i, Vec2::new(i as f32, 0.0)), 0).unwrap();
        }
        assert_eq!(a.state_hash(), b.state_hash());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core world`
Expected: FAIL — `cannot find type World in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/world.rs`:

```rust
//! Structure-of-arrays agent state.
//!
//! Contract section 5.2. Each hot field is its own `Vec` indexed by dense
//! slot; a stable-ID-to-slot table keeps IDs stable while slots stay dense.
//! Slot order is derived from stable IDs, so iteration order is deterministic
//! by construction.
//!
//! Only fields this slice writes exist. `group_id`, `fidelity_tier`,
//! `blackboard_handle`, and the animation columns from the contract are
//! omitted because nothing would write them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{hash_combine, AgentId};
use crate::units::Vec2;

/// A handle into the route arena. `NO_ROUTE` means "no path assigned".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteHandle(pub u32);

pub const NO_ROUTE: RouteHandle = RouteHandle(u32::MAX);

/// Why the avoidance solver produced the velocity it did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolverStatus {
    /// No neighbor or wall constrained the choice.
    #[default]
    Free,
    /// A constraint moved the agent off its preferred velocity.
    Avoiding,
    /// No candidate was feasible; the agent slowed or stopped.
    Braking,
}

/// Everything needed to introduce one agent.
#[derive(Clone, Copy, Debug)]
pub struct AgentSpawn {
    pub agent_id: AgentId,
    pub population_id: u16,
    pub position: Vec2,
    pub yaw: f32,
    pub radius: f32,
    pub max_speed: f32,
    pub preferred_speed: f32,
    pub route: RouteHandle,
    pub destination: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnError {
    /// Contract section 10.3 makes this a bake-blocking condition.
    DuplicateAgentId(AgentId),
}

/// Dense structure-of-arrays agent storage.
#[derive(Clone, Debug, Default)]
pub struct World {
    // Identity.
    pub agent_id: Vec<AgentId>,
    pub population_id: Vec<u16>,
    pub spawn_tick: Vec<u64>,

    // Kinematic.
    pub pos_x: Vec<f32>,
    pub pos_y: Vec<f32>,
    pub yaw: Vec<f32>,
    pub vel_x: Vec<f32>,
    pub vel_y: Vec<f32>,
    pub radius: Vec<f32>,
    pub max_speed: Vec<f32>,
    pub preferred_speed: Vec<f32>,

    // Navigation.
    pub route: Vec<RouteHandle>,
    pub route_index: Vec<u16>,
    pub destination: Vec<u16>,
    pub arrived: Vec<bool>,

    // Staging. Written by steer, consumed by integrate.
    pub des_vel_x: Vec<f32>,
    pub des_vel_y: Vec<f32>,
    pub next_pos_x: Vec<f32>,
    pub next_pos_y: Vec<f32>,
    pub next_vel_x: Vec<f32>,
    pub next_vel_y: Vec<f32>,
    pub next_yaw: Vec<f32>,

    // Debug.
    pub solver_status: Vec<SolverStatus>,
    pub stall_ticks: Vec<u16>,

    slot_of_id: BTreeMap<AgentId, u32>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.agent_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.agent_id.is_empty()
    }

    pub fn slot_of(&self, id: AgentId) -> Option<u32> {
        self.slot_of_id.get(&id).copied()
    }

    pub fn position(&self, slot: u32) -> Vec2 {
        Vec2::new(self.pos_x[slot as usize], self.pos_y[slot as usize])
    }

    pub fn velocity(&self, slot: u32) -> Vec2 {
        Vec2::new(self.vel_x[slot as usize], self.vel_y[slot as usize])
    }

    pub fn desired_velocity(&self, slot: u32) -> Vec2 {
        Vec2::new(self.des_vel_x[slot as usize], self.des_vel_y[slot as usize])
    }

    pub fn spawn(&mut self, spawn: AgentSpawn, tick: u64) -> Result<u32, SpawnError> {
        if self.slot_of_id.contains_key(&spawn.agent_id) {
            return Err(SpawnError::DuplicateAgentId(spawn.agent_id));
        }
        let slot = self.agent_id.len() as u32;

        self.agent_id.push(spawn.agent_id);
        self.population_id.push(spawn.population_id);
        self.spawn_tick.push(tick);

        self.pos_x.push(spawn.position.x);
        self.pos_y.push(spawn.position.y);
        self.yaw.push(spawn.yaw);
        self.vel_x.push(0.0);
        self.vel_y.push(0.0);
        self.radius.push(spawn.radius);
        self.max_speed.push(spawn.max_speed);
        self.preferred_speed.push(spawn.preferred_speed);

        self.route.push(spawn.route);
        self.route_index.push(0);
        self.destination.push(spawn.destination);
        self.arrived.push(false);

        self.des_vel_x.push(0.0);
        self.des_vel_y.push(0.0);
        self.next_pos_x.push(spawn.position.x);
        self.next_pos_y.push(spawn.position.y);
        self.next_vel_x.push(0.0);
        self.next_vel_y.push(0.0);
        self.next_yaw.push(spawn.yaw);

        self.solver_status.push(SolverStatus::Free);
        self.stall_ticks.push(0);

        self.slot_of_id.insert(spawn.agent_id, slot);
        Ok(slot)
    }

    /// Publish staged next-state into current state.
    ///
    /// Called once at the end of a tick. Until this runs, every phase reads a
    /// consistent snapshot of the previous tick, which is what makes results
    /// independent of iteration order.
    pub fn commit(&mut self) {
        self.pos_x.copy_from_slice(&self.next_pos_x);
        self.pos_y.copy_from_slice(&self.next_pos_y);
        self.vel_x.copy_from_slice(&self.next_vel_x);
        self.vel_y.copy_from_slice(&self.next_vel_y);
        self.yaw.copy_from_slice(&self.next_yaw);
    }

    /// A bitwise digest of all authoritative agent state.
    ///
    /// Hashes float *bits*, not values, so the determinism tests compare
    /// exactly rather than within a tolerance.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xa5a5_5a5a_dead_beef;
        for slot in 0..self.len() {
            h = hash_combine(h, self.agent_id[slot].0);
            h = hash_combine(h, self.pos_x[slot].to_bits() as u64);
            h = hash_combine(h, self.pos_y[slot].to_bits() as u64);
            h = hash_combine(h, self.vel_x[slot].to_bits() as u64);
            h = hash_combine(h, self.vel_y[slot].to_bits() as u64);
            h = hash_combine(h, self.yaw[slot].to_bits() as u64);
            h = hash_combine(h, self.route_index[slot] as u64);
            h = hash_combine(h, self.arrived[slot] as u64);
        }
        h
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod world;` and `pub use world::{AgentSpawn, RouteHandle, SolverStatus, SpawnError, World, NO_ROUTE};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core world`
Expected: PASS, 8 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/world.rs crates/crowd-core/src/lib.rs
git commit -m "Add structure-of-arrays agent world with staged next-state"
```

---

## Task 8: Uniform grid and neighbor arena

**Files:**
- Create: `crates/crowd-core/src/grid.rs`
- Create: `crates/crowd-core/src/arena.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `crowd_core::units::{Aabb, Vec2}`, `crowd_core::geometry::Segment`.
- Produces: `UniformGrid::new(bounds: Aabb, cell_size: f32) -> UniformGrid`, `rebuild(&mut self, pos_x: &[f32], pos_y: &[f32])`, `query(&self, center: Vec2, radius: f32, out: &mut Vec<u32>)`, `cell_size(&self) -> f32`. `SegmentIndex::build(bounds: Aabb, cell_size: f32, segments: &[Segment]) -> SegmentIndex` and `query(&self, center: Vec2, radius: f32, out: &mut Vec<u32>)`. `Neighbor { slot: u32, dist_sq: f32 }` and `NeighborArena::new()`, `begin(&mut self, agent_count: usize)`, `push(&mut self, slot_owner: usize, neighbors: &[Neighbor])`, `neighbors(&self, slot: usize) -> &[Neighbor]`.

`query` appends slot indices whose cell overlaps the query circle's bounding box. It is a broad phase: callers must still filter by exact distance.

- [ ] **Step 1: Write the failing test for the arena**

Create `crates/crowd-core/src/arena.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_arena_returns_empty_slices() {
        let mut arena = NeighborArena::new();
        arena.begin(3);
        assert!(arena.neighbors(0).is_empty());
        assert!(arena.neighbors(2).is_empty());
    }

    #[test]
    fn pushed_neighbors_round_trip_per_agent() {
        let mut arena = NeighborArena::new();
        arena.begin(2);
        arena.push(0, &[Neighbor { slot: 5, dist_sq: 1.0 }]);
        arena.push(
            1,
            &[
                Neighbor { slot: 6, dist_sq: 2.0 },
                Neighbor { slot: 7, dist_sq: 3.0 },
            ],
        );
        assert_eq!(arena.neighbors(0).len(), 1);
        assert_eq!(arena.neighbors(0)[0].slot, 5);
        assert_eq!(arena.neighbors(1).len(), 2);
        assert_eq!(arena.neighbors(1)[1].slot, 7);
    }

    #[test]
    fn begin_reuses_capacity_without_reallocating() {
        let mut arena = NeighborArena::new();
        arena.begin(4);
        arena.push(0, &[Neighbor { slot: 1, dist_sq: 1.0 }; 32]);
        let capacity = arena.capacity();
        arena.begin(4);
        assert_eq!(arena.capacity(), capacity);
        assert!(arena.neighbors(0).is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core arena`
Expected: FAIL — `cannot find type NeighborArena in this scope`.

- [ ] **Step 3: Write the arena implementation**

Prepend to `crates/crowd-core/src/arena.rs`:

```rust
//! Pooled per-tick neighbor storage.
//!
//! Contract section 5.2 requires the hot loop not to allocate. Buffers are
//! cleared and refilled each tick rather than freed, so steady-state ticks do
//! no allocator work at all.

/// One observed neighbor: its slot and squared distance at observation time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbor {
    pub slot: u32,
    pub dist_sq: f32,
}

/// Flat per-agent neighbor lists with `(start, len)` indexing.
#[derive(Clone, Debug, Default)]
pub struct NeighborArena {
    entries: Vec<Neighbor>,
    start: Vec<u32>,
    len: Vec<u32>,
}

impl NeighborArena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset for a tick with `agent_count` agents, keeping allocated capacity.
    pub fn begin(&mut self, agent_count: usize) {
        self.entries.clear();
        self.start.clear();
        self.len.clear();
        self.start.resize(agent_count, 0);
        self.len.resize(agent_count, 0);
    }

    /// Record `neighbors` as belonging to `slot_owner`.
    ///
    /// Must be called at most once per agent per tick, in ascending slot
    /// order, which the perceive phase guarantees.
    pub fn push(&mut self, slot_owner: usize, neighbors: &[Neighbor]) {
        self.start[slot_owner] = self.entries.len() as u32;
        self.len[slot_owner] = neighbors.len() as u32;
        self.entries.extend_from_slice(neighbors);
    }

    pub fn neighbors(&self, slot: usize) -> &[Neighbor] {
        let start = self.start[slot] as usize;
        let len = self.len[slot] as usize;
        &self.entries[start..start + len]
    }

    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}
```

- [ ] **Step 4: Run arena tests**

Run: `cargo test -p crowd-core arena` (after adding `pub mod arena;` to `lib.rs`)
Expected: PASS, 3 tests.

- [ ] **Step 5: Write the failing test for the grid**

Create `crates/crowd-core/src/grid.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Segment;
    use crate::units::{Aabb, Vec2};

    fn test_bounds() -> Aabb {
        Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0))
    }

    #[test]
    fn query_finds_a_point_at_the_query_centre() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[5.0], &[5.0]);
        let mut out = Vec::new();
        grid.query(Vec2::new(5.0, 5.0), 0.5, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn query_excludes_points_far_outside_the_radius() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[1.0, 9.0], &[1.0, 9.0]);
        let mut out = Vec::new();
        grid.query(Vec2::new(1.0, 1.0), 0.5, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn query_clears_the_output_buffer_first() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[5.0], &[5.0]);
        let mut out = vec![999, 998];
        grid.query(Vec2::new(5.0, 5.0), 0.5, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn positions_outside_bounds_are_clamped_not_dropped() {
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&[-50.0], &[-50.0]);
        let mut out = Vec::new();
        grid.query(Vec2::new(0.0, 0.0), 1.0, &mut out);
        assert_eq!(out, vec![0], "an escaped agent must remain findable");
    }

    #[test]
    fn broad_phase_is_a_superset_of_brute_force() {
        let xs: Vec<f32> = (0..200).map(|i| (i % 20) as f32 * 0.5).collect();
        let ys: Vec<f32> = (0..200).map(|i| (i / 20) as f32 * 0.5).collect();
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&xs, &ys);

        let centre = Vec2::new(4.0, 4.0);
        let radius = 2.0;
        let mut out = Vec::new();
        grid.query(centre, radius, &mut out);

        for i in 0..xs.len() {
            let d = Vec2::new(xs[i], ys[i]).distance_squared(centre);
            if d <= radius * radius {
                assert!(out.contains(&(i as u32)), "grid missed slot {i}");
            }
        }
    }

    #[test]
    fn rebuild_produces_ascending_slot_order_within_a_query() {
        let xs: Vec<f32> = vec![5.0; 50];
        let ys: Vec<f32> = vec![5.0; 50];
        let mut grid = UniformGrid::new(test_bounds(), 1.0);
        grid.rebuild(&xs, &ys);
        let mut out = Vec::new();
        grid.query(Vec2::new(5.0, 5.0), 0.5, &mut out);
        let mut sorted = out.clone();
        sorted.sort_unstable();
        assert_eq!(out, sorted, "counting sort must preserve slot order");
    }

    #[test]
    fn rebuild_is_deterministic_across_runs() {
        let xs: Vec<f32> = (0..500).map(|i| (i % 37) as f32 * 0.25).collect();
        let ys: Vec<f32> = (0..500).map(|i| (i % 23) as f32 * 0.4).collect();
        let mut a = UniformGrid::new(test_bounds(), 1.0);
        let mut b = UniformGrid::new(test_bounds(), 1.0);
        a.rebuild(&xs, &ys);
        b.rebuild(&xs, &ys);
        let (mut oa, mut ob) = (Vec::new(), Vec::new());
        a.query(Vec2::new(3.0, 3.0), 2.0, &mut oa);
        b.query(Vec2::new(3.0, 3.0), 2.0, &mut ob);
        assert_eq!(oa, ob);
    }

    #[test]
    fn segment_index_finds_a_nearby_wall() {
        let walls = vec![
            Segment::new(Vec2::new(2.0, 0.0), Vec2::new(2.0, 10.0)),
            Segment::new(Vec2::new(9.0, 0.0), Vec2::new(9.0, 10.0)),
        ];
        let index = SegmentIndex::build(test_bounds(), 1.0, &walls);
        let mut out = Vec::new();
        index.query(Vec2::new(2.5, 5.0), 1.0, &mut out);
        assert!(out.contains(&0));
        assert!(!out.contains(&1));
    }

    #[test]
    fn segment_index_returns_each_wall_at_most_once() {
        let walls = vec![Segment::new(Vec2::new(0.0, 5.0), Vec2::new(10.0, 5.0))];
        let index = SegmentIndex::build(test_bounds(), 1.0, &walls);
        let mut out = Vec::new();
        index.query(Vec2::new(5.0, 5.0), 4.0, &mut out);
        assert_eq!(out, vec![0]);
    }
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cargo test -p crowd-core grid`
Expected: FAIL — `cannot find type UniformGrid in this scope`.

- [ ] **Step 7: Write the grid implementation**

Prepend to `crates/crowd-core/src/grid.rs`:

```rust
//! Uniform grid spatial index.
//!
//! Rebuilt every tick by counting sort. Counting sort is chosen deliberately:
//! it is O(n), allocates into reused buffers, and — unlike a hash map —
//! produces one canonical ordering, so neighbor lists never depend on
//! insertion history.

use crate::geometry::Segment;
use crate::units::{Aabb, Vec2};

/// Agent broad-phase index, rebuilt each tick.
#[derive(Clone, Debug)]
pub struct UniformGrid {
    bounds: Aabb,
    cell_size: f32,
    inv_cell_size: f32,
    cols: u32,
    rows: u32,
    /// Prefix sums, length `cols * rows + 1`.
    cell_start: Vec<u32>,
    /// Agent slots grouped by cell, ascending within each cell.
    items: Vec<u32>,
    counts: Vec<u32>,
}

impl UniformGrid {
    pub fn new(bounds: Aabb, cell_size: f32) -> Self {
        assert!(cell_size > 0.0, "cell_size must be positive");
        let size = bounds.size();
        let cols = ((size.x / cell_size).ceil() as u32).max(1);
        let rows = ((size.y / cell_size).ceil() as u32).max(1);
        Self {
            bounds,
            cell_size,
            inv_cell_size: 1.0 / cell_size,
            cols,
            rows,
            cell_start: vec![0; (cols * rows + 1) as usize],
            items: Vec::new(),
            counts: vec![0; (cols * rows) as usize],
        }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Cell coordinates for a world position, clamped to the grid.
    ///
    /// Clamping rather than rejecting matters: an agent that escapes the
    /// bounds through a solver failure must stay findable, or it becomes
    /// invisible to every other agent and the failure compounds silently.
    fn cell_of(&self, x: f32, y: f32) -> (u32, u32) {
        let cx = ((x - self.bounds.min.x) * self.inv_cell_size).floor();
        let cy = ((y - self.bounds.min.y) * self.inv_cell_size).floor();
        let cx = if cx.is_finite() { cx } else { 0.0 };
        let cy = if cy.is_finite() { cy } else { 0.0 };
        (
            (cx.max(0.0) as u32).min(self.cols - 1),
            (cy.max(0.0) as u32).min(self.rows - 1),
        )
    }

    fn cell_index(&self, cx: u32, cy: u32) -> usize {
        (cy * self.cols + cx) as usize
    }

    pub fn rebuild(&mut self, pos_x: &[f32], pos_y: &[f32]) {
        debug_assert_eq!(pos_x.len(), pos_y.len());
        let n = pos_x.len();

        self.counts.iter_mut().for_each(|c| *c = 0);
        for i in 0..n {
            let (cx, cy) = self.cell_of(pos_x[i], pos_y[i]);
            self.counts[self.cell_index(cx, cy)] += 1;
        }

        let mut running = 0u32;
        for (cell, count) in self.counts.iter().enumerate() {
            self.cell_start[cell] = running;
            running += count;
        }
        *self.cell_start.last_mut().expect("non-empty prefix sums") = running;

        // Second pass fills each cell in ascending slot order, because slots
        // are visited in ascending order and each cell's cursor advances
        // monotonically. That ordering is what makes queries reproducible.
        self.items.clear();
        self.items.resize(n, 0);
        let mut cursor: Vec<u32> = self.cell_start[..self.counts.len()].to_vec();
        for i in 0..n {
            let (cx, cy) = self.cell_of(pos_x[i], pos_y[i]);
            let cell = self.cell_index(cx, cy);
            self.items[cursor[cell] as usize] = i as u32;
            cursor[cell] += 1;
        }
    }

    /// Broad-phase query: append every slot in a cell overlapping the circle.
    ///
    /// Callers must still filter by exact distance.
    pub fn query(&self, center: Vec2, radius: f32, out: &mut Vec<u32>) {
        out.clear();
        let (min_cx, min_cy) = self.cell_of(center.x - radius, center.y - radius);
        let (max_cx, max_cy) = self.cell_of(center.x + radius, center.y + radius);
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                let cell = self.cell_index(cx, cy);
                let start = self.cell_start[cell] as usize;
                let end = self.cell_start[cell + 1] as usize;
                out.extend_from_slice(&self.items[start..end]);
            }
        }
    }
}

/// Static wall broad-phase, built once at scene compile time.
#[derive(Clone, Debug)]
pub struct SegmentIndex {
    grid: UniformGrid,
    cell_items: Vec<Vec<u32>>,
}

impl SegmentIndex {
    pub fn build(bounds: Aabb, cell_size: f32, segments: &[Segment]) -> Self {
        let grid = UniformGrid::new(bounds, cell_size);
        let mut cell_items = vec![Vec::new(); (grid.cols * grid.rows) as usize];
        for (index, seg) in segments.iter().enumerate() {
            let b = seg.bounds();
            let (min_cx, min_cy) = grid.cell_of(b.min.x, b.min.y);
            let (max_cx, max_cy) = grid.cell_of(b.max.x, b.max.y);
            for cy in min_cy..=max_cy {
                for cx in min_cx..=max_cx {
                    cell_items[grid.cell_index(cx, cy)].push(index as u32);
                }
            }
        }
        Self { grid, cell_items }
    }

    /// Append the indices of walls near `center`, each at most once.
    pub fn query(&self, center: Vec2, radius: f32, out: &mut Vec<u32>) {
        out.clear();
        let (min_cx, min_cy) = self.grid.cell_of(center.x - radius, center.y - radius);
        let (max_cx, max_cy) = self.grid.cell_of(center.x + radius, center.y + radius);
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                out.extend_from_slice(&self.cell_items[self.grid.cell_index(cx, cy)]);
            }
        }
        // A long wall spans many cells, so dedupe. Sorting first keeps the
        // result in ascending index order, which callers rely on.
        out.sort_unstable();
        out.dedup();
    }
}
```

- [ ] **Step 8: Declare the modules**

In `crates/crowd-core/src/lib.rs` add `pub mod arena;` and `pub mod grid;`, plus `pub use arena::{Neighbor, NeighborArena};` and `pub use grid::{SegmentIndex, UniformGrid};`.

- [ ] **Step 9: Run tests to verify they pass**

Run: `cargo test -p crowd-core`
Expected: PASS, all tests including 9 grid and 3 arena tests.

- [ ] **Step 10: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/grid.rs crates/crowd-core/src/arena.rs crates/crowd-core/src/lib.rs
git commit -m "Add counting-sort uniform grid, segment index, and neighbor arena"
```

---

## Task 9: Waypoint routing

This module is the deliberate stand-in for the deferred navmesh. Its whole value is the interface: `next_target` is exactly the operation a polygon corridor will later implement.

**Files:**
- Create: `crates/crowd-core/src/route.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `crowd_core::units::Vec2`, `crowd_core::world::{RouteHandle, NO_ROUTE}`.
- Produces: `WaypointGraph` with `new()`, `add_node(&mut self, p: Vec2) -> u32`, `add_edge(&mut self, a: u32, b: u32)`, `node_count(&self)`, `position(&self, node: u32) -> Vec2`, `nearest_node(&self, p: Vec2) -> Option<u32>`, `shortest_path(&self, from: u32, to: u32) -> Option<Vec<u32>>`, `is_connected(&self) -> bool`. `RouteArena` with `new()`, `push_route(&mut self, points: &[Vec2]) -> RouteHandle`, `points(&self, handle: RouteHandle) -> &[Vec2]`, `len(&self)`. Free function `next_target(points: &[Vec2], index: &mut u16, pos: Vec2, arrive_radius: f32) -> Option<Vec2>`.

`next_target` returns `None` once the agent has consumed the final waypoint, which the decide phase reads as arrival.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/route.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    /// 0 -- 1 -- 2  with a detour 0 -- 3 -- 2 that is longer.
    fn diamond() -> WaypointGraph {
        let mut g = WaypointGraph::new();
        let n0 = g.add_node(Vec2::new(0.0, 0.0));
        let n1 = g.add_node(Vec2::new(1.0, 0.0));
        let n2 = g.add_node(Vec2::new(2.0, 0.0));
        let n3 = g.add_node(Vec2::new(1.0, 5.0));
        g.add_edge(n0, n1);
        g.add_edge(n1, n2);
        g.add_edge(n0, n3);
        g.add_edge(n3, n2);
        g
    }

    #[test]
    fn shortest_path_prefers_the_shorter_route() {
        assert_eq!(diamond().shortest_path(0, 2), Some(vec![0, 1, 2]));
    }

    #[test]
    fn shortest_path_to_self_is_a_single_node() {
        assert_eq!(diamond().shortest_path(1, 1), Some(vec![1]));
    }

    #[test]
    fn shortest_path_returns_none_when_unreachable() {
        let mut g = diamond();
        let isolated = g.add_node(Vec2::new(99.0, 99.0));
        assert_eq!(g.shortest_path(0, isolated), None);
    }

    #[test]
    fn nearest_node_picks_the_closest_and_breaks_ties_by_index() {
        let g = diamond();
        assert_eq!(g.nearest_node(Vec2::new(1.9, 0.0)), Some(2));
        // Equidistant from nodes 0 and 1: lower index wins.
        assert_eq!(g.nearest_node(Vec2::new(0.5, 0.0)), Some(0));
    }

    #[test]
    fn is_connected_detects_an_isolated_node() {
        assert!(diamond().is_connected());
        let mut g = diamond();
        g.add_node(Vec2::new(99.0, 99.0));
        assert!(!g.is_connected());
    }

    #[test]
    fn empty_graph_has_no_nearest_node() {
        assert_eq!(WaypointGraph::new().nearest_node(Vec2::ZERO), None);
    }

    #[test]
    fn route_arena_round_trips_points() {
        let mut arena = RouteArena::new();
        let a = arena.push_route(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)]);
        let b = arena.push_route(&[Vec2::new(5.0, 5.0)]);
        assert_eq!(arena.points(a).len(), 2);
        assert_eq!(arena.points(b), &[Vec2::new(5.0, 5.0)]);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn no_route_handle_yields_no_points() {
        let arena = RouteArena::new();
        assert!(arena.points(NO_ROUTE).is_empty());
    }

    #[test]
    fn next_target_returns_the_current_waypoint_when_far_away() {
        let points = [Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut index = 0;
        assert_eq!(
            next_target(&points, &mut index, Vec2::ZERO, 0.5),
            Some(Vec2::new(10.0, 0.0))
        );
        assert_eq!(index, 0);
    }

    #[test]
    fn next_target_advances_when_the_waypoint_is_reached() {
        let points = [Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(10.1, 0.0), 0.5);
        assert_eq!(target, Some(Vec2::new(20.0, 0.0)));
        assert_eq!(index, 1);
    }

    #[test]
    fn next_target_skips_multiple_waypoints_in_one_call() {
        let points = [
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(9.0, 0.0),
        ];
        let mut index = 0;
        let target = next_target(&points, &mut index, Vec2::new(2.0, 0.0), 0.5);
        assert_eq!(target, Some(Vec2::new(9.0, 0.0)));
        assert_eq!(index, 2);
    }

    #[test]
    fn next_target_reports_none_after_the_final_waypoint() {
        let points = [Vec2::new(1.0, 0.0)];
        let mut index = 0;
        assert_eq!(next_target(&points, &mut index, Vec2::new(1.0, 0.0), 0.5), None);
    }

    #[test]
    fn next_target_on_an_empty_route_is_none() {
        let mut index = 0;
        assert_eq!(next_target(&[], &mut index, Vec2::ZERO, 0.5), None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core route`
Expected: FAIL — `cannot find type WaypointGraph in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/route.rs`:

```rust
//! Authored waypoint routing — a deliberate stand-in for the tiled navmesh.
//!
//! With analytic walls, straight-line steering to a goal deadlocks in the
//! corner beside a doorway, so agents need a global route before the navmesh
//! exists (contract section 6.1).
//!
//! The point is the interface, not the implementation. A route exposes exactly
//! one operation — given my position, what is the next steering target? — which
//! is precisely what a navmesh polygon corridor will implement. When real
//! navigation lands, it replaces this module and touches no agent state.

use crate::units::Vec2;
use crate::world::{RouteHandle, NO_ROUTE};

/// A small hand-authored navigation graph.
#[derive(Clone, Debug, Default)]
pub struct WaypointGraph {
    nodes: Vec<Vec2>,
    /// Adjacency, each inner list kept sorted so traversal order is fixed.
    adjacency: Vec<Vec<u32>>,
}

impl WaypointGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, p: Vec2) -> u32 {
        self.nodes.push(p);
        self.adjacency.push(Vec::new());
        (self.nodes.len() - 1) as u32
    }

    /// Add an undirected edge. Ignores duplicates and self-loops.
    pub fn add_edge(&mut self, a: u32, b: u32) {
        if a == b {
            return;
        }
        for (from, to) in [(a, b), (b, a)] {
            let list = &mut self.adjacency[from as usize];
            if let Err(insert_at) = list.binary_search(&to) {
                list.insert(insert_at, to);
            }
        }
    }

    pub fn node_count(&self) -> u32 {
        self.nodes.len() as u32
    }

    pub fn position(&self, node: u32) -> Vec2 {
        self.nodes[node as usize]
    }

    /// The nearest node to `p`, breaking exact ties by lower node index.
    pub fn nearest_node(&self, p: Vec2) -> Option<u32> {
        let mut best: Option<(f32, u32)> = None;
        for (index, node) in self.nodes.iter().enumerate() {
            let d = node.distance_squared(p);
            // Strict `<` means the first (lowest-index) node wins a tie.
            if best.is_none_or(|(best_d, _)| d < best_d) {
                best = Some((d, index as u32));
            }
        }
        best.map(|(_, index)| index)
    }

    /// Dijkstra over Euclidean edge lengths.
    ///
    /// Deliberately the O(V^2) scan rather than a binary heap: these graphs
    /// have tens of nodes, and a heap would need a total order over `f32`
    /// costs, which is exactly the kind of subtle ordering dependency the
    /// determinism contract forbids. The linear scan breaks ties by node
    /// index, which is unambiguous.
    pub fn shortest_path(&self, from: u32, to: u32) -> Option<Vec<u32>> {
        let n = self.nodes.len();
        if from as usize >= n || to as usize >= n {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }

        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![u32::MAX; n];
        let mut visited = vec![false; n];
        dist[from as usize] = 0.0;

        loop {
            let mut current: Option<usize> = None;
            for i in 0..n {
                if visited[i] || !dist[i].is_finite() {
                    continue;
                }
                if current.is_none_or(|c| dist[i] < dist[c]) {
                    current = Some(i);
                }
            }
            let Some(current) = current else { break };
            if current == to as usize {
                break;
            }
            visited[current] = true;

            for &next in &self.adjacency[current] {
                let next = next as usize;
                if visited[next] {
                    continue;
                }
                let step = self.nodes[current].distance_squared(self.nodes[next]).sqrt();
                let candidate = dist[current] + step;
                if candidate < dist[next] {
                    dist[next] = candidate;
                    prev[next] = current as u32;
                }
            }
        }

        if !dist[to as usize].is_finite() {
            return None;
        }

        let mut path = vec![to];
        let mut cursor = to;
        while cursor != from {
            cursor = prev[cursor as usize];
            debug_assert_ne!(cursor, u32::MAX, "reachable node must have a predecessor");
            path.push(cursor);
        }
        path.reverse();
        Some(path)
    }

    /// True when every node is reachable from node 0.
    ///
    /// Scene compilation rejects disconnected graphs, because an agent routed
    /// into an isolated component would stall forever with no diagnostic.
    pub fn is_connected(&self) -> bool {
        if self.nodes.is_empty() {
            return true;
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(current) = stack.pop() {
            for &next in &self.adjacency[current] {
                if !seen[next as usize] {
                    seen[next as usize] = true;
                    stack.push(next as usize);
                }
            }
        }
        seen.iter().all(|s| *s)
    }
}

/// Pooled storage for resolved routes.
#[derive(Clone, Debug, Default)]
pub struct RouteArena {
    points: Vec<Vec2>,
    start: Vec<u32>,
    len: Vec<u32>,
}

impl RouteArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_route(&mut self, points: &[Vec2]) -> RouteHandle {
        let handle = RouteHandle(self.start.len() as u32);
        self.start.push(self.points.len() as u32);
        self.len.push(points.len() as u32);
        self.points.extend_from_slice(points);
        handle
    }

    pub fn points(&self, handle: RouteHandle) -> &[Vec2] {
        if handle == NO_ROUTE || handle.0 as usize >= self.start.len() {
            return &[];
        }
        let start = self.start[handle.0 as usize] as usize;
        let len = self.len[handle.0 as usize] as usize;
        &self.points[start..start + len]
    }

    pub fn len(&self) -> usize {
        self.start.len()
    }

    pub fn is_empty(&self) -> bool {
        self.start.is_empty()
    }
}

/// The next steering target along a route, advancing `index` past any
/// waypoints already reached.
///
/// Returns `None` once the final waypoint is consumed, which the decide phase
/// reads as arrival. This signature is the contract a navmesh corridor will
/// inherit.
pub fn next_target(
    points: &[Vec2],
    index: &mut u16,
    pos: Vec2,
    arrive_radius: f32,
) -> Option<Vec2> {
    let arrive_sq = arrive_radius * arrive_radius;
    while (*index as usize) < points.len() {
        let target = points[*index as usize];
        if target.distance_squared(pos) > arrive_sq {
            return Some(target);
        }
        *index += 1;
    }
    None
}
```

`is_none_or` requires Rust 1.82+; the pinned 1.94.1 has it.

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod route;` and `pub use route::{next_target, RouteArena, WaypointGraph};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core route`
Expected: PASS, 13 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/route.rs crates/crowd-core/src/lib.rs
git commit -m "Add waypoint graph routing as the navmesh stand-in"
```

---

## Task 10: Scene definition and compilation

**Files:**
- Create: `crates/crowd-core/src/scene.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `Aabb`, `Vec2`, `Segment`, `WaypointGraph`, `SegmentIndex`, `hash_str`.
- Produces: `Destination { name: String, node: u32 }`. `SpawnRegion { id: u16, population_id: u16, area: Aabb, count: u32, per_tick: u32, destination: u16 }`. `PopulationParams { radius_min, radius_max, speed_mean, speed_stddev, max_speed_factor }` with `Default`. `SceneDef` with public fields and `compile(self) -> Result<CompiledScene, Vec<SceneError>>`. `SceneError` enum. `CompiledScene` with `scene_hash(&self) -> u64` and public accessors.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/scene.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    /// A corridor with two nodes, one spawn at the left, one exit at the right.
    fn valid_scene() -> SceneDef {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);

        SceneDef {
            name: "corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
                Segment::new(Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0)),
            ],
            waypoints,
            destinations: vec![Destination { name: "exit".into(), node: b }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count: 10,
                per_tick: 2,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 300,
        }
    }

    #[test]
    fn a_valid_scene_compiles() {
        assert!(valid_scene().compile().is_ok());
    }

    #[test]
    fn compiled_scene_reports_total_agent_count() {
        let compiled = valid_scene().compile().unwrap();
        assert_eq!(compiled.total_agents(), 10);
    }

    #[test]
    fn spawn_outside_bounds_is_rejected() {
        let mut scene = valid_scene();
        scene.spawns[0].area = Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(-40.0, -40.0));
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::SpawnOutsideBounds { spawn: 0 }));
    }

    #[test]
    fn unknown_destination_reference_is_rejected() {
        let mut scene = valid_scene();
        scene.spawns[0].destination = 7;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnknownDestination { spawn: 0, destination: 7 }));
    }

    #[test]
    fn disconnected_waypoint_graph_is_rejected() {
        let mut scene = valid_scene();
        scene.waypoints.add_node(Vec2::new(5.0, 9.0));
        let errors = scene.compile().unwrap_err();
        assert!(matches!(
            errors.as_slice(),
            [SceneError::DisconnectedWaypointGraph]
        ));
    }

    #[test]
    fn empty_waypoint_graph_is_rejected() {
        let mut scene = valid_scene();
        scene.waypoints = WaypointGraph::new();
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::EmptyWaypointGraph));
    }

    #[test]
    fn destination_node_outside_the_graph_is_rejected() {
        let mut scene = valid_scene();
        scene.destinations[0].node = 99;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::DestinationNodeMissing {
            destination: 0,
            node: 99
        }));
    }

    #[test]
    fn missing_population_reference_is_rejected() {
        let mut scene = valid_scene();
        scene.spawns[0].population_id = 5;
        let errors = scene.compile().unwrap_err();
        assert!(errors.contains(&SceneError::UnknownPopulation { spawn: 0, population: 5 }));
    }

    #[test]
    fn all_independent_errors_are_reported_together() {
        // A user fixing one problem at a time is a bad experience; the
        // compiler reports every independent fault in one pass.
        let mut scene = valid_scene();
        scene.spawns[0].destination = 7;
        scene.spawns[0].population_id = 5;
        let errors = scene.compile().unwrap_err();
        assert!(errors.len() >= 2, "got {errors:?}");
    }

    #[test]
    fn scene_hash_is_stable_for_identical_input() {
        let a = valid_scene().compile().unwrap();
        let b = valid_scene().compile().unwrap();
        assert_eq!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_changes_when_geometry_changes() {
        let a = valid_scene().compile().unwrap();
        let mut scene = valid_scene();
        scene.walls.push(Segment::new(Vec2::new(5.0, 0.0), Vec2::new(5.0, 4.0)));
        let b = scene.compile().unwrap();
        assert_ne!(a.scene_hash(), b.scene_hash());
    }

    #[test]
    fn scene_hash_changes_when_the_seed_changes() {
        let a = valid_scene().compile().unwrap();
        let mut scene = valid_scene();
        scene.project_seed = 43;
        let b = scene.compile().unwrap();
        assert_ne!(a.scene_hash(), b.scene_hash());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core scene`
Expected: FAIL — `cannot find type SceneDef in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/scene.rs`:

```rust
//! Scene authoring input and its compiled, validated form.
//!
//! Compilation is where contract section 10.3's error model lives: every
//! diagnostic names the offending entity and every independent fault is
//! reported in one pass, so a user is not forced to fix problems one at a
//! time.

use crate::geometry::Segment;
use crate::grid::SegmentIndex;
use crate::ids::{hash_combine, hash_str, mix64};
use crate::route::WaypointGraph;
use crate::units::{Aabb, Vec2};

/// A named goal region, anchored to a waypoint node.
#[derive(Clone, Debug)]
pub struct Destination {
    pub name: String,
    pub node: u32,
}

/// Where and how fast agents enter the scene.
#[derive(Clone, Copy, Debug)]
pub struct SpawnRegion {
    pub id: u16,
    pub population_id: u16,
    pub area: Aabb,
    /// Total agents this region will ever emit.
    pub count: u32,
    /// Agents emitted per tick until `count` is exhausted.
    pub per_tick: u32,
    /// Index into `SceneDef::destinations`.
    pub destination: u16,
}

/// Distributions an agent's varied attributes are drawn from.
#[derive(Clone, Copy, Debug)]
pub struct PopulationParams {
    pub radius_min: f32,
    pub radius_max: f32,
    pub speed_mean: f32,
    pub speed_stddev: f32,
    /// Maximum speed as a multiple of the agent's preferred speed.
    pub max_speed_factor: f32,
}

impl Default for PopulationParams {
    /// Pedestrian defaults from contract section 4.2.
    fn default() -> Self {
        Self {
            radius_min: 0.24,
            radius_max: 0.38,
            speed_mean: 1.35,
            speed_stddev: 0.18,
            max_speed_factor: 1.5,
        }
    }
}

/// Authoring input for one benchmark scene.
#[derive(Clone, Debug)]
pub struct SceneDef {
    pub name: String,
    pub bounds: Aabb,
    pub walls: Vec<Segment>,
    pub waypoints: WaypointGraph,
    pub destinations: Vec<Destination>,
    pub spawns: Vec<SpawnRegion>,
    pub populations: Vec<PopulationParams>,
    pub project_seed: u64,
    pub ticks_per_second: u32,
    pub duration_ticks: u64,
}

/// A bake-blocking authoring fault. Each names the entity at fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    EmptyWaypointGraph,
    DisconnectedWaypointGraph,
    NoDestinations,
    NoSpawns,
    SpawnOutsideBounds { spawn: u16 },
    UnknownDestination { spawn: u16, destination: u16 },
    UnknownPopulation { spawn: u16, population: u16 },
    DestinationNodeMissing { destination: u16, node: u32 },
    UnreachableDestination { spawn: u16, destination: u16 },
    InvalidTickRate { ticks_per_second: u32 },
}

/// A validated scene, ready to simulate.
#[derive(Clone, Debug)]
pub struct CompiledScene {
    pub name: String,
    pub bounds: Aabb,
    pub walls: Vec<Segment>,
    pub wall_index: SegmentIndex,
    pub waypoints: WaypointGraph,
    pub destinations: Vec<Destination>,
    pub spawns: Vec<SpawnRegion>,
    pub populations: Vec<PopulationParams>,
    pub project_seed: u64,
    pub ticks_per_second: u32,
    pub duration_ticks: u64,
    scene_hash: u64,
}

/// Wall index cell size. Chosen as a few agent diameters: small enough that a
/// query touches few cells, large enough that long walls do not fan out.
const WALL_CELL_SIZE: f32 = 2.0;

impl SceneDef {
    /// Validate and compile, reporting every independent fault at once.
    pub fn compile(self) -> Result<CompiledScene, Vec<SceneError>> {
        let mut errors = Vec::new();

        if self.ticks_per_second == 0 {
            errors.push(SceneError::InvalidTickRate {
                ticks_per_second: self.ticks_per_second,
            });
        }
        if self.waypoints.node_count() == 0 {
            errors.push(SceneError::EmptyWaypointGraph);
        } else if !self.waypoints.is_connected() {
            errors.push(SceneError::DisconnectedWaypointGraph);
        }
        if self.destinations.is_empty() {
            errors.push(SceneError::NoDestinations);
        }
        if self.spawns.is_empty() {
            errors.push(SceneError::NoSpawns);
        }

        for (index, destination) in self.destinations.iter().enumerate() {
            if destination.node >= self.waypoints.node_count() {
                errors.push(SceneError::DestinationNodeMissing {
                    destination: index as u16,
                    node: destination.node,
                });
            }
        }

        for spawn in &self.spawns {
            if !self.bounds.contains(spawn.area.min) || !self.bounds.contains(spawn.area.max) {
                errors.push(SceneError::SpawnOutsideBounds { spawn: spawn.id });
            }
            if spawn.population_id as usize >= self.populations.len() {
                errors.push(SceneError::UnknownPopulation {
                    spawn: spawn.id,
                    population: spawn.population_id,
                });
            }
            let Some(destination) = self.destinations.get(spawn.destination as usize) else {
                errors.push(SceneError::UnknownDestination {
                    spawn: spawn.id,
                    destination: spawn.destination,
                });
                continue;
            };
            // Reachability is only meaningful once the graph itself is sound.
            if self.waypoints.node_count() > 0
                && destination.node < self.waypoints.node_count()
            {
                let from = self
                    .waypoints
                    .nearest_node(spawn.area.center())
                    .expect("non-empty graph has a nearest node");
                if self.waypoints.shortest_path(from, destination.node).is_none() {
                    errors.push(SceneError::UnreachableDestination {
                        spawn: spawn.id,
                        destination: spawn.destination,
                    });
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let scene_hash = compute_scene_hash(&self);
        let wall_index = SegmentIndex::build(
            self.bounds.expanded(WALL_CELL_SIZE),
            WALL_CELL_SIZE,
            &self.walls,
        );

        Ok(CompiledScene {
            name: self.name,
            bounds: self.bounds,
            walls: self.walls,
            wall_index,
            waypoints: self.waypoints,
            destinations: self.destinations,
            spawns: self.spawns,
            populations: self.populations,
            project_seed: self.project_seed,
            ticks_per_second: self.ticks_per_second,
            duration_ticks: self.duration_ticks,
            scene_hash,
        })
    }
}

/// Content hash over everything that affects simulation output.
///
/// Reports carry this so a metrics comparison against a baseline generated
/// from a different scene is detectable rather than silently misleading.
fn compute_scene_hash(scene: &SceneDef) -> u64 {
    let mut h = hash_str(&scene.name);
    h = hash_combine(h, scene.project_seed);
    h = hash_combine(h, scene.ticks_per_second as u64);
    h = hash_combine(h, scene.duration_ticks);
    for value in [
        scene.bounds.min.x,
        scene.bounds.min.y,
        scene.bounds.max.x,
        scene.bounds.max.y,
    ] {
        h = hash_combine(h, value.to_bits() as u64);
    }
    for wall in &scene.walls {
        for value in [wall.a.x, wall.a.y, wall.b.x, wall.b.y] {
            h = hash_combine(h, value.to_bits() as u64);
        }
    }
    for node in 0..scene.waypoints.node_count() {
        let p = scene.waypoints.position(node);
        h = hash_combine(h, p.x.to_bits() as u64);
        h = hash_combine(h, p.y.to_bits() as u64);
    }
    for destination in &scene.destinations {
        h = hash_combine(h, hash_str(&destination.name));
        h = hash_combine(h, destination.node as u64);
    }
    for spawn in &scene.spawns {
        h = hash_combine(h, spawn.id as u64);
        h = hash_combine(h, spawn.population_id as u64);
        h = hash_combine(h, spawn.count as u64);
        h = hash_combine(h, spawn.per_tick as u64);
        h = hash_combine(h, spawn.destination as u64);
        for value in [
            spawn.area.min.x,
            spawn.area.min.y,
            spawn.area.max.x,
            spawn.area.max.y,
        ] {
            h = hash_combine(h, value.to_bits() as u64);
        }
    }
    for population in &scene.populations {
        for value in [
            population.radius_min,
            population.radius_max,
            population.speed_mean,
            population.speed_stddev,
            population.max_speed_factor,
        ] {
            h = hash_combine(h, value.to_bits() as u64);
        }
    }
    mix64(h)
}

impl CompiledScene {
    pub fn scene_hash(&self) -> u64 {
        self.scene_hash
    }

    pub fn total_agents(&self) -> u32 {
        self.spawns.iter().map(|s| s.count).sum()
    }

    pub fn destination_position(&self, destination: u16) -> Vec2 {
        self.waypoints
            .position(self.destinations[destination as usize].node)
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod scene;` and `pub use scene::{CompiledScene, Destination, PopulationParams, SceneDef, SceneError, SpawnRegion};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core scene`
Expected: PASS, 12 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/scene.rs crates/crowd-core/src/lib.rs
git commit -m "Add scene definition, validation diagnostics, and content hash"
```

---

## Task 11: Spawn phase

**Files:**
- Create: `crates/crowd-core/src/phases/mod.rs`
- Create: `crates/crowd-core/src/phases/spawn.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `CompiledScene`, `World`, `AgentSpawn`, `RouteArena`, `StableRng`, `Purpose`, `derive_agent_id`.
- Produces: `SpawnState::new(scene: &CompiledScene) -> SpawnState` with `emitted(&self, spawn_index: usize) -> u32` and `all_emitted(&self, scene: &CompiledScene) -> bool`; free function `apply_spawns(scene: &CompiledScene, state: &mut SpawnState, world: &mut World, routes: &mut RouteArena, tick: u64) -> Vec<SpawnError>`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/phases/spawn.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};

    fn scene(count: u32, per_tick: u32) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "spawn_test".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            walls: Vec::new(),
            waypoints,
            destinations: vec![Destination { name: "exit".into(), node: b }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count,
                per_tick,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 42,
            ticks_per_second: 30,
            duration_ticks: 100,
        }
        .compile()
        .unwrap()
    }

    fn run_ticks(scene: &CompiledScene, ticks: u64) -> (World, RouteArena) {
        let mut world = World::new();
        let mut routes = RouteArena::new();
        let mut state = SpawnState::new(scene);
        for tick in 0..ticks {
            let errors = apply_spawns(scene, &mut state, &mut world, &mut routes, tick);
            assert!(errors.is_empty(), "{errors:?}");
        }
        (world, routes)
    }

    #[test]
    fn spawns_are_rate_limited_per_tick() {
        let scene = scene(10, 3);
        let (world, _) = run_ticks(&scene, 1);
        assert_eq!(world.len(), 3);
    }

    #[test]
    fn spawning_stops_at_the_configured_count() {
        let scene = scene(10, 3);
        let (world, _) = run_ticks(&scene, 100);
        assert_eq!(world.len(), 10);
    }

    #[test]
    fn spawned_agents_land_inside_the_spawn_area() {
        let scene = scene(50, 50);
        let (world, _) = run_ticks(&scene, 1);
        let area = scene.spawns[0].area;
        for slot in 0..world.len() as u32 {
            assert!(area.contains(world.position(slot)), "slot {slot} escaped");
        }
    }

    #[test]
    fn spawned_agents_receive_varied_attributes() {
        let scene = scene(200, 200);
        let (world, _) = run_ticks(&scene, 1);
        let first = world.radius[0];
        assert!(world.radius.iter().any(|r| *r != first), "no radius variation");
        assert!(world.radius.iter().all(|r| (0.24..=0.38).contains(r)));
        assert!(world.preferred_speed.iter().all(|s| *s > 0.0));
        assert!(world
            .max_speed
            .iter()
            .zip(&world.preferred_speed)
            .all(|(m, p)| m >= p));
    }

    #[test]
    fn spawned_agents_receive_a_route_to_their_destination() {
        let scene = scene(5, 5);
        let (world, routes) = run_ticks(&scene, 1);
        for slot in 0..world.len() {
            let points = routes.points(world.route[slot]);
            assert!(!points.is_empty(), "slot {slot} has no route");
            assert_eq!(*points.last().unwrap(), Vec2::new(9.0, 5.0));
        }
    }

    #[test]
    fn agent_attributes_do_not_depend_on_spawn_rate() {
        // The same agent ordinal must get the same attributes whether it was
        // emitted alone or in a burst. This is contract section 4.2 applied to
        // the spawn scheduler.
        let (fast, _) = run_ticks(&scene(10, 10), 10);
        let (slow, _) = run_ticks(&scene(10, 1), 10);
        assert_eq!(fast.agent_id, slow.agent_id);
        assert_eq!(fast.radius, slow.radius);
        assert_eq!(fast.preferred_speed, slow.preferred_speed);
    }

    #[test]
    fn all_emitted_reports_completion() {
        let scene = scene(4, 2);
        let mut world = World::new();
        let mut routes = RouteArena::new();
        let mut state = SpawnState::new(&scene);
        apply_spawns(&scene, &mut state, &mut world, &mut routes, 0);
        assert!(!state.all_emitted(&scene));
        apply_spawns(&scene, &mut state, &mut world, &mut routes, 1);
        assert!(state.all_emitted(&scene));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core spawn`
Expected: FAIL — `cannot find type SpawnState in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/phases/spawn.rs`:

```rust
//! Tick phase 1: apply inputs.
//!
//! The only timed input this slice has is scheduled agent spawns. Every varied
//! attribute is drawn from a stream keyed by the agent's stable ID, never from
//! a shared sequence, so emission rate and ordering cannot change what any
//! individual agent looks like.

use crate::ids::derive_agent_id;
use crate::rng::{Purpose, StableRng};
use crate::route::RouteArena;
use crate::scene::CompiledScene;
use crate::units::Vec2;
use crate::world::{AgentSpawn, SpawnError, World, NO_ROUTE};

/// How many agents each spawn region has emitted so far.
#[derive(Clone, Debug, Default)]
pub struct SpawnState {
    emitted: Vec<u32>,
}

impl SpawnState {
    pub fn new(scene: &CompiledScene) -> Self {
        Self {
            emitted: vec![0; scene.spawns.len()],
        }
    }

    pub fn emitted(&self, spawn_index: usize) -> u32 {
        self.emitted[spawn_index]
    }

    pub fn all_emitted(&self, scene: &CompiledScene) -> bool {
        scene
            .spawns
            .iter()
            .zip(&self.emitted)
            .all(|(spawn, emitted)| *emitted >= spawn.count)
    }
}

/// Emit this tick's scheduled agents.
///
/// Returns any duplicate-ID diagnostics rather than panicking, so a caller can
/// surface them as bake-blocking errors per contract section 10.3.
pub fn apply_spawns(
    scene: &CompiledScene,
    state: &mut SpawnState,
    world: &mut World,
    routes: &mut RouteArena,
    tick: u64,
) -> Vec<SpawnError> {
    let mut errors = Vec::new();

    for (spawn_index, region) in scene.spawns.iter().enumerate() {
        let already = state.emitted[spawn_index];
        let remaining = region.count.saturating_sub(already);
        let this_tick = remaining.min(region.per_tick);

        for offset in 0..this_tick {
            let ordinal = already + offset;
            let agent_id = derive_agent_id(
                scene.project_seed,
                region.population_id,
                region.id,
                ordinal,
            );

            let params = &scene.populations[region.population_id as usize];

            let mut radius_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::Radius);
            let radius = radius_rng.range_f32(params.radius_min, params.radius_max);

            let mut speed_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::PreferredSpeed);
            // Clamp keeps a rare tail sample from producing a zero or negative
            // preferred speed, which would make an agent permanently stalled.
            let preferred_speed = speed_rng
                .normal_f32(params.speed_mean, params.speed_stddev)
                .clamp(0.4, params.speed_mean * 2.0);

            let mut position_rng =
                StableRng::for_agent(scene.project_seed, agent_id, Purpose::SpawnPosition);
            let position = Vec2::new(
                position_rng.range_f32(region.area.min.x, region.area.max.x),
                position_rng.range_f32(region.area.min.y, region.area.max.y),
            );

            let destination_node = scene.destinations[region.destination as usize].node;
            let route = match scene.waypoints.nearest_node(position) {
                Some(from) => match scene.waypoints.shortest_path(from, destination_node) {
                    Some(path) => {
                        let points: Vec<Vec2> =
                            path.iter().map(|n| scene.waypoints.position(*n)).collect();
                        routes.push_route(&points)
                    }
                    // Compilation already proved reachability from the region
                    // centre; an individual sample can still fail only if the
                    // graph is malformed, and an unrouted agent is preferable
                    // to a panic mid-bake.
                    None => NO_ROUTE,
                },
                None => NO_ROUTE,
            };

            let heading = (scene.waypoints.position(destination_node) - position)
                .normalize_or_zero();

            let spawn = AgentSpawn {
                agent_id,
                population_id: region.population_id,
                position,
                yaw: heading.to_yaw(),
                radius,
                max_speed: preferred_speed * params.max_speed_factor,
                preferred_speed,
                route,
                destination: region.destination,
            };

            if let Err(error) = world.spawn(spawn, tick) {
                errors.push(error);
            }
        }

        state.emitted[spawn_index] = already + this_tick;
    }

    errors
}
```

- [ ] **Step 4: Create the phases module and declare it**

`crates/crowd-core/src/phases/mod.rs`:

```rust
//! Fixed-order tick phases.
//!
//! Each phase is a free function taking immutable previous-state buffers and
//! mutable next-state buffers, so read and write sets are visible in the
//! signature and a later parallel pass needs no semantic change.

pub mod spawn;

pub use spawn::{apply_spawns, SpawnState};
```

In `crates/crowd-core/src/lib.rs` add `pub mod phases;`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core spawn`
Expected: PASS, 7 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/phases/ crates/crowd-core/src/lib.rs
git commit -m "Add spawn phase with ID-keyed attribute variation"
```

---

## Task 12: Perceive phase

**Files:**
- Create: `crates/crowd-core/src/phases/perceive.rs`
- Modify: `crates/crowd-core/src/phases/mod.rs`

**Interfaces:**
- Consumes: `World`, `UniformGrid`, `NeighborArena`, `Neighbor`.
- Produces: `PerceiveConfig { query_radius: f32, budget: usize }` with `Default` (`query_radius: 5.0`, `budget: 16`); `PerceiveScratch::default()`; free function `perceive(world: &World, grid: &UniformGrid, config: &PerceiveConfig, scratch: &mut PerceiveScratch, arena: &mut NeighborArena)`.

Neighbors are sorted by `(dist_sq, agent_id)` so a budget cutoff is never ambiguous between two equidistant neighbors.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/phases/perceive.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn world_at(points: &[Vec2]) -> World {
        let mut world = World::new();
        for (i, p) in points.iter().enumerate() {
            world
                .spawn(
                    AgentSpawn {
                        agent_id: AgentId(i as u64 + 1),
                        population_id: 0,
                        position: *p,
                        yaw: 0.0,
                        radius: 0.3,
                        max_speed: 1.8,
                        preferred_speed: 1.35,
                        route: NO_ROUTE,
                        destination: 0,
                    },
                    0,
                )
                .unwrap();
        }
        world
    }

    fn perceive_world(world: &World, config: &PerceiveConfig) -> NeighborArena {
        let mut grid = UniformGrid::new(
            Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0)),
            config.query_radius,
        );
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut scratch = PerceiveScratch::default();
        let mut arena = NeighborArena::new();
        perceive(world, &grid, config, &mut scratch, &mut arena);
        arena
    }

    #[test]
    fn an_agent_never_perceives_itself() {
        let world = world_at(&[Vec2::ZERO]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert!(arena.neighbors(0).is_empty());
    }

    #[test]
    fn nearby_agents_are_perceived_reciprocally() {
        let world = world_at(&[Vec2::ZERO, Vec2::new(1.0, 0.0)]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert_eq!(arena.neighbors(0).len(), 1);
        assert_eq!(arena.neighbors(0)[0].slot, 1);
        assert_eq!(arena.neighbors(1)[0].slot, 0);
    }

    #[test]
    fn agents_beyond_the_query_radius_are_excluded() {
        let config = PerceiveConfig { query_radius: 2.0, budget: 16 };
        let world = world_at(&[Vec2::ZERO, Vec2::new(50.0, 0.0)]);
        let arena = perceive_world(&world, &config);
        assert!(arena.neighbors(0).is_empty());
    }

    #[test]
    fn neighbors_are_sorted_nearest_first() {
        let world = world_at(&[
            Vec2::ZERO,
            Vec2::new(3.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
        ]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        let slots: Vec<u32> = arena.neighbors(0).iter().map(|n| n.slot).collect();
        assert_eq!(slots, vec![2, 3, 1]);
    }

    #[test]
    fn the_budget_keeps_only_the_nearest_neighbors() {
        let points: Vec<Vec2> = (0..20).map(|i| Vec2::new(i as f32 * 0.2, 0.0)).collect();
        let world = world_at(&points);
        let config = PerceiveConfig { query_radius: 10.0, budget: 4 };
        let arena = perceive_world(&world, &config);
        assert_eq!(arena.neighbors(0).len(), 4);
        let slots: Vec<u32> = arena.neighbors(0).iter().map(|n| n.slot).collect();
        assert_eq!(slots, vec![1, 2, 3, 4]);
    }

    #[test]
    fn equidistant_neighbors_are_ordered_by_stable_id() {
        // Two neighbors at identical distance: the tie must resolve by agent
        // ID, or a budget cutoff would silently depend on slot layout.
        let world = world_at(&[Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0)]);
        let arena = perceive_world(&world, &PerceiveConfig::default());
        let ids: Vec<u64> = arena
            .neighbors(0)
            .iter()
            .map(|n| world.agent_id[n.slot as usize].0)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn perceiving_an_empty_world_is_a_no_op() {
        let world = World::new();
        let arena = perceive_world(&world, &PerceiveConfig::default());
        assert_eq!(arena.capacity(), 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core perceive`
Expected: FAIL — `cannot find type PerceiveConfig in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/phases/perceive.rs`:

```rust
//! Tick phase 3: perceive.
//!
//! Collects each agent's nearest neighbors under a fixed budget. Sorting by
//! `(distance_squared, agent_id)` matters: with distance alone, two exactly
//! equidistant neighbors would be ordered by slot layout, and a budget cutoff
//! between them would silently depend on spawn history.

use crate::arena::{Neighbor, NeighborArena};
use crate::grid::UniformGrid;
use crate::units::Vec2;
use crate::world::World;

/// Perception limits. Contract section 6 calls these a per-tier budget; this
/// slice has one tier, so they are global.
#[derive(Clone, Copy, Debug)]
pub struct PerceiveConfig {
    pub query_radius: f32,
    pub budget: usize,
}

impl Default for PerceiveConfig {
    fn default() -> Self {
        Self {
            query_radius: 5.0,
            budget: 16,
        }
    }
}

/// Reused buffers, so the phase does not allocate after warmup.
#[derive(Clone, Debug, Default)]
pub struct PerceiveScratch {
    candidates: Vec<u32>,
    accepted: Vec<Neighbor>,
}

pub fn perceive(
    world: &World,
    grid: &UniformGrid,
    config: &PerceiveConfig,
    scratch: &mut PerceiveScratch,
    arena: &mut NeighborArena,
) {
    arena.begin(world.len());
    let radius_sq = config.query_radius * config.query_radius;

    for slot in 0..world.len() {
        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        grid.query(position, config.query_radius, &mut scratch.candidates);

        scratch.accepted.clear();
        for &candidate in &scratch.candidates {
            if candidate as usize == slot {
                continue;
            }
            let other = Vec2::new(
                world.pos_x[candidate as usize],
                world.pos_y[candidate as usize],
            );
            let dist_sq = position.distance_squared(other);
            if dist_sq <= radius_sq {
                scratch.accepted.push(Neighbor {
                    slot: candidate,
                    dist_sq,
                });
            }
        }

        // `total_cmp` gives a total order over floats without NaN ambiguity;
        // the agent-ID tiebreak makes the result independent of slot layout.
        scratch.accepted.sort_unstable_by(|a, b| {
            a.dist_sq
                .total_cmp(&b.dist_sq)
                .then_with(|| world.agent_id[a.slot as usize].cmp(&world.agent_id[b.slot as usize]))
        });
        scratch.accepted.truncate(config.budget);

        arena.push(slot, &scratch.accepted);
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/phases/mod.rs` add `pub mod perceive;` and `pub use perceive::{perceive, PerceiveConfig, PerceiveScratch};`. Add `use crate::grid::UniformGrid;` to the test module's imports if the compiler asks.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core perceive`
Expected: PASS, 7 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/phases/
git commit -m "Add perceive phase with ID-tiebroken neighbor budget"
```

---

## Task 13: Decide phase

Contract phases 4 (decide) and 5 (plan) collapse into one function here, because with a fixed rule and pre-resolved routes there is nothing to separate them. The behavior graph (contract section 4.4) will split them apart later.

**Files:**
- Create: `crates/crowd-core/src/phases/decide.rs`
- Modify: `crates/crowd-core/src/phases/mod.rs`

**Interfaces:**
- Consumes: `World`, `RouteArena`, `next_target`.
- Produces: `DecideConfig { arrive_radius: f32 }` with `Default` (`0.6`); free function `decide(world: &mut World, routes: &RouteArena, config: &DecideConfig)`.

`decide` writes each agent's *preferred* velocity into `des_vel_*`, and sets `arrived` once the route is exhausted. The steer phase then applies avoidance on top.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/phases/decide.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use crate::units::Vec2;
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn world_with_route(position: Vec2, points: &[Vec2]) -> (World, RouteArena) {
        let mut routes = RouteArena::new();
        let route = if points.is_empty() {
            NO_ROUTE
        } else {
            routes.push_route(points)
        };
        let mut world = World::new();
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(1),
                    population_id: 0,
                    position,
                    yaw: 0.0,
                    radius: 0.3,
                    max_speed: 2.0,
                    preferred_speed: 1.5,
                    route,
                    destination: 0,
                },
                0,
            )
            .unwrap();
        (world, routes)
    }

    #[test]
    fn preferred_velocity_points_at_the_next_waypoint() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert!((world.des_vel_x[0] - 1.5).abs() < 1e-5);
        assert!(world.des_vel_y[0].abs() < 1e-5);
    }

    #[test]
    fn preferred_velocity_magnitude_is_the_preferred_speed() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[Vec2::new(3.0, 4.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        let speed = world.desired_velocity(0).length();
        assert!((speed - 1.5).abs() < 1e-5, "speed was {speed}");
    }

    #[test]
    fn reaching_the_final_waypoint_marks_arrival_and_stops_the_agent() {
        let (mut world, routes) = world_with_route(Vec2::new(10.0, 0.0), &[Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert!(world.arrived[0]);
        assert_eq!(world.desired_velocity(0), Vec2::ZERO);
    }

    #[test]
    fn passing_a_waypoint_advances_the_route_index() {
        let (mut world, routes) =
            world_with_route(Vec2::ZERO, &[Vec2::ZERO, Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert_eq!(world.route_index[0], 1);
        assert!(!world.arrived[0]);
    }

    #[test]
    fn an_agent_with_no_route_stops_rather_than_drifting() {
        let (mut world, routes) = world_with_route(Vec2::ZERO, &[]);
        decide(&mut world, &routes, &DecideConfig::default());
        assert_eq!(world.desired_velocity(0), Vec2::ZERO);
        assert!(world.arrived[0]);
    }

    #[test]
    fn an_arrived_agent_stays_arrived() {
        let (mut world, routes) = world_with_route(Vec2::new(10.0, 0.0), &[Vec2::new(10.0, 0.0)]);
        decide(&mut world, &routes, &DecideConfig::default());
        let index_after_arrival = world.route_index[0];
        decide(&mut world, &routes, &DecideConfig::default());
        assert!(world.arrived[0]);
        assert_eq!(world.route_index[0], index_after_arrival);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core decide`
Expected: FAIL — `cannot find type DecideConfig in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/phases/decide.rs`:

```rust
//! Tick phases 4 and 5: decide and plan.
//!
//! Contract section 6 separates these, but with a fixed rule and routes
//! resolved at spawn there is nothing to separate. The behavior graph
//! (contract section 4.4) will split them apart; until then, collapsing them
//! avoids a phase that does nothing.
//!
//! Writes each agent's *preferred* velocity. The steer phase applies avoidance
//! on top, so the two concerns stay independently testable.

use crate::route::{next_target, RouteArena};
use crate::units::Vec2;
use crate::world::World;

#[derive(Clone, Copy, Debug)]
pub struct DecideConfig {
    /// How close counts as having reached a waypoint.
    pub arrive_radius: f32,
}

impl Default for DecideConfig {
    fn default() -> Self {
        Self { arrive_radius: 0.6 }
    }
}

pub fn decide(world: &mut World, routes: &RouteArena, config: &DecideConfig) {
    for slot in 0..world.len() {
        if world.arrived[slot] {
            world.des_vel_x[slot] = 0.0;
            world.des_vel_y[slot] = 0.0;
            continue;
        }

        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        let points = routes.points(world.route[slot]);
        let mut index = world.route_index[slot];
        let target = next_target(points, &mut index, position, config.arrive_radius);
        world.route_index[slot] = index;

        match target {
            Some(target) => {
                let direction = (target - position).normalize_or_zero();
                let preferred = direction * world.preferred_speed[slot];
                world.des_vel_x[slot] = preferred.x;
                world.des_vel_y[slot] = preferred.y;
            }
            None => {
                // Route exhausted, or the agent never had one. Stopping rather
                // than drifting keeps an unrouted agent from wandering into
                // other agents and corrupting the metrics.
                world.arrived[slot] = true;
                world.des_vel_x[slot] = 0.0;
                world.des_vel_y[slot] = 0.0;
            }
        }
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/phases/mod.rs` add `pub mod decide;` and `pub use decide::{decide, DecideConfig};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core decide`
Expected: PASS, 6 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/phases/
git commit -m "Add decide phase producing preferred velocity along routes"
```

---

## Task 14: Avoidance trait and sampled-velocity solver

**Files:**
- Create: `crates/crowd-core/src/avoidance/mod.rs`
- Create: `crates/crowd-core/src/avoidance/sampled.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `AgentId`, `Vec2`, `Segment`, `SolverStatus`, `time_to_collision_disc`, `time_to_collision_segment`.
- Produces: `NeighborState { position: Vec2, velocity: Vec2, radius: f32, agent_id: AgentId }`. `AvoidanceInput<'a> { agent_id, position, velocity, preferred, radius, max_speed, neighbors: &'a [NeighborState], walls: &'a [Segment] }`. `AvoidanceOutput { velocity: Vec2, status: SolverStatus, min_time_to_collision: f32 }`. `trait AvoidanceSolver { fn name(&self) -> &'static str; fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput; }`. `SampledVelocitySolver` with `Default` and all tuning fields public.

The trait exists so the ORCA-style and scoped time-to-collision candidates slot in for the next slice's bake-off without touching any phase.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/avoidance/sampled.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use crate::units::Vec2;

    fn solver() -> SampledVelocitySolver {
        SampledVelocitySolver::default()
    }

    fn input<'a>(
        agent_id: u64,
        position: Vec2,
        velocity: Vec2,
        preferred: Vec2,
        neighbors: &'a [NeighborState],
        walls: &'a [Segment],
    ) -> AvoidanceInput<'a> {
        AvoidanceInput {
            agent_id: AgentId(agent_id),
            position,
            velocity,
            preferred,
            radius: 0.3,
            max_speed: 2.0,
            neighbors,
            walls,
        }
    }

    #[test]
    fn an_unobstructed_agent_keeps_its_preferred_velocity() {
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &[]));
        assert_eq!(out.status, SolverStatus::Free);
        assert!((out.velocity - preferred).length() < 1e-4);
    }

    #[test]
    fn a_stopped_agent_with_no_goal_stays_stopped() {
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, &[], &[]));
        assert_eq!(out.velocity, Vec2::ZERO);
        assert_eq!(out.status, SolverStatus::Free);
    }

    #[test]
    fn a_head_on_neighbor_deflects_the_agent() {
        let neighbors = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &neighbors, &[]));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(out.velocity.y.abs() > 0.05, "no lateral deflection: {out:?}");
    }

    #[test]
    fn head_on_agents_choose_opposite_sides() {
        // Deterministic tie-breaking by stable ID, contract section 6.2. Both
        // agents run the same solver on mirrored inputs; if they picked the
        // same side they would deadlock.
        let a_neighbors = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let b_neighbors = [NeighborState {
            position: Vec2::new(0.0, 0.0),
            velocity: Vec2::new(1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(1),
        }];
        let a = solver().solve(&input(
            1,
            Vec2::ZERO,
            Vec2::new(1.35, 0.0),
            Vec2::new(1.35, 0.0),
            &a_neighbors,
            &[],
        ));
        let b = solver().solve(&input(
            2,
            Vec2::new(4.0, 0.0),
            Vec2::new(-1.35, 0.0),
            Vec2::new(-1.35, 0.0),
            &b_neighbors,
            &[],
        ));
        // In world space, passing on opposite sides means both deflect the
        // same way along Y is WRONG; they must deflect oppositely.
        assert!(
            a.velocity.y * b.velocity.y < 0.0,
            "agents chose the same side: a={:?} b={:?}",
            a.velocity,
            b.velocity
        );
    }

    #[test]
    fn head_on_side_choice_does_not_depend_on_id_ordering() {
        // The head-on rule must be a *fixed* convention, not an ID
        // comparison. Two agents meeting head-on see mirrored geometry, so if
        // the side were derived from "am I the lower ID?", they would derive
        // opposite answers, deflect the same way in world space, and stay on
        // a collision course. See the module docs.
        let neighbors_low = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(99),
        }];
        let neighbors_high = [NeighborState {
            position: Vec2::new(4.0, 0.0),
            velocity: Vec2::new(-1.35, 0.0),
            radius: 0.3,
            agent_id: AgentId(1),
        }];
        let preferred = Vec2::new(1.35, 0.0);
        let lower_id =
            solver().solve(&input(5, Vec2::ZERO, preferred, preferred, &neighbors_low, &[]));
        let higher_id =
            solver().solve(&input(5, Vec2::ZERO, preferred, preferred, &neighbors_high, &[]));
        assert!(
            lower_id.velocity.y * higher_id.velocity.y > 0.0,
            "head-on side must be a fixed convention: {:?} vs {:?}",
            lower_id.velocity,
            higher_id.velocity
        );
    }

    #[test]
    fn the_higher_id_yields_more_in_a_crossing_conflict() {
        // A perpendicular conflict is symmetric, so the keep-left convention
        // is degenerate there. Stable IDs supply the asymmetry: the higher ID
        // gives way. This is contract section 6.2's deterministic
        // tie-breaking.
        let crossing_neighbor = |id: u64| {
            [NeighborState {
                position: Vec2::new(2.0, -2.0),
                velocity: Vec2::new(0.0, 1.35),
                radius: 0.3,
                agent_id: AgentId(id),
            }]
        };
        let preferred = Vec2::new(1.35, 0.0);
        let lower = solver().solve(&input(
            10,
            Vec2::ZERO,
            preferred,
            preferred,
            &crossing_neighbor(20),
            &[],
        ));
        let higher = solver().solve(&input(
            30,
            Vec2::ZERO,
            preferred,
            preferred,
            &crossing_neighbor(20),
            &[],
        ));
        assert!(
            higher.velocity.length() <= lower.velocity.length(),
            "the higher ID must not push harder: lower={:?} higher={:?}",
            lower.velocity,
            higher.velocity
        );
    }

    #[test]
    fn a_wall_ahead_deflects_the_agent() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(out.velocity.x < preferred.x, "agent drove into the wall");
    }

    #[test]
    fn a_boxed_in_agent_brakes_rather_than_escaping() {
        // Walls on three sides and a neighbor on the fourth: no candidate is
        // safe, so the contract's graceful fallback applies.
        let walls = [
            Segment::new(Vec2::new(0.6, -2.0), Vec2::new(0.6, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.6), Vec2::new(2.0, 0.6)),
            Segment::new(Vec2::new(-2.0, -0.6), Vec2::new(2.0, -0.6)),
            Segment::new(Vec2::new(-0.6, -2.0), Vec2::new(-0.6, 2.0)),
        ];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_eq!(out.status, SolverStatus::Braking);
        assert!(out.velocity.length() < preferred.length());
    }

    #[test]
    fn the_solution_never_exceeds_max_speed() {
        let preferred = Vec2::new(100.0, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, preferred, &[], &[]));
        assert!(out.velocity.length() <= 2.0 + 1e-4, "got {}", out.velocity.length());
    }

    #[test]
    fn dense_neighbors_reduce_speed() {
        // Contract section 6.2 density-aware speed reduction.
        let crowd: Vec<NeighborState> = (0..8)
            .map(|i| NeighborState {
                position: Vec2::from_yaw(i as f32) * 0.75,
                velocity: Vec2::ZERO,
                radius: 0.3,
                agent_id: AgentId(100 + i as u64),
            })
            .collect();
        let preferred = Vec2::new(1.35, 0.0);
        let sparse = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &[]));
        let dense = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &crowd, &[]));
        assert!(
            dense.velocity.length() < sparse.velocity.length(),
            "density did not slow the agent"
        );
    }

    #[test]
    fn the_output_is_always_finite() {
        let neighbors = [NeighborState {
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let walls = [Segment::new(Vec2::ZERO, Vec2::ZERO)];
        let out = solver().solve(&input(
            1,
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(1.35, 0.0),
            &neighbors,
            &walls,
        ));
        assert!(out.velocity.is_finite(), "got {:?}", out.velocity);
    }

    #[test]
    fn solving_is_deterministic_for_identical_input() {
        let neighbors = [NeighborState {
            position: Vec2::new(2.0, 0.5),
            velocity: Vec2::new(-1.0, 0.0),
            radius: 0.3,
            agent_id: AgentId(2),
        }];
        let preferred = Vec2::new(1.35, 0.0);
        let first = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &neighbors, &[]));
        let second = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &neighbors, &[]));
        assert_eq!(first.velocity, second.velocity);
        assert_eq!(first.status, second.status);
    }

    #[test]
    fn the_solver_reports_its_name() {
        assert_eq!(solver().name(), "sampled_velocity");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core sampled`
Expected: FAIL — `cannot find type SampledVelocitySolver in this scope`.

- [ ] **Step 3: Write the trait and I/O types**

`crates/crowd-core/src/avoidance/mod.rs`:

```rust
//! Local avoidance, contract section 6.2.
//!
//! The trait exists so the ORCA-style and scoped time-to-collision candidates
//! can be measured against this baseline in the next slice without touching
//! any tick phase.

pub mod sampled;

use crate::geometry::Segment;
use crate::ids::AgentId;
use crate::units::Vec2;
use crate::world::SolverStatus;

pub use sampled::SampledVelocitySolver;

/// One neighbor as the solver sees it: a disc with a velocity and an ID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeighborState {
    pub position: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
    pub agent_id: AgentId,
}

/// Everything one agent's avoidance decision depends on.
///
/// Deliberately a plain snapshot rather than a reference to the world: it
/// makes the solver trivially testable in isolation and keeps solvers from
/// reaching into state they should not read.
#[derive(Clone, Copy, Debug)]
pub struct AvoidanceInput<'a> {
    pub agent_id: AgentId,
    pub position: Vec2,
    pub velocity: Vec2,
    /// Goal-seeking velocity from the decide phase, before avoidance.
    pub preferred: Vec2,
    pub radius: f32,
    pub max_speed: f32,
    pub neighbors: &'a [NeighborState],
    pub walls: &'a [Segment],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvoidanceOutput {
    pub velocity: Vec2,
    pub status: SolverStatus,
    /// Predicted time to collision for the chosen velocity, or `f32::INFINITY`.
    /// Reported so the metrics layer does not recompute it.
    pub min_time_to_collision: f32,
}

pub trait AvoidanceSolver {
    fn name(&self) -> &'static str;
    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput;
}
```

- [ ] **Step 4: Write the sampled solver**

Prepend to `crates/crowd-core/src/avoidance/sampled.rs`:

```rust
//! Sampled-velocity avoidance, the 1.0 baseline solver.
//!
//! Each agent scores a fixed, fixed-order set of candidate velocities and
//! takes the cheapest. The cost has four terms:
//!
//! - distance from the preferred velocity (make progress);
//! - predicted time to collision with neighbors and walls (stay safe);
//! - deviation from the current velocity (stay smooth);
//! - a fixed keep-left preference for head-on encounters.
//!
//! The smoothness term is not decoration: without it, a candidate set
//! re-evaluated every tick flips between near-equal options and produces the
//! high-frequency oscillation contract section 6.2 names as a production
//! blocker.
//!
//! # Why head-on side choice is a convention, not an ID comparison
//!
//! Two agents meeting head-on see mirrored geometry. If each asked "am I the
//! lower ID?" they would get *opposite* answers, and because their frames are
//! mirrored, opposite answers produce the **same** world-space deflection —
//! both drift the same way and stay on a collision course. A fixed convention
//! evaluated in each agent's own frame produces opposite world deflections,
//! which is what actually separates them. Real pedestrians solve it the same
//! way, with a cultural side convention rather than a negotiation.
//!
//! Stable IDs still supply the asymmetry contract section 6.2 requires, but
//! where it is actually needed: a perpendicular crossing conflict is
//! symmetric under the keep-left rule, so the higher ID yields via a heavier
//! collision weight. Both agents can compute that from data both already
//! have, so they never both yield or both push.

use super::{AvoidanceInput, AvoidanceOutput, AvoidanceSolver};
use crate::geometry::{time_to_collision_disc, time_to_collision_segment, Segment};
use crate::units::Vec2;
use crate::world::SolverStatus;

// Re-exported for the test module's convenience.
pub use super::NeighborState;

#[derive(Clone, Copy, Debug)]
pub struct SampledVelocitySolver {
    /// Speed rings sampled below the preferred speed, plus a stop candidate.
    pub speed_samples: u32,
    /// Headings sampled around the full circle.
    pub heading_samples: u32,
    /// Ignore predicted collisions further out than this, in seconds.
    pub time_horizon: f32,
    /// Wall lookahead, in seconds. Shorter than `time_horizon` because static
    /// geometry is avoided by turning, not by long-range planning.
    pub wall_horizon: f32,
    pub goal_weight: f32,
    pub collision_weight: f32,
    /// Walls do not yield, so they weigh more than a neighbor at equal range.
    pub wall_weight: f32,
    pub smoothness_weight: f32,
    pub side_bias_weight: f32,
    /// Extra collision weight carried by the higher-ID agent in a conflict,
    /// which is what makes a symmetric crossing resolve.
    pub yield_factor: f32,
    /// Below this predicted time to collision, the agent is reported braking.
    pub critical_time_to_collision: f32,
    /// Comfortable clearance beyond touching radii, in meters.
    pub personal_space: f32,
    /// How strongly local crowding reduces preferred speed.
    pub density_speed_factor: f32,
    /// Cosine threshold for treating an encounter as head-on.
    pub head_on_cosine: f32,
}

impl Default for SampledVelocitySolver {
    fn default() -> Self {
        Self {
            speed_samples: 3,
            heading_samples: 16,
            time_horizon: 3.0,
            wall_horizon: 2.0,
            goal_weight: 1.0,
            collision_weight: 2.0,
            wall_weight: 1.5,
            smoothness_weight: 0.35,
            side_bias_weight: 0.6,
            yield_factor: 1.4,
            critical_time_to_collision: 0.5,
            personal_space: 0.45,
            density_speed_factor: 0.18,
            head_on_cosine: 0.7,
        }
    }
}

impl SampledVelocitySolver {
    /// Collision penalty and earliest predicted collision for one candidate.
    ///
    /// Neighbors use the reciprocal construction: assuming the neighbor holds
    /// its velocity while this agent takes on the full correction produces
    /// mutual over-correction, so each side is credited with half the change.
    ///
    /// Penalties are summed across threats rather than taken from the single
    /// worst one, so an agent squeezed between two neighbors feels both.
    fn collision_cost(&self, input: &AvoidanceInput<'_>, candidate: Vec2) -> (f32, f32) {
        let mut cost = 0.0;
        let mut earliest = f32::INFINITY;

        for neighbor in input.neighbors {
            let reciprocal_velocity = candidate * 2.0 - input.velocity;
            let relative_position = neighbor.position - input.position;
            let relative_velocity = neighbor.velocity - reciprocal_velocity;
            let combined_radius = input.radius + neighbor.radius;
            let Some(t) =
                time_to_collision_disc(relative_position, relative_velocity, combined_radius)
            else {
                continue;
            };
            if t < earliest {
                earliest = t;
            }
            if t < self.time_horizon {
                // The higher stable ID yields. A perpendicular conflict is
                // symmetric under the keep-left rule, so without this both
                // agents would make the identical choice and collide.
                let yield_weight = if input.agent_id > neighbor.agent_id {
                    self.yield_factor
                } else {
                    1.0
                };
                // `max` keeps an already-overlapping pair (t == 0) finite
                // while still dominating every other term.
                cost += self.collision_weight * yield_weight / t.max(0.01);
            }
        }

        for wall in input.walls {
            let Some(t) = time_to_collision_segment(
                input.position,
                candidate,
                input.radius,
                wall,
                self.wall_horizon,
            ) else {
                continue;
            };
            if t < earliest {
                earliest = t;
            }
            // Walls never yield, so they are weighted more heavily than a
            // neighbor at the same predicted range.
            cost += self.collision_weight * self.wall_weight / t.max(0.01);
        }

        (cost, earliest)
    }

    /// Extra cost for passing a head-on neighbor on the wrong side.
    ///
    /// A **fixed** keep-left convention, evaluated in the agent's own frame.
    /// Deriving the side from an ID comparison would be actively wrong here:
    /// mirrored agents would derive opposite answers, which in mirrored frames
    /// means the same world-space deflection, and they would fail to separate.
    fn side_bias_cost(&self, input: &AvoidanceInput<'_>, candidate: Vec2) -> f32 {
        let mut cost = 0.0;
        let heading = input.preferred.normalize_or_zero();
        if heading == Vec2::ZERO {
            return 0.0;
        }

        for neighbor in input.neighbors {
            let to_neighbor = (neighbor.position - input.position).normalize_or_zero();
            if to_neighbor == Vec2::ZERO || heading.dot(to_neighbor) < self.head_on_cosine {
                continue;
            }
            // Only bias against neighbors actually closing on us.
            if (neighbor.velocity - input.velocity).dot(to_neighbor) >= 0.0 {
                continue;
            }

            // Positive cross product means the candidate passes to the left as
            // the agent looks at the neighbor — its own left, in its own frame.
            let candidate_side = to_neighbor.x * candidate.y - to_neighbor.y * candidate.x;
            if candidate_side < 0.0 {
                cost += self.side_bias_weight;
            }
        }
        cost
    }

    /// Preferred velocity scaled down by local crowding.
    fn density_adjusted_preferred(&self, input: &AvoidanceInput<'_>) -> Vec2 {
        let crowding = input
            .neighbors
            .iter()
            .filter(|n| {
                let clearance = input.radius + n.radius + self.personal_space;
                (n.position - input.position).length_squared() < clearance * clearance
            })
            .count() as f32;
        input.preferred * (1.0 / (1.0 + self.density_speed_factor * crowding))
    }
}

impl AvoidanceSolver for SampledVelocitySolver {
    fn name(&self) -> &'static str {
        "sampled_velocity"
    }

    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput {
        let preferred = self
            .density_adjusted_preferred(input)
            .clamp_length(input.max_speed);

        // A stationary agent with no goal has nothing to solve, and sampling
        // headings around a zero vector would be meaningless.
        if preferred.length_squared() <= f32::MIN_POSITIVE
            && input.velocity.length_squared() <= f32::MIN_POSITIVE
        {
            return AvoidanceOutput {
                velocity: Vec2::ZERO,
                status: SolverStatus::Free,
                min_time_to_collision: f32::INFINITY,
            };
        }

        let preferred_speed = preferred.length();
        let heading = if preferred_speed > f32::MIN_POSITIVE {
            preferred.normalize_or_zero()
        } else {
            input.velocity.normalize_or_zero()
        };

        let mut best_velocity = Vec2::ZERO;
        let mut best_cost = f32::INFINITY;
        let mut best_ttc = f32::INFINITY;

        // Candidate generation order is fixed, so ties resolve identically on
        // every run. The preferred velocity is evaluated first so an
        // unobstructed agent keeps it exactly.
        let mut evaluate = |candidate: Vec2,
                            best_velocity: &mut Vec2,
                            best_cost: &mut f32,
                            best_ttc: &mut f32| {
            let (collision_cost, ttc) = self.collision_cost(input, candidate);
            let cost = self.goal_weight * (candidate - preferred).length()
                + self.smoothness_weight * (candidate - input.velocity).length()
                + collision_cost
                + self.side_bias_cost(input, candidate);
            // Strict `<` means the first candidate evaluated wins a tie, and
            // candidate order is fixed, so ties resolve identically every run.
            if cost < *best_cost {
                *best_cost = cost;
                *best_velocity = candidate;
                *best_ttc = ttc;
            }
        };

        evaluate(preferred, &mut best_velocity, &mut best_cost, &mut best_ttc);

        let speed_reference = preferred_speed.max(input.velocity.length());
        for speed_index in 1..=self.speed_samples {
            let speed = speed_reference * speed_index as f32 / self.speed_samples as f32;
            for heading_index in 0..self.heading_samples {
                let angle = std::f32::consts::TAU * heading_index as f32
                    / self.heading_samples as f32;
                let direction = rotate(heading, angle);
                evaluate(
                    direction * speed,
                    &mut best_velocity,
                    &mut best_cost,
                    &mut best_ttc,
                );
            }
        }

        // Stopping is always available. It is the contract's graceful fallback
        // when no feasible velocity exists.
        evaluate(Vec2::ZERO, &mut best_velocity, &mut best_cost, &mut best_ttc);

        let status = if best_ttc < self.critical_time_to_collision {
            SolverStatus::Braking
        } else if (best_velocity - preferred).length() > 1e-3 {
            SolverStatus::Avoiding
        } else {
            SolverStatus::Free
        };

        AvoidanceOutput {
            velocity: best_velocity.clamp_length(input.max_speed),
            status,
            min_time_to_collision: best_ttc,
        }
    }
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}
```

- [ ] **Step 5: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod avoidance;` and `pub use avoidance::{AvoidanceInput, AvoidanceOutput, AvoidanceSolver, NeighborState, SampledVelocitySolver};`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p crowd-core avoidance`
Expected: PASS, 13 tests.

If `a_boxed_in_agent_brakes_rather_than_escaping` fails, the box in the fixture is smaller than the agent diameter plus clearance — widen the walls to ±0.8 rather than weakening the assertion.

If `head_on_agents_choose_opposite_sides` fails, do not reach for an ID comparison — that is the bug this design exists to avoid. Check instead that `side_bias_cost` computes the cross product against `to_neighbor` (the agent's own frame) and not against a world axis, and that the penalty is applied for `candidate_side < 0.0` in both agents identically.

- [ ] **Step 7: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/avoidance/ crates/crowd-core/src/lib.rs
git commit -m "Add avoidance solver trait and sampled-velocity baseline"
```

---

## Task 15: Steer phase

**Files:**
- Create: `crates/crowd-core/src/phases/steer.rs`
- Modify: `crates/crowd-core/src/phases/mod.rs`

**Interfaces:**
- Consumes: `World`, `NeighborArena`, `CompiledScene`, `AvoidanceSolver`, `NeighborState`, `AvoidanceInput`.
- Produces: `SteerConfig { wall_query_radius: f32 }` with `Default` (`3.0`); `SteerScratch::default()`; free function `steer(world: &mut World, arena: &NeighborArena, scene: &CompiledScene, solver: &dyn AvoidanceSolver, config: &SteerConfig, scratch: &mut SteerScratch) -> SteerReport` where `SteerReport { min_time_to_collision: f32, braking_agents: u32 }`.

`steer` overwrites `des_vel_*` with the solved velocity, and updates `solver_status` and `stall_ticks`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/phases/steer.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::grid::UniformGrid;
    use crate::ids::AgentId;
    use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, SolverStatus, World, NO_ROUTE};

    fn open_scene(walls: Vec<Segment>) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "steer_test".into(),
            bounds: Aabb::new(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0)),
            walls,
            waypoints,
            destinations: vec![Destination { name: "exit".into(), node: b }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count: 1,
                per_tick: 1,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 1,
            ticks_per_second: 30,
            duration_ticks: 10,
        }
        .compile()
        .unwrap()
    }

    fn world_with(agents: &[(u64, Vec2, Vec2)]) -> World {
        let mut world = World::new();
        for (id, position, desired) in agents {
            let slot = world
                .spawn(
                    AgentSpawn {
                        agent_id: AgentId(*id),
                        population_id: 0,
                        position: *position,
                        yaw: 0.0,
                        radius: 0.3,
                        max_speed: 2.0,
                        preferred_speed: 1.35,
                        route: NO_ROUTE,
                        destination: 0,
                    },
                    0,
                )
                .unwrap() as usize;
            world.des_vel_x[slot] = desired.x;
            world.des_vel_y[slot] = desired.y;
        }
        world
    }

    fn run_steer(world: &mut World, scene: &CompiledScene) -> SteerReport {
        let mut grid = UniformGrid::new(scene.bounds, 5.0);
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut perceive_scratch = PerceiveScratch::default();
        let mut arena = NeighborArena::new();
        perceive(
            world,
            &grid,
            &PerceiveConfig::default(),
            &mut perceive_scratch,
            &mut arena,
        );
        let solver = SampledVelocitySolver::default();
        let mut scratch = SteerScratch::default();
        steer(
            world,
            &arena,
            scene,
            &solver,
            &SteerConfig::default(),
            &mut scratch,
        )
    }

    #[test]
    fn an_isolated_agent_keeps_its_preferred_velocity() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        run_steer(&mut world, &scene);
        assert!((world.desired_velocity(0) - Vec2::new(1.35, 0.0)).length() < 1e-4);
        assert_eq!(world.solver_status[0], SolverStatus::Free);
        assert_eq!(world.stall_ticks[0], 0);
    }

    #[test]
    fn converging_agents_are_deflected() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[
            (1, Vec2::new(0.0, 0.0), Vec2::new(1.35, 0.0)),
            (2, Vec2::new(3.0, 0.0), Vec2::new(-1.35, 0.0)),
        ]);
        run_steer(&mut world, &scene);
        assert!(world.desired_velocity(0).y.abs() > 0.01);
        assert!(world.desired_velocity(1).y.abs() > 0.01);
    }

    #[test]
    fn walls_are_supplied_to_the_solver() {
        let walls = vec![Segment::new(Vec2::new(2.0, -5.0), Vec2::new(2.0, 5.0))];
        let scene = open_scene(walls);
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        run_steer(&mut world, &scene);
        assert_ne!(world.solver_status[0], SolverStatus::Free);
    }

    #[test]
    fn braking_increments_the_stall_counter() {
        let walls = vec![
            Segment::new(Vec2::new(0.7, -2.0), Vec2::new(0.7, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.7), Vec2::new(2.0, 0.7)),
            Segment::new(Vec2::new(-2.0, -0.7), Vec2::new(2.0, -0.7)),
            Segment::new(Vec2::new(-0.7, -2.0), Vec2::new(-0.7, 2.0)),
        ];
        let scene = open_scene(walls);
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        run_steer(&mut world, &scene);
        assert_eq!(world.solver_status[0], SolverStatus::Braking);
        assert_eq!(world.stall_ticks[0], 1);
        run_steer(&mut world, &scene);
        assert_eq!(world.stall_ticks[0], 2);
    }

    #[test]
    fn leaving_a_stall_resets_the_counter() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[(1, Vec2::ZERO, Vec2::new(1.35, 0.0))]);
        world.stall_ticks[0] = 9;
        run_steer(&mut world, &scene);
        assert_eq!(world.stall_ticks[0], 0);
    }

    #[test]
    fn the_report_aggregates_the_worst_time_to_collision() {
        let scene = open_scene(Vec::new());
        let mut world = world_with(&[
            (1, Vec2::new(0.0, 0.0), Vec2::new(1.35, 0.0)),
            (2, Vec2::new(2.0, 0.0), Vec2::new(-1.35, 0.0)),
        ]);
        let report = run_steer(&mut world, &scene);
        assert!(report.min_time_to_collision.is_finite());
    }

    #[test]
    fn steering_an_empty_world_is_a_no_op() {
        let scene = open_scene(Vec::new());
        let mut world = World::new();
        let report = run_steer(&mut world, &scene);
        assert_eq!(report.braking_agents, 0);
        assert!(report.min_time_to_collision.is_infinite());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core steer`
Expected: FAIL — `cannot find type SteerConfig in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/phases/steer.rs`:

```rust
//! Tick phase 6: steer.
//!
//! Turns the decide phase's preferred velocity into a solved velocity by
//! running the avoidance solver against perceived neighbors and nearby walls.
//! Reads only previous-tick state, so the result does not depend on the order
//! agents are visited.

use crate::arena::NeighborArena;
use crate::avoidance::{AvoidanceInput, AvoidanceSolver, NeighborState};
use crate::geometry::Segment;
use crate::scene::CompiledScene;
use crate::units::Vec2;
use crate::world::{SolverStatus, World};

#[derive(Clone, Copy, Debug)]
pub struct SteerConfig {
    /// How far to look for walls. Should exceed the solver's wall horizon
    /// times the maximum speed, or an agent can turn into a wall it never saw.
    pub wall_query_radius: f32,
}

impl Default for SteerConfig {
    fn default() -> Self {
        Self {
            wall_query_radius: 3.0,
        }
    }
}

/// Reused buffers so the phase does not allocate after warmup.
#[derive(Clone, Debug, Default)]
pub struct SteerScratch {
    neighbors: Vec<NeighborState>,
    wall_indices: Vec<u32>,
    walls: Vec<Segment>,
}

/// Tick-level aggregates the metrics layer would otherwise recompute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteerReport {
    pub min_time_to_collision: f32,
    pub braking_agents: u32,
}

pub fn steer(
    world: &mut World,
    arena: &NeighborArena,
    scene: &CompiledScene,
    solver: &dyn AvoidanceSolver,
    config: &SteerConfig,
    scratch: &mut SteerScratch,
) -> SteerReport {
    let mut min_time_to_collision = f32::INFINITY;
    let mut braking_agents = 0;

    for slot in 0..world.len() {
        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);

        scratch.neighbors.clear();
        for neighbor in arena.neighbors(slot) {
            let other = neighbor.slot as usize;
            scratch.neighbors.push(NeighborState {
                position: Vec2::new(world.pos_x[other], world.pos_y[other]),
                velocity: Vec2::new(world.vel_x[other], world.vel_y[other]),
                radius: world.radius[other],
                agent_id: world.agent_id[other],
            });
        }

        scene
            .wall_index
            .query(position, config.wall_query_radius, &mut scratch.wall_indices);
        scratch.walls.clear();
        for &index in &scratch.wall_indices {
            scratch.walls.push(scene.walls[index as usize]);
        }

        let output = solver.solve(&AvoidanceInput {
            agent_id: world.agent_id[slot],
            position,
            velocity: Vec2::new(world.vel_x[slot], world.vel_y[slot]),
            preferred: Vec2::new(world.des_vel_x[slot], world.des_vel_y[slot]),
            radius: world.radius[slot],
            max_speed: world.max_speed[slot],
            neighbors: &scratch.neighbors,
            walls: &scratch.walls,
        });

        // A non-finite solution would propagate into position and poison the
        // whole bake, so refuse it here rather than letting it escape.
        debug_assert!(output.velocity.is_finite(), "solver produced {output:?}");
        let velocity = if output.velocity.is_finite() {
            output.velocity
        } else {
            Vec2::ZERO
        };

        world.des_vel_x[slot] = velocity.x;
        world.des_vel_y[slot] = velocity.y;
        world.solver_status[slot] = output.status;

        if output.status == SolverStatus::Braking {
            braking_agents += 1;
            world.stall_ticks[slot] = world.stall_ticks[slot].saturating_add(1);
        } else {
            world.stall_ticks[slot] = 0;
        }

        if output.min_time_to_collision < min_time_to_collision {
            min_time_to_collision = output.min_time_to_collision;
        }
    }

    SteerReport {
        min_time_to_collision,
        braking_agents,
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/phases/mod.rs` add `pub mod steer;` and `pub use steer::{steer, SteerConfig, SteerReport, SteerScratch};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core steer`
Expected: PASS, 7 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/phases/
git commit -m "Add steer phase wiring perception and walls into the solver"
```

---

## Task 16: Integrate phase

**Files:**
- Create: `crates/crowd-core/src/phases/integrate.rs`
- Modify: `crates/crowd-core/src/phases/mod.rs`

**Interfaces:**
- Consumes: `World`, `CompiledScene`, `Segment`.
- Produces: `IntegrateConfig { max_acceleration: f32, max_turn_rate: f32, wall_query_radius: f32 }` with `Default` (`4.0`, `6.0`, `2.0`); `IntegrateScratch::default()`; free function `integrate(world: &mut World, scene: &CompiledScene, config: &IntegrateConfig, dt: f32, scratch: &mut IntegrateScratch) -> IntegrateReport` where `IntegrateReport { wall_corrections: u32, nonfinite_corrections: u32 }`.

`integrate` is the sole writer of `next_pos_*`, `next_vel_*`, and `next_yaw`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/phases/integrate.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AgentId;
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn scene_with(walls: Vec<Segment>) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(1.0, 5.0));
        let b = waypoints.add_node(Vec2::new(9.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "integrate_test".into(),
            bounds: Aabb::new(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0)),
            walls,
            waypoints,
            destinations: vec![Destination { name: "exit".into(), node: b }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 4.0), Vec2::new(1.5, 6.0)),
                count: 1,
                per_tick: 1,
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 1,
            ticks_per_second: 30,
            duration_ticks: 10,
        }
        .compile()
        .unwrap()
    }

    fn world_with(position: Vec2, velocity: Vec2, desired: Vec2, yaw: f32) -> World {
        let mut world = World::new();
        world
            .spawn(
                AgentSpawn {
                    agent_id: AgentId(1),
                    population_id: 0,
                    position,
                    yaw,
                    radius: 0.3,
                    max_speed: 2.0,
                    preferred_speed: 1.35,
                    route: NO_ROUTE,
                    destination: 0,
                },
                0,
            )
            .unwrap();
        world.vel_x[0] = velocity.x;
        world.vel_y[0] = velocity.y;
        world.des_vel_x[0] = desired.x;
        world.des_vel_y[0] = desired.y;
        world
    }

    fn run(world: &mut World, scene: &CompiledScene) -> IntegrateReport {
        let mut scratch = IntegrateScratch::default();
        let report = integrate(
            world,
            scene,
            &IntegrateConfig::default(),
            1.0 / 30.0,
            &mut scratch,
        );
        world.commit();
        report
    }

    #[test]
    fn an_agent_moves_along_its_desired_velocity() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0), 0.0);
        run(&mut world, &scene);
        assert!((world.position(0).x - 1.0 / 30.0).abs() < 1e-5);
        assert!(world.position(0).y.abs() < 1e-6);
    }

    #[test]
    fn acceleration_is_limited() {
        // Desired velocity jumps from rest to 2 m/s in one 1/30 s tick. With
        // max_acceleration 4.0, only 4/30 m/s of change is allowed.
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::new(2.0, 0.0), 0.0);
        run(&mut world, &scene);
        let speed = world.velocity(0).length();
        assert!((speed - 4.0 / 30.0).abs() < 1e-4, "speed was {speed}");
    }

    #[test]
    fn speed_never_exceeds_max_speed() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.9, 0.0), Vec2::new(50.0, 0.0), 0.0);
        for _ in 0..100 {
            run(&mut world, &scene);
        }
        assert!(world.velocity(0).length() <= 2.0 + 1e-4);
    }

    #[test]
    fn turn_rate_is_limited() {
        // A 180-degree reversal cannot complete in one tick at 6 rad/s.
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0), 0.0);
        run(&mut world, &scene);
        assert!(world.yaw[0].abs() <= 6.0 / 30.0 + 1e-4, "yaw was {}", world.yaw[0]);
    }

    #[test]
    fn yaw_is_unchanged_when_an_agent_is_stationary() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 1.234);
        run(&mut world, &scene);
        assert!((world.yaw[0] - 1.234).abs() < 1e-6);
    }

    #[test]
    fn an_agent_is_pushed_out_of_a_wall_it_penetrated() {
        let walls = vec![Segment::new(Vec2::new(1.0, -5.0), Vec2::new(1.0, 5.0))];
        let scene = scene_with(walls);
        // Start 0.1m from the wall with an agent radius of 0.3: penetrated.
        let mut world = world_with(Vec2::new(0.9, 0.0), Vec2::ZERO, Vec2::ZERO, 0.0);
        let report = run(&mut world, &scene);
        assert_eq!(report.wall_corrections, 1);
        assert!(
            world.position(0).x <= 0.7 + 1e-4,
            "agent was not pushed clear: {:?}",
            world.position(0)
        );
    }

    #[test]
    fn an_agent_clear_of_walls_is_not_corrected() {
        let walls = vec![Segment::new(Vec2::new(5.0, -5.0), Vec2::new(5.0, 5.0))];
        let scene = scene_with(walls);
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 0.0);
        assert_eq!(run(&mut world, &scene).wall_corrections, 0);
    }

    #[test]
    fn a_nonfinite_desired_velocity_is_neutralised() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, 0.0);
        world.des_vel_x[0] = f32::NAN;
        let report = integrate(
            &mut world,
            &scene,
            &IntegrateConfig::default(),
            1.0 / 30.0,
            &mut IntegrateScratch::default(),
        );
        world.commit();
        assert_eq!(report.nonfinite_corrections, 1);
        assert!(world.position(0).is_finite());
        assert!(world.velocity(0).is_finite());
    }

    #[test]
    fn integration_is_the_only_writer_of_position() {
        let scene = scene_with(Vec::new());
        let mut world = world_with(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0), 0.0);
        let before = world.position(0);
        let mut scratch = IntegrateScratch::default();
        integrate(&mut world, &scene, &IntegrateConfig::default(), 1.0 / 30.0, &mut scratch);
        // Until commit, current state is untouched.
        assert_eq!(world.position(0), before);
        world.commit();
        assert_ne!(world.position(0), before);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core integrate`
Expected: FAIL — `cannot find type IntegrateConfig in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/phases/integrate.rs`:

```rust
//! Tick phase 7: integrate.
//!
//! The sole writer of position and orientation. Applies acceleration and
//! turn-rate limits, advances state, and resolves residual wall penetration.
//!
//! Non-finite state is neutralised and counted rather than propagated: contract
//! section 10 makes the tick loop infallible, and silently carrying a NaN
//! through a bake is the worst available outcome.

use crate::geometry::Segment;
use crate::scene::CompiledScene;
use crate::units::{wrap_angle, Vec2};
use crate::world::World;

#[derive(Clone, Copy, Debug)]
pub struct IntegrateConfig {
    /// Metres per second squared. Pedestrians accelerate modestly; a high cap
    /// lets the solver produce visible velocity discontinuities.
    pub max_acceleration: f32,
    /// Radians per second.
    pub max_turn_rate: f32,
    pub wall_query_radius: f32,
}

impl Default for IntegrateConfig {
    fn default() -> Self {
        Self {
            max_acceleration: 4.0,
            max_turn_rate: 6.0,
            wall_query_radius: 2.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct IntegrateScratch {
    wall_indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IntegrateReport {
    /// Agents pushed out of a wall this tick. A persistently non-zero count
    /// means avoidance is failing upstream, not that the fix-up is working.
    pub wall_corrections: u32,
    pub nonfinite_corrections: u32,
}

pub fn integrate(
    world: &mut World,
    scene: &CompiledScene,
    config: &IntegrateConfig,
    dt: f32,
    scratch: &mut IntegrateScratch,
) -> IntegrateReport {
    let mut report = IntegrateReport::default();
    let max_delta_v = config.max_acceleration * dt;
    let max_delta_yaw = config.max_turn_rate * dt;

    for slot in 0..world.len() {
        let position = Vec2::new(world.pos_x[slot], world.pos_y[slot]);
        let velocity = Vec2::new(world.vel_x[slot], world.vel_y[slot]);
        let mut desired = Vec2::new(world.des_vel_x[slot], world.des_vel_y[slot]);

        if !desired.is_finite() {
            desired = Vec2::ZERO;
            report.nonfinite_corrections += 1;
        }

        // Acceleration limit, then speed limit. Order matters: clamping speed
        // first would let a large lateral correction slip through.
        let delta = (desired - velocity).clamp_length(max_delta_v);
        let new_velocity = (velocity + delta).clamp_length(world.max_speed[slot]);
        let mut new_position = position + new_velocity * dt;

        // Resolve residual penetration. Avoidance should have prevented this;
        // when it did not, pushing out along the wall normal is preferable to
        // letting an agent walk through geometry.
        scene
            .wall_index
            .query(new_position, config.wall_query_radius, &mut scratch.wall_indices);
        let mut corrected = false;
        for &index in &scratch.wall_indices {
            let wall: Segment = scene.walls[index as usize];
            let closest = wall.closest_point(new_position);
            let offset = new_position - closest;
            let distance = offset.length();
            let radius = world.radius[slot];
            if distance < radius {
                let normal = if distance > f32::MIN_POSITIVE {
                    offset * (1.0 / distance)
                } else {
                    // Exactly on the wall: push along its normal, chosen by a
                    // fixed rule so the correction stays deterministic.
                    (wall.b - wall.a).normalize_or_zero().perp()
                };
                new_position = closest + normal * radius;
                corrected = true;
            }
        }
        if corrected {
            report.wall_corrections += 1;
        }

        let mut new_yaw = world.yaw[slot];
        if new_velocity.length_squared() > 1e-6 {
            let target_yaw = new_velocity.to_yaw();
            let delta_yaw = wrap_angle(target_yaw - new_yaw).clamp(-max_delta_yaw, max_delta_yaw);
            new_yaw = wrap_angle(new_yaw + delta_yaw);
        }

        if !new_position.is_finite() || !new_velocity.is_finite() || !new_yaw.is_finite() {
            report.nonfinite_corrections += 1;
            world.next_pos_x[slot] = position.x;
            world.next_pos_y[slot] = position.y;
            world.next_vel_x[slot] = 0.0;
            world.next_vel_y[slot] = 0.0;
            world.next_yaw[slot] = world.yaw[slot];
            continue;
        }

        world.next_pos_x[slot] = new_position.x;
        world.next_pos_y[slot] = new_position.y;
        world.next_vel_x[slot] = new_velocity.x;
        world.next_vel_y[slot] = new_velocity.y;
        world.next_yaw[slot] = new_yaw;
    }

    report
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/phases/mod.rs` add `pub mod integrate;` and `pub use integrate::{integrate, IntegrateConfig, IntegrateReport, IntegrateScratch};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core integrate`
Expected: PASS, 9 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/phases/
git commit -m "Add integrate phase with acceleration, turn, and wall limits"
```

---

## Task 17: Metrics accumulation

**Files:**
- Create: `crates/crowd-core/src/metrics.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `World`, `NeighborArena`, `CompiledScene`, `SolverStatus`, `Clock`.
- Produces: `MetricsConfig { near_miss_time: f32, stall_ticks_threshold: u16, abrupt_turn_radians: f32, throughput_gate: Option<Segment> }` with `Default`. `Metrics::new()` with `begin_tick(&mut self)`, `observe_tick(&mut self, world: &World, arena: &NeighborArena, clock: &Clock, config: &MetricsConfig)`, `record_steer(&mut self, report: &SteerReport)`, `record_integrate(&mut self, report: &IntegrateReport)`, `record_phase(&mut self, phase: Phase, nanos: u64)`, `record_arrivals(&mut self, world: &World, clock: &Clock)`, and `summarize(&self, world: &World, scene: &CompiledScene, wall_time_seconds: f64, peak_allocated_bytes: u64) -> MetricsSummary`. `Phase` enum (`Spawn`, `Index`, `Perceive`, `Decide`, `Steer`, `Integrate`, `Metrics`). `MetricsSummary` deriving `Serialize`/`Deserialize`/`PartialEq`/`Debug`/`Clone`.

`MetricsSummary` field names are the keys baselines compare on, so they are a schema. Task 21 depends on them exactly.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/metrics.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NeighborArena;
    use crate::grid::UniformGrid;
    use crate::ids::AgentId;
    use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn world_at(points: &[Vec2], radius: f32) -> World {
        let mut world = World::new();
        for (i, p) in points.iter().enumerate() {
            world
                .spawn(
                    AgentSpawn {
                        agent_id: AgentId(i as u64 + 1),
                        population_id: 0,
                        position: *p,
                        yaw: 0.0,
                        radius,
                        max_speed: 2.0,
                        preferred_speed: 1.35,
                        route: NO_ROUTE,
                        destination: 0,
                    },
                    0,
                )
                .unwrap();
        }
        world
    }

    fn observe(world: &World, metrics: &mut Metrics, clock: &Clock) {
        let mut grid = UniformGrid::new(
            Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0)),
            5.0,
        );
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut arena = NeighborArena::new();
        perceive(
            world,
            &grid,
            &PerceiveConfig::default(),
            &mut PerceiveScratch::default(),
            &mut arena,
        );
        metrics.begin_tick();
        metrics.observe_tick(world, &arena, clock, &MetricsConfig::default());
    }

    #[test]
    fn separated_agents_record_no_penetration() {
        let world = world_at(&[Vec2::ZERO, Vec2::new(5.0, 0.0)], 0.3);
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());
        assert_eq!(metrics.penetration_events(), 0);
        assert_eq!(metrics.max_penetration_depth(), 0.0);
    }

    #[test]
    fn overlapping_agents_record_penetration_depth() {
        // Radii 0.3 each, centres 0.4 apart: overlap of 0.2.
        let world = world_at(&[Vec2::ZERO, Vec2::new(0.4, 0.0)], 0.3);
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());
        assert_eq!(metrics.penetration_events(), 1, "one pair, counted once");
        assert!((metrics.max_penetration_depth() - 0.2).abs() < 1e-5);
    }

    #[test]
    fn penetration_duration_accumulates_across_ticks() {
        let world = world_at(&[Vec2::ZERO, Vec2::new(0.4, 0.0)], 0.3);
        let mut metrics = Metrics::new();
        let mut clock = Clock::default();
        for _ in 0..3 {
            observe(&world, &mut metrics, &clock);
            clock.advance();
        }
        assert_eq!(metrics.penetration_agent_ticks(), 6, "2 agents x 3 ticks");
    }

    #[test]
    fn stalled_agents_are_counted_once_past_the_threshold() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let config = MetricsConfig::default();
        world.stall_ticks[0] = config.stall_ticks_threshold;
        metrics.begin_tick();
        let arena = NeighborArena::new();
        metrics.observe_tick(&world, &arena, &Clock::default(), &config);
        assert_eq!(metrics.stalled_agents(), 1);
    }

    #[test]
    fn a_reversing_agent_records_a_heading_reversal() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let arena = NeighborArena::new();
        let config = MetricsConfig::default();
        let clock = Clock::default();

        world.vel_x[0] = 1.0;
        world.vel_y[0] = 0.5;
        metrics.begin_tick();
        metrics.observe_tick(&world, &arena, &clock, &config);

        world.vel_y[0] = -0.5;
        metrics.begin_tick();
        metrics.observe_tick(&world, &arena, &clock, &config);

        world.vel_y[0] = 0.5;
        metrics.begin_tick();
        metrics.observe_tick(&world, &arena, &clock, &config);

        assert!(metrics.heading_reversals() >= 1);
    }

    #[test]
    fn a_straight_walker_records_no_reversals() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let arena = NeighborArena::new();
        world.vel_x[0] = 1.35;
        for _ in 0..10 {
            metrics.begin_tick();
            metrics.observe_tick(&world, &arena, &Clock::default(), &MetricsConfig::default());
        }
        assert_eq!(metrics.heading_reversals(), 0);
        assert_eq!(metrics.abrupt_turns(), 0);
    }

    #[test]
    fn arrivals_record_travel_time() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let mut clock = Clock::default();
        for _ in 0..45 {
            clock.advance();
        }
        world.arrived[0] = true;
        metrics.record_arrivals(&world, &clock);
        assert_eq!(metrics.arrived(), 1);
        // 45 ticks at 30 Hz is 1.5 s.
        let summary = metrics.summarize_travel_times();
        assert!((summary.0 - 1.5).abs() < 1e-4, "median was {}", summary.0);
    }

    #[test]
    fn an_agent_is_only_counted_as_arriving_once() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let clock = Clock::default();
        world.arrived[0] = true;
        metrics.record_arrivals(&world, &clock);
        metrics.record_arrivals(&world, &clock);
        assert_eq!(metrics.arrived(), 1);
    }

    #[test]
    fn phase_timings_accumulate() {
        let mut metrics = Metrics::new();
        metrics.record_phase(Phase::Steer, 1000);
        metrics.record_phase(Phase::Steer, 500);
        assert_eq!(metrics.phase_nanos(Phase::Steer), 1500);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core metrics`
Expected: FAIL — `cannot find type Metrics in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/metrics.rs`:

```rust
//! Quality and performance metrics, contract section 12.3.
//!
//! Accumulated during simulation and summarised at the end. `MetricsSummary`
//! field names are the keys baselines compare on, so treat them as a schema:
//! renaming one silently invalidates every checked-in baseline.

use serde::{Deserialize, Serialize};

use crate::arena::NeighborArena;
use crate::clock::Clock;
use crate::geometry::Segment;
use crate::phases::integrate::IntegrateReport;
use crate::phases::steer::SteerReport;
use crate::scene::CompiledScene;
use crate::units::{wrap_angle, Vec2};
use crate::world::{SolverStatus, World};

/// Which pipeline phase a timing sample belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Spawn,
    Index,
    Perceive,
    Decide,
    Steer,
    Integrate,
    Metrics,
}

impl Phase {
    pub const ALL: [Phase; 7] = [
        Phase::Spawn,
        Phase::Index,
        Phase::Perceive,
        Phase::Decide,
        Phase::Steer,
        Phase::Integrate,
        Phase::Metrics,
    ];

    const fn index(self) -> usize {
        match self {
            Phase::Spawn => 0,
            Phase::Index => 1,
            Phase::Perceive => 2,
            Phase::Decide => 3,
            Phase::Steer => 4,
            Phase::Integrate => 5,
            Phase::Metrics => 6,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Phase::Spawn => "spawn",
            Phase::Index => "index",
            Phase::Perceive => "perceive",
            Phase::Decide => "decide",
            Phase::Steer => "steer",
            Phase::Integrate => "integrate",
            Phase::Metrics => "metrics",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MetricsConfig {
    /// Predicted time to collision below which an encounter is a near miss.
    pub near_miss_time: f32,
    /// Consecutive braking ticks before an agent counts as stalled.
    pub stall_ticks_threshold: u16,
    /// Heading change in one tick that counts as abrupt.
    pub abrupt_turn_radians: f32,
    /// Optional line agents are counted crossing, for throughput.
    pub throughput_gate: Option<Segment>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            near_miss_time: 0.5,
            stall_ticks_threshold: 15,
            abrupt_turn_radians: 0.9,
            throughput_gate: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    ticks: u64,

    penetration_events: u64,
    max_penetration_depth: f32,
    penetration_agent_ticks: u64,

    min_time_to_collision: f32,
    /// This tick's value only. The running minimum cannot be used to detect a
    /// near miss, because once it drops it stays down and every later tick
    /// would be counted.
    tick_min_time_to_collision: f32,
    near_misses: u64,

    wall_corrections: u64,
    nonfinite_corrections: u64,

    stalled_agents: u64,
    stall_agent_ticks: u64,

    heading_reversals: u64,
    abrupt_turns: u64,

    gate_crossings: u64,

    arrived: u64,
    travel_seconds: Vec<f32>,

    /// Per-agent state carried between ticks, indexed by slot.
    previous_heading: Vec<f32>,
    previous_turn_sign: Vec<i8>,
    counted_arrival: Vec<bool>,
    counted_stall: Vec<bool>,
    previous_gate_side: Vec<i8>,

    phase_nanos: [u64; 7],
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            min_time_to_collision: f32::INFINITY,
            tick_min_time_to_collision: f32::INFINITY,
            ..Default::default()
        }
    }

    pub fn begin_tick(&mut self) {
        self.ticks += 1;
        self.tick_min_time_to_collision = f32::INFINITY;
    }

    pub fn record_phase(&mut self, phase: Phase, nanos: u64) {
        self.phase_nanos[phase.index()] += nanos;
    }

    pub fn phase_nanos(&self, phase: Phase) -> u64 {
        self.phase_nanos[phase.index()]
    }

    pub fn record_steer(&mut self, report: &SteerReport) {
        if report.min_time_to_collision < self.min_time_to_collision {
            self.min_time_to_collision = report.min_time_to_collision;
        }
        if report.min_time_to_collision < self.tick_min_time_to_collision {
            self.tick_min_time_to_collision = report.min_time_to_collision;
        }
    }

    pub fn record_integrate(&mut self, report: &IntegrateReport) {
        self.wall_corrections += report.wall_corrections as u64;
        self.nonfinite_corrections += report.nonfinite_corrections as u64;
    }

    /// Grow per-agent tracking to cover newly spawned slots.
    fn ensure_capacity(&mut self, agent_count: usize) {
        self.previous_heading.resize(agent_count, f32::NAN);
        self.previous_turn_sign.resize(agent_count, 0);
        self.counted_arrival.resize(agent_count, false);
        self.counted_stall.resize(agent_count, false);
        self.previous_gate_side.resize(agent_count, 0);
    }

    pub fn observe_tick(
        &mut self,
        world: &World,
        arena: &NeighborArena,
        _clock: &Clock,
        config: &MetricsConfig,
    ) {
        self.ensure_capacity(world.len());

        for slot in 0..world.len() {
            let position = world.position(slot as u32);
            let velocity = world.velocity(slot as u32);

            // Penetration. Counted once per pair by only considering
            // neighbors with a higher agent ID, so the pair is not
            // double-counted from both sides.
            let mut penetrating = false;
            for neighbor in arena.neighbors(slot) {
                let other = neighbor.slot as usize;
                if world.agent_id[other] <= world.agent_id[slot] {
                    continue;
                }
                let combined = world.radius[slot] + world.radius[other];
                let distance = neighbor.dist_sq.sqrt();
                if distance < combined {
                    let depth = combined - distance;
                    self.penetration_events += 1;
                    if depth > self.max_penetration_depth {
                        self.max_penetration_depth = depth;
                    }
                }
            }
            // Separate pass so an agent overlapping several others still
            // contributes one agent-tick, not several.
            for neighbor in arena.neighbors(slot) {
                let other = neighbor.slot as usize;
                let combined = world.radius[slot] + world.radius[other];
                if neighbor.dist_sq.sqrt() < combined {
                    penetrating = true;
                    break;
                }
            }
            if penetrating {
                self.penetration_agent_ticks += 1;
            }

            // Stalls.
            if world.stall_ticks[slot] >= config.stall_ticks_threshold {
                self.stall_agent_ticks += 1;
                if !self.counted_stall[slot] {
                    self.counted_stall[slot] = true;
                    self.stalled_agents += 1;
                }
            } else if world.solver_status[slot] != SolverStatus::Braking {
                self.counted_stall[slot] = false;
            }

            // Oscillation. A reversal is a change in the *sign* of turning,
            // which is what reads as jitter; a steady arc is not a reversal
            // however sharp it is.
            let speed_sq = velocity.length_squared();
            if speed_sq > 1e-4 {
                let heading = velocity.to_yaw();
                let previous = self.previous_heading[slot];
                if previous.is_finite() {
                    let delta = wrap_angle(heading - previous);
                    if delta.abs() > config.abrupt_turn_radians {
                        self.abrupt_turns += 1;
                    }
                    let sign = if delta > 1e-3 {
                        1
                    } else if delta < -1e-3 {
                        -1
                    } else {
                        0
                    };
                    if sign != 0 {
                        if self.previous_turn_sign[slot] != 0
                            && sign != self.previous_turn_sign[slot]
                        {
                            self.heading_reversals += 1;
                        }
                        self.previous_turn_sign[slot] = sign;
                    }
                }
                self.previous_heading[slot] = heading;
            }

            // Throughput gate.
            if let Some(gate) = config.throughput_gate {
                let side = gate_side(&gate, position);
                let previous = self.previous_gate_side[slot];
                if previous != 0 && side != 0 && side != previous {
                    self.gate_crossings += 1;
                }
                if side != 0 {
                    self.previous_gate_side[slot] = side;
                }
            }
        }

        if self.tick_min_time_to_collision < config.near_miss_time {
            self.near_misses += 1;
        }
    }

    /// Count newly arrived agents and record their travel time.
    pub fn record_arrivals(&mut self, world: &World, clock: &Clock) {
        self.ensure_capacity(world.len());
        for slot in 0..world.len() {
            if world.arrived[slot] && !self.counted_arrival[slot] {
                self.counted_arrival[slot] = true;
                self.arrived += 1;
                let ticks = clock.tick().saturating_sub(world.spawn_tick[slot]);
                self.travel_seconds
                    .push(ticks as f32 / clock.ticks_per_second() as f32);
            }
        }
    }

    pub fn penetration_events(&self) -> u64 {
        self.penetration_events
    }

    pub fn max_penetration_depth(&self) -> f32 {
        self.max_penetration_depth
    }

    pub fn penetration_agent_ticks(&self) -> u64 {
        self.penetration_agent_ticks
    }

    pub fn stalled_agents(&self) -> u64 {
        self.stalled_agents
    }

    pub fn heading_reversals(&self) -> u64 {
        self.heading_reversals
    }

    pub fn abrupt_turns(&self) -> u64 {
        self.abrupt_turns
    }

    pub fn arrived(&self) -> u64 {
        self.arrived
    }

    /// `(median, p95)` travel time in seconds. Zero when nobody arrived.
    pub fn summarize_travel_times(&self) -> (f32, f32) {
        if self.travel_seconds.is_empty() {
            return (0.0, 0.0);
        }
        let mut sorted = self.travel_seconds.clone();
        sorted.sort_by(f32::total_cmp);
        let median = sorted[sorted.len() / 2];
        let p95_index = ((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1);
        (median, sorted[p95_index])
    }

    pub fn summarize(
        &self,
        world: &World,
        scene: &CompiledScene,
        wall_time_seconds: f64,
        peak_allocated_bytes: u64,
    ) -> MetricsSummary {
        let (median_travel, p95_travel) = self.summarize_travel_times();
        let total = scene.total_agents().max(1) as f32;
        let phase_total: u64 = self.phase_nanos.iter().sum();

        MetricsSummary {
            ticks: self.ticks,
            agents_spawned: world.len() as u64,
            agents_arrived: self.arrived,
            completion_rate: self.arrived as f32 / total,
            median_travel_seconds: median_travel,
            p95_travel_seconds: p95_travel,

            penetration_events: self.penetration_events,
            max_penetration_depth: self.max_penetration_depth,
            penetration_agent_ticks: self.penetration_agent_ticks,

            min_time_to_collision: if self.min_time_to_collision.is_finite() {
                self.min_time_to_collision
            } else {
                // JSON has no infinity, and a sentinel that survives a round
                // trip beats a value that silently becomes null.
                -1.0
            },
            near_miss_ticks: self.near_misses,

            wall_corrections: self.wall_corrections,
            nonfinite_corrections: self.nonfinite_corrections,

            stalled_agents: self.stalled_agents,
            stall_agent_ticks: self.stall_agent_ticks,

            heading_reversals: self.heading_reversals,
            abrupt_turns: self.abrupt_turns,
            gate_crossings: self.gate_crossings,

            wall_time_seconds,
            ticks_per_second_achieved: if wall_time_seconds > 0.0 {
                self.ticks as f64 / wall_time_seconds
            } else {
                0.0
            },
            peak_allocated_bytes,

            phase_time_shares: Phase::ALL
                .iter()
                .map(|phase| PhaseShare {
                    phase: phase.name().to_string(),
                    nanos: self.phase_nanos[phase.index()],
                    share: if phase_total > 0 {
                        self.phase_nanos[phase.index()] as f32 / phase_total as f32
                    } else {
                        0.0
                    },
                })
                .collect(),
        }
    }
}

/// Which side of the gate line a point is on: `-1`, `0`, or `1`.
fn gate_side(gate: &Segment, p: Vec2) -> i8 {
    let along = gate.b - gate.a;
    let to_point = p - gate.a;
    let cross = along.x * to_point.y - along.y * to_point.x;
    if cross > 0.0 {
        1
    } else if cross < 0.0 {
        -1
    } else {
        0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseShare {
    pub phase: String,
    pub nanos: u64,
    pub share: f32,
}

/// The report schema. Field names are baseline keys — renaming one invalidates
/// every checked-in baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub ticks: u64,
    pub agents_spawned: u64,
    pub agents_arrived: u64,
    pub completion_rate: f32,
    pub median_travel_seconds: f32,
    pub p95_travel_seconds: f32,

    pub penetration_events: u64,
    pub max_penetration_depth: f32,
    pub penetration_agent_ticks: u64,

    /// `-1.0` means no collision was ever predicted.
    pub min_time_to_collision: f32,
    pub near_miss_ticks: u64,

    pub wall_corrections: u64,
    pub nonfinite_corrections: u64,

    pub stalled_agents: u64,
    pub stall_agent_ticks: u64,

    pub heading_reversals: u64,
    pub abrupt_turns: u64,
    pub gate_crossings: u64,

    pub wall_time_seconds: f64,
    pub ticks_per_second_achieved: f64,
    pub peak_allocated_bytes: u64,

    pub phase_time_shares: Vec<PhaseShare>,
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod metrics;` and `pub use metrics::{Metrics, MetricsConfig, MetricsSummary, Phase};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core metrics`
Expected: PASS, 9 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/metrics.rs crates/crowd-core/src/lib.rs
git commit -m "Add quality and performance metrics accumulation"
```

---

## Task 18: Simulation loop

**Files:**
- Create: `crates/crowd-core/src/sim.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: everything from Tasks 2–17.
- Produces: `SimConfig { perceive: PerceiveConfig, decide: DecideConfig, steer: SteerConfig, integrate: IntegrateConfig, metrics: MetricsConfig, grid_cell_size: f32 }` with `Default`. `Simulation::new(scene: CompiledScene, solver: Box<dyn AvoidanceSolver>, config: SimConfig) -> Simulation`, with `step(&mut self)`, `run(&mut self, ticks: u64)`, `run_to_completion(&mut self)`, `world(&self) -> &World`, `clock(&self) -> &Clock`, `scene(&self) -> &CompiledScene`, `metrics(&self) -> &Metrics`, `solver_name(&self) -> &'static str`, `state_hash(&self) -> u64`, `spawn_errors(&self) -> &[SpawnError]`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/sim.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::route::WaypointGraph;
    use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
    use crate::units::{Aabb, Vec2};

    fn corridor(count: u32) -> CompiledScene {
        let mut waypoints = WaypointGraph::new();
        let a = waypoints.add_node(Vec2::new(2.0, 5.0));
        let b = waypoints.add_node(Vec2::new(18.0, 5.0));
        waypoints.add_edge(a, b);
        SceneDef {
            name: "sim_corridor".into(),
            bounds: Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(20.0, 10.0)),
            walls: vec![
                Segment::new(Vec2::new(0.0, 2.0), Vec2::new(20.0, 2.0)),
                Segment::new(Vec2::new(0.0, 8.0), Vec2::new(20.0, 8.0)),
            ],
            waypoints,
            destinations: vec![Destination { name: "exit".into(), node: b }],
            spawns: vec![SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(1.0, 3.0), Vec2::new(3.0, 7.0)),
                count,
                per_tick: count.min(20),
                destination: 0,
            }],
            populations: vec![PopulationParams::default()],
            project_seed: 2026,
            ticks_per_second: 30,
            duration_ticks: 900,
        }
        .compile()
        .unwrap()
    }

    fn simulation(count: u32) -> Simulation {
        Simulation::new(
            corridor(count),
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        )
    }

    #[test]
    fn a_new_simulation_starts_empty_at_tick_zero() {
        let sim = simulation(10);
        assert_eq!(sim.clock().tick(), 0);
        assert_eq!(sim.world().len(), 0);
    }

    #[test]
    fn stepping_spawns_agents_and_advances_the_clock() {
        let mut sim = simulation(10);
        sim.step();
        assert_eq!(sim.clock().tick(), 1);
        assert_eq!(sim.world().len(), 10);
        assert!(sim.spawn_errors().is_empty());
    }

    #[test]
    fn agents_make_progress_toward_their_destination() {
        let mut sim = simulation(20);
        sim.step();
        let start_x: f32 = sim.world().pos_x.iter().sum::<f32>() / 20.0;
        sim.run(120);
        let end_x: f32 = sim.world().pos_x.iter().sum::<f32>() / 20.0;
        assert!(end_x > start_x + 2.0, "agents did not advance: {start_x} to {end_x}");
    }

    #[test]
    fn agents_eventually_arrive() {
        let mut sim = simulation(20);
        sim.run_to_completion();
        assert!(sim.metrics().arrived() > 0, "nobody reached the destination");
    }

    #[test]
    fn agents_stay_inside_the_corridor_walls() {
        let mut sim = simulation(50);
        sim.run(300);
        for slot in 0..sim.world().len() {
            let y = sim.world().pos_y[slot];
            assert!(
                (1.5..=8.5).contains(&y),
                "slot {slot} escaped the corridor at y={y}"
            );
        }
    }

    #[test]
    fn all_state_stays_finite() {
        let mut sim = simulation(100);
        sim.run(300);
        for slot in 0..sim.world().len() {
            assert!(sim.world().position(slot as u32).is_finite());
            assert!(sim.world().velocity(slot as u32).is_finite());
            assert!(sim.world().yaw[slot].is_finite());
        }
    }

    #[test]
    fn identical_runs_produce_identical_state_hashes() {
        let mut a = simulation(50);
        let mut b = simulation(50);
        a.run(200);
        b.run(200);
        assert_eq!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn state_hashes_match_at_every_tick() {
        // A single end-state comparison can hide a divergence that later
        // reconverges, so compare the whole trajectory.
        let mut a = simulation(30);
        let mut b = simulation(30);
        for tick in 0..150 {
            a.step();
            b.step();
            assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {tick}");
        }
    }

    #[test]
    fn run_to_completion_stops_at_the_scene_duration() {
        let mut sim = simulation(5);
        sim.run_to_completion();
        assert!(sim.clock().tick() <= sim.scene().duration_ticks);
    }

    #[test]
    fn phase_timings_are_recorded() {
        let mut sim = simulation(10);
        sim.run(10);
        assert!(sim.metrics().phase_nanos(Phase::Steer) > 0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core sim`
Expected: FAIL — `cannot find type Simulation in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/sim.rs`:

```rust
//! The fixed-step tick loop.
//!
//! Phases run in the fixed order of contract section 6, with `commit` at the
//! end publishing staged next-state. Until then every phase reads a consistent
//! previous-tick snapshot, which is what makes results independent of the
//! order agents are visited.
//!
//! `Animate` is omitted rather than stubbed: there is no clip data to select
//! from in this slice.

use std::time::Instant;

use crate::arena::NeighborArena;
use crate::avoidance::AvoidanceSolver;
use crate::clock::Clock;
use crate::geometry::Segment;
use crate::grid::UniformGrid;
use crate::metrics::{Metrics, MetricsConfig, Phase};
use crate::phases::decide::{decide, DecideConfig};
use crate::phases::integrate::{integrate, IntegrateConfig, IntegrateScratch};
use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
use crate::phases::spawn::{apply_spawns, SpawnState};
use crate::phases::steer::{steer, SteerConfig, SteerScratch};
use crate::route::RouteArena;
use crate::scene::CompiledScene;
use crate::world::{SpawnError, World};

#[derive(Clone, Debug, Default)]
pub struct SimConfig {
    pub perceive: PerceiveConfig,
    pub decide: DecideConfig,
    pub steer: SteerConfig,
    pub integrate: IntegrateConfig,
    pub metrics: MetricsConfig,
    /// Grid cell size. Zero means "derive from the perception radius", which
    /// is the right default: cells much smaller than the query radius make
    /// every query touch many cells.
    pub grid_cell_size: f32,
}

pub struct Simulation {
    scene: CompiledScene,
    solver: Box<dyn AvoidanceSolver>,
    config: SimConfig,

    world: World,
    clock: Clock,
    routes: RouteArena,
    spawn_state: SpawnState,
    spawn_errors: Vec<SpawnError>,

    grid: UniformGrid,
    neighbors: NeighborArena,
    perceive_scratch: PerceiveScratch,
    steer_scratch: SteerScratch,
    integrate_scratch: IntegrateScratch,

    metrics: Metrics,
}

impl Simulation {
    pub fn new(
        scene: CompiledScene,
        solver: Box<dyn AvoidanceSolver>,
        config: SimConfig,
    ) -> Self {
        let cell_size = if config.grid_cell_size > 0.0 {
            config.grid_cell_size
        } else {
            config.perceive.query_radius
        };
        // Expand the grid past the scene bounds so an agent that slips outside
        // still lands in a real cell rather than being clamped onto the edge
        // with every other escapee.
        let grid = UniformGrid::new(scene.bounds.expanded(cell_size * 2.0), cell_size);
        let spawn_state = SpawnState::new(&scene);
        let clock = Clock::new(scene.ticks_per_second);

        Self {
            scene,
            solver,
            config,
            world: World::new(),
            clock,
            routes: RouteArena::new(),
            spawn_state,
            spawn_errors: Vec::new(),
            grid,
            neighbors: NeighborArena::new(),
            perceive_scratch: PerceiveScratch::default(),
            steer_scratch: SteerScratch::default(),
            integrate_scratch: IntegrateScratch::default(),
            metrics: Metrics::new(),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    pub fn scene(&self) -> &CompiledScene {
        &self.scene
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn routes(&self) -> &RouteArena {
        &self.routes
    }

    pub fn solver_name(&self) -> &'static str {
        self.solver.name()
    }

    pub fn spawn_errors(&self) -> &[SpawnError] {
        &self.spawn_errors
    }

    pub fn state_hash(&self) -> u64 {
        self.world.state_hash()
    }

    /// Advance one tick through the fixed phase order.
    ///
    /// Timing uses `Instant`, which is wall-clock and therefore varies between
    /// runs. It only ever feeds the metrics report, never a simulation
    /// decision, so determinism is unaffected.
    pub fn step(&mut self) {
        self.metrics.begin_tick();

        let start = Instant::now();
        let errors = apply_spawns(
            &self.scene,
            &mut self.spawn_state,
            &mut self.world,
            &mut self.routes,
            self.clock.tick(),
        );
        self.spawn_errors.extend(errors);
        self.metrics
            .record_phase(Phase::Spawn, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        self.grid.rebuild(&self.world.pos_x, &self.world.pos_y);
        self.metrics
            .record_phase(Phase::Index, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        perceive(
            &self.world,
            &self.grid,
            &self.config.perceive,
            &mut self.perceive_scratch,
            &mut self.neighbors,
        );
        self.metrics
            .record_phase(Phase::Perceive, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        decide(&mut self.world, &self.routes, &self.config.decide);
        self.metrics
            .record_phase(Phase::Decide, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        let steer_report = steer(
            &mut self.world,
            &self.neighbors,
            &self.scene,
            self.solver.as_ref(),
            &self.config.steer,
            &mut self.steer_scratch,
        );
        self.metrics
            .record_phase(Phase::Steer, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        let integrate_report = integrate(
            &mut self.world,
            &self.scene,
            &self.config.integrate,
            self.clock.dt(),
            &mut self.integrate_scratch,
        );
        self.world.commit();
        self.metrics
            .record_phase(Phase::Integrate, start.elapsed().as_nanos() as u64);

        let start = Instant::now();
        self.metrics.record_steer(&steer_report);
        self.metrics.record_integrate(&integrate_report);
        self.metrics.observe_tick(
            &self.world,
            &self.neighbors,
            &self.clock,
            &self.config.metrics,
        );
        self.clock.advance();
        self.metrics.record_arrivals(&self.world, &self.clock);
        self.metrics
            .record_phase(Phase::Metrics, start.elapsed().as_nanos() as u64);
    }

    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// Run until the scene's declared duration elapses.
    pub fn run_to_completion(&mut self) {
        while self.clock.tick() < self.scene.duration_ticks {
            self.step();
        }
    }

    /// Wall segments, for the SVG dump.
    pub fn walls(&self) -> &[Segment] {
        &self.scene.walls
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod sim;` and `pub use sim::{SimConfig, Simulation};`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core sim`
Expected: PASS, 10 tests.

If `agents_stay_inside_the_corridor_walls` fails, that is a real avoidance or integration defect, not a bad test — investigate before loosening the bound.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/sim.rs crates/crowd-core/src/lib.rs
git commit -m "Add fixed-step simulation loop wiring all tick phases"
```

---

## Task 19: Benchmark scenes

**Files:**
- Create: `crates/crowd-core/src/scenes.rs`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Consumes: `SceneDef`, `WaypointGraph`, `Segment`, `Aabb`, `Vec2`, `Destination`, `SpawnRegion`, `PopulationParams`.
- Produces: `pub const SCENE_NAMES: [&str; 5]`; `pub fn build(name: &str, agents: u32, seed: u64) -> Option<SceneDef>`; `pub fn throughput_gate(name: &str) -> Option<Segment>`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/src/scenes.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoidance::SampledVelocitySolver;
    use crate::sim::{SimConfig, Simulation};

    #[test]
    fn every_named_scene_builds() {
        for name in SCENE_NAMES {
            assert!(build(name, 100, 42).is_some(), "{name} did not build");
        }
    }

    #[test]
    fn an_unknown_scene_name_returns_none() {
        assert!(build("no_such_scene", 100, 42).is_none());
    }

    #[test]
    fn every_named_scene_compiles_without_diagnostics() {
        for name in SCENE_NAMES {
            let scene = build(name, 100, 42).unwrap();
            if let Err(errors) = scene.compile() {
                panic!("{name} failed to compile: {errors:?}");
            }
        }
    }

    #[test]
    fn every_scene_spawns_the_requested_agent_count() {
        for name in SCENE_NAMES {
            let compiled = build(name, 200, 42).unwrap().compile().unwrap();
            assert_eq!(compiled.total_agents(), 200, "{name} miscounted");
        }
    }

    #[test]
    fn every_scene_runs_without_producing_nonfinite_state() {
        for name in SCENE_NAMES {
            let compiled = build(name, 100, 42).unwrap().compile().unwrap();
            let mut sim = Simulation::new(
                compiled,
                Box::new(SampledVelocitySolver::default()),
                SimConfig::default(),
            );
            sim.run(200);
            for slot in 0..sim.world().len() {
                assert!(
                    sim.world().position(slot as u32).is_finite(),
                    "{name} slot {slot} went non-finite"
                );
            }
        }
    }

    #[test]
    fn agents_reach_destinations_in_every_scene() {
        for name in SCENE_NAMES {
            let compiled = build(name, 100, 42).unwrap().compile().unwrap();
            let mut sim = Simulation::new(
                compiled,
                Box::new(SampledVelocitySolver::default()),
                SimConfig::default(),
            );
            sim.run_to_completion();
            assert!(
                sim.metrics().arrived() > 0,
                "{name}: nobody reached a destination"
            );
        }
    }

    #[test]
    fn the_bottleneck_scene_has_a_throughput_gate() {
        assert!(throughput_gate("bottleneck").is_some());
        assert!(throughput_gate("circle").is_none());
    }

    #[test]
    fn scene_agent_counts_split_evenly_across_spawn_regions() {
        // A count that does not divide evenly must still total exactly.
        let compiled = build("crossing", 101, 42).unwrap().compile().unwrap();
        assert_eq!(compiled.total_agents(), 101);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-core scenes`
Expected: FAIL — `cannot find function build in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-core/src/scenes.rs`:

```rust
//! The five benchmark scenes.
//!
//! Chosen to cover the failure modes contract section 6.2 names: lane
//! formation, perpendicular conflict, doorway congestion, dense convergence,
//! and the antipodal swap that is the cheapest known exposure of oscillation
//! and deadlock.

use crate::geometry::Segment;
use crate::route::WaypointGraph;
use crate::scene::{Destination, PopulationParams, SceneDef, SpawnRegion};
use crate::units::{Aabb, Vec2};

pub const SCENE_NAMES: [&str; 5] = [
    "bidirectional_corridor",
    "crossing",
    "bottleneck",
    "dense_flow",
    "circle",
];

pub fn build(name: &str, agents: u32, seed: u64) -> Option<SceneDef> {
    match name {
        "bidirectional_corridor" => Some(bidirectional_corridor(agents, seed)),
        "crossing" => Some(crossing(agents, seed)),
        "bottleneck" => Some(bottleneck(agents, seed)),
        "dense_flow" => Some(dense_flow(agents, seed)),
        "circle" => Some(circle(agents, seed)),
        _ => None,
    }
}

/// The line crossings are counted against, for scenes where throughput is the
/// interesting measure.
pub fn throughput_gate(name: &str) -> Option<Segment> {
    match name {
        "bottleneck" => Some(Segment::new(Vec2::new(20.0, 8.0), Vec2::new(20.0, 12.0))),
        "dense_flow" => Some(Segment::new(Vec2::new(28.0, 12.0), Vec2::new(28.0, 18.0))),
        _ => None,
    }
}

/// Split `total` across `parts` so the counts sum to exactly `total`.
///
/// The naive `total / parts` silently drops the remainder, which would make a
/// requested 1,000-agent benchmark quietly run 992 agents.
fn split_count(total: u32, parts: u32, index: u32) -> u32 {
    let base = total / parts;
    let remainder = total % parts;
    base + u32::from(index < remainder)
}

fn box_walls(bounds: Aabb) -> Vec<Segment> {
    let Aabb { min, max } = bounds;
    vec![
        Segment::new(min, Vec2::new(max.x, min.y)),
        Segment::new(Vec2::new(max.x, min.y), max),
        Segment::new(max, Vec2::new(min.x, max.y)),
        Segment::new(Vec2::new(min.x, max.y), min),
    ]
}

/// Two opposing flows down one corridor. Lane formation is the thing to watch.
fn bidirectional_corridor(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 8.0));
    let mut waypoints = WaypointGraph::new();
    let west = waypoints.add_node(Vec2::new(2.0, 4.0));
    let east = waypoints.add_node(Vec2::new(38.0, 4.0));
    waypoints.add_edge(west, east);

    SceneDef {
        name: "bidirectional_corridor".into(),
        bounds,
        walls: vec![
            Segment::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.0)),
            Segment::new(Vec2::new(0.0, 8.0), Vec2::new(40.0, 8.0)),
        ],
        waypoints,
        destinations: vec![
            Destination { name: "west".into(), node: west },
            Destination { name: "east".into(), node: east },
        ],
        spawns: vec![
            SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 0.5), Vec2::new(6.0, 7.5)),
                count: split_count(agents, 2, 0),
                per_tick: 4,
                destination: 1,
            },
            SpawnRegion {
                id: 1,
                population_id: 0,
                area: Aabb::new(Vec2::new(34.0, 0.5), Vec2::new(39.5, 7.5)),
                count: split_count(agents, 2, 1),
                per_tick: 4,
                destination: 0,
            },
        ],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 1800,
    }
}

/// Two perpendicular flows through a shared plaza.
fn crossing(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 40.0));
    let mut waypoints = WaypointGraph::new();
    let centre = waypoints.add_node(Vec2::new(20.0, 20.0));
    let west = waypoints.add_node(Vec2::new(2.0, 20.0));
    let east = waypoints.add_node(Vec2::new(38.0, 20.0));
    let south = waypoints.add_node(Vec2::new(20.0, 2.0));
    let north = waypoints.add_node(Vec2::new(20.0, 38.0));
    for node in [west, east, south, north] {
        waypoints.add_edge(centre, node);
    }

    SceneDef {
        name: "crossing".into(),
        bounds,
        walls: box_walls(bounds),
        waypoints,
        destinations: vec![
            Destination { name: "east".into(), node: east },
            Destination { name: "north".into(), node: north },
        ],
        spawns: vec![
            SpawnRegion {
                id: 0,
                population_id: 0,
                area: Aabb::new(Vec2::new(0.5, 15.0), Vec2::new(6.0, 25.0)),
                count: split_count(agents, 2, 0),
                per_tick: 4,
                destination: 0,
            },
            SpawnRegion {
                id: 1,
                population_id: 0,
                area: Aabb::new(Vec2::new(15.0, 0.5), Vec2::new(25.0, 6.0)),
                count: split_count(agents, 2, 1),
                per_tick: 4,
                destination: 1,
            },
        ],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 1800,
    }
}

/// One room emptying into another through a 1.6 m doorway.
fn bottleneck(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(8.0, 10.0));
    let approach = waypoints.add_node(Vec2::new(18.5, 10.0));
    let through = waypoints.add_node(Vec2::new(21.5, 10.0));
    let exit = waypoints.add_node(Vec2::new(34.0, 10.0));
    waypoints.add_edge(start, approach);
    waypoints.add_edge(approach, through);
    waypoints.add_edge(through, exit);

    let mut walls = box_walls(bounds);
    // The divider, with a gap from y = 9.2 to y = 10.8.
    walls.push(Segment::new(Vec2::new(20.0, 0.0), Vec2::new(20.0, 9.2)));
    walls.push(Segment::new(Vec2::new(20.0, 10.8), Vec2::new(20.0, 20.0)));

    SceneDef {
        name: "bottleneck".into(),
        bounds,
        walls,
        waypoints,
        destinations: vec![Destination { name: "exit".into(), node: exit }],
        spawns: vec![SpawnRegion {
            id: 0,
            population_id: 0,
            area: Aabb::new(Vec2::new(1.0, 2.0), Vec2::new(14.0, 18.0)),
            count: agents,
            per_tick: 8,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 3600,
    }
}

/// A dense blob converging on a single exit corridor.
fn dense_flow(agents: u32, seed: u64) -> SceneDef {
    let bounds = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 30.0));
    let mut waypoints = WaypointGraph::new();
    let start = waypoints.add_node(Vec2::new(12.0, 15.0));
    let mouth = waypoints.add_node(Vec2::new(26.0, 15.0));
    let exit = waypoints.add_node(Vec2::new(38.0, 15.0));
    waypoints.add_edge(start, mouth);
    waypoints.add_edge(mouth, exit);

    let mut walls = box_walls(bounds);
    // A funnel narrowing to a 6 m mouth.
    walls.push(Segment::new(Vec2::new(24.0, 0.0), Vec2::new(28.0, 12.0)));
    walls.push(Segment::new(Vec2::new(24.0, 30.0), Vec2::new(28.0, 18.0)));
    walls.push(Segment::new(Vec2::new(28.0, 12.0), Vec2::new(40.0, 12.0)));
    walls.push(Segment::new(Vec2::new(28.0, 18.0), Vec2::new(40.0, 18.0)));

    SceneDef {
        name: "dense_flow".into(),
        bounds,
        walls,
        waypoints,
        destinations: vec![Destination { name: "exit".into(), node: exit }],
        spawns: vec![SpawnRegion {
            id: 0,
            population_id: 0,
            area: Aabb::new(Vec2::new(1.0, 2.0), Vec2::new(18.0, 28.0)),
            count: agents,
            per_tick: 12,
            destination: 0,
        }],
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 3600,
    }
}

/// Agents on a ring walking to the antipodal point.
///
/// Everyone converges on the centre simultaneously with perfect symmetry,
/// which is why this is the standard oscillation and deadlock test.
fn circle(agents: u32, seed: u64) -> SceneDef {
    const SECTORS: u32 = 16;
    const RADIUS: f32 = 15.0;
    let bounds = Aabb::new(Vec2::new(-20.0, -20.0), Vec2::new(20.0, 20.0));

    let mut waypoints = WaypointGraph::new();
    let centre = waypoints.add_node(Vec2::ZERO);
    let mut perimeter = Vec::new();
    for sector in 0..SECTORS {
        let angle = std::f32::consts::TAU * sector as f32 / SECTORS as f32;
        let node = waypoints.add_node(Vec2::from_yaw(angle) * RADIUS);
        waypoints.add_edge(centre, node);
        perimeter.push(node);
    }

    let destinations: Vec<Destination> = (0..SECTORS)
        .map(|sector| Destination {
            name: format!("sector_{sector}"),
            node: perimeter[sector as usize],
        })
        .collect();

    let spawns: Vec<SpawnRegion> = (0..SECTORS)
        .map(|sector| {
            let angle = std::f32::consts::TAU * sector as f32 / SECTORS as f32;
            let centre_point = Vec2::from_yaw(angle) * RADIUS;
            SpawnRegion {
                id: sector as u16,
                population_id: 0,
                area: Aabb::new(
                    Vec2::new(centre_point.x - 1.5, centre_point.y - 1.5),
                    Vec2::new(centre_point.x + 1.5, centre_point.y + 1.5),
                ),
                count: split_count(agents, SECTORS, sector),
                per_tick: 4,
                // The antipodal sector.
                destination: ((sector + SECTORS / 2) % SECTORS) as u16,
            }
        })
        .collect();

    SceneDef {
        name: "circle".into(),
        bounds,
        walls: Vec::new(),
        waypoints,
        destinations,
        spawns,
        populations: vec![PopulationParams::default()],
        project_seed: seed,
        ticks_per_second: 30,
        duration_ticks: 1800,
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/crowd-core/src/lib.rs` add `pub mod scenes;`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-core scenes`
Expected: PASS, 8 tests.

If `agents_reach_destinations_in_every_scene` fails for `circle`, that is a genuine deadlock at the centre and the finding belongs in the report — investigate the solver before changing the scene. If it fails for `dense_flow`, check that the funnel walls actually leave a gap; a closed funnel is a scene bug.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/src/scenes.rs crates/crowd-core/src/lib.rs
git commit -m "Add the five benchmark scenes"
```

---

## Task 20: Determinism tests

This is the task that makes the whole determinism contract real rather than aspirational.

**Files:**
- Create: `crates/crowd-core/tests/determinism.rs`

**Interfaces:**
- Consumes: `crowd_core::{scenes, sim::{SimConfig, Simulation}, avoidance::SampledVelocitySolver, ids::AgentId}`.
- Produces: nothing; it is a test target.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-core/tests/determinism.rs`:

```rust
//! The determinism contract, contract section 9.4 `Strict` mode.
//!
//! The claim is bitwise-identical output for the same binary on the same
//! machine. Cross-machine identity is not claimed.

use std::collections::BTreeMap;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::ids::AgentId;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::World;

fn simulate(scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
    let scene = scenes::build(scene_name, agents, seed)
        .expect("known scene")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    sim.run(ticks);
    sim
}

/// Per-agent state keyed by stable ID, so slot layout cannot affect the
/// comparison.
fn state_by_id(world: &World) -> BTreeMap<AgentId, (u32, u32, u32, u32, u32, u16, bool)> {
    (0..world.len())
        .map(|slot| {
            (
                world.agent_id[slot],
                (
                    world.pos_x[slot].to_bits(),
                    world.pos_y[slot].to_bits(),
                    world.vel_x[slot].to_bits(),
                    world.vel_y[slot].to_bits(),
                    world.yaw[slot].to_bits(),
                    world.route_index[slot],
                    world.arrived[slot],
                ),
            )
        })
        .collect()
}

#[test]
fn repeated_runs_are_bitwise_identical_in_every_scene() {
    for name in scenes::SCENE_NAMES {
        let a = simulate(name, 200, 2026, 300);
        let b = simulate(name, 200, 2026, 300);
        assert_eq!(a.state_hash(), b.state_hash(), "{name} diverged");
        assert_eq!(state_by_id(a.world()), state_by_id(b.world()), "{name}");
    }
}

#[test]
fn state_hashes_agree_at_every_tick() {
    // An end-state comparison can hide a divergence that later reconverges.
    let scene = |seed| {
        scenes::build("bottleneck", 150, seed)
            .unwrap()
            .compile()
            .unwrap()
    };
    let mut a = Simulation::new(
        scene(7),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut b = Simulation::new(
        scene(7),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    for tick in 0..400 {
        a.step();
        b.step();
        assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {tick}");
    }
}

#[test]
fn permuting_spawn_region_order_does_not_change_results() {
    // Reversing the spawn regions changes every agent's slot, so any result
    // that depends on iteration order will differ. Comparing by stable ID
    // isolates that from the legitimate change in slot layout.
    let mut forward = scenes::build("bidirectional_corridor", 200, 99).unwrap();
    let mut reversed = forward.clone();
    reversed.spawns.reverse();

    forward.duration_ticks = 300;
    reversed.duration_ticks = 300;

    let mut a = Simulation::new(
        forward.compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    let mut b = Simulation::new(
        reversed.compile().unwrap(),
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    a.run(300);
    b.run(300);

    assert_eq!(
        state_by_id(a.world()),
        state_by_id(b.world()),
        "results depended on spawn region ordering"
    );
}

#[test]
fn adding_one_agent_does_not_change_existing_agents_attributes() {
    // Contract section 4.2. Trajectories legitimately differ once the extra
    // agent interacts, so this compares derived attributes at spawn.
    let small = simulate("crossing", 100, 5, 1);
    let large = simulate("crossing", 101, 5, 1);

    let attributes = |sim: &Simulation| -> BTreeMap<AgentId, (u32, u32)> {
        let world = sim.world();
        (0..world.len())
            .map(|slot| {
                (
                    world.agent_id[slot],
                    (
                        world.radius[slot].to_bits(),
                        world.preferred_speed[slot].to_bits(),
                    ),
                )
            })
            .collect()
    };

    let small_attributes = attributes(&small);
    let large_attributes = attributes(&large);

    assert!(!small_attributes.is_empty(), "no agents spawned");
    for (id, expected) in &small_attributes {
        assert_eq!(
            large_attributes.get(id),
            Some(expected),
            "agent {id:?} was reshuffled by adding another agent"
        );
    }
}

#[test]
fn changing_the_seed_changes_the_outcome() {
    // Guards against a determinism implementation so aggressive it ignores the
    // seed entirely, which would pass every other test in this file.
    let a = simulate("crossing", 200, 1, 200);
    let b = simulate("crossing", 200, 2, 200);
    assert_ne!(a.state_hash(), b.state_hash());
}

#[test]
fn no_spawn_errors_occur_in_any_scene() {
    for name in scenes::SCENE_NAMES {
        let sim = simulate(name, 500, 3, 100);
        assert!(
            sim.spawn_errors().is_empty(),
            "{name}: {:?}",
            sim.spawn_errors()
        );
    }
}
```

- [ ] **Step 2: Derive `Clone` on `SceneDef` if needed**

`permuting_spawn_region_order_does_not_change_results` calls `.clone()` on a `SceneDef`. Task 10 already derives `Clone`; if the compiler disagrees, add it there rather than restructuring the test.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p crowd-core --test determinism`
Expected: PASS, 6 tests.

A failure here is never a test bug to be relaxed. Diagnose it as a real determinism defect. The usual causes, in order of likelihood:

1. A `HashMap` or `HashSet` iterated somewhere in the tick path — Rust randomises its hash seed per process, so this fails across runs but often passes within one.
2. A sort without a stable tiebreak, so equal keys resolve by prior order.
3. Reading `Instant`, thread identity, or a pointer value into a simulation decision rather than into metrics.
4. Accumulating `dt` rather than using the constant.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add crates/crowd-core/tests/determinism.rs
git commit -m "Add determinism tests for bitwise, ordering, and seeding contracts"
```

---

## Task 21: Benchmark binary, allocator, and report

**Files:**
- Create: `crates/crowd-bench/Cargo.toml`
- Create: `crates/crowd-bench/build.rs`
- Create: `crates/crowd-bench/src/alloc.rs`
- Create: `crates/crowd-bench/src/report.rs`
- Create: `crates/crowd-bench/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

**Interfaces:**
- Consumes: `crowd_core::{scenes, sim::{SimConfig, Simulation}, avoidance::SampledVelocitySolver, metrics::MetricsSummary}`.
- Produces: `alloc::{CountingAllocator, peak_bytes, reset_peak}`; `report::{Environment, Report, RunOptions, run_scene}` where `run_scene(options: &RunOptions) -> Result<Report, String>` and `RunOptions { scene: String, agents: u32, seed: u64, svg: bool, out_dir: PathBuf }`.

- [ ] **Step 1: Write the crate manifest and workspace entry**

`crates/crowd-bench/Cargo.toml`:

```toml
[package]
name = "crowd-bench"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
crowd-core = { path = "../crowd-core" }
serde = { workspace = true }
serde_json = { workspace = true }
```

In the root `Cargo.toml`, change members to:

```toml
members = ["crates/crowd-core", "crates/crowd-bench"]
```

- [ ] **Step 2: Write the build script**

`crates/crowd-bench/build.rs`. This records the rustc version as report metadata only. It must never gate compilation — an unparseable version string degrades to `"unknown"`, it does not fail the build.

```rust
use std::process::Command;

fn main() {
    let version = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CROWD_RUSTC_VERSION={version}");
    println!("cargo:rerun-if-changed=build.rs");
}
```

- [ ] **Step 3: Write the failing test for the allocator**

Create `crates/crowd-bench/src/alloc.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_tracks_the_largest_live_allocation() {
        reset_peak();
        let before = peak_bytes();
        let big: Vec<u8> = vec![0; 4 * 1024 * 1024];
        let after_alloc = peak_bytes();
        drop(big);
        let after_drop = peak_bytes();
        assert!(after_alloc >= before + 4 * 1024 * 1024);
        assert_eq!(after_drop, after_alloc, "peak must not fall when freed");
    }

    #[test]
    fn reset_peak_lowers_the_high_water_mark() {
        let big: Vec<u8> = vec![0; 2 * 1024 * 1024];
        let high = peak_bytes();
        drop(big);
        reset_peak();
        assert!(peak_bytes() < high);
    }
}
```

- [ ] **Step 4: Write the allocator**

Prepend to `crates/crowd-bench/src/alloc.rs`:

```rust
//! Allocation accounting for the memory metric.
//!
//! Reports *peak allocated bytes*, not resident set size. A counting allocator
//! avoids platform-specific RSS APIs and is itself deterministic, at the cost
//! of excluding allocator overhead and static data. Stating which number is
//! being reported matters more than reporting the larger one.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            // Track the net change so a shrink is not counted as growth.
            let live = if new_size >= layout.size() {
                LIVE.fetch_add(new_size - layout.size(), Ordering::Relaxed)
                    + (new_size - layout.size())
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed)
                    - (layout.size() - new_size)
            };
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        new_ptr
    }
}

pub fn peak_bytes() -> usize {
    PEAK.load(Ordering::Relaxed)
}

pub fn live_bytes() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// Drop the high-water mark to the current live total.
///
/// Called immediately before a measured run so setup allocations do not count
/// against the simulation's memory figure.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}
```

- [ ] **Step 5: Write the failing test for the report**

Create `crates/crowd-bench/src/report.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn options(scene: &str, agents: u32) -> RunOptions {
        RunOptions {
            scene: scene.to_string(),
            agents,
            seed: 2026,
            svg: false,
            out_dir: std::env::temp_dir().join("crowd_bench_test"),
        }
    }

    #[test]
    fn running_a_known_scene_produces_a_report() {
        let report = run_scene(&options("bidirectional_corridor", 50)).unwrap();
        assert_eq!(report.scene, "bidirectional_corridor");
        assert_eq!(report.requested_agents, 50);
        assert_eq!(report.solver, "sampled_velocity");
        assert!(report.metrics.ticks > 0);
    }

    #[test]
    fn running_an_unknown_scene_reports_an_error() {
        let error = run_scene(&options("nope", 10)).unwrap_err();
        assert!(error.contains("nope"), "unhelpful error: {error}");
    }

    #[test]
    fn the_report_round_trips_through_json() {
        let report = run_scene(&options("crossing", 30)).unwrap();
        let json = serde_json::to_string_pretty(&report).unwrap();
        let parsed: Report = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, report);
    }

    #[test]
    fn identical_runs_produce_identical_quality_metrics() {
        // Timing fields legitimately vary; quality fields must not, because
        // the simulation is deterministic.
        let a = run_scene(&options("bottleneck", 40)).unwrap();
        let b = run_scene(&options("bottleneck", 40)).unwrap();
        assert_eq!(a.final_state_hash, b.final_state_hash);
        assert_eq!(a.metrics.penetration_events, b.metrics.penetration_events);
        assert_eq!(a.metrics.agents_arrived, b.metrics.agents_arrived);
        assert_eq!(a.metrics.heading_reversals, b.metrics.heading_reversals);
    }

    #[test]
    fn the_environment_is_captured() {
        let environment = Environment::capture();
        assert!(!environment.os.is_empty());
        assert!(!environment.arch.is_empty());
        assert!(!environment.rustc_version.is_empty());
    }

    #[test]
    fn the_report_records_the_scene_hash() {
        let report = run_scene(&options("circle", 32)).unwrap();
        assert_ne!(report.scene_hash, 0);
    }
}
```

- [ ] **Step 6: Write the report implementation**

Prepend to `crates/crowd-bench/src/report.rs`:

```rust
//! Benchmark execution and the report schema.
//!
//! Reports record the environment contract section 8.3 requires, so a metrics
//! number can never be quoted without the machine it came from.

use std::path::PathBuf;
use std::time::Instant;

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::metrics::{MetricsConfig, MetricsSummary};
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use serde::{Deserialize, Serialize};

use crate::alloc;

/// Bumped whenever the report schema changes incompatibly.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub svg: bool,
    pub out_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub rustc_version: String,
    pub build_profile: String,
}

impl Environment {
    pub fn capture() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu: detect_cpu(),
            ram_bytes: detect_ram_bytes(),
            rustc_version: env!("CROWD_RUSTC_VERSION").to_string(),
            build_profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
        }
    }
}

/// Best-effort CPU name. Unknown is acceptable; a wrong value is not.
///
/// `Command` is imported inside each `cfg` block rather than at module scope so
/// a platform with no detection path does not trip the unused-import lint,
/// which `clippy -D warnings` treats as an error.
fn detect_cpu() -> String {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if let Ok(text) = String::from_utf8(out.stdout) {
                let text = text.trim();
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("model name") {
                    if let Some(value) = value.split(':').nth(1) {
                        return value.trim().to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn detect_ram_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                if let Ok(bytes) = text.trim().parse::<u64>() {
                    return bytes;
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
            for line in text.lines() {
                if let Some(value) = line.strip_prefix("MemTotal:") {
                    if let Some(kb) = value.split_whitespace().next() {
                        if let Ok(kb) = kb.parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
    }
    0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub scene: String,
    pub solver: String,
    pub requested_agents: u32,
    pub seed: u64,
    pub ticks_per_second: u32,
    pub duration_ticks: u64,
    pub scene_hash: u64,
    pub final_state_hash: u64,
    pub environment: Environment,
    pub metrics: MetricsSummary,
}

/// Build, run, and measure one scene.
pub fn run_scene(options: &RunOptions) -> Result<Report, String> {
    let scene_def = scenes::build(&options.scene, options.agents, options.seed)
        .ok_or_else(|| format!("unknown scene: {}", options.scene))?;
    let scene = scene_def
        .compile()
        .map_err(|errors| format!("{} failed to compile: {errors:?}", options.scene))?;

    let scene_hash = scene.scene_hash();
    let ticks_per_second = scene.ticks_per_second;
    let duration_ticks = scene.duration_ticks;

    let mut config = SimConfig::default();
    config.metrics = MetricsConfig {
        throughput_gate: scenes::throughput_gate(&options.scene),
        ..MetricsConfig::default()
    };

    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        config,
    );

    // Reset after construction so scene-build allocations are excluded.
    alloc::reset_peak();
    let started = Instant::now();
    sim.run_to_completion();
    let wall_time_seconds = started.elapsed().as_secs_f64();
    let peak_allocated_bytes = alloc::peak_bytes() as u64;

    let metrics = sim.metrics().summarize(
        sim.world(),
        sim.scene(),
        wall_time_seconds,
        peak_allocated_bytes,
    );

    Ok(Report {
        schema_version: REPORT_SCHEMA_VERSION,
        scene: options.scene.clone(),
        solver: sim.solver_name().to_string(),
        requested_agents: options.agents,
        seed: options.seed,
        ticks_per_second,
        duration_ticks,
        scene_hash,
        final_state_hash: sim.state_hash(),
        environment: Environment::capture(),
        metrics,
    })
}
```

- [ ] **Step 7: Write a minimal `main.rs` so the crate compiles**

`crates/crowd-bench/src/main.rs`:

```rust
mod alloc;
mod report;

#[global_allocator]
static ALLOCATOR: alloc::CountingAllocator = alloc::CountingAllocator;

fn main() {
    // Replaced by the full CLI in Task 23.
    println!("crowd-bench");
}
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test -p crowd-bench`
Expected: PASS, 8 tests.

Note the allocator tests assume the process-wide `#[global_allocator]` is installed, which it is for the test binary because `main.rs` declares it.

- [ ] **Step 9: Commit**

```bash
cargo fmt
git add Cargo.toml crates/crowd-bench/
git commit -m "Add benchmark crate with counting allocator and report schema"
```

---

## Task 22: Trajectory SVG dump

Metrics cannot tell you a crowd looks robotic. Contract section 16 lists "avoidance looks robotic or deadlocks" as a top risk, and this is the only way to see it before the Blender bridge exists.

**Files:**
- Create: `crates/crowd-bench/src/svg.rs`
- Modify: `crates/crowd-bench/src/report.rs`
- Modify: `crates/crowd-bench/src/main.rs`

**Interfaces:**
- Consumes: `crowd_core::{Simulation, units::{Aabb, Vec2}, geometry::Segment}`.
- Produces: `TrajectoryRecorder::new(sample_interval: u64, max_agents: usize)` with `record(&mut self, sim: &Simulation)` and `write_svg(&self, scene_name: &str, bounds: Aabb, walls: &[Segment]) -> String`.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-bench/src/svg.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crowd_core::avoidance::SampledVelocitySolver;
    use crowd_core::scenes;
    use crowd_core::sim::{SimConfig, Simulation};

    fn simulation() -> Simulation {
        let scene = scenes::build("bidirectional_corridor", 20, 1)
            .unwrap()
            .compile()
            .unwrap();
        Simulation::new(
            scene,
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        )
    }

    #[test]
    fn an_empty_recorder_still_writes_valid_svg() {
        let sim = simulation();
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg("empty", sim.scene().bounds, sim.walls());
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn recorded_trajectories_appear_as_polylines() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(1, 100);
        for _ in 0..30 {
            sim.step();
            recorder.record(&sim);
        }
        let svg = recorder.write_svg("corridor", sim.scene().bounds, sim.walls());
        assert!(svg.contains("<polyline"), "no trajectories drawn");
    }

    #[test]
    fn walls_are_drawn() {
        let sim = simulation();
        let recorder = TrajectoryRecorder::new(5, 100);
        let svg = recorder.write_svg("corridor", sim.scene().bounds, sim.walls());
        assert!(svg.contains("<line"), "no walls drawn");
    }

    #[test]
    fn the_sample_interval_is_respected() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(10, 100);
        for _ in 0..30 {
            sim.step();
            recorder.record(&sim);
        }
        assert_eq!(recorder.sample_count(), 3);
    }

    #[test]
    fn the_agent_cap_is_respected() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(1, 5);
        for _ in 0..20 {
            sim.step();
            recorder.record(&sim);
        }
        assert!(recorder.tracked_agents() <= 5);
    }

    #[test]
    fn output_contains_no_non_finite_coordinates() {
        let mut sim = simulation();
        let mut recorder = TrajectoryRecorder::new(2, 50);
        for _ in 0..40 {
            sim.step();
            recorder.record(&sim);
        }
        let svg = recorder.write_svg("corridor", sim.scene().bounds, sim.walls());
        assert!(!svg.contains("NaN"), "NaN leaked into the SVG");
        assert!(!svg.contains("inf"), "infinity leaked into the SVG");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-bench svg`
Expected: FAIL — `cannot find type TrajectoryRecorder in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-bench/src/svg.rs`:

```rust
//! Dependency-free trajectory visualisation.
//!
//! Metrics cannot tell you a crowd looks robotic. Contract section 16 names
//! "avoidance looks robotic or deadlocks" as a top risk, and this is the only
//! way to see it before the Blender bridge exists.

use std::fmt::Write;

use crowd_core::geometry::Segment;
use crowd_core::sim::Simulation;
use crowd_core::units::{Aabb, Vec2};

/// Pixels per meter in the emitted SVG.
const SCALE: f32 = 20.0;
const MARGIN: f32 = 20.0;

pub struct TrajectoryRecorder {
    sample_interval: u64,
    max_agents: usize,
    ticks_seen: u64,
    sample_count: usize,
    /// One polyline per tracked agent, in stable slot order.
    tracks: Vec<Vec<Vec2>>,
}

impl TrajectoryRecorder {
    pub fn new(sample_interval: u64, max_agents: usize) -> Self {
        Self {
            sample_interval: sample_interval.max(1),
            max_agents,
            ticks_seen: 0,
            sample_count: 0,
            tracks: Vec::new(),
        }
    }

    pub fn sample_count(&self) -> usize {
        self.sample_count
    }

    pub fn tracked_agents(&self) -> usize {
        self.tracks.len()
    }

    /// Sample the world if this tick falls on the interval.
    ///
    /// Only the first `max_agents` slots are tracked. Slots are assigned in
    /// spawn order, so the tracked set is stable across runs rather than an
    /// arbitrary sample that changes shape between renders.
    pub fn record(&mut self, sim: &Simulation) {
        self.ticks_seen += 1;
        if self.ticks_seen % self.sample_interval != 0 {
            return;
        }
        self.sample_count += 1;

        let world = sim.world();
        let tracked = world.len().min(self.max_agents);
        if self.tracks.len() < tracked {
            self.tracks.resize(tracked, Vec::new());
        }
        for slot in 0..tracked {
            let p = world.position(slot as u32);
            if p.is_finite() {
                self.tracks[slot].push(p);
            }
        }
    }

    pub fn write_svg(&self, scene_name: &str, bounds: Aabb, walls: &[Segment]) -> String {
        let size = bounds.size();
        let width = size.x * SCALE + MARGIN * 2.0;
        let height = size.y * SCALE + MARGIN * 2.0;

        // SVG's Y axis points down; the simulation's points up. Flipping here
        // keeps the rendered image matching the scene as authored.
        let project = |p: Vec2| -> (f32, f32) {
            (
                (p.x - bounds.min.x) * SCALE + MARGIN,
                height - ((p.y - bounds.min.y) * SCALE + MARGIN),
            )
        };

        let mut out = String::new();
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        );
        let _ = write!(out, r#"<rect width="100%" height="100%" fill="#111"/>"#);
        let _ = write!(
            out,
            r#"<text x="{MARGIN:.0}" y="{:.0}" fill="#eee" font-family="monospace" font-size="14">{scene_name}</text>"#,
            MARGIN - 4.0
        );

        for wall in walls {
            let (x1, y1) = project(wall.a);
            let (x2, y2) = project(wall.b);
            if [x1, y1, x2, y2].iter().all(|v| v.is_finite()) {
                let _ = write!(
                    out,
                    r#"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="#888" stroke-width="2"/>"#
                );
            }
        }

        for (index, track) in self.tracks.iter().enumerate() {
            if track.len() < 2 {
                continue;
            }
            // A fixed hue rotation keeps neighboring agents distinguishable
            // without needing a palette dependency.
            let hue = (index * 47) % 360;
            let mut points = String::new();
            for p in track {
                let (x, y) = project(*p);
                if x.is_finite() && y.is_finite() {
                    let _ = write!(points, "{x:.1},{y:.1} ");
                }
            }
            let _ = write!(
                out,
                r#"<polyline points="{}" fill="none" stroke="hsl({hue},70%,60%)" stroke-width="1.2" opacity="0.75"/>"#,
                points.trim_end()
            );
        }

        out.push_str("</svg>\n");
        out
    }
}
```

- [ ] **Step 4: Wire the recorder into `run_scene`**

In `crates/crowd-bench/src/report.rs`, add `use crate::svg::TrajectoryRecorder;` and replace the run block:

```rust
    alloc::reset_peak();
    let started = Instant::now();
    let mut recorder = if options.svg {
        Some(TrajectoryRecorder::new(5, 400))
    } else {
        None
    };
    match recorder.as_mut() {
        // Recording costs a per-tick sample, so the un-recorded path stays a
        // tight loop and the timing metric is not skewed by visualisation.
        Some(recorder) => {
            while sim.clock().tick() < duration_ticks {
                sim.step();
                recorder.record(&sim);
            }
        }
        None => sim.run_to_completion(),
    }
    let wall_time_seconds = started.elapsed().as_secs_f64();
    let peak_allocated_bytes = alloc::peak_bytes() as u64;

    if let Some(recorder) = recorder.as_ref() {
        std::fs::create_dir_all(&options.out_dir)
            .map_err(|e| format!("cannot create {}: {e}", options.out_dir.display()))?;
        let svg = recorder.write_svg(&options.scene, sim.scene().bounds, sim.walls());
        let path = options.out_dir.join(format!("{}.svg", options.scene));
        std::fs::write(&path, svg)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
```

Add `mod svg;` to `crates/crowd-bench/src/main.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-bench`
Expected: PASS, 14 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-bench/
git commit -m "Add dependency-free trajectory SVG dump"
```

---

## Task 23: CLI, baselines, and regression check

**Files:**
- Create: `crates/crowd-bench/src/baseline.rs`
- Modify: `crates/crowd-bench/src/main.rs`
- Create: `benchmarks/baselines/.gitkeep`

**Interfaces:**
- Consumes: `report::{Report, RunOptions, run_scene}`.
- Produces: `baseline::{Baseline, BaselineMetric, metric_map, from_report, compare, Comparison, Drift}`.

Because the simulation is deterministic, quality metrics are *exactly* reproducible on the same machine. Their tolerance is therefore zero, and any drift is a real behavior change rather than noise. Only timing and memory need a tolerance band.

- [ ] **Step 1: Write the failing test**

Create `crates/crowd-bench/src/baseline.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{run_scene, RunOptions};

    fn report() -> crate::report::Report {
        run_scene(&RunOptions {
            scene: "crossing".to_string(),
            agents: 40,
            seed: 2026,
            svg: false,
            out_dir: std::env::temp_dir().join("crowd_bench_baseline_test"),
        })
        .unwrap()
    }

    #[test]
    fn a_report_compared_against_its_own_baseline_passes() {
        let report = report();
        let baseline = from_report(&report);
        let comparison = compare(&baseline, &report);
        assert!(comparison.passed, "drift: {:?}", comparison.drifts);
    }

    #[test]
    fn quality_metrics_have_zero_tolerance() {
        let baseline = from_report(&report());
        let metric = baseline.metrics.get("penetration_events").unwrap();
        assert_eq!(metric.tolerance, 0.0);
    }

    #[test]
    fn timing_metrics_have_a_tolerance_band() {
        let baseline = from_report(&report());
        assert!(baseline.metrics.get("wall_time_seconds").unwrap().tolerance > 0.0);
    }

    #[test]
    fn a_changed_quality_metric_fails_the_check() {
        let report = report();
        let mut baseline = from_report(&report);
        baseline
            .metrics
            .get_mut("agents_arrived")
            .unwrap()
            .value += 5.0;
        let comparison = compare(&baseline, &report);
        assert!(!comparison.passed);
        assert!(comparison.drifts.iter().any(|d| d.metric == "agents_arrived"));
    }

    #[test]
    fn timing_noise_within_tolerance_passes() {
        let report = report();
        let mut baseline = from_report(&report);
        let metric = baseline.metrics.get_mut("wall_time_seconds").unwrap();
        // Ten percent slower, well inside the timing band.
        metric.value *= 1.1;
        assert!(compare(&baseline, &report).passed);
    }

    #[test]
    fn a_scene_hash_mismatch_fails_immediately() {
        let report = report();
        let mut baseline = from_report(&report);
        baseline.scene_hash = 12345;
        let comparison = compare(&baseline, &report);
        assert!(!comparison.passed);
        assert!(
            comparison.drifts.iter().any(|d| d.metric == "scene_hash"),
            "a baseline from a different scene must be rejected, not compared"
        );
    }

    #[test]
    fn a_baseline_round_trips_through_json() {
        let baseline = from_report(&report());
        let json = serde_json::to_string_pretty(&baseline).unwrap();
        let parsed: Baseline = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, baseline);
    }

    #[test]
    fn every_summary_field_appears_in_the_metric_map() {
        // Guards against a metric being added to the summary but silently
        // never compared.
        let map = metric_map(&report().metrics);
        for key in [
            "agents_arrived",
            "completion_rate",
            "penetration_events",
            "max_penetration_depth",
            "min_time_to_collision",
            "stalled_agents",
            "heading_reversals",
            "abrupt_turns",
            "gate_crossings",
            "wall_corrections",
            "nonfinite_corrections",
            "wall_time_seconds",
            "peak_allocated_bytes",
        ] {
            assert!(map.contains_key(key), "missing metric: {key}");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p crowd-bench baseline`
Expected: FAIL — `cannot find function from_report in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/crowd-bench/src/baseline.rs`:

```rust
//! Measured baselines and relative regression checking.
//!
//! Contract section 12.3 fixes thresholds only after a baseline is measured,
//! so nothing here asserts an absolute quality bar. It asserts only that today
//! matches what was measured and reviewed.
//!
//! Because the simulation is deterministic, quality metrics are exactly
//! reproducible on the same machine, so their tolerance is zero: any drift is
//! a real behavior change, not noise. Only timing and memory need a band.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::report::Report;
use crowd_core::metrics::MetricsSummary;

/// Fractional tolerance for wall-clock and throughput figures.
const TIMING_TOLERANCE: f64 = 0.5;
/// Fractional tolerance for peak allocation, which varies with allocator
/// behavior but not nearly as much as timing.
const MEMORY_TOLERANCE: f64 = 0.15;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaselineMetric {
    pub value: f64,
    /// Fractional tolerance. Zero demands an exact match.
    pub tolerance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub scene_hash: u64,
    pub solver: String,
    pub metrics: BTreeMap<String, BaselineMetric>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drift {
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    pub tolerance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub passed: bool,
    pub drifts: Vec<Drift>,
}

/// Flatten a summary into comparable named values.
///
/// Written out by hand rather than derived: an explicit list makes adding a
/// metric a deliberate act, and the test above catches omissions.
pub fn metric_map(summary: &MetricsSummary) -> BTreeMap<String, f64> {
    // Built as an array first rather than through a closure that captures the
    // map: a `FnMut` holding `&mut map` would still be alive at the return,
    // and the borrow checker rejects that.
    [
        ("ticks", summary.ticks as f64),
        ("agents_spawned", summary.agents_spawned as f64),
        ("agents_arrived", summary.agents_arrived as f64),
        ("completion_rate", summary.completion_rate as f64),
        ("median_travel_seconds", summary.median_travel_seconds as f64),
        ("p95_travel_seconds", summary.p95_travel_seconds as f64),
        ("penetration_events", summary.penetration_events as f64),
        ("max_penetration_depth", summary.max_penetration_depth as f64),
        (
            "penetration_agent_ticks",
            summary.penetration_agent_ticks as f64,
        ),
        ("min_time_to_collision", summary.min_time_to_collision as f64),
        ("near_miss_ticks", summary.near_miss_ticks as f64),
        ("wall_corrections", summary.wall_corrections as f64),
        ("nonfinite_corrections", summary.nonfinite_corrections as f64),
        ("stalled_agents", summary.stalled_agents as f64),
        ("stall_agent_ticks", summary.stall_agent_ticks as f64),
        ("heading_reversals", summary.heading_reversals as f64),
        ("abrupt_turns", summary.abrupt_turns as f64),
        ("gate_crossings", summary.gate_crossings as f64),
        ("wall_time_seconds", summary.wall_time_seconds),
        (
            "ticks_per_second_achieved",
            summary.ticks_per_second_achieved,
        ),
        ("peak_allocated_bytes", summary.peak_allocated_bytes as f64),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect()
}

fn tolerance_for(metric: &str) -> f64 {
    match metric {
        "wall_time_seconds" | "ticks_per_second_achieved" => TIMING_TOLERANCE,
        "peak_allocated_bytes" => MEMORY_TOLERANCE,
        _ => 0.0,
    }
}

pub fn from_report(report: &Report) -> Baseline {
    Baseline {
        scene: report.scene.clone(),
        agents: report.requested_agents,
        seed: report.seed,
        scene_hash: report.scene_hash,
        solver: report.solver.clone(),
        metrics: metric_map(&report.metrics)
            .into_iter()
            .map(|(key, value)| {
                let tolerance = tolerance_for(&key);
                (key, BaselineMetric { value, tolerance })
            })
            .collect(),
    }
}

pub fn compare(baseline: &Baseline, report: &Report) -> Comparison {
    let mut drifts = Vec::new();

    // A baseline recorded from different geometry cannot be meaningfully
    // compared, so say that rather than emitting twenty confusing drifts.
    if baseline.scene_hash != report.scene_hash {
        drifts.push(Drift {
            metric: "scene_hash".to_string(),
            baseline: baseline.scene_hash as f64,
            current: report.scene_hash as f64,
            tolerance: 0.0,
        });
        return Comparison {
            passed: false,
            drifts,
        };
    }

    let current = metric_map(&report.metrics);
    for (key, expected) in &baseline.metrics {
        let Some(&actual) = current.get(key) else {
            drifts.push(Drift {
                metric: key.clone(),
                baseline: expected.value,
                current: f64::NAN,
                tolerance: expected.tolerance,
            });
            continue;
        };

        let allowed = if expected.tolerance == 0.0 {
            0.0
        } else {
            expected.value.abs() * expected.tolerance
        };
        if (actual - expected.value).abs() > allowed {
            drifts.push(Drift {
                metric: key.clone(),
                baseline: expected.value,
                current: actual,
                tolerance: expected.tolerance,
            });
        }
    }

    Comparison {
        passed: drifts.is_empty(),
        drifts,
    }
}
```

- [ ] **Step 4: Write the CLI**

Replace `crates/crowd-bench/src/main.rs`:

```rust
//! Benchmark runner.
//!
//! Argument parsing is hand-rolled to keep the dependency set at serde alone;
//! the surface is small enough that a parser crate would cost more than it
//! saves.

mod alloc;
mod baseline;
mod report;
mod svg;

use std::path::PathBuf;
use std::process::ExitCode;

use crowd_core::scenes;

use crate::report::{run_scene, RunOptions};

#[global_allocator]
static ALLOCATOR: alloc::CountingAllocator = alloc::CountingAllocator;

const DEFAULT_AGENTS: u32 = 1000;
const DEFAULT_SEED: u64 = 2026;
const BASELINE_DIR: &str = "benchmarks/baselines";
const REPORT_DIR: &str = "benchmarks/reports";

fn usage() -> &'static str {
    "usage:
  crowd-bench run [--scene NAME] [--agents N] [--seed N] [--svg] [--out DIR]
  crowd-bench sweep [--scene NAME] [--seed N]
  crowd-bench baseline [--scene NAME] [--agents N] [--seed N]
  crowd-bench check [--agents N] [--seed N]

Omitting --scene runs every scene."
}

struct Args {
    scene: Option<String>,
    agents: u32,
    seed: u64,
    svg: bool,
    out: PathBuf,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args {
        scene: None,
        agents: DEFAULT_AGENTS,
        seed: DEFAULT_SEED,
        svg: false,
        out: PathBuf::from(REPORT_DIR),
    };
    let mut index = 0;
    while index < raw.len() {
        match raw[index].as_str() {
            "--scene" => {
                index += 1;
                args.scene = Some(raw.get(index).ok_or("--scene needs a value")?.clone());
            }
            "--agents" => {
                index += 1;
                args.agents = raw
                    .get(index)
                    .ok_or("--agents needs a value")?
                    .parse()
                    .map_err(|_| "--agents must be a number")?;
            }
            "--seed" => {
                index += 1;
                args.seed = raw
                    .get(index)
                    .ok_or("--seed needs a value")?
                    .parse()
                    .map_err(|_| "--seed must be a number")?;
            }
            "--out" => {
                index += 1;
                args.out = PathBuf::from(raw.get(index).ok_or("--out needs a value")?);
            }
            "--svg" => args.svg = true,
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }
    Ok(args)
}

fn scenes_to_run(args: &Args) -> Result<Vec<String>, String> {
    match &args.scene {
        Some(name) => {
            if !scenes::SCENE_NAMES.contains(&name.as_str()) {
                return Err(format!(
                    "unknown scene {name}; known scenes: {}",
                    scenes::SCENE_NAMES.join(", ")
                ));
            }
            Ok(vec![name.clone()])
        }
        None => Ok(scenes::SCENE_NAMES.iter().map(|s| s.to_string()).collect()),
    }
}

fn options_for(scene: &str, args: &Args) -> RunOptions {
    RunOptions {
        scene: scene.to_string(),
        agents: args.agents,
        seed: args.seed,
        svg: args.svg,
        out_dir: args.out.clone(),
    }
}

fn command_run(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    for scene in scenes_to_run(args)? {
        let report = run_scene(&options_for(&scene, args))?;
        let path = args.out.join(format!("{scene}-{}.json", args.agents));
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        println!(
            "{scene}: {} arrived / {} spawned, {} penetrations, {:.2}s wall, {:.0} ticks/s -> {}",
            report.metrics.agents_arrived,
            report.metrics.agents_spawned,
            report.metrics.penetration_events,
            report.metrics.wall_time_seconds,
            report.metrics.ticks_per_second_achieved,
            path.display()
        );
    }
    Ok(())
}

fn command_sweep(args: &Args) -> Result<(), String> {
    for scene in scenes_to_run(args)? {
        for agents in [100u32, 500, 1000, 2000] {
            // Never record SVGs during a sweep: the per-tick sampling would
            // skew the very timing numbers the sweep exists to measure.
            let sweep_args = Args {
                scene: Some(scene.clone()),
                agents,
                seed: args.seed,
                svg: false,
                out: args.out.clone(),
            };
            let report = run_scene(&options_for(&scene, &sweep_args))?;
            println!(
                "{scene},{agents},{:.4},{:.1},{}",
                report.metrics.wall_time_seconds,
                report.metrics.ticks_per_second_achieved,
                report.metrics.peak_allocated_bytes
            );
        }
    }
    Ok(())
}

fn command_baseline(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(BASELINE_DIR).map_err(|e| e.to_string())?;
    for scene in scenes_to_run(args)? {
        let report = run_scene(&options_for(&scene, args))?;
        let baseline = baseline::from_report(&report);
        let path = PathBuf::from(BASELINE_DIR).join(format!("{scene}.json"));
        let json = serde_json::to_string_pretty(&baseline).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

fn command_check(args: &Args) -> Result<bool, String> {
    let mut all_passed = true;
    for scene in scenes_to_run(args)? {
        let path = PathBuf::from(BASELINE_DIR).join(format!("{scene}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let stored: baseline::Baseline =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

        let report = run_scene(&options_for(
            &scene,
            &Args {
                scene: Some(scene.clone()),
                agents: stored.agents,
                seed: stored.seed,
                svg: false,
                out: args.out.clone(),
            },
        ))?;

        let comparison = baseline::compare(&stored, &report);
        if comparison.passed {
            println!("{scene}: OK");
        } else {
            all_passed = false;
            println!("{scene}: DRIFT");
            for drift in &comparison.drifts {
                println!(
                    "  {}: baseline {}, now {} (tolerance {:.0}%)",
                    drift.metric,
                    drift.baseline,
                    drift.current,
                    drift.tolerance * 100.0
                );
            }
        }
    }
    Ok(all_passed)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = argv.split_first() else {
        eprintln!("{}", usage());
        return ExitCode::FAILURE;
    };

    let args = match parse_args(rest) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n\n{}", usage());
            return ExitCode::FAILURE;
        }
    };

    let outcome = match command.as_str() {
        "run" => command_run(&args).map(|()| true),
        "sweep" => command_sweep(&args).map(|()| true),
        "baseline" => command_baseline(&args).map(|()| true),
        "check" => command_check(&args),
        other => Err(format!("unknown command: {other}\n\n{}", usage())),
    };

    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        // A drift is a reportable result, not a crash; the distinct exit code
        // lets CI treat it differently from a broken build.
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p crowd-bench`
Expected: PASS, 22 tests.

- [ ] **Step 6: Measure and check in the baselines**

This is where the plan stops prescribing numbers and starts recording them. Build in release — a debug build's timings are meaningless.

```bash
mkdir -p benchmarks/baselines
cargo run --release -p crowd-bench -- baseline --agents 1000
cargo run --release -p crowd-bench -- check --agents 1000
```

Expected: `baseline` writes five files; `check` prints `OK` for all five and exits zero.

Read the five baseline files before committing them. If `agents_arrived` is near zero for a scene, or `penetration_events` is enormous, the baseline is recording a broken simulation — fix the defect rather than blessing it. A baseline is a claim that this output was reviewed and found acceptable.

- [ ] **Step 7: Generate the visual dumps and look at them**

```bash
cargo run --release -p crowd-bench -- run --agents 1000 --svg
open benchmarks/reports/*.svg   # or any SVG viewer
```

Confirm trajectories look like walking people: lanes forming in the corridor, a queue at the doorway, no dense scribble of oscillation, no agents through walls.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add crates/crowd-bench/ benchmarks/baselines/
git commit -m "Add benchmark CLI with measured baselines and regression check"
```

---

## Task 24: Property and fuzz tests

**Files:**
- Create: `crates/crowd-core/tests/properties.rs`
- Create: `crates/crowd-core/tests/fuzz_density.rs`

**Interfaces:**
- Consumes: everything public in `crowd_core`.
- Produces: nothing; test targets.

- [ ] **Step 1: Write the property tests**

Create `crates/crowd-core/tests/properties.rs`:

```rust
//! Property tests, contract section 15.1.

use crowd_core::geometry::{time_to_collision_disc, Segment};
use crowd_core::grid::UniformGrid;
use crowd_core::ids::derive_agent_id;
use crowd_core::rng::{Purpose, StableRng};
use crowd_core::units::{Aabb, Vec2};
use proptest::prelude::*;

fn any_coordinate() -> impl Strategy<Value = f32> {
    -100.0f32..100.0f32
}

proptest! {
    #[test]
    fn derived_ids_are_injective_over_ordinals(
        seed in any::<u64>(),
        population in any::<u16>(),
        source in any::<u16>(),
        a in any::<u32>(),
        b in any::<u32>(),
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(
            derive_agent_id(seed, population, source, a),
            derive_agent_id(seed, population, source, b)
        );
    }

    #[test]
    fn random_draws_stay_within_their_range(
        seed in any::<u64>(),
        ordinal in any::<u32>(),
        lo in -50.0f32..0.0f32,
        span in 0.1f32..50.0f32,
    ) {
        let id = derive_agent_id(seed, 0, 0, ordinal);
        let mut rng = StableRng::for_agent(seed, id, Purpose::Radius);
        let value = rng.range_f32(lo, lo + span);
        prop_assert!(value >= lo && value <= lo + span);
    }

    #[test]
    fn normal_draws_are_always_finite(seed in any::<u64>()) {
        let mut rng = StableRng::from_seed(seed);
        for _ in 0..64 {
            prop_assert!(rng.normal_f32(1.35, 0.18).is_finite());
        }
    }

    #[test]
    fn closest_point_is_never_further_than_either_endpoint(
        ax in any_coordinate(), ay in any_coordinate(),
        bx in any_coordinate(), by in any_coordinate(),
        px in any_coordinate(), py in any_coordinate(),
    ) {
        let seg = Segment::new(Vec2::new(ax, ay), Vec2::new(bx, by));
        let p = Vec2::new(px, py);
        let d = seg.distance_to(p);
        prop_assert!(d <= (p - seg.a).length() + 1e-3);
        prop_assert!(d <= (p - seg.b).length() + 1e-3);
    }

    #[test]
    fn time_to_collision_is_never_negative(
        px in any_coordinate(), py in any_coordinate(),
        vx in -5.0f32..5.0f32, vy in -5.0f32..5.0f32,
        radius in 0.1f32..2.0f32,
    ) {
        if let Some(t) = time_to_collision_disc(Vec2::new(px, py), Vec2::new(vx, vy), radius) {
            prop_assert!(t >= 0.0);
            prop_assert!(t.is_finite());
        }
    }

    #[test]
    fn grid_queries_never_miss_a_point_brute_force_finds(
        xs in prop::collection::vec(-40.0f32..40.0f32, 1..80),
        radius in 0.5f32..8.0f32,
    ) {
        let ys: Vec<f32> = xs.iter().map(|x| x * 0.37).collect();
        let bounds = Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0));
        let mut grid = UniformGrid::new(bounds, 4.0);
        grid.rebuild(&xs, &ys);

        let centre = Vec2::new(xs[0], ys[0]);
        let mut found = Vec::new();
        grid.query(centre, radius, &mut found);

        for i in 0..xs.len() {
            let d = Vec2::new(xs[i], ys[i]).distance_squared(centre);
            if d <= radius * radius {
                prop_assert!(
                    found.contains(&(i as u32)),
                    "grid missed slot {i} at distance {}",
                    d.sqrt()
                );
            }
        }
    }
}
```

- [ ] **Step 2: Run the property tests**

Run: `cargo test -p crowd-core --test properties`
Expected: PASS, 6 property tests.

If `grid_queries_never_miss_a_point_brute_force_finds` shrinks to a failing case, the bug is real — most likely the cell clamping in `cell_of` folding a distant point onto an edge cell that the query range does not cover. Fix `grid.rs`, do not widen the test.

- [ ] **Step 3: Write the density fuzz tests**

Create `crates/crowd-core/tests/fuzz_density.rs`:

```rust
//! Randomised density stress, contract section 15.1: checked for NaN, escape,
//! and deadlock.

use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};

fn stress(scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
    let scene = scenes::build(scene_name, agents, seed)
        .expect("known scene")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(
        scene,
        Box::new(SampledVelocitySolver::default()),
        SimConfig::default(),
    );
    sim.run(ticks);
    sim
}

#[test]
fn no_agent_state_goes_non_finite_under_density() {
    for seed in 0..8u64 {
        for scene in scenes::SCENE_NAMES {
            let sim = stress(scene, 800, seed, 400);
            for slot in 0..sim.world().len() {
                let position = sim.world().position(slot as u32);
                let velocity = sim.world().velocity(slot as u32);
                assert!(
                    position.is_finite() && velocity.is_finite(),
                    "{scene} seed {seed} slot {slot} went non-finite"
                );
                assert!(sim.world().yaw[slot].is_finite());
            }
        }
    }
}

#[test]
fn no_agent_escapes_far_beyond_the_scene_bounds() {
    // A small margin is legitimate: the wall push-out resolves penetration
    // against the nearest surface, which can nudge an agent just outside.
    const MARGIN: f32 = 2.0;
    for seed in 0..4u64 {
        for scene in scenes::SCENE_NAMES {
            let sim = stress(scene, 800, seed, 400);
            let bounds = sim.scene().bounds.expanded(MARGIN);
            for slot in 0..sim.world().len() {
                let position = sim.world().position(slot as u32);
                assert!(
                    bounds.contains(position),
                    "{scene} seed {seed} slot {slot} escaped to {position:?}"
                );
            }
        }
    }
}

#[test]
fn speeds_never_exceed_the_per_agent_maximum() {
    for scene in scenes::SCENE_NAMES {
        let sim = stress(scene, 500, 11, 300);
        for slot in 0..sim.world().len() {
            let speed = sim.world().velocity(slot as u32).length();
            assert!(
                speed <= sim.world().max_speed[slot] + 1e-3,
                "{scene} slot {slot} exceeded max speed: {speed}"
            );
        }
    }
}

#[test]
fn the_crowd_does_not_deadlock_wholesale() {
    // Not a quality threshold: this asserts only that the simulation is still
    // making progress, which is the difference between a slow crowd and a
    // frozen one. Real quality bars come from measured baselines.
    for scene in scenes::SCENE_NAMES {
        let sim = stress(scene, 400, 3, 900);
        let moving = (0..sim.world().len())
            .filter(|slot| {
                !sim.world().arrived[*slot]
                    && sim.world().velocity(*slot as u32).length() > 0.05
            })
            .count();
        let unfinished = (0..sim.world().len())
            .filter(|slot| !sim.world().arrived[*slot])
            .count();
        if unfinished > 10 {
            assert!(
                moving > 0,
                "{scene}: {unfinished} agents unfinished and none moving"
            );
        }
    }
}
```

- [ ] **Step 4: Run the fuzz tests**

Run: `cargo test --release -p crowd-core --test fuzz_density`
Expected: PASS, 4 tests. Use `--release`; at 800 agents across five scenes and eight seeds, a debug build is impractically slow.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test --workspace` then `cargo test --release --workspace`
Expected: everything passes in both profiles.

Run `cargo clippy --workspace --all-targets -- -D warnings` and fix what it finds.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/crowd-core/tests/
git commit -m "Add property tests and randomized density fuzzing"
```

---

## Task 25: Document the results

The contract's Phase 0 exit gate is a *reproducible benchmark report*, not working code. This task produces it.

**Files:**
- Create: `docs/benchmarks/2026-08-04-kernel-slice-1.md`
- Modify: `README.md`

**Interfaces:**
- Consumes: the JSON reports and SVGs from Task 23.
- Produces: the written record.

- [ ] **Step 1: Regenerate everything from a clean build**

```bash
cargo clean
cargo run --release -p crowd-bench -- run --agents 1000 --svg
cargo run --release -p crowd-bench -- sweep
```

- [ ] **Step 2: Write the report**

Create `docs/benchmarks/2026-08-04-kernel-slice-1.md` containing, with real measured numbers rather than placeholders:

1. **Environment** — CPU, RAM, OS, rustc version, build profile, copied from a report's `environment` block.
2. **Per-scene results table** — scene, agents, completion rate, median and p95 travel time, penetration events and max depth, minimum time to collision, stalled agents, heading reversals, wall time, ticks/s, peak allocated bytes.
3. **Scaling curve** — the `sweep` output at 100/500/1,000/2,000 agents, with ticks/s and peak memory.
4. **Budget check against contract section 8.3** — the 1K gate asks for at least real-time simulation at 30 ticks/s without armature evaluation. State plainly whether that was met, and by how much.
5. **Embedded SVGs** or links, with a sentence each on what the trajectories show.
6. **Known defects and open questions** — anything the metrics or the pictures revealed. This section is the most valuable one; do not leave it empty because it looks bad.
7. **What this does not prove** — no navmesh, no behavior graph, no cache, no Blender, single-threaded, single machine, one solver with no comparison yet.

- [ ] **Step 3: Update the README**

Add a section pointing at the new report, the spec, and the commands to reproduce:

```markdown
## Development

Requires the pinned Rust toolchain (`rust-toolchain.toml`); `mise install` sets it up.

```sh
cargo test --workspace                                   # unit, property, determinism
cargo test --release -p crowd-core --test fuzz_density   # density stress
cargo run --release -p crowd-bench -- run --agents 1000 --svg
cargo run --release -p crowd-bench -- check --agents 1000
```

Measured results: [kernel slice 1 benchmark report](docs/benchmarks/2026-08-04-kernel-slice-1.md).
```

Replace the CLAUDE.md claim that "There is no build system or automated test suite yet" with these commands, since that statement is now false.

- [ ] **Step 4: Commit**

```bash
git add docs/benchmarks/ README.md CLAUDE.md
git commit -m "Add measured benchmark report for the kernel slice"
```

---

## Done

At this point the slice's success criteria from the spec are met: five scenes run headlessly at 1,000 agents, output is bitwise reproducible, spawn ordering and population size do not perturb results, baselines are checked in with a regression command, and trajectories are visually inspectable.

The next slices, in contract order: the avoidance bake-off (two more `AvoidanceSolver` implementations measured against these baselines), the tiled navmesh replacing `route.rs`, then cache v0 and the Blender bridge.
