# M3 — Production Blender Crowd 1.0

## Objective

Harden the authorable MVP into a trustworthy, installable, recoverable release
on a declared Blender/OS support matrix.

## Sources of truth

- [Definition of Blender Crowd 1.0](../blender-crowd-1.0.md#18-definition-of-blender-crowd-10)
- [Verification and risk sections](../blender-crowd-1.0.md#15-verification-strategy)
- [M2 authorable MVP](M2-authorable-mvp.md)

## Prerequisites

M2 acceptance passes. Quality and performance thresholds are fixed from
checked-in baselines; no threshold is invented after seeing a release result.

## In scope

- Cross-platform native packaging for the declared Blender LTS-compatible
  versions and OS/architecture matrix.
- Cache atomicity, cancellation, recovery, invalidation explanations, migration,
  partial corruption handling, optional channels, and compatibility policy.
- Blender undo/redo, save/reload, dependency-graph, linked/overridden data,
  missing assets, moved projects, and clean-preference stress tests.
- Performance, memory, playback, armature, and render budgets with resource
  estimation and bounded debug overhead.
- Crash diagnostics that exclude private scene content by default.
- Signed/reproducible release artifacts where supported, license/SBOM review,
  documentation, examples, upgrade notes, and support triage policy.

## Explicit exclusions

M3 does not claim Golaem/MASSIVE feature completeness, validated USD layering,
10K/100K scale, advanced motion matching, or Blender mainline readiness. Those
have their own gates.

## Required artifacts

- Installable release archives, build provenance, dependency/license manifest,
  support matrix, migration fixtures, failure-drill reports, and release notes.
- CI or reproducible platform runners for every supported combination.
- Dated 1.0 acceptance report containing all section 12 metrics and limitations.

## Acceptance criteria

1. Every supported clean installation enables, authors, bakes, reloads, and
   renders the acceptance scene through documented commands/workflows.
2. Complete caches survive playback without the simulator; incomplete, corrupt,
   stale, older, and newer caches fail or degrade according to documented policy.
3. Undo/save/reload and dependency-graph tests reveal no hidden authoritative
   simulation state or uncontrolled scene mutation.
4. Performance, memory, cache, playback, quality, and package-size thresholds
   pass on the declared matrix, with debug features measured separately.
5. Release binaries contain no contributor-machine paths or unreviewed runtime
   dependencies and carry complete license/provenance information.
6. The user, node, schema/cache, troubleshooting, headless, and upgrade docs are
   exercised by release tests or a documented review.
7. Known limitations and crash/data-recovery procedures are public and specific.

## Validation and proof

Run the complete core, property, integration, determinism, performance, package,
Blender headless, render, migration, and failure-drill suites from clean release
artifacts. Record each environment and link immutable outputs in the release
report. A development-tree pass does not substitute for archive testing.

## Definition of done and stop conditions

M3 is done only when all Blender Crowd 1.0 criteria pass from clean release
archives. Stop the release on data loss, cache misidentification, unsupported
binary linkage, nondeterministic discrete decisions, or an unbounded crash. Do
not reclassify a failing criterion as post-1.0 to ship on schedule.
