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

Phase 0, slice 1 is implemented: a headless, deterministic Rust simulation
kernel with structure-of-arrays agents, a fixed tick, spatial queries, one
sampled-velocity avoidance solver, five benchmark scenes, and a measured
metrics report. Nothing here touches Blender yet.

Measured results, including where the current solver falls short:

- [Kernel slice 1 benchmark report](docs/benchmarks/2026-08-05-kernel-slice-1.md)

## Development

Requires the pinned Rust toolchain in `rust-toolchain.toml`; `mise install`
sets it up. On macOS, `.cargo/config.toml` points the linker at the system
clang — without it, nothing links.

```sh
cargo test --workspace                                    # unit, property, determinism
cargo test --release -p crowd-core --test fuzz_density    # 800-agent density stress
cargo clippy --workspace --all-targets -- -D warnings

cargo run --release -p crowd-bench -- run --agents 1000 --svg --solver orca
cargo run --release -p crowd-bench -- check --agents 1000 # regression against baselines
cargo run --release -p crowd-bench -- compare --out benchmarks/reports  # three-solver, four-scale bake-off
```

`cargo test --workspace` runs the density fuzz in debug, which is slow; use the
release invocation above when iterating.

Baselines in `benchmarks/baselines/` record measured output, not targets. Per
the contract, quality thresholds are set only after a baseline is reviewed.
