# M3 candidate acceptance audit — 2026-08-12

Decision: **not accepted**. This report records the current candidate state and
the precise evidence still needed for Blender Crowd 1.0; it is not a release
announcement.

## Criterion audit

| M3 criterion | Current evidence | Decision |
| --- | --- | --- |
| Clean install enables, authors, bakes, reloads, and renders on every supported platform | Blender Crowd 1.0 now claims only Blender 5.2 LTS on macOS 11+ Apple Silicon; the post-scope-change clean archive rerun is pending | unproven |
| Complete cache plays without simulator; incomplete/corrupt/stale/older/newer behavior is documented | macOS archive drills pass complete, canceled, corrupt, stale, moved-project, save/reload, and newer-schema cases; incomplete is covered natively, but the older-cache drill is absent | partially proven |
| Undo/save/reload/dependency graph have no hidden state | Existing M2 tests cover authoring undo/save/reload and M3 recovery survives save/reload; linked/library-override, clean-preference, and explicit dependency-graph stress evidence is absent | partially proven |
| Resource, quality, package-size budgets pass | The macOS archive run records bake, memory, cache, playback/render, and package measurements, but M3 fixed thresholds and an enforcing budget report do not exist | unproven |
| Release binaries contain no contributor paths and have reviewed dependencies | The clean archive passes path, wheel, SPDX, and provenance audit; reproducible staging now produces byte-identical ZIPs, but a clean post-fix rebuild and human license review are outstanding | partially proven |
| Required user/schema/troubleshooting/headless/upgrade docs are exercised | Required documents are staged and archive-audited; there is no complete documented exercise/review record for every document | partially proven |
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

The archive-audit contract is unit tested. The clean commit `ca3e147` produced a
Blender-generated macOS archive that passed validation and archive audit. The
corrected harness then passed the 10,000-tick reference workflow from that clean
archive: 1,000 agents, a 50.847-second authorable bake, cache-only replay,
stale/corrupt rejection, and Eevee/Cycles renders.

The M3 recovery drill also passed cancellation, inspection, privacy-safe support
bundle, save/reload, moved-project, and newer-schema rejection. Two acceptance
harness defects were fixed while debugging: Blender Python raises
`RuntimeError` for expected `ERROR`/`CANCELLED` operator reports, and the M3 test
used `json` without importing it. Relative cache-path property flags were fixed
after the drill exposed Blender 5.2 warnings.

Two archives staged independently after timestamp normalization are byte-for-byte
identical at SHA-256
`240a3efb27fb791f0cecdd80c09042124da921b76682ffcdc3ed00393b47d2ea`.
These post-fix archives correctly attest to a dirty tree and therefore are
diagnostic artifacts, not releasable candidates; the fixes need a clean commit,
rebuild, and complete rerun.

## Remaining blocking evidence

The earlier `extension validate` crash is not an add-on blocker. A minimal
factory-startup Blender process crashed inside
`supports_barycentric_whitelist` only under the restricted execution sandbox;
the identical command and the complete release pipeline pass with normal host
Metal access.

The release remains stopped on:

- a clean commit, rebuild, and complete archive-first rerun containing the
  harness, reproducibility, and relative-path fixes;
- a clean macOS-arm64 archive build and complete archive-first rerun after the
  1.0 support contract was narrowed to its sole manifest platform;
- fixed, predeclared thresholds plus an enforcing report for quality,
  performance, peak memory, cache/playback/render, debug overhead, and package
  size (the macOS diagnostic run measured about 3.97 GB peak Blender RSS, an
  874 MiB cache including a 340 MiB behavior-event file, and a 1.07 MB ZIP);
- the absent older-cache, missing-asset, linked/library-override,
  clean-preference, and explicit dependency-graph stress drills;
- a documented signing-channel applicability decision, human license/SBOM
  review, and immutable provenance/evidence retention;
- keyboard/focus, contrast, scaling, readable-label, and assistive-technology
  review, plus two independent evaluator records per claimed platform.
