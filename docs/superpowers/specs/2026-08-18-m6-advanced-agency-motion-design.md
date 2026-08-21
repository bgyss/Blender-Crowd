# M6 Advanced Agency and Motion Design

## Decision

Implement M6 as a sequence of deterministic, versioned vertical slices that
share one ownership boundary: Rust owns intent, observations, roots, contacts,
outcomes, event timing, and fallback decisions; Blender owns authoring and
presentation; optional workers may propose motion but cannot become simulation
or render-time dependencies.

The first executable gate is R0 from the reactive-motion research track. It
provides a model-independent interaction request/response path, an authored
paired-clip baseline, validation, an optional sparse animation layer, and
model-absent playback. R0 is a required M6 artifact and proof gate, not a claim
that the complete M6 milestone is finished.

## Problem and constraints

The current repository has a deterministic behavior graph compiler and runtime,
M4 adjacent layer composition, M4 physics handoff, and M5 fidelity scheduling.
It does not yet have versioned M6 perception, activity, trajectory, contact, or
interaction contracts. The M6 contract requires those capabilities to remain
inspectable, deterministic in strict mode, compatible with lower tiers, and
safe to remove without mutating the base cache.

The implementation must therefore:

- preserve stable IDs, fixed-step ordering, and Rust/Python/Geometry Nodes
  ownership boundaries;
- use fixed-point or explicitly bounded numeric representations at
  authoring/runtime decision boundaries where strict reproducibility matters;
- keep all M6 artifacts adjacent to the base cache and content-addressed by
  source/provenance hashes;
- make unsupported, invalid, over-budget, or unavailable motion fall back to
  deterministic clip-state behavior;
- reject penetration, impossible contacts, root teleportation, undeclared
  channels, version mismatches, and cross-cache layer attachment;
- keep semantic domain packs and combat-specific meaning in M8; M6 provides
  generic interaction roles, contact constraints, and extension points;
- report local proof separately from production, licensed-data, and hardware
  evidence; independent-user and learned-model claims are deferred to M9.

## Architecture

### Versioned contracts

The repository will add schemas and serde types for:

- typed perception snapshots and bounded memory;
- brain/blackboard and action-channel declarations;
- scheduled activities and resource reservations;
- trajectory samples, motion clips, and motion-match queries/results;
- contact intents/results and interaction request/motion responses;
- optional physics/recovery transitions and hero-solver provenance.

Every schema rejects unknown fields, carries a version, and has a golden
fixture plus migration or rejection coverage. Rust types are the validation
authority; Python helpers only construct and display already validated data.

### Runtime data flow

```text
World + neighbors + semantics
        -> typed perception snapshot and bounded memory
        -> deterministic brain VM / activity and group decisions
        -> root trajectory and interaction intent
        -> motion query or optional worker request
        -> validator
             accepted -> sparse animation/physics layer
             rejected -> deterministic clip or motion-match fallback
        -> trace/cache/debugger/presentation
```

The base simulation cache is never rewritten by a motion worker. Accepted
outputs reference the base cache hash, target stable IDs, interval, provenance,
and validator result. Lower-fidelity tiers retain the deterministic base
channel and may omit M6 diagnostics with an explicit degraded-evidence state.

### R0 interaction baseline

`interaction_request_v1` contains stable request/group/participant IDs, roles,
tick interval, fixed seed, mode, authored action/outcome, root/facing samples,
retarget profile IDs, required and forbidden contact windows, environment and
prop constraints, and cost/provenance budgets.

`interaction_motion_v1` contains per-participant root deltas and skeletal
channels, contact labels, support state, continuity metadata, provenance,
warnings, unsatisfied constraints, and a deterministic fallback reference.

The checked-in worker is an authored paired-clip adapter. It exercises the
same request/response boundary an external worker will use, but has no model
or accelerator dependency. It produces a sparse M4-compatible animation layer
only after validation. The validator owns the acceptance decision and never
changes the authored outcome.

### Agency and motion slices after R0

After R0 is green, the remaining M6 work proceeds in this order:

1. typed perception and richer trace evidence;
2. typed blackboards, fuzzy comparisons, reusable actions, and large graph
   compilation/runtime;
3. deterministic activities, reservations, schedules, and full group roles;
4. motion database tooling, trajectory queries, matching, warping, terrain,
   foot locking, and navigation feedback;
5. interaction-to-physics/recovery layers and declared hero solver boundaries;
6. Blender brain/motion debugger, extension-boundary examples, scale evidence,
   bidirectional navigation, reusable subgraph/action/preset authoring, and
   automated trace/search/diagnostic/degraded-evidence verification.

Each slice must leave the earlier slices runnable and must add a dated report
or update the active M6 evidence ledger.

The current debugger foundation proves trace summary, graph search/highlight,
and degraded-evidence text. Bidirectional navigation and reusable
subgraph/action/preset authoring remain pending M6 work.

## Failure and recovery policy

Validation failures are structured records with a stable code, target, interval,
severity, and corrective action. A failed worker request is isolated to its
interaction group; the base cache and unrelated agents remain playable. A
worker crash, missing model, unsupported hardware, corrupt response, or budget
overflow selects the deterministic fallback and records why. An accepted layer
can be muted, removed, replaced, or replayed without regeneration.

## Verification

The implementation must provide:

- unit, property, schema, migration, determinism, and failure-isolation tests;
- golden perception/brain/activity/trajectory/contact/interaction fixtures;
- R0 request/response, validator, layer, reload, correction, removal, and
  model-absent playback tests;
- motion matching and physics transition fixtures with declared thresholds;
- a copy-ready `scripts/m6-foundation-test.sh` runner before reporting any
  local M6 foundation result;
- dated reports that distinguish accepted criteria, missing evidence, and
  unsupported claims.

No physics quality claim is accepted from a render or reel alone. Neural quality
and independent-user claims belong to M9. The full milestone is complete only
when the deterministic M6 acceptance criteria and automated Blender verification
have requirement-level evidence; otherwise the report must name the exact
remaining M6 gates.

## Scope migration

The approved 2026-08-18 milestone split moved R1–R4 neural animation and the
independent-user debugger/motion-authoring study to
[M9](../../milestones/M9-neural-animation-operator-validation.md). R0 remains
M6 because it is model-independent and supplies the deterministic boundary that
future backends must consume.
