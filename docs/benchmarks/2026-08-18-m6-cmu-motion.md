# M6 CMU motion baseline — 2026-08-18

## Result

The fixed CMU evidence lane fetched and hash-verified five declared files,
parsed 1,065 source frames, retained 268 deterministic 30 Hz samples across
three clips, and inferred contact windows in every clip. No source frame was
malformed, no source hash drift or retarget-foot mapping failure occurred, and
the derived soft baselines are 25 mm foot slide, 3 mm trajectory deviation,
61,911 microradians adjacent turn discontinuity, and 0 rejected frames per
million.

The baseline does **not** pass every hard threshold. The official AMC channels
contain 3,587 values outside limits declared by their matching ASF skeletons,
while the checked joint-limit threshold remains zero. The importer did not
clamp, smooth, wrap, or repair those values. This is an evidence finding, not a
reason to loosen the hard gate.

## Environment

- Repository input commit: `8eec59193ece003a2e30cc1b844aa5f2333ef396`
- Host: Apple arm64, macOS 27.0 build 26A5378n, Darwin 27.0.0
- Python: 3.14.2
- Rust: 1.94.1
- Run date: 2026-08-18 (America/Los_Angeles)

## License and provenance boundary

The source is the Carnegie Mellon University Graphics Lab Motion Capture
Database. Its official FAQ permits use, including commercial use, while
prohibiting direct resale of the data. The checked manifest therefore records
`CMU-Mocap-Free-All-Uses` and `redistribution_allowed: false`. Raw ASF/AMC and
the converted database stayed under `/tmp/blender-crowd-m6-cmu`; only source
URLs, exact hashes, code, hand-authored mini fixtures, thresholds, and derived
aggregate reports are repository artifacts.

## Method

The fetcher accepted only the five manifest URLs on `mocap.cs.cmu.edu`, refused
off-host redirects and extra files, and verified SHA-256 before atomic rename.
The importer parsed ASF units, root channel order, bone axes, DOFs, limits, and
hierarchy; parsed fully specified AMC frames; evaluated world-space root and
foot transforms; and retained source frames 1, 5, 9, and so on for deterministic
120 Hz to 30 Hz conversion.

For each foot, local support is the world-height minimum over ±15 retained
samples. Contact requires height within 45 mm of that support and horizontal
speed at most 120 mm/s for at least two samples. Foot slide is the maximum
horizontal displacement inside a declared contact window. Trajectory deviation
compares every valid 120 Hz source-root sample with piecewise-linear 30 Hz root
reconstruction. Turn discontinuity is the maximum wrapped facing delta between
adjacent retained samples. Rejected-frame rate is the ceiling of rejected
frames times one million divided by parsed frames.

Soft limits are the integer ceilings of the maximum per-clip observations with
no headroom. The threshold artifact embeds all five source hashes. Increasing a
soft limit requires a new dated adjudication report. Root teleportation,
undeclared contact, hash drift, cross-cache mutation, and joint-limit violations
retain hard zero limits.

## Per-clip evidence

| Clip | Parsed | Retained | Foot slide (mm) | Trajectory deviation (mm) | Turn delta (µrad) | Joint-limit violations |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `35_01_walk` | 358 | 90 | 25 | 2 | 29,341 | 1,344 |
| `35_24_run` | 150 | 38 | 1 | 3 | 61,911 | 724 |
| `36_01_uneven_walk` | 557 | 140 | 22 | 1 | 42,494 | 1,519 |

## Limitations and unsupported claims

Three short trials cannot establish broad locomotion quality, skeleton
generality, production retarget quality, runtime performance, or compatibility
with arbitrary terrain. The terrain-related evidence is limited to
world-space foot support, source root height/slope observations, contact slide,
and root reconstruction error; it is not a Blender collision or navmesh test.
The report also does not authorize redistribution of raw or converted CMU data.

Because the joint-limit hard gate fails, this baseline does not support a claim
that the selected source clips obey every ASF joint bound. It does support the
narrower claims that acquisition is fixed and reproducible, hashes and
provenance are checked, parsing and kinematics are deterministic, malformed
frames are not repaired, and the measured soft thresholds are traceable to the
dated source-hash set.
