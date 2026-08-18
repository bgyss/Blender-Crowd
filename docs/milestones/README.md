# Blender Crowd milestone contracts

These files are the single ordered execution path from the current project
contract to the industrial product destination. They replace informal feature
lists; they do not replace the canonical
[Blender Crowd 1.0 contract](../blender-crowd-1.0.md) or the
[capability roadmap](../industrial-crowd-capability-roadmap.md).

## Sequence

| ID | Contract | Gate |
|---|---|---|
| M0 | [Proving grounds](M0-proving-grounds.md) | Select and prove the kernel, navigation, cache, and Blender bridge architecture |
| M1 | [1,000-agent vertical slice](M1-vertical-slice.md) | Bake and render the simple concourse end to end |
| M2 | [Authorable MVP](M2-authorable-mvp.md) | A non-developer authors the full reference shot and corrects it sparsely |
| M3 | [Production 1.0](M3-production-1.0.md) | Clean-install production acceptance passes on the support matrix |
| M4 | [Layered layout and interchange](M4-layout-interchange.md) | Golaem-class post-sim direction and validated layered exchange |
| M5 | [Scale and procedural rendering](M5-scale-rendering.md) | Earn 10K, then 100K, with declared tier mixes and reproducible evidence |
| M6 | [Advanced agency, motion, and physics](M6-advanced-agency-motion.md) | MASSIVE-class brains, trajectory-aware motion, and validated reactive-interaction research |
| M7 | [Blender ecosystem and mainline readiness](M7-blender-integration.md) | Production evidence supports narrowly scoped upstream proposals |
| M8 | [Semantic authoring, domain packs, and synthetic data](M8-semantic-domains-data.md) | Expansion tracks reuse stable core contracts without weakening determinism |

M0 through M3 are strict ordered gates. Do not start a later gate to avoid an
unmet earlier exit condition. M4 through M6 may be staffed as coordinated tracks
after M3, but their schema and cache changes must remain compatible. M7 collects
evidence throughout the project but cannot claim readiness before M3 and real
production evaluation. M8 consumes stable outputs from M4 through M6.

## Rules shared by every milestone

1. Read the canonical 1.0 contract, capability roadmap, this index, and the
   milestone file before implementation.
2. Preserve stable IDs, fixed-step simulation, deterministic event ordering,
   versioned schemas, and the Rust/Python/GN ownership boundaries.
3. Geometry Nodes remains authoring/presentation, not authoritative simulation.
4. Add a test with every implemented behavior. Performance claims require a
   checked-in fixture, runner, report, and recorded environment.
5. Do not let a lower proof tier satisfy a higher gate. Scaffolding, schema
   checks, synthetic fixtures, a render, or one-machine benchmarks prove only
   what they directly exercise.
6. Do not introduce proprietary code, copied UI, production assets without
   redistribution rights, or dependencies without license review.
7. Cloud services, paid compute, external publication, and communication with
   Blender maintainers require explicit authorization in the task that performs
   them. Planning and local artifacts remain inert until then.
8. When an implementation runner is introduced, update `AGENTS.md` and
   `README.md` with exact copy-ready commands. Never claim a nonexistent runner
   passed.
9. End each milestone with a dated evidence report listing environment, inputs,
   results, known failures, unsupported claims, and the next gate.

## Current baseline

As of 2026-08-18, **M0 through M5 are accepted**, with one qualification on M5:
both scale gates pass, but the M5 UI gate's artist task has not been conducted,
so M5 is functionally accepted and not operator-validated.

M5's two scale gates are recorded in the
[10K report](../benchmarks/2026-08-14-m5-10k.md) (2026-08-14) and the
[100K report](../benchmarks/2026-08-18-m5-100k.md) (2026-08-18). Read the 100K
report's "Exactly one threshold change is load-bearing" section before citing
the result: the pass turns on a single recalibrated limit, and the reasoning is
set out there rather than asserted. The
[scale-invariance report](../benchmarks/2026-08-15-m5-100k-scale-invariance.md)
records the three metric defects found on the way and the solver theory that
measurement refuted.

M4's layer, physics-handoff, procedural-render, migration, and narrow OpenUSD
profile evidence is recorded
in the [M4 acceptance evidence](../benchmarks/2026-08-12-m4-foundation.md).
The complete
M0 ordered runner and criterion-by-criterion result are in the
[M0 consolidated acceptance report](../benchmarks/2026-08-10-m0-consolidated.md),
with its [machine-readable summary](../benchmarks/2026-08-10-m0-acceptance.json).
The strict rebake, cache-only Blender workflow, sparse override, and separated
render evidence are in the
[M1 vertical-slice acceptance report](../benchmarks/2026-08-10-m1-vertical-slice.md).
M2 engineering and operator evidence is consolidated in the
[M2 acceptance record](../benchmarks/2026-08-12-m2-acceptance.md). The remaining
UI/UX and Figma work is deliberately deferred and tracked in the
[UI/UX roadmap](../ui-ux-roadmap.md#deferred-uiux-todo); it does not reopen M2.

Implemented: a Rust workspace of five crates (`crowd-core`, `crowd-cache`,
`crowd-trace`, `crowd-blender`, and `crowd-bench`); versioned project/cache
schemas; a deterministic checked reference project; the Blender extension;
the selected tiled-navigation and sampled-velocity path; a recoverable cache;
and the coarse abi3 Python facade.

### M0 in-scope items

| # | Item | State |
|---|---|---|
| 1 | Clock, IDs, SoA state, spatial index, tick phases, metrics | Done |
| 2 | Avoidance scenes at 100/500/1,000/2,000 agents | Done — six scenes, exceeding the five required |
| 3 | Three avoidance candidates behind one interface, compared | Done — `sampled_velocity` selected |
| 4 | Tiled navmesh/corridor prototype, portal change, path budgeting | Done — `crowd_core::nav`, the `plan` phase, and the `two_room` scene |
| 5 | Cache v0 experiments | Done — Cache v1 selected as affine-i16 / 120-tick chunks |
| 6 | Extension skeleton, packaging spike, 1,000-point GN playback | Done |
| 7 | Coarse Python/Rust facade and bundled-CPython ABI validation | Done |

### M0 acceptance criteria

All seven criteria are met. The complete runner passed workspace and release
stress tests, the 1,000-agent timed-portal reroute, all six selected-solver
baselines, cache completion/cancellation/corruption recovery, the measured
cache matrix, the abi3 coarse facade, a clean Blender install, and fresh-process
1,000-point playback. The report preserves simulation/bake/playback distinctions
and makes no 10K/100K claim.

Evidence to date, each with its own environment and unsupported-claims section:

- [Kernel slice 1](../benchmarks/2026-08-05-kernel-slice-1.md)
- [Avoidance solver comparison](../benchmarks/2026-08-06-avoidance-solver-comparison.md)
- [Blender bridge and native packaging](../benchmarks/2026-08-07-blender-bridge.md)
- [Tiled navmesh/corridor prototype](../benchmarks/2026-08-08-tiled-navmesh-prototype.md)
- [Cache format experiment](../benchmarks/2026-08-10-cache-v0-experiment.md)
- [M0 consolidated acceptance](../benchmarks/2026-08-10-m0-consolidated.md)
- [M1 1,000-agent vertical slice](../benchmarks/2026-08-10-m1-vertical-slice.md)

### M1 acceptance criteria

All eight criteria are met. The checked project compiles exactly 1,000 unique
stable IDs; two 10,000-tick strict bakes agree on exact static/discrete state
and have 0.0 m observed position delta; 96.4% of agents arrive with zero static
boundary escapes; the timed portal event isolates affected routes; canceled
caches remain recoverable but incomplete; fresh Blender processes play and
render the complete cache without a live session; the selected-agent overlay
and one-agent reversible override pass; and every required cost is reported
separately. The [clean-file walkthrough](../user/m1-reference-walkthrough.md)
requires no code or JSON edits.

### M2 acceptance criteria

All eight criteria are accepted. The full authorable Blender runner baked the
1,000-agent, 10,000-tick reference, persisted graph/queue/group evidence,
replayed and rendered without a live session, and proved sparse override
isolation. A project operator who did not implement M2 completed the six-step
Blender UI spot check through selected-agent inspection, overlays, and a pinned
override. The operator also identified substantial usability debt; the accepted
functional gate and the deferred redesign are recorded separately rather than
mistaking one for the other.

### Checks

`README.md` and `CLAUDE.md` carry the component runners and the complete
copy-ready M0 and M1 acceptance commands:

```sh
scripts/m0-acceptance.sh
cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture
scripts/m1-bake-test.sh
scripts/m1-blender-test.sh
scripts/m1-render-test.sh --out /tmp/blender-crowd-m1-render
```

The documentation checks remain required alongside it:

```sh
git diff --check
rg '^## ' docs/blender-crowd-1.0.md
rg '^#' docs/milestones/*.md
git status --short
```
