# Repository Guidelines

See [CLAUDE.md](CLAUDE.md) for repository guidelines.

M6 checks: `scripts/m6-foundation-test.sh` and
`scripts/m6-extension-examples-test.sh`; full host audit:
`M6_RUN_BLENDER=1 scripts/m6-acceptance.sh`.
M6 is accepted with criterion 5 (production motion matching) deferred to M9
Track C on 2026-08-20. It is deferred, not satisfied: the CMU candidate is
rejected at 3,587 joint-limit violations against the hard limit of zero, the CC0
authored motion is a checked fixture baseline only, and every measured threshold
moved to M9 unchanged. Never state or imply a production motion-matching result
for M6. `M6_ALLOW_OPEN=1` acknowledges an open audit; it does not promote the
milestone or a failed gate. Preserve the unsupported-claim and M9 boundaries in
`docs/benchmarks/2026-08-20-m6-acceptance.md` and
`docs/benchmarks/2026-08-20-m6-criterion-5-deferral.md`.
The production-motion result stays evidence-driven: a valid future candidate may
PASS the unchanged thresholds, the current rejected candidate is OPEN and
adjudicated as DEFERRED TO M9, and malformed or inconsistent evidence is FAILED
and still fails M6 closed.

## Blender automation on macOS

Run every Blender process with normal host Metal access. Do not launch Blender
inside a restricted automation sandbox: Metal device discovery can return no
device and abort Blender before Python starts. If a backtrace ends in
`gpu::MTLBackend::metal_is_supported` or
`MTLCreateSystemDefaultDevice()` returns `nil`, rerun the same repository
runner outside the restricted sandbox before diagnosing add-on code.

Source-add-on runners must load the current checkout and a freshly built native
wheel, not a potentially stale extension from the user's Blender profile. Keep
`--python-use-system-env` whenever `PYTHONPATH` supplies those paths; Blender
5.2 otherwise ignores them. Follow the M4/M5/M6 runner pattern when adding a
new Blender workstream.
