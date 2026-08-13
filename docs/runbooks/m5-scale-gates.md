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

`m5_city_flow` now enables its declared stable-ID 10% S1 / 90% S2 profile;
the generated report records that profile. Do not compare it directly with the
pre-profile failed baseline except as an optimization reference.

The `run` command is the complete simulation measurement. Do not substitute
the shorter `--cache-frames 8` preflight for it. The cache matrix proves cache
size, range-read throughput, encoding error, and cancellation recovery only.

Before accepting 10K, also capture Blender playback/render, profiling-panel,
tier-transition, and CPU-fallback comparisons. The repository does not yet
package a single M5 Blender acceptance runner, so these artifacts must remain
explicitly pending rather than inferred from the Rust benchmark.

Create a dated `docs/benchmarks/YYYY-MM-DD-m5-10k.md` only after reviewing the
declared S/R counts; simulation/quality/memory/cache metrics; stable-ID/contact
and layer evidence across transitions; viewport/render measurements; and
backend/API/driver plus CPU-fallback results. If any gate fails, publish a
failed report and stop: do not begin 100K.

## 100K gate

Begin only with an accepted dated 10K report. Preserve 10K evidence unchanged:

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
```

The 100K report must prove streaming/procedural extraction does not create
100,000 Blender character objects. State tier mix, hardware, tick/frame rate,
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
