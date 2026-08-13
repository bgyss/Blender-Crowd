# M3 candidate acceptance audit — 2026-08-12

Decision: **not accepted**. This report records the current candidate state and
the precise evidence still needed for Blender Crowd 1.0; it is not a release
announcement.

## Criterion audit

| M3 criterion | Current evidence | Decision |
| --- | --- | --- |
| Clean install enables, authors, bakes, reloads, and renders on every supported platform | The sole 1.0 row, Blender 5.2 LTS on macOS 11+ Apple Silicon, passes from clean archive `1af41f8f…79da07` | proven |
| Complete cache plays without simulator; incomplete/corrupt/stale/older/newer behavior is documented | The archive drills pass complete, canceled, corrupt, stale, moved-project, save/reload, and newer-schema cases; incomplete is covered natively, but the older-cache drill is absent | partially proven |
| Undo/save/reload/dependency graph have no hidden state | The window-context authoring suite passes undo/save/reload and native revalidation; M3 recovery survives save/reload, but linked/library-override, clean-preference, and explicit dependency-graph stress evidence is absent | partially proven |
| Resource, quality, package-size budgets pass | Six fixed solver baselines, release density, strict rebake, reroute, and measurements pass; M3 does not define or enforce release thresholds for peak memory, cache/render/package size, or debug overhead | unproven |
| Release binaries contain no contributor paths and have reviewed dependencies | Two clean full builds are byte-identical; path, platform, wheel, SPDX, and provenance audits pass, but human license/SBOM review is absent | partially proven |
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

## Final candidate identity

- Source revision: `0a6db9196d7aa85cc7e2060bf9f425e8ff3382db`.
- Claimed matrix: Blender 5.2 LTS, macOS 11+, Apple Silicon only.
- Archive: `blender_crowd-1.0.0.zip`, 1,063,131 bytes.
- Archive SHA-256: `1af41f8fc69abb7839e2a23f0966dd5f636eac9d9dcb6101172a3cb1af79da07`.
- Two independent complete builds from the clean revision produced the same
  archive hash. The release audit confirms `source_dirty: false` provenance and
  exactly `platforms = ["macos-arm64"]`.

## Fresh verification

```text
scripts/m3-build-release.sh (two clean outputs)          PASS, byte-identical
scripts/m3-acceptance.sh --archive … --out …             PASS
cargo test --workspace (release-density tests skipped)  PASS
cargo test --release -p crowd-core --test fuzz_density  PASS, 4/4 in 570.69 s
cargo test --release -p crowd-core --test two_room_reroute -- --ignored --nocapture
                                                        PASS, 1/1 in 25.64 s
cargo run --release -p crowd-bench -- check --agents 1000
                                                        PASS, 6/6 baselines
cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture
                                                        PASS, 1/1 in 36.06 s
scripts/m2-reference-acceptance.sh                       PASS, 1/1 in 50.38 s
scripts/m2-blender-authoring-test.sh                     PASS, undo/save/reload
scripts/cache-experiment.sh                              PASS, 9 candidates
scripts/verify-wheel.sh 25                              PASS
python3 -m unittest tests/test_m3_release_audit.py tests/test_m0_acceptance_runner.py
                                                        PASS, 9/9 at audit time
cargo fmt --check                                       PASS
cargo clippy --workspace --all-targets -- -D warnings   PASS
git diff --check                                        PASS
```

The clean archive workflow baked 1,000 agents through 10,000 ticks in 59.555
seconds, replayed without a simulation session, rejected stale and corrupt
caches, preserved the sparse override boundary, and rendered tick 4,999 with
700 proxy instances through Eevee and Cycles.

The measured archive-run outputs were 3,768,778,752 bytes peak Blender RSS,
915,727,474 bytes of cache data including a 355,672,491-byte behavior-event
file, 0.00630 seconds point upload, 0.00319 seconds canonical armature
evaluation, 0.616 seconds Eevee GPU render, and 0.086 seconds Cycles CPU render.
These are measurements, not M3 threshold passes, because the M3 release budget
contract is not yet defined.

The recovery drill passed cancellation, inspection, privacy-safe support bundle,
save/reload, moved-project resolution, and newer-schema rejection. The complete
audit also exposed and fixed a source-install staging-path collision and
non-reproducible maturin wheel ZIP timestamps before the final clean rerun.

## Remaining blocking evidence

The earlier `extension validate` crash is not an add-on blocker. A minimal
factory-startup Blender process crashed inside
`supports_barycentric_whitelist` only under the restricted execution sandbox;
the identical command and the complete release pipeline pass with normal host
Metal access.

The release remains stopped on:

- fixed, predeclared thresholds plus an enforcing report for peak memory,
  cache/playback/render, debug overhead, and package size;
- the absent older-cache, missing-asset, linked/library-override,
  clean-preference, and explicit dependency-graph stress drills;
- a documented signing-channel applicability decision, human license/SBOM
  review, and publication of immutable provenance/evidence;
- keyboard/focus, contrast, scaling, readable-label, and assistive-technology
  review, plus two independent evaluator records on the sole claimed platform.
