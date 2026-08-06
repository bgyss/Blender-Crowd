# Avoidance solver comparison (M0 item 3) — design

Date: 2026-08-06
Status: approved design, ready for implementation planning
Parent contract: [Blender Crowd 1.0 architecture and MVP](../../blender-crowd-1.0.md), section 6.2
Owning milestone: [M0 — Proving grounds](../../milestones/M0-proving-grounds.md)
Prior slice: [Deterministic crowd simulation kernel](2026-08-04-crowd-sim-kernel-design.md)

## 1. Scope

The kernel slice built one avoidance solver (`SampledVelocitySolver`) behind an
`AvoidanceSolver` trait that was deliberately shaped to admit two more
candidates "without touching any tick phase." This slice adds those two
candidates, extends the benchmark harness to run all three across the full
range of contract scales, and produces the dated comparison report that
selects a production default — closing M0 items 2 and 3.

### 1.1 In scope

- `OrcaSolver`: a from-scratch reciprocal velocity obstacle (ORCA) solver.
- `AnticipatorySolver`: a scoped, multi-step-lookahead solver.
- A shared candidate-sampling helper, factored out of `SampledVelocitySolver`
  and reused by `AnticipatorySolver`.
- `crowd-bench` changes: a `--solver` flag, a `solver` field on the baseline
  schema, and a `compare` subcommand.
- Checked-in benchmark results at 500 and 2,000 agents (100 and 1,000 already
  exist), closing out M0 item 2's four target scales.
- Per-solver unit tests mirroring `sampled.rs`'s existing coverage, and
  parametrizing the existing determinism suite and density fuzz test across
  all three solvers.
- A dated decision record under `docs/benchmarks/` selecting the production
  default solver.

### 1.2 Explicitly out of scope

- Changing which solver `Simulation` or any tick phase uses by default outside
  of this slice's own selection step. (The five benchmark scenes today run
  `SampledVelocitySolver` directly from `crowd-bench`; nothing in
  `crowd-core` hardcodes a default solver, so there is nothing to migrate
  except `crowd-bench`'s own `run_scene`.)
- The tiled navmesh (M0 item 4) — routing stays the existing waypoint
  stand-in; none of the three solvers changes how routes are produced.
- Cache v0, the Blender bridge, or the Python/Rust facade (M0 items 5-7).
- Multithreaded or SIMD solving. All three solvers are single-threaded, `f32`,
  and evaluated per agent in slot order, matching the existing tick pipeline.
- A GPU or vectorized ORCA formulation. The from-scratch ORCA here is the
  textbook sequential-LP construction, not a production-tuned one.

### 1.3 Success criteria

1. `OrcaSolver` and `AnticipatorySolver` implement `AvoidanceSolver` and pass
   the same category of tests `SampledVelocitySolver` already passes:
   unobstructed pass-through, stopped-agent, head-on deflection that is
   independent of ID ordering, ID-based crossing yield, wall deflection,
   boxed-in braking, escape-from-inside-a-wall, max-speed clamp, bitwise
   determinism for identical input, and a reported `name()`.
2. The existing determinism suite (bitwise per-tick hash, spawn-order
   permutation, add-one-agent, seed sensitivity) and the 800-agent density
   fuzz test pass for all three solvers, not only the default.
3. `crowd-bench compare` runs all three solvers across all five scenes at
   100/500/1,000/2,000 agents and emits one JSON report plus a printed
   summary table.
4. A dated report under `docs/benchmarks/` presents that comparison and states
   which solver becomes the production default, with explicit tradeoffs and
   why the other two were rejected.
5. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D
   warnings` stay clean throughout.

Absolute quality thresholds are still not part of these criteria, for the same
reason the kernel slice's spec gave: contract section 12.3 fixes thresholds
only after a checked-in baseline is measured.

## 2. `OrcaSolver`

### 2.1 Construction

Standard reciprocal velocity obstacles (van den Berg et al., 2011), evaluated
per candidate agent against every neighbor and every wall segment already
passed in via `AvoidanceInput`:

- For each neighbor, build the ORCA half-plane in velocity space from the
  relative position, combined radius, relative velocity, and a fixed time
  horizon (mirrors `SampledVelocitySolver::time_horizon`). Each side is
  credited with half the avoidance velocity, matching the reciprocal
  convention `sampled.rs` already uses for its own collision cost (relative
  velocity is built against `candidate * 2 - velocity`, i.e. this agent
  assumed to take the full correction and then halved).
- For each wall, build a **non-reciprocal** half-plane: the wall takes none of
  the responsibility, so the agent's constraint absorbs the full correction.
  This mirrors the contract's "walls never yield" statement, which
  `sampled.rs` already encodes as a heavier `wall_weight`.
- Solve for the velocity nearest the preferred velocity subject to every
  half-plane, via the sequential incremental 2D linear program: intersect the
  new constraint's line with the previous best answer's feasible region one
  constraint at a time; when a new constraint is violated, project the
  candidate onto that constraint's boundary within the region defined by all
  *prior* constraints. If constraints are jointly infeasible (a boxed-in
  agent), fall back to the standard 3D LP that minimizes total constraint
  violation instead of solving exactly — this is the ORCA literature's
  graceful-failure path and lines up with the contract's braking fallback.
- Clip the final velocity to `max_speed`.

### 2.2 Determinism

The sequential LP's result can depend on the *order* constraints are applied
when the constraint set is infeasible (the violation-minimizing fallback
picks whichever constraint is processed last as the one it satisfies exactly).
`AvoidanceInput::neighbors` order is not itself guaranteed stable across
world-state changes upstream (contract section 6.2's promise is about
per-agent *IDs*, not slice order). So `OrcaSolver::solve` sorts neighbors by
`agent_id` before building constraints, then processes walls (in their given,
already-fixed scene order) after neighbors. This makes the constructed LP,
and therefore its solution, depend only on stable IDs and scene geometry —
never on incidental neighbor-list order.

### 2.3 The symmetric head-on case

Pure ORCA has the same degeneracy `sampled.rs`'s doc comment already names:
two agents meeting exactly head-on produce mirrored half-planes, and without
a tie-break the LP can (numerically) settle either side, and settle it
differently between mirrored agents' own solves — reproducing the "both
deflect the same way and fail to separate" failure. `OrcaSolver` reuses
`sampled.rs`'s existing fix in spirit: each neighbor's ORCA half-plane normal
is rotated by a small fixed epsilon (rotate right, in the agent's own frame,
before halving) when the encounter is classified head-on by the same cosine
test `sampled.rs` uses (`head_on_cosine`). This is a fixed convention
evaluated in each agent's own frame, so it produces opposite world-space
deflections between the two agents, exactly like the existing solver's
keep-left rule — not an ID comparison, which would fail for the reason
`sampled.rs`'s doc comment already explains in depth.

### 2.4 Status classification

`AvoidanceOutput::status` is derived the same way as `sampled.rs`: `Braking`
when the solved speed falls below `brake_speed_fraction` of the preferred
speed (this is what the 3D LP fallback produces when constraints are
jointly infeasible), `Avoiding` when the solved velocity differs from
preferred by more than a small epsilon, `Free` otherwise.

## 3. `AnticipatorySolver`

### 3.1 Construction

Reuses the shared candidate-sampling helper (section 4) to enumerate the same
kind of velocity-space candidates `sampled.rs` does, but scores each candidate
differently:

- **Threat scoping.** Neighbors are ranked once per solve (not per candidate)
  by a cheap distance-based proxy — `distance_squared` to the agent, breaking
  ties by `agent_id` for determinism — and the nearest `lookahead_neighbors`
  (default 4) become "scoped threats." The rest contribute only a lightweight
  distance-based repulsion term (no lookahead), bounding the solver's total
  cost to `O(candidates * (lookahead_neighbors * lookahead_steps + remaining
  neighbors))` rather than `O(candidates * all_neighbors * lookahead_steps)`.
- **Multi-step lookahead.** For each candidate and each scoped threat, the
  solver extrapolates both the candidate agent (at the candidate velocity)
  and the threat (at its current, most-recently-perceived velocity — the
  same reciprocal-construction assumption `sampled.rs` makes) forward over
  `lookahead_steps` (default 3) fixed sub-steps spanning `time_horizon`, and
  takes the *minimum* disc-disc separation observed across those sub-steps
  as that pair's collision proxy for the candidate. This is the feature that
  makes it "anticipatory": a candidate that looks clear under one
  instantaneous time-to-collision check but closes sharply two sub-steps out
  (e.g. because the neighbor is curving through its own avoidance) scores
  worse here than it would under `sampled.rs`'s single analytic check.
- **Cost terms.** Goal-seeking, smoothness, and side-bias terms are reused
  unchanged from the shared helper / `sampled.rs`'s existing formulas (moving
  them is part of the section 4 refactor). Only the collision term's
  *computation* differs; its output shape (a cost float, an earliest-time
  float) stays compatible so the rest of the candidate evaluation loop is
  identical.
- **Walls** are scoped threats unconditionally (never demoted to the
  cheap tier) — there are always few enough per scene that this does not
  reintroduce the cost blowup the neighbor scoping avoids, and walls never
  yield, so under-scoping one would be a correctness regression, not a
  performance tradeoff.

### 3.2 Determinism

Neighbor ranking breaks distance ties by `agent_id`, so which neighbors land
in the scoped-threat tier is stable regardless of upstream list order. Given a
stable scoped set, the multi-step extrapolation is a pure function of each
agent's own recorded position/velocity, so it needs no further tie-break
beyond the same head-on and crossing-conflict handling reused from `sampled.rs`
through the shared helper.

## 4. Shared candidate-sampling helper

`SampledVelocitySolver::solve` and `AnticipatorySolver::solve` both need to
enumerate the *same* candidate velocities in the *same* fixed order (preferred
velocity first, then speed rings × headings, then stop) — sharing this
guarantees both solvers see identical candidate sequences for identical
inputs, rather than two independently-written loops that could silently
diverge in enumeration order and make a future determinism regression harder
to attribute. Extracted into `avoidance/mod.rs` as:

```rust
pub(crate) fn sample_candidates(
    heading: Vec2,
    speed_reference: f32,
    speed_samples: u32,
    heading_samples: u32,
    mut visit: impl FnMut(Vec2),
)
```

`sampled.rs` is refactored to call this instead of its inline loop; behavior
is unchanged (verified by its existing test suite passing unmodified). This is
the only change to `sampled.rs` in this slice — its cost terms, weights, and
public API are untouched.

## 5. `crowd-bench` changes

### 5.1 Solver selection

`RunOptions` gains a `solver: SolverKind` field:

```rust
pub enum SolverKind {
    SampledVelocity,
    Orca,
    Anticipatory,
}
```

`run_scene` matches on it to construct the right boxed solver instead of
hardcoding `SampledVelocitySolver::default()`. The CLI gains `--solver NAME`
(accepted values `sampled_velocity | orca | anticipatory`, default
`sampled_velocity`, preserving every existing invocation's behavior
unchanged).

### 5.2 Baseline schema

`Baseline` gains a `solver: String` field, populated from `Report::solver`.
`command_check` compares it against the stored baseline's solver and fails
loudly (a distinct error, not a silent metric drift) if they differ, so a
baseline can never be checked against the wrong solver's numbers by a
`--solver` typo. The six committed baseline files are updated in place with
`"solver": "sampled_velocity"`, since that is what produced them.

### 5.3 `compare` subcommand

```
crowd-bench compare [--out DIR]
```

Runs all three solvers × five scenes × four scales (100/500/1,000/2,000),
each with the existing default seed, and writes one JSON array of `Report`s
to `<out>/compare-<date>.json` (date from the environment's captured
timestamp — actually captured freshly, since `Environment` does not currently
carry one; this slice adds a `captured_at: String` RFC 3339 field to
`Environment` so the comparison file and report can both be dated without
relying on the filesystem). Also prints a compact table (`scene, agents,
solver, completion_rate, mean_time_to_collision, penetration_pair_ticks,
ticks_per_second_achieved, peak_allocated_bytes`) to stdout for quick
inspection without opening the JSON.

This does not touch `command_run`, `command_sweep`, `command_baseline`, or
`command_check` beyond the solver-field addition in 5.2.

## 6. Filling in 500 and 2,000 agents

`scenes::build` already scales geometry by `sqrt(population)` (kernel slice
commit `57ef4d5`) and `command_sweep` already iterates
`[100, 500, 1000, 2000]`, so no scene code changes are needed. This slice does
**not** run `crowd-bench baseline --agents 500` / `--agents 2000` — baselines
stay one-per-scene at the existing regression-check scale (1,000, matching
today), since baselines exist to catch regressions cheaply, not to hold every
scale. The four-scale coverage requirement is satisfied by `compare`'s output
and the decision report referencing all four, not by expanding the
regression-check baseline set. Reports produced by `compare` at 500 and 2,000
(for all three solvers) are what get checked in under `benchmarks/reports/`,
alongside the existing 100/1,000 reports.

## 7. Decision record

`docs/benchmarks/2026-08-06-avoidance-solver-comparison.md` (written after
`compare` produces real numbers, not before) presents, per the M0 acceptance
criterion: a table of the three solvers' quality (`completion_rate`,
`mean_time_to_collision`, `penetration_pair_ticks`, `heading_reversals`),
determinism (bitwise pass/fail per the extended suite), time
(`ticks_per_second_achieved`), and memory (`peak_allocated_bytes`) across all
twenty scene/scale combinations; states which solver becomes crowd-core's
production default; and states explicitly why each rejected candidate was
not selected. This is the artifact M0's stop condition asks for: "one
navigation/avoidance/cache/bridge path is selected by a reproducible report."

Selecting a default here does not delete the other two solvers — all three
stay in the codebase behind the same trait, since the M0 gate is about
proving the architecture accommodates a bake-off, not about only ever
shipping one implementation.

## 8. Testing

- `orca.rs` and `anticipatory.rs` each get a test module mirroring
  `sampled.rs`'s: `an_unobstructed_agent_keeps_its_preferred_velocity`,
  `a_stopped_agent_with_no_goal_stays_stopped`, `a_head_on_neighbor_deflects_the_agent`,
  `head_on_agents_choose_opposite_sides`,
  `head_on_side_choice_does_not_depend_on_id_ordering`,
  `the_higher_id_yields_more_in_a_crossing_conflict`,
  `a_wall_ahead_deflects_the_agent`,
  `a_boxed_in_agent_brakes_rather_than_escaping`,
  `an_agent_inside_a_wall_is_given_a_way_out`,
  `the_solution_never_exceeds_max_speed`, `the_output_is_always_finite`,
  `solving_is_deterministic_for_identical_input`, `the_solver_reports_its_name`.
  Some may need solver-specific fixtures (e.g. `AnticipatorySolver`'s boxed-in
  case needs enough sub-steps to see the trap; `OrcaSolver`'s crossing-yield
  case needs the yield expressed as an ORCA constraint weight rather than a
  sampled cost term) — the *behavior* asserted is the same, the setup may
  differ.
- `crates/crowd-core/tests/determinism.rs` and `fuzz_density.rs` are
  parametrized over `[SolverKind::SampledVelocity, SolverKind::Orca,
  SolverKind::Anticipatory]` (a local helper enum in the test files, or reuse
  crowd-bench's if it moves to crowd-core — this slice keeps `SolverKind` in
  crowd-bench and duplicates the three-arm match in the two test files, since
  promoting it to crowd-core for two call sites is not yet justified).
- `avoidance/mod.rs`'s new `sample_candidates` helper gets its own direct
  test: fixed enumeration order and count for given sample parameters,
  independent of any solver.

## 9. Risks and open questions carried into the plan

- The ORCA 3D-LP infeasibility fallback is the part of this design with the
  least precedent elsewhere in this codebase (everything else here is
  adapting patterns `sampled.rs` already established). The implementation
  plan should budget explicit test-driven iteration on the boxed-in case,
  the way the kernel slice's task log shows real plan defects being found
  and fixed by tests rather than assumed away.
- `AnticipatorySolver`'s `lookahead_neighbors` and `lookahead_steps` defaults
  (4 and 3) are starting points, not measured. The comparison report may
  show they need tuning before the three-way bake-off is fair; if so, that
  tuning happens before the report is written, not after, so the report
  reflects each solver's best reasonable configuration.
