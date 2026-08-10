# M0 Cache Closure and M1 Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the outstanding M0 cache/facade gates, then deliver and prove the self-contained 1,000-agent M1 concourse from versioned project input through cache-only Blender rendering.

**Architecture:** `crowd-cache` owns a recoverable versioned directory cache while `crowd-core` owns project compilation, deterministic commuters, and authoritative simulation state. `crowd-blender` exposes coarse compile/session/bake/cache/query operations; Blender Python extracts authoring data and pushes cache buffers to a procedural Geometry Nodes presentation without running per-agent simulation logic.

**Tech Stack:** Rust 1.94.1 workspace, serde/serde_json, PyO3 0.29 abi3, `blake3 = "1.8"`, `crc32c = "0.6"`, test-only `tempfile = "3.20"` and `jsonschema = "0.33"`, Python 3.13 in Blender 5.2 LTS, Blender Python, NumPy, Geometry Nodes, JSON Schema fixtures, shell runners.

## Global Constraints

- M0 must pass all seven criteria before M1 is declared unblocked; a failed M0 gate stops M1 acceptance work.
- Preserve stable IDs, fixed-step simulation, deterministic event ordering, versioned schemas, and the Rust/Python/GN ownership boundaries.
- Geometry Nodes is presentation only; no goals, path planning, avoidance, or authoritative behavior may enter GN or per-agent Python loops.
- Reference scenes, characters, materials, and clips are generated from checked-in procedural specifications; no downloaded asset is allowed.
- Stable random choices derive independently from stable IDs and purpose keys; adding or reordering an agent cannot reshuffle existing choices.
- Cache position quantization error may not exceed 1 mm; canceled caches never report complete; complete caches validate every declared file.
- New dependencies require a checked-in license decision. `blake3` and `crc32c` must be pinned and documented before use.
- Existing trace v0 playback and all six avoidance benchmark baselines remain compatible.
- `cargo fmt --check`, `cargo test --workspace`, release density/navigation tests, and `cargo clippy --workspace --all-targets -- -D warnings` must pass before completion.
- New runners must be copied exactly into `README.md` and `CLAUDE.md`; no report may claim a runner passed before it exists and is executed.
- Sampled, traced, serialized, Blender playback, armature, and render timings remain separate from isolated simulation throughput.

---

## File map

New Rust/cache files:

- `crates/crowd-cache/Cargo.toml` — cache crate dependencies and package metadata.
- `crates/crowd-cache/src/lib.rs` — public cache API.
- `crates/crowd-cache/src/manifest.rs` — manifest/status/channel/chunk types and validation.
- `crates/crowd-cache/src/checksum.rs` — CRC-32C payload checksums and BLAKE3 content hashes.
- `crates/crowd-cache/src/codec.rs` — static-agent and frame-channel codecs/quantization.
- `crates/crowd-cache/src/defaults.rs` — measured cache-v1 chunk/encoding defaults.
- `crates/crowd-cache/src/writer.rs` — atomic chunk/manifest publication and cancellation.
- `crates/crowd-cache/src/reader.rs` — complete reader and incomplete-cache recovery inspector.
- `crates/crowd-cache/src/override_layer.rs` — versioned sparse transform layers.
- `crates/crowd-cache/tests/cache_lifecycle.rs` — full complete/canceled/corrupt lifecycle.
- `crates/crowd-core/src/project.rs` — `ProjectIrV1`, diagnostics, content hash, deterministic compilation.
- `crates/crowd-core/src/commuter.rs` — fixed commuter/animation state and decision codes.
- `crates/crowd-core/src/phases/animate.rs` — authoritative clip/phase/playback-rate update.
- `crates/crowd-core/src/concourse.rs` — checked reference-project compiler and scene construction.
- `crates/crowd-core/tests/m1_strict.rs` — 1,000-agent strict rebake and portal acceptance.
- `crates/crowd-bench/src/cache_bench.rs` — M0 cache experiment.
- `crates/crowd-bench/src/m1_bench.rs` — headless compile/bake/check report path.

New schemas/assets/docs:

- `schemas/project-ir-v1.schema.json`
- `schemas/cache-manifest-v1.schema.json`
- `schemas/decision-trace-v1.schema.json`
- `schemas/override-layer-v1.schema.json`
- `assets/reference/concourse-project-v1.json`
- `assets/reference/commuter-assets-v1.json`
- `assets/reference/README.md`
- `docs/cache-format-v1.md`
- `docs/dependencies/cache-v1.md`
- `docs/user/m1-reference-walkthrough.md`
- `docs/benchmarks/2026-08-10-cache-v0-experiment.md`
- `docs/benchmarks/2026-08-10-m0-consolidated.md`
- `docs/benchmarks/2026-08-10-m1-vertical-slice.md`

New Blender/add-on/test files:

- `addon/blender_crowd/project.py` — Blender scene property extraction to `ProjectIrV1`.
- `addon/blender_crowd/properties.py` — minimal project/cache/selection settings.
- `addon/blender_crowd/panels.py` — narrow M1 workflow and diagnostics panel.
- `addon/blender_crowd/cache_playback.py` — cache-only point-buffer playback.
- `addon/blender_crowd/reference_assets.py` — procedural concourse, commuters, materials, rigs, actions.
- `addon/blender_crowd/debug_overlay.py` — selected path/velocity evidence.
- `addon/blender_crowd/overrides.py` — pinned transform-layer authoring.
- `tests/blender/test_m1_project.py`
- `tests/blender/test_m1_cache_playback.py`
- `tests/blender/test_m1_override.py`
- `tests/blender/test_m1_render.py`
- `scripts/cache-experiment.sh`
- `scripts/m0-acceptance.sh`
- `scripts/m1-bake-test.sh`
- `scripts/m1-blender-test.sh`
- `scripts/m1-render-test.sh`

Modified:

- `Cargo.toml` — add `crowd-cache` and pinned shared dependencies.
- `Cargo.lock` — resolved dependencies.
- `crates/crowd-core/src/lib.rs`, `world.rs`, `sim.rs`, `phases/mod.rs`, `metrics.rs` — new project/commuter/animate/snapshot state.
- `crates/crowd-bench/Cargo.toml`, `src/lib.rs`, `src/main.rs` — cache/M1 commands.
- `crates/crowd-blender/Cargo.toml`, `src/lib.rs` — coarse project/session/cache facade while retaining `Trace`.
- `addon/blender_crowd/__init__.py`, `operators.py`, `geometry_nodes.py` — registration and M1 operators/presentation.
- `scripts/build-wheel.sh`, `scripts/verify-wheel.sh`, `scripts/verify_wheel.py` — package and verify the expanded facade.
- `README.md`, `CLAUDE.md`, `docs/milestones/README.md` — exact runners and milestone evidence state.

---

### Task 1: Cache manifest and status contract

**Files:**
- Create: `crates/crowd-cache/Cargo.toml`
- Create: `crates/crowd-cache/src/lib.rs`
- Create: `crates/crowd-cache/src/manifest.rs`
- Create: `crates/crowd-cache/tests/manifest_contract.rs`
- Create: `schemas/cache-manifest-v1.schema.json`
- Modify: `Cargo.toml`

**Interfaces:**
- Produces: `CacheStatus::{Incomplete, Canceled, Complete}`, `ChannelDef`, `ChunkDef`, `CacheManifestV1`, `CacheManifestV1::validate(&self) -> Result<(), ManifestError>`, `CACHE_SCHEMA_VERSION: u32 = 1`.

- [ ] **Step 1: Add crate scaffolding and the failing status-validation test**

```rust
// crates/crowd-cache/tests/manifest_contract.rs
use crowd_cache::{CacheManifestV1, CacheStatus, ManifestError};

#[test]
fn complete_manifest_rejects_an_unfinished_declared_chunk() {
    let mut manifest = CacheManifestV1::test_fixture(1_000, 120);
    manifest.status = CacheStatus::Complete;
    manifest.chunks[0].complete = false;

    assert!(matches!(
        manifest.validate(),
        Err(ManifestError::IncompleteChunk { index: 0 })
    ));
}

#[test]
fn canceled_manifest_records_the_last_complete_tick() {
    let mut manifest = CacheManifestV1::test_fixture(1_000, 120);
    manifest.status = CacheStatus::Canceled;
    manifest.last_complete_tick = Some(59);
    assert_eq!(manifest.validate(), Ok(()));
}
```

- [ ] **Step 2: Run the test and observe the missing API failure**

Run: `cargo test -p crowd-cache --test manifest_contract`
Expected: FAIL with unresolved imports for `CacheManifestV1`, `CacheStatus`, and `ManifestError`.

- [ ] **Step 3: Implement the manifest types and invariant checks**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus { Incomplete, Canceled, Complete }

impl CacheManifestV1 {
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != CACHE_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedVersion(self.schema_version));
        }
        if self.status == CacheStatus::Complete {
            if let Some((index, _)) = self.chunks.iter().enumerate().find(|(_, c)| !c.complete) {
                return Err(ManifestError::IncompleteChunk { index });
            }
        }
        if self.status == CacheStatus::Canceled && self.last_complete_tick.is_none() {
            return Err(ManifestError::MissingLastCompleteTick);
        }
        Ok(())
    }
}
```

`test_fixture` is public only under `#[cfg(any(test, feature = "test-fixtures"))]`; integration tests enable the feature. Production constructors are `CacheManifestV1::new_incomplete(...)` and `CacheManifestV1::finish(...)`.

- [ ] **Step 4: Add the exact JSON Schema and serde golden test**

The schema requires `schema_version`, `engine_version`, `project_id`, `source_hash`, `tick_start`, `tick_end`, `ticks_per_second`, `agent_count`, `channels`, `chunks`, `status`, and `last_complete_tick`; `additionalProperties` is `false` at every versioned object boundary. Add a test serializing the fixture and asserting those literal keys and `schema_version == 1`.

- [ ] **Step 5: Run focused tests, format, and commit**

Run: `cargo test -p crowd-cache --test manifest_contract && cargo fmt --check`
Expected: PASS.

Commit: `Add the versioned crowd cache manifest contract`

---

### Task 2: Checksums and frame-channel codecs

**Files:**
- Create: `crates/crowd-cache/src/checksum.rs`
- Create: `crates/crowd-cache/src/codec.rs`
- Create: `crates/crowd-cache/tests/codec_round_trip.rs`
- Modify: `crates/crowd-cache/src/lib.rs`
- Modify: `crates/crowd-cache/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `Cargo.toml`
- Create: `docs/dependencies/cache-v1.md`

**Interfaces:**
- Produces: `content_hash(bytes: &[u8]) -> [u8; 32]`, `payload_checksum(bytes: &[u8]) -> u32`, `PositionEncoding::{F32, MillimeterI32, AffineI16}`, `AgentStatic`, `FrameRecord`, `EncodedChunk`, `encode_chunk(...)`, `decode_chunk(...)`.

- [ ] **Step 1: Write failing checksum and literal round-trip tests**

```rust
#[test]
fn crc32c_matches_the_standard_check_value() {
    assert_eq!(payload_checksum(b"123456789"), 0xe306_9283);
}

#[test]
fn millimeter_positions_round_trip_with_half_millimeter_error() {
    let input = vec![FrameRecord::fixture(7, [12.3454, -8.7656], 0.25)];
    let encoded = encode_chunk(4, &input, PositionEncoding::MillimeterI32).unwrap();
    let decoded = decode_chunk(&encoded.bytes).unwrap();
    assert_eq!(decoded.records[0].agent_id, 7);
    assert!((decoded.records[0].position[0] - 12.3454).abs() <= 0.0005);
    assert!((decoded.records[0].position[1] + 8.7656).abs() <= 0.0005);
}

#[test]
fn a_payload_bit_flip_is_rejected() {
    let mut encoded = encode_chunk(0, &[FrameRecord::fixture(1, [1.0, 2.0], 0.0)], PositionEncoding::F32).unwrap();
    *encoded.bytes.last_mut().unwrap() ^= 1;
    assert!(matches!(decode_chunk(&encoded.bytes), Err(CodecError::ChecksumMismatch { .. })));
}
```

- [ ] **Step 2: Run and verify the missing-codec failure**

Run: `cargo test -p crowd-cache --test codec_round_trip`
Expected: FAIL because `checksum` and `codec` APIs do not exist.

- [ ] **Step 3: Implement fixed headers, CRC-32C, BLAKE3, and the three position encodings**

Use magic `BCFRM\0\x01\0`, little-endian fields, checked integer conversions, explicit payload lengths, and channel-major payloads. `AffineI16` stores per-chunk `[min_x, min_y]` and `[scale_x, scale_y]`; zero-span axes use scale `1.0` and code `0`. All discrete fields remain exact.

- [ ] **Step 4: Add property tests for finite values, boundaries, and corrupted headers**

Generate finite positions in `-10_000.0..10_000.0`, 1–32 records, and all encodings. Assert IDs/discrete channels exact and decoded positions within each encoding's declared bound. Reject NaN/Inf input, a wrong magic, truncated header/payload, overflowed counts, and a declared length larger than the file.

- [ ] **Step 5: Record dependency licenses and commit**

Document exact resolved versions, upstream repository URLs, SPDX licenses, use, and rejection rationale for implementing nonstandard hashes. Include test-only `tempfile` and `jsonschema`; use the latter to validate emitted manifests against the checked schema rather than only grepping keys. Run `cargo test -p crowd-cache && cargo fmt --check`.

Commit: `Add checksummed crowd cache frame codecs`

---

### Task 3: Atomic writer, cancellation, recovery, and reader

**Files:**
- Create: `crates/crowd-cache/src/writer.rs`
- Create: `crates/crowd-cache/src/reader.rs`
- Create: `crates/crowd-cache/tests/cache_lifecycle.rs`
- Modify: `crates/crowd-cache/src/lib.rs`

**Interfaces:**
- Produces: `CancelToken`, `CacheWriter::create`, `CacheWriter::write_agents`, `CacheWriter::push_tick`, `CacheWriter::finish`, `CacheWriter::cancel`, `CacheReader::open_complete`, `CacheReader::read_tick`, `RecoveryInspector::open`, `RecoveryReport`.

- [ ] **Step 1: Write the failing real-filesystem lifecycle test**

```rust
#[test]
fn canceled_cache_is_recoverable_but_never_complete() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("shot.crowd");
    let mut writer = CacheWriter::create(&target, BakeSpec::fixture(10, 120)).unwrap();
    writer.write_agents(&AgentStatic::fixtures(10)).unwrap();
    for tick in 0..60 { writer.push_tick(tick, &FrameRecord::fixtures(10, tick)).unwrap(); }
    writer.cancel("test cancellation").unwrap();

    assert!(matches!(CacheReader::open_complete(&target), Err(CacheError::NotComplete(CacheStatus::Canceled))));
    let recovery = RecoveryInspector::open(&target).unwrap();
    assert_eq!(recovery.status, CacheStatus::Canceled);
    assert_eq!(recovery.last_complete_tick, Some(59));
    assert_eq!(recovery.readable_tick_range, Some(0..=59));
}
```

- [ ] **Step 2: Run and verify failure from missing writer/reader APIs**

Run: `cargo test -p crowd-cache --test cache_lifecycle`
Expected: FAIL with unresolved writer/reader imports.

- [ ] **Step 3: Implement atomic manifest/chunk publication**

Write a sibling `.tmp`, `sync_all`, rename to the final file, then atomically replace `manifest.json`. Index only renamed chunks. `finish` read-validates all files before publishing `complete`; `cancel` flushes the current complete chunk, publishes `canceled`, and retains prior chunks.

- [ ] **Step 4: Implement complete reader and recovery inspector**

`open_complete` validates manifest status, agent table, chunk existence, sizes, and checksums. `read_tick` binary-searches the chunk index and decodes only that chunk. `RecoveryInspector` accepts incomplete/canceled manifests, ignores orphan `.tmp` files, and reports only checksum-valid finalized chunks.

- [ ] **Step 5: Add complete, corrupt, missing, nonsequential, and atomicity tests**

Assert a complete 120-tick cache reads ticks `0`, `119`, `60`, `1` exactly; corrupting one payload names that chunk; deleting one indexed file names it; a failing rename leaves the manifest incomplete; cancellation token observation occurs between tick writes without a Python callback.

- [ ] **Step 6: Run and commit**

Run: `cargo test -p crowd-cache && cargo test --workspace && cargo fmt --check`
Expected: PASS.

Commit: `Add recoverable atomic crowd cache lifecycle`

---

### Task 4: Cache experiment runner and selected defaults

**Files:**
- Create: `crates/crowd-bench/src/cache_bench.rs`
- Create: `crates/crowd-cache/src/defaults.rs`
- Modify: `crates/crowd-bench/src/lib.rs`
- Modify: `crates/crowd-bench/src/main.rs`
- Modify: `crates/crowd-bench/Cargo.toml`
- Modify: `crates/crowd-cache/src/lib.rs`
- Create: `scripts/cache-experiment.sh`
- Create: `docs/cache-format-v1.md`
- Create: `docs/benchmarks/2026-08-10-cache-v0-experiment.md`

**Interfaces:**
- Produces: CLI `crowd-bench cache-experiment --agents 1000 --out <dir>`, `CacheExperimentReport`, machine-readable `report.json`, and checked constants `CacheDefaults { chunk_ticks, position_encoding }` consumed by Task 6.

- [ ] **Step 1: Write a failing CLI integration test for the experiment matrix**

Invoke the library runner with 20 deterministic fixture frames and assert nine results: three chunk sizes times three encodings. Assert every result includes bytes, write/read duration, frames/s, error bound, cancel latency, and recovered chunks.

- [ ] **Step 2: Run and observe the missing runner failure**

Run: `cargo test -p crowd-bench cache_bench::tests::matrix_contains_every_candidate`
Expected: FAIL because `cache_bench` does not exist.

- [ ] **Step 3: Implement the matrix and deterministic selection rule**

Select the smallest candidate with maximum error `<= 0.001` meters and sequential playback time `<= raw_f32_time * 1.10`; ties prefer fewer chunks, then the enum order `AffineI16`, `MillimeterI32`, `F32`. Record the selected encoding/chunk size in the report and set the literal constants in `defaults.rs`; a test compares the constants to the checked `report.json` selection so they cannot drift.

- [ ] **Step 4: Add the shell runner and execute the 1,000-agent experiment**

The script runs the release binary directly after `cargo build --release -p crowd-bench`, records `uname`, `sysctl` CPU/RAM where available, `rustc -Vv`, git commit, and input hash, and writes into a temporary output directory before copying only `report.json` and the human report into `docs/benchmarks/`.

- [ ] **Step 5: Review measured selection, update report, and commit**

Run focused tests, `scripts/cache-experiment.sh`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`. Write `docs/cache-format-v1.md` from the selected header/channel/status/quantization contract. The report must list measured results and unsupported claims rather than copying design targets.

Commit: `Measure and select the M0 crowd cache format`

---

### Task 5: Versioned project IR and deterministic population compilation

**Files:**
- Create: `crates/crowd-core/src/project.rs`
- Create: `crates/crowd-core/tests/project_compile.rs`
- Create: `schemas/project-ir-v1.schema.json`
- Create: `assets/reference/concourse-project-v1.json`
- Modify: `crates/crowd-core/src/lib.rs`

**Interfaces:**
- Produces: `ProjectIrV1`, `PopulationIrV1`, `SemanticIrV1`, `TimedPortalEventV1`, `Diagnostic`, `DiagnosticCode`, `CompiledProject`, `compile_project(&ProjectIrV1) -> Result<CompiledProject, Vec<Diagnostic>>`, `CompiledProject::agent_spawns()`.

- [ ] **Step 1: Write failing tests for ordered diagnostics and stable variants**

```rust
#[test]
fn adding_one_agent_does_not_reshuffle_existing_choices() {
    let project_100 = reference_project(100);
    let project_101 = reference_project(101);
    let a = compile_project(&project_100).unwrap();
    let b = compile_project(&project_101).unwrap();
    assert_eq!(&a.agent_spawns()[..100], &b.agent_spawns()[..100]);
}

#[test]
fn diagnostics_are_stably_ordered_and_name_the_entity() {
    let mut project = reference_project(10);
    project.populations[0].archetypes.clear();
    project.semantics.destinations.clear();
    let errors = compile_project(&project).unwrap_err();
    assert_eq!(errors.iter().map(|d| (&d.code, d.entity_id.as_str())).collect::<Vec<_>>(), vec![
        (&DiagnosticCode::MissingDestination, "population:commuters"),
        (&DiagnosticCode::InvalidWeights, "population:commuters"),
    ]);
}
```

- [ ] **Step 2: Run and verify the missing compiler failure**

Run: `cargo test -p crowd-core --test project_compile`
Expected: FAIL because `project` types/functions do not exist.

- [ ] **Step 3: Implement serde IR, validation, stable weighted choice, and content hash**

Use current `derive_agent_id` and `StableRng` purpose-key conventions. Sort authored entity IDs before assigning compact 32-bit table indices. Validate every reference and numeric bound before scene compilation; never partially compile invalid input.

- [ ] **Step 4: Add golden JSON/schema and permutation tests**

Deserialize `assets/reference/concourse-project-v1.json`, reserialize to canonical sorted JSON, and compare its BLAKE3 hash to a literal checked fixture. Permute population/archetype input order and assert stable compiled choices. Add duplicate-ID, unreachable-destination, invalid-unit, zero-weight, and contradictory-portal-event cases.

- [ ] **Step 5: Run and commit**

Run: `cargo test -p crowd-core --test project_compile && cargo test --workspace && cargo fmt --check`.

Commit: `Add versioned deterministic crowd project compilation`

---

### Task 6: Coarse session/bake/cache PyO3 facade

**Files:**
- Modify: `crates/crowd-blender/Cargo.toml`
- Modify: `crates/crowd-blender/src/lib.rs`
- Modify: `scripts/verify_wheel.py`
- Modify: `scripts/verify-wheel.sh`

**Interfaces:**
- Produces Python classes `CompiledProject`, `Session`, `CancelToken`, `Cache`, plus `compile_project(project_json: str) -> CompiledProject`; retains existing `Trace` unchanged.

- [ ] **Step 1: Extend the plain-CPython wheel verifier with failing facade behavior**

The verifier loads the reference JSON, compiles it, creates a strict 25-agent session, steps 10 ticks, queries one stable ID, bakes 60 ticks, destroys the session, opens `Cache(require_complete=True)`, and validates bulk-buffer lengths. A second bake cancels after the first completed chunk and asserts `Cache` rejects it while `inspect_cache` reports `canceled`.

- [ ] **Step 2: Build/install the wheel and observe missing facade symbols**

Run: `scripts/build-wheel.sh && scripts/verify-wheel.sh`
Expected: FAIL at `blender_crowd_native.compile_project` missing.

- [ ] **Step 3: Implement PyO3 wrappers with native-only coarse operations**

`CompiledProject` owns `Arc<crowd_core::CompiledProject>`. `Session` owns the simulation and exposes `step`, `query_agent`, and `bake`. `CancelToken` wraps `Arc<AtomicBool>`. `bake` calls `py.detach` around simulation/cache work. `Cache.read_tick` returns prefixed channel buffers; errors include stable code and responsible path/entity.

- [ ] **Step 4: Add Rust packing tests and CPython cancellation thread test**

Packing tests hand-reassemble 64-bit IDs, check every buffer length for three agents, and compare literal position/channel values. The Python verifier starts bake on a worker thread, calls `token.cancel()`, joins, and asserts no Python callback or Blender import was used.

- [ ] **Step 5: Run wheel verification and commit**

Run: `cargo test -p crowd-blender && scripts/build-wheel.sh && scripts/verify-wheel.sh && cargo clippy --workspace --all-targets -- -D warnings`.

Commit: `Expose coarse project session and cache facade`

---

### Task 7: Consolidated M0 acceptance gate

**Files:**
- Create: `scripts/m0-acceptance.sh`
- Create: `docs/benchmarks/2026-08-10-m0-consolidated.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `docs/milestones/README.md`

**Interfaces:**
- Produces: one runner whose exit code is nonzero unless all M0 criteria have fresh evidence; one dated report mapping criteria 1–7 to commands/results.

- [ ] **Step 1: Write the runner as executable acceptance behavior**

Run, in order: workspace tests; release density; release two-room reroute; solver baseline check; cache lifecycle and 1,000-agent experiment; wheel facade verification; clean Blender install; 1,000-point playback. Capture each command, exit code, duration, environment, and evidence path in a machine-readable JSON summary.

- [ ] **Step 2: Execute it and preserve the first real failure**

Run: `scripts/m0-acceptance.sh`
Expected: either PASS or a nonzero exit naming one evidenced M0 blocker. Do not begin Task 8 while it fails.

- [ ] **Step 3: Fix only evidenced M0 defects test-first and rerun**

For each defect, add a focused test reproducing the failure, observe red, implement the narrow fix, observe green, then restart the complete acceptance runner so the report reflects one fresh run.

- [ ] **Step 4: Write the consolidated report and update milestone state**

The report lists environment, repository/input hashes, every command/result, chosen navigation/avoidance/cache/bridge path, real-time 1K result, known failures, unsupported 10K/100K claims, and the M1 unblock decision. Update the milestone index to `M0 accepted` only when the runner passes.

- [ ] **Step 5: Validate docs and commit**

Run: `git diff --check`, `rg '^## ' docs/blender-crowd-1.0.md`, `rg '^#' docs/milestones/*.md`, and `scripts/m0-acceptance.sh`.

Commit: `Accept M0 with consolidated cache and facade evidence`

---

### Task 8: Concourse scene, timed portals, commuter state, and animation phase

**Files:**
- Create: `crates/crowd-core/src/concourse.rs`
- Create: `crates/crowd-core/src/commuter.rs`
- Create: `crates/crowd-core/src/phases/animate.rs`
- Create: `crates/crowd-core/tests/m1_commuter.rs`
- Modify: `crates/crowd-core/src/lib.rs`
- Modify: `crates/crowd-core/src/world.rs`
- Modify: `crates/crowd-core/src/sim.rs`
- Modify: `crates/crowd-core/src/phases/mod.rs`
- Modify: `crates/crowd-core/src/metrics.rs`

**Interfaces:**
- Produces: `CommuterState::{Unspawned, Travel, Arrived, Blocked}`, `DecisionReason`, `ClipState`, `AgentSnapshot`, `Simulation::apply_timed_inputs`, `Simulation::frame_snapshot`, `Simulation::query_agent`, and a `compile_concourse(&CompiledProject) -> Result<CompiledScene, Vec<Diagnostic>>` path.

- [ ] **Step 1: Write failing state and phase tests**

```rust
#[test]
fn a_traveling_agent_uses_distance_to_advance_walk_phase() {
    let mut sim = concourse_simulation(1);
    sim.step().unwrap();
    let before = sim.frame_snapshot().agents[0].clone();
    sim.step().unwrap();
    let after = sim.frame_snapshot().agents[0].clone();
    assert_eq!(after.commuter_state, CommuterState::Travel);
    assert_eq!(after.clip_state.clip_id, WALK_CLIP_ID);
    assert!(after.clip_state.phase > before.clip_state.phase);
}

#[test]
fn portal_close_records_replan_reason_only_for_affected_routes() {
    let mut sim = concourse_simulation(40);
    sim.step_n(90).unwrap();
    let unaffected = sim.agent_ids_not_using_portal("south_door");
    sim.set_named_portal_open("south_door", false).unwrap();
    sim.step().unwrap();
    for id in unaffected {
        assert_ne!(sim.query_agent(id).unwrap().decision_reason, DecisionReason::PortalClosedReplan);
    }
}
```

- [ ] **Step 2: Run and observe missing commuter/animate APIs**

Run: `cargo test -p crowd-core --test m1_commuter`
Expected: FAIL with unresolved commuter state, snapshot, and concourse symbols.

- [ ] **Step 3: Extend SoA world state and add the animate phase**

Add parallel arrays for population/variant IDs, scale, commuter state, decision reason, clip ID, phase, playback rate, visibility, and render tier. Spawn publishes deterministic static choices. `animate` runs after integration and before commit, derives idle/walk/jog from solved speed, advances normalized phase from distance/stride, and never changes position.

- [ ] **Step 4: Compile the reference concourse and timed inputs**

Build a tiled scene with two spawn regions, three destination regions, north/south named door sets, walkable bounds, blocked boundary segments, and close/reopen ticks from `ProjectIrV1`. Apply events in `(tick, portal table index, authored ordinal)` order before spatial update.

- [ ] **Step 5: Add arrival, stationary, jog, phase-wrap, and state-hash tests**

Assert arrived agents use idle, near-zero speed preserves orientation, jog threshold selects the literal jog clip, phase wraps into `[0, 1)`, and the deterministic state hash changes if any commuter/animation discrete field changes.

- [ ] **Step 6: Run and commit**

Run: `cargo test -p crowd-core --test m1_commuter && cargo test --workspace && cargo fmt --check && cargo clippy -p crowd-core --all-targets -- -D warnings`.

Commit: `Add the fixed M1 commuter and animation runtime`

---

### Task 9: M1 headless bake, strict comparison, and selected-agent evidence

**Files:**
- Create: `crates/crowd-core/tests/m1_strict.rs`
- Create: `crates/crowd-bench/src/m1_bench.rs`
- Modify: `crates/crowd-bench/src/lib.rs`
- Modify: `crates/crowd-bench/src/main.rs`
- Create: `schemas/decision-trace-v1.schema.json`
- Create: `scripts/m1-bake-test.sh`

**Interfaces:**
- Produces CLI commands `m1 validate`, `m1 bake`, `m1 compare`, `m1 inspect-agent`; report fields for completion, boundary escapes, reroutes, channel equality, quantization error, and separated phase timings.

- [ ] **Step 1: Write the ignored 1,000-agent failing acceptance test**

```rust
#[test]
#[ignore = "release M1 acceptance"]
fn strict_reference_rebakes_reproduce_and_meet_navigation_gate() {
    let first = bake_reference_strict(1_000);
    let second = bake_reference_strict(1_000);
    assert_eq!(first.agent_count, 1_000);
    assert_eq!(first.discrete_digest, second.discrete_digest);
    assert!(first.max_position_delta(&second) <= 0.001);
    assert!(first.destination_completion >= 0.95);
    assert_eq!(first.static_boundary_escapes, 0);
    assert!(first.portal_reroute.accepted);
    assert!(first.unrelated_routes_unchanged);
}
```

- [ ] **Step 2: Run in release and observe missing harness or failed gate**

Run: `cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture`
Expected: FAIL because `bake_reference_strict` and its measured result do not exist.

- [ ] **Step 3: Implement headless bake and strict comparator using real cache files**

Each bake starts from the checked JSON, compiles a fresh project/session, writes cache v1, destroys the session, and reopens the cache. Compare exact static/discrete buffers and decoded positions within the selected encoding bound. Scan blocked segments independently from the simulator's collision path.

- [ ] **Step 4: Implement selected-agent evidence and JSON schema**

`inspect-agent` returns stable ID/tick, position, desired/solved velocity, corridor portal IDs, next target, destination, path status, commuter state, clip/phase/rate, relevant portal states, and decision code/text. Validate a emitted fixture against the checked schema.

- [ ] **Step 5: Tune only authored concourse duration/capacity if the fixed 95% gate fails**

Do not change solver thresholds merely to pass. A scene-duration or doorway-width change must remain within the approved reference semantics, be recorded in the asset hash/report, and preserve the portal bottleneck/reroute behavior. Any core correctness failure gets a focused red-green regression test.

- [ ] **Step 6: Add runner, rerun, and commit**

Run: `scripts/m1-bake-test.sh`, which performs validate, two strict release bakes, compare, cancellation recovery, and inspect-agent.

Commit: `Prove the strict headless M1 concourse bake`

---

### Task 10: Minimal Blender project workflow and procedural assets

**Files:**
- Create: `assets/reference/commuter-assets-v1.json`
- Create: `assets/reference/README.md`
- Create: `addon/blender_crowd/properties.py`
- Create: `addon/blender_crowd/project.py`
- Create: `addon/blender_crowd/reference_assets.py`
- Create: `addon/blender_crowd/panels.py`
- Create: `tests/blender/test_m1_project.py`
- Create: `scripts/m1-blender-test.sh`
- Modify: `addon/blender_crowd/__init__.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `scripts/blender-install-test.sh`

**Interfaces:**
- Produces operators `crowd.create_reference_project`, `crowd.validate_project`, `crowd.bake_cache`, `crowd.cancel_bake`; scene pointer `Scene.crowd_project`; `project.extract_ir(scene) -> dict`; `reference_assets.ensure_reference_assets(scene)`.

- [ ] **Step 1: Write the failing clean-file Blender test**

From an empty factory scene, invoke `crowd.create_reference_project`, extract IR, validate through native `compile_project`, and assert literal counts: one 1,000-agent population, two spawns, three destinations, two named doors, three archetypes, and idle/walk/jog clip logical IDs. Assert no external file path exists in the asset table.

- [ ] **Step 2: Run and observe missing operators/properties**

Run: `scripts/blender-install-test.sh --python tests/blender/test_m1_project.py`
Expected: FAIL because `crowd.create_reference_project` is not registered.

- [ ] **Step 3: Implement narrow property groups and IR extraction**

Store project UUID, seed, tick rate, cache path, status, selected stable ID halves, and reference fixture version. The create operator loads the checked JSON, builds semantic Blender objects with stable custom IDs, and generates project properties. Extraction reads typed properties/object bounds, never object names as semantic meaning.

- [ ] **Step 4: Generate self-contained meshes, materials, armatures, and actions**

Generate three low-poly commuter proportions from literal JSON dimensions; deterministic material palettes; one canonical armature; and idle/walk/jog actions with keyframes at normalized phases `0.0`, `0.25`, `0.5`, `0.75`, and `1.0`. Re-running generation reuses objects by stable logical ID and does not duplicate data blocks.

- [ ] **Step 5: Implement worker-thread bake and modal cancellation**

The worker receives only serialized IR, cache path, and native token. It performs no `bpy` call. A modal timer polls a thread-safe result/progress record on the main thread; cancel sets the token. Extend `scripts/blender-install-test.sh` with an optional `--python PATH` argument that runs the supplied test in the same isolated clean-install environment. Add `scripts/m1-blender-test.sh` to invoke the project test. Add a Blender test that cancels and confirms the resulting cache status is `canceled`.

- [ ] **Step 6: Run and commit**

Run: the focused Blender test twice in clean processes via `scripts/m1-blender-test.sh` plus `scripts/blender-install-test.sh`.

Commit: `Add the self-contained M1 Blender project workflow`

---

### Task 11: Cache-only Geometry Nodes playback

**Files:**
- Create: `addon/blender_crowd/cache_playback.py`
- Create: `tests/blender/test_m1_cache_playback.py`
- Modify: `addon/blender_crowd/geometry_nodes.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `crates/crowd-blender/src/lib.rs`
- Modify: `scripts/m1-blender-test.sh`

**Interfaces:**
- Produces `CachePlayback`, operator `crowd.attach_cache`, prefixed GN v1 attributes, frame-change handler, and procedural variant/phase presentation.

- [ ] **Step 1: Write the failing fresh-process playback test**

Bake 1,000 agents headlessly, exit that process, start Blender factory mode with only the cache path, attach it, seek first/middle/last/nonsequential frames, and assert point count plus exact attribute names/types/counts. Reassemble five literal stable IDs from low/high halves and compare to `agents.bin`.

- [ ] **Step 2: Run and observe missing `CachePlayback` failure**

Run: `scripts/m1-blender-test.sh --only cache-playback`.
Expected: FAIL importing `.cache_playback` or resolving `crowd.attach_cache`.

- [ ] **Step 3: Implement bulk buffer sync and frame mapping**

Use native `Cache.read_tick`, NumPy `frombuffer`, and Blender `foreach_set`. Static buffers are uploaded once; frame buffers update per seek. Negative/pre-start/post-end frames clamp with a visible warning rather than wrapping. Registration adds one idempotent frame handler and removes it on unregister.

- [ ] **Step 4: Build GN v1 procedural commuter presentation**

Select one of three generated prototype collections by `crowd_variant_id`; honor `crowd_visible` and `crowd_render_tier`; apply orientation/scale; drive bounded arm/leg proxy swing from `sin(2π * crowd_clip_phase)` for walk/jog while idle remains neutral. Retain legacy trace node group behavior under its original name.

- [ ] **Step 5: Add channel round-trip and fresh-session assertions**

Assert clip, phase, rate, orientation, scale, variant, visibility, tier, population, behavior, and reason buffers match the cache reader at sampled indices. Assert no `Session` Python/native object exists after attachment.

- [ ] **Step 6: Run and commit**

Run: `scripts/blender-playback-test.sh` and `scripts/m1-blender-test.sh --only cache-playback`.

Commit: `Add cache-only M1 Geometry Nodes playback`

---

### Task 12: Selected-agent overlay and pinned transform layer

**Files:**
- Create: `crates/crowd-cache/src/override_layer.rs`
- Create: `crates/crowd-cache/tests/override_layer.rs`
- Create: `schemas/override-layer-v1.schema.json`
- Create: `addon/blender_crowd/debug_overlay.py`
- Create: `addon/blender_crowd/overrides.py`
- Create: `tests/blender/test_m1_override.py`
- Modify: `addon/blender_crowd/panels.py`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `crates/crowd-blender/src/lib.rs`
- Modify: `scripts/m1-blender-test.sh`

**Interfaces:**
- Produces `OverrideLayerV1`, `TransformOverride`, `compose_frame`, operators `crowd.inspect_agent` and `crowd.pin_selected_agent`, selected-agent labels/path/velocity overlay.

- [ ] **Step 1: Write failing base-immutability and one-agent targeting tests**

Hash manifest/agents/frame chunks; apply an additive `[1.0, -2.0, 0.5]` transform to one literal stable ID for ticks `30..=60`; assert only that ID changes in-range, no ID changes out-of-range, disabling the layer restores literal base positions, and the base hash is unchanged.

- [ ] **Step 2: Run and observe missing override API failure**

Run: `cargo test -p crowd-cache --test override_layer`
Expected: FAIL because `OverrideLayerV1` and `compose_frame` do not exist.

- [ ] **Step 3: Implement versioned layer parsing, validation, and composition**

Validate schema version, unique layer ID, target existence, ordered inclusive tick range, finite transform samples, and priority. Sort layers by `(priority, layer_id)`; additive operations sum, absolute operations replace earlier values. M1 authoring emits one layer, but deterministic composition is defined now.

- [ ] **Step 4: Implement Blender pin sampling and selected-agent UI/overlay**

The pin operator samples an authored object's world transform over the selected frame range into the layer, then attaches it to playback. The inspect operator reads native cached evidence and displays stable ID, goal/path, desired/solved velocity, state/reason, clip/phase/rate, and portal state; the overlay draws path and two velocity arrows.

- [ ] **Step 5: Run Rust and Blender tests, then commit**

Run: `cargo test -p crowd-cache --test override_layer` and `scripts/m1-blender-test.sh --only override`, including layer enable/disable and base file hash assertions.

Commit: `Add selected-agent evidence and sparse pin overrides`

---

### Task 13: Cache-only Eevee/Cycles render smoke and clean-file walkthrough

**Files:**
- Create: `tests/blender/test_m1_render.py`
- Modify: `scripts/m1-blender-test.sh`
- Create: `scripts/m1-render-test.sh`
- Create: `docs/user/m1-reference-walkthrough.md`
- Modify: `addon/blender_crowd/operators.py`
- Modify: `addon/blender_crowd/panels.py`

**Interfaces:**
- Produces operator `crowd.render_reference_frame`; fresh-process PNG outputs for Eevee and Cycles; measured JSON separating point playback, canonical armature evaluation, and renderer costs.

- [ ] **Step 1: Write the failing render smoke**

Open a factory file, attach a complete cache, generate reference assets, configure a deterministic camera/world/lights, render the same mid-shot frame with `BLENDER_EEVEE_NEXT` and `CYCLES` CPU, and assert each PNG exists, is nonempty, has the expected dimensions, and contains non-background pixels.

- [ ] **Step 2: Run and observe missing render workflow**

Run: `scripts/m1-render-test.sh`
Expected: FAIL because the runner/operator/test output contract does not exist.

- [ ] **Step 3: Implement render operator and separated measurement output**

Measure cache decode+point upload separately, evaluate the small canonical armature fixture in its own timed loop, then time each render engine. Record Blender version, engine/device, image size/samples, cache hash, scene hash, peak memory when available, and output paths. Never sum these into a simulation throughput number.

- [ ] **Step 4: Write and exercise the clean-file walkthrough**

Document installation, Create Reference Concourse, Validate, Bake, Cancel/rebake, Attach, Inspect, Pin, and Render using UI labels and exact headless equivalents. Follow it in a fresh Blender process with no code/JSON edits.

- [ ] **Step 5: Run and commit**

Run: `scripts/m1-blender-test.sh && scripts/m1-render-test.sh`.

Commit: `Render the self-contained M1 cache-only reference shot`

---

### Task 14: M1 evidence, milestone state, and full verification

**Files:**
- Create: `docs/benchmarks/2026-08-10-m1-vertical-slice.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `docs/milestones/README.md`
- Modify: `docs/milestones/M0-proving-grounds.md` only to link final evidence, without rewriting its contract
- Modify: `docs/milestones/M1-vertical-slice.md` only to link final evidence, without rewriting its contract

**Interfaces:**
- Produces: copy-ready full validation commands and a criterion-by-criterion M1 acceptance report.

- [ ] **Step 1: Run every fresh verification command**

```sh
cargo fmt --check
cargo test --workspace
cargo test --release -p crowd-core --test fuzz_density
cargo test --release -p crowd-core --test two_room_reroute -- --ignored
cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p crowd-bench -- check --agents 1000
scripts/cache-experiment.sh
scripts/m0-acceptance.sh
scripts/verify-wheel.sh
scripts/blender-install-test.sh
scripts/blender-playback-test.sh
scripts/m1-bake-test.sh
scripts/m1-blender-test.sh
scripts/m1-render-test.sh
git diff --check
```

- [ ] **Step 2: Build the criterion matrix from actual outputs**

Map M1 criteria 1–8 to input hashes, exact commands, exit codes, measured results, and artifacts. Include exactly 1,000 IDs, strict discrete digest, position bound, completion, boundary escapes, portal result, canceled/complete cache states, channel list, selected-agent evidence, one-agent override/base hash, and separately measured costs.

- [ ] **Step 3: Record limitations and unsupported claims**

Explicitly state procedural proxy versus full armature coverage, single Blender/macOS platform scope, no general behavior graph/groups/queues, no migration promise, no 10K/100K or GPU claim, and any observed quality failure. If any gate fails, mark M1 open and keep the milestone index blocked at that criterion.

- [ ] **Step 4: Update copy-ready commands and milestone status**

Only after all required commands exit zero, update `README.md`, `CLAUDE.md`, and the milestone index to say M0 accepted and M1 accepted. Link reports and walkthrough. Preserve exact runner arguments.

- [ ] **Step 5: Final diff review and commit**

Review `git diff --stat`, `git diff --check`, `git status --short`, and every changed report claim against fresh output. Confirm no generated wheels, caches, renders, local absolute paths, or temporary files are staged unless explicitly designated evidence.

Commit: `Accept the M1 self-contained vertical slice`
