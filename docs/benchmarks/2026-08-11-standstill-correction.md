# Standstill-deadlock correction

Date: 2026-08-11

This note corrects an invalid conclusion in the historical
[2026-08-05 kernel-slice report](2026-08-05-kernel-slice-1.md). That report's
dated measurements and narrative are preserved as the record of that run; it
is not edited retroactively.

## Correction

The claim that every scene drained and that nothing deadlocked is false. In
the 1,000-agent `crossing` run used to diagnose the sampled-velocity solver,
both inflow arms had drained by tick 4,000 while roughly 760 agents remained
stationary at the intersection. The prior guard only required 10% of
unfinished agents to move, so a small moving fringe could hide that frozen
core.

The standing-queue change reduces the collision urgency of a non-closing
neighbor. Its separate `queue_urgency = 1.0` control reproduces the former
all-stationary-neighbor behavior, so it remains the compatibility reference.

## Follow-up guard and recovery scope

`the_crowd_does_not_deadlock_wholesale` now requires 20% of unfinished agents
to be moving. This is still a liveness check, not a quality bar; the checked-in
per-scene baselines carry the measured quality metrics.

The solver also has a narrowly scoped restart escape for an agent already
stopped behind a dense stationary core: it adds cost only to selecting the
zero-velocity fallback when eight or more stationary neighbors block the
preferred route. A unit regression proves that a deterministic lateral detour
is chosen, and that `queue_urgency = 1.0` preserves the old absorbing fallback.
This is not a claim that all jams are solved; physical overlaps, closed exits,
and arbitrary queue behavior remain outside M1.

## Verification required for a baseline change

Regenerate and check the six deterministic 1,000-agent baselines before
merging a solver revision:

```sh
cargo run --release -p crowd-bench -- baseline --agents 1000 --seed 2026
cargo run --release -p crowd-bench -- check --agents 1000 --seed 2026
cargo test --release -p crowd-core --test fuzz_density
```
