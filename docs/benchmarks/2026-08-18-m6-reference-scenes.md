# M6 integrated deterministic reference scenes — 2026-08-18

## Result

The six checked M6 reference scenes pass against the pinned CC0 authored
motion baseline. Two complete runner invocations emitted byte-identical JSON,
including identical runtime-derived metrics and replay hashes. The combined
replay hash is
`7a1e140ea825a65676e962f688cd7312736892e4d61d2c14192641d85a88c4db`.

This result does not accept the official CMU candidate. The pinned
[CMU report](2026-08-18-m6-cmu-motion.json) contains 3,587 measured raw
joint-limit exceedances against the pinned hard-zero threshold. The runner
rejects that candidate before scene execution. No CMU clip contributes
accepted scene motion, and no component probe is promoted as integrated CMU
evidence.

## Pinned source selection

The runner recomputes BLAKE3 from every consumed byte stream and accepts source
selection only when all pinned digests, IDs, and relationships agree.

| Artifact | Required identity | Pinned BLAKE3 |
| --- | --- | --- |
| CMU report | `cmu-mocap-subjects-35-36-m6-v1`; manifest `cmu-mocap-subjects-35-36-m6-v1`; source hash `a75af4c0…ec91424`; measured joint evidence 3,587 | `b05e9bce668fab8ef11fe55e6b396841fd23a7d2373e8281e9c654280cb5f2f9` |
| Threshold contract | `m6-cmu-motion-2026-08-18`; measured baseline 3,587; limit 0; exact report/manifest relationships | `993bb4897305524e359943820fdae24f347e8cc025429f604fbdc628b76de154` |
| CC0 database | `reference-humanoid-motion`; retarget profile `reference-humanoid`; exact walk/jog fixture clips | `c687fede242e359fb7b94e91e1c17a44ddacd01963697f2e5f4e687c01998e08` |
| CC0 provenance | `reference-walk-metadata`; `CC0-1.0`; redistribution allowed; content hash equals the database BLAKE3 | `60d1bf5aa98f66ab1a37096876140a53b6bb6d63e03f8a83a5ed7370c895340d` |

The passing baseline is labeled `accepted` only after those checks succeed.
Mutating the CMU report, authored database, provenance content hash, or
threshold bytes makes the runner fail closed with exit 2 and no report.

## Environment and inputs

- Platform: Darwin arm64
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`
- Cargo: `cargo 1.94.1 (29ea6fb6a 2026-03-24)`
- Fix-round-2 base revision: `ca11034`
- Fixture:
  [acceptance-scenes-v1.json](../../assets/reference/m6/acceptance-scenes-v1.json)
- Report schema:
  [m6-acceptance-scenes-v1.schema.json](../../schemas/m6-acceptance-scenes-v1.schema.json)
- Fixture BLAKE3:
  `e56504b02f4be531fbba348cebde5928e18e0fc42d1969591161a34d842abba6`

Generated JSON reports remained outside Git.

## Integrated execution method

Each scene constructs a complete population frame from its declared seed and
population. Stable agent IDs, initial positions, presentation state, and
per-agent motion variation are generated through the existing deterministic
RNG/ID contracts. Every declared tick executes motion matching and feedback for
every agent. The current phase chooses the nearest sample that satisfies an
available required contact, the exact chosen sample supplies observed contact,
and horizontal desired-versus-executed displacement over that 30 Hz tick
supplies foot-slide evidence. No scene fixture supplies a measured slide value.
Seed, population, tick, desired-velocity, and required-contact mutations therefore
affect executed state or measured evidence rather than only changing report text.

The domain operation then composes existing authorities:

- `scheduled_cafe`: finite-capacity reservation, waiting, release, promotion,
  and a runtime-created paired layer applied to the owner set read after the
  waiting agent is promoted;
- `family_split_regroup`: formation evaluation and bounded cohesion on every
  declared tick, with intrusion candidates drawn from the executed population;
- `terrain_motion_feedback`: terrain acceptance, foot locks, and navigation
  feedback measured on every applicable tick;
- `paired_handoff`: consumed request, motion, and animation-layer artifacts;
  request/motion validation; exact request/layer binding to the executed full
  base cache hash; atomic scheduling; and checked layer edits applied to the
  complete state;
- `ragdoll_recovery`: consumed physics transition and hero boundary,
  deterministic physics samples, phase recovery, promotion, and target-state
  application; and
- `mixed_tier_diagnostics`: consumed tier, request, and motion artifacts;
  fidelity scheduling over all 100 agents; atomic interaction promotion;
  validated contact; and participant root application.

The source ledger hashes only parsed/consumed artifacts. A declared source that
is not consumed causes exit 2. Shared motion/provenance/threshold/report hashes
appear in each scene because each scene executes that pinned baseline and
source-selection decision.

## Scene metrics

| Scene | Ticks | Agent-ticks | Promoted groups | Trajectory max (mm) | Foot slide (mm) | Contacts observed/required | Safety | Runtime fallback | Isolation | Base/unrelated mutations | Target mutations | Replay hash |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |
| `scheduled_cafe` | 121 | 363 | 1 | 15 | 1 | 364/364 | 0 | 0 | measured | 0/0 | 2 | `5945952c…6841cf7` |
| `family_split_regroup` | 31 | 124 | 0 | 5 | 1 | 124/124 | 0 | 0 | not applicable | n/a | 0 | `d5366aca…23497b` |
| `terrain_motion_feedback` | 11 | 22 | 0 | 25 | 1 | 33/33 | 0 | 0 | not applicable | n/a | 0 | `f676bddc…22813f` |
| `paired_handoff` | 11 | 33 | 1 | 5 | 1 | 34/34 | 0 | 0 | measured | 0/0 | 2 | `d841113b…c06987` |
| `ragdoll_recovery` | 11 | 22 | 1 | 15 | 1 | 23/23 | 0 | 0 | measured | 0/0 | 1 | `3261430b…311abe` |
| `mixed_tier_diagnostics` | 16 | 1,600 | 1 | 5 | 1 | 1,601/1,601 | 0 | 0 | measured | 0/0 | 2 | `f460ebdd…75bc1e` |

Contact precision is 1,000,000 millionths in every scene. Contact totals include
one authored locomotion-contact observation per executed agent-tick plus each
scene's domain contact where applicable. The source fallback count remains one
per scene for explicit CMU-rejection-to-CC0 selection; the separate runtime
motion-matcher fallback count is zero in every scene.

Family and terrain isolation are explicitly `not_applicable`: neither scene
executes a promoted layer/runtime operation, so the report does not fabricate
zero isolation measurements. The other four scenes compare the complete base
population before and after the actual promoted operation, require target
mutation, and measure base-cache and unrelated-agent preservation. The valid
paired request and layer both identify the executed full-base BLAKE3
`b2c74ec5a6038dc1761afdcb727f756b092ad64113aeeed3a9c5e14611c138d7`.

Scene-specific runtime evidence includes:

- café: two grants, one waiter, one promotion after release, zero double
  ownership, and a layer target set that includes the promoted waiter while
  excluding the released owner;
- family: 16 split samples, 15 regrouped samples, zero runtime-derived
  intrusion samples, and 4,000 mm maximum separation;
- terrain: 11 accepted terrain ticks, 11 satisfied foot-lock ticks, and 11
  navigation-feedback events;
- paired handoff: exact executed-base provenance, atomic participant locking,
  one required interaction contact, completion, and two consumed layer edits;
- ragdoll: a validated hero boundary, 11 physics samples, five floor-contact
  samples, and 1/2/8 impact/stabilize/resume ticks; and
- mixed tier: one actual promoted group and validated contact, 3 full, 2
  reduced, and 1 aggregate diagnostic channels, and 880 scheduled animation
  evaluations.

## Mutation and failure evidence

The real-binary suite covers:

- four source-spoof mutations, all rejected before report publication;
- seed, population, and tick mutations that alter executed state or
  agent-tick counts;
- a zero desired-velocity mutation that measures 34 mm of per-tick slide rather
  than copying a fixture value, plus a left-to-right contact mutation that
  changes the executed sample phase;
- post-release café target identity, including promoted/released membership;
- full-population target/base/unrelated isolation;
- paired request and layer hashes mutated together to another valid 64-character
  value, rejected against the executed full base with exit 2 and no report;
- declared-but-unconsumed source rejection;
- actual paired layer, hero boundary, and mixed request/motion consumption;
- required common fields and exact typed evidence for all six scene kinds;
- canonical scene names; and
- a nonzero hard-safety result under a permissive configured maximum, which
  still writes schema-valid failure evidence, includes a hard-safety reason,
  and exits 1.

## Verified commands

Fix Round 1 started with 13 intended failures and two existing passes:

```text
cargo test -p crowd-bench --test m6_acceptance_scenes
test result: FAILED. 2 passed; 13 failed
```

Fix Round 2 added four real-binary causal regressions. Against `ca11034`, the
suite retained all 15 prior passes and failed exactly those four tests:

```text
cargo test -p crowd-bench --test m6_acceptance_scenes
test result: FAILED. 15 passed; 4 failed
```

The focused GREEN run passes every real-binary regression:

```text
cargo test -p crowd-bench --test m6_acceptance_scenes
test result: ok. 20 passed; 0 failed
```

The prescribed two-pass runner validates byte equality and the regenerated
combined hash:

```text
scripts/m6-reference-scenes-test.sh
M6 reference scenes passed twice with exact hashes and metrics: 7a1e140ea825a65676e962f688cd7312736892e4d61d2c14192641d85a88c4db
```

Workspace linting passes:

```text
cargo clippy --workspace --all-targets -- -D warnings
```

## Interpretation and limits

This is deterministic, checked-fixture integration evidence. It establishes
causal scene execution, source consumption, trajectory/contact/safety
accounting, stable replay, and applicable full-state isolation for this narrow
authored baseline. It does not establish accepted CMU motion, arbitrary-rig
retargeting, Blender visual fidelity, human perceptual quality, or mixed-tier
performance. Blender layer proof and the 10,000-agent performance gate remain
Task 13 scope; requirement-level M6 adjudication remains Task 14 scope.
