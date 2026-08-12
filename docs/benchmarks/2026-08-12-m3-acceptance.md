# M3 candidate acceptance audit — 2026-08-12

Decision: **not accepted**. This report records the current candidate state and
the precise evidence still needed for Blender Crowd 1.0; it is not a release
announcement.

## Criterion audit

| M3 criterion | Current evidence | Decision |
| --- | --- | --- |
| Clean install enables, authors, bakes, reloads, and renders on every supported platform | Archive-first runner exists and calls the 10,000-tick reference workflow, but macOS Blender crashes in `extension validate`; Windows/Linux archives are not built | unproven |
| Complete cache plays without simulator; incomplete/corrupt/stale/older/newer behavior is documented | Native wheel verifies complete/corrupt/canceled cache states; archive drills cover canceled, corrupt, stale, moved-project, and newer-schema states but await Blender execution | partially proven |
| Undo/save/reload/dependency graph have no hidden state | Existing M2 tests cover authoring undo/save/reload; M3 cache-recovery test is implemented but blocked from execution by the host crash | unproven |
| Resource, quality, package-size budgets pass | Existing M0–M2 evidence remains available; no release-archive support-matrix budget run exists | unproven |
| Release binaries contain no contributor paths and have reviewed dependencies | Archive auditor checks text for common contributor paths and validates staged SPDX/provenance; no Blender-generated release archive or human license review exists | partially proven |
| Required user/schema/troubleshooting/headless/upgrade docs are exercised | Recovery, headless release, compatibility, support matrix, checklist, and limitations documents are staged and archive-audited; archive workflow is blocked | partially proven |
| Limitations and recovery are public and specific | Candidate limitations and recovery policy are checked in; independent evaluator review remains outstanding | partially proven |

## Implemented M3 controls

- Persistent workflow stage, selection context, next action, cache health,
  recovery guidance, exact minimum point-buffer preflight, measured cache size,
  and saved diagnostic history.
- Cache inspection that never attaches incomplete/canceled data, reload-time
  revalidation that hides stale saved point geometry, project source-hash
  stale-cache rejection, corrupt-complete-cache detection, relative moved-cache
  recovery, and newer-schema rejection.
- A safe support bundle that deliberately omits scene contents, absolute paths,
  and diagnostic detail.
- Version 1.0.0 metadata, lockfile, ABI wheel builder, staged release docs,
  deterministic SPDX inventory, git provenance, archive audit, and
  archive-first acceptance entrypoint.

## Verified local checks

```text
python3 -m unittest tests/test_m3_release_audit.py       PASS
scripts/build-wheel.sh                                  PASS (1.0.0 macOS arm64 abi3 wheel)
scripts/verify-wheel.sh 25                              PASS (complete/corrupt/canceled FFI cache checks)
cargo test -p crowd-blender --lib                       PASS (8 native bridge tests)
cargo test -p crowd-cache                               PASS
cargo test -p crowd-core --test behavior_graph          PASS
cargo clippy --workspace --all-targets -- -D warnings   PASS
cargo fmt --check                                       PASS
```

The archive-audit contract is unit tested. A manually staged zip from this
dirty development tree is intentionally rejected for dirty provenance; even a
passing clean staging audit would not be a Blender-generated installable archive
and therefore could not close the install criterion.

## Blocking evidence

`scripts/m3-build-release.sh` builds the wheel and stages the extension, then
the local Blender 5.2.0 LTS process writes `blender.crash.txt` and exits with
segmentation fault 11 during `Blender --command extension validate`. The same
failure was observed through the clean-install runner. This occurs before the
release archive is created or any add-on code is loaded.

Windows/Linux platform builds, signing, support-matrix budget data, and the two
independent evaluator records are also absent. The release remains stopped.
