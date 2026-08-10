# M0 consolidated acceptance

Date: 2026-08-10  
Milestone: [M0 — Proving grounds](../milestones/M0-proving-grounds.md)  
Machine summary: [2026-08-10-m0-acceptance.json](2026-08-10-m0-acceptance.json)

## Decision

**M0 is accepted. All seven acceptance criteria pass, and M1 is unblocked.**

The single acceptance command was:

```sh
scripts/m0-acceptance.sh
```

It ran every gate in order, stopped on failure, and atomically retained each
command, exit code, duration, environment, log path, and evidence path in the
linked JSON. The accepted run started at `2026-08-10T18:02:50Z`, finished at
`2026-08-10T18:27:31Z`, and passed all ten runner steps in 1,481.009 seconds.

## Environment and input identity

| Field | Accepted value |
|---|---|
| CPU / RAM | Apple M1 Max / 68,719,476,736 bytes |
| OS / architecture | macOS 27.0 / arm64 |
| Blender | 5.2.0 LTS |
| Python used by runner | 3.14.2 |
| Rust | rustc 1.94.1 (`e408947bf`, 2026-03-25) |
| Cargo | 1.94.1 (`29ea6fb6a`, 2026-03-24) |
| Git HEAD before the Task 7 changes | `fb5ee70536eb6e90007a85d1de26a09dc3c2f60f` |
| Source-tree BLAKE2b-256 | `11582ea370d5177a71d5358af1758e5a8ca73460746d253bd4bfa094834abcf9` |
| Reference project BLAKE2b-256 | `53234d8a2c5a203b16f723ac984f12d4395f394c46e452fcc5177688ae00321f` |
| Cargo.lock BLAKE2b-256 | `581ad148a853e7b0524dd0e59393d1bf3569d15d2608706864d70aafd1ad36f6` |

The accepted tree was intentionally dirty because the acceptance runner, its
tests, refreshed baselines, and this evidence set were the Task 7 work being
proved before commit. The source-tree hash covers tracked and non-ignored
untracked inputs while excluding the summary itself. The machine summary pins
the individual cache schema and six baseline hashes as well.

## Complete gate result

The workspace step excludes the four density tests in debug mode because the
same four tests run immediately afterward in the required release profile. It
does not omit them from acceptance.

| Step | Exact command | Result | Duration |
|---|---|---:|---:|
| Workspace tests | `cargo test --workspace -- --skip no_agent_state_goes_non_finite_under_density --skip no_agent_escapes_far_beyond_the_scene_bounds --skip speeds_never_exceed_the_per_agent_maximum --skip the_crowd_does_not_deadlock_wholesale` | PASS | 352.953 s |
| Release density | `cargo test --release -p crowd-core --test fuzz_density` | PASS, 4/4 | 547.264 s |
| Release reroute | `cargo test --release -p crowd-core --test two_room_reroute -- --ignored --nocapture` | PASS | 22.631 s |
| Six baselines | `cargo run --release -p crowd-bench -- check --agents 1000` | PASS, 6/6 | 474.460 s |
| Cache lifecycle | `cargo test -p crowd-cache` | PASS, 28 tests | 1.946 s |
| Cache matrix | `scripts/cache-experiment.sh` | PASS, 9 candidates | 4.294 s |
| Wheel/facade | `scripts/build-wheel.sh && scripts/verify-wheel.sh` | PASS | 9.773 s |
| Clean Blender install | `scripts/blender-install-test.sh` | PASS | 6.097 s |
| Blender playback | `scripts/blender-playback-test.sh` | PASS | 58.379 s |
| Static checks | `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && python3 tests/test_m0_acceptance_runner.py && git diff --check && rg '^## ' docs/blender-crowd-1.0.md && rg '^#' docs/milestones/*.md` | PASS | 2.307 s |

Per-step console logs are under the ignored
`benchmarks/reports/m0-acceptance-logs/` directory. Durable measured cache
results, schemas, baselines, and this report are checked in.

## Selected M0 path

- **Avoidance:** `sampled_velocity`. The 72-run solver comparison found the
  best penetration and predicted time-to-collision profile at scale without a
  scene-specific collapse, while still exceeding 30 ticks/s at 2,000 agents.
  ORCA remains faster but has much worse penetration; the scoped anticipatory
  solver has its worst failure in the circle scene.
- **Navigation:** the deterministic `crowd_core::nav` tiled grid, A* corridor,
  budgeted `plan` phase, and named multi-portal topology events. The 1,000-agent
  `two_room` gate closes the whole south door and proves unrelated corridors
  remain untouched while affected agents replan through the north door.
- **Cache:** Crowd Cache v1 directory format with a static agent table,
  checksummed atomic chunks, incomplete/canceled/complete manifests, and a
  recovery inspector. The measured default is affine-i16 position encoding in
  120-tick chunks. The selected candidate used 6,748,865 bytes for 1,000 agents
  over 120 frames, observed 0.000240 m maximum position error, wrote at 1,861.6
  frames/s, and read at 260.4 frames/s on this machine.
- **Bridge:** a CPython-abi3 PyO3 wheel exposing coarse compile, session, bake,
  cancel, cache, and query operations. Blender Python orchestrates these
  operations; Geometry Nodes presents bulk cached attributes and owns no
  authoritative simulation logic.

## Acceptance criteria 1–7

| # | Evidence | Result |
|---:|---|---|
| 1 | The same six scenes, four scales, and three solver implementations produce 72 comparable reports; the selection and rejected tradeoffs are documented in [the avoidance comparison](2026-08-06-avoidance-solver-comparison.md). | PASS |
| 2 | Workspace determinism covers per-tick state hashes, spawn-order permutation, add-one-agent stability, seed sensitivity, and exact discrete state. Cache codecs enforce declared continuous bounds. | PASS |
| 3 | The release-only 1,000-agent `two_room` test closes a timed multi-portal door, reroutes affected agents, and preserves 495 unrelated corridors; see [the navigation report](2026-08-08-tiled-navmesh-prototype.md). | PASS |
| 4 | Cache tests cover complete nonsequential reads, corruption/missing files, cancellation before and after chunks, incomplete recovery, orphan temporary files, and stable slot IDs. The nine-candidate [cache experiment](2026-08-10-cache-v0-experiment.md) supplies the measured default. | PASS |
| 5 | A fresh Blender 5.2 install builds the extension archive, removes any prior extension, installs/enables it, and loads the abi3 native module from Blender's extension-local site-packages path. | PASS |
| 6 | A simulation process writes a 199,220,032-byte, 5,692-tick trace and exits. A fresh Blender process then presents exactly 1,000 points with stable IDs at 0.1601 seconds total / 0.0281 ms per tick. | PASS |
| 7 | The isolated kernel reports 98–105 ticks/s at 1,000 agents against the 30 ticks/s real-time budget, a 3.3x margin. The accepted playback run keeps its 57.2558-second simulate-plus-serialize bake separate from Blender playback and makes no unmeasured scale extrapolation. | PASS |

## First failure and corrective evidence

The first complete runner correctly stopped at the six-scene baseline gate.
All six baselines had obsolete scene hashes after the scene-identity contract
was expanded to include navigation presence and data. A new fast test now
rebuilds every checked baseline input and compares its current scene identity.

That test then exposed a second issue: the circle fixture computed authored
points with runtime trigonometry, and optimized angle reassociation changed one
or more f32 bits between debug and release builds. The fixture now pins the
previously measured release coordinate bits directly. A focused test asserts
all 16 points in both profiles. The final six-scene gate passes, and the circle
quality metrics remain identical to the pre-fix release baseline; the fix did
not replace the benchmark with a more favorable geometry.

## Measured budget and separated costs

The contract's real-time 1,000-agent proving budget is met by the isolated
kernel measurement: 98–105 ticks/s versus 30 ticks/s required. This is not the
same measurement as the accepted playback runner:

- simulate plus serialize 5,692 ticks to a 199 MB trace: 57.2558 s;
- load and seek the 1,000-point trace in Blender: 0.1601 s total;
- Blender point playback: 0.0281 ms/tick.

The bake number includes cargo invocation and trace disk I/O. It is not an
isolated simulation-throughput number, and none of these numbers includes
armature evaluation or renderer time.

## Known limitations and unsupported claims

- Evidence is from one Apple M1 Max running macOS arm64 and Blender 5.2 LTS.
  Cross-machine, Linux, Windows, and other Blender versions are not proven.
- The selected navigation prototype uses tile-center and portal-midpoint
  polylines, has a fixed tile size, and does not yet funnel-smooth corridors.
- Avoidance quality still degrades at constrictions and high density. M0
  selects the most credible measured default; it does not claim final crowd
  quality.
- The legacy trace-v0 playback proof coexists with Cache v1. M1 must render
  from the completed Cache v1 path with the simulation session destroyed.
- No 10,000- or 100,000-agent behavior, memory, cache, playback, or render
  result is claimed. Those gates remain explicitly unmeasured.
- No armature, Eevee, or Cycles performance result is claimed by M0.

## M1 unblock decision

M1 is unblocked because every M0 criterion has fresh passing evidence and a
single selected navigation/avoidance/cache/bridge path. M1 must retain the
limitations above and prove the checked concourse input through strict Cache
v1 rebake, cache-only Blender presentation, sparse override, and separated
render measurements; this report does not pre-accept any M1 criterion.
