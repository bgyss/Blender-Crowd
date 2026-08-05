# M8 — Semantic authoring, domain packs, and synthetic data

## Objective

Expand the stable crowd framework into reviewed semantic authoring, reusable
domain packs, and validated synthetic-data generation without allowing learned
systems or domain shortcuts to weaken deterministic simulation truth.

## Sources of truth

- [Industrial capability roadmap strategic conclusions](../industrial-crowd-capability-roadmap.md#strategic-conclusions)
- [Blender Crowd 1.0 post-1.0 roadmap](../blender-crowd-1.0.md#17-post-10-roadmap-ordering)
- Accepted M4 interchange and M6 agency/motion contracts consumed by each track

## Prerequisites

M3 is accepted. A track starts only when its required M4/M5/M6 schemas and
quality gates are stable. Models, datasets, characters, motions, maps, and
consumer benchmarks require documented licenses and provenance.

## Track A — Reviewed semantic authoring

- Convert high-level intent such as “visit a shop, queue, buy, then sit with two
  friends” into a proposed agenda, semantic objects, population changes, and
  behavior graph diff.
- Validate the proposal against typed node, semantic, asset, resource, and
  determinism contracts before the artist can accept it.
- Store prompt/model/version/proposal provenance as authoring metadata, never as
  a runtime requirement for an accepted deterministic bake.
- Permit local rules/templates without an AI dependency and support a fully
  manual review/edit path.

Track A passes when independent prompts produce reviewable bounded diffs, invalid
or unsafe references are rejected, accepted output compiles to deterministic IR,
and the same accepted project bakes without the model present.

## Track B — Domain packs

Candidate packs include traffic, combat, evacuation, retail/city needs,
surveillance, robotics, autonomous-vehicle pedestrian scenarios, and non-human
flock/herd/swarm behaviors. Each pack
defines only its semantics, actions/interactions, assets, metrics, and reference
scenes; it reuses core identity, perception, brain, navigation, motion, cache,
layout, and interchange contracts.

Each pack must have domain-specific safety/quality metrics and failure cases. A
combat reel, city-needs demo, or evacuation animation does not establish physical,
engineering, or policy validity outside the stated simulation model.

Track B passes per pack when its versioned extension data composes with a clean
core install, reference scenes and headless tests pass, core behavior does not
fork silently, and limitations are documented for the intended use.

## Track C — Synthetic-data crowds

Versioned ground truth may include:

```text
agent_id, population_id, group_id, intent, behavior/action state
world/camera pose, skeleton joints, velocity, acceleration
2D/3D bounding boxes, instance/class segmentation, depth, optical flow
visibility, occlusion, contact, render/simulation tier
camera/sensor model, scene seed, asset and cache provenance
```

The track includes sensor/camera configuration, deterministic annotation,
occlusion/visibility definitions, dataset splits, asset/license filters, export
formats, and downstream validation. Demographic or safety-sensitive labels need
an explicit definition, provenance, bias review, and lawful use; they are not
inferred casually from character appearance.

Track C passes when rendered samples and annotations reproduce from the recorded
project/cache/environment, geometric labels agree with independent checks, data
formats load in the declared consumers, licensing permits the published use, and
a downstream benchmark demonstrates the dataset is usable for its stated task.

## Shared exclusions

- No LLM/VLM directly controls per-frame feet, avoidance, contacts, or safety.
- No accepted project requires a cloud model or service at simulation/render
  time unless a later explicit product contract authorizes that dependency.
- No generative output bypasses asset licensing, graph validation, human review,
  or cache provenance.
- No synthetic dataset is described as representative, unbiased, or suitable
  for a safety-critical use solely because it has perfect engine ground truth.

## Required artifacts

- Track-specific versioned schemas, validators, audit/provenance logs, fixtures,
  headless runners, user guides, threat/failure analysis, and dated reports.
- For semantic authoring: proposal/diff UI, deterministic compiler boundary, and
  model-absent replay fixtures.
- For each domain pack: manifest, semantic/action library, assets, metrics,
  reference scenes, and compatibility declaration.
- For synthetic data: annotation spec, sensor models, license manifest, sample
  dataset, independent label checks, and downstream evaluation.

## Validation and proof

Use graph/schema golden tests, adversarial semantic proposals, model-absent bake
tests, core/pack compatibility tests, randomized domain failure scenarios,
annotation geometry/unit tests, render reproducibility, format consumer tests,
license audits, bias/coverage reports where relevant, and a downstream dataset
benchmark. Paid models, cloud generation, or external publication require
explicit authorization.

## Definition of done and stop conditions

M8 is a collection of independently accepted tracks, not one all-or-nothing
release. A track is done only when its own criteria and evidence report pass.
Stop on unreviewable model output, runtime nondeterminism, core-contract forks,
unclear asset/data rights, annotation mismatch, unsupported safety claims, or a
required unapproved external service.
