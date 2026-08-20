# Task 14 — External examples and requirement-level audit

Status: COMPLETE — TASK 14 VERIFIED; M6 REMAINS OPEN

Commit: included in the Task 14 commit identified in the final handoff. This
report is part of that commit, so it does not embed a self-referential Git hash.

No subagent or reviewer was dispatched.

## Result

Task 14 is complete without promoting the M6 proof level. The repository now
has executable Rust and Python external-extension examples, execution tests for
the four required outcomes, a requirement-level acceptance runner, a dated
criterion-by-criterion report, and aligned M6 status documentation.

M6 remains unaccepted. Criteria 1–4 and 6–10 are adjudicated PASS at their
documented deterministic fixture or host-Blender proof levels. Criterion 5 is
OPEN because the CMU candidate has 3,587 measured joint-limit violations
against the hard limit of zero. The accepted CC0 authored motion is a narrow
fixture baseline only and was not promoted into production-motion evidence.

`scripts/m6-acceptance.sh` exits 2 without `M6_ALLOW_OPEN=1` while criterion 5
is OPEN. The override acknowledges OPEN status only. A FAILED deterministic
gate still exits 2 even when the override is present. R1–R4 neural work and
independent-user verification remain informational M9 deferrals.

## Implementation

### External examples

- Registered `examples/m6-extension-rust.rs` as the `crowd-core`
  `m6-extension-rust` example target.
- Added `examples/m6_extension_python.py` against the coarse Python facade.
- Both examples declare schema and channel versions, input/output channels, a
  fixed cost budget, deterministic mode, and failure isolation.
- Both execute one accepted call, one over-budget fallback, one undeclared-input
  rejection, and one version-mismatch rejection as newline-delimited JSON.
- No C or C++ API or compatibility claim was added.

### Acceptance runner

- Replaced the historical open-gate summary with executable foundation,
  debugger/library, motion-source, reference-scene, optional host-Blender,
  mixed-tier, extension-example, report-schema, and release-workspace gates.
- Added explicit PASS, OPEN, and FAILED propagation from gates to all ten M6
  criteria and the overall audit result.
- Preserved the CMU hard-gate failure as an OPEN milestone criterion while
  separately recording the accepted CC0 authored fixture baseline.
- Added test-only gate simulation to prove exit and status behavior without
  replaying long component lanes.
- Added a machine-neutral report validator and motion-source ruling checker in
  `scripts/m6_acceptance_checks.py`.
- Extended the foundation runner to compile and execute both examples and their
  tests.

### Evidence and documentation

- Added `docs/benchmarks/2026-08-19-m6-acceptance.md` with environment,
  recomputed input hashes, direct evidence for every criterion, rejected inputs,
  unsupported claims, M9 deferrals, and recorded versus fresh verification.
- Updated the M6 foundation report, milestone contract, milestone index,
  README, CLAUDE, and AGENTS guidance to name criterion 5 as the single open M6
  criterion.
- Explicitly excluded unsupported Blender cloth/hair/Geometry Nodes
  deformation, rigid-body parity, GPU and arbitrary-scene performance,
  long-duration stability, neural motion, visual quality, and operator-study
  claims.

## RED/GREEN evidence

### RED 1 — rejected production motion was incorrectly promoted

The focused acceptance-runner regression was changed first to require OPEN
criterion 5 and a nonzero exit. Against the inherited partial runner it failed:

```text
AssertionError: 0 != 2
M6 acceptance audit: PASS
CMU source candidate: REJECTED
accepted motion baseline: PASS
criterion 5: PASS
remaining M6 gates: none
```

This proved that the partial runner treated a correctly rejected production
candidate plus the lower-level CC0 fixture as a closed M6 motion gate.

### GREEN 1 — fail-closed criterion propagation

After introducing the explicit OPEN motion-source gate, focused runner tests
prove:

- host Blender simulated PASS plus all other simulated PASS gates still exits 2
  with criterion 5 OPEN;
- `M6_ALLOW_OPEN=1` changes only that OPEN audit's process exit to zero;
- an omitted Blender lane remains OPEN and exits 2; and
- a simulated failed extension gate remains FAILED and exits 2 even with
  `M6_ALLOW_OPEN=1`.

### RED 2 — no reusable report ruling

The report-boundary test was added before implementation and failed with:

```text
AttributeError: module 'm6_acceptance_checks' has no attribute 'check_acceptance_report'
```

### GREEN 2 — report status and proof boundaries are executable

The checker now requires the dated report structure, exactly nine PASS rows and
criterion 5 OPEN, the CMU/CC0 distinction, every unsupported-claim phrase, M9
deferrals, and no contributor-local path.

Final focused example/audit result:

```text
python3 -m unittest -v tests/test_m6_extension_examples.py
Ran 8 tests in 0.687s
OK
```

The example tests invoke the real Python script and the real Cargo example; no
mock output is asserted.

## Focused verification

The following checks were run during Task 14 and exited zero:

```text
python3 -m unittest -v tests/test_m6_extension_examples.py
cargo test -p crowd-core --test m6_extensions
cargo fmt --all -- --check
bash -n scripts/m6-acceptance.sh scripts/m6-foundation-test.sh
python3 -m py_compile scripts/m6_acceptance_checks.py examples/m6_extension_python.py tests/test_m6_extension_examples.py
python3 scripts/m6_acceptance_checks.py motion-source
python3 scripts/m6_acceptance_checks.py acceptance-report --report docs/benchmarks/2026-08-19-m6-acceptance.md
scripts/m6-acceptance.sh --list
git diff --check
```

Exact focused Rust result:

```text
running 2 tests
test extension_channel_requires_declared_inputs_cost_and_failure_isolation ... ok
test extension_manifest_rejects_non_deterministic_or_non_isolated_channels ... ok
test result: ok. 2 passed; 0 failed
```

Exact source/report ruling:

```text
CMU source candidate remains rejected: 3587 joint-limit violations > hard limit 0
Accepted motion baseline: checked CC0-1.0 authored data
M6 acceptance report structure: PASS; milestone status: OPEN
```

The scoped documentation privacy scan found no contributor-local path in the
new acceptance report. Its only hits elsewhere were intentional existing
platform interfaces: the documented Blender application path and the M5
external artifact-root example.

## Recorded long-running evidence not repeated

Per the Task 14 instruction, the already completed
`cargo test --workspace --release` run was not repeated. The preceding worker's
recorded evidence is retained:

- full optimized Rust workspace: PASS;
- release density-fuzz portion: PASS in 498.29 seconds;
- host Blender M6 runner with normal Metal access: PASS;
- M6 foundation: PASS;
- reference scenes: PASS twice with exact replay identity;
- fixed 10K mixed-tier lane: PASS;
- clippy with warnings denied: PASS;
- Python checks: PASS; and
- Rust formatting: PASS.

The unmodified full acceptance runner was likewise not executed because it
would repeat that release workspace and density-fuzz lane. Its orchestration and
exit semantics were covered by the focused real-process tests above. The runner
will remain nonzero in a normal run until criterion 5 is closed, even when all
other component gates pass.

## Self-review

The complete Task 14 diff was reviewed against the brief and all current M6
reports. The review confirmed:

- every required Task 14 artifact exists;
- the requested August 19 acceptance-report path supersedes the brief's older
  August 18 filename;
- examples use the real checked Rust/Python extension boundaries;
- the runner executes every prescribed deterministic component and full release
  workspace in normal mode;
- the report checker cannot accept ten PASS rows while the CMU gate remains
  open;
- `M6_ALLOW_OPEN` cannot hide a FAILED gate;
- M9 lines never enter M6 criterion status;
- no C/C++ wrapper or claim was invented;
- the report contains no contributor-local path; and
- unrelated user changes were not discarded.

One shell robustness issue was found and fixed during self-review:
`check_motion_source_ruling` now explicitly chains its source ruling and Python
motion suites with `&&`, so a failed source check cannot be masked by a later
passing unit-test command when the function is evaluated in a shell conditional.

## Files changed

- `examples/m6-extension-rust.rs`
- `examples/m6_extension_python.py`
- `tests/test_m6_extension_examples.py`
- `scripts/m6_acceptance_checks.py`
- `scripts/m6-acceptance.sh`
- `scripts/m6-foundation-test.sh`
- `crates/crowd-core/Cargo.toml`
- `docs/benchmarks/2026-08-19-m6-acceptance.md`
- `docs/benchmarks/2026-08-18-m6-foundation.md`
- `docs/milestones/M6-advanced-agency-motion.md`
- `docs/milestones/README.md`
- `README.md`
- `CLAUDE.md`
- `AGENTS.md`
- `.superpowers/sdd/2026-08-18-m6-advanced-agency-motion/task-14-report.md`

## Remaining concerns

- M6 is still OPEN, not accepted. Criterion 5 needs a licensed production
  motion candidate that passes every unchanged hard threshold.
- The existing CC0 fixture remains valid deterministic integration evidence but
  must not be advertised as broad production motion quality.
- Current Blender evidence does not support cloth/hair/Geometry Nodes
  deformation, rigid-body parity, arbitrary collision scenes, visual quality,
  or artist-usability claims.
- Current performance evidence is a fixed CPU fixture, not GPU, arbitrary-scene,
  long-duration, viewport/render, or Cache v1 streaming evidence.
- Neural R1–R4 and independent-user evidence remain deferred to M9.
