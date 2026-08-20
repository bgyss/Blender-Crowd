# M6 — Advanced agency, motion, interactions, and hero physics

## Objective

Complete the path from the 1.0 behavior graph to MASSIVE-class inspectable agency
and pair it with a modern trajectory-aware motion stack for close-to-camera
characters, while preserving deterministic lower-fidelity tiers.

## Current status

M6 is accepted with criterion 5 deferred to M9. The
[2026-08-20 requirement-level audit](../benchmarks/2026-08-20-m6-acceptance.md)
adjudicates criteria 1–4 and 6–10 as PASS at their documented deterministic
fixture or host-Blender proof levels.

Criterion 5, production motion matching, was rescoped out of M6 into
[M9 Track C](M9-neural-animation-operator-validation.md) on 2026-08-20 because
it is blocked on unscheduled motion data acquisition rather than on implemented
behavior. It is deferred, not satisfied: the CMU candidate has 3,587 measured
joint-limit violations against the hard limit of zero and stays rejected, and
the accepted CC0 authored motion is a narrow deterministic fixture baseline
only. Every measured threshold moved to M9 unchanged, and M6 makes no
production motion-matching claim. See the
[deferral record](../benchmarks/2026-08-20-m6-criterion-5-deferral.md). The
`motion_source` gate still runs on every audit and still fails M6 closed on
malformed or inconsistent motion evidence, because the CC0 fixture it validates
is consumed by criteria 3, 4, and 6.

This status does not claim Blender cloth/hair/Geometry Nodes deformation,
rigid-body parity, arbitrary-scene or long-duration performance, GPU execution,
or visual quality. R1–R4 neural animation and independent-user verification
remain deferred to M9.

## Sources of truth

- [Industrial capability ledger](../industrial-crowd-capability-roadmap.md#industrial-capability-ledger)
- [Blender Crowd 1.0 post-1.0 ordering](../blender-crowd-1.0.md#17-post-10-roadmap-ordering)
- [Crowd simulation research synthesis](../crowd-simulation-research-2026.md)
- Future [M9 neural animation and operator validation](M9-neural-animation-operator-validation.md)
- [M4 layered layout](M4-layout-interchange.md) and [M5 tiers](M5-scale-rendering.md)
- [UI/UX roadmap](../ui-ux-roadmap.md)

## Prerequisites

M3 is accepted. M4 layer/physics/cache contracts and M5 fidelity scheduling are
stable enough that advanced state can remain optional and versioned. Licensed
motion/trajectory data and redistribution terms are documented before ingestion.

## In scope

1. Typed perception for vision cones, occlusion/line of sight, attention targets,
   hearing events, touch/contact, nearest friend/threat, local density/flow,
   semantic distance, group extent, and bounded memory.
2. Brain authoring that combines explicit states, utility scorers, behavior-tree
   composition, arbitrary typed blackboard variables, deterministic fuzzy
   membership/comparison nodes, reusable action libraries, interrupts, and
   hundreds-of-actions scale tests without arbitrary runtime code.
3. Rich activities: schedules, needs, seats, doors, handoffs, reservations,
   paired actions, capacities, conversational groups, and recoverable failure.
4. Full group roles and formations for families, squads, tours, protest and
   emergency groups, including shared perception and group-level decisions.
5. Motion database build/validation, future-trajectory queries, pose/contact
   features, motion matching, animation graph integration, stride/turn warping,
   terrain/slope adaptation, foot locking/IK, and navigation feedback. The
   implementation is in M6 scope and is exercised against the checked CC0
   fixture; accepting it against a *production* motion corpus is M9 Track C.
6. Interaction animation, authoritative contact ownership, animation-to-ragdoll
   and recovery transitions, rigid-body layers, and promoted-hero cloth/hair or
   facial integration through Blender-supported presentation/physics paths.
7. Offline trajectory evaluation/profile fitting and optional deterministic
   authoring critics; learned runtime control is deferred to M9 and is never
   silently substituted for the deterministic graph.
8. A documented behavior/action extension boundary for studios and researchers.
   Python remains coarse-grained; native extensions use versioned IR or a stable
   C ABI with an optional C++ wrapper only if measured demand justifies its
   compatibility and security cost.
9. A model-independent R0 interaction-motion boundary for promoted groups. The
   core owns roles, roots, required/forbidden contacts, outcome, and fallback;
   the deterministic paired-clip worker output must pass validation before
   becoming a sparse animation layer. Learned backends begin in M9.

## UI/UX goals and automated verification

- Evolve the M2 trace view into a scalable brain debugger with a synchronized
  event timeline, current graph state, decisive node, observations, utility
  scores, blackboard changes, interrupts, and cross-agent/group context.
- Navigate bidirectionally between viewport agent, cached event, graph node,
  action, motion clip, contact, and resulting correction without copied IDs.
- Support graph search, reusable subgraphs/actions, presets, typed ports, compile
  diagnostics, large-graph overview, and focused trace highlighting without
  turning the graph into an unbounded scripting surface.
- Present motion matching, trajectory fit, foot contact, terrain, interaction,
  ragdoll, recovery, and solver ownership as readable diagnostics with the
  responsible layer and failure/recovery action identified.
- Allow evidence density and debug cost to be reduced by tier while clearly
  stating which observations or diagnostics are unavailable.

M6 automated verification passes when checked Blender tests cover every UI goal
above: current-source loading, trace inspection, graph/path search and
highlighting, bidirectional navigation without copied IDs, reusable
subgraphs/actions and presets, typed/compile diagnostics, motion/contact/physics
ownership, corrections, and full versus reduced evidence. The current host
Blender runner now covers that automated list through its debugger and layer
processes, including native graph compilation and correction/lifecycle failure
paths. This is checked-fixture automation, not independent-user evidence;
independent-user authoring and repair verification remains an M9 gate.

## Explicit exclusions

No per-agent per-frame LLM/VLM, opaque behavior that cannot be traced, universal
automatic rig conversion, unlicensed motion data, or requirement that cloth/hair
run on all agents. Neural animation and independent-user verification belong to
M9. Semantic AI and domain packs belong to M8.

## Required artifacts

- Versioned perception, brain, activity, interaction, trajectory, contact, and
  optional physics/cache schemas with migration fixtures.
- Node/action library, debugger, motion database tooling, canonical rig/retarget
  profiles, and redistributable reference motions.
- Social, activity, terrain, paired-interaction, ragdoll/recovery, and mixed-tier
  acceptance scenes plus dated evidence reports.
- Versioned R0 interaction request/response schemas, deterministic paired-clip
  worker baseline, constraint/contact validators, cache-layer fixtures, and
  worker-absent playback proof.

## Acceptance criteria

1. A brain can reproduce state-machine, utility, and behavior-tree reference
   graphs; all decisive observations/scores/transitions are traceable and strict-
   mode reproducible.
2. Vision, hearing, touch, density/flow, group, and semantic observations pass
   occlusion, ordering, budget, and cache/trace tests.
3. A scheduled needs/activity scene reserves finite resources without double
   ownership, deadlock outside declared policy, or nondeterministic admission.
4. Social/group scenes improve their fixed formation/split/intrusion and intent-
   readability metrics without regressing hard safety thresholds.
5. **DEFERRED TO M9 (2026-08-20).** Motion-matched agents meet trajectory,
   contact, foot-slip, turn, terrain, transition, and performance thresholds
   against the clip-state baseline. Rescoped to
   [M9 Track C](M9-neural-animation-operator-validation.md) with all measured
   thresholds unchanged; it is unclaimed until M9 closes it. The criterion
   number is retained so existing evidence and cross-references stay valid.
6. The crowd/motion feedback loop remains stable: infeasible motion is reported
   and constrained without allowing animation to teleport or hide collisions.
7. Interaction and ragdoll/recovery results compose as optional layers and
   preserve unrelated agents, base caches, and lower-fidelity playback.
8. Hero cloth/hair/facial integrations declare their solver, ownership, cache,
   failure, and support boundaries rather than becoming hidden dependencies.
9. External behavior examples pass determinism, channel-declaration, cost-budget,
   version-mismatch, and failure-isolation tests on every claimed API language.
10. The deterministic R0 interaction boundary passes request/response,
    validation, fallback, sparse-layer composition, reload, removal,
    cross-cache rejection, unrelated-agent isolation, and worker-absent replay.

## Validation and proof

Use golden graph/IR tests, deterministic perception scenarios, large action-
library compile/runtime benchmarks, resource-reservation properties, social
comparisons, motion/contact/terrain fixtures, physics transition/recovery tests,
mixed-tier performance reports, deterministic partner perturbations, and worker
failure/recovery tests. Human and learned-model evidence belongs to M9.

## Definition of done and stop conditions

M6 is done when authored brains and trajectory-aware motion pass the measurable
reference scenes, the automated Blender debugger proof passes, and the result
remains explainable, layer-compatible, and scalable by tier. Production motion
matching against an accepted clip-state baseline is explicitly out of this
definition as of 2026-08-20 and belongs to M9 Track C.
Stop if a learned or physics path becomes untraceable authoritative state, if
motion quality is asserted only from a reel, or if hero features compromise the
background contracts.

## Deferred future workstream

[M9](M9-neural-animation-operator-validation.md) owns the production motion
corpus and criterion 5 (Track C), R1–R4 neural animation,
model/checkpoint/data authorization, blinded perceptual claims, and independent
operator verification. Those gates may consume accepted M6 artifacts but do not
reopen or block M6.
