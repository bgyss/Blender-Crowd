# Blender Crowd

Blender Crowd is a proposed Blender-native platform for authoring, simulating,
editing, caching, and rendering autonomous character crowds.

The project is designed around a high-performance deterministic simulation core,
with Blender serving as the authoring, debugging, layout, and rendering
environment. Geometry Nodes is a presentation and procedural-authoring layer,
not the authoritative simulator.

The canonical product and engineering contract is:

- [Blender Crowd 1.0 architecture and MVP](docs/blender-crowd-1.0.md)

The industrial capability target and its traceability to the delivery sequence
are documented in:

- [Industrial crowd capability and Blender integration roadmap](docs/industrial-crowd-capability-roadmap.md)
- [Milestone contract index](docs/milestones/README.md)

Research informing the avoidance, social-attention, scale, and animation-tier
decisions is summarized in:

- [Crowd simulation research synthesis](docs/crowd-simulation-research-2026.md)

The first release is intentionally focused: build a trustworthy pedestrian-crowd
pipeline for 1,000 interactive agents before expanding into semantic activities,
combat, traffic, motion matching, or 100,000-agent backgrounds. Those later
capabilities are deferred, not discarded: the milestone suite carries the
project from the 1.0 proof toward a Golaem-class Blender production workflow,
MASSIVE-style authorable agency, and an eventual Blender ecosystem/mainline
integration proposal backed by production evidence.

## Status

Phase 0 is implemented: a headless, deterministic Rust simulation kernel with
structure-of-arrays agents, a fixed tick, spatial queries, three avoidance
solvers, six benchmark scenes, and measured metrics reports. A reproducible
72-report bake-off selected `sampled_velocity` as the production default.
Nothing here touches Blender yet.

![600 agents crossing, sampled_velocity solver](docs/media/crossing-600.gif)

The `crossing` scene, 600 agents, `sampled_velocity` solver, seed 2026, coloured
by destination. It shows both the working part and the open problem: the streams
resolve into lanes, and they also jam hard where they intersect, which is the
quality gap the benchmark report quantifies. Regenerate it with
`scripts/make-gif.sh` (needs `ffmpeg`; the frames themselves come from
`crowd-bench run --frames` and need nothing extra).

Measured results, including where the current solver falls short:

- [Kernel slice 1 benchmark report](docs/benchmarks/2026-08-05-kernel-slice-1.md)
- [Avoidance solver comparison and production default](docs/benchmarks/2026-08-06-avoidance-solver-comparison.md)

## Development

Requires the pinned Rust toolchain in `rust-toolchain.toml`; `mise install`
sets it up. On macOS, `.cargo/config.toml` points the linker at the system
clang — without it, nothing links.

```sh
cargo test --workspace                                    # unit, property, determinism
cargo test --release -p crowd-core --test fuzz_density    # 800-agent density stress
cargo clippy --workspace --all-targets -- -D warnings

cargo run --release -p crowd-bench -- run --agents 1000 --svg --solver sampled_velocity
cargo run --release -p crowd-bench -- check --agents 1000 # regression against baselines
cargo run --release -p crowd-bench -- compare --out benchmarks/reports  # three-solver, four-scale bake-off

cargo run --release -p crowd-bench -- run --scene crossing --agents 600 --frames
scripts/make-gif.sh crossing 600                          # frames -> docs/media/crossing-600.gif

scripts/build-wheel.sh                                    # abi3 wheel -> addon/blender_crowd/wheels/
scripts/verify-wheel.sh                                   # trace + wheel round trip in a plain CPython
```

The wheel build needs `maturin`, pinned in `mise.toml` and installed by
`mise install`. It is pinned through the `pipx:` backend (uv) rather than
`cargo:`, because building maturin from source happens outside this repo,
where the nix `cc` on `PATH` is the linker and cannot resolve libSystem.
Built wheels are not committed. The wheel is `abi3` on purpose: Blender 5.2
treats an `abi3` tag as "any CPython 3", so a single wheel survives Blender
moving to a newer CPython.

`--svg` and `--frames` sample the simulation every tick, so a run recorded with
either reports a `ticks_per_second` that is not a performance measurement. Quote
timings only from unrecorded runs.

`cargo test --workspace` runs the density fuzz in debug, which is slow; use the
release invocation above when iterating.

Baselines in `benchmarks/baselines/` record measured output, not targets. Per
the contract, quality thresholds are set only after a baseline is reviewed.
