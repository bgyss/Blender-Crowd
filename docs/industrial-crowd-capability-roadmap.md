# Industrial crowd capability and Blender integration roadmap

Status: long-term product and capability contract
Source lineage: August 2026 competitive and architectural survey supplied for
project planning
Canonical 1.0 contract: [Blender Crowd 1.0 architecture and MVP](blender-crowd-1.0.md)
Execution contracts: [Milestone index](milestones/README.md)

## Purpose

Blender Crowd is not intended to stop at a path-following add-on. The product
destination is a serious Blender-native crowd framework with the complete
production loop associated with Golaem, the authorable agent model associated
with MASSIVE, and a modern open architecture that can eventually support a
credible Blender ecosystem or mainline integration proposal.

This document integrates the conclusions of the supplied survey into one
traceable contract. It does not expand Blender Crowd 1.0: the 1,000-agent MVP
remains the first release gate. It prevents deferred industrial capabilities
from becoming unowned aspirations.

## Strategic conclusions

1. Blender now provides enough host infrastructure for a crowd-specific systems
   layer; the project should build that layer instead of another scatter preset.
2. Existing Blender products establish a high usability baseline for stadiums,
   curves, formations, visual diversity, custom characters, and per-instance
   edits. Blender Crowd must not trade those everyday workflows for an
   impressive but unusable simulator.
3. Golaem's decisive production ideas are post-simulation directing,
   non-destructive crowd levels, procedural rendering, viewport efficiency,
   reusable populations, and dependable cache/interchange—not movement alone.
4. MASSIVE's decisive abstraction is an agent that perceives and decides through
   an artist-authored brain, not a particle assigned a fixed behavior mode.
5. Geometry Nodes is the Blender-native authoring and presentation layer. It is
   not the authoritative navigation, behavior, or per-agent simulation kernel.
6. Navigation belongs behind a native API with tiled navmeshes, corridors,
   dynamic topology, spatial queries, and purpose-built avoidance. Recast/Detour
   is a candidate, not a pre-approved dependency.
7. The durable product is a framework of separable core, navigation, brain,
   motion, and layout/interchange contracts.
8. The highest-value production UX is “simulate, then direct”: fix a few agents,
   regions, timings, appearances, or actions without destroying a good bake.
9. Simulation, layout, animation repair, physics, and shot overrides should
   compose as sparse versioned layers. USD is the preferred interchange model
   when its representation is validated against real consumers.
10. Render-time or extraction-time procedural instancing is required for large
    scenes; full character geometry must not be expanded into the working
    `.blend` merely because an agent exists.
11. Simulation fidelity and render fidelity are separate. A point/capsule agent,
    animated background actor, and hero with IK/physics may share identity and
    intent without sharing runtime cost.
12. Motion matching should eventually exchange trajectory constraints with the
    crowd solver instead of being a one-way clip picker.
13. GPU acceleration is a measured scale path, not a 1.0 dependency. CPU, Metal,
    CUDA, or Vulkan backends must preserve the public behavior/cache contracts;
    no backend is promised before reproducible benchmarks exist.
14. Language or vision-language models belong at semantic authoring time: they
    may propose agendas or behavior graphs, but deterministic compiled behavior
    executes the shot. Per-agent per-frame LLM steering is out of scope.
15. Synthetic-data generation is a valid later product track if ground-truth
    channels, sensor models, licensing, and dataset validation are treated as
    product requirements rather than free by-products.
16. Extension-first production proof is the path toward Blender integration.
    Mainline inclusion is an external maintainer decision and must never be
    represented as guaranteed.
17. Future M9 reactive neural motion is a promoted-interaction animation option,
    not a replacement for the crowd brain. Models propose validated, cached
    motion behind a versioned contract while the core owns intent, roots,
    contacts, outcomes, fallbacks, and unrelated agents.

## Product architecture

```text
Blender authoring and Geometry Nodes presentation
                         |
                  versioned Crowd API
                         |
     +-------------------+-------------------+
     |                   |                   |
  CrowdNav           CrowdBrain         Environment
     |                   |                   |
     +-------------------+-------------------+
                         |
                  desired trajectory
                         |
                    CrowdMotion
              +----------+----------+
              |          |          |
         clip graph  matching/IK  physics handoff
              +----------+----------+
                         |
                  versioned agent cache
                         |
                    CrowdLayout
       +-----------------+------------------+
       |                 |                  |
  Blender/GN       USD/interchange    procedural render
```

`CrowdCore` underlies all five visible subsystems and owns the fixed clock,
stable IDs, data-oriented storage, events, determinism, metrics, and scheduling.
The boundaries are public data contracts; package splits follow implementation
and ownership needs rather than the diagram.

## Industrial capability ledger

The milestone column names the first release that must prove the capability.
Later milestones may deepen it. “Reference” means parity is conceptual; it does
not authorize copying proprietary code, assets, UI, or trade dress.

| Capability conclusion | Product requirement | First proof |
|---|---|---|
| Scatter, stadium, curve, formation, and background workflows are the Blender usability baseline | Population tools include regions, curves/lanes, seats/stadium layouts, formations, preview proxies, and reproducible variation | M2, broadened M4 |
| Weighted body, mesh, clothing, material, prop, and animation variation | Reusable archetype/entity profiles with stable weighted selection and per-agent reassignment | M1/M2 |
| Custom characters cannot be an afterthought | Canonical humanoid profile, explicit retarget maps, asset validation, and documented import failures | M2 |
| Tiled production navigation must not live in GN | Native navmesh build/query boundary, corridors, dynamic obstacles/topology, path budgets, cost painting, and debug overlays | M0/M1 |
| Semantic navigation needs more than walkable/blocked | Preferred and forbidden zones, lanes, stairs, escalators, doors, queues, portals, destinations, seats, resources, and timed states | M2/M6 |
| Local avoidance quality is more than collision freedom | Compare reciprocal, sampled, and anticipatory candidates; measure penetration, stalls, oscillation, jerk, near misses, and throughput | M0 |
| Group behavior is first-class | Couples, families, squads, tours, protesters, emergency groups, leaders/followers, formations, cohesion, separation, and shared goals | M2/M6 |
| Agent decisions must be arbitrary but inspectable | Typed compiled graphs combine state machines, utility AI, behavior-tree composition, blackboards, events, and bounded fuzzy numeric nodes | M2/M6 |
| Perception is part of the agent abstraction | Vision, line of sight, hearing, touch/contact, nearby friend/threat, density, flow, semantic distance, and attention feed typed observations | M2 foundation, M6 complete |
| Artists need to understand failures | Selected-agent traces expose observations, decisive node, alternatives, events, path, avoidance constraints, animation state, and stall reason | M1/M2 |
| Animation state blending is only the baseline | Validated clip sets, starts/stops/turns, phase, speed/stride warping, transition graphs, and root-motion policy | M1/M2 |
| Terrain and foot adaptation materially affect quality | Terrain projection, slope policy, foot contacts, foot locking, stride warping, IK, and failure diagnostics | M2 foundation, M6 complete |
| Motion and trajectory should become a feedback loop | Motion database queries future trajectory; feasible selected motion constrains subsequent locomotion | M6 |
| Reactive interactions need more than two agents playing adjacent clips | M6 supplies a model-independent paired-motion request, deterministic worker baseline, validation, sparse layers, and fallback; M9 evaluates learned backends, while M8 owns combat semantics | M6 R0, M8 combat pack, M9 neural research |
| Physical interaction and crowd-to-physics transitions are production needs | Deterministic trigger and state handoff for ragdoll/rigid-body intervals, recoverable cache layers, collision masks, and resumption policy | M4/M6 |
| Cloth and hair are hero fidelity, not universal simulation state | Blender physics or validated external cache attaches to promoted hero agents without changing background truth | M6 |
| Post-simulation directing is the most valuable UX differentiator | Move, hide/delete, freeze, retime, redirect, replace appearance/animation, change speed, promote, or locally resimulate stable IDs | M2 foundation, M4 complete |
| Non-destructive crowd levels prevent full resimulation | Ordered base, layout, animation-fix, hero, physics, and shot layers with visible conflicts and provenance | M4 |
| USD should model crowd composition, not only flattened export | Population, simulation, layout, animation, physics, and shot opinions are exportable through a validated composition profile | M4 |
| General Blender USD support may not cover all crowd cases | A dedicated writer is allowed behind the public cache/IR; consumer and round-trip tests decide support claims | M4 |
| Procedural rendering is required for large scenes | Cache stores lightweight transforms, variants, clip/phase, materials, and LOD; render extraction instances prototypes without scene expansion | M4/M5 |
| Viewport optimization is a product capability | Proxy tiers, culling, bounded debug data, streaming, and camera/focus scheduling preserve editing responsiveness | M3/M5 |
| Multi-resolution agents make mixed shots feasible | Explicit point, capsule, skeleton, full-character, and hero presentation/physics policies share one stable identity | M2 foundation, M5/M6 complete |
| 50K–100K scale must be earned | Publish tier mix, scene, hardware, quality, memory, cache, playback, and render evidence at 10K before a 100K gate | M5 |
| GPU simulation can differentiate the project | Benchmark shared-flow, spatial-query, perception, steering, and simple behavior kernels behind backend-neutral contracts | M5 |
| Open APIs are a strategic advantage | Versioned Rust/Python/cache/IR/GN contracts, headless use, examples, compatibility policy, external behavior extension points, and a stable C ABI/C++ wrapper if native studio demand is proven | M1-M7 |
| Semantic AI can assist without controlling feet | Natural-language/VLM input proposes agenda, semantics, graph, or population changes as a reviewed diff compiled to deterministic IR | M8 |
| Synthetic-data crowds are a separate market | Versioned ground truth includes identity, pose, boxes, segmentation, velocity, intent, group, occlusion, and sensor provenance | M8 |
| Domain breadth should reuse a stable core | Traffic, combat, evacuation, retail, robotics, and surveillance packs share core identity, semantics, brain, motion, cache, and metrics | M8 |
| Blender integration requires evidence and governance | Stable host-facing contracts, clean packaging, performance profiles, design docs, tests, licensing, support commitment, and narrowly scoped upstream proposals | M7 |

## Required artist workflows

### Author and populate

- Create reusable character/entity types and weighted appearance sets.
- Scatter by region, fill stadium seating, follow curves/lanes, author formations,
  and bring validated custom characters.
- Inspect or override the stable random choices of an individual agent.

### Build the world

- Generate and update tiled navigation from Blender geometry.
- Paint costs and preferred/forbidden zones.
- Author doors, portals, stairs, escalators, queues, lanes, seats, resources,
  flow fields, timed closures, and capacity constraints.

### Author agency

- Compose state machines, utility decisions, and behavior-tree flows in a typed
  node editor.
- Read vision, hearing, contact, group, density, flow, environment, and arbitrary
  typed blackboard variables.
- Trace why a goal/action won and reproduce that result from the same inputs.

### Animate and interact

- Validate rigs and clips, then blend idle/start/walk/run/turn/stop states.
- Adapt stride and feet to trajectory and terrain.
- Promote selected agents into trajectory-aware motion matching, paired actions,
  IK, ragdoll, rigid body, cloth, hair, or hero-control layers.
- For promoted interaction groups, optionally generate and validate reactive
  paired motion in a local worker, then direct or remove the sparse animation
  layer without changing the base crowd bake.

### Simulate, direct, and render

- Preview and bake with deterministic IDs, cancel/recover, and inspect metrics.
- Correct individual agents or regions in sparse layers without modifying the
  base simulation.
- Stream cache playback, render prototypes procedurally, and exchange validated
  layered data with USD-capable consumers.

## Fidelity model

The survey's five conceptual levels map onto two independent policies:

| Conceptual level | Simulation representation | Presentation |
|---|---|---|
| Level 0 | Point/flow participant | Point or hidden |
| Level 1 | Capsule with avoidance | Proxy/billboard/low LOD |
| Level 2 | Individual agent with clip state | Skeleton or animated instance |
| Level 3 | Full trajectory and contacts | Full character with terrain/foot IK |
| Level 4 | Hero interaction/physics state | Facial, cloth, hair, rigid body, or authored rig |

Tier changes occur at deterministic boundaries with hysteresis. Camera distance
can influence presentation and scheduled fidelity but cannot silently change an
agent's stable identity, authored intent, cached root trajectory, or required
interaction state.

## Release flow

| Milestone | Product outcome | Industrial emphasis |
|---|---|---|
| [M0](milestones/M0-proving-grounds.md) | Measured deterministic kernel, navigation and cache/Blender bridge proof | Technical feasibility |
| [M1](milestones/M1-vertical-slice.md) | 1,000-agent end-to-end concourse | Dependable simulation/cache baseline |
| [M2](milestones/M2-authorable-mvp.md) | Artist-authored behavior, groups, semantics, variation, and sparse fixes | Initial MASSIVE/Golaem workflow foundation |
| [M3](milestones/M3-production-1.0.md) | Installable, recoverable, supported Blender Crowd 1.0 | Production trust |
| [M4](milestones/M4-layout-interchange.md) | Full layered directing, physics handoff, procedural extraction, and USD | Golaem-class layout/interchange |
| [M5](milestones/M5-scale-rendering.md) | Earned 10K then 100K tiers and GPU/procedural presentation | Industrial scale |
| [M6](milestones/M6-advanced-agency-motion.md) | Multi-sensory brains, activities, motion matching, interactions, and hero physics | MASSIVE-class agency and modern motion |
| [M7](milestones/M7-blender-integration.md) | Ecosystem adoption evidence and narrowly scoped mainline proposals | Blender integration readiness |
| [M8](milestones/M8-semantic-domains-data.md) | Reviewed semantic AI, domain packs, and synthetic-data outputs | Expansion tracks |
| [M9](milestones/M9-neural-animation-operator-validation.md) | Optional neural animation plus independent operator verification | Learned-motion and human-evidence promotion |

M0 through M3 are ordered release gates. M4 through M6 depend on the stable 1.0
data/cache contracts and may use separate implementation tracks only when their
shared schema changes are coordinated. M7 begins evidence collection at M0 but
cannot pass until production adoption exists. M8 follows the stable semantic,
motion, and interchange contracts it consumes. M9 starts only after M6
acceptance; its combat research also depends on the relevant M8 domain pack.

## Blender ecosystem and mainline strategy

The intended path mirrors successful DCC ecosystem integration in ambition, not
in assumed corporate outcome:

1. Ship and validate an extension with no private Blender fork requirement.
2. Keep simulation truth independent from Blender RNA and the dependency graph.
3. Publish versioned schemas, performance evidence, failure cases, and support
   policy.
4. Identify generally useful host gaps from production evidence—for example,
   point-instancer animation interchange, cache streaming, or viewport hooks.
5. Write Blender design proposals that separate general host capability from
   Blender Crowd product policy.
6. Submit small reviewable patches only where maintainers agree the capability
   belongs in Blender.
7. Preserve a supported extension path for code or policy that does not belong
   in mainline.

The project must not make mainline-only assumptions, claim endorsement, or use
upstream acceptance as a substitute for user-facing production proof.

## Proof policy

- A schema or synthetic unit test proves a contract, not production behavior.
- A local benchmark proves only its recorded machine, scene, tier mix, and build.
- A valid USD file proves serialization, not round-trip fidelity or consumer use.
- A rendered demo proves appearance, not determinism, navigation quality,
  recoverability, or scale.
- A GPU prototype proves one backend, not cross-platform support.
- A proposed Blender patch proves neither maintainer acceptance nor ecosystem
  adoption.

Every public parity or scale claim must link to the relevant milestone report,
test scene, environment, thresholds, and remaining limitations.
