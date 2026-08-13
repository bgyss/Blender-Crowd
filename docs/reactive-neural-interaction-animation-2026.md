# Reactive neural interaction animation research track

Status: post-1.0 research and stretch-goal contract
Evidence reviewed: primary papers, project pages, and released repositories
available through August 12, 2026
Owning milestones: [M6 advanced agency and motion](milestones/M6-advanced-agency-motion.md),
with combat semantics and assets in [M8 domain packs](milestones/M8-semantic-domains-data.md)

## Decision

Blender Crowd should investigate neural motion generation for promoted interaction
groups, beginning with two characters. The goal is not a text prompt that emits a
finished fight clip. It is a reactive motion layer in which each participant can
respond to the other's recent motion while respecting authored intent, root paths,
contacts, props, outcomes, and shot direction.

This is a future M6 research track, not a Blender Crowd 1.0 feature and not a
commitment to one model. [ARDY](https://research.nvidia.com/labs/sil/projects/ardy/)
is the preferred first single-character integration candidate because its streaming
interface accepts online text and sparse, long-horizon kinematic constraints. It is
not yet a paired-interaction solution: the published system generates one humanoid
stream and does not expose another character's state, inter-character contacts, or
weapon dynamics as native conditioning.

The durable product decision is therefore:

1. keep Blender Crowd authoritative for perception, action choice, interaction
   roles, root-space feasibility, contact intent, outcome, and event timing;
2. pass a bounded, versioned interaction request to an optional local motion worker;
3. let ARDY, an ARDY-derived paired model, or another backend propose skeletal
   motion for the whole interaction group;
4. validate and bake the result as a sparse animation layer with complete model,
   data, seed, and constraint provenance; and
5. fall back to motion matching or authored paired clips whenever generation,
   validation, licensing, hardware, or reproducibility requirements are not met.

This is “generative motion behind stable contracts,” not a learned model taking
ownership of the crowd simulation.

### Relationship to the existing ARDY project

This proposal integrates the existing ARDY workstream as a sibling subsystem; it
does not copy ARDY training or model code into Blender Crowd.

- ARDY owns model architecture, checkpoints, training and domain adaptation,
  Core-skeleton representation, inference, and model-side evaluation.
- Blender Crowd owns interaction intent, participant identity, root feasibility,
  request/response schemas, retarget profiles, contact/outcome validation, sparse
  cache layers, Blender UX, and deterministic fallback.
- Shared conformance fixtures are versioned in both projects and identified by
  content hash. Neither project reaches into the other's source checkout or
  depends on a machine-local path.
- A released worker protocol or package version is the integration point. ARDY
  can evolve independently, and Blender Crowd can compare or replace the backend
  without migrating shot data to model-specific tensors.

## Why this is worth pursuing

Traditional crowd animation can make two combatants arrive near one another and
select attack, defend, hit, or fall clips. It usually cannot make the visible
performance respond continuously to spacing, timing, feints, balance, the other
character's actual pose, or an interrupted contact. The result reads as two agents
playing animations near each other rather than sharing an encounter.

A paired generative layer could improve:

- causal reactions such as evade, parry, brace, counter, stumble, catch, push,
  help up, restrain, or recover;
- variation within a fixed authored action and outcome instead of repeating one
  synchronized clip pair;
- continuity between locomotion, engagement, contact, reaction, and separation;
- adaptation to actual roots, terrain, body proportions, weapon reach, and sparse
  director keyframes; and
- background cooking: expensive motion may be generated after the deterministic
  crowd bake, reviewed, corrected, and reused without resimulating unrelated agents.

The model still cannot decide whether a sword strike lands, whether an agent is
injured, or whether a push changes navigation state. Those are inspectable crowd
and domain rules. The model expresses an already bounded interaction and may
report infeasibility; it does not invent authoritative consequences.

## Evidence assessment

| Candidate or precedent | What it demonstrates | Fit for Blender Crowd | Material limitation |
|---|---|---|---|
| [ARDY, SIGGRAPH 2026](https://research.nvidia.com/labs/sil/projects/ardy/) | Streaming autoregressive diffusion with online text, root paths and waypoints, full-body keyframes, and sparse joint position/rotation constraints | Best first worker for one promoted agent, root-path following, constrained transitions, and measuring the integration boundary | Published conditioning is single-character; no native partner, paired-contact, weapon, or interaction-outcome model |
| [Ready-to-React, ICLR 2025](https://arxiv.org/abs/2502.20370) | Separate online reaction policies generate each next pose from both characters' observed history; the paper evaluates boxing and long streaming sequences | Closest architectural precedent for genuinely reactive combat and for testing independent-agent causality | Boxing-specific evidence and a separate older Python/CUDA stack; repository and DuoBox data terms require a full license review before reuse |
| [ReMoS, ECCV 2024](https://www.dfki.de/web/forschung/projekte-publikationen/publikation/15173) | Generates a second person's reactive full-body and hand motion from the first person's complete motion using spatiotemporal cross-attention | Useful reaction baseline and contact/hand representation reference for Ninjutsu, kickboxing, dance, and acrobatics | Follower-from-complete-sequence formulation is an offline editing baseline, not two independent online agents |
| [Human-X Interaction, ICCV 2025](https://github.com/humanx-interaction/Human-X-Interaction) | Pairs an autoregressive reaction diffusion planner with physics-based tracking and reports real-time human-agent interaction | Strong precedent for separating high-level motion planning from physical tracking and contact correction | As of its March 2026 release plan, the diffusion planner is available but capture and Isaac Gym tracker modules are still marked coming soon; its simulator stack is not a Blender integration |
| [InterGen, IJCV 2024](https://tr3e.github.io/intergen-page/) | Jointly generates two-person motions from text with explicit world-space relations; examples include boxing and fencing | Good offline joint-choreography baseline and source of evaluation ideas | Not an online reaction policy; released code and InterHuman data are CC BY-NC-SA and therefore not a shippable commercial dependency |
| [CG-HOI, CVPR 2024](https://openaccess.thecvf.com/content/CVPR2024/html/Diller_CG-HOI_Contact-Guided_3D_Human-Object_Interaction_Generation_CVPR_2024_paper.html) | Joint diffusion over human motion, object motion, and explicit contact improves their coherence | The explicit-contact lesson is directly relevant to swords, shields, props, doors, and furniture | Human-object rather than adversarial human-human generation; does not provide crowd runtime behavior or combat rules |
| [ActFormer, ICCV 2023](https://liangxuy.github.io/actformer/) | Alternates temporal and interaction transformers and introduced roughly 7,000 synthetic two-to-five-person combat sequences | A multi-person combat representation and stress-data precedent | Action-conditioned generation is not reactive; GTA-derived data has provenance and redistribution questions that must be resolved before use |
| [AnimationGPT / CombatMotion](https://github.com/fyyakaxyy/AnimationGPT) | Released a text-annotated combat-motion pipeline with 8,700 processed single-character game-animation entries | Potential vocabulary and solo attack/defend motion baseline after an asset-by-asset rights audit | Mostly isolated game animations, not paired causality; repository code licensing does not establish rights to every source animation or trained artifact |

ARDY's official code is Apache-2.0, but its checkpoints and datasets have
separate terms. Every candidate must pass a dependency, model, checkpoint, and
training-data review; a permissive code license alone is insufficient.

## Proposed architecture

### Authoritative interaction contract

The Rust core compiles a bounded request at a deterministic tick boundary:

```text
interaction_request_v1
  request_id, group_id, participant stable IDs, roles
  frame/tick range, fixed seed, strict or exploratory mode
  semantic action and authored outcome
  root trajectories/waypoints and facing envelopes
  participant skeleton/retarget profiles and proportions
  body, hand, weapon, prop, and environment constraints
  desired and forbidden contact windows
  terrain/support surfaces, collision volumes, joint limits
  source cache hash, graph/IR hash, worker/model/checkpoint hashes
  latency, memory, fidelity, and retry budgets
```

Examples of authored outcomes are `parry_then_separate`, `push_then_fall`,
`strike_blocked`, `strike_lands_left_arm`, or `grapple_break`. They remain domain
events even when the generated performance varies.

### Optional local worker

The first integration should be an out-of-process local worker, not a model
embedded in Blender's Python process or the deterministic Rust hot loop. A worker
can use CUDA or another accelerator, crash independently, retain model state, and
stream preview chunks while Blender remains responsive. Blender Crowd owns the
request and response schema rather than importing a candidate model's internal
tensor layout.

The worker returns:

```text
interaction_motion_v1
  participant local rotations and proposed root deltas
  contact labels/confidence and support state
  generation windows and continuity metadata
  model/checkpoint/config/seed/runtime provenance
  warnings, unsatisfied constraints, and failure reason
```

It may return several ranked variations. It must never directly mutate the base
cache, change an outcome, or write Blender data from a background thread.

### Validation and composition

Before acceptance, native validators check:

- skeleton mapping, joint limits, discontinuities, self-penetration, pair
  penetration, environment penetration, balance/support, and prop attachment;
- foot slide and support consistency, root-path/facing error, phase continuity,
  and transition quality at layer boundaries;
- contact timing, position, orientation, relative velocity, weapon reach, and
  whether required or forbidden contacts occurred;
- agreement with the authored outcome and preservation of all unrelated agents;
  and
- declared time, memory, hardware, determinism, and cache-size budgets.

Accepted motion becomes a sparse M4-compatible animation layer. The base crowd
cache remains usable without the model. Rejected output records its diagnostics
and either retries within a fixed budget or selects a deterministic fallback.

### Fidelity policy

Neural motion is never a requirement for the entire crowd:

| Tier | Animation policy | Intended use |
|---|---|---|
| Background | Existing clip state/phase | Large numbers and distant encounters |
| Mid | Motion matching, warping, short paired clips | Readable but non-hero interactions |
| Promoted interaction | Validated paired neural animation layer | Close or narratively important two-character encounters |
| Hero | Neural proposal plus authored corrections, IK, and optional physics | Contacts requiring art direction or physical follow-through |

Promotion is deterministic and authored or budget-driven. Once an interaction
starts, both participants are scheduled as one group so distance-based LOD cannot
silently demote only one side or remove required contact state.

## ARDY integration hypothesis

The first ARDY experiment should reuse its public boundary rather than fork the
crowd core around it:

1. map a canonical Blender Crowd humanoid profile to ARDY's Core skeleton;
2. translate the cached root trajectory, action label, boundary keyframes, and
   end-effector targets into ARDY constraints;
3. generate one agent's motion in a worker and retarget it back to the canonical
   Blender profile;
4. validate it against the same clip-state baseline; and
5. bake only the accepted skeletal channels as an animation-fix layer.

That experiment proves packaging, skeleton conversion, temporal alignment,
constraint fidelity, cache layering, and Blender playback. It does **not** prove
reactive interaction.

A later paired ARDY research branch may test dual motion streams with relative-
pose features, cross-character attention, and explicit body/weapon contact tokens.
Ready-to-React supplies the stronger online-policy precedent; InterGen and ReMoS
supply offline joint/follower baselines. The project should select an approach by
measured acceptance results rather than preserving ARDY compatibility at any cost.

## Staged research gates

### R0 — Contract and clip baseline

- Define `interaction_request_v1` and `interaction_motion_v1` without a model.
- Run an authored paired-clip worker through request, validation, cache layer,
  reload, correction, removal, and fallback.
- Record penetration, contact error, foot slide, root deviation, transition
  discontinuity, bake time, playback cost, and cache size.

Exit: the model-independent boundary composes with M4 layers and cannot alter an
authoritative outcome or unrelated agent.

### R1 — ARDY single-character constrained motion

- Test locomotion-to-action-to-recovery, a shove reaction, a fall/recovery, and
  weapon-ready movement against clip-state and motion-matching baselines.
- Measure constraint adherence, retargeting error, latency, peak memory, quality,
  determinism/replay behavior, and Blender-side editability.
- Freeze exact code, checkpoint, configuration, skeleton map, and output hashes.

Exit: ARDY earns a backend slot only if it improves at least one predeclared
quality dimension without failing hard contact, root, stability, licensing, or
pipeline requirements. A show reel alone does not pass.

### R2 — Online two-character reaction

- Begin with approach/avoid, mirroring, handoff, and point-sparring at range.
- Progress to push-and-stumble, boxing attack/defend/counter, and interrupted
  actions with explicit desired and forbidden body contacts.
- Compare independent reaction policies, joint pair generation, and deterministic
  paired clips under the same initial conditions and outcomes.

Exit: held-out perturbations demonstrate bounded reaction latency, long-horizon
stability, role/outcome fidelity, valid contact, and better blinded preference
than the clip baseline. Both agents must remain independently traceable.

### R3 — Weapon interaction and combat domain pack

- Add weapon geometry, grip constraints, reach, swept-volume checks, blade/blunt
  contact types, parry/block/hit timing, disarm/drop, and safe fallback poses.
- Prove at least `attack-parry-separate`, `attack-evade-counter`, and
  `strike-land-reaction-recovery` with reversible outcomes.
- Package semantics, actions, metrics, reference scenes, assets, and limitations
  through M8 rather than hard-coding combat into the core.

Exit: independent reviewers can distinguish intended contacts/outcomes, correct
the result without regenerating the full crowd, and reproduce the accepted cache
without the model present. This is entertainment animation evidence, not a claim
of martial, injury, or physical simulation accuracy.

### R4 — Scheduled scale and production study

- Measure concurrent generation for one, two, four, and sixteen interaction
  groups, queueing and cancellation, GPU memory, worker crashes, model reload,
  cache reuse, and promotion/demotion behavior.
- Run an independent artist study covering discovery, art direction, rejection,
  correction, fallback, and render-from-cache.

Exit: the system has a declared supported tier mix and production workflow. No
result may be extrapolated to every crowd agent or unsupported hardware.

## Acceptance metrics

Thresholds must be set from checked-in baselines before implementation. At
minimum, every dated report records:

| Category | Measures |
|---|---|
| Intent and causality | role accuracy, outcome agreement, reaction latency, interrupt response, counterfactual partner perturbations |
| Contact and physics | required/forbidden contact precision, contact distance/orientation/relative velocity, penetration depth/time, support violations, weapon reach |
| Motion quality | foot slide, jerk, joint-limit violations, root/facing deviation, boundary discontinuity, fall/recovery success |
| Production control | seed/model/config provenance, accepted-output hash, retry/fallback rate, localized correction time, unrelated-agent isolation |
| Cost | generation latency, throughput by concurrent pair count, peak CPU/GPU memory, cache bytes per character-second, Blender playback overhead |
| Perception | blinded preference against paired clips and motion matching, with protocol and failures published |

The tests include unseen heights/proportions, mirrored roles, different approach
angles and speeds, late interrupts, missing props, impossible contacts, narrow
terrain, partial worker output, unsupported hardware, and corrupted model or cache
artifacts.

## Reproducibility modes

Bit-identical neural inference across devices is not assumed.

- **Strict bake:** use an approved frozen model/device path only if repeated runs
  meet the declared tolerance or hash requirement; otherwise use the deterministic
  fallback. Once accepted, the baked layer is content-addressed and replay is exact.
- **Exploratory bake:** generation may vary, but all inputs, model artifacts,
  outputs, and review decisions are recorded. It cannot satisfy a strict gate.
- **Model-absent playback:** every accepted shot must load, render, layer, and
  export from cache without the model, worker, or accelerator.

## Stop conditions

Stop or fall back if:

- the model controls authoritative intent, collision safety, damage, or outcome;
- paired motion cannot be debugged as two stable agents plus a shared interaction;
- a backend needs unlicensed or provenance-unclear motion, model, or game assets;
- output penetrates bodies, weapons, or the environment and validation cannot
  repair or reject it reliably;
- a model worker becomes a render-time or Blender-file dependency;
- quality is supported only by cherry-picked video; or
- promoted interactions destabilize, mutate, or invalidate unrelated base-cache
  agents.

## Source notes

- [ARDY project and paper](https://research.nvidia.com/labs/sil/projects/ardy/),
  NVIDIA and ETH Zurich, ACM TOG / SIGGRAPH 2026. The
  [official repository](https://github.com/nv-tlabs/ardy) releases code under
  Apache-2.0 and explicitly separates checkpoint and dataset licenses.
- [Ready-to-React paper](https://arxiv.org/abs/2502.20370) and
  [official repository](https://github.com/zju3dv/ready_to_react), ICLR 2025.
- [ReMoS publication record](https://www.dfki.de/web/forschung/projekte-publikationen/publikation/15173),
  ECCV 2024.
- [Human-X Interaction repository](https://github.com/humanx-interaction/Human-X-Interaction),
  ICCV 2025; the repository is MIT-licensed but its external datasets and model
  dependencies retain separate terms.
- [InterGen project](https://tr3e.github.io/intergen-page/) and
  [repository](https://github.com/tr3e/InterGen), IJCV 2024.
- [CG-HOI open-access paper](https://openaccess.thecvf.com/content/CVPR2024/html/Diller_CG-HOI_Contact-Guided_3D_Human-Object_Interaction_Generation_CVPR_2024_paper.html),
  CVPR 2024.
- [ActFormer project](https://liangxuy.github.io/actformer/), ICCV 2023.
- [AnimationGPT repository](https://github.com/fyyakaxyy/AnimationGPT), including
  the CombatMotion dataset description.
