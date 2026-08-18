# M5 10K and 100K scale-gate runbook

Use this runbook to generate the evidence required by
[M5 — Scale, GPU tiers, and procedural rendering](../milestones/M5-scale-rendering.md).
The commands generate evidence; the report review decides whether a gate passes.

## Where to run it

Run the full 10K and 100K commands in a normal, long-lived terminal on the
named reference workstation. On macOS, that means normal Metal access, not a
restricted automation sandbox. `tmux` is recommended so a multi-minute run
continues if the terminal UI disconnects.

Codex can run source checks, short smoke runs, and the bounded cache matrix in
this worktree. It cannot reliably retain the full 10K simulation beyond this
tool session's execution window, so it cannot certify a gate that finishes
after the session is interrupted. A user terminal can run these same checked-in
commands without that limitation.

## Prerequisites

1. Use a clean commit or record the dirty diff in the evidence directory.
2. Install the pinned toolchain with `mise install`.
3. For Blender evidence on macOS, use Blender 5.2 LTS with normal host Metal
   access as documented in `CLAUDE.md`.
4. Record workstation model, RAM, OS, Blender version, GPU/driver/API, and
   commit before making a support claim.

```sh
git status --short
git rev-parse HEAD
rustc --version
system_profiler SPHardwareDataType SPDisplaysDataType
```

## 10K gate

Start a durable session and save raw artifacts outside the repository worktree:

```sh
tmux new -s blender-crowd-m5
mkdir -p "$HOME/blender-crowd-m5/10k"
cd /path/to/Blender-Crowd
scripts/m5-foundation-test.sh
cargo run --release -p crowd-bench -- run \
  --scene m5_city_flow --agents 10000 \
  --out "$HOME/blender-crowd-m5/10k/simulation"
cargo run --release -p crowd-bench -- cache-experiment \
  --agents 10000 --cache-frames 120 \
  --out "$HOME/blender-crowd-m5/10k/cache"
```

`m5_city_flow` now enables a declared stable-agent-ID profile targeting 10%
S1/R1 and 90% S2/R2. Schema-v4 reports count the committed S/R assignments;
because a stable hash partitions a finite population, the exact count may vary
slightly around that target. Do not accept a schema-v3 city-flow report: it
restated target counts while classifying lane-local ordinals incorrectly.
Do not compare the schema-v4 result directly with the pre-profile failed
baseline except as an optimization reference.

**The 10K gate now passes.** See
[2026-08-14-m5-10k.md](../benchmarks/2026-08-14-m5-10k.md) for the accepted
report. Rerun the sequence here to reproduce it, or to re-adjudicate after a
change; do not treat an older 10K report as evidence for current code.

Adjudicate the report against the checked-in per-tier thresholds. The gate
exits non-zero on failure, so it can be chained:

```sh
cargo run --release -p crowd-bench -- m5-gate \
  --report "$HOME/blender-crowd-m5/10k/simulation/m5_city_flow-10000.json" \
  --out "$HOME/blender-crowd-m5/10k/adjudication.json"
```

The thresholds live in `benchmarks/thresholds/m5-city-flow.json` and are
compiled into the binary, so a run cannot be judged against a loosened copy on
disk. They are stated as rates per observed agent-tick — or, for stall
episodes, per agent-kilometre walked — which is what lets one file gate 1K,
10K, and 100K. Changing a threshold is a reviewable change to that file with
its `basis` field updated, not a per-run adjustment.

### Two figures are reported but never gated

`stalled_agent_share` and `max_penetration_depth_m` print as `note` lines and
cannot fail a run. Both were gated until the 100K attempt of 2026-08-15 showed
that neither is scale-invariant, so a fixed limit on them was silently stricter
at each larger population:

- `stalled_agent_share` is a lifetime cumulative probability. At a perfectly
  constant blocking rate per metre it still tends toward 1.0 as routes
  lengthen, and this fixture's routes grow with the square root of population.
- `max_penetration_depth_m` is an extremum over samples. 100K draws 32x the
  agent-ticks of 10K, so its expected value rises with population even when
  solver behavior is unchanged.

They stay printed because a jump in either is still worth looking at. The
pass/fail decision rests on their rate-shaped replacements,
`stall_episodes_per_agent_km` and `mean_penetration_depth_fraction`. Re-gating
a reported figure is a deliberate edit that
`the_two_scale_dependent_figures_are_reported_but_never_fail_a_run` will catch.

`deep_penetration_agent_ticks_per_agent_tick` is reported for a different
reason: it measured exactly zero at every scale up to 40K, so there was nothing
to set a bar from. The 100K run of 2026-08-18 produced the first non-zero value
(S2 4.082e-8). Gate it once a second run confirms that figure — one measurement
is not a calibration, which is the mistake the S1 severity bar was built on.

Contact limits were recalibrated on 2026-08-17 from 1K/10K/20K/40K, after the
background-tier exposure defect was fixed. Any contact figure measured before
that fix is not a valid calibration input: a tier on a sparse perception cadence
read ~2x better than it was. The rule is in the file's `basis` — worst
calibration value, times measured per-scale-step growth, times 2 — and it puts
every gated quality margin at 100K between 1.7x and 3.0x.

**This does not mean a failed 100K may be waved through.** The same run also
degraded on genuinely rate-shaped measures: `stall_agent_ticks_per_agent_tick`
grew 5.15x from 10K and passed only on headroom. A specification defect and a
real regression can be present at once, and neither excuses the other.

A report older than schema v6 is rejected by the gate rather than adjudicated.
v5 and earlier carry neither `distance_travelled_m` nor the deep-contact
counters, so the gated metrics cannot be computed from them at all. Rerun it.

Then collect the Blender playback, render, and scale/profiling UI evidence:

```sh
M5_BLENDER_AGENTS=10000 \
M5_ARTIFACT_DIR="$HOME/blender-crowd-m5/10k/blender" \
M5_REPORT="$HOME/blender-crowd-m5/10k/simulation/m5_city_flow-10000.json" \
M5_ADJUDICATION="$HOME/blender-crowd-m5/10k/adjudication.json" \
  scripts/m5-blender-test.sh
```

This needs Blender 5.2 LTS with normal host Metal access. It bakes at the
declared `m5_background_10_90` profile and asserts the population stays
procedural — one attached scene object carrying every agent as point data —
rather than expanding into per-agent objects.

Tier-transition and CPU-fallback evidence is produced by
`scripts/m5-foundation-test.sh`.

The current fixture scales the count of one-way route lanes with linear scene
scale (six per direction at 100 agents and sixty per direction at 10K). This
keeps per-lane linear density comparable as population grows. Do not compare a
report from the older fixed twelve-lane fixture with this revised fixture as
acceptance evidence; retain it only as a diagnostic baseline.

S2 uses a stable-ID-staggered two-tick perception and steering interval, and
presentation classification follows the same cadence. Verify
`s2_perception_interval_ticks` and `s2_steering_interval_ticks` in the report
before comparing it with an older four-tick result. The gate rejects a run at a
different cadence rather than scoring it, because cadence is part of the
quality/cost tradeoff the thresholds were set against.

The `run` command is the complete simulation measurement. Do not substitute
the shorter `--cache-frames 8` preflight for it. The cache matrix proves cache
size, range-read throughput, encoding error, and cancellation recovery only.

Create a dated `docs/benchmarks/YYYY-MM-DD-m5-10k.md` only after reviewing the
declared S/R counts; simulation/quality/memory/cache metrics; stable-ID/contact
and layer evidence across transitions; viewport/render measurements; and
backend/API/driver plus CPU-fallback results. If any gate fails, publish a
failed report and stop: do not begin 100K.

## 100K gate

**The 100K gate passed on 2026-08-18.** See
[2026-08-18-m5-100k.md](../benchmarks/2026-08-18-m5-100k.md) for the accepted
report, and read its "Exactly one threshold change is load-bearing" and
"Corrections to earlier findings" sections before citing the result: the pass
turns on a single recalibrated limit, and two findings from the investigation
that produced it are revised there.

Rerun the sequence below to reproduce it, or to re-adjudicate after a change.
Do not treat an older 100K report as evidence for current code — in particular,
the S2 contact figures in the 2026-08-14 and 2026-08-15 reports are understated
2x by the exposure defect fixed on 2026-08-17, and are not comparable with
anything measured after it.

The 10K gate passed on 2026-08-14. Read that report's "What this report does not
establish" section first — it bounds what the 10K result licenses.

`scripts/m5-100k-gate.sh` runs every stage below in the right order, in one
command, with a per-stage log. Prefer it over running the stages by hand:

```sh
tmux new -s blender-crowd-m5-100k
cd /path/to/Blender-Crowd
scripts/m5-100k-gate.sh
```

Budget most of a day. The fixture's duration scales with the square root of
population, so 100,000 agents run 142,302 ticks against the 10K gate's 45,000,
at roughly a tenth of its per-tick throughput. Use `tmux`: a disconnected
terminal must not take the run with it.

The script adjudicates the simulation before gathering Blender evidence and
stops on a failed gate, because the milestone says to publish a failed report
rather than accumulate supporting evidence for a failed run. `M5_RESUME=1`
reuses a simulation report that is already present, so a later stage can be
rerun without repeating the multi-hour simulation.

To run the stages individually instead, preserve the 10K evidence unchanged:

```sh
tmux new -s blender-crowd-m5-100k
mkdir -p "$HOME/blender-crowd-m5/100k"
cd /path/to/Blender-Crowd
scripts/m5-foundation-test.sh
cargo run --release -p crowd-bench -- run \
  --scene m5_city_flow --agents 100000 \
  --out "$HOME/blender-crowd-m5/100k/simulation"
cargo run --release -p crowd-bench -- cache-experiment \
  --agents 100000 --cache-frames 120 \
  --out "$HOME/blender-crowd-m5/100k/cache"
cargo run --release -p crowd-bench -- m5-gate \
  --report "$HOME/blender-crowd-m5/100k/simulation/m5_city_flow-100000.json" \
  --out "$HOME/blender-crowd-m5/100k/adjudication.json"
M5_BLENDER_AGENTS=100000 \
M5_ARTIFACT_DIR="$HOME/blender-crowd-m5/100k/blender" \
M5_REPORT="$HOME/blender-crowd-m5/100k/simulation/m5_city_flow-100000.json" \
M5_ADJUDICATION="$HOME/blender-crowd-m5/100k/adjudication.json" \
  scripts/m5-blender-test.sh
```

The 100K run is adjudicated against the *same* threshold file. That is the
point of expressing the limits as per-agent-tick rates: if 100K needs looser
numbers than 10K, that is a finding to report, not a threshold to relax.

The 100K report must prove streaming/procedural extraction does not create
100,000 Blender character objects. The Blender runner asserts exactly that —
persistent scene objects added by attaching the cache, against the whole
population carried as point data — so run it and quote its numbers rather than
arguing the property from the architecture. State tier mix, hardware, tick/frame rate,
cache size, render path, and quality limits in any headline. Do not claim fully
autonomous skinned heroes or cross-vendor GPU parity.

## After a run

Keep raw JSON/cache reports with screenshots/video and workstation capture.
Validate code before publishing a report:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

M5 completes only with separate passing 10K and 100K reports.

## 1K optimization confirmation

Run this shorter confirmation after an optimization change and before repeating
the full 10K simulation. It is evidence for the next optimization decision, not
an M5 acceptance gate and does not authorize the 100K run.

```sh
mkdir -p "$HOME/blender-crowd-m5/1k-confirmation"
cargo run --release -p crowd-bench -- run \
  --scene m5_city_flow --agents 1000 \
  --out "$HOME/blender-crowd-m5/1k-confirmation"
```

The report's scale measurements are nested under `.metrics`; only
`fidelity_profile`, `environment`, `ticks_per_second`, and `duration_ticks` are
top-level fields. Use this query rather than a `.metrics` wrapper around every
field:

```sh
jq '{
  fidelity_profile,
  environment,
  ticks_per_second,
  duration_ticks,
  metrics: {
    agents_spawned: .metrics.agents_spawned,
    agents_arrived: .metrics.agents_arrived,
    completion_rate: .metrics.completion_rate,
    penetration_pair_ticks: .metrics.penetration_pair_ticks,
    max_penetration_depth: .metrics.max_penetration_depth,
    agents_ever_stalled: .metrics.agents_ever_stalled,
    ticks_per_second_achieved: .metrics.ticks_per_second_achieved,
    wall_time_seconds: .metrics.wall_time_seconds,
    phase_time_shares: .metrics.phase_time_shares
  }
}' "$HOME/blender-crowd-m5/1k-confirmation/m5_city_flow-1000.json"
```
