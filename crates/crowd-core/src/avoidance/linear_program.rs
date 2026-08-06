//! Sequential incremental 2D linear program over half-plane constraints.
//!
//! Each line further constrains the feasible region for the velocity closest
//! to `preferred`. A line the current best point violates is resolved by
//! solving on that line's own feasible interval against every prior line. If
//! the whole set is jointly infeasible, `solve` falls back to minimizing the
//! worst constraint violation instead of leaving the caller with an undefined
//! or non-finite result -- the graceful-failure path a boxed-in agent needs.

// The entire module is unused until Task 3 of the avoidance-solver-comparison plan
// wires the `solve` function into the ORCA solver. Suppressing dead_code warnings
// for all items rather than individually annotating each one.
#![allow(dead_code)]

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

    for prior_line in lines.iter().take(line_no) {
        let denominator = det(line.direction, prior_line.direction);
        let numerator = det(prior_line.direction, line.point - prior_line.point);
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
