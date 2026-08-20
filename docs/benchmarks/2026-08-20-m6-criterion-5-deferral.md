# M6 criterion 5 deferral to M9 — 2026-08-20

## Decision

**M6 acceptance criterion 5 — production motion matching against an accepted
clip-state baseline — is rescoped out of M6 and into
[M9](../milestones/M9-neural-animation-operator-validation.md) as Track C.**

The criterion is deferred, not satisfied, weakened, or withdrawn. Every measured
threshold, validator, schema, and evidence requirement moves to M9 unchanged.
M6 makes no production motion-matching claim as a result of this decision.

## Why

The criterion is blocked on motion data acquisition, not on implemented
behavior. The full motion stack required by M6 in-scope item 5 is implemented
and exercised — database build/validation, future-trajectory queries,
pose/contact features, motion matching, stride/turn warping, terrain adaptation,
foot locking, and navigation feedback all run deterministically against checked
fixtures, and their surrounding criteria (3, 4, and 6) pass on that basis.

What is missing is a production motion corpus that meets the unchanged
thresholds:

- The official CMU candidate is **rejected** at 3,587 measured joint-limit
  violations against a hard limit of zero. The importer did not clamp, smooth,
  wrap, or loosen anything to reduce that number, and the rejection stands.
- The accepted CC0 `reference-humanoid-motion` fixture is a narrow
  redistributable baseline for deterministic integration. It is explicitly not a
  production corpus and does not close the criterion.

Acquiring or licensing a conforming replacement corpus has no schedule. Holding
the whole milestone open on an unscheduled data-acquisition dependency does not
make the remaining nine criteria any less proven, and it obscures which work is
actually outstanding. M9 already owns motion data authorization, provenance,
redistribution review, and the learned-motion track, so it is the correct home.

## What moves to M9

The goal transferred to M9 Track C is: **acquire or author a production motion
corpus that satisfies the unchanged M6 motion thresholds, and close the
production motion-matching criterion against it.**

The thresholds and contracts below are carried across verbatim. Changing any of
them is a contract change requiring its own record; it is not something the M9
gate may do to make a candidate pass.

Hard limits (`assets/reference/m6/motion-thresholds-v1.json`, all limit zero):

| Hard limit | Limit |
| --- | ---: |
| `joint_limit_violations` | 0 |
| `root_teleportations` | 0 |
| `undeclared_contacts` | 0 |
| `source_hash_drift` | 0 |
| `cross_cache_mutations` | 0 |

Soft limits:

| Soft limit | Limit |
| --- | ---: |
| `max_foot_slide_millimeters` | 21 |
| `max_trajectory_deviation_millimeters` | 3 |
| `max_turn_discontinuity_microradians` | 60,005 |
| `rejected_frame_rate_ppm` | 0 |

Also carried across unchanged:

- provenance, licensing, and redistribution review before ingestion
  (`../m6-motion-data-policy.md`);
- the retarget/manifest/report identity and source-hash relationships enforced
  by `scripts/m6_acceptance_checks.py motion-source`; and
- the trajectory, contact, foot-slip, turn, terrain, transition, and performance
  evidence required by the original criterion text.

## What did not change

- The `motion_source` gate still executes on every M6 audit run. Malformed,
  inconsistent, or unverified motion evidence still returns `FAILED` and still
  fails M6 closed, because the accepted CC0 fixture it validates is consumed by
  criteria 3, 4, and 6. Only the `OPEN` outcome — "no production candidate meets
  the unchanged thresholds" — is now reported as `DEFERRED TO M9`.
- The CMU candidate's rejection and its 3,587 measured violations remain checked
  in as a real negative result.
- A future candidate that meets every unchanged limit still returns `PASS` from
  the same gate. The deferral does not hardcode the current rejected snapshot as
  the only possible future state.
- No motion threshold, validator, or schema was relaxed as part of this
  decision.

## How M9 closes it

M9 Track C closes when a candidate corpus with reviewed rights and recorded
provenance passes `scripts/m6_acceptance_checks.py motion-source` at `PASS`
against the unchanged thresholds, and the trajectory, contact, foot-slip, turn,
terrain, transition, and performance evidence is published in a dated report.
Until then the criterion remains openly deferred and unclaimed.

## References

- [M6 requirement-level acceptance — 2026-08-20](2026-08-20-m6-acceptance.md)
- [Superseded M6 audit — 2026-08-19](2026-08-19-m6-acceptance.md)
- [CMU candidate measurement — 2026-08-18](2026-08-18-m6-cmu-motion.md)
- [M6 motion data policy](../m6-motion-data-policy.md)
- [M6 contract](../milestones/M6-advanced-agency-motion.md)
- [M9 contract](../milestones/M9-neural-animation-operator-validation.md)
