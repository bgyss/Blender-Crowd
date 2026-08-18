# M6 integrated deterministic reference scenes — 2026-08-18

## Result

The six integrated M6 reference scenes pass against the checked CC0 authored
motion baseline. Two complete runner invocations emitted byte-identical JSON,
including identical per-scene metrics and replay hashes. The combined replay
hash is
`620e086637a7df18e78c13347e26a3452078bc7fed1d8f65d79d0e81fbc569b8`.

This result does not accept the official CMU candidate. The checked
[CMU report](2026-08-18-m6-cmu-motion.json) records 3,587 raw joint-limit
exceedances against the unchanged hard-zero limit. The runner rejects that
candidate and explicitly falls back to the checked
[CC0 database](../../assets/reference/m6/motion-database-input-v1.json) and
[provenance contract](../../assets/reference/m6/motion-provenance-v1.json).
No CMU clip contributes accepted scene motion.

## Environment and inputs

- Platform: Darwin arm64
- Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`
- Cargo: `cargo 1.94.1 (29ea6fb6a 2026-03-24)`
- Pre-task repository revision: `c29693b`
- Fixture:
  [acceptance-scenes-v1.json](../../assets/reference/m6/acceptance-scenes-v1.json)
- Report schema:
  [m6-acceptance-scenes-v1.schema.json](../../schemas/m6-acceptance-scenes-v1.schema.json)
- Fixture BLAKE3:
  `42aad3858811c2e1820542e91c5441ed1afd655ba38a677cadb1a71daef98396`
- CMU report BLAKE3:
  `b05e9bce668fab8ef11fe55e6b396841fd23a7d2373e8281e9c654280cb5f2f9`
- Accepted database BLAKE3:
  `c687fede242e359fb7b94e91e1c17a44ddacd01963697f2e5f4e687c01998e08`
- Accepted provenance BLAKE3:
  `66fecce25c4b37a1dde217118018215275fa4fbed2ab50d07e4cd7f05dd793d0`

All generated JSON reports remained outside Git. The checked fixture records
stable seeds, source paths, tick ranges, population size, promoted targets,
motion requests, and explicit criteria. The emitted report hashes every source
file actually consumed by each scene.

## Method

The runner composes existing M6 authorities instead of reimplementing their
decisions:

- `ReservationRuntimeV1` performs finite-capacity café admission and promotion.
- `FormationV1` detects the family split, supplies bounded cohesion, and
  verifies regrouping and intrusion state.
- `MotionMatcher`, `MotionFeedbackV1`, `TerrainConstraintV1`, and
  `FootLockWindowV1` measure trajectory fit, contact, foot slide, terrain
  feasibility, and navigation feedback.
- `InteractionSchedulerV1`, `InteractionMotionV1::validate_against`, and
  `deterministic_paired_clip` verify atomic promotion, required contact, and
  completion.
- `simulate_physics_handoff_v1`, `validate_transition`, and `recovery_phase`
  produce the inspectable ragdoll cache and recovery sequence.
- `FidelityPolicy::animation_due` and `render_for` verify the mixed-tier
  diagnostic schedule and tier mapping.
- `compose_interaction_frame_v1` measures immutable-base and unrelated-agent
  isolation independently in every scene.

Every scene also runs the checked authored motion database through the same
matcher and feedback path. Required versus observed contact counts are kept
separate, and a zero-required-contact case would report perfect precision
without inventing a contact. A failing criterion still produces schema-valid
JSON and exits with status 1; malformed inputs or execution failures exit 2.

## Scene metrics

| Scene | Ticks | Agents | Promoted groups | Trajectory fit (mm) | Foot slide (mm) | Contacts observed/required | Safety violations | Source fallback | Runtime motion fallback | Unrelated mutations | Replay hash |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `scheduled_cafe` | 0–120 | 3 | 1 | 10 | 10 | 2/2 | 0 | 1 | 0 | 0 | `9caf3eb1…1347c` |
| `family_split_regroup` | 0–30 | 4 | 0 | 0 | 8 | 1/1 | 0 | 1 | 0 | 0 | `a3125bfa…5f53e` |
| `terrain_motion_feedback` | 10–20 | 2 | 0 | 20 | 10 | 2/2 | 0 | 1 | 0 | 0 | `f925d7ce…3bbc8` |
| `paired_handoff` | 10–20 | 3 | 1 | 0 | 6 | 2/2 | 0 | 1 | 0 | 0 | `6e0f150f…f847b` |
| `ragdoll_recovery` | 20–30 | 2 | 1 | 10 | 10 | 2/2 | 0 | 1 | 0 | 0 | `5a7cc489…ea4b` |
| `mixed_tier_diagnostics` | 0–15 | 100 | 1 | 0 | 8 | 2/2 | 0 | 1 | 0 | 0 | `f37b3f8f…6c41` |

Contact precision is 1,000,000 millionths in every scene. The combined source
fallback count is six: one explicit CMU-rejection-to-CC0 selection per scene.
The independent runtime motion-matcher fallback count is zero in every scene.
This distinction prevents the rejected external source from being hidden by a
generic fallback total.

Scene-specific evidence also records:

- café: two initial grants, one waiter, one deterministic promotion after
  release, and zero double ownership;
- family: one split sample, one regrouped sample, 4,000 mm maximum split
  separation, and zero intrusion samples;
- terrain: one accepted terrain constraint, one satisfied foot lock, and one
  navigation-feedback event;
- paired handoff: two atomically locked participants, one required interaction
  contact, and one completed interaction;
- ragdoll: 11 cached samples, five floor-contact samples, and the expected
  1/2/8 impact/stabilize/resume tick distribution; and
- mixed tier: 3 full, 2 reduced, and 1 aggregate diagnostic channels, with 880
  scheduled animation evaluations over the declared interval.

## Verified commands

The focused RED run failed before implementation because Cargo could not find
the absent `m6-acceptance-scenes` binary:

```text
cargo test -p crowd-bench --test m6_acceptance_scenes
error: environment variable `CARGO_BIN_EXE_m6-acceptance-scenes` not defined at compile time
```

The GREEN focused run passed all nine tests:

```text
cargo test -p crowd-bench --test m6_acceptance_scenes
test result: ok. 9 passed; 0 failed
```

The prescribed runner then executed the focused suite and two complete reports,
used `cmp` for byte equality, and checked the combined result:

```text
scripts/m6-reference-scenes-test.sh
M6 reference scenes passed twice with exact hashes and metrics: 620e086637a7df18e78c13347e26a3452078bc7fed1d8f65d79d0e81fbc569b8
```

Focused linting also passed:

```text
cargo clippy -p crowd-bench --bin m6-acceptance-scenes --test m6_acceptance_scenes -- -D warnings
```

## Interpretation and limits

This is deterministic, fixture-level integration evidence. It establishes
trajectory/contact/safety accounting, stable replay, and base/unrelated-agent
isolation for the authored reference scenes. It does not establish that the
rejected CMU clips meet joint limits, broad production motion quality,
arbitrary-rig retargeting, Blender visual fidelity, or mixed-tier performance.
Blender layer proof and the 10,000-agent performance gate remain Task 13 scope;
the final requirement-level M6 adjudication remains Task 14 scope.
