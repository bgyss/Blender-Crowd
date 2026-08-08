# Blender bridge and native packaging (M0 item 6) — design

Date: 2026-08-07
Status: approved design, ready for implementation planning
Parent contract: [Blender Crowd 1.0 architecture and MVP](../../blender-crowd-1.0.md), sections 9 and 13
Owning milestone: [M0 — Proving grounds](../../milestones/M0-proving-grounds.md)
Prior slices: [Deterministic crowd simulation kernel](2026-08-04-crowd-sim-kernel-design.md),
[Avoidance solver comparison](2026-08-06-avoidance-solver-comparison-design.md)

## 1. Why this slice, and why now

The user asked to begin M1. M1 cannot begin: the milestone index declares M0
through M3 strict ordered gates, and M1's stated prerequisite is an accepted
M0. Of M0's seven in-scope items, three are implemented (fixed-step kernel,
benchmark scenes, avoidance solver selection) and four are not (tiled
navigation, cache v0, Blender bridge and packaging, Python/Rust facade).

Of the four remaining, the bridge carries the most architectural risk. If a
Rust native module cannot be loaded by a stock Blender install, or if 1,000
cached transforms cannot be pushed into Geometry Nodes at an acceptable cost,
then the project's central premise — Rust simulates, Blender presents — is
wrong, and it is wrong in a way that invalidates the cache and navigation work
too. This slice is therefore ordered first so that it fails early and cheaply
if it is going to fail at all.

## 2. Target host

The canonical contract's line 4 states "Target host: Blender 4.x LTS-compatible
API surface". That is superseded: **the target host is Blender 5.2 LTS only**.
Amending line 4 of the canonical contract is part of this slice.

Verified on the development machine, 2026-08-07:

| Property | Value |
|---|---|
| Blender | 5.2.0 LTS, build hash `fbe6228777e7`, built 2026-07-14 |
| Bundled CPython | 3.13.13 |
| Native extension suffix | `.cpython-313-darwin.so` (SOABI `cpython-313-darwin`) |
| Bundled numpy | 2.3.4 |
| User extensions directory | `~/Library/Application Support/Blender/5.2/extensions/user_default` |
| Bundled-wheel install directory | `~/Library/Application Support/Blender/5.2/extensions/.local/lib/python3.13/site-packages` |

Widening to further Blender versions is a deliberate M3 support-matrix decision
requiring evidence, not a default.

## 3. Scope

### 3.1 In scope

- `crates/crowd-trace`: trace v0 format, writer, and reader. No pyo3, no bpy.
- `crates/crowd-blender`: PyO3 extension module wrapping `crowd-trace`.
- `addon/blender_crowd/`: Blender 5.2 extension package, manifest, operators,
  point-cloud sync, and a Geometry Nodes asset instancing 1,000 points.
- `crowd-bench --trace <path>`: emit a trace v0 file from an existing run.
- Checked-in headless runners for clean install, native module load, and
  1,000-point playback.
- A dated bridge report under `docs/benchmarks/`.
- Amending the canonical contract's target-host line.

### 3.2 Explicitly out of scope

Cache v0 proper (chunking, quantization, checksums, cancellation, incomplete
state); tiled navigation; behavior UI; character assets; render tiers beyond a
single stub channel; hero or sparse overrides; Linux and Windows packaging; any
performance claim beyond the single recorded machine; and any claim that trace
v0 is the cache format.

## 4. Component boundaries

| Unit | Responsibility | Depends on |
|---|---|---|
| `crowd-trace` | Trace v0 write/read, version checking. Pure Rust. | `crowd-core` types |
| `crowd-blender` | PyO3 module: open a trace, expose counts, fill caller buffers. | `crowd-trace` |
| `addon/blender_crowd` | Extension package: manifest, operators, point-cloud sync, GN asset. | the built wheel |
| `crowd-bench` | Gains `--trace` to emit trace v0. | `crowd-trace` |

The invariant that keeps the FFI layer auditable: **`crowd-blender` decides
nothing.** It performs no simulation, applies no policy, and holds no state
beyond an open file handle and its parsed header. Any behavior requiring a
decision belongs in `crowd-core` or in the addon. This keeps the unsafe surface
small enough to review, and lets `crowd-trace` be tested with no Python in the
loop.

## 5. Trace v0 format

Deliberately the simplest thing that proves the claim.

Header (fixed size, little-endian): magic `CRWDTRC0`, `format_version: u32`,
`tick_count: u64`, `agent_count: u32`, `ticks_per_second: u32`,
`world_to_meter: f32`.

Per-agent record, tick-major, fixed stride, little-endian:

| Field | Type | Notes |
|---|---|---|
| `agent_id` | `u32` | stable ID from `crowd_core::ids` |
| `position` | `[f32; 2]` | world units |
| `orientation` | `f32` | radians; agents are upright on a walkable surface |
| `flags` | `u32` | active/arrived, matching what `frames.rs` already distinguishes |
| `clip_index` | `u16` | stubbed, written as a fixed default |
| `phase` | `f32` | stubbed |
| `playback_rate` | `f32` | stubbed |
| `render_tier` | `u8` | stubbed |

Records are **packed, not padded**: stride is exactly 31 bytes, and readers
must not assume natural alignment. Uncompressed. No chunking, no quantization,
no checksums. **Those four
omissions are the point**: they are precisely the cache v0 design decisions
that require their own measured review, so trace v0 leaves them open rather
than settling them by accident.

The four animation channels are stubbed rather than omitted. No animation
system exists to populate them, but M1 acceptance criterion 5 requires clip,
phase, playback rate, orientation, variant, visibility, and render tier to
survive a cache round trip. Carrying clip, phase, playback rate, and render
tier at full width now proves the reader, the numpy buffer path, and the GN
attribute plumbing for a representative mix of integer and float channels while
the format is still free to change. Variant and visibility are deliberately
absent: both depend on the archetype and appearance system, which does not
exist, and stubbing them would prove nothing the four stubbed channels do not
already prove. The cost is roughly 11 bytes per agent per
tick of zeros.

`format_version` is validated on read; a mismatch is a hard error. When cache
v0 supersedes this format, old files fail loudly instead of being misread.

## 6. Packaging

`maturin` builds `crowd-blender` into a wheel; the wheel is listed under
`wheels` in `blender_manifest.toml`; `blender --command extension build`
produces the installable zip.

The wheel is built **abi3** (`pyo3/abi3-py311`). This was verified rather than
assumed — see section 8.

### 6.1 Constraints discovered by the packaging probe

1. **Bundled wheels unpack to a shared location.** They land in
   `extensions/.local/lib/python3.13/site-packages/`, common to every installed
   extension, not a per-extension directory. Two extensions bundling the same
   distribution name collide. The Python module name is therefore
   `blender_crowd_native` — distinctive enough not to collide — and must never
   be a generic name like `crowd` or `core`.
2. **`--output-dir` must already exist.** `extension build` fails with a bare
   `Errno 2` rather than creating it. Build scripts must `mkdir -p` first.
3. **`extension remove` takes `repo.pkg_id` as one positional argument**
   (`user_default.blender_crowd`), not a `--repo` flag.
4. `--split-platforms` exists and requires `platforms` to be declared in the
   manifest. Out of scope here, but it is how multi-platform shipping works
   later without one oversized archive.

### 6.2 Rejected alternatives

- **cdylib + ctypes, no PyO3** — the approach used by
  [hallr](https://github.com/eadf/hallr), the only shipped Rust Blender add-on
  found, which uses `crate-type = ["cdylib", "rlib"]`, a hand-rolled
  `build_script.py`, and a legacy ZIP add-on rather than the extension system.
  Rejected because the hot path hands large flat buffers to numpy for
  `foreach_set`; hand-rolled marshalling of 1,000×N floats per tick is where
  defects would concentrate, and PyO3's buffer support exists precisely for it.
- **Out-of-process server** — the approach used by
  [toxicblend.rs](https://github.com/eadf/toxicblend.rs), since abandoned; its
  own README states the intent to "remove the gRPC layer and publish as a
  Python package using the Rust binaries directly." Rejected for the per-tick
  latency and for the operational burden of a second process.
- **Hand-rolled wheel assembly without maturin** — demonstrated viable during
  the probe, but it means owning platform tags, `RECORD` hashes, and abi3
  correctness by hand for no benefit beyond one fewer build dependency.
- **cp313-specific wheel** — correct for a 5.2-only target, but abi3 costs one
  Cargo feature and prevents silent breakage on a Blender built against a newer
  CPython.

Note that no prominent Rust Blender add-on was found shipping through the
extension-plus-wheel path. The ecosystem is converging on in-process native
code, but this slice is early to that path. This is a recorded risk, not a
blocker.

## 7. Playback path

Verified working on Blender 5.2 (see section 8):

```python
pc = bpy.data.pointclouds.new("crowd")
pc.resize(agent_count)
pc.attributes["position"].data.foreach_set("vector", positions_f32)
```

Custom `FLOAT` and `INT` point-domain attributes accept numpy buffers via
`foreach_set`, and a `NODES` modifier attaches to the resulting object. Note
that the API is `.resize(n)`, not `.add(n)`.

Per-frame, the addon reads one tick from the trace into preallocated numpy
arrays and pushes them to point attributes. Instancing and orientation are
handled by the GN asset, which is presentation only and never authoritative.

## 8. Evidence already gathered

All of the following was executed against the installed Blender 5.2.0 LTS on
2026-08-07, before this design was accepted:

1. **Host facts** (section 2) read from a headless
   `--factory-startup --python-expr` session.
2. **abi3 wheels load correctly on 5.2.** Blender issue
   [#130561](https://projects.blender.org/blender/blender/issues/130561)
   reported abi3 wheels rejected as incompatible on 4.2.4 and 4.3, with a
   filename-renaming workaround. A hand-built
   `crowdabi3-0.1.0-cp313-abi3-macosx_11_0_arm64.whl` was packaged as an
   extension, built, installed with `extension install-file --enable`, and
   imported successfully in a subsequent headless session, resolving to the
   shared site-packages path. The probe extension was then removed and both
   directories confirmed empty. 5.2's resolver reads the `abi3` tag as "any
   CPython 3" and lets it take priority over the `cp3xx` tag.
3. **The point-cloud playback path works** (section 7), with 1,000 positions
   set in approximately 30 µs. This is an API-viability observation from a
   single unrecorded run, **not** a performance measurement, and must not be
   quoted as one.

## 9. Verification

Every acceptance claim gets a checked-in runner, all headless.

1. `cargo test -p crowd-trace` — round trip, version-mismatch rejection,
   truncated-file rejection, and stable-ID preservation.
2. `scripts/blender-install-test.sh` — build wheel, `mkdir -p` output dir,
   build extension zip, remove any prior install, `extension install-file`,
   then import the native module in a fresh
   `-b --factory-startup --python-expr` session. Asserts the loaded module
   resolves under the Blender extensions directory and that no path into this
   checkout appears. This automates M0 acceptance criterion 5.
3. `scripts/blender-playback-test.sh` — load a checked-in 1,000-agent trace,
   step every tick, assert IDs and positions match the Rust reader exactly, and
   report simulation cost and Blender playback cost **separately**, as M0
   acceptance criterion 6 requires.
4. A dated report under `docs/benchmarks/` linking these outputs and recording
   the environment.

`README.md` and `AGENTS.md` gain copy-ready commands for each runner as it is
checked in, per milestone rule 8.

## 10. Definition of done

- `cargo test --workspace` and `cargo clippy --workspace --all-targets -D warnings`
  clean.
- Both Blender scripts pass from a clean install on the recorded machine.
- The dated report exists and states plainly what was measured, on what
  hardware, and what remains unproven.
- The canonical contract's target-host line reads Blender 5.2 LTS.

## 11. Stop conditions

Stop and record a failed gate if the native module cannot load from a clean
install without linking to a contributor environment, if bundled-wheel name
collisions cannot be avoided, or if 1,000-point playback proves too costly to
make the 1K vertical slice credible. Do not proceed to cache v0 or navigation
on an unresolved bridge failure, and do not start M1 to route around it.
