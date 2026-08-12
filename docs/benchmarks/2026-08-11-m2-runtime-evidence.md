# M2 authorable runtime evidence — 2026-08-11

Historical status: the runtime-evidence subgate passed on 2026-08-11, while M2
was still awaiting operator reproduction. M2 was subsequently accepted on
2026-08-12; see the [final acceptance record](2026-08-12-m2-acceptance.md).

The checked [JSON report](2026-08-11-m2-runtime-evidence.json) was generated
by `scripts/m2-reference-acceptance.sh` from the 1,000-agent reference
configuration over the complete 10,000-tick shot.

It records 6,752,500 graph decisions, 1,001 queue requests, four deterministic
admissions, six queue releases, and an initial group split report. The runner
proves that the compiled authorable controller executes through the whole
reference duration and emits the expected M2 runtime evidence.

It does not prove the remaining visual authoring, Blender save/reload, terrain
presentation, or independent artist-reproduction gates. Therefore the report
intentionally carries `m2_milestone_accepted: false`.
