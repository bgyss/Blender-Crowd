# Avoidance solver comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an ORCA-style solver and a scoped-anticipatory solver alongside the existing sampled-velocity solver, extend `crowd-bench` to compare all three across the four contract scales, and produce the dated report that selects a production default — closing M0 items 2 and 3.

**Architecture:** Two new modules (`crates/crowd-core/src/avoidance/orca.rs`, `.../anticipatory.rs`) implement the existing `AvoidanceSolver` trait. A shared-helpers pass extracts candidate sampling, density adjustment, side-bias cost, and wall-avoidance cost out of `sampled.rs` into `avoidance/mod.rs` so all three solvers reuse identical, already-tested building blocks instead of diverging copies. `crowd-bench` gains a `--solver` flag and a `compare` subcommand that runs the three-way bake-off; a dated markdown report under `docs/benchmarks/` presents the result and selects the default.

**Tech Stack:** Rust (workspace already on the pinned toolchain in `rust-toolchain.toml`), no new dependencies.

## Global Constraints

- `cargo fmt` before every commit; `cargo clippy --workspace --all-targets -- -D warnings` must stay clean.
- No new crate dependencies (workspace `Cargo.toml`/`Cargo.lock` do not change). This is a from-scratch ORCA implementation, per the approved spec.
- All solver code is `f32`, single-threaded, and deterministic: no `rand`, no wall-clock reads, no HashMap iteration where order matters (use `BTreeMap`/sorted `Vec` when stable ordering is required).
- Every new public behavior gets a test in the same commit that introduces it (repo-wide rule from `CLAUDE.md`).
- Run the density fuzz test only in release (`cargo test --release`), matching existing project guidance — it is impractically slow in debug.
- Never claim a benchmark or comparison result before actually running the commands and reading their output.

---

## Task 1: Shared avoidance helpers, extracted from `sampled.rs`

**Files:**
- Modify: `crates/crowd-core/src/avoidance/mod.rs`
- Modify: `crates/crowd-core/src/avoidance/sampled.rs`

**Interfaces:**
- Produces (used by Tasks 3, 4, 5): `pub(crate) fn rotate(v: Vec2, angle: f32) -> Vec2`, `pub(crate) fn sample_candidates(heading: Vec2, speed_reference: f32, speed_samples: u32, heading_samples: u32, visit: impl FnMut(Vec2))`, `pub(crate) fn density_adjusted_preferred(preferred: Vec2, position: Vec2, radius: f32, neighbors: &[NeighborState], personal_space: f32, density_speed_factor: f32) -> Vec2`, `pub(crate) fn is_head_on_encounter(heading: Vec2, position: Vec2, velocity: Vec2, neighbor: &NeighborState, head_on_cosine: f32) -> bool`, `pub(crate) fn side_bias_cost(preferred: Vec2, position: Vec2, velocity: Vec2, neighbors: &[NeighborState], candidate: Vec2, head_on_cosine: f32, side_bias_weight: f32) -> f32`, `pub(crate) fn wall_avoidance_cost(position: Vec2, max_speed: f32, candidate: Vec2, radius: f32, walls: &[Segment], wall_horizon: f32, collision_weight: f32, wall_weight: f32, overlap_urgency: f32, min_time_for_cost: f32) -> (f32, f32)`, `pub(crate) const OVERLAP_URGENCY: f32`, `pub(crate) const MIN_TIME_FOR_COST: f32`.
- Consumes: nothing new — this task only moves existing, already-tested logic out of `sampled.rs`.

This task is behavior-preserving: `sampled.rs`'s existing 13-test suite must pass unmodified afterward, which is the safety net for the refactor.

- [ ] **Step 1: Add the shared helpers to `avoidance/mod.rs`**

Open `crates/crowd-core/src/avoidance/mod.rs` and add, after the existing `AvoidanceSolver` trait definition:

```rust
use crate::geometry::Segment;

/// Cost scale for an existing overlap. Shared by every solver so "already
/// touching" reads as comparably urgent everywhere, not tuned per solver.
pub(crate) const OVERLAP_URGENCY: f32 = 8.0;

/// Floor on predicted time-to-collision when converting it to a cost.
pub(crate) const MIN_TIME_FOR_COST: f32 = 0.25;

pub(crate) fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// Enumerate velocity-space candidates on fixed speed rings around `heading`,
/// in a fixed order, so every solver that samples candidates produces the
/// identical sequence for identical inputs. Does not include the preferred
/// velocity or the stop candidate -- callers evaluate those explicitly around
/// this call, since where they sit in evaluation order is solver bookkeeping,
/// not enumeration.
pub(crate) fn sample_candidates(
    heading: Vec2,
    speed_reference: f32,
    speed_samples: u32,
    heading_samples: u32,
    mut visit: impl FnMut(Vec2),
) {
    for speed_index in 1..=speed_samples {
        let speed = speed_reference * speed_index as f32 / speed_samples as f32;
        for heading_index in 0..heading_samples {
            let angle = std::f32::consts::TAU * heading_index as f32 / heading_samples as f32;
            visit(rotate(heading, angle) * speed);
        }
    }
}

/// Preferred velocity scaled down by local crowding.
pub(crate) fn density_adjusted_preferred(
    preferred: Vec2,
    position: Vec2,
    radius: f32,
    neighbors: &[NeighborState],
    personal_space: f32,
    density_speed_factor: f32,
) -> Vec2 {
    let crowding = neighbors
        .iter()
        .filter(|n| {
            let clearance = radius + n.radius + personal_space;
            (n.position - position).length_squared() < clearance * clearance
        })
        .count() as f32;
    preferred * (1.0 / (1.0 + density_speed_factor * crowding))
}

/// Whether `neighbor` is closing on `position` from roughly ahead, by
/// `heading`. A fixed, ID-independent test: two agents meeting head-on see
/// mirrored geometry, so deriving this from an ID comparison would make both
/// derive opposite answers and, in mirrored frames, deflect the same way in
/// world space -- staying on a collision course rather than separating.
pub(crate) fn is_head_on_encounter(
    heading: Vec2,
    position: Vec2,
    velocity: Vec2,
    neighbor: &NeighborState,
    head_on_cosine: f32,
) -> bool {
    if heading == Vec2::ZERO {
        return false;
    }
    let to_neighbor = (neighbor.position - position).normalize_or_zero();
    if to_neighbor == Vec2::ZERO || heading.dot(to_neighbor) < head_on_cosine {
        return false;
    }
    (neighbor.velocity - velocity).dot(to_neighbor) < 0.0
}

/// Extra cost for passing a head-on neighbor on the wrong side: a fixed
/// keep-left convention evaluated in the agent's own frame.
pub(crate) fn side_bias_cost(
    preferred: Vec2,
    position: Vec2,
    velocity: Vec2,
    neighbors: &[NeighborState],
    candidate: Vec2,
    head_on_cosine: f32,
    side_bias_weight: f32,
) -> f32 {
    let heading = preferred.normalize_or_zero();
    if heading == Vec2::ZERO {
        return 0.0;
    }
    let mut cost = 0.0;
    for neighbor in neighbors {
        if !is_head_on_encounter(heading, position, velocity, neighbor, head_on_cosine) {
            continue;
        }
        let to_neighbor = (neighbor.position - position).normalize_or_zero();
        // Positive cross product means the candidate passes to the left as
        // the agent looks at the neighbor -- its own left, in its own frame.
        let candidate_side = to_neighbor.x * candidate.y - to_neighbor.y * candidate.x;
        if candidate_side < 0.0 {
            cost += side_bias_weight;
        }
    }
    cost
}

/// Collision cost and earliest predicted time-to-collision against every
/// wall, for one candidate velocity. Walls never yield, so this is always the
/// full (non-reciprocal) correction, never halved.
pub(crate) fn wall_avoidance_cost(
    position: Vec2,
    max_speed: f32,
    candidate: Vec2,
    radius: f32,
    walls: &[Segment],
    wall_horizon: f32,
    collision_weight: f32,
    wall_weight: f32,
    overlap_urgency: f32,
    min_time_for_cost: f32,
) -> (f32, f32) {
    let mut cost = 0.0;
    let mut earliest = f32::INFINITY;
    for wall in walls {
        let Some(t) =
            crate::geometry::time_to_collision_segment(position, candidate, radius, wall, wall_horizon)
        else {
            continue;
        };
        if t < earliest {
            earliest = t;
        }
        if t <= 0.0 {
            // Already inside the wall. A penalty derived from `t` alone
            // would be a constant that cancels out of the argmin -- see
            // `sampled.rs`'s original comment on this, which this code
            // preserves verbatim in behavior.
            let closest = wall.closest_point(position);
            let outward = (position - closest).normalize_or_zero();
            let outward = if outward == Vec2::ZERO {
                (wall.b - wall.a).normalize_or_zero().perp()
            } else {
                outward
            };
            let escape_rate = candidate.dot(outward);
            let relief = (escape_rate / max_speed.max(0.1)).clamp(0.0, 1.0);
            cost += collision_weight * wall_weight * overlap_urgency * (1.0 - relief);
        } else {
            cost += collision_weight * wall_weight / t.max(min_time_for_cost);
        }
    }
    (cost, earliest)
}
```

- [ ] **Step 2: Refactor `sampled.rs` to use the shared helpers**

In `crates/crowd-core/src/avoidance/sampled.rs`:

Remove the file-level `OVERLAP_URGENCY` and `MIN_TIME_FOR_COST` consts (now shared) and the private `fn rotate` at the bottom of the file. Add to the imports:

```rust
use super::{
    density_adjusted_preferred, is_head_on_encounter, rotate, sample_candidates, side_bias_cost,
    wall_avoidance_cost, AvoidanceInput, AvoidanceOutput, AvoidanceSolver, OVERLAP_URGENCY,
    MIN_TIME_FOR_COST,
};
```

(`is_head_on_encounter` is imported for parity even though `sampled.rs` reaches it only through `side_bias_cost`; drop it from the `use` list if `cargo clippy` flags it unused.)

Replace the `collision_cost` method's wall loop (the `for wall in input.walls { ... }` block) with a single delegated call, keeping the neighbor loop above it untouched:

```rust
    fn collision_cost(&self, input: &AvoidanceInput<'_>, candidate: Vec2) -> (f32, f32) {
        let mut cost = 0.0;
        let mut earliest = f32::INFINITY;

        for neighbor in input.neighbors {
            // ... existing neighbor loop body, unchanged ...
        }

        let (wall_cost, wall_earliest) = wall_avoidance_cost(
            input.position,
            input.max_speed,
            candidate,
            input.radius,
            input.walls,
            self.wall_horizon,
            self.collision_weight,
            self.wall_weight,
            OVERLAP_URGENCY,
            MIN_TIME_FOR_COST,
        );
        cost += wall_cost;
        if wall_earliest < earliest {
            earliest = wall_earliest;
        }

        (cost, earliest)
    }
```

Remove the `side_bias_cost` and `density_adjusted_preferred` methods entirely. In `solve`, replace:

```rust
        let preferred = self
            .density_adjusted_preferred(input)
            .clamp_length(input.max_speed);
```

with:

```rust
        let preferred = density_adjusted_preferred(
            input.preferred,
            input.position,
            input.radius,
            input.neighbors,
            self.personal_space,
            self.density_speed_factor,
        )
        .clamp_length(input.max_speed);
```

In the `evaluate` closure, replace `self.side_bias_cost(input, candidate)` with:

```rust
                    + side_bias_cost(
                        input.preferred,
                        input.position,
                        input.velocity,
                        input.neighbors,
                        candidate,
                        self.head_on_cosine,
                        self.side_bias_weight,
                    )
```

Replace the manual speed/heading sampling loop:

```rust
        let speed_reference = preferred_speed.max(input.velocity.length());
        for speed_index in 1..=self.speed_samples {
            let speed = speed_reference * speed_index as f32 / self.speed_samples as f32;
            for heading_index in 0..self.heading_samples {
                let angle =
                    std::f32::consts::TAU * heading_index as f32 / self.heading_samples as f32;
                let direction = rotate(heading, angle);
                evaluate(
                    direction * speed,
                    &mut best_velocity,
                    &mut best_cost,
                    &mut best_ttc,
                );
            }
        }
```

with:

```rust
        let speed_reference = preferred_speed.max(input.velocity.length());
        sample_candidates(
            heading,
            speed_reference,
            self.speed_samples,
            self.heading_samples,
            |candidate| evaluate(candidate, &mut best_velocity, &mut best_cost, &mut best_ttc),
        );
```

Finally, remove the standalone `fn rotate` function at the bottom of the file (now imported from `super`).

- [ ] **Step 3: Run the existing test suite to confirm the refactor is behavior-preserving**

Run: `cargo test -p crowd-core avoidance::sampled`
Expected: all 13 existing tests in `sampled.rs` still PASS, unmodified.

- [ ] **Step 4: Add a direct test for `sample_candidates`' enumeration order**

Add to `avoidance/mod.rs`'s test module (create one if none exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_candidates_enumerates_speed_rings_in_a_fixed_order() {
        let mut seen = Vec::new();
        sample_candidates(Vec2::new(1.0, 0.0), 2.0, 2, 4, |v| seen.push(v));
        assert_eq!(seen.len(), 8, "2 speed samples x 4 headings");
        // First ring at half the reference speed, heading 0 first.
        assert!((seen[0].length() - 1.0).abs() < 1e-4);
        assert!((seen[0].x - 1.0).abs() < 1e-4 && seen[0].y.abs() < 1e-4);
        // Second ring at the full reference speed.
        assert!((seen[4].length() - 2.0).abs() < 1e-4);
    }
}
```

- [ ] **Step 5: Run clippy and the full workspace test suite**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace`
Expected: all pass (same count as before this task, since nothing new-user-facing was added besides the one enumeration test).

- [ ] **Step 6: Commit**

```bash
git add crates/crowd-core/src/avoidance/mod.rs crates/crowd-core/src/avoidance/sampled.rs
git commit -m "Extract shared avoidance helpers out of the sampled-velocity solver"
```

---

## Task 2: ORCA sequential linear program

**Files:**
- Create: `crates/crowd-core/src/avoidance/linear_program.rs`
- Modify: `crates/crowd-core/src/avoidance/mod.rs` (add `mod linear_program;`)

**Interfaces:**
- Produces (used by Task 3): `pub(crate) struct Line { pub point: Vec2, pub direction: Vec2 }` (feasible half-plane: `v` is feasible iff `det(direction, v - point) >= 0`), `pub(crate) fn solve(lines: &[Line], radius: f32, preferred: Vec2) -> Vec2`.
- Consumes: `crate::units::Vec2`.

This task is pure math, tested in isolation from any ORCA-specific geometry — the goal is to prove the LP itself is correct before Task 3 builds agent-shaped constraints on top of it.

- [ ] **Step 1: Write the linear program module**

Create `crates/crowd-core/src/avoidance/linear_program.rs`:

```rust
//! Sequential incremental 2D linear program over half-plane constraints.
//!
//! Each line further constrains the feasible region for the velocity closest
//! to `preferred`. A line the current best point violates is resolved by
//! solving on that line's own feasible interval against every prior line. If
//! the whole set is jointly infeasible, `solve` falls back to minimizing the
//! worst constraint violation instead of leaving the caller with an undefined
//! or non-finite result -- the graceful-failure path a boxed-in agent needs.

use crate::units::Vec2;

const EPSILON: f32 = 1e-5;

/// A half-plane constraint. `v` is feasible iff `det(direction, v - point) >= 0`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line {
    pub point: Vec2,
    pub direction: Vec2,
}

fn det(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// The interval of `t` along `lines[line_no]` (parameterised as
/// `point + direction * t`) that lies within the disc of radius `radius` and
/// satisfies every line before it. `None` if no such interval exists.
fn feasible_interval(lines: &[Line], line_no: usize, radius: f32) -> Option<(f32, f32)> {
    let line = lines[line_no];
    let dot = line.point.dot(line.direction);
    let discriminant = dot * dot + radius * radius - line.point.length_squared();
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_discriminant = discriminant.sqrt();
    let mut t_left = -dot - sqrt_discriminant;
    let mut t_right = -dot + sqrt_discriminant;

    for i in 0..line_no {
        let denominator = det(line.direction, lines[i].direction);
        let numerator = det(lines[i].direction, line.point - lines[i].point);
        if denominator.abs() <= EPSILON {
            // Parallel constraints: either line_no is fully inside line i's
            // half-plane (no new bound) or fully outside it (infeasible).
            if numerator < 0.0 {
                return None;
            }
            continue;
        }
        let t = numerator / denominator;
        if denominator >= 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left = t_left.max(t);
        }
        if t_left > t_right {
            return None;
        }
    }
    Some((t_left, t_right))
}

/// Point on `lines[line_no]`'s feasible interval closest to `preferred`.
fn solve_line_toward_point(
    lines: &[Line],
    line_no: usize,
    radius: f32,
    preferred: Vec2,
    result: &mut Vec2,
) -> bool {
    let Some((t_left, t_right)) = feasible_interval(lines, line_no, radius) else {
        return false;
    };
    let line = lines[line_no];
    let t = line.direction.dot(preferred - line.point).clamp(t_left, t_right);
    *result = line.point + line.direction * t;
    true
}

/// Furthest point on `lines[line_no]`'s feasible interval in `direction`.
fn solve_line_toward_direction(
    lines: &[Line],
    line_no: usize,
    radius: f32,
    direction: Vec2,
    result: &mut Vec2,
) -> bool {
    let Some((t_left, t_right)) = feasible_interval(lines, line_no, radius) else {
        return false;
    };
    let line = lines[line_no];
    let t = if direction.dot(line.direction) > 0.0 {
        t_right
    } else {
        t_left
    };
    *result = line.point + line.direction * t;
    true
}

/// Try to satisfy every line in order, closest to `preferred`, within the
/// disc of radius `radius`. Returns the index of the first line that could
/// not be satisfied, or `lines.len()` on full success.
fn solve_incremental_toward_point(
    lines: &[Line],
    radius: f32,
    preferred: Vec2,
    result: &mut Vec2,
) -> usize {
    *result = preferred.clamp_length(radius);
    for i in 0..lines.len() {
        if det(lines[i].direction, lines[i].point - *result) > 0.0 {
            let mut candidate = *result;
            if !solve_line_toward_point(lines, i, radius, preferred, &mut candidate) {
                return i;
            }
            *result = candidate;
        }
    }
    lines.len()
}

/// Direction-optimizing variant of `solve_incremental_toward_point`, used by
/// the infeasible-fallback search.
fn solve_incremental_toward_direction(
    lines: &[Line],
    radius: f32,
    direction: Vec2,
    result: &mut Vec2,
) -> usize {
    *result = direction.normalize_or_zero() * radius;
    for i in 0..lines.len() {
        if det(lines[i].direction, lines[i].point - *result) > 0.0 {
            let mut candidate = *result;
            if !solve_line_toward_direction(lines, i, radius, direction, &mut candidate) {
                return i;
            }
            *result = candidate;
        }
    }
    lines.len()
}

/// Minimize the worst violation among `lines[first_failed..]`, given that
/// `solve_incremental_toward_point` could not satisfy all of them. This is
/// the graceful fallback for a jointly infeasible constraint set (an agent
/// boxed in on every side).
fn solve_fallback(lines: &[Line], first_failed: usize, radius: f32, result: &mut Vec2) {
    let mut distance = 0.0f32;
    for i in first_failed..lines.len() {
        if det(lines[i].direction, lines[i].point - *result) > distance {
            let mut projected: Vec<Line> = Vec::with_capacity(i);
            for j in 0..i {
                let denominator = det(lines[i].direction, lines[j].direction);
                let point = if denominator.abs() <= EPSILON {
                    if lines[i].direction.dot(lines[j].direction) > 0.0 {
                        // Parallel and compatible: no new bound.
                        continue;
                    }
                    (lines[i].point + lines[j].point) * 0.5
                } else {
                    lines[i].point
                        + lines[i].direction
                            * (det(lines[j].direction, lines[i].point - lines[j].point)
                                / denominator)
                };
                let direction = (lines[j].direction - lines[i].direction).normalize_or_zero();
                projected.push(Line { point, direction });
            }
            let search_direction = Vec2::new(-lines[i].direction.y, lines[i].direction.x);
            let mut candidate = *result;
            let solved =
                solve_incremental_toward_direction(&projected, radius, search_direction, &mut candidate);
            if solved == projected.len() {
                *result = candidate;
            }
            distance = det(lines[i].direction, lines[i].point - *result);
        }
    }
}

/// Solve for the velocity within the disc of radius `radius` closest to
/// `preferred` that satisfies every line in `lines`, falling back to a
/// minimal-violation point if the set is jointly infeasible.
pub(crate) fn solve(lines: &[Line], radius: f32, preferred: Vec2) -> Vec2 {
    let mut result = Vec2::ZERO;
    let first_failed = solve_incremental_toward_point(lines, radius, preferred, &mut result);
    if first_failed < lines.len() {
        solve_fallback(lines, first_failed, radius, &mut result);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_constraints_the_result_is_the_preferred_velocity() {
        let result = solve(&[], 2.0, Vec2::new(1.0, 0.5));
        assert_eq!(result, Vec2::new(1.0, 0.5));
    }

    #[test]
    fn a_satisfied_constraint_does_not_move_the_preferred_velocity() {
        let lines = [Line {
            point: Vec2::ZERO,
            direction: Vec2::new(1.0, 0.0),
        }];
        let result = solve(&lines, 2.0, Vec2::new(0.0, 1.0));
        assert_eq!(result, Vec2::new(0.0, 1.0));
    }

    #[test]
    fn a_violated_constraint_projects_onto_its_line() {
        let lines = [Line {
            point: Vec2::ZERO,
            direction: Vec2::new(1.0, 0.0),
        }];
        let result = solve(&lines, 2.0, Vec2::new(0.0, -1.0));
        assert!(
            result.y.abs() < 1e-5,
            "expected the boundary y=0, got {result:?}"
        );
    }

    #[test]
    fn a_preferred_velocity_outside_a_wedge_of_two_constraints_lands_on_their_corner() {
        // Feasible region v.y >= 0 and v.x >= 0 (the first quadrant).
        let lines = [
            Line {
                point: Vec2::ZERO,
                direction: Vec2::new(1.0, 0.0),
            },
            Line {
                point: Vec2::ZERO,
                direction: Vec2::new(0.0, -1.0),
            },
        ];
        let result = solve(&lines, 5.0, Vec2::new(-1.0, -1.0));
        assert!(
            result.x.abs() < 1e-5 && result.y.abs() < 1e-5,
            "expected the corner, got {result:?}"
        );
    }

    #[test]
    fn a_line_the_speed_disc_cannot_reach_falls_back_to_a_finite_result_within_the_disc() {
        let lines = [Line {
            point: Vec2::new(10.0, 0.0),
            direction: Vec2::new(0.0, -1.0),
        }];
        let result = solve(&lines, 2.0, Vec2::new(1.0, 0.0));
        assert!(result.is_finite());
        assert!(
            result.length() <= 2.0 + 1e-4,
            "result escaped the speed disc: {result:?}"
        );
    }

    #[test]
    fn a_satisfiable_set_of_constraints_produces_a_result_satisfying_all_of_them() {
        let lines = [
            Line {
                point: Vec2::ZERO,
                direction: Vec2::new(1.0, 0.0),
            },
            Line {
                point: Vec2::new(0.0, 0.5),
                direction: Vec2::new(-1.0, 0.0),
            },
        ];
        let result = solve(&lines, 3.0, Vec2::new(0.0, 5.0));
        for line in &lines {
            assert!(
                det(line.direction, line.point - result) <= 1e-4,
                "result {result:?} violates a constraint"
            );
        }
    }
}
```

- [ ] **Step 2: Wire the module in**

In `crates/crowd-core/src/avoidance/mod.rs`, add near the top (alongside `pub mod sampled;`):

```rust
mod linear_program;
```

(Not `pub` — only sibling solver modules under `avoidance` need it, via `super::linear_program`.)

- [ ] **Step 3: Run the new tests**

Run: `cargo test -p crowd-core avoidance::linear_program`
Expected: all 6 tests PASS. If `a_preferred_velocity_outside_a_wedge_of_two_constraints_lands_on_their_corner` or the fallback test fails, the most likely bug is a sign flip in `det`'s argument order or in the feasibility convention (`>= 0` vs `<= 0`) — re-derive from the failing test's hand-traceable geometry rather than guessing.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/crowd-core/src/avoidance/linear_program.rs crates/crowd-core/src/avoidance/mod.rs
git commit -m "Add the sequential 2D linear program ORCA will solve against"
```

---

## Task 3: `OrcaSolver`

**Files:**
- Create: `crates/crowd-core/src/avoidance/orca.rs`
- Modify: `crates/crowd-core/src/avoidance/mod.rs` (add `pub mod orca; pub use orca::OrcaSolver;`)

**Interfaces:**
- Consumes: `Line`/`solve` from Task 2 (`super::linear_program`), `rotate`, `density_adjusted_preferred`, `is_head_on_encounter`, `wall_avoidance_cost` is **not** used here (ORCA builds wall half-planes, not a wall cost) from Task 1, `AvoidanceInput`/`AvoidanceOutput`/`AvoidanceSolver`/`NeighborState` from `avoidance/mod.rs`, `crate::ids::AgentId`, `crate::geometry::Segment`, `crate::world::SolverStatus`.
- Produces (used by Task 6): `pub struct OrcaSolver { .. }` implementing `AvoidanceSolver`, `name() == "orca"`.

- [ ] **Step 1: Write `OrcaSolver` with neighbor and wall constraints**

Create `crates/crowd-core/src/avoidance/orca.rs`:

```rust
//! From-scratch reciprocal velocity obstacle (ORCA) avoidance, contract
//! section 6.2's second candidate.
//!
//! Unlike the sampled-velocity solver, this does not score many candidate
//! velocities: it builds one half-plane constraint per neighbor and per wall
//! directly in velocity space (the standard ORCA construction), then solves
//! for the feasible velocity closest to the preferred one via the sequential
//! linear program in `super::linear_program`.

use super::linear_program::{self, Line};
use super::{is_head_on_encounter, density_adjusted_preferred, rotate};
use super::{AvoidanceInput, AvoidanceOutput, AvoidanceSolver, NeighborState};
use crate::geometry::Segment;
use crate::ids::AgentId;
use crate::units::Vec2;
use crate::world::SolverStatus;

/// Assumed step, in seconds, used only when two discs already overlap: the
/// standard ORCA treatment of an existing penetration replaces the time
/// horizon with a short fixed step so the correction is proportionally more
/// urgent. Matches the kernel's default tick rate; it does not need to track
/// the actual tick length; it only shapes how sharply an overlap is
/// penalised.
const COLLISION_TIME_STEP: f32 = 1.0 / 30.0;

/// Fixed rotation applied to a head-on neighbor's constraint line so the
/// perfectly symmetric case resolves to one side rather than leaving both
/// mirrored agents to settle it independently (and possibly identically in
/// world space). A convention, like `sampled.rs`'s keep-left bias -- not an
/// ID comparison, which would fail for the same mirrored-geometry reason
/// documented there.
const HEAD_ON_TIE_BREAK_RADIANS: f32 = 0.03;

#[derive(Clone, Copy, Debug)]
pub struct OrcaSolver {
    /// How far ahead, in seconds, a neighbor's velocity obstacle is built.
    pub time_horizon: f32,
    /// Wall lookahead, in seconds. Shorter than `time_horizon` for the same
    /// reason `sampled.rs` gives: static geometry is avoided by turning, not
    /// long-range planning.
    pub wall_horizon: f32,
    pub head_on_cosine: f32,
    pub brake_speed_fraction: f32,
    pub personal_space: f32,
    pub density_speed_factor: f32,
}

impl Default for OrcaSolver {
    fn default() -> Self {
        Self {
            time_horizon: 3.0,
            wall_horizon: 2.0,
            head_on_cosine: 0.7,
            brake_speed_fraction: 0.5,
            personal_space: 0.45,
            density_speed_factor: 0.18,
        }
    }
}

fn det(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// One ORCA half-plane induced by `neighbor`, in this agent's own velocity
/// space, with each side credited half the necessary correction.
fn orca_line_for_neighbor(
    position: Vec2,
    self_velocity: Vec2,
    radius: f32,
    neighbor: &NeighborState,
    time_horizon: f32,
) -> Line {
    let relative_position = neighbor.position - position;
    let relative_velocity = self_velocity - neighbor.velocity;
    let combined_radius = radius + neighbor.radius;
    let dist_sq = relative_position.length_squared();
    let combined_radius_sq = combined_radius * combined_radius;

    let (direction, u) = if dist_sq > combined_radius_sq {
        let inv_time_horizon = 1.0 / time_horizon;
        let w = relative_velocity - relative_position * inv_time_horizon;
        let w_length_sq = w.length_squared();
        let dot1 = w.dot(relative_position);

        if dot1 < 0.0 && dot1 * dot1 > combined_radius_sq * w_length_sq {
            // Relative velocity is inside the truncated cone's near side:
            // project onto the cutoff circle.
            let w_length = w_length_sq.sqrt();
            let unit_w = w * (1.0 / w_length);
            let direction = Vec2::new(unit_w.y, -unit_w.x);
            let u = unit_w * (combined_radius * inv_time_horizon - w_length);
            (direction, u)
        } else {
            // Project onto whichever leg of the cone is nearer.
            let leg = (dist_sq - combined_radius_sq).sqrt();
            let direction = if det(relative_position, w) > 0.0 {
                Vec2::new(
                    relative_position.x * leg - relative_position.y * combined_radius,
                    relative_position.x * combined_radius + relative_position.y * leg,
                ) * (1.0 / dist_sq)
            } else {
                -Vec2::new(
                    relative_position.x * leg + relative_position.y * combined_radius,
                    -relative_position.x * combined_radius + relative_position.y * leg,
                ) * (1.0 / dist_sq)
            };
            let u = direction * relative_velocity.dot(direction) - relative_velocity;
            (direction, u)
        }
    } else {
        // Already overlapping: use the short collision time step instead of
        // the full horizon, matching an existing penetration's urgency.
        let inv_step = 1.0 / COLLISION_TIME_STEP;
        let w = relative_velocity - relative_position * inv_step;
        let w_length = w.length();
        let unit_w = if w_length > f32::MIN_POSITIVE {
            w * (1.0 / w_length)
        } else {
            Vec2::new(0.0, 1.0)
        };
        let direction = Vec2::new(unit_w.y, -unit_w.x);
        let u = unit_w * (combined_radius * inv_step - w_length);
        (direction, u)
    };

    Line {
        point: self_velocity + u * 0.5,
        direction,
    }
}

/// A wall's ORCA half-plane, built by treating the wall's closest point to
/// the agent as a zero-radius, zero-velocity neighbor (the agent's own
/// radius supplies the clearance), then undoing the reciprocal halving --
/// walls never yield, so the agent's own constraint carries the full
/// correction.
fn orca_line_for_wall(
    position: Vec2,
    self_velocity: Vec2,
    radius: f32,
    wall: &Segment,
    wall_horizon: f32,
) -> Option<Line> {
    let closest = wall.closest_point(position);
    if (closest - position).length_squared() <= f32::MIN_POSITIVE {
        return None;
    }
    let proxy = NeighborState {
        position: closest,
        velocity: Vec2::ZERO,
        radius: 0.0,
        agent_id: AgentId(u64::MAX),
    };
    let line = orca_line_for_neighbor(position, self_velocity, radius, &proxy, wall_horizon);
    Some(Line {
        point: self_velocity + (line.point - self_velocity) * 2.0,
        direction: line.direction,
    })
}

impl AvoidanceSolver for OrcaSolver {
    fn name(&self) -> &'static str {
        "orca"
    }

    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput {
        let preferred = density_adjusted_preferred(
            input.preferred,
            input.position,
            input.radius,
            input.neighbors,
            self.personal_space,
            self.density_speed_factor,
        )
        .clamp_length(input.max_speed);

        if preferred.length_squared() <= f32::MIN_POSITIVE
            && input.velocity.length_squared() <= f32::MIN_POSITIVE
        {
            return AvoidanceOutput {
                velocity: Vec2::ZERO,
                status: SolverStatus::Free,
            };
        }

        let heading = input.preferred.normalize_or_zero();

        // Fixed order by stable ID: the LP's infeasible-fallback path
        // depends on constraint order, so this must not depend on upstream
        // neighbor-list order.
        let mut ordered: Vec<&NeighborState> = input.neighbors.iter().collect();
        ordered.sort_by_key(|n| n.agent_id);

        let mut lines = Vec::with_capacity(ordered.len() + input.walls.len());
        for neighbor in ordered {
            let mut line = orca_line_for_neighbor(
                input.position,
                input.velocity,
                input.radius,
                neighbor,
                self.time_horizon,
            );
            if is_head_on_encounter(
                heading,
                input.position,
                input.velocity,
                neighbor,
                self.head_on_cosine,
            ) {
                line.direction = rotate(line.direction, HEAD_ON_TIE_BREAK_RADIANS);
            }
            lines.push(line);
        }
        for wall in input.walls {
            if let Some(line) = orca_line_for_wall(
                input.position,
                input.velocity,
                input.radius,
                wall,
                self.wall_horizon,
            ) {
                lines.push(line);
            }
        }

        let solved = linear_program::solve(&lines, input.max_speed, preferred);
        let preferred_speed = preferred.length();

        let status = if solved.length() < preferred_speed * self.brake_speed_fraction {
            SolverStatus::Braking
        } else if (solved - preferred).length() > 1e-3 {
            SolverStatus::Avoiding
        } else {
            SolverStatus::Free
        };

        AvoidanceOutput {
            velocity: solved.clamp_length(input.max_speed),
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Vec2;

    fn solver() -> OrcaSolver {
        OrcaSolver::default()
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
        assert!((out.velocity - preferred).length() < 1e-3);
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
        assert!(
            out.velocity.y.abs() > 0.01,
            "no lateral deflection: {out:?}"
        );
    }

    #[test]
    fn head_on_agents_choose_opposite_sides() {
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
        assert!(
            a.velocity.y * b.velocity.y < 0.0,
            "agents chose the same world-space side: a={:?} b={:?}",
            a.velocity,
            b.velocity
        );
    }

    #[test]
    fn head_on_side_choice_does_not_depend_on_id_ordering() {
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
        let lower_id = solver().solve(&input(5, Vec2::ZERO, preferred, preferred, &neighbors_low, &[]));
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
    fn a_wall_ahead_deflects_the_agent() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(out.velocity.x < preferred.x, "agent drove into the wall");
    }

    #[test]
    fn a_boxed_in_agent_brakes_rather_than_escaping() {
        let walls = [
            Segment::new(Vec2::new(0.8, -2.0), Vec2::new(0.8, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.8), Vec2::new(2.0, 0.8)),
            Segment::new(Vec2::new(-2.0, -0.8), Vec2::new(2.0, -0.8)),
            Segment::new(Vec2::new(-0.8, -2.0), Vec2::new(-0.8, 2.0)),
        ];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert!(out.velocity.is_finite());
        assert!(
            out.velocity.length() < preferred.length(),
            "did not brake in a fully boxed-in space: {out:?}"
        );
    }

    #[test]
    fn an_agent_inside_a_wall_is_given_a_way_out() {
        let wall = [Segment::new(Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0))];
        let position = Vec2::new(0.1, 0.0);
        let out = solver().solve(&input(
            1,
            position,
            Vec2::ZERO,
            Vec2::new(-1.35, 0.0),
            &[],
            &wall,
        ));
        assert!(
            out.velocity.x >= -1e-3,
            "steered deeper into the wall: {:?}",
            out.velocity
        );
    }

    #[test]
    fn the_solution_never_exceeds_max_speed() {
        let preferred = Vec2::new(100.0, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, preferred, &[], &[]));
        assert!(
            out.velocity.length() <= 2.0 + 1e-3,
            "got {}",
            out.velocity.length()
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
        assert_eq!(solver().name(), "orca");
    }
}
```

- [ ] **Step 2: Wire the module into `avoidance/mod.rs`**

Add, near `pub mod sampled; pub use sampled::SampledVelocitySolver;`:

```rust
pub mod orca;
pub use orca::OrcaSolver;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p crowd-core avoidance::orca`
Expected: all 12 tests PASS.

If `head_on_agents_choose_opposite_sides` or `head_on_side_choice_does_not_depend_on_id_ordering` fails, flip the sign of `HEAD_ON_TIE_BREAK_RADIANS` first — this is the single most likely sign error in a from-scratch ORCA port, exactly the kind of plan defect the kernel slice's own task log repeatedly found via its tests rather than by inspection. If `a_boxed_in_agent_brakes_rather_than_escaping` fails with a non-finite result instead of a slow one, check `solve_fallback`'s `denominator.abs() <= EPSILON` branch in Task 2's module first.

- [ ] **Step 4: Run clippy and the workspace suite**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/crowd-core/src/avoidance/orca.rs crates/crowd-core/src/avoidance/mod.rs
git commit -m "Add the from-scratch ORCA-style avoidance solver"
```

---

## Task 4: `AnticipatorySolver`, part 1 — walls, goal-seeking, density, no neighbor collision yet

**Files:**
- Create: `crates/crowd-core/src/avoidance/anticipatory.rs`
- Modify: `crates/crowd-core/src/avoidance/mod.rs` (add `pub mod anticipatory; pub use anticipatory::AnticipatorySolver;`)

**Interfaces:**
- Consumes: `sample_candidates`, `density_adjusted_preferred`, `side_bias_cost`, `wall_avoidance_cost`, `OVERLAP_URGENCY`, `MIN_TIME_FOR_COST` from Task 1.
- Produces (used by Task 5, which adds to this same file): `pub struct AnticipatorySolver { .. }` implementing `AvoidanceSolver`, `name() == "anticipatory"`.

This task deliberately does **not** yet give the solver any neighbor-collision awareness — only walls, goal-seeking, smoothness, side-bias infrastructure (inert until Task 5 supplies interacting neighbors), and density-based speed reduction. Task 5 adds the scoped multi-step lookahead that makes this solver's neighbor handling real. Splitting it this way keeps every intermediate state honestly tested: this task's tests never claim neighbor-avoidance behavior that isn't implemented yet.

- [ ] **Step 1: Write the solver skeleton**

Create `crates/crowd-core/src/avoidance/anticipatory.rs`:

```rust
//! Scoped, multi-step anticipatory avoidance, contract section 6.2's third
//! candidate.
//!
//! Distinct from both other solvers in *where* it spends its attention:
//! sampled-velocity scores every neighbor with one analytic
//! time-to-collision per candidate; ORCA solves one closed-form half-plane
//! per neighbor. This solver instead ranks neighbors by distance and gives
//! only the nearest few (`lookahead_neighbors`) a multi-step constant-velocity
//! extrapolation, so its cost is bounded by that count rather than by the
//! full neighbor list -- see `Task 5` (`lookahead_collision_cost`) for the
//! part that makes this solver's name accurate. This file starts with
//! everything *except* that: walls, goal-seeking, smoothness, side bias, and
//! density-based speed reduction, all reused unchanged from the shared
//! helpers `sampled.rs` also uses.

use super::{
    density_adjusted_preferred, sample_candidates, side_bias_cost, wall_avoidance_cost,
    AvoidanceInput, AvoidanceOutput, AvoidanceSolver, NeighborState, MIN_TIME_FOR_COST,
    OVERLAP_URGENCY,
};
use crate::units::Vec2;
use crate::world::SolverStatus;

#[derive(Clone, Copy, Debug)]
pub struct AnticipatorySolver {
    pub speed_samples: u32,
    pub heading_samples: u32,
    pub time_horizon: f32,
    pub wall_horizon: f32,
    /// How many of the nearest neighbors get full multi-step lookahead.
    /// Wired up in Task 5; unused until then.
    pub lookahead_neighbors: usize,
    /// How many sub-steps the lookahead walks across `time_horizon`. Wired up
    /// in Task 5; unused until then.
    pub lookahead_steps: u32,
    pub goal_weight: f32,
    pub collision_weight: f32,
    pub wall_weight: f32,
    pub smoothness_weight: f32,
    pub side_bias_weight: f32,
    /// Extra collision weight carried by the higher-ID agent in a crossing
    /// conflict. Wired up in Task 5; unused until then.
    pub yield_factor: f32,
    pub brake_speed_fraction: f32,
    pub personal_space: f32,
    pub density_speed_factor: f32,
    pub head_on_cosine: f32,
    /// Cheap repulsion weight for neighbors past the lookahead cutoff. Wired
    /// up in Task 5; unused until then.
    pub far_field_weight: f32,
}

impl Default for AnticipatorySolver {
    fn default() -> Self {
        Self {
            speed_samples: 3,
            heading_samples: 16,
            time_horizon: 3.0,
            wall_horizon: 2.0,
            lookahead_neighbors: 4,
            lookahead_steps: 3,
            goal_weight: 1.0,
            collision_weight: 2.0,
            wall_weight: 1.5,
            smoothness_weight: 0.35,
            side_bias_weight: 0.6,
            yield_factor: 1.4,
            brake_speed_fraction: 0.5,
            personal_space: 0.45,
            density_speed_factor: 0.18,
            head_on_cosine: 0.7,
            far_field_weight: 0.15,
        }
    }
}

impl AvoidanceSolver for AnticipatorySolver {
    fn name(&self) -> &'static str {
        "anticipatory"
    }

    fn solve(&self, input: &AvoidanceInput<'_>) -> AvoidanceOutput {
        let preferred = density_adjusted_preferred(
            input.preferred,
            input.position,
            input.radius,
            input.neighbors,
            self.personal_space,
            self.density_speed_factor,
        )
        .clamp_length(input.max_speed);

        if preferred.length_squared() <= f32::MIN_POSITIVE
            && input.velocity.length_squared() <= f32::MIN_POSITIVE
        {
            return AvoidanceOutput {
                velocity: Vec2::ZERO,
                status: SolverStatus::Free,
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

        let evaluate = |candidate: Vec2,
                        best_velocity: &mut Vec2,
                        best_cost: &mut f32,
                        best_ttc: &mut f32| {
            let (wall_cost, wall_ttc) = wall_avoidance_cost(
                input.position,
                input.max_speed,
                candidate,
                input.radius,
                input.walls,
                self.wall_horizon,
                self.collision_weight,
                self.wall_weight,
                OVERLAP_URGENCY,
                MIN_TIME_FOR_COST,
            );
            let bias_cost = side_bias_cost(
                input.preferred,
                input.position,
                input.velocity,
                input.neighbors,
                candidate,
                self.head_on_cosine,
                self.side_bias_weight,
            );
            let cost = self.goal_weight * (candidate - preferred).length()
                + self.smoothness_weight * (candidate - input.velocity).length()
                + wall_cost
                + bias_cost;
            if cost < *best_cost {
                *best_cost = cost;
                *best_velocity = candidate;
                *best_ttc = wall_ttc;
            }
        };

        evaluate(preferred, &mut best_velocity, &mut best_cost, &mut best_ttc);
        let speed_reference = preferred_speed.max(input.velocity.length());
        sample_candidates(
            heading,
            speed_reference,
            self.speed_samples,
            self.heading_samples,
            |candidate| evaluate(candidate, &mut best_velocity, &mut best_cost, &mut best_ttc),
        );
        evaluate(Vec2::ZERO, &mut best_velocity, &mut best_cost, &mut best_ttc);

        let status = if best_velocity.length() < preferred_speed * self.brake_speed_fraction {
            SolverStatus::Braking
        } else if (best_velocity - preferred).length() > 1e-3 {
            SolverStatus::Avoiding
        } else {
            SolverStatus::Free
        };

        AvoidanceOutput {
            velocity: best_velocity.clamp_length(input.max_speed),
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Segment;
    use crate::ids::AgentId;

    fn solver() -> AnticipatorySolver {
        AnticipatorySolver::default()
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
    fn a_wall_ahead_deflects_the_agent() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_ne!(out.status, SolverStatus::Free);
        assert!(out.velocity.x < preferred.x, "agent drove into the wall");
    }

    #[test]
    fn a_boxed_in_agent_brakes_rather_than_escaping() {
        let walls = [
            Segment::new(Vec2::new(0.8, -2.0), Vec2::new(0.8, 2.0)),
            Segment::new(Vec2::new(-2.0, 0.8), Vec2::new(2.0, 0.8)),
            Segment::new(Vec2::new(-2.0, -0.8), Vec2::new(2.0, -0.8)),
            Segment::new(Vec2::new(-0.8, -2.0), Vec2::new(-0.8, 2.0)),
        ];
        let preferred = Vec2::new(1.35, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_eq!(out.status, SolverStatus::Braking);
        assert!(out.velocity.length() < preferred.length());
    }

    #[test]
    fn an_agent_inside_a_wall_is_given_a_way_out() {
        let wall = [Segment::new(Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0))];
        let position = Vec2::new(0.1, 0.0);
        let out = solver().solve(&input(
            1,
            position,
            Vec2::ZERO,
            Vec2::new(-1.35, 0.0),
            &[],
            &wall,
        ));
        assert!(
            out.velocity.x >= 0.0,
            "steered deeper into the wall: {:?}",
            out.velocity
        );
    }

    #[test]
    fn the_solution_never_exceeds_max_speed() {
        let preferred = Vec2::new(100.0, 0.0);
        let out = solver().solve(&input(1, Vec2::ZERO, Vec2::ZERO, preferred, &[], &[]));
        assert!(out.velocity.length() <= 2.0 + 1e-4, "got {}", out.velocity.length());
    }

    #[test]
    fn the_output_is_always_finite() {
        let walls = [Segment::new(Vec2::ZERO, Vec2::ZERO)];
        let out = solver().solve(&input(
            1,
            Vec2::ZERO,
            Vec2::ZERO,
            Vec2::new(1.35, 0.0),
            &[],
            &walls,
        ));
        assert!(out.velocity.is_finite(), "got {:?}", out.velocity);
    }

    #[test]
    fn solving_is_deterministic_for_identical_input() {
        let walls = [Segment::new(Vec2::new(3.0, -5.0), Vec2::new(3.0, 5.0))];
        let preferred = Vec2::new(1.35, 0.0);
        let first = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        let second = solver().solve(&input(1, Vec2::ZERO, preferred, preferred, &[], &walls));
        assert_eq!(first.velocity, second.velocity);
        assert_eq!(first.status, second.status);
    }

    #[test]
    fn the_solver_reports_its_name() {
        assert_eq!(solver().name(), "anticipatory");
    }

    #[test]
    fn dense_neighbors_reduce_speed() {
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
}
```

- [ ] **Step 2: Wire the module into `avoidance/mod.rs`**

Add, near the other `pub mod` / `pub use` lines:

```rust
pub mod anticipatory;
pub use anticipatory::AnticipatorySolver;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p crowd-core avoidance::anticipatory`
Expected: all 9 tests PASS.

- [ ] **Step 4: Run clippy and the workspace suite**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean. (Unused-field warnings on `lookahead_neighbors`, `lookahead_steps`, `yield_factor`, `far_field_weight` should not fire since they are `pub` struct fields, which clippy does not flag as dead code — if it does, add `#[allow(dead_code)]` is **not** the fix; instead double check they truly are `pub`, since a private unused field is the actual dead-code case clippy is right to flag.)

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/crowd-core/src/avoidance/anticipatory.rs crates/crowd-core/src/avoidance/mod.rs
git commit -m "Add the anticipatory solver skeleton: walls, goal-seeking, density"
```

---

## Task 5: `AnticipatorySolver`, part 2 — scoped multi-step lookahead

**Files:**
- Modify: `crates/crowd-core/src/avoidance/anticipatory.rs`

**Interfaces:**
- Consumes: everything from Task 4's version of this file, plus `Vec2::distance_squared`.
- Produces: the completed `AnticipatorySolver`, now genuinely using `lookahead_neighbors`, `lookahead_steps`, `yield_factor`, `far_field_weight`.

**A known, documented limitation carried into the comparison report (Task 10):** because each scoped neighbor is extrapolated at its *current* velocity held constant, this is not more accurate than the sampled solver's closed-form time-to-collision for straight-line motion — multi-step sampling can in principle tunnel through a collision that falls between two sub-steps, the same failure mode `geometry.rs`'s doc comment describes for segment collision. This solver's actual advantage is bounded attention (cost independent of total neighbor count past `lookahead_neighbors`), not superior single-pair physics. Say this plainly in the report; do not oversell it.

- [ ] **Step 1: Add the scoped ranking, far-field cost, and lookahead collision cost**

In `crates/crowd-core/src/avoidance/anticipatory.rs`, add a constant near the top (below the existing `use` block):

```rust
/// Floor on predicted separation, in meters, when converting it to a cost —
/// the lookahead analogue of `MIN_TIME_FOR_COST`.
const MIN_SEPARATION_FOR_COST: f32 = 0.1;
```

Add these methods to `impl AnticipatorySolver` (create the `impl` block if `solve` currently lives directly under `impl AvoidanceSolver for AnticipatorySolver` — add a separate `impl AnticipatorySolver { .. }` block above it):

```rust
impl AnticipatorySolver {
    /// Cheap directional repulsion for neighbors past the lookahead cutoff.
    /// Only candidates heading *toward* a nearby far-field neighbor are
    /// penalised, so this is a real gradient rather than a constant that
    /// would cancel out of the argmin.
    fn far_field_cost(&self, position: Vec2, radius: f32, candidate: Vec2, far: &[&NeighborState]) -> f32 {
        let mut cost = 0.0;
        for neighbor in far {
            let clearance = radius + neighbor.radius + self.personal_space;
            let offset = neighbor.position - position;
            let dist_sq = offset.length_squared();
            if dist_sq < clearance * clearance {
                let dist = dist_sq.sqrt().max(0.05);
                let toward = candidate.dot(offset.normalize_or_zero()).max(0.0);
                cost += self.far_field_weight * toward / dist;
            }
        }
        cost
    }

    /// Collision cost and earliest predicted contact for one candidate,
    /// against only the scoped (nearest `lookahead_neighbors`) threats, via a
    /// fixed number of constant-velocity sub-steps across `time_horizon`.
    fn lookahead_collision_cost(
        &self,
        input: &AvoidanceInput<'_>,
        candidate: Vec2,
        scoped: &[&NeighborState],
    ) -> (f32, f32) {
        let mut cost = 0.0;
        let mut earliest = f32::INFINITY;
        let step_dt = self.time_horizon / self.lookahead_steps as f32;

        for neighbor in scoped {
            let combined_radius = input.radius + neighbor.radius;
            // The higher stable ID yields, exactly as in `sampled.rs`: a
            // perpendicular conflict is symmetric under the keep-left rule,
            // so without this both agents derive the identical choice.
            let yield_weight = if input.agent_id > neighbor.agent_id {
                self.yield_factor
            } else {
                1.0
            };

            let mut min_separation = f32::INFINITY;
            let mut min_separation_time = f32::INFINITY;
            for step in 1..=self.lookahead_steps {
                let t = step_dt * step as f32;
                let self_pos = input.position + candidate * t;
                let neighbor_pos = neighbor.position + neighbor.velocity * t;
                let separation = self_pos.distance_squared(neighbor_pos).sqrt() - combined_radius;
                if separation < min_separation {
                    min_separation = separation;
                    min_separation_time = t;
                }
            }

            if min_separation <= 0.0 {
                let offset = neighbor.position - input.position;
                let direction = offset.normalize_or_zero();
                let relative_velocity = neighbor.velocity - candidate;
                let separation_rate = relative_velocity.dot(direction);
                let relief = (separation_rate / input.max_speed.max(0.1)).clamp(0.0, 1.0);
                cost += self.collision_weight * yield_weight * OVERLAP_URGENCY * (1.0 - relief);
                earliest = earliest.min(min_separation_time);
            } else if min_separation < self.personal_space {
                cost += self.collision_weight * yield_weight / min_separation.max(MIN_SEPARATION_FOR_COST);
                earliest = earliest.min(min_separation_time);
            }
        }
        (cost, earliest)
    }
}
```

Now update `solve` to rank neighbors and use both new costs. Replace the existing `evaluate` closure and the code around it (from `let mut best_velocity = Vec2::ZERO;` through the `evaluate(Vec2::ZERO, ...)` call) with:

```rust
        // Rank by distance, breaking ties by stable ID so the scoped/far
        // split never depends on upstream neighbor-list order.
        let mut ranked: Vec<&NeighborState> = input.neighbors.iter().collect();
        ranked.sort_by(|a, b| {
            let da = input.position.distance_squared(a.position);
            let db = input.position.distance_squared(b.position);
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.agent_id.cmp(&b.agent_id))
        });
        let (scoped, far): (&[&NeighborState], &[&NeighborState]) =
            if ranked.len() > self.lookahead_neighbors {
                ranked.split_at(self.lookahead_neighbors)
            } else {
                (&ranked[..], &[])
            };

        let mut best_velocity = Vec2::ZERO;
        let mut best_cost = f32::INFINITY;
        let mut best_ttc = f32::INFINITY;

        let evaluate = |candidate: Vec2,
                        best_velocity: &mut Vec2,
                        best_cost: &mut f32,
                        best_ttc: &mut f32| {
            let (near_cost, near_ttc) = self.lookahead_collision_cost(input, candidate, scoped);
            let (wall_cost, wall_ttc) = wall_avoidance_cost(
                input.position,
                input.max_speed,
                candidate,
                input.radius,
                input.walls,
                self.wall_horizon,
                self.collision_weight,
                self.wall_weight,
                OVERLAP_URGENCY,
                MIN_TIME_FOR_COST,
            );
            let far_cost = self.far_field_cost(input.position, input.radius, candidate, far);
            let bias_cost = side_bias_cost(
                input.preferred,
                input.position,
                input.velocity,
                input.neighbors,
                candidate,
                self.head_on_cosine,
                self.side_bias_weight,
            );
            let cost = self.goal_weight * (candidate - preferred).length()
                + self.smoothness_weight * (candidate - input.velocity).length()
                + near_cost
                + wall_cost
                + far_cost
                + bias_cost;
            if cost < *best_cost {
                *best_cost = cost;
                *best_velocity = candidate;
                *best_ttc = near_ttc.min(wall_ttc);
            }
        };

        evaluate(preferred, &mut best_velocity, &mut best_cost, &mut best_ttc);
        let speed_reference = preferred_speed.max(input.velocity.length());
        sample_candidates(
            heading,
            speed_reference,
            self.speed_samples,
            self.heading_samples,
            |candidate| evaluate(candidate, &mut best_velocity, &mut best_cost, &mut best_ttc),
        );
        evaluate(Vec2::ZERO, &mut best_velocity, &mut best_cost, &mut best_ttc);
```

- [ ] **Step 2: Add the new tests**

Add to the `mod tests` block in the same file:

```rust
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
        assert!(
            a.velocity.y * b.velocity.y < 0.0,
            "agents chose the same world-space side: a={:?} b={:?}",
            a.velocity,
            b.velocity
        );
    }

    #[test]
    fn the_higher_id_yields_more_in_a_crossing_conflict() {
        let crossing_neighbor = |id: u64| {
            [NeighborState {
                position: Vec2::new(2.0, -2.0),
                velocity: Vec2::new(0.0, 1.35),
                radius: 0.3,
                agent_id: AgentId(id),
            }]
        };
        let preferred = Vec2::new(1.35, 0.0);
        let lower = solver().solve(&input(10, Vec2::ZERO, preferred, preferred, &crossing_neighbor(20), &[]));
        let higher = solver().solve(&input(30, Vec2::ZERO, preferred, preferred, &crossing_neighbor(20), &[]));
        assert!(
            higher.velocity.length() <= lower.velocity.length() + 1e-3,
            "the higher ID must not push harder: lower={:?} higher={:?}",
            lower.velocity,
            higher.velocity
        );
    }

    #[test]
    fn only_the_nearest_k_neighbors_receive_full_lookahead() {
        // Five threats, but lookahead_neighbors defaults to 4: the 5th
        // (furthest) must not get the full multi-step treatment, so removing
        // it changes the result less than removing one of the nearest four.
        let mut base = solver();
        base.lookahead_neighbors = 2;
        let near: Vec<NeighborState> = (0..2)
            .map(|i| NeighborState {
                position: Vec2::new(1.0, if i == 0 { 0.3 } else { -0.3 }),
                velocity: Vec2::new(-1.35, 0.0),
                radius: 0.3,
                agent_id: AgentId(10 + i as u64),
            })
            .collect();
        let far_threat = NeighborState {
            position: Vec2::new(1.0, 20.0),
            velocity: Vec2::new(0.0, -1.35),
            radius: 0.3,
            agent_id: AgentId(99),
        };
        let preferred = Vec2::new(1.35, 0.0);
        let without_far = base.solve(&input(1, Vec2::ZERO, preferred, preferred, &near, &[]));
        let with_far_neighbors: Vec<NeighborState> =
            near.iter().cloned().chain(std::iter::once(far_threat)).collect();
        let with_far = base.solve(&input(1, Vec2::ZERO, preferred, preferred, &with_far_neighbors, &[]));
        assert!(
            (without_far.velocity - with_far.velocity).length() < 0.05,
            "a far, out-of-scope neighbor changed the outcome as much as a scoped one: \
             without={:?} with={:?}",
            without_far.velocity,
            with_far.velocity
        );
    }

    #[test]
    fn solving_is_deterministic_with_neighbors_present() {
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
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p crowd-core avoidance::anticipatory`
Expected: all 14 tests PASS (9 from Task 4 plus 5 new).

If `only_the_nearest_k_neighbors_receive_full_lookahead` fails because the far neighbor changes the result *more* than expected, check that the far neighbor's distance (20 m) actually exceeds every scoped neighbor's, and that `ranked.split_at` is slicing at `lookahead_neighbors` from the front (nearest), not the back.

- [ ] **Step 4: Run clippy and the workspace suite**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/crowd-core/src/avoidance/anticipatory.rs
git commit -m "Add scoped multi-step lookahead to the anticipatory solver"
```

---

## Task 6: Parametrize determinism and density-fuzz tests across all three solvers

**Files:**
- Modify: `crates/crowd-core/tests/determinism.rs`
- Modify: `crates/crowd-core/tests/fuzz_density.rs`

**Interfaces:**
- Consumes: `crowd_core::avoidance::{SampledVelocitySolver, OrcaSolver, AnticipatorySolver}`, all implementing `AvoidanceSolver`.

- [ ] **Step 1: Add a solver-selecting helper to `determinism.rs`**

In `crates/crowd-core/tests/determinism.rs`, replace the imports and `simulate` helper:

```rust
use crowd_core::avoidance::{AnticipatorySolver, AvoidanceSolver, OrcaSolver, SampledVelocitySolver};
use crowd_core::ids::AgentId;
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};
use crowd_core::world::World;
use std::collections::BTreeMap;

const SOLVER_NAMES: [&str; 3] = ["sampled_velocity", "orca", "anticipatory"];

fn boxed_solver(name: &str) -> Box<dyn AvoidanceSolver> {
    match name {
        "sampled_velocity" => Box::new(SampledVelocitySolver::default()),
        "orca" => Box::new(OrcaSolver::default()),
        "anticipatory" => Box::new(AnticipatorySolver::default()),
        other => panic!("unknown solver: {other}"),
    }
}

fn simulate(solver_name: &str, scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
    let scene = scenes::build(scene_name, agents, seed)
        .expect("known scene")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(scene, boxed_solver(solver_name), SimConfig::default());
    sim.run(ticks);
    sim
}
```

Update every call site of `simulate(...)` in this file to pass a `solver_name` first argument, and wrap each test body in `for solver_name in SOLVER_NAMES { ... }`:

```rust
#[test]
fn repeated_runs_are_bitwise_identical_in_every_scene() {
    for solver_name in SOLVER_NAMES {
        for name in scenes::SCENE_NAMES {
            let a = simulate(solver_name, name, 200, 2026, 300);
            let b = simulate(solver_name, name, 200, 2026, 300);
            assert_eq!(a.state_hash(), b.state_hash(), "{solver_name}/{name} diverged");
            assert_eq!(
                state_by_id(a.world()),
                state_by_id(b.world()),
                "{solver_name}/{name}"
            );
        }
    }
}

#[test]
fn state_hashes_agree_at_every_tick() {
    for solver_name in SOLVER_NAMES {
        let scene = |seed| {
            scenes::build("bottleneck", 150, seed)
                .unwrap()
                .compile()
                .unwrap()
        };
        let mut a = Simulation::new(scene(7), boxed_solver(solver_name), SimConfig::default());
        let mut b = Simulation::new(scene(7), boxed_solver(solver_name), SimConfig::default());
        for tick in 0..400 {
            a.step();
            b.step();
            assert_eq!(
                a.state_hash(),
                b.state_hash(),
                "{solver_name} diverged at tick {tick}"
            );
        }
    }
}

#[test]
fn permuting_spawn_region_order_does_not_change_results() {
    for solver_name in SOLVER_NAMES {
        let mut forward = scenes::build("bidirectional_corridor", 200, 99).unwrap();
        let mut reversed = forward.clone();
        reversed.spawns.reverse();

        forward.duration_ticks = 300;
        reversed.duration_ticks = 300;

        let mut a = Simulation::new(
            forward.compile().unwrap(),
            boxed_solver(solver_name),
            SimConfig::default(),
        );
        let mut b = Simulation::new(
            reversed.compile().unwrap(),
            boxed_solver(solver_name),
            SimConfig::default(),
        );
        a.run(300);
        b.run(300);

        assert_eq!(
            state_by_id(a.world()),
            state_by_id(b.world()),
            "{solver_name}: results depended on spawn region ordering"
        );
    }
}

#[test]
fn adding_one_agent_does_not_change_existing_agents_attributes() {
    for solver_name in SOLVER_NAMES {
        let small = simulate(solver_name, "crossing", 100, 5, 1);
        let large = simulate(solver_name, "crossing", 101, 5, 1);

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
                "{solver_name}: agent {id:?} was reshuffled by adding another agent"
            );
        }
    }
}

#[test]
fn changing_the_seed_changes_the_outcome() {
    for solver_name in SOLVER_NAMES {
        let a = simulate(solver_name, "crossing", 200, 1, 200);
        let b = simulate(solver_name, "crossing", 200, 2, 200);
        assert_ne!(a.state_hash(), b.state_hash(), "{solver_name}");
    }
}
```

Leave `no_spawn_errors_occur_in_any_scene` as-is (spawn errors do not depend on the avoidance solver) but update its one `simulate(...)` call to pass `"sampled_velocity"` as the first argument, since the helper's signature changed:

```rust
#[test]
fn no_spawn_errors_occur_in_any_scene() {
    for name in scenes::SCENE_NAMES {
        let sim = simulate("sampled_velocity", name, 500, 3, 100);
        assert!(sim.spawn_errors().is_empty(), "{name}: {:?}", sim.spawn_errors());
    }
}
```

- [ ] **Step 2: Do the same for `fuzz_density.rs`**

In `crates/crowd-core/tests/fuzz_density.rs`, replace the imports and `stress` helper:

```rust
use crowd_core::avoidance::{AnticipatorySolver, AvoidanceSolver, OrcaSolver, SampledVelocitySolver};
use crowd_core::scenes;
use crowd_core::sim::{SimConfig, Simulation};

const SOLVER_NAMES: [&str; 3] = ["sampled_velocity", "orca", "anticipatory"];

fn boxed_solver(name: &str) -> Box<dyn AvoidanceSolver> {
    match name {
        "sampled_velocity" => Box::new(SampledVelocitySolver::default()),
        "orca" => Box::new(OrcaSolver::default()),
        "anticipatory" => Box::new(AnticipatorySolver::default()),
        other => panic!("unknown solver: {other}"),
    }
}

fn stress(solver_name: &str, scene_name: &str, agents: u32, seed: u64, ticks: u64) -> Simulation {
    let scene = scenes::build(scene_name, agents, seed)
        .expect("known scene")
        .compile()
        .expect("scene compiles");
    let mut sim = Simulation::new(scene, boxed_solver(solver_name), SimConfig::default());
    sim.run(ticks);
    sim
}
```

Wrap each of the four existing test bodies in `for solver_name in SOLVER_NAMES { ... }` and pass `solver_name` as the first argument to every `stress(...)` call, e.g.:

```rust
#[test]
fn no_agent_state_goes_non_finite_under_density() {
    for solver_name in SOLVER_NAMES {
        for seed in 0..8u64 {
            for scene in scenes::SCENE_NAMES {
                let sim = stress(solver_name, scene, 800, seed, 400);
                for slot in 0..sim.world().len() {
                    let position = sim.world().position(slot as u32);
                    let velocity = sim.world().velocity(slot as u32);
                    assert!(
                        position.is_finite() && velocity.is_finite(),
                        "{solver_name}/{scene} seed {seed} slot {slot} went non-finite"
                    );
                    assert!(sim.world().yaw[slot].is_finite());
                }
            }
        }
    }
}
```

(Apply the same wrapping pattern to `no_agent_escapes_far_beyond_the_scene_bounds`, `speeds_never_exceed_the_per_agent_maximum`, and `the_crowd_does_not_deadlock_wholesale`, threading `solver_name` through their existing loop bodies and assertion messages the same way.)

- [ ] **Step 3: Run both suites in release**

Run: `cargo test --release -p crowd-core --test determinism`
Expected: all 6 tests PASS.

Run: `cargo test --release -p crowd-core --test fuzz_density`
Expected: all 4 tests PASS. This is the slow one — budget several minutes (3 solvers x up to 8 seeds x 5 scenes x 800 agents x 400-900 ticks). If `the_crowd_does_not_deadlock_wholesale` fails for `orca` or `anticipatory` specifically, that is real information for the Task 10 report, not necessarily a bug to fix blindly — but do check for an obvious sign error first (e.g., a wrong-signed `HEAD_ON_TIE_BREAK_RADIANS` making agents steer toward each other) before concluding the solver is genuinely worse here.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/crowd-core/tests/determinism.rs crates/crowd-core/tests/fuzz_density.rs
git commit -m "Run the determinism and density-fuzz suites against all three solvers"
```

---

## Task 7: `crowd-bench` solver selection and baseline schema

**Files:**
- Modify: `crates/crowd-bench/src/report.rs`
- Modify: `crates/crowd-bench/src/main.rs`
- Modify: `crates/crowd-bench/src/baseline.rs`
- Modify: `benchmarks/baselines/bottleneck.json`, `circle.json`, `crossing.json`, `dense_flow.json`, `l_corridor.json`, `bidirectional_corridor.json`

**Interfaces:**
- Produces (used by Task 8): `pub enum SolverKind { SampledVelocity, Orca, Anticipatory }` with `FromStr`-style parsing via a `parse_solver_kind` function, and `RunOptions.solver: SolverKind`.
- Consumes: `crowd_core::avoidance::{SampledVelocitySolver, OrcaSolver, AnticipatorySolver}`.

- [ ] **Step 1: Add `SolverKind` and thread it through `run_scene`**

In `crates/crowd-bench/src/report.rs`, change the import:

```rust
use crowd_core::avoidance::{AnticipatorySolver, AvoidanceSolver, OrcaSolver, SampledVelocitySolver};
```

Add, above `RunOptions`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolverKind {
    SampledVelocity,
    Orca,
    Anticipatory,
}

impl SolverKind {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "sampled_velocity" => Ok(SolverKind::SampledVelocity),
            "orca" => Ok(SolverKind::Orca),
            "anticipatory" => Ok(SolverKind::Anticipatory),
            other => Err(format!(
                "unknown solver: {other}; known solvers: sampled_velocity, orca, anticipatory"
            )),
        }
    }

    fn build(self) -> Box<dyn AvoidanceSolver> {
        match self {
            SolverKind::SampledVelocity => Box::new(SampledVelocitySolver::default()),
            SolverKind::Orca => Box::new(OrcaSolver::default()),
            SolverKind::Anticipatory => Box::new(AnticipatorySolver::default()),
        }
    }
}
```

Add a `solver: SolverKind` field to `RunOptions`:

```rust
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub svg: bool,
    pub out_dir: PathBuf,
    pub solver: SolverKind,
}
```

In `run_scene`, replace:

```rust
    let mut sim = Simulation::new(scene, Box::new(SampledVelocitySolver::default()), config);
```

with:

```rust
    let mut sim = Simulation::new(scene, options.solver.build(), config);
```

Update the three existing tests' `options(...)` helper in `report.rs`'s test module to include `solver: SolverKind::SampledVelocity,` in the struct literal (matching the default CLI behavior before this task).

- [ ] **Step 2: Add `--solver` to the CLI**

In `crates/crowd-bench/src/main.rs`, update the `usage()` string to mention `[--solver NAME]` on the `run`, `baseline`, and `check` lines, add `solver: crate::report::SolverKind` to `Args`, default it to `SolverKind::SampledVelocity` in `parse_args`'s initial `Args { .. }`, and add a match arm:

```rust
            "--solver" => {
                index += 1;
                let name = raw.get(index).ok_or("--solver needs a value")?;
                args.solver = crate::report::SolverKind::parse(name)?;
            }
```

Update `options_for` to pass `solver: args.solver,` into the `RunOptions` literal it builds.

`command_check`'s inner `Args { .. }` construction (built from a stored baseline) also needs a `solver` field — set it to the baseline's own recorded solver, added in Step 3 below, rather than `args.solver`, so `check` always replays a baseline against the solver it was captured with regardless of what `--solver` the invocation passed.

- [ ] **Step 3: Add the `solver` field to the baseline schema**

In `crates/crowd-bench/src/baseline.rs`, find the `Baseline` struct and `from_report`/`compare` functions (read the file first to get exact field names and the `compare` signature). Add a `pub solver: String` field to `Baseline`, populate it from `report.solver.clone()` in `from_report`, and in `compare`, return an immediate mismatch (not a metric drift) if `stored.solver != report.solver` — add a `solver_mismatch: Option<(String, String)>` field to whatever comparison-result type `compare` returns, or a dedicated early-return variant if `compare`'s signature makes that cleaner; match the existing error-reporting style in that file rather than introducing a new one.

Update `command_check` in `main.rs` to print a distinct message and fail when `comparison.solver_mismatch` is `Some`, before printing per-metric drift.

- [ ] **Step 4: Migrate the six committed baseline files**

Add `"solver": "sampled_velocity",` to each of the six files under `benchmarks/baselines/` (`bottleneck.json`, `circle.json`, `crossing.json`, `dense_flow.json`, `l_corridor.json`, `bidirectional_corridor.json`), matching whatever key ordering `serde_json` produces elsewhere in those files (open one to check).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p crowd-bench`
Expected: all pass, including the updated `report.rs` tests.

Run: `cargo run --release -p crowd-bench -- check --agents 1000`
Expected: `OK` for all five scenes (baselines were captured with `sampled_velocity`, the CLI default, and now carry that field explicitly, so nothing should have changed numerically).

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/crowd-bench/src crates/crowd-bench/Cargo.toml benchmarks/baselines
git commit -m "Add --solver selection and a solver field to the baseline schema"
```

---

## Task 8: `crowd-bench compare` subcommand

**Files:**
- Modify: `crates/crowd-bench/src/report.rs`
- Modify: `crates/crowd-bench/src/main.rs`
- Modify: `README.md`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: `SolverKind` from Task 7.
- Produces: `crowd-bench compare [--out DIR]` writing `<out>/compare-<date>.json`.

- [ ] **Step 1: Add a captured timestamp to `Environment`**

In `crates/crowd-bench/src/report.rs`, add a field to `Environment`:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub rustc_version: String,
    pub build_profile: String,
    /// RFC 3339, e.g. "2026-08-06T00:00:00Z". Best-effort: this project has
    /// no date/time dependency, so it is read from the system clock via
    /// `std::time::SystemTime` and formatted by hand rather than pulling one
    /// in for a single field.
    pub captured_at: String,
}
```

Add a small formatter and use it in `Environment::capture`:

```rust
fn format_utc_now() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let days_since_epoch = duration.as_secs() / 86_400;
    let seconds_today = duration.as_secs() % 86_400;
    let (year, month, day) = civil_from_days(days_since_epoch as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_today / 3600,
        (seconds_today % 3600) / 60,
        seconds_today % 60
    )
}

/// Howard Hinnant's `civil_from_days`: days since the Unix epoch to a
/// proleptic Gregorian (year, month, day). Small, dependency-free, and exact
/// -- the usual reason to reach for a date-time crate (leap years, month
/// lengths) is handled by this one closed-form conversion.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
```

In `Environment::capture`, add `captured_at: format_utc_now(),` to the struct literal.

- [ ] **Step 2: Write a direct test for the date formatter**

Add to `report.rs`'s test module:

```rust
    #[test]
    fn civil_from_days_matches_a_known_date() {
        // 2026-08-06 is 20,306 days after the Unix epoch (1970-01-01).
        assert_eq!(civil_from_days(20_306), (2026, 8, 6));
    }

    #[test]
    fn captured_at_is_well_formed_rfc3339() {
        let environment = Environment::capture();
        assert_eq!(environment.captured_at.len(), 20, "{}", environment.captured_at);
        assert!(environment.captured_at.ends_with('Z'));
    }
```

Run: `cargo test -p crowd-bench civil_from_days -- --exact`
Expected: PASS. If it fails, the arithmetic was transcribed wrong from Hinnant's algorithm — check the constants (`719468`, `146097`, `1460`, `36524`, `146096`, `365`, `153`) against a second source rather than adjusting them by trial and error.

- [ ] **Step 3: Add the `compare` subcommand**

In `crates/crowd-bench/src/main.rs`, add to `usage()`:

```
  crowd-bench compare [--out DIR]
```

Add a `command_compare` function:

```rust
const COMPARE_SOLVERS: [(&str, crate::report::SolverKind); 3] = [
    ("sampled_velocity", crate::report::SolverKind::SampledVelocity),
    ("orca", crate::report::SolverKind::Orca),
    ("anticipatory", crate::report::SolverKind::Anticipatory),
];
const COMPARE_SCALES: [u32; 4] = [100, 500, 1000, 2000];

fn command_compare(args: &Args) -> Result<(), String> {
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let mut reports = Vec::new();
    for scene in scenes::SCENE_NAMES {
        for &(_, solver) in &COMPARE_SOLVERS {
            for &agents in &COMPARE_SCALES {
                let options = RunOptions {
                    scene: scene.to_string(),
                    agents,
                    seed: args.seed,
                    svg: false,
                    out_dir: args.out.clone(),
                    solver,
                };
                let report = run_scene(&options)?;
                println!(
                    "{},{},{agents},{:.3},{:.2},{},{},{}",
                    report.scene,
                    report.solver,
                    report.metrics.completion_rate,
                    report.metrics.mean_time_to_collision,
                    report.metrics.penetration_pair_ticks,
                    report.metrics.ticks_per_second_achieved as u64,
                    report.metrics.peak_allocated_bytes,
                );
                reports.push(report);
            }
        }
    }
    let date = reports
        .first()
        .map(|r| r.environment.captured_at[..10].to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let path = args.out.join(format!("compare-{date}.json"));
    let json = serde_json::to_string_pretty(&reports).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    println!("wrote {}", path.display());
    Ok(())
}
```

Add `"compare" => command_compare(&args).map(|()| true),` to the `match command.as_str()` block in `main`.

- [ ] **Step 4: Run it once against a small population to check it works end to end**

Run: `cargo run --release -p crowd-bench -- compare --out /tmp/compare-smoke`

This runs the *real* four-scale sweep (100/500/1,000/2,000 agents x 3 solvers x 5 scenes = 60 runs) — it will take a while at 2,000 agents. If that is too slow for a smoke check, temporarily hardcode `COMPARE_SCALES` to `[10, 20]` in a throwaway local edit, run once to confirm the command and JSON write succeed, then revert the temporary edit before committing (do not commit a reduced-scale `COMPARE_SCALES`; Task 9 needs the real one).

Expected: prints one CSV-ish line per run, ends with `wrote /tmp/compare-smoke/compare-<date>.json`, and that file parses as a JSON array of 60 (or however many, if scales were temporarily reduced) `Report` objects.

- [ ] **Step 5: Update `README.md` and `AGENTS.md`**

Add the `compare` command line and the `--solver` flag to wherever `README.md` and `AGENTS.md` currently list the `crowd-bench` commands (read those files first to match their existing format exactly rather than guessing it):

```
cargo run --release -p crowd-bench -- compare --out benchmarks/reports  # three-solver, four-scale bake-off
```

- [ ] **Step 6: Run clippy and the full test suite**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/crowd-bench/src/report.rs crates/crowd-bench/src/main.rs README.md AGENTS.md
git commit -m "Add crowd-bench compare: a three-solver, four-scale bake-off"
```

---

## Task 9: Run the comparison and check in results at all four scales

**Files:**
- Create: `benchmarks/reports/compare-<actual-date>.json` (exact name depends on the date `Environment::capture` produces when run)
- Possibly modify: `crates/crowd-core/src/avoidance/anticipatory.rs` (only if defaults need tuning — see Step 3)

This task is procedural: run the real bake-off, look at the numbers, and decide whether the anticipatory solver's defaults need adjustment before Task 10 writes them up. It has no invented pass/fail thresholds — the spec is explicit that absolute quality bars are not part of this slice's success criteria.

- [ ] **Step 1: Run the full comparison**

Run: `cargo run --release -p crowd-bench -- compare --out benchmarks/reports`

Expected: completes (budget real time for 60 full runs, several of them at 2,000 agents), ends with `wrote benchmarks/reports/compare-<date>.json`.

- [ ] **Step 2: Inspect the results for obvious pathology**

Run: `python3 -c "
import json, sys
data = json.load(open(sys.argv[1]))
for r in data:
    m = r['metrics']
    print(f\"{r['scene']:24} {r['solver']:16} n={r['requested_agents']:5} completion={m['completion_rate']:.2f} pen_ticks={m['penetration_pair_ticks']:6} ttc={m['mean_time_to_collision']:.2f} tps={m['ticks_per_second_achieved']:.0f}\")
" benchmarks/reports/compare-*.json
```

Read through all 60 lines. Look specifically for: any solver with `completion_rate` near zero across the board (a sign error, not a quality difference — e.g. an inverted `HEAD_ON_TIE_BREAK_RADIANS` making agents steer toward each other instead of apart); `ticks_per_second_achieved` for `orca` collapsing at 2,000 agents far more than the others (would indicate the LP's `O(n)` per-agent constraint count times `O(n)` neighbors is behaving as expected, or worse if something is quadratic where it shouldn't be); and any `NaN`/`null` in the JSON (should be impossible given Task 6's fuzz tests passed, but check).

- [ ] **Step 3: Decide whether `AnticipatorySolver`'s defaults need tuning**

If `anticipatory`'s `completion_rate` or `penetration_pair_ticks` are dramatically worse than both other solvers across most scenes (not just one edge case), that is a signal its defaults are miscalibrated, not that the design is unsound — the spec flagged `lookahead_neighbors` (4) and `lookahead_steps` (3) as starting points, not measured values. Try `lookahead_neighbors: 6` and/or `lookahead_steps: 5` in `AnticipatorySolver::default()` (`crates/crowd-core/src/avoidance/anticipatory.rs`), rerun just that solver's rows:

Run: `cargo run --release -p crowd-bench -- compare --out /tmp/anticipatory-retune` (temporarily comment out the `sampled_velocity` and `orca` entries in `COMPARE_SOLVERS` for this check only, then restore them before the real run below)

If the retuned defaults measurably help, keep them as the new `Default` and redo Step 1's full run so the checked-in comparison reflects the tuned solver. If they do not help, leave the original defaults and note the attempted tuning in the Task 10 report rather than silently discarding the finding — a documented negative result is still evidence.

- [ ] **Step 4: Check in the comparison JSON and the per-scale reports**

The `compare` command already wrote `benchmarks/reports/compare-<date>.json` in Step 1 (or its rerun in Step 3). Confirm it is present and non-empty:

Run: `ls -la benchmarks/reports/compare-*.json && python3 -c "import json; print(len(json.load(open('$(ls benchmarks/reports/compare-*.json)'))))"`
Expected: file exists, prints `60`.

- [ ] **Step 5: Run the full workspace test suite once more, to catch anything the retune (if any) broke**

Run: `cargo test --workspace`
Expected: all pass.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/reports/compare-*.json
git add crates/crowd-core/src/avoidance/anticipatory.rs
git commit -m "Run and check in the three-solver, four-scale comparison"
```

(The second `git add` is a no-op if Step 3 did not change the defaults — that is fine, `git commit` will simply have nothing staged from that file.)

---

## Task 10: The decision record

**Files:**
- Create: `docs/benchmarks/2026-08-06-avoidance-solver-comparison.md`

This is the artifact M0's stop condition asks for: "one navigation/avoidance/cache/bridge path is selected by a reproducible report." Write it only from the numbers Task 9 actually produced — do not estimate or extrapolate any value that was not measured.

- [ ] **Step 1: Write the report**

Create `docs/benchmarks/2026-08-06-avoidance-solver-comparison.md` following the shape of the existing `docs/benchmarks/2026-08-05-kernel-slice-1.md` (read it first to match section headings and tone). Include, at minimum:

- The environment `Environment::capture()` recorded (OS, arch, CPU, RAM, rustc version, build profile, `captured_at`), read directly from one of the checked-in `Report` entries in `benchmarks/reports/compare-*.json` — do not retype it from memory.
- A table (or one per scene) of `completion_rate`, `mean_time_to_collision`, `penetration_pair_ticks`, `heading_reversals`, `ticks_per_second_achieved`, and `peak_allocated_bytes` for all three solvers at all four scales, transcribed from that same JSON file.
- A determinism section stating plainly that Task 6's extended suite (bitwise identity, spawn-order permutation, add-one-agent, seed sensitivity, 800-agent density fuzz) passed for all three solvers, or naming exactly which one did not and why, per Task 6 Step 3's actual outcome.
- A named production-default selection with explicit reasoning, and an explicit statement of why each of the other two was not selected — the acceptance criterion requires both halves, not just the winner.
- The anticipatory solver's known multi-step-sampling tunneling limitation (documented in Task 5's header), stated as a limitation of the measurement, not buried.
- Whether Task 9 Step 3's tuning attempt (if any) changed the defaults, and what was tried.
- A one-paragraph statement of what remains open for M0 after this slice: the tiled navmesh (item 4), cache v0 (item 5), the Blender bridge (items 6-7) — so the report does not read as if M0 is closed.

- [ ] **Step 2: Cross-check the report against the spec's acceptance criterion**

Re-read `docs/superpowers/specs/2026-08-06-avoidance-solver-comparison-design.md` section 7 and confirm every sentence of it has a corresponding statement in the new report, not just a table. If something in section 7 has no corresponding sentence, add it now rather than leaving it implicit.

- [ ] **Step 3: Commit**

```bash
git add docs/benchmarks/2026-08-06-avoidance-solver-comparison.md
git commit -m "Select a production-default avoidance solver from the measured comparison"
```

---

## Plan self-review notes

- **Spec coverage:** Task 1 covers spec section 4; Tasks 2-3 cover section 2 (ORCA); Tasks 4-5 cover section 3 (anticipatory), including the honest tunneling caveat from section 3.1's implicit limitation; Task 6 covers section 8's determinism/fuzz parametrization; Tasks 7-8 cover section 5 (crowd-bench changes); Task 9 covers section 6 (four-scale fill-in); Task 10 covers section 7 (decision record). Section 1.2's exclusions (no default-solver migration beyond crowd-bench, no navmesh/cache/bridge work) are respected — no task touches `phases/decide.rs`, `route.rs`, or anything outside `avoidance/`, `crowd-bench/`, and the two doc directories.
- **Type consistency:** `SolverKind` (Task 7) matches the three `name()` strings used throughout (`"sampled_velocity"`, `"orca"`, `"anticipatory"`) and the `SOLVER_NAMES` arrays in Task 6 — all four places use the same three literal strings, checked by re-reading each task above.
- **No placeholders:** every step above contains complete code or an exact command with expected output; Task 4's deliberately-deferred fields (`lookahead_neighbors` etc.) are real `pub` struct fields with real defaults, documented as unused *until Task 5*, not `TODO`s — and Task 5 is the very next task, not an indefinite deferral.
