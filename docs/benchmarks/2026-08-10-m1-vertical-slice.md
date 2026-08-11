# M1 1,000-agent vertical-slice acceptance

Date: 2026-08-10

Milestone: [M1 — 1,000-agent vertical slice](../milestones/M1-vertical-slice.md)

Walkthrough: [M1 reference concourse](../user/m1-reference-walkthrough.md)

## Decision

**M1 is accepted. All eight acceptance criteria pass, and M2 is unblocked.**

The same checked project compiles to exactly 1,000 stable agents, simulates for
10,000 ticks, produces two independently baked caches with identical declared
state, survives cancellation, plays in fresh Blender processes without a live
simulation session, supports a reversible one-agent override, and renders the
same cache-only frame with Eevee GPU and Cycles CPU.

M0 was rerun after the M1 implementation. Its complete ten-step acceptance
runner passed in 1,488.151 seconds; the refreshed machine summary is linked from
the [M0 consolidated report](2026-08-10-m0-consolidated.md).

## Environment and input identity

| Field | Accepted value |
|---|---|
| CPU / RAM | Apple M1 Max / 68,719,476,736 bytes |
| OS / architecture | macOS 27.0 / arm64 |
| Blender | 5.2.0 LTS, hash `fbe6228777e7` |
| Blender Python | CPython 3.13 through the installed extension |
| Runner Python | 3.14.2 |
| Rust / Cargo | 1.94.1 / 1.94.1 |
| Reference project ID | `6b5ad627-360b-4c58-9df5-52e306cf20d6` |
| Compiled source hash | `cfeb0ae7bb4ae1c651e7d3f6614453dad6d1d34b808ff42292cba3af5927fb74` |
| Static-agent digest | `4076b0e828eaf990e502abd020751cca5c7938cd1c989b306a653061874f8988` |
| Dynamic discrete digest | `80737b41bc70d8cbe3b70c3dd23721ec72ad0359bdcbbc0d4832ef7e21fd247a` |
| Render cache-manifest hash | `619eecf7925fc7423ff9d708804dc4eb65261790a82cf35d14ef4c5058028e4a` |
| Render scene hash | `554b6cb1b767cf174d9619f9a9b402e1bb3de0bd92ab175289165b58d67edc8a` |

The reference assets are redistributable procedural fixtures bundled with the
extension: four meshes, four materials, one canonical proxy armature, and
idle/walk/jog actions. Headless compilation and Blender project creation both
consume the same versioned project identity and source hash.

## Exact acceptance commands

Every command below exited zero in the final verification pass. Blender was run
with normal host graphics access; the render output directory is replaceable.

```sh
scripts/m0-acceptance.sh
cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture
scripts/m1-bake-test.sh
scripts/m1-blender-test.sh
scripts/m1-render-test.sh --out /private/tmp/blender-crowd-m1-final-render
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

The complete M0 runner includes workspace tests, release density stress, the
1,000-agent reroute, six solver baselines, cache lifecycle and codec selection,
wheel/facade verification, clean Blender installation, 1,000-point playback,
formatting, lint, runner tests, and documentation checks.

## Fixed gates and measured result

| Gate | Required | Measured | Result |
|---|---:|---:|---:|
| Compiled agents / unique stable IDs | exactly 1,000 / 1,000 | 1,000 / 1,000 | PASS |
| Strict bake range | ticks 0–9,999 | ticks 0–9,999 | PASS |
| Destination completion | at least 95% | 960/1,000, 96% | PASS |
| Static-boundary escapes | 0 | 0 | PASS |
| Strict discrete/static rebake | exact | exact; matching digests | PASS |
| Strict continuous rebake | position delta within declared bound | 0.0 m observed; 0.000469699 m declared bound | PASS |
| Portal close/reopen | affected routes replan; unrelated routes unchanged | 65 affected, 55 unrelated, all recovered by tick 913 | PASS |
| Canceled cache | recoverable but rejected as complete | 2 valid chunks through tick 136; complete reader rejected | PASS |
| Cache-only Blender playback | no live `Session` | fresh-process reader and GN playback, no `Session` | PASS |
| Sparse override | one target only; reversible; immutable base | one stable ID, inclusive ticks 30–60, base hash unchanged | PASS |
| Render smoke | Eevee and Cycles CPU from the same completed cache | both PNGs validated at 320×180 | PASS |

The timed portal closes at tick 600 and reopens at tick 900. All 65 routes that
used `east_gate` before closure were invalidated and recovered; all 55 routes
that did not use it remained unchanged. This proves topology isolation as well
as eventual recovery.

## Cache and channel proof

Each completed strict cache is 560,057,053 bytes for 10,000 ticks and 1,000
static agent slots. Two independent runs agree on every static and discrete
channel. All continuous channels other than quantized position agree exactly;
positions also had 0.0 m cross-cache delta, inside the declared per-cache
0.000469699 m reconstruction bound.

Fresh-process Blender playback sampled ticks `0`, `913`, `4999`, `9999`, and
the cancellation boundary `137`. It reconstructed the full v1 presentation
contract:

- position, orientation, scale, and velocity;
- stable ID low/high words, population, archetype, variant, and spawn ordinal;
- clip, phase, playback rate, behavior state, decision reason, and destination;
- visibility and render tier.

The Geometry Nodes object received stable named attributes and static agent data
from `agents.bin`; no simulation session or per-agent Python simulation loop was
present in the playback or render processes.

## Selected-agent and override proof

Stable agent `2506968674689638394` was captured at tick 599. Its evidence names
the `travel` commuter state, `following planned corridor` decision, destination
2, current/desired/solved velocity, next target, full corridor, relevant portal
state, clip 1, phase, and playback rate. Blender presents the cached corridor
and desired/solved velocity overlay from that record.

The pin test adds `[1.0, -2.0, 0.5]` to only that stable ID from ticks 30 through
60. Disabling the layer restores base playback, and the base cache retains
BLAKE2b-256 `04bff650460a593300950f234ca1e93e55f9a10571497d561e417f26f8301823`.
Layer ordering is deterministic by priority and logical ID; override data lives
beside the cache and never rewrites its manifest, agent table, or frame chunks.

## Separated costs

The two strict rebakes deliberately record simulation, cache write, and
sequential cache read separately:

| Measurement | First bake | Second bake |
|---|---:|---:|
| Native simulation, 10,000 ticks | 13.429 s | 13.492 s |
| Cache write | 2.837 s | 3.059 s |
| Sequential full-cache read | 0.803 s | 0.802 s |
| Cache size | 560,057,053 bytes | 560,057,053 bytes |

The final standalone render run used reference tick 4,999, when 700 staggered
commuters were visible. It measured presentation costs independently:

| Measurement | Result |
|---|---:|
| Cache decode plus point upload | 0.00485 s |
| Canonical single-armature evaluation loop | 0.00144 s |
| Eevee, GPU, 16 samples | 0.50545 s |
| Cycles, CPU, 4 samples | 0.05709 s |
| Peak Blender resident memory | 425,394,176 bytes |

These five figures were remeasured on 2026-08-10, after the original run was
found to be wrong and after the reference camera was reframed; see
[Corrections](#corrections-2026-08-10). The cache manifest hash is unchanged.
The render scene hash changed with the camera and is stated above.

The tiny smoke renders use different engines and sample counts; their wall
times are not a quality-normalized renderer comparison. None of these values is
added to, or presented as, isolated simulation throughput.

## Corrections (2026-08-10)

The first version of this report published five presentation figures that were
measured wrongly. Both defects are fixed, the reference camera was reframed, the
render run was repeated, and the tables above carry the remeasured values. The
record of what was wrong stays here rather than being quietly overwritten.

**The reference images were drawn at tick 1, not tick 4,999.** `CachePlayback`
registers a `frame_change_post` handler, and `bpy.ops.render.render()` fires it.
The workflow positioned playback with `sync_to_tick(4999)`, measured 700 proxy
instances there, and then rendered — at which point the handler re-synced
playback to `frame_current`, which was still 1. The reported tick and instance
count were honest measurements of a state that was discarded before a pixel was
drawn. The published Eevee and Cycles times were therefore timings of a nearly
empty concourse, and understated the real cost.

`render_reference` now positions through the scene frame, the shipped playback
path, and raises if the frame does not land on the reference tick. The metrics
gained `rendered_tick` and `post_render_proxy_instance_count`, both measured
after the renders rather than before, and `tests/blender/test_m1_render.py`
requires `rendered_tick` to equal `reference_tick`. The previous test could not
catch this: it only asserted that a render contained more than 100
non-background pixels, which an opening frame satisfies.

**The armature figure was mostly cache decoding.** `_measure_armature` steps the
timeline 31 times, and with the handler attached each step decoded a cache tick.
The published 0.13464 s was therefore a measurement of 31 cache reads plus
armature evaluation, in a report whose stated purpose is to keep those costs
apart. The loop now runs under `cache_playback.suspended_frame_sync()`, and the
isolated cost is 0.00122 s.

**The reference camera was reframed.** The concourse ground is 60 x 20, roughly
3:1, against a 16:9 frame. Viewing it square-on from the south left the crowd in
a band across the middle with dead space above and below. The camera now looks
along the long axis from the south-west corner, at `(-10, -22, 17)` with a 42 mm
lens aimed at `(32, 11, 1)`, which runs the concourse up the frame diagonal and
keeps near agents large enough to read. This changes the render scene hash from
`9b4dff120c5b4a95ab9e97abb476d89a994cc0d464bd53194d7df5da9b1d04f1` to the value
stated in the environment table. It is a framing change only: the cache, the
tick, and the 700 instances rendered are the same.

**The proxy gait tipped commuters over.** The node group fed its walk-cycle
swing straight into the instance rotation's X component, which is in radians,
with an implied amplitude of 1.0 rad. Every moving commuter therefore pitched
through +/-57.3 degrees, and at the reference tick 47 of the 109 moving agents
were leaning past 45 degrees, which reads as agents lying on the floor. The
amplitude was also invented rather than read: `commuter-assets-v1.json` declares
`swing_radians` per clip, 0.0 idle, 0.55 walk, 0.9 jog, and the node group
ignored all three, so the graph and the canonical rig disagreed about the same
clip. That declared value is how far a limb travels, not how far a body leans,
so the fixture now declares both: `body_swing_radians` is 0.0 idle, 0.08 walk,
0.14 jog, and the node group selects each by clip ID. Neither consumer derives
one amplitude from the other or invents its own. The worst instance lean at the
reference tick is now 4.58 degrees, and `tests/blender/test_m1_cache_playback.py`
measures the worst lean off the evaluated instance transforms and fails past 15
degrees.

No simulation, cache, determinism, portal, override, or channel result is
affected: all of those come from the Rust kernel and the cache readers, not from
the render workflow or the node group. The remeasured renders are slower than
the retracted ones, and the remeasured armature figure is faster.

## Acceptance criteria 1–8

| # | Evidence | Result |
|---:|---|---|
| 1 | `scripts/m1-blender-test.sh` repeatedly builds the abi3 wheel and archive, removes any prior extension, clean-installs/enables it, validates bundled assets, and creates the same typed concourse twice with stable data-block counts. | PASS |
| 2 | `m1_strict` and `m1-bake-test.sh` compile exactly 1,000 unique IDs and compare two complete 10,000-tick caches: exact static/discrete digests and 0.0 m observed position delta. | PASS |
| 3 | The concourse reaches 96% completion with zero boundary escapes; portal closure affects exactly the routes using it, preserves unrelated routes, and fully recovers after reopen. | PASS |
| 4 | Cancellation publishes two valid chunks and a canceled manifest that recovery can inspect but the complete reader rejects. Fresh Blender processes play and render the completed cache with no live simulation session. | PASS |
| 5 | Cache-only Blender tests verify all transform, identity, animation, behavior, visibility, and tier channels at representative and boundary ticks. | PASS |
| 6 | The selected-agent record and Blender overlay expose path, portal, state, decision, target, desired/solved velocity, clip, phase, and playback evidence. | PASS |
| 7 | The hero pin changes one stable ID only over an inclusive tick range, disables reversibly, composes deterministically, and leaves the base cache hash unchanged. | PASS |
| 8 | Native simulation, cache write/read, point upload, canonical armature evaluation, resident memory, cache size, Eevee, and Cycles CPU are recorded as distinct measurements. | PASS |

## M0 regression closure

M1 adds commuter and animation columns to the structure-of-arrays world state.
The final M0 baseline run therefore measured and reviewed two narrow allocation
changes: `bidirectional_corridor` increased from 354,078 to 418,139 bytes and
`bottleneck` from 419,368 to 483,363 bytes. All zero-tolerance quality metrics
passed; the other four selected-solver scenes required no baseline change. The
complete post-change M0 runner then passed all ten gates.

## Known limitations and unsupported claims

- The reference crowd uses procedural proxy variants. The 700 visible instances
  at the mid-shot frame are not 700 or 1,000 independently evaluated production
  armatures; the single canonical armature timing is intentionally separate.
- Forty agents have not completed by tick 9,999. The measured 96% result clears
  the fixed 95% M1 gate but is not represented as perfect flow quality.
- Evidence covers one Apple M1 Max, macOS arm64, Blender 5.2 LTS, and the bundled
  fixtures. Linux, Windows, other Blender versions, arbitrary rigs, and studio
  assets are not yet accepted.
- M1 implements one fixed `commuter_v1` program. It does not provide a general
  behavior graph, production groups or queues, motion matching, ragdolls, USD,
  or arbitrary rig conversion.
- Cache v1 rejects unsupported versions and corruption. No released-version
  migration promise or in-place cache repair is made.
- No 10,000- or 100,000-agent simulation, cache, playback, memory, or render
  claim is made. Eevee GPU rendering is not GPU crowd simulation.
- The 320×180 low-sample renders are deterministic integration smoke tests, not
  production image-quality or renderer-throughput benchmarks.

## Next gate

M2 may begin. It must prove the [authorable MVP](../milestones/M2-authorable-mvp.md)
with non-developer authoring, editing, validation, and correction workflows; it
may not reinterpret this procedural vertical slice as production support.
