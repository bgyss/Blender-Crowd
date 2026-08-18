# Repository Guidelines

## Project Structure & Module Organization

This repository contains the accepted M0 proving grounds and M1 1,000-agent vertical slice. Start with `README.md`, then treat `docs/blender-crowd-1.0.md` as the canonical product and engineering specification. Keep additional design decisions in `docs/` and link durable, project-wide guidance from the README.

The implementation layout is documented in section 14 of the contract: Blender Python belongs in `addon/`, Rust crates in `crates/`, versioned formats in `schemas/`, cross-layer tests in `tests/`, and redistributable fixtures in `assets/reference/`. Do not create all planned packages preemptively; add a module when an implemented feature or ownership boundary requires it.

## Build, Test, and Development Commands

The Rust workspace is implemented and tested. Use these:

```sh
cargo test --workspace                                    # unit, property, determinism
cargo test -p crowd-core --test behavior_graph            # M2 typed graph schema/compiler
scripts/m2-foundation-test.sh                             # implemented M2 compiler/data-layer checks
scripts/m4-foundation-test.sh                             # M4 layer composition, migration, bridge, and profile checks
scripts/m5-foundation-test.sh                             # M5 tier scheduler, per-tier gate, transitions, CPU fallback
scripts/m6-foundation-test.sh                             # M6 typed perception/brain/activity/motion/physics contracts and R0 interaction foundation
scripts/m6-blender-test.sh                                # M6 Blender-process debugger/graph-search smoke; requires Blender 5.2 LTS
scripts/m6-acceptance.sh                                  # M6 deterministic requirement audit; M9 neural/operator gates are separate
cargo run --release -p crowd-bench -- m5-gate --report REPORT.json --out ADJUDICATION.json # fixed per-tier M5 thresholds
scripts/m5-blender-test.sh                                # M5 procedural playback, render, and scale/profiling UI proof
M5_BLENDER_AGENTS=10000 scripts/m5-blender-test.sh        # the same proof at the 10K gate's population
scripts/m5-100k-gate.sh                                   # every M5 100K stage in one command; multi-hour, run under tmux
scripts/m4-blender-test.sh                                # M4 clean-install layer editor, conflict, flatten, USD, and reload proof
M4_ARTIFACT_DIR=/tmp/blender-crowd-m4-captures scripts/m4-blender-test.sh # retain M4 before/after and scale PNGs
cargo test --release -p crowd-core --test fuzz_density    # 800-agent density stress
cargo clippy --workspace --all-targets -- -D warnings     # must be clean before commit
cargo fmt                                                 # before every commit

cargo run --release -p crowd-bench -- run --agents 1000 --svg --solver sampled_velocity
cargo run --release -p crowd-bench -- check --agents 1000 # regression against baselines
cargo run --release -p crowd-bench -- compare --out benchmarks/reports  # three-solver, four-scale bake-off

cargo run --release -p crowd-bench -- nav-reroute --agents 1000 --svg  # tiled-navmesh portal reroute (M0 item 4)
cargo test --release -p crowd-core --test two_room_reroute -- --ignored  # 1,000-agent reroute acceptance test
scripts/cache-experiment.sh              # measured 1,000-agent cache matrix
scripts/m0-acceptance.sh                 # complete ordered M0 gate + JSON evidence
cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture
scripts/m1-bake-test.sh                  # two strict bakes + cancel/recovery proof
scripts/m1-blender-test.sh               # clean project/cache/override/render suite
scripts/m1-render-test.sh --out /tmp/blender-crowd-m1-render

cargo run --release -p crowd-bench -- run --scene crossing --agents 600 --frames
scripts/make-gif.sh crossing 600           # frames -> docs/media/crossing-600.gif (needs ffmpeg)

scripts/build-wheel.sh                     # abi3 wheel -> addon/blender_crowd/wheels/ (needs maturin)
scripts/verify-wheel.sh                    # trace + wheel round trip in a plain CPython
scripts/blender-install-test.sh            # clean install + native module load
scripts/blender-playback-test.sh           # 1,000-point playback, costs reported separately
scripts/make-blender-recording.sh crossing 1000  # playback clip -> docs/media/ (needs ffmpeg)
scripts/make-m1-recording.sh                     # M1 cache-only concourse clip -> docs/media/ (needs ffmpeg)

cargo run --release -p crowd-bench -- run --scene crossing --agents 1000 --trace
```

`maturin` is pinned in `mise.toml` via the `pipx:` backend and installed by
`mise install`; the `cargo:` backend cannot build it here because outside this
repo the nix `cc` on `PATH` is the linker. Built wheels are gitignored.

The Blender runners require Blender 5.2 LTS at
/Applications/Blender.app/Contents/MacOS/Blender (override with BLENDER=...).
They must run with normal host Metal access on macOS; a restricted automation
sandbox can return no Metal device and crash Blender before Python starts. The
M4 source-add-on runner must retain `--python-use-system-env`, because Blender
5.2 otherwise ignores its `PYTHONPATH`.

The addon package uses relative imports throughout: extensions are imported
as `bl_ext.user_default.blender_crowd`, so absolute imports of the package
name fail. Bundled wheels unpack into a site-packages directory shared by all
installed extensions, so the native module name `blender_crowd_native` must
stay distinctive.

`--svg` and `--frames` sample every tick, so a recorded run's `ticks_per_second`
is not a performance measurement and must not be quoted as one. `--trace` also
writes every tick to disk, so a wall-clock time wrapped around a `--trace` run
(simulate + serialize) is likewise not an isolated simulation-throughput
measurement — it includes the trace write and the invoking process's own
overhead — and must not be quoted as one either.

The toolchain is pinned in `rust-toolchain.toml` (`mise install` sets it up).
On macOS, `.cargo/config.toml` points the linker at the system clang; without
it nothing links, because the nix `cc` on `PATH` cannot resolve `libSystem`.

Run the density fuzz in release — it is impractically slow in debug.

For documentation-only changes, these lightweight checks still apply:

```sh
git diff --check                       # detect whitespace errors
rg '^## ' docs/blender-crowd-1.0.md    # review the contract outline
git status --short                     # confirm the intended change set
```

Blender and Python tooling is implemented. Keep exact, copy-ready commands here
and in `README.md` as runners change. Never claim a test passed if its runner is
not checked into the repository.

## Coding Style & Naming Conventions

Use four spaces for Python and standard `rustfmt` formatting for Rust. Prefer `snake_case` for Python modules, functions, Rust modules, and crate directories (crate package names may use kebab-case, such as `crowd-core`). Use `PascalCase` for types and Blender-facing classes. Keep Python orchestration coarse-grained; per-agent hot loops and authoritative simulation state belong in Rust. Preserve deterministic behavior, stable identifiers, versioned schemas, and the ownership boundaries defined by the contract.

## Testing Guidelines

Add tests with every implemented behavior. Rust unit and property tests should live beside their modules; cross-layer, packaging, and Blender headless tests belong in `tests/`. Name tests after observable behavior, for example `stable_ids_do_not_depend_on_iteration_order`. Include deterministic scenario snapshots, cache round trips, schema migration checks, and failure cases. Performance claims require a reproducible benchmark, fixture, and recorded environment.

## Commit & Pull Request Guidelines

The history currently uses concise, imperative subjects (for example, `Add Blender Crowd 1.0 architecture and MVP`). Keep commits focused and explain contract changes in the body. Pull requests should state scope, link the relevant contract section or issue, list verification performed, and call out schema/cache compatibility effects. Include screenshots or renders for Blender UI, Geometry Nodes, or visual-output changes.
