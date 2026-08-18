# M6 Advanced Agency and Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the full M6 advanced-agency, trajectory-aware-motion, interaction, physics-boundary, and debugger contract while preserving deterministic lower-fidelity playback.

**Architecture:** Add versioned Rust-owned M6 contracts and deterministic runtimes, compose accepted promoted motion as removable adjacent layers, and expose the resulting trace and diagnostics through coarse Blender operators/panels. The model-independent R0 interaction boundary is the first gate; later motion and physics capabilities build on the same validator, provenance, fallback, and cache-isolation rules.

**Tech Stack:** Rust 2021 workspace, serde/serde_json, existing crowd-core/crowd-cache/crowd-bench crates, Blender Python add-on, JSON Schema, Python unittest, and repository shell runners.

**Spec:** `docs/superpowers/specs/2026-08-18-m6-advanced-agency-motion-design.md`

**Scope note:** R1–R4 neural animation and independent-user verification moved
to `docs/milestones/M9-neural-animation-operator-validation.md`. M6 retains the
model-independent R0 boundary and automated Blender debugger proof.

## Global Constraints

- Preserve stable IDs, fixed-step simulation, deterministic event ordering, versioned schemas, and Rust/Python/Geometry Nodes ownership boundaries.
- Do not add per-agent per-frame LLM/VLM control, opaque runtime callbacks, unlicensed motion data, universal rig conversion, or a model/render-time dependency.
- Rust remains authoritative for intent, roots, contacts, outcomes, event timing, validation, and fallback; Python remains coarse-grained.
- Accepted promoted outputs are adjacent, content-addressed, editable, removable, and playable without a worker, model, or accelerator.
- Every behavior change has a test that fails before its implementation; performance claims require a checked-in fixture, runner, report, and environment.
- M8 owns semantic domain packs and combat meaning; M6 exposes generic roles, contacts, and extension points.
- M9 owns learned animation backends, perceptual claims, and independent-user verification.

---

### Task 1: Add the M6 contract package and golden fixtures

**Files:**
- Create: `schemas/perception-v1.schema.json`
- Create: `schemas/brain-v1.schema.json`
- Create: `schemas/activity-v1.schema.json`
- Create: `schemas/trajectory-v1.schema.json`
- Create: `schemas/contact-v1.schema.json`
- Create: `schemas/interaction-request-v1.schema.json`
- Create: `schemas/interaction-motion-v1.schema.json`
- Create: `schemas/physics-transition-v1.schema.json`
- Create: `assets/reference/m6/README.md`
- Create: `assets/reference/m6/interaction-request-v1.json`
- Create: `assets/reference/m6/interaction-motion-v1.json`
- Create: `assets/reference/m6/interaction-motion-invalid-penetration-v1.json`
- Create: `assets/reference/m6/trajectory-database-v1.json`
- Create: `crates/crowd-core/tests/m6_schema_fixtures.rs`

**Interfaces:**
- Produces schema IDs and fixture shapes consumed by every later M6 module.
- Every JSON object uses `schema_version`, `deny_unknown_fields`-compatible
  fields, stable IDs, explicit tick ranges, and content/provenance references.

- [x] **Step 1: Write the fixture-validation tests.**

```rust
#[test]
fn m6_fixtures_validate_against_their_versioned_schemas() {
    for (schema_path, fixture_path) in M6_FIXTURES {
        validate_json_schema(schema_path, fixture_path);
    }
}

#[test]
fn m6_unknown_fields_are_rejected() {
    assert_rejects_unknown_field("interaction-request-v1", "unexpected");
}
```

- [x] **Step 2: Run the focused tests and verify they fail because the schemas and fixtures are absent.**

Run: `cargo test -p crowd-core --test m6_schema_fixtures`

Expected: FAIL with a missing-file or missing-fixture error.

- [x] **Step 3: Add the schemas and fixtures with the exact fields defined in the M6 design.**

- [x] **Step 4: Run the focused tests and the JSON parser checks.**

Run: `cargo test -p crowd-core --test m6_schema_fixtures`

Expected: PASS with all fixtures accepted and the mutated unknown-field fixture rejected.

- [ ] **Step 5: Commit the contract package.**

```bash
git add schemas assets/reference/m6 crates/crowd-core/tests/m6_schema_fixtures.rs
git commit -m "Add M6 versioned contracts and golden fixtures"
```

### Task 2: Implement typed interaction request/response and deterministic R0 validation

**Files:**
- Create: `crates/crowd-core/src/interaction.rs`
- Modify: `crates/crowd-core/src/lib.rs`
- Create: `crates/crowd-core/tests/m6_interaction.rs`
- Create: `crates/crowd-core/tests/m6_interaction_invalid.rs`
- Modify: `crates/crowd-cache/src/lib.rs`
- Create: `crates/crowd-cache/src/interaction_layers.rs`
- Create: `crates/crowd-cache/tests/interaction_layers.rs`

**Interfaces:**
- `InteractionRequestV1::validate(&self) -> Result<(), InteractionValidationError>`
- `InteractionMotionV1::validate_against(&self) -> Result<(), Vec<InteractionIssue>>`
- `deterministic_paired_clip(request: &InteractionRequestV1) -> InteractionMotionV1`
- `compose_interaction_layer(base_cache_hash: &str, request: &InteractionRequestV1, motion: &InteractionMotionV1) -> Result<AnimationLayerV1, LayerError>`
- `AnimationLayerV1::fallback_reference()` identifies the deterministic clip path.

- [x] **Step 1: Add failing tests for strict request validation, stable participant ordering, and deterministic paired output.**

```rust
#[test]
fn paired_clip_output_is_identical_for_repeated_strict_requests() {
    let request = fixture_request();
    assert_eq!(deterministic_paired_clip(&request), deterministic_paired_clip(&request));
}

#[test]
fn motion_rejects_root_teleportation_and_forbidden_contact() {
    let request = fixture_request();
    let mut motion = fixture_motion();
    motion.participants[0].root_samples[1].translation_m = [100.0, 0.0, 0.0];
    motion.contacts[0].label = ContactLabel::Forbidden;
    let errors = motion.validate_against(&request).unwrap_err();
    assert!(errors.iter().any(|error| error.code == InteractionIssueCode::RootDeviation));
    assert!(errors.iter().any(|error| error.code == InteractionIssueCode::ForbiddenContact));
}
```

- [x] **Step 2: Run the focused tests and verify the missing API failure.**

Run: `cargo test -p crowd-core --test m6_interaction --test m6_interaction_invalid`

Expected: FAIL because the interaction types and validator do not exist.

- [x] **Step 3: Implement the serde types, fixed-step authored paired-clip adapter, validator, and structured issue codes.**

- [x] **Step 4: Run the focused tests and verify strict determinism, contact rules, root bounds, provenance, and fallback behavior.**

Run: `cargo test -p crowd-core --test m6_interaction --test m6_interaction_invalid`

Expected: PASS with no warnings or ignored tests.

- [ ] **Step 5: Commit the core R0 interaction implementation.**

```bash
git add crates/crowd-core/src crates/crowd-core/tests/m6_interaction*.rs
git commit -m "Implement deterministic M6 interaction validation"
```

### Task 3: Compose, persist, remove, and replay R0 animation layers

**Files:**
- Modify: `schemas/override-layer-v2.schema.json`
- Modify: `addon/blender_crowd/m4_layout.py`
- Create: `addon/blender_crowd/m6_interaction.py`
- Create: `tests/test_m6_interaction_layers.py`
- Modify: `crates/crowd-cache/src/manifest.rs`
- Modify: `schemas/cache-manifest-v1.schema.json`
- Modify: `crates/crowd-cache/tests/manifest_contract.rs`

**Interfaces:**
- `m6_interaction.write_layer(path, layer)` writes an atomic adjacent artifact.
- `m6_interaction.load_layer(path, expected_base_hash)` rejects cross-cache attachment.
- `m6_interaction.remove_layer(path, layer_id)` removes only the selected layer.
- `m6_interaction.fallback_layer(request, base_hash)` returns a deterministic clip-state layer.

- [x] **Step 1: Write failing Python and Rust tests for round-trip, base-hash isolation, mute/remove, and fallback.**

```python
def test_interaction_layer_round_trip_preserves_base_and_provenance():
    layer = m6_interaction.new_animation_layer("pair", "a" * 64, [7, 9], 10, 20)
    path = Path(directory) / "layers" / "interaction-pair-v1.json"
    m6_interaction.write_layer(path, layer)
    assert m6_interaction.load_layer(path, "a" * 64)["base_cache_hash"] == "a" * 64

def test_interaction_layer_rejects_another_base_cache():
    with pytest.raises(ValueError, match="another base cache"):
        m6_interaction.load_layer(path, "b" * 64)
```

- [x] **Step 2: Run the focused tests and verify they fail before implementation.**

Run: `python3 -m unittest -q tests/test_m6_interaction_layers.py && cargo test -p crowd-cache --test manifest_contract`

Expected: FAIL because the M6 layer helper and schema branch are absent.

- [x] **Step 3: Add the M6 animation-layer branch, atomic persistence, explicit removal, and cache-manifest metadata.**

- [x] **Step 4: Run focused tests plus existing M4 layer tests.**

Run: `python3 -m unittest -q tests/test_m6_interaction_layers.py tests/test_m4_layout_artifacts.py && cargo test -p crowd-cache --test manifest_contract --test interaction_layers`

Expected: PASS; existing M4 behavior remains unchanged.

- [ ] **Step 5: Commit the layer composition slice.**

```bash
git add schemas addon/blender_crowd crates/crowd-cache tests/test_m6_interaction_layers.py
git commit -m "Compose removable M6 interaction animation layers"
```

### Task 4: Add deterministic typed perception, blackboard, and trace evidence

**Files:**
- Create: `crates/crowd-core/src/perception.rs`
- Create: `crates/crowd-core/src/blackboard.rs`
- Modify: `crates/crowd-core/src/behavior.rs`
- Modify: `crates/crowd-core/src/runtime_behavior.rs`
- Modify: `crates/crowd-core/src/sim.rs`
- Create: `crates/crowd-core/tests/m6_perception.rs`
- Create: `crates/crowd-core/tests/m6_brain_runtime.rs`
- Create: `crates/crowd-core/tests/m6_trace_determinism.rs`

**Interfaces:**
- `PerceptionSnapshotV1` exposes ordered typed observations for vision, hearing,
  touch, density/flow, group extent, semantic distance, and bounded memory.
- `PerceptionEngine::observe(world, neighbors, semantics, tick) -> BTreeMap<AgentId, PerceptionSnapshotV1>`.
- `BlackboardValueV1` supports bool, fixed-point number, enum, stable ID, and
  bounded string values; undeclared channels are rejected at compile time.
- `DecisionTraceV2` records observations, utility scores, blackboard changes,
  interrupts, decisive node, action, group context, and degraded evidence.

- [x] **Step 1: Write failing tests for occlusion/order determinism, bounded memory, typed channel declaration, and state/utility/tree trace agreement.**
- [x] **Step 2: Run `cargo test -p crowd-core --test m6_perception --test m6_brain_runtime --test m6_runtime_perception` and verify the missing APIs fail.**
- [x] **Step 3: Implement the fixed-point perception snapshot, typed blackboard, and trace extension without changing existing M2 fixture semantics.**
- [x] **Step 4: Run focused tests and `cargo test -p crowd-core --test behavior_graph --test authorable_runtime`.**
- [x] **Step 5: Add the bounded 512-action library test and record it as foundation evidence without presenting it as a scale acceptance result.**

### Task 5: Add activities, reservations, and full group roles/formations

**Files:**
- Create: `crates/crowd-core/src/activity.rs`
- Modify: `crates/crowd-core/src/social.rs`
- Modify: `crates/crowd-core/src/authoring.rs`
- Create: `crates/crowd-core/tests/m6_activity.rs`
- Create: `crates/crowd-core/tests/m6_formations.rs`
- Create: `assets/reference/m6/activity-reservation-v1.json`

**Interfaces:**
- `ActivityScheduleV1` defines needs, windows, priorities, declared recovery policy, and resource IDs.
- `ReservationRuntime::request_batch` orders requests by stable agent ID and
  returns `Granted`, `Waiting`, `Released`, or `Failed` with reason.
- `FormationV1` defines role slots, offsets, split policy, shared perception,
  and deterministic leader/group decisions.

- [x] **Step 1: Write failing reservation and formation property tests for no double ownership, stable admission, declared deadlock policy, split/regroup, and unrelated-agent safety.**
- [x] **Step 2: Run the focused tests and verify the new activity/formations APIs are missing.**
- [x] **Step 3: Implement sorted reservation arbitration, rich activity declarations, recovery, group roles, formation offsets, and shared reports.**
- [x] **Step 4: Run focused tests plus `cargo test -p crowd-core --test groups_queues --test authorable_runtime`.**
- [x] **Step 5: Add the reference activity fixture and its schema/type test.**

### Task 6: Add motion database, trajectory queries, deterministic matching, and feedback diagnostics

**Files:**
- Create: `crates/crowd-core/src/motion.rs`
- Modify: `crates/crowd-core/src/phases/animate.rs`
- Create: `crates/crowd-core/tests/m6_motion_matching.rs`
- Create: `crates/crowd-core/tests/m6_motion_feedback.rs`
- Create: `assets/reference/m6/trajectory-database-v1.json`
- Create: `scripts/m6-motion-build.py`
- Create: `tests/test_m6_motion_database.py`

**Interfaces:**
- `MotionDatabaseV1` contains licensed/provenance-tagged clips and canonical
  feature channels for future trajectory, pose, contact, terrain, and turn.
- `MotionQueryV1` requests a bounded future trajectory and contact/terrain state.
- `MotionMatcher::select(query) -> MotionMatchResultV1` uses stable tie-breaking
  and returns a fallback clip when no candidate is feasible.
- `MotionFeedbackV1` reports root deviation, foot slip, turn/terrain mismatch,
  infeasibility, and the chosen corrective action.

- [x] **Step 1: Write failing tests for feature extraction, stable tie-breaking, root/terrain constraints, foot-contact windows, and no-teleport feedback.**
- [x] **Step 2: Run focused Rust/Python tests and verify the motion APIs are missing.**
- [x] **Step 3: Implement a deterministic in-repo reference database using redistributable synthetic/reference motion metadata only; do not ingest unlicensed clips.**
- [x] **Step 4: Integrate the matcher with animation scheduling as an optional promoted path and retain clip-state for background tiers.**
- [x] **Step 5: Run focused tests plus existing animation and M5 tier tests; record local thresholds and environment boundaries.**

### Task 7: Add generic interaction-to-physics/recovery and hero solver declarations

**Files:**
- Create: `crates/crowd-core/src/physics.rs`
- Modify: `addon/blender_crowd/m4_layout.py`
- Create: `addon/blender_crowd/m6_physics.py`
- Create: `crates/crowd-core/tests/m6_physics_recovery.rs`
- Create: `tests/test_m6_physics_boundaries.py`
- Create: `assets/reference/m6/physics-transition-v1.json`

**Interfaces:**
- `PhysicsTransitionV1` declares owner, interval, incoming state, collision
  mask, solver/cache provenance, recovery action, and failure policy.
- `validate_transition` rejects hidden solver ownership, unsupported channels,
  penetration, and missing recovery/fallback declarations.
- `HeroIntegrationBoundaryV1` records cloth/hair/facial solver, cache, support,
  failure, and per-tier applicability without requiring those features on the
  background crowd.

- [x] **Step 1: Write failing transition, recovery, solver-boundary, and unrelated-agent isolation tests.**
- [x] **Step 2: Run focused tests and verify the new boundary types are absent.**
- [x] **Step 3: Implement generic transitions by adapting the existing M4 physics handoff contract rather than adding a hidden Blender solver dependency.**
- [x] **Step 4: Run focused tests plus existing M4 physics/cache tests.**
- [x] **Step 5: Record the supported hero boundary and unsupported solver cases in the M6 evidence report.**

### Task 8: Build the M6 Blender debugger and extension-boundary examples

**Files:**
- Modify: `addon/blender_crowd/properties.py`
- Modify: `addon/blender_crowd/panels.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `addon/blender_crowd/debug_overlay.py`
- Create: `addon/blender_crowd/m6_debugger.py`
- Create: `tests/blender/test_m6_debugger.py`
- Create: `crates/crowd-core/src/extensions.rs`
- Create: `crates/crowd-core/tests/m6_extensions.rs`

**Interfaces:**
- `m6_debugger.build_trace_summary(trace, selected_agent_id, tier)` returns
  readable graph/action/observation/score/contact/layer ownership data.
- Blender exposes a synchronized timeline, graph search, node diagnostics,
  cross-agent/group context, motion/contact/physics diagnostics, and explicit
  reduced-evidence text for lower tiers.
- `ExtensionChannelV1` declares name, version, input/output channels, cost
  budget, determinism mode, and failure isolation; undeclared channels fail
  validation.

- [x] **Step 1: Write failing pure-Python summary tests and Rust extension contract tests.**
- [x] **Step 2: Run focused tests and verify the debugger/extension APIs are absent.**
- [x] **Step 3: Implement the pure summary model, Blender properties/operators/panel, and channel declarations; keep Blender calls coarse-grained.**
- [x] **Step 4: Run Python tests and the M6 debugger headless runner with normal host Metal access.**
- [x] **Step 5: Record trace-to-node and degraded-evidence behavior as automated M6 evidence; defer independent-user claims to M9.**
- [x] **Step 6: Implement and test bidirectional navigation among viewport agent, event, node, action, clip, contact, owning layer, and correction without copied IDs.**
- [x] **Step 7: Implement and test reusable subgraphs/actions and presets through the bounded typed graph surface.**
- [x] **Step 8: Extend the Blender smoke so every M6 automated UI goal has an observable assertion.**

### Task 9: Add the M6 runners, reports, documentation, and completion audit

**Files:**
- Create: `scripts/m6-foundation-test.sh`
- Create: `scripts/m6-acceptance.sh`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `README.md`
- Create: `docs/benchmarks/2026-08-18-m6-foundation.md`
- Create: `docs/benchmarks/2026-08-18-m6-acceptance.md`
- Modify: `docs/milestones/README.md`
- Modify: `docs/ui-ux-roadmap.md`

- [ ] **Step 1: Write runner contract tests that assert every command is checked in and each report criterion has an evidence status.**
- [ ] **Step 2: Run the runner test and verify it fails before the runners/report exist.**
- [x] **Step 3: Add copy-ready foundation and acceptance runners with separated local, Blender, performance, and licensed-data lanes plus an explicit M9 deferral.**
- [x] **Step 4: Run the foundation runner, full release workspace tests, clippy, formatting, Python tests, and available Blender runner.**
- [ ] **Step 5: Perform a requirement-by-requirement audit against all M6 contract items and write only evidence-backed statuses.**
- [ ] **Step 6: Mark the goal complete only if every deterministic M6 acceptance criterion and automated Blender gate has authoritative evidence; otherwise leave the goal active and list the exact remaining M6 gates. M9 gates never block M6.**

### Task 10: Complete automated debugger navigation and reusable graph authoring

**Files:**
- Create: `schemas/brain-library-v1.schema.json`
- Create: `assets/reference/m6/brain-library-v1.json`
- Create: `addon/blender_crowd/m6_library.py`
- Modify: `addon/blender_crowd/m6_debugger.py`
- Modify: `addon/blender_crowd/behavior_editor.py`
- Modify: `addon/blender_crowd/properties.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `addon/blender_crowd/panels.py`
- Create: `tests/test_m6_debugger_navigation.py`
- Create: `tests/test_m6_library.py`
- Modify: `tests/blender/test_m6_debugger.py`

**Interfaces:**
- `m6_debugger.build_navigation_index(trace, graph)` returns stable records with
  `target_kind`, `target_id`, `agent_id`, `tick`, `graph_node_id`, `action_id`,
  `motion_clip_id`, `contact_id`, `layer_id`, and `correction_id`. Target kinds
  are `agent`, `event`, `node`, `action`, `clip`, `contact`, `layer`, and
  `correction`.
- `m6_debugger.resolve_navigation(index, target_kind, target_id)` returns the
  complete linked record or raises `ValueError`; callers never paste a stable
  ID to move among linked contexts.
- `m6_library.validate_library(value)` validates version 1, unique action,
  subgraph, and preset IDs, declared typed parameters, bounded node kinds, and
  all references.
- `m6_library.instantiate_preset(value, preset_id, instance_id, parameters)`
  returns a deterministic bounded graph with namespaced node/action IDs and no
  runtime callback or source-code field.
- Blender operator `crowd.navigate_m6_context` resolves the selected target and
  updates scene selection, graph highlight, clip/contact/layer/correction
  context, and readable status. `crowd.apply_m6_brain_preset` instantiates the
  selected checked preset into the bounded behavior editor.

- [x] **Step 1: Write failing pure-Python navigation and library tests.**

```python
def test_navigation_resolves_every_context_without_copied_ids():
    index = m6_debugger.build_navigation_index(TRACE, GRAPH)
    node = m6_debugger.resolve_navigation(index, "node", "hold")
    assert node == {
        "target_kind": "node",
        "target_id": "hold",
        "agent_id": 7,
        "tick": 12,
        "graph_node_id": "hold",
        "action_id": "hold_position",
        "motion_clip_id": "idle_ready",
        "contact_id": "right_hand_guard",
        "layer_id": "interaction-pair-7-9",
        "correction_id": "mute-pair-7-9",
    }

def test_preset_instantiation_is_namespaced_and_deterministic():
    first = m6_library.instantiate_preset(LIBRARY, "guarded_exit", "north", {"destination": "exit_n"})
    second = m6_library.instantiate_preset(LIBRARY, "guarded_exit", "north", {"destination": "exit_n"})
    assert first == second
    assert first["entry_id"] == "north::root"
    assert {node["id"] for node in first["nodes"]} == {"north::root", "north::leave", "north::hold"}
```

- [x] **Step 2: Run the focused tests and verify the APIs are absent.**

Run: `python3 -m unittest -q tests/test_m6_debugger_navigation.py tests/test_m6_library.py`

Expected: FAIL because `build_navigation_index`, `resolve_navigation`, and
`m6_library` do not exist.

- [x] **Step 3: Add the versioned library fixture/schema and implement the pure
  navigation/library functions.**

Reject duplicate IDs, missing references, unknown parameters, unsupported node
kinds, empty instance IDs, source-code fields, and namespace collisions. Sort
all emitted actions/nodes by namespaced ID.

- [x] **Step 4: Add Blender properties/operators/panel controls and extend the
  headless test.**

The Blender test must navigate from node to agent, event, action, clip, contact,
layer, and correction in both directions, instantiate a preset, serialize it
through `behavior_editor.graph_from_tree`, and assert no copied-ID field is used.

- [x] **Step 5: Run pure Python, Blender, schema, and regression checks.**

Run:

```bash
python3 -m unittest -q tests/test_m6_debugger.py tests/test_m6_debugger_navigation.py tests/test_m6_library.py
M6_RUN_BLENDER=1 M6_ALLOW_OPEN=1 scripts/m6-acceptance.sh
git diff --check
```

Expected: all focused tests and the Blender lane pass; the audit no longer lists
the two debugger/library gates as open.

- [x] **Step 6: Commit.**

```bash
git add schemas/brain-library-v1.schema.json assets/reference/m6/brain-library-v1.json addon/blender_crowd tests scripts/m6-acceptance.sh
git commit -m "Complete M6 debugger navigation and graph presets"
```

### Task 11: Ingest licensed CMU motion and measure motion/terrain thresholds

**Files:**
- Create: `assets/reference/m6/cmu-motion-source-v1.json`
- Create: `scripts/m6_fetch_cmu_motion.py`
- Create: `scripts/m6_cmu_motion_ingest.py`
- Modify: `scripts/m6_motion_build.py`
- Modify: `scripts/m6_motion_evaluate.py`
- Modify: `docs/m6-motion-data-policy.md`
- Create: `tests/fixtures/m6/cmu-mini.asf`
- Create: `tests/fixtures/m6/cmu-mini.amc`
- Create: `tests/test_m6_cmu_motion.py`
- Modify: `tests/test_m6_motion_database.py`
- Modify: `tests/test_m6_motion_evaluation.py`

**Interfaces and fixed sources:**
- `m6_fetch_cmu_motion.fetch_manifest(manifest, output_dir)` downloads only the
  five declared files, verifies exact SHA-256 before rename, and refuses unknown
  hosts, redirects away from `mocap.cs.cmu.edu`, hash mismatch, or extra files.
- Fixed official sources and hashes:
  - `http://mocap.cs.cmu.edu/subjects/35/35.asf` — `2a8e2eda3c0d7d828566b2a9a8ab36b2b8b3864110574e8b73c8f069fded416c`
  - `http://mocap.cs.cmu.edu/subjects/35/35_01.amc` — `0743f4ea48e7e199cd56b2810b5ce81f8ede08d32ff79aa4e363c44cc4fe33aa`
  - `http://mocap.cs.cmu.edu/subjects/35/35_24.amc` — `29059fb2c15493983e4dccdf45453a495fb567dd28ff36cc1a0dbc02ad409445`
  - `http://mocap.cs.cmu.edu/subjects/36/36.asf` — `05e190867ead216b5dcdc94b210aa19b2eaaf383df44f1d9bb247e64fbf1c02b`
  - `http://mocap.cs.cmu.edu/subjects/36/36_01.amc` — `882e9f8c35622c2e10e9a3f578b5e0e7033ceb53232f415640b47fc05f3c2fac`
- The source manifest records `license_id: CMU-Mocap-Free-All-Uses`,
  `redistribution_allowed: false` for raw/converted files, the official terms
  URL, required attribution, subject/trial descriptions, source frame rate 120,
  and the exact hashes above. Raw or converted CMU motion is never committed.
- `m6_cmu_motion_ingest.ingest(asf_path, amc_paths, manifest)` parses Acclaim
  units, hierarchy, root order, bone axes/DOFs, and frames; computes world-space
  root and foot positions; downsamples deterministically from 120 Hz to 30 Hz;
  derives root velocity, facing, left/right contact windows, foot slide, joint
  limits, and source-frame provenance; and emits the existing version-1 motion
  database input shape.

- [ ] **Step 1: Write failing parser, downloader-boundary, and metric tests from
  hand-checked mini ASF/AMC fixtures.**

```python
def test_ingest_uses_declared_units_and_world_space_feet():
    database = m6_cmu_motion_ingest.ingest(ASF, [AMC], MANIFEST)
    assert database["clips"][0]["samples"][1]["velocity_millimeters_per_second"] == [1000, 0]
    assert database["clips"][0]["left_foot_contacts"] == [[0, 1]]

def test_fetch_rejects_a_hash_mismatch_before_publish():
    with self.assertRaisesRegex(ValueError, "SHA-256"):
        m6_fetch_cmu_motion.verify_download(b"wrong", "0" * 64)
```

- [ ] **Step 2: Run tests and verify the importer/fetcher are absent.**

Run: `python3 -m unittest -q tests/test_m6_cmu_motion.py`

Expected: FAIL with import errors for both scripts.

- [ ] **Step 3: Implement strict parsing, fixed-source fetching, provenance,
  retarget metadata, contact inference, and deterministic reports.**

Foot contact is declared only when foot height is at most 45 mm above its local
support minimum and horizontal foot speed is at most 120 mm/s for at least two
30 Hz samples. The report records root-speed error, foot slide during declared
contacts, turn discontinuity, joint-limit violations, retarget failures, source
hashes, and rejected frames. Do not silently smooth or repair source data.

- [ ] **Step 4: Fetch the five fixed files into a temporary artifact directory,
  ingest all three trials, and write dated derived reports only.**

Run:

```bash
python3 scripts/m6_fetch_cmu_motion.py assets/reference/m6/cmu-motion-source-v1.json /tmp/blender-crowd-m6-cmu
python3 scripts/m6_cmu_motion_ingest.py assets/reference/m6/cmu-motion-source-v1.json /tmp/blender-crowd-m6-cmu /tmp/blender-crowd-m6-cmu/database.json
python3 scripts/m6_motion_build.py /tmp/blender-crowd-m6-cmu/database.json /tmp/blender-crowd-m6-cmu/build-report.json
python3 scripts/m6_motion_evaluate.py /tmp/blender-crowd-m6-cmu/database.json /tmp/blender-crowd-m6-cmu/evaluation.json
```

Expected: three clips (`35_01_walk`, `35_24_run`, `36_01_uneven_walk`), exact
source hashes, nonzero samples, declared contacts, and no raw/converted source
file added to Git.

- [ ] **Step 5: Set and adjudicate checked M6 thresholds from the derived report.**

Create the threshold file only after the baseline report exists. It must retain
hard zero limits for root teleportation, undeclared contact, source-hash drift,
cross-cache mutation, and joint-limit violations; measured soft limits for foot
slide, trajectory deviation, turn discontinuity, and rejected-frame rate are
recorded with the baseline values and cannot be loosened without a new dated
adjudication report.

- [ ] **Step 6: Run focused and existing motion tests, then commit code,
  manifests, policy, tests, thresholds, and derived reports only.**

Run: `python3 -m unittest -q tests/test_m6_cmu_motion.py tests/test_m6_motion_database.py tests/test_m6_motion_evaluation.py && cargo test -p crowd-core --test m6_motion_matching --test m6_motion_feedback --test m6_provenance`

### Task 12: Realize the integrated deterministic M6 reference scenes

**Files:**
- Create: `schemas/m6-acceptance-scenes-v1.schema.json`
- Create: `assets/reference/m6/acceptance-scenes-v1.json`
- Create: `crates/crowd-bench/src/bin/m6-acceptance-scenes.rs`
- Create: `crates/crowd-bench/tests/m6_acceptance_scenes.rs`
- Create: `scripts/m6-reference-scenes-test.sh`
- Create: `docs/benchmarks/2026-08-18-m6-reference-scenes.md`

**Interfaces:**
- `m6-acceptance-scenes` accepts `--fixture`, `--motion-report`, and `--out`.
  It executes `scheduled_cafe`, `family_split_regroup`,
  `terrain_motion_feedback`, `paired_handoff`, `ragdoll_recovery`, and
  `mixed_tier_diagnostics` with stable seeds and emits schema version 1.
- Every scene report contains exact fixture/source hashes, tick range, agent and
  promoted-group counts, deterministic replay hash, hard-safety result,
  scene-specific metrics, fallback count, unrelated-agent mutation count, and
  pass/fail reasons. A schema-valid report with a failed criterion exits 1.

- [ ] **Step 1: Write failing binary tests for each scene and one combined
  deterministic report.**

```rust
#[test]
fn integrated_scenes_are_repeatable_and_preserve_unrelated_agents() {
    let first = run_fixture("assets/reference/m6/acceptance-scenes-v1.json");
    let second = run_fixture("assets/reference/m6/acceptance-scenes-v1.json");
    assert_eq!(first.deterministic_replay_hash, second.deterministic_replay_hash);
    assert_eq!(first.unrelated_agent_mutations, 0);
    assert!(first.scenes.iter().all(|scene| scene.passed));
}
```

- [ ] **Step 2: Run `cargo test -p crowd-bench --test m6_acceptance_scenes` and
  verify the fixture/binary are absent.**
- [ ] **Step 3: Implement the smallest integrated runner by composing the
  existing M6 runtimes; do not duplicate their decision logic in the binary.**
- [ ] **Step 4: Run the runner twice and require identical hashes and metrics.**

Run: `scripts/m6-reference-scenes-test.sh`

- [ ] **Step 5: Check in the fixture, schema, runner, tests, and dated report;
  keep generated caches outside Git.**

### Task 13: Prove Blender physics/hero layers and mixed-tier performance

**Files:**
- Modify: `addon/blender_crowd/m6_physics.py`
- Modify: `addon/blender_crowd/m6_interaction.py`
- Modify: `addon/blender_crowd/properties.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `addon/blender_crowd/panels.py`
- Create: `tests/blender/test_m6_layers.py`
- Create: `scripts/m6-performance-test.sh`
- Create: `crates/crowd-bench/tests/m6_mixed_tier.rs`
- Create: `docs/benchmarks/2026-08-18-m6-blender-layers.md`
- Create: `docs/benchmarks/2026-08-18-m6-mixed-tier.md`
- Modify: `scripts/m6-blender-test.sh`

**Interfaces:**
- Blender loads the deterministic paired-interaction and physics-transition
  artifacts, composes them against a complete base-cache hash, exposes owner,
  interval, contacts, solver/cache provenance, recovery/failure policy, and hero
  support boundaries, then mutes/removes/reloads them without changing base or
  unrelated agents.
- The mixed-tier gate runs 10,000 agents at the checked M5 distribution with a
  fixed promoted subset: 10 S0 hero, 990 S1 promoted, and 9,000 S2 background.
  It measures perception/brain/activity/group/motion/interaction phases
  separately and requires at least 10 ticks/s, zero hard-safety failures, zero
  unrelated-agent mutations, deterministic replay hashes, and explicit fallback
  accounting. Debug evidence is full for S0, reduced for S1, aggregate-only for
  S2, and never inferred when absent.

- [ ] **Step 1: Write failing Blender layer and Rust mixed-tier tests.**
- [ ] **Step 2: Run focused tests and verify layer UI/runtime and mixed-tier
  report are absent.**
- [ ] **Step 3: Implement coarse Blender layer attachment/inspection and the
  backend-neutral mixed-tier benchmark without per-agent Python loops.**
- [ ] **Step 4: Run host Blender with normal Metal access and the optimized
  mixed-tier gate.**

Run:

```bash
scripts/m6-blender-test.sh
scripts/m6-performance-test.sh
```

- [ ] **Step 5: Record environment, separate phase timings, memory/cache size,
  tier counts, unsupported solver claims, and exact evidence paths; commit.**

### Task 14: Add external examples and close the requirement-level audit

**Files:**
- Create: `examples/m6-extension-rust.rs`
- Create: `examples/m6_extension_python.py`
- Create: `tests/test_m6_extension_examples.py`
- Create: `docs/benchmarks/2026-08-18-m6-acceptance.md`
- Modify: `scripts/m6-foundation-test.sh`
- Modify: `scripts/m6-acceptance.sh`
- Modify: `docs/benchmarks/2026-08-18-m6-foundation.md`
- Modify: `docs/milestones/M6-advanced-agency-motion.md`
- Modify: `docs/milestones/README.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `AGENTS.md`

**Interfaces:**
- The Rust and Python examples declare version, input/output channels, fixed
  cost budget, deterministic mode, and failure isolation; run one accepted call,
  one over-budget fallback, one undeclared-channel rejection, and one version
  mismatch. No C/C++ API is claimed unless a real wrapper and matching example
  are implemented in this task.
- `scripts/m6-acceptance.sh` executes foundation, debugger/library, CMU-derived
  motion thresholds, integrated scenes, host Blender layer/debugger proof when
  `M6_RUN_BLENDER=1`, mixed-tier performance, extension examples, report-schema
  checks, and release workspace tests. It exits 0 without `M6_ALLOW_OPEN` only
  when every deterministic M6 criterion passes. M9 lines remain informational
  `DEFERRED`, never M6 failures.

- [ ] **Step 1: Write failing example execution and acceptance-contract tests.**
- [ ] **Step 2: Run them and verify examples and closed-audit behavior are absent.**
- [ ] **Step 3: Implement the two claimed-language examples and wire all
  deterministic gates into the acceptance runner.**
- [ ] **Step 4: Run the full release workspace, clippy, formatting, Python,
  motion, scene, Blender, mixed-tier, extension, and acceptance commands.**
- [ ] **Step 5: Write the dated criterion-by-criterion report with environment,
  inputs/hashes, results, known failures, unsupported claims, and M9 deferrals.**
- [ ] **Step 6: Mark M6 accepted only after the unmodified acceptance runner
  exits 0 and the report contains direct evidence for all ten criteria.**
- [ ] **Step 7: Commit the final audit and documentation.**
