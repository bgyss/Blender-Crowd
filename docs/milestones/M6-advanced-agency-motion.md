# M6 — Advanced agency, motion, interactions, and hero physics

## Objective

Complete the path from the 1.0 behavior graph to MASSIVE-class inspectable agency
and pair it with a modern trajectory-aware motion stack for close-to-camera
characters, while preserving deterministic lower-fidelity tiers.

## Sources of truth

- [Industrial capability ledger](../industrial-crowd-capability-roadmap.md#industrial-capability-ledger)
- [Blender Crowd 1.0 post-1.0 ordering](../blender-crowd-1.0.md#17-post-10-roadmap-ordering)
- [Crowd simulation research synthesis](../crowd-simulation-research-2026.md)
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
   terrain/slope adaptation, foot locking/IK, and navigation feedback.
6. Interaction animation, authoritative contact ownership, animation-to-ragdoll
   and recovery transitions, rigid-body layers, and promoted-hero cloth/hair or
   facial integration through Blender-supported presentation/physics paths.
7. Offline trajectory evaluation/profile fitting and optional authoring critics;
   learned runtime control is a separately evidenced experiment, never silently
   substituted for the deterministic graph.
8. A documented behavior/action extension boundary for studios and researchers.
   Python remains coarse-grained; native extensions use versioned IR or a stable
   C ABI with an optional C++ wrapper only if measured demand justifies its
   compatibility and security cost.

## UI/UX goals and gate

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

The M6 UI gate passes when independent users author and repair representative
state-machine, utility, behavior-tree, motion-contact, and interaction failures
using only the interface and node reference. Evidence must show trace-to-node
agreement, discovery and repair time, large-graph navigation, debug overhead,
and clear degraded-evidence states at lower fidelity tiers.

## Explicit exclusions

No per-agent per-frame LLM/VLM, opaque behavior that cannot be traced, universal
automatic rig conversion, unlicensed motion data, or requirement that cloth/hair
run on all agents. Semantic AI and domain packs belong to M8.

## Required artifacts

- Versioned perception, brain, activity, interaction, trajectory, contact, and
  optional physics/cache schemas with migration fixtures.
- Node/action library, debugger, motion database tooling, canonical rig/retarget
  profiles, and redistributable reference motions.
- Social, activity, terrain, paired-interaction, ragdoll/recovery, and mixed-tier
  acceptance scenes plus dated evidence reports.

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
5. Motion-matched agents meet trajectory, contact, foot-slip, turn, terrain,
   transition, and performance thresholds against the clip-state baseline.
6. The crowd/motion feedback loop remains stable: infeasible motion is reported
   and constrained without allowing animation to teleport or hide collisions.
7. Interaction and ragdoll/recovery results compose as optional layers and
   preserve unrelated agents, base caches, and lower-fidelity playback.
8. Hero cloth/hair/facial integrations declare their solver, ownership, cache,
   failure, and support boundaries rather than becoming hidden dependencies.
9. External behavior examples pass determinism, channel-declaration, cost-budget,
   version-mismatch, and failure-isolation tests on every claimed API language.

## Validation and proof

Use golden graph/IR tests, deterministic perception scenarios, large action-
library compile/runtime benchmarks, resource-reservation properties, social
comparisons, motion/contact/terrain fixtures, physics transition/recovery tests,
mixed-tier performance reports, and blinded human review only for claims that
genuinely concern perceived realism.

## Definition of done and stop conditions

M6 is done when authored brains and trajectory-aware motion pass the measurable
reference scenes and remain explainable, layer-compatible, and scalable by tier.
Stop if a learned or physics path becomes untraceable authoritative state, if
motion quality is asserted only from a reel, or if hero features compromise the
background contracts.
