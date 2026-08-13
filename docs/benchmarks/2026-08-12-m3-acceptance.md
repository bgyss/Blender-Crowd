# M3 acceptance record — 2026-08-12

Decision: **accepted** for Blender 5.2 LTS on macOS 11+ Apple Silicon only.
Windows, Linux, Intel macOS, older macOS, and other Blender versions remain
outside the Blender Crowd 1.0 support contract.

## Criterion audit

| M3 criterion | Accepted evidence | Decision |
| --- | --- | --- |
| Clean installation across the support matrix | The sole support row enables, authors, bakes, reloads, and renders from the clean archive | proven |
| Cache compatibility and recovery | Complete, canceled, incomplete, corrupt, stale, older, newer, moved-project, and save/reload paths follow the published policy | proven |
| Blender lifecycle integrity | Window-context undo/save/reload, clean preferences, linked data/library override, missing asset, and repeated dependency-graph evaluation pass without mutating authoritative cache data | proven |
| Resource, quality, and package budgets | Ten fixed macOS arm64 limits pass; exact solver baselines, release density, strict rebake, and reroute gates pass separately | proven |
| Binary provenance and dependencies | Two builds are byte-identical; native platform, contributor-path, SPDX, lockfile, license review, signing applicability, and clean provenance audits pass | proven |
| Release documentation | Every required document is staged and covered by the documented review and archive-first workflow | proven |
| Limitations and recovery | Public limitation, compatibility, recovery, support-bundle, and support-matrix policies are staged and tested | proven |

Independent evaluator records are not silently waived. They are deferred to M7,
where two non-implementers must complete the documented production workflow and
record task success, recovery, accessibility findings, and limitations.

## Accepted artifact

- Source revision: `bbde2c2a81b02075721ae3c89a63bf2369a7bec8`.
- Archive: `blender_crowd-1.0.0.zip`, 1,077,567 bytes.
- SHA-256: `253c35429df7f1e0239241f66c28cb5374ca1817bebeb807fc856d953b47d351`.
- Two independent clean builds produced that exact byte sequence.
- Archive provenance records `source_dirty: false` and the same full revision.
- Manifest platforms are exactly `macos-arm64` with Blender 5.2.0 minimum.

The build output retains `SHA256SUMS`, `archive-audit.json`, the SPDX 2.3 SBOM,
and release provenance beside the archive. This committed report and its
[machine-readable companion](2026-08-12-m3-acceptance.json) preserve the exact
artifact identity and acceptance measurements.

## Archive-first evidence

```text
scripts/m3-build-release.sh (two clean outputs)          PASS, byte-identical
scripts/m3-acceptance.sh --archive … --out …             PASS
  archive audit                                          PASS
  1,000-agent / 10,000-tick bake and cache-only render  PASS
  stale, corrupt, and missing-asset rejection            PASS
  canceled, older/newer, moved-project recovery          PASS
  clean preference, linked/override, dependency graph    PASS
  accessibility invariants                               PASS
  fixed budget audit, 10/10                              PASS
  reviewed release-policy audit                          PASS
cargo test --release -p crowd-core --test fuzz_density  PASS, 4/4
cargo run --release -p crowd-bench -- check --agents 1000
                                                        PASS, 6/6 baselines
cargo test --release -p crowd-core --test m1_strict -- --ignored --nocapture
                                                        PASS
cargo test --release -p crowd-core --test two_room_reroute -- --ignored --nocapture
                                                        PASS
scripts/m2-reference-acceptance.sh                       PASS
cargo test --workspace (release-density tests skipped)  PASS
python3 -m unittest tests/test_m3_release_audit.py tests/test_m3_budget_audit.py tests/test_m3_policy_audit.py tests/test_m0_acceptance_runner.py
                                                        PASS, 13/13
cargo fmt --check                                       PASS
cargo clippy --workspace --all-targets -- -D warnings   PASS
git diff --check                                        PASS
```

## Enforced measurements

| Metric | Measured | Fixed gate | Result |
| --- | ---: | ---: | --- |
| Authorable bake | 56.164 s | <= 120 s | pass |
| Peak Blender resident memory | 3,141,042,176 bytes | <= 6 GiB | pass |
| Cache size | 915,727,474 bytes | <= 1.25 GiB | pass |
| Sequential native cache scan | 19,462.34 ticks/s | >= 100 ticks/s | pass |
| Cached-agent debug query | 0.00519 s/query | <= 0.1 s/query | pass |
| Point upload | 0.00636 s | <= 0.05 s | pass |
| 31-frame canonical armature evaluation | 0.00403 s | <= 0.1 s | pass |
| Eevee reference render | 1.192 s | <= 5 s | pass |
| Cycles CPU reference render | 0.093 s | <= 5 s | pass |
| Extension archive | 1,077,567 bytes | <= 2 MiB | pass |

The archive workflow rendered tick 4,999 with 700 proxy instances through
Eevee and Cycles without a simulation session. Sparse correction left the base
cache unchanged. The debug-performance fix retains the most recent selected
agent event query instead of reparsing the 355,672,491-byte event file on every
inspection.

## Release-policy decision

The deterministic SBOM contains 142 locked packages and no missing license
assertions. Its hash and the Cargo lockfile hash are pinned in the reviewed
policy; changing either invalidates the archive gate. Blender's documented
direct-ZIP/static-repository workflow defines build, validation, installation,
and repository generation but no package-signing operation, so 1.0 records
signing as not applicable to this channel and uses reproducibility, SHA-256,
SBOM, exact platform tags, and source provenance as integrity controls.

The accessibility review verifies extension-owned behavior: stock Blender
controls, no custom drawing or keymaps, an open primary workflow, a closed and
labeled Advanced raw-data panel, persistent text diagnostics, and readable
operator labels/descriptions. Host-native contrast, focus, scaling, and
assistive-technology behavior remain Blender responsibilities; real-user study
evidence belongs to M7.
