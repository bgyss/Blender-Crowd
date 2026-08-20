# M6 requirement-level acceptance — 2026-08-19

## Result

**OPEN — M6 remains unaccepted.** Nine deterministic criteria have direct
checked-fixture or host-Blender evidence, but criterion 5 is still open. The
official CMU candidate was rejected with 3,587 measured joint-limit violations
against the hard limit of zero. The accepted CC0 authored data is a narrow
redistributable fixture baseline for deterministic integration; it is not a
replacement production-motion acceptance result.

`scripts/m6-acceptance.sh` therefore exits 2 without `M6_ALLOW_OPEN=1`. The
override acknowledges an open audit only; it does not change any criterion to
PASS and cannot convert a failed gate into a deferral. R1–R4 neural animation
and independent-user verification remain M9 work and are not M6 failures.

## Environment

- Evidence window: 2026-08-18 through 2026-08-20, America/Los_Angeles.
- Task 14 audit base: Git revision `2f72dfa815a02265c94372e497f966caadf3203f`.
- Host: Apple arm64, Darwin 27.0.0, macOS 27.0 build `26A5416b`.
- Blender: 5.2.0 LTS, build `fbe6228777e7`, run with normal host Metal access.
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`.
- Cargo: `cargo 1.94.1 (29ea6fb6a 2026-03-24)`.
- Python: 3.14.2.
- Optimized workspace and mixed-tier lanes used Cargo's release profile.

The dated component reports retain their own exact run dates and measurements.
Generated scene and performance JSON remained outside the committed evidence
set except for the checked CMU aggregate report.

## Inputs and hashes

The audit consumes repository-relative, versioned inputs. SHA-256 values below
were recomputed during Task 14; the integrated scene runner separately pins and
recomputes BLAKE3 over consumed source bytes.

| Input | SHA-256 | Role |
| --- | --- | --- |
| `assets/reference/m6/motion-thresholds-v1.json` | `f75b89a50162887aefa770d10a8d14bbee694c91bf3d27277b2c9b5776eb61e2` | Hard-zero and dated soft motion limits |
| `docs/benchmarks/2026-08-18-m6-cmu-motion.json` | `84aecd09f83ba775d0b1b69531282556ca0108d07ca2be3d55b9a76c5a8cc06a` | Measured CMU candidate evidence |
| `assets/reference/m6/motion-database-input-v1.json` | `0a49fc3b0fd38e0a07e715f15f5ec2bd961ca922c1613a16c2b0d6b42189bb2e` | Accepted authored CC0 fixture database |
| `assets/reference/m6/motion-provenance-v1.json` | `615de533e8061736942f83c25de99a494575e6535ce4f1888b380c81e3e6c2ac` | CC0 provenance and redistribution decision |
| `assets/reference/m6/acceptance-scenes-v1.json` | `781d3b538ce9cff64d22c5eb3e6dddbc147cdcaddc2c66dad3f3a4e63f2309ef` | Six deterministic integrated scenes |
| `schemas/m6-acceptance-scenes-v1.schema.json` | `97bb2befefb8b7ca5644ce974c505917436812d2cee79d70074535af68b0eaea` | Scene evidence schema |

Pinned BLAKE3 identities used by the scene authority are:

- CMU report: `b05e9bce668fab8ef11fe55e6b396841fd23a7d2373e8281e9c654280cb5f2f9`;
- threshold contract: `993bb4897305524e359943820fdae24f347e8cc025429f604fbdc628b76de154`;
- CC0 database: `c687fede242e359fb7b94e91e1c17a44ddacd01963697f2e5f4e687c01998e08`;
- CC0 provenance: `60d1bf5aa98f66ab1a37096876140a53b6bb6d63e03f8a83a5ed7370c895340d`;
  and
- combined six-scene replay:
  `7a1e140ea825a65676e962f688cd7312736892e4d61d2c14192641d85a88c4db`.

The criterion adjudication consumes these durable evidence files directly:

- `docs/benchmarks/2026-08-18-m6-foundation.md`;
- `docs/benchmarks/2026-08-18-m6-cmu-motion.md`;
- `docs/benchmarks/2026-08-18-m6-reference-scenes.md`;
- `docs/benchmarks/2026-08-18-m6-blender-layers.md`;
- `docs/benchmarks/2026-08-18-m6-mixed-tier.md`;
- `examples/m6-extension-rust.rs`;
- `examples/m6_extension_python.py`;
- `scripts/m6-foundation-test.sh`;
- `scripts/m6-reference-scenes-test.sh`;
- `scripts/m6-blender-test.sh`;
- `scripts/m6-performance-test.sh`; and
- `scripts/m6-extension-examples-test.sh`.

## Criterion-by-criterion adjudication

PASS below means the exact deterministic M6 criterion has direct evidence at
the documented checked-fixture or host-Blender proof level. It does not widen
that evidence into an unsupported production, visual, hardware, or user-study
claim.

| # | Criterion | Direct evidence and boundary | Status |
| ---: | --- | --- | --- |
| 1 | State-machine, utility, and behavior-tree brains are reproducible and traceable | Foundation tests cover typed blackboards, deterministic fuzzy/utility decisions, interrupts, action libraries, and strict traces. The host Blender debugger proof covers current-source inspection, path search/highlight, bidirectional context navigation, reusable actions/presets, native graph compilation, corrections, and tier-reduced evidence. | PASS |
| 2 | Typed perception passes modality, ordering, budget, and cache/trace checks | `scripts/m6-foundation-test.sh` executes the perception, blackboard, runtime-perception, metrics, and schema lanes for vision/occlusion, hearing, touch, density/flow, group extent, semantic distance, stable ordering, bounded memory, and degradation. | PASS |
| 3 | Scheduled finite-resource activity avoids double ownership and undeclared deadlock | The `scheduled_cafe` integrated scene executes 121 ticks, two grants, one waiter, release and promotion, with zero double ownership and exact deterministic replay. | PASS |
| 4 | Social/group scenes improve declared readability metrics without hard-safety regression | The family scene executes split/regroup evidence with 4,000 mm maximum separation, zero runtime-derived intrusion samples, zero hard-safety failures, and deterministic replay; formation and cohesion comparisons are covered by the foundation lanes. | PASS |
| 5 | Motion matching meets trajectory, contact, foot-slip, turn, terrain, transition, and performance thresholds against an accepted clip-state baseline | The six scenes and fixed 10K lane pass against the checked CC0 authored fixture, with zero runtime motion fallbacks and the recorded trajectory/contact/safety evidence. The production CMU candidate itself has 3,587 joint-limit violations against zero and is rejected, so it cannot close this criterion. | OPEN |
| 6 | Motion feedback reports and constrains infeasible motion without teleporting or hiding collisions | Integrated terrain and motion-feedback execution measures root/foot/contact evidence, reports infeasibility, records zero runtime fallbacks and hard-safety failures, and causally changes under desired-velocity/contact mutations. | PASS |
| 7 | Interaction and recovery layers preserve unrelated agents, base caches, and lower tiers | Paired, recovery, café, and mixed-tier scenes measure zero base/unrelated mutations. Host Blender attach, failed replacement, mute, unmute, remove, and reload preserve the independent M4 stack and exact base-cache identity. | PASS |
| 8 | Hero integrations declare solver, ownership, cache, failure, and support boundaries | The host proof exposes deterministic cached-physics ownership and lifecycle. Hero cloth is explicitly `declaration-only unsupported` and `not attached`; no hidden dependency or broad visual claim is made. | PASS |
| 9 | Every claimed external API language passes deterministic channel, budget, version, and isolation cases | The claimed-language gate executes native Rust contract tests, Python operation-failure isolation tests, and both examples twice. Rust and Python emit byte-identical output with replay SHA-256 `7132ecd92ab0feb0efc7592fdb144fd625727769b5abecad8d869726d73f83fc`; accepted and over-budget calls carry real output/fallback payloads, while undeclared-input and version-mismatch calls carry null output. A foundation or claimed-language contract failure makes this criterion non-PASS. No C or C++ API is claimed. | PASS |
| 10 | Deterministic R0 request/response, validation, fallback, layer lifecycle, isolation, and worker-absent replay pass | Foundation, integrated paired-handoff, cache-layer, worker, and host Blender lanes cover request/response validation, source fallback, sparse composition, reload/removal, cross-cache rejection, unrelated-agent isolation, and playback from retained artifacts without a live worker. | PASS |

## Known failures and rejected inputs

- The CMU candidate is rejected, not repaired or relabeled. Its 3,587 raw
  ASF-bound violations exceed the hard limit of zero. The importer did not
  clamp, smooth, wrap, or loosen the threshold.
- The CC0 `reference-humanoid-motion` fixture is accepted only as the narrow
  deterministic baseline consumed by the integrated scenes and fixed mixed-tier
  lane. It does not promote the rejected CMU candidate or establish a broad
  production motion corpus.
- The active CMU candidate is not hardcoded as the only future state. The motion
  gate verifies report/manifest/retarget identities, source-hash relationships,
  hard and soft threshold evidence, and the accepted CC0 fixture provenance.
  A well-formed future production candidate that meets every unchanged measured
  limit will return PASS; malformed or inconsistent evidence returns FAILED.
- Focused Fix Round 1 verification still leaves criterion 5 OPEN. The remaining
  item is the milestone's production-motion criterion, not an omitted Task 14
  deliverable.

## Unsupported claims

- Blender cloth/hair/Geometry Nodes deformation is not attached, executed, or
  benchmarked. Hero cloth remains a declaration-only support boundary.
- The deterministic cached handoff is not evidence for Blender rigid-body parity
  or arbitrary collision scenes.
- No neural motion, external model worker, accelerator, or GPU backend was
  attached or measured.
- The checked Blender and 10K fixtures do not establish arbitrary-scene,
  long-duration, production viewport/render, Cache v1 disk/streaming, artist
  usability, or visual quality claims.
- The mixed-tier rate applies only to the fixed 10,000-agent, 30-tick,
  10/990/9,000 S0/S1/S2 fixture on the recorded host. It is not a portable
  performance guarantee.

## M9 deferrals

R1–R4 neural animation, model/checkpoint/data authorization, blinded perceptual
comparisons, combat-domain learned motion, and independent-user verification are
DEFERRED TO M9. They are informational in `scripts/m6-acceptance.sh`, never
converted into M6 failures, and do not close criterion 5.

## Verification

The long-running results below are retained from the base Task 14 verification
record. They were not repeated during Fix Round 1; in particular,
`cargo test --workspace --release` was not rerun. Exact focused Fix Round 1
results are recorded in the Task 14 report.

| Command | Result | Evidence boundary |
| --- | --- | --- |
| `cargo test --workspace --release` | PASS (recorded) | Full optimized workspace; the release density-fuzz lane took 498.29 seconds |
| `scripts/m6-blender-test.sh` | PASS (recorded) | Current source add-on, fresh native wheel, Blender 5.2 LTS, normal host Metal access |
| `scripts/m6-foundation-test.sh` | PASS (recorded) | M6 Rust/Python/schema/worker foundation |
| `scripts/m6-reference-scenes-test.sh` | PASS (recorded) | Two byte-identical six-scene reports and combined replay hash |
| `scripts/m6-performance-test.sh` | PASS (recorded) | Fixed 10K mixed-tier lane exceeded 10 ticks/s twice with identical replay identity |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (recorded) | Warning-clean workspace |
| `cargo fmt --all -- --check` | PASS (recorded) | Rust formatting clean |
| Full repository Python suite | PASS (recorded) | Base Task 14 Python checks |

The full public runner was not executed during Fix Round 1 because it includes
the already completed release workspace and density-fuzz lane. In a future
public run, every status is collected from commands executed by that process,
then compared with the checked-in expected matrix below. This matrix is a
validation contract, not a claim that every lane was rerun during this fix
round. Any disagreement fails the acceptance-report gate.

| Gate | Result |
| --- | --- |
| `foundation` | PASS (expected current run) |
| `debugger_library` | PASS (expected current run) |
| `motion_source` | OPEN (expected current run) |
| `reference_scenes` | PASS (expected current run) |
| `blender` | PASS (expected current run) |
| `mixed_tier` | PASS (expected current run) |
| `extension_examples` | PASS (expected current run) |
| `release_workspace` | PASS (expected current run) |
| `clippy` | PASS (expected current run) |
| `format` | PASS (expected current run) |
| `python` | PASS (expected current run) |
