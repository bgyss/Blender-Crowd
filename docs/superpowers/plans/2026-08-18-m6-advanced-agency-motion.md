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
- [ ] **Step 6: Implement and test bidirectional navigation among viewport agent, event, node, action, clip, contact, owning layer, and correction without copied IDs.**
- [ ] **Step 7: Implement and test reusable subgraphs/actions and presets through the bounded typed graph surface.**
- [ ] **Step 8: Extend the Blender smoke so every M6 automated UI goal has an observable assertion.**

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
