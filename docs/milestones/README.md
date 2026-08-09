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
| M6 | [Advanced agency, motion, and physics](M6-advanced-agency-motion.md) | MASSIVE-class perception/brain authoring and trajectory-aware hero motion |
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

As of 2026-08-08, M0 is in progress and not yet accepted. M1 is blocked.

Implemented: a Rust workspace of four crates (`crowd-core` kernel,
`crowd-trace` trace v0, `crowd-blender` PyO3 bridge, `crowd-bench` harness)
and the `addon/blender_crowd` extension with a bundled `abi3` wheel. There is
no `schemas/` or `assets/reference/` yet, because no implemented behavior owns
one.

### M0 in-scope items

| # | Item | State |
|---|---|---|
| 1 | Clock, IDs, SoA state, spatial index, tick phases, metrics | Done |
| 2 | Avoidance scenes at 100/500/1,000/2,000 agents | Done — six scenes, exceeding the five required |
| 3 | Three avoidance candidates behind one interface, compared | Done — `sampled_velocity` selected |
| 4 | Tiled navmesh/corridor prototype, portal change, path budgeting | Done — `crowd_core::nav`, the `plan` phase, and the `two_room` scene |
| 5 | Cache v0 experiments | **Open** — trace v0 is not the cache format |
| 6 | Extension skeleton, packaging spike, 1,000-point GN playback | Done |
| 7 | Coarse Python/Rust facade and bundled-CPython ABI validation | **Partial** — ABI validated, facade not built |

### M0 acceptance criteria

Met: 1 (comparable solver reports with documented tradeoffs), 2 (strict rerun,
spawn-order permutation, and add-one-agent tests all pass), 3 (a 1,000-agent
tiled-navigation case reroutes after a portal change without corrupting
unrelated corridors), 5 (clean Blender install loads the native module), 6
(1,000 cached point transforms with stable IDs, costs reported separately).

Not met: 4, which cannot be attempted before item 5 exists.
Criterion 7 is partial — the kernel slice report records the contract's
real-time 1K budget as met with margin, but the consolidated dated M0 report
that criterion asks for is not written.

Evidence to date, each with its own environment and unsupported-claims section:

- [Kernel slice 1](../benchmarks/2026-08-05-kernel-slice-1.md)
- [Avoidance solver comparison](../benchmarks/2026-08-06-avoidance-solver-comparison.md)
- [Blender bridge and native packaging](../benchmarks/2026-08-07-blender-bridge.md)
- [Tiled navmesh/corridor prototype](../benchmarks/2026-08-08-tiled-navmesh-prototype.md)

### Checks

`README.md` and `AGENTS.md` carry the copy-ready runners for the workspace
tests, density stress, benchmark scenes, wheel build, and the two Blender
runners. The documentation checks remain required alongside them:

```sh
git diff --check
rg '^## ' docs/blender-crowd-1.0.md
rg '^#' docs/milestones/*.md
git status --short
```
