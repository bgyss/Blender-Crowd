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

The repository is still documentation-only. The detailed
[deterministic kernel implementation plan](../superpowers/plans/2026-08-04-crowd-sim-kernel.md)
is the first executable slice of M0; it has not been implemented merely because
the plan exists.

Current documentation checks:

```sh
git diff --check
rg '^## ' docs/blender-crowd-1.0.md
rg '^#' docs/milestones/*.md
git status --short
```

