# M6 requirement-level acceptance — 2026-08-20

## Result

**PASS — M6 is accepted with criterion 5 deferred to M9.** Nine deterministic
criteria have direct checked-fixture or host-Blender evidence. Criterion 5,
production motion matching against an accepted clip-state baseline, was rescoped
out of M6 into M9 on this date and is reported as `DEFERRED TO M9`. It is
deferred, not satisfied: the official CMU candidate remains rejected at 3,587
measured joint-limit violations against the hard limit of zero, and the accepted
CC0 authored data remains a narrow redistributable fixture baseline rather than a
production motion corpus. M6 therefore makes no production motion-matching
claim. The full rationale, the unchanged thresholds carried across, and the M9
closing conditions are recorded in
[docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md](2026-08-20-m6-criterion-5-deferral.md).

This report supersedes the [2026-08-19 audit](2026-08-19-m6-acceptance.md),
which adjudicated the same evidence while criterion 5 was still in M6 scope and
therefore returned OPEN. No criterion status improved through re-measurement, no
threshold was loosened, and no rejected input was promoted. R1–R4 neural
animation and independent-user verification remain M9 work and are not M6
failures.

## Environment

- Evidence window: 2026-08-18 through 2026-08-20, America/Los_Angeles.
- Audit base: Git revision `871b5e12a018c21a9266e7c1a9d401e90ffce285`.
- Host: Apple arm64, Darwin 27.0.0, macOS 27.0 build `26A5416b`.
- Blender: 5.2.0 LTS, build date 2026-07-14, run with normal host Metal access.
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`.
- Cargo: `cargo 1.94.1 (29ea6fb6a 2026-03-24)`.
- Python: 3.14.6.
- Optimized workspace and mixed-tier lanes used Cargo's release profile.

The dated component reports retain their own exact run dates and measurements.
Generated scene and performance JSON remained outside the committed evidence
set except for the checked CMU aggregate report.

## Inputs and hashes

The audit consumes repository-relative, versioned inputs. SHA-256 values below
were recomputed during this run; the integrated scene runner separately pins and
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

- `docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md`;
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
claim. `DEFERRED TO M9` means the criterion left M6 scope with its thresholds
unchanged and is unclaimed until M9 closes it.

| # | Criterion | Direct evidence and boundary | Status |
| ---: | --- | --- | --- |
| 1 | State-machine, utility, and behavior-tree brains are reproducible and traceable | Foundation tests cover typed blackboards, deterministic fuzzy/utility decisions, interrupts, action libraries, and strict traces. The host Blender debugger proof covers current-source inspection, path search/highlight, bidirectional context navigation, reusable actions/presets, native graph compilation, corrections, and tier-reduced evidence. | PASS |
| 2 | Typed perception passes modality, ordering, budget, and cache/trace checks | `scripts/m6-foundation-test.sh` executes the perception, blackboard, runtime-perception, metrics, and schema lanes for vision/occlusion, hearing, touch, density/flow, group extent, semantic distance, stable ordering, bounded memory, and degradation. | PASS |
| 3 | Scheduled finite-resource activity avoids double ownership and undeclared deadlock | The `scheduled_cafe` integrated scene executes 121 ticks, two grants, one waiter, release and promotion, with zero double ownership and exact deterministic replay. | PASS |
| 4 | Social/group scenes improve declared readability metrics without hard-safety regression | The family scene executes split/regroup evidence with 4,000 mm maximum separation, zero runtime-derived intrusion samples, zero hard-safety failures, and deterministic replay; formation and cohesion comparisons are covered by the foundation lanes. | PASS |
| 5 | Motion matching meets trajectory, contact, foot-slip, turn, terrain, transition, and performance thresholds against an accepted clip-state baseline | Rescoped to M9 Track C on 2026-08-20 with every measured threshold unchanged. No production motion corpus meets those thresholds: the CMU candidate has 3,587 joint-limit violations against zero and stays rejected, and the checked CC0 authored data is a fixture baseline only. The six scenes and fixed 10K lane continue to pass against that fixture, which supports criteria 3, 4, and 6 but does not constitute a production motion-matching claim. | DEFERRED TO M9 |
| 6 | Motion feedback reports and constrains infeasible motion without teleporting or hiding collisions | Integrated terrain and motion-feedback execution measures root/foot/contact evidence, reports infeasibility, records zero runtime fallbacks and hard-safety failures, and causally changes under desired-velocity/contact mutations. | PASS |
| 7 | Interaction and recovery layers preserve unrelated agents, base caches, and lower tiers | Paired, recovery, café, and mixed-tier scenes measure zero base/unrelated mutations. Host Blender attach, failed replacement, mute, unmute, remove, and reload preserve the independent M4 stack and exact base-cache identity. | PASS |
| 8 | Hero integrations declare solver, ownership, cache, failure, and support boundaries | The host proof exposes deterministic cached-physics ownership and lifecycle. Hero cloth is explicitly `declaration-only unsupported` and `not attached`; no hidden dependency or broad visual claim is made. | PASS |
| 9 | Every claimed external API language passes deterministic channel, budget, version, and isolation cases | The claimed-language gate executes native Rust contract tests, Python operation-failure isolation tests, and both examples twice. Rust and Python emit byte-identical output with replay SHA-256 `7132ecd92ab0feb0efc7592fdb144fd625727769b5abecad8d869726d73f83fc`; accepted and over-budget calls carry real output/fallback payloads, while undeclared-input and version-mismatch calls carry null output. A foundation or claimed-language contract failure makes this criterion non-PASS. No C or C++ API is claimed. | PASS |
| 10 | Deterministic R0 request/response, validation, fallback, layer lifecycle, isolation, and worker-absent replay pass | Foundation, integrated paired-handoff, cache-layer, worker, and host Blender lanes cover request/response validation, source fallback, sparse composition, reload/removal, cross-cache rejection, unrelated-agent isolation, and playback from retained artifacts without a live worker. | PASS |

## Known failures and rejected inputs

- The CMU candidate is rejected, not repaired or relabeled. Its 3,587 raw
  ASF-bound violations exceed the hard limit of zero. The importer did not
  clamp, smooth, wrap, or loosen the threshold, and the deferral did not change
  that result.
- The CC0 `reference-humanoid-motion` fixture is accepted only as the narrow
  deterministic baseline consumed by the integrated scenes and fixed mixed-tier
  lane. It does not promote the rejected CMU candidate or establish a broad
  production motion corpus.
- The active CMU candidate is not hardcoded as the only future state. The motion
  gate verifies report/manifest/retarget identities, source-hash relationships,
  hard and soft threshold evidence, and the accepted CC0 fixture provenance.
  A well-formed future production candidate that meets every unchanged measured
  limit will return PASS; malformed or inconsistent evidence returns FAILED.

## Unsupported claims

- Production motion matching is not claimed. Criterion 5 is deferred, and no
  measurement in this report stands in for it.
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

## Criterion 5 deferral to M9

Criterion 5 moved to M9 Track C because it is blocked on motion data
acquisition, not on implemented behavior, and that acquisition has no schedule.
The motion stack itself is implemented and exercised deterministically; what is
missing is a production corpus that meets the unchanged limits.

The following are carried into M9 verbatim and may not be relaxed to make a
candidate pass:

- hard limits of zero for `joint_limit_violations`, `root_teleportations`,
  `undeclared_contacts`, `source_hash_drift`, and `cross_cache_mutations`;
- soft limits of 21 mm foot slide, 3 mm trajectory deviation, 60,005 µrad turn
  discontinuity, and 0 ppm rejected frames;
- provenance, licensing, and redistribution review before ingestion; and
- the retarget/manifest/report identity and source-hash relationships enforced
  by `scripts/m6_acceptance_checks.py motion-source`.

The `motion_source` gate still runs on every audit and still fails M6 closed
when its evidence is malformed, inconsistent, or unverified, because the
accepted CC0 fixture it validates is consumed by criteria 3, 4, and 6. Only its
`OPEN` outcome is deferred. See
[docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md](2026-08-20-m6-criterion-5-deferral.md)
and the [M9 contract](../milestones/M9-neural-animation-operator-validation.md).

## M9 deferrals

Production motion matching (criterion 5), R1–R4 neural animation,
model/checkpoint/data authorization, blinded perceptual comparisons,
combat-domain learned motion, and independent-user verification are DEFERRED TO
M9. They are informational in `scripts/m6-acceptance.sh` and are never converted
into M6 failures or M6 claims.

## Verification

All lanes below were executed by `M6_RUN_BLENDER=1 scripts/m6-acceptance.sh` on
2026-08-20 at the recorded revision.

| Command | Result | Evidence boundary |
| --- | --- | --- |
| `cargo test --workspace --release` | PASS | Full optimized workspace including the release density-fuzz lane |
| `scripts/m6-blender-test.sh` | PASS | Current source add-on, fresh native wheel, Blender 5.2 LTS, normal host Metal access |
| `scripts/m6-foundation-test.sh` | PASS | M6 Rust/Python/schema/worker foundation |
| `scripts/m6-reference-scenes-test.sh` | PASS | Two byte-identical six-scene reports and combined replay hash |
| `scripts/m6-performance-test.sh` | PASS | Fixed 10K mixed-tier lane exceeded 10 ticks/s twice with identical replay identity |
| `scripts/m6-extension-examples-test.sh` | PASS | Rust and Python claimed-language contracts, determinism, and failure isolation |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Warning-clean workspace |
| `cargo fmt --all -- --check` | PASS | Rust formatting clean |
| Full repository Python suite | PASS | `python3 -m unittest discover -s tests -p 'test_*.py'` |

Every status is collected from commands executed by the audit process, then
compared with the checked-in expected matrix below. Any disagreement fails the
acceptance-report gate.

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

The `motion_source` row stays OPEN because no production candidate meets the
unchanged thresholds. That outcome is adjudicated as `DEFERRED TO M9` for
criterion 5 and no longer blocks the milestone; a `FAILED` motion gate still
does.
