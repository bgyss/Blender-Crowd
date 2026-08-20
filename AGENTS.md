# Repository Guidelines

See [CLAUDE.md](CLAUDE.md) for repository guidelines.

M6 checks: `scripts/m6-foundation-test.sh` and
`scripts/m6-extension-examples-test.sh`; full host audit:
`M6_RUN_BLENDER=1 scripts/m6-acceptance.sh`.
M6 remains unaccepted while criterion 5 is open: the CMU candidate is rejected
at 3,587 joint-limit violations against the hard limit of zero, and the CC0
authored motion is a checked fixture baseline only. `M6_ALLOW_OPEN=1`
acknowledges an open audit; it does not promote the milestone. Preserve the
unsupported-claim and M9 boundaries in
`docs/benchmarks/2026-08-19-m6-acceptance.md`.
The production-motion result is evidence-driven: a valid future candidate may
PASS unchanged thresholds, the current rejected candidate is OPEN, and
malformed or inconsistent evidence is FAILED.

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
