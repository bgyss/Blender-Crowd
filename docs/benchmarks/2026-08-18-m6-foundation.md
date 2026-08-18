# M6 deterministic foundation evidence — 2026-08-18

## Result

The M6 deterministic foundation and the model-independent R0 interaction gate
pass in this checkout. This report is deliberately not an M6 acceptance claim:
the full milestone still requires production motion/terrain/physics evidence,
licensed deterministic motion-data review, scale measurements, complete
reference scenes, and a requirement-level acceptance report. Model-backed
R1–R4 research and independent-user verification are deferred to M9.

## Environment and scope

- Date: 2026-08-18
- Scope: local Rust/Python contract, runtime, cache-layer, worker, and debugger
  foundation
- Motion inputs: checked-in redistributable metadata and authored paired-clip
  fixtures only; no external or unlicensed motion data
- Worker: `m6-interaction-worker`, deterministic and model/accelerator-free
- Runtime boundary: Rust owns roles, roots, contacts, outcomes, event timing,
  validation, and fallback; Blender consumes coarse summaries and layers

## Verified commands

| Command | Result | Evidence |
|---|---|---|
| `scripts/m6-foundation-test.sh` | PASS | Versioned fixtures, interaction validator, typed perception/brain/activity/motion/physics/extensions, local quality metrics, cache layers, worker, and Python debugger tests all passed |
| `cargo fmt --all -- --check` | PASS | Rust formatting is clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Current workspace is warning-clean |
| `cargo test --workspace` | INCOMPLETE | The current run passed benchmark/cache/Blender-native/core unit/authoring/determinism suites, then was interrupted during the repository's long `fuzz_density` lane; no full-workspace pass is claimed |
| `cargo test --release -p crowd-core --test fuzz_density` | PASS | All 4 prescribed density-fuzz tests passed in the optimized release lane (507.46s) |
| `cargo test --workspace --release` | PASS | Full optimized workspace suite passed, including all non-ignored M6 tests, doc tests, and the density-fuzz lane; intentionally ignored long M1/M2/M5 acceptance tests remain separate gates |
| `scripts/m6-blender-test.sh` | PASS | Blender 5.2.0 LTS loaded the current source add-on and freshly built abi3 native wheel with normal host Metal access; trace inspection and graph search passed. A restricted-sandbox launch reproduced the pre-Python Metal abort, and a host launch against the stale installed extension reproduced the old-package `ImportError`, confirming both runner safeguards are necessary |
| `scripts/m6-acceptance.sh` | OPEN BY DESIGN | The audit passes the deterministic foundation and names the remaining bidirectional-debugger, reusable-subgraph/preset, reference-scene, licensed-motion/terrain, Blender physics/hero, extension-example, mixed-tier performance, acceptance-report, optional Blender, and debug-workspace gates; it identifies neural and independent-user work as deferred to M9 and exits nonzero unless `M6_ALLOW_OPEN=1` is explicitly set |
| `python3 -m unittest -q tests/test_m6_extensions.py tests/test_m6_interaction_layers.py tests/test_m6_debugger.py tests/test_m6_motion_database.py tests/test_m6_motion_evaluation.py tests/test_m6_physics_boundaries.py` | PASS | 23 pure-Python extension, persistence, debugger, motion-build/evaluation, and physics-boundary tests passed |
| `python3 -m py_compile scripts/m6_motion_build.py scripts/m6_motion_evaluate.py addon/blender_crowd/m6_debugger.py addon/blender_crowd/m6_extensions.py addon/blender_crowd/m6_interaction.py addon/blender_crowd/m6_physics.py addon/blender_crowd/operators.py addon/blender_crowd/panels.py addon/blender_crowd/properties.py` | PASS | Blender-facing Python syntax checked without requiring a Blender process |
| `git diff --check` | PASS | No whitespace errors |

## Requirement-level status

| M6 area | Current evidence | Status boundary |
|---|---|---|
| Typed perception | Stable ordering, wall occlusion, touch, hearing, density/flow, group extent, semantic distance, bounded memory, and budget degradation tests | Local deterministic foundation; no production perception budget study |
| Brain authoring/runtime | Typed blackboard, fixed-point fuzzy predicates, utility scores, reusable named actions, interrupts, reserve actions, and perception-to-trace evidence | Local deterministic runtime evidence; independent authoring claims belong to M9 |
| Activities | Stable priority/agent admission, finite capacity, release/promotion, and live reserve trace events | Reference resource runtime; no full schedule/needs scene report yet |
| Groups/formations | Stable roles, offsets, cohesion, split, missing-member, and intrusion reports; optional runtime attachment | Reference formation evidence; no blinded social readability comparison |
| Local quality comparisons | Fixed-point group readability and motion-quality comparisons cover split/intrusion, feasibility/fallback, root deviation, foot slide, contacts, and transition discontinuity | Fixture-level baseline comparison only; no production threshold or human preference claim |
| Acceptance scenes | Versioned activity, formation, terrain/foot-lock, paired-interaction, recovery, retarget, hero-boundary, motion-provenance, and mixed-tier fixtures | Fixture/schema coverage only; no measured scene acceptance report yet |
| Motion | Versioned metadata database, canonical retarget provenance, deterministic build/evaluation hashes, fitted offline profile metrics, future-trajectory/contact matching, bounded stride/turn warp diagnostics, promoted S0 clip selection, root/foot/slope feedback, and fixed-point quality comparisons | Foundation only; no licensed production database, IK/terrain threshold report, or performance claim |
| Interaction R0 | Versioned request/response schemas, strict validation, deterministic paired baseline, atomic group promotion/locking, out-of-process worker, fallback provenance, removable layer, cross-cache isolation, and model-absent response generation | R0 local exit evidence passes |
| Physics/hero boundary | Versioned transition and recovery declarations with solver/cache/failure ownership | Boundary validation only; no Blender physics/cloth/hair acceptance |
| Extensions | Versioned channels, declared inputs/outputs, cost budgets, deterministic mode, failure isolation, and matching Rust/Python facades | Contract tests pass; no external studio API compatibility claim |
| Debugger | Pure trace summary/search model, Blender properties/panels/operators, trace-to-node path, reduced-evidence text, and passing Blender 5.2 source-add-on smoke | Trace/search smoke passes; bidirectional navigation and reusable subgraphs/actions/presets remain open in M6, while independent-user claims belong to M9 |

## R0 acceptance mapping

The R0 path exercises request, authored paired response, validation, cache-layer
composition, reload/serialization, explicit fallback, cross-cache rejection,
unrelated-agent isolation, and worker/model absence. The invalid fixture proves
forbidden contact/root deviation rejection. The accepted output is a sparse
animation-layer artifact and never changes an authoritative interaction outcome.

## Open gates and unsupported claims

- R1 ARDY integration, R2 online two-character reaction, R3 weapon/combat
  domain-pack proof, and R4 concurrent production study were not run and are
  explicitly deferred to M9; they do not block M6.
- No neural checkpoint, accelerator, cloud service, or external motion corpus was
  used or authorized by this task.
- No claim is made for production-scale motion matching, foot locking/IK,
  terrain adaptation, cloth/hair/facial simulation, or physical combat accuracy.
- Independent-user authoring and repair evidence is deferred to M9. The
  copy-ready [M9 study protocol](../user/m9-debugger-study.md) preserves the
  tasks, timings, and degraded-tier review requirements.
- Full M6 completion remains open until every acceptance criterion in the M6
  contract has requirement-level evidence.

## Next gate

Build the complete M6 reference scenes and acceptance runner around the verified
contracts: schedule/needs activities, group formations, terrain/contact motion
fixtures, physics recovery, mixed-tier diagnostics, licensed deterministic
motion inputs, and measured thresholds. Keep M9 R1–R4 and participant-study work
inert until their separate authorization and provenance requirements are met.
