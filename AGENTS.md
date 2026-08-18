# Repository Guidelines

See [CLAUDE.md](CLAUDE.md) for repository guidelines.

M6 checks: `scripts/m6-foundation-test.sh`; full audit: `scripts/m6-acceptance.sh`.

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
