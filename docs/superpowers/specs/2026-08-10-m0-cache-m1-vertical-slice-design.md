# M0 cache closure and M1 1,000-agent vertical slice — design

Date: 2026-08-10
Status: approved design, awaiting written-spec review
Parent contract: [Blender Crowd 1.0 architecture and MVP](../../blender-crowd-1.0.md),
sections 4–12 and delivery phases 0–1
Owning milestones: [M0 — Proving grounds](../../milestones/M0-proving-grounds.md),
[M1 — 1,000-agent vertical slice](../../milestones/M1-vertical-slice.md)
Prior slices: [Deterministic crowd simulation kernel](2026-08-04-crowd-sim-kernel-design.md),
[Avoidance solver comparison](2026-08-06-avoidance-solver-comparison-design.md),
[Blender bridge](2026-08-07-blender-bridge-slice-design.md),
[Tiled navigation](2026-08-08-tiled-navmesh-prototype-design.md)

## 1. Outcome

This slice closes the two remaining M0 implementation gaps, accepts M0 only if
all seven of its criteria pass, and then implements the deliberately narrow M1
vertical slice. The resulting repository can compile one versioned concourse
project, simulate exactly 1,000 deterministic commuters, safely cancel or
finish a recoverable cache, inspect one agent, compose one sparse pinned
transform override, and render from the cache after the simulation session has
been destroyed.

The implementation remains self-contained. Reference commuters, materials,
locomotion actions, and the concourse are generated from checked-in procedural
specifications. No downloaded character, animation, texture, or scene asset is
needed to run the M1 acceptance path.

This is not M2. It does not add a general behavior graph editor, arbitrary
character retargeting, general groups or queues, production layout tools, or a
polished multi-workspace UI.

## 2. Scope

### 2.1 M0 closure

M0 gains:

- a production-shaped cache v0 experiment and selected cache v1 working
  format, distinct from the existing debug-oriented trace v0;
- independently checksummed, atomically published frame chunks;
- explicit `incomplete`, `canceled`, and `complete` cache states;
- transform, stable-ID, animation-placeholder, visibility, and render-tier
  channels with measured quantization and chunk-size choices;
- complete-cache round-trip and sequential-playback tests;
- canceled-cache recovery diagnostics;
- a coarse PyO3 facade for project compilation, session creation, stepping,
  baking, cancellation, cache reading, and selected-agent queries;
- a dated consolidated M0 report that evaluates every acceptance criterion and
  states whether the measured 1K real-time proving budget is met.

### 2.2 M1 vertical slice

M1 gains:

- versioned project, population, semantic-scene, cache, debug, and override
  data contracts;
- minimal Blender Crowd Project settings and operators for creating,
  validating, baking, canceling, attaching, inspecting, overriding, and
  rendering the fixed reference shot;
- a checked-in concourse specification with two spawn regions, three
  destinations, walkable and blocked bounds, two alternative doorway routes,
  and a timed named-portal close/open event;
- deterministic weighted assignment of three commuter archetypes, appearance
  variants, preferred speeds, destinations, scales, and locomotion choices;
- a fixed built-in commuter state machine and animation-state phase;
- cache-only GN playback with stable named attributes;
- readable selected-agent navigation and decision evidence;
- one sparse transform/pin override composed after the base cache;
- self-contained low-poly commuter and locomotion fixtures;
- exact headless runners for validation, bake, strict comparison, cache
  recovery, Blender playback, GN compatibility, override, and render smoke;
- a clean-file user walkthrough and a dated M1 evidence report.

### 2.3 Explicit exclusions

- General behavior graphs, arbitrary graph nodes, blackboard authoring, or a
  node editor. The commuter behavior uses the future compiled runtime boundary
  but exposes exactly one built-in versioned program.
- General group and queue authoring. Those are M2 scope. Doorway capacity and
  deterministic destination assignment are sufficient for the M1 contract.
- Arbitrary rig conversion, retarget profiles, foot IK, motion matching,
  ragdolls, cloth, hair, or per-agent full-armature evaluation.
- General cache migration across released versions. M1 rejects unsupported
  versions clearly; M3 owns migration and compatibility hardening.
- Replacing the selected sampled-velocity solver or the tiled-navigation
  prototype unless an M1 acceptance test exposes a correctness blocker.
- USD, GPU simulation, 10K/100K claims, cloud compute, external publication,
  or Blender-mainline work.

## 3. Repository boundaries

### 3.1 New cache crate

`crates/crowd-cache/` owns the directory cache, manifest types, chunk codec,
checksums, atomic publishing, cancellation finalization, recovery inspection,
sequential reader, and sparse override format. It does not depend on Blender.

The cache crate uses plain record types at its boundary. `crowd-core` does not
depend on `crowd-cache`; the facade and headless bake runner translate a core
snapshot into a cache frame. This keeps the simulator usable without an output
format and prevents a cache dependency cycle.

### 3.2 Core additions

`crowd-core` gains only behavior that is authoritative simulation state:

- `ProjectIrV1`, validation diagnostics, and `CompiledProject`;
- stable population, semantic-entity, variant, and destination identifiers;
- deterministic population expansion from stable spawn ordinals;
- timed portal inputs applied in the existing input phase;
- commuter behavior state and decision-reason codes;
- animation clip, normalized phase, playback rate, visibility, and render tier
  state;
- a real animate phase after integration;
- coarse frame and selected-agent snapshots.

The existing benchmark `SceneDef` API remains available. The M1 compiler may
produce a `CompiledScene` internally, but existing M0 scenes and baselines do
not migrate to JSON merely to accommodate the new path.

### 3.3 Facade and add-on

`crates/crowd-blender` depends on `crowd-core`, `crowd-cache`, and
`crowd-trace`. Trace playback remains supported for the existing M0 evidence.
New cache classes do not silently reinterpret trace files.

The native facade exposes coarse operations and plain versioned data. It never
accepts a Blender pointer and never calls Python once per agent or tick.

The add-on gains focused modules for project extraction, cache playback,
procedural reference assets, debug display, sparse overrides, panels, and
operators. Existing trace playback stays isolated as the legacy proving path.

### 3.4 Versioned schemas and fixtures

The implemented owners create these durable paths:

```text
schemas/
  project-ir-v1.schema.json
  cache-manifest-v1.schema.json
  decision-trace-v1.schema.json
  override-layer-v1.schema.json
assets/reference/
  concourse-project-v1.json
  commuter-assets-v1.json
  README.md
```

The JSON Schemas document external structure and provide golden fixtures. Rust
types remain the executable authority, with tests proving that emitted JSON
validates against the checked-in schema and that the fixtures deserialize.

## 4. Project IR and deterministic compilation

`ProjectIrV1` contains only authoring input, not runtime state:

```text
schema version and project UUID
clock, units, seed, frame range, and strict/fast mode
population UUID, count, spawn-source references, and weighted choices
archetype and appearance logical IDs
spawn, destination, walkable, blocked, and named-portal semantics
timed portal events
fixed commuter-program version
navigation, avoidance, and animation settings
```

Blender Python extracts this plain data from scene property groups. The
checked-in reference JSON exercises the same compiler, so the headless Rust
path and Blender-authored path cannot diverge into separate simulators.

Validation returns ordered diagnostics sorted by severity, stable diagnostic
code, and entity ID. Compilation is blocked by:

- unsupported versions or invalid units/tick rates;
- duplicate project, population, semantic, archetype, or appearance IDs;
- invalid counts, ranges, or non-positive weight totals;
- missing spawn, destination, walkable, blocked, or portal references;
- spawn or destination regions outside the walkable domain;
- destinations unreachable under the initial portal topology;
- contradictory portal events at the same tick;
- unsupported commuter-program or cache-channel requirements.

Stable agent IDs continue to derive from project identity, population identity,
spawn-source identity, and spawn ordinal. Each random concern uses its own
purpose key derived from the stable ID. Archetype, appearance, speed,
destination, scale, and optional jog choice therefore do not depend on vector
iteration order and do not reshuffle when another agent is added.

## 5. Fixed commuter runtime

M1 implements one compiled program, `commuter_v1`, with stable enum values:

```text
UNSPAWNED -> TRAVEL -> ARRIVED
                   -> BLOCKED (diagnostic terminal state)
```

`TRAVEL` owns destination assignment and uses the existing budgeted tiled
navigation plus selected sampled-velocity avoidance. A portal event is an
ordered input; the existing selective invalidation mechanism replans only
corridors that used the changed named door. The decision reason records
`initial_destination`, `follow_corridor`, `portal_closed_replan`,
`portal_reopened`, `destination_reached`, or a stable failure code.

The animate phase maps authoritative kinematics to:

- `idle` while unspawned, arrived, or stationary;
- `walk` for ordinary solved speeds;
- `jog` above the configured deterministic speed threshold;
- a stable clip ID per locomotion state and archetype;
- normalized phase advanced from solved distance and clip stride length;
- bounded playback rate derived from solved speed;
- root orientation derived from solved velocity with the existing stable
  fallback when speed is near zero.

Simulation trajectory remains authoritative. Clip metadata never moves an
agent or changes avoidance.

## 6. Cache v0 experiment and cache v1 format

### 6.1 Experiment matrix

Before selecting cache v1 defaults, the M0 runner bakes the same 1,000-agent
input across:

- 30-, 60-, and 120-tick chunks;
- raw `f32` positions;
- signed millimeter `i32` positions;
- per-chunk affine `i16` positions with declared bounds.

The report records total bytes, write time, sequential-read time and frames/s,
peak resident memory where the runner can measure it, maximum positional
error, cancel latency, and recovered-chunk count. Recorded runs use identical
discrete channels and simulation state. The chosen default is the smallest
encoding that remains within a declared 1 mm maximum position error and does
not make sequential playback slower than the raw baseline by more than 10% on
the recorded M0 workstation. The measured result, not this design, selects the
winning chunk size and position encoding.

### 6.2 Working directory layout

```text
shot.crowd/
  manifest.json
  agents.bin
  frames/
    000000-000059.chunk
    000060-000119.chunk
  debug/
    agents.json
  overrides/
    pinned-hero.layer.json
  metrics.json
```

`manifest.json` includes schema and engine versions, project and scene IDs,
source content hash, frame/tick ranges, units, agent/population counts, global
seed, determinism mode, channel declarations, quantization bounds, chunk index,
checksums, logical asset references, build/platform provenance, and cache
status.

`agents.bin` contains stable per-agent data once: stable ID, population ID,
archetype ID, appearance/variant ID, base scale, and spawn ordinal. Frame chunks
contain varying channels in channel-major order:

- position and orientation;
- animation clip ID, normalized phase, and playback rate;
- behavior state and decision-reason code;
- destination ID;
- velocity;
- visibility and render tier.

Every binary file starts with a fixed magic, endian marker, schema version,
record/tick counts, payload length, and CRC-32C checksum. Checksums cover the
encoded payload. Readers reject a wrong magic, version, endian marker, declared
size, or checksum and name the failing file. Project/source content hashes use
BLAKE3. The implementation records the exact crate versions, licenses, and
reasons for both algorithms before accepting them as dependencies.

### 6.3 Atomic completion and cancellation

The writer creates the target directory with an atomically replaced manifest
whose initial status is `incomplete`. Each frame chunk is written to a sibling
temporary file, flushed, and renamed into place before the manifest may index
it. Cancellation finishes the current bounded write operation, removes no
completed chunk, writes status `canceled` plus the last complete tick and
reason, and returns a bake report.

Only successful finalization after every declared file passes a read-back
header/checksum check may publish status `complete`. A complete reader rejects
an incomplete or canceled cache. A recovery inspector can open those states,
list valid chunks, and report the readable tick range without presenting the
cache as render-ready.

No in-place cache repair is part of M1. A corrupt or missing complete-cache
chunk is an error that names the chunk and requires rebaking.

## 7. Coarse Rust/Python facade

The PyO3 module adds this conceptual API while retaining the existing `Trace`:

```python
compiled = native.compile_project(project_ir_json)
diagnostics = compiled.diagnostics()
session = compiled.create_session(seed=42, mode="STRICT")
session.step(tick_count=30)
snapshot = session.query_agent(agent_id)
report = session.bake(cache_path, tick_start, tick_end, cancel_token)

cache = native.Cache(cache_path, require_complete=True)
buffers = cache.read_tick(tick)
debug = cache.query_agent(agent_id, tick)
```

`CancelToken` owns an atomic flag. Bake releases the GIL while core simulation,
encoding, and file I/O run. The Blender bake operator starts a worker thread
that performs native-only work, while a modal timer on Blender's main thread
polls progress without touching the simulation. The cancel operator only sets
the token. No worker thread accesses `bpy`, and no Python callback enters the
per-agent or per-tick hot loop.

Native methods translate all validation, I/O, cancellation, corruption, and
version failures into stable Python exception classes with diagnostic codes.
Bulk frame reads return flat buffers shaped for `numpy.frombuffer` and Blender
`foreach_set`; there is no per-agent Python marshaling.

## 8. Blender authoring and playback

### 8.1 Narrow workspace

M1 exposes one panel with project health and these operators:

1. Create Reference Concourse.
2. Validate Project.
3. Bake Crowd Cache.
4. Cancel Bake.
5. Attach Cache.
6. Inspect Selected Agent.
7. Add/Update Pinned Override.
8. Render Reference Frame.

Scene property groups store only the project settings and stable references
needed by `ProjectIrV1`. Operators report actionable diagnostics and never ask
the user to edit Python, Rust, or JSON. The checked-in walkthrough starts from
a clean Blender file and exercises these operators.

### 8.2 Cache-only presentation

`CachePlayback` owns only a cache reader, Blender point cloud, frame mapping,
and optional override layer. It does not hold or recreate a simulation session.
The cache-only test destroys the session, opens a fresh Blender process, loads
the cache, seeks multiple nonsequential frames, and renders.

The versioned GN contract uses prefixed names. Because Blender point integer
attributes are 32-bit, stable 64-bit values use documented low/high halves:

```text
crowd_agent_id_lo / crowd_agent_id_hi
crowd_population_id
crowd_position / crowd_orientation / crowd_scale
crowd_variant_id
crowd_clip_id / crowd_clip_phase / crowd_playback_rate
crowd_behavior_state / crowd_decision_reason
crowd_render_tier / crowd_visible
```

Existing trace playback retains its legacy unprefixed attribute names. The new
cache reader does not silently change that already-recorded M0 fixture.
`crowd_population_id`, `crowd_variant_id`, clip/state/reason codes, and render
tiers are deterministic 32-bit cache-table indices; manifests map them back to
their stable logical IDs. Stable agent identity alone requires the documented
64-bit split.

### 8.3 Self-contained procedural fixtures

`commuter-assets-v1.json` describes three visibly distinct low-poly humanoid
proportions, deterministic material palettes, and idle/walk/jog metadata.
Blender Python generates the meshes, simple canonical armatures, and actions.
Generated content is reproducible from the JSON and checked-in code; generated
`.blend` output is a test product, not a source fixture.

The 1,000-agent GN representation uses instanced low-poly commuters selected by
`crowd_variant_id`. Clip phase drives a lightweight procedural arm/leg swing so
per-agent phase is visible without expanding 1,000 armature object graphs. A
small canonical armature fixture verifies the same clip metadata and measures
Blender armature evaluation separately. The evidence report does not describe
the procedural proxy cost as full 1,000-character armature cost.

The reference runner emits both Eevee and Cycles smoke images on the declared
Blender 5.2/macOS platform. A smoke render proves cache-to-render continuity;
it is not used as evidence of simulation correctness or production visual
quality.

## 9. Selected-agent evidence and sparse override

The core snapshot and cache debug record expose, for one stable ID and tick:

- position, desired and solved velocity;
- current corridor tile/portal sequence and next target;
- destination and path status;
- commuter state, clip, phase, and playback rate;
- current portal states relevant to the route;
- last stable decision-reason code and diagnostic text.

The Blender panel renders these fields as readable labels and draws the path,
next target, and desired/solved velocity for the selected stable ID. Debug data
is bounded to selected or explicitly requested agents; final-cache consumers
need not load it for ordinary playback.

`OverrideLayerV1` is separate from the base cache and contains layer identity,
author, timestamp, priority, target stable ID, frame range, operation, and
transform samples. M1 implements one operation: an absolute or additive
transform over a frame range. Blender's pin operator samples an authored
object into those transform records when it writes the layer, so later cache
playback does not require a live simulation or a live constraint. Composition
occurs after base-cache decode and before Blender buffers are returned. A test
hashes the base-cache manifest, agent table, and frame chunks before and after,
proves exactly one agent changes, and proves disabling the layer restores the
original playback.

## 10. Failure model

Every failure names the responsible entity or file and a corrective action.
Stable categories are:

- project/schema validation failure;
- unreachable navigation or invalid portal event;
- missing or mismatched logical asset;
- unwritable output path;
- canceled bake;
- incomplete cache opened as complete;
- missing, truncated, or checksum-failing chunk;
- stale source-content hash;
- unsupported cache or override version;
- selected stable ID absent at the requested tick.

Warnings such as excessive density, clip-speed mismatch, low destination
completion, or high playback cost remain visible in reports but do not masquerade
as structural validity. Acceptance thresholds decide whether M1 passes.

## 11. Acceptance thresholds

M0 remains open until its existing seven acceptance criteria are re-evaluated
in one dated report. In particular, a canceled cache must be readable only as
incomplete/canceled, a complete cache must pass round trip and sequential
playback, the coarse facade must be exercised through the bundled ABI, and the
report must state the measured 1K real-time result without extrapolating 10K or
100K behavior.

M1 uses these fixed gates:

1. Exactly 1,000 agents spawn with unique stable IDs. Strict rebakes reproduce
   every ID, archetype, appearance, destination, behavior state, clip, render
   tier, and visibility value exactly.
2. Decoded positions from strict rebakes differ by no more than the cache v1
   format's selected and reported quantization bound, which may not exceed
   1 mm.
3. At least 95% of agents reach a valid destination by the declared end tick;
   no agent crosses a blocked/static boundary; the portal event causes bounded
   rerouting; and agents whose corridors do not use the changed door retain
   their unrelated routes.
4. A canceled bake never reports complete. A completed cache validates every
   declared checksum, plays sequentially and nonsequentially, and renders in a
   fresh Blender process with no simulation session.
5. Every required identity, transform, animation, behavior, visibility, and
   tier channel survives the cache round trip.
6. A selected agent exposes the navigation, velocity, state, animation, portal,
   destination, and decision evidence listed in section 9.
7. The pinned override changes exactly one stable ID over its declared frame
   range, leaves the base-cache hash unchanged, and is reversible by disabling
   the layer.
8. The report records simulation, cache writing, sequential cache reading,
   Blender point playback, canonical armature evaluation, peak memory, cache
   size, Eevee render, and Cycles render costs separately.

The first accepted baseline may tighten later regression limits, but a failed
gate above is recorded as a failed milestone. The implementation does not lower
a threshold after observing a failure.

## 12. Test strategy

### 12.1 Test-first implementation

Every new behavior begins with a focused failing test that fails because the
behavior is absent. Production code is added only after the failure is observed.
Each slice finishes with its focused tests and the broader workspace suite
green before the next behavior begins.

### 12.2 Rust unit and property tests

- Project IR parsing, ordered diagnostics, stable ID/variant independence, and
  content hashing.
- Commuter transitions, portal-event ordering, animation state, phase advance,
  and decision-reason codes.
- Each cache codec, its measured error bound, binary headers, size checks,
  checksum test vectors, chunk indexing, and random round trips.
- Atomic manifest replacement, cancellation, incomplete recovery, corruption,
  unsupported versions, sequential and nonsequential reads.
- Sparse override targeting, frame bounds, priority, reversibility, and base
  immutability.

### 12.3 Cross-layer integration tests

- Reference project compilation and fast reduced-agent bake.
- Release 1,000-agent strict rebake comparison.
- Timed portal close/open and unrelated-route preservation.
- Complete/canceled cache behavior and deliberate chunk corruption.
- Cache v0 experiment runner and machine-readable comparison report.
- PyO3 compile/session/bake/cancel/cache/query round trip in plain CPython.

### 12.4 Blender tests

- Clean extension installation and native module import.
- Clean-file reference-project creation and validation.
- Procedural asset/clip generation and logical-ID validation.
- 1,000-point cache attachment and every GN attribute's type/count.
- Cache-only seek/playback in a fresh process.
- Selected-agent labels and debug geometry.
- One-agent pinned override and unchanged base cache.
- Eevee and Cycles render smokes with nonempty output.

### 12.5 Full validation set

Implementation adds exact copy-ready commands to `README.md` and `CLAUDE.md`
when the runners exist. The completion pass includes:

```sh
cargo fmt --check
cargo test --workspace
cargo test --release -p crowd-core --test fuzz_density
cargo test --release -p crowd-core --test two_room_reroute -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
scripts/verify-wheel.sh
scripts/blender-install-test.sh
scripts/blender-playback-test.sh
```

It also includes the new cache experiment, M0 acceptance, M1 headless bake,
strict comparison, cache recovery, facade, Blender cache-only playback,
override, and render-smoke runners under their final checked-in names.

## 13. Evidence and documentation

The implementation writes:

- a cache format/schema reference and dependency/license decision record;
- a clean-file M1 walkthrough;
- a dated cache experiment report;
- a consolidated dated M0 report evaluating criteria 1–7;
- a dated M1 report containing environment, input hashes, thresholds, results,
  known failures, unsupported claims, and the next gate.

Reports distinguish fresh measured results from design targets and preserve the
project's existing timing discipline: sampled, traced, serialized, Blender
playback, armature, and render measurements are never quoted as isolated
simulation throughput.

## 14. Implementation order

The implementation plan will decompose this design into independently testable
slices in this order:

1. Project schemas and deterministic compilation.
2. Commuter/animation state and selected-agent snapshots.
3. Cache primitives, cancellation, recovery, and experiment runner.
4. Cache bake integration and M0 evidence closure.
5. Coarse PyO3 facade and plain-CPython round trip.
6. Blender authoring properties and operators.
7. Cache-only GN playback and procedural assets.
8. Debug evidence and sparse pinned override.
9. Headless reference bake, strict comparison, and render smokes.
10. M1 walkthrough, evidence report, and complete validation.

M1 implementation cannot begin by bypassing a failed M0 acceptance criterion.
If the cache experiment, recovery behavior, facade, or consolidated report does
not pass, the plan stops at M0 and records the failed gate.
