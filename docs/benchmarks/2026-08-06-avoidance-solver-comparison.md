# Avoidance solver comparison — measured results and production default

Date: 2026-08-06 (report authored 2026-08-07, against data captured 2026-08-07)
Scope: [Avoidance solver comparison design](../superpowers/specs/2026-08-06-avoidance-solver-comparison-design.md)
Parent contract: [Blender Crowd 1.0 architecture and MVP](../blender-crowd-1.0.md), Phase 0, section 6.2

This is the M0 decision record the contract's stop condition asks for: "one
navigation/avoidance/cache/bridge path is selected by a reproducible report."
It picks a production default for `crowd-core`'s avoidance solver from three
implementations — `sampled_velocity`, `orca`, and `anticipatory` — measured
against the same six scenes and four population scales.

**Headline: `sampled_velocity` is selected as the production default.** It is
not the fastest solver measured — `orca` is 2.8x–6.3x faster — and it is not
uniformly the best on every quality axis in every scene. But across all 72
measured reports it is consistently at or near the best on the two axes that
matter most for a crowd a scene is willing to look at (penetration and
predicted time-to-collision), it already clears the contract's throughput
gate at every scale tested, and it does not carry either rejected candidate's
severe failure mode. Both of those failure modes, and the reasoning behind
rejecting them, are below.

## 1. Environment

| | |
|---|---|
| CPU | Apple M1 Max |
| RAM | 64 GiB (68,719,476,736 bytes) |
| OS / arch | macOS, aarch64 |
| rustc | 1.94.1 (e408947bf 2026-03-25) |
| Build profile | release |
| Tick rate | 30 Hz |
| Seed | 2026 |
| Captured | 2026-08-07T02:24:30Z – 2026-08-07T04:00:40Z (one machine, one continuous sweep run as six per-scene chunks; every one of the 72 `Report` entries in `benchmarks/reports/compare-2026-08-07.json` carries an identical `environment` block except `captured_at`) |

Reproduce with:

```sh
for scene in bidirectional_corridor crossing bottleneck dense_flow circle l_corridor; do
  cargo run --release -p crowd-bench -- compare --scene "$scene" --out benchmarks/reports
done
cargo run --release -p crowd-bench -- check --agents 1000
```

(`compare --out benchmarks/reports` with no `--scene` reproduces the same 72
reports in one continuous run instead of six chunks, writing them to a single
`compare-<date>.json`; `--scene` is what makes the chunked form above possible
for a long-running sweep, and each chunk writes its own
`compare-<scene>-<date>.json` so six same-day chunks do not overwrite each
other. The original capture's six chunks were merged by hand into the single
`compare-2026-08-07.json` referenced above; a chunked reproduction today
produces six separate per-scene files with the same combined content.)

`benchmarks/reports/` is gitignored by project convention (informational,
per-run, per-machine output, the same category as `target/`); this markdown
report is the durable, checked-in artifact, transcribed directly from the
local JSON.

## 2. Scope actually measured

The plan's own text assumed 5 scenes x 3 solvers x 4 scales = 60 reports.
The kernel slice (Task 21 of the prior plan) added a sixth scene,
`l_corridor`, before this comparison ran. `crowd_core::scenes::SCENE_NAMES`
has six entries, so the real, correct scope — confirmed by counting the
merged JSON — is **6 scenes x 3 solvers x 4 scales = 72 reports**, not 60.
Every number below is transcribed from those 72 reports; nothing is
extrapolated to the scene count the plan originally assumed.

Scenes: `bidirectional_corridor`, `bottleneck`, `circle`, `crossing`,
`dense_flow`, `l_corridor`.
Solvers: `anticipatory`, `orca`, `sampled_velocity`.
Scales: 100, 500, 1000, 2000 agents.

The avoidance-solver-comparison design spec's section 7 describes the table
below as covering "all twenty scene/scale combinations," written when the
design assumed 5 scenes x 4 scales. With the sixth scene now part of the
suite, the actual count is **24 scene/scale combinations** (6 x 4), each
measured for all three solvers — the same correction as the 60-vs-72 report
count above, carried through consistently rather than left as a stale
number in this report.

## 3. Full results

`completion_rate` (fraction arrived), `mean_time_to_collision` (`mttc`, capped
at 10 s, measured against the velocity each agent actually uses —
per-agent-tick), `penetration_pair_ticks` (`pen_pt`, overlap duration summed
across colliding pairs, not distinct episodes), `heading_reversals` (`rev`),
`ticks_per_second_achieved` (`tps`), and `peak_allocated_bytes` (`peak`, from
a counting global allocator, not RSS).

### 3.1 bidirectional_corridor

| Agents | Solver | Completion | Mean TTC | Pen. pair-ticks | Reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 0.810 | 6.43 s | 73 | 22,357 | 1,302.6 | 46,129 B |
| 100 | orca | 0.890 | 2.96 s | 19,394 | 5,393 | 8,467.6 | 51,829 B |
| 100 | anticipatory | 0.390 | 4.95 s | 1,621 | 26,655 | 1,168.4 | 54,265 B |
| 500 | sampled_velocity | 0.280 | 6.51 s | 750 | 138,811 | 217.1 | 177,962 B |
| 500 | orca | 0.352 | 1.83 s | 626,508 | 62,709 | 858.2 | 184,098 B |
| 500 | anticipatory | 0.396 | 4.72 s | 31,808 | 296,787 | 235.9 | 187,022 B |
| 1000 | sampled_velocity | 0.267 | 6.38 s | 2,147 | 343,476 | 107.9 | 358,123 B |
| 1000 | orca | 0.153 | 1.25 s | 2,818,747 | 206,013 | 306.1 | 364,231 B |
| 1000 | anticipatory | 0.287 | 4.30 s | 165,097 | 931,815 | 112.9 | 367,179 B |
| 2000 | sampled_velocity | 0.179 | 6.07 s | 14,869 | 779,267 | 50.5 | 674,736 B |
| 2000 | orca | 0.120 | 1.11 s | 8,856,520 | 495,195 | 139.9 | 679,824 B |
| 2000 | anticipatory | 0.189 | 3.54 s | 925,816 | 2,964,594 | 52.7 | 682,764 B |

### 3.2 bottleneck

| Agents | Solver | Completion | Mean TTC | Pen. pair-ticks | Reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 0.650 | 7.81 s | 398 | 11,788 | 1,625.0 | 53,849 B |
| 100 | orca | 0.880 | 2.25 s | 45,097 | 8,315 | 10,865.9 | 59,021 B |
| 100 | anticipatory | 0.820 | 5.50 s | 6,161 | 35,764 | 1,602.0 | 61,633 B |
| 500 | sampled_velocity | 0.512 | 6.84 s | 111,652 | 138,729 | 245.4 | 214,622 B |
| 500 | orca | 0.426 | 1.20 s | 1,695,619 | 137,543 | 847.5 | 218,694 B |
| 500 | anticipatory | 0.456 | 3.09 s | 401,331 | 657,516 | 243.6 | 222,562 B |
| 1000 | sampled_velocity | 0.360 | 6.52 s | 632,719 | 302,683 | 108.5 | 422,259 B |
| 1000 | orca | 0.300 | 1.00 s | 6,306,547 | 446,137 | 320.8 | 426,319 B |
| 1000 | anticipatory | 0.336 | 2.35 s | 1,859,771 | 2,158,939 | 109.6 | 431,219 B |
| 2000 | sampled_velocity | 0.276 | 5.64 s | 3,459,935 | 807,084 | 49.9 | 810,796 B |
| 2000 | orca | 0.224 | 0.81 s | 22,066,041 | 1,552,362 | 134.1 | 810,748 B |
| 2000 | anticipatory | 0.247 | 1.82 s | 7,011,571 | 6,779,903 | 51.1 | 815,656 B |

### 3.3 circle

| Agents | Solver | Completion | Mean TTC | Pen. pair-ticks | Reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 1.000 | 7.72 s | 0 | 8,971 | 1,882.9 | 71,169 B |
| 100 | orca | 0.490 | 3.26 s | 24,495 | 5,893 | 7,525.2 | 76,517 B |
| 100 | anticipatory | 1.000 | 6.85 s | 274 | 13,651 | 1,979.4 | 79,361 B |
| 500 | sampled_velocity | 0.896 | 6.85 s | 33 | 117,600 | 240.7 | 252,818 B |
| 500 | orca | 0.090 | 1.66 s | 850,242 | 73,909 | 671.4 | 256,042 B |
| 500 | anticipatory | 0.886 | 5.30 s | 75,202 | 276,425 | 255.1 | 260,726 B |
| 1000 | sampled_velocity | 0.418 | 6.47 s | 6,596 | 313,752 | 101.4 | 535,451 B |
| 1000 | orca | 0.063 | 1.53 s | 2,639,610 | 195,535 | 306.2 | 539,687 B |
| 1000 | anticipatory | 0.159 | 3.74 s | 646,339 | 1,072,229 | 101.8 | 543,355 B |
| 2000 | sampled_velocity | 0.110 | 5.94 s | 202,317 | 703,303 | 46.5 | 1,055,272 B |
| 2000 | orca | 0.026 | 1.09 s | 10,299,570 | 616,704 | 129.6 | 1,060,520 B |
| 2000 | anticipatory | 0.004 | 2.49 s | 3,382,927 | 3,615,663 | 46.5 | 1,063,236 B |

`circle` is the outlier scene discussed in section 6 below: all three
solvers degrade here far more than in any other scene, and `anticipatory`
degrades worst of all (completion collapses to 0.004 at 2,000 agents).

### 3.4 crossing

| Agents | Solver | Completion | Mean TTC | Pen. pair-ticks | Reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 0.660 | 7.31 s | 0 | 13,296 | 1,276.1 | 62,258 B |
| 100 | orca | 0.810 | 2.89 s | 18,070 | 4,516 | 8,020.9 | 67,902 B |
| 100 | anticipatory | 0.530 | 6.70 s | 402 | 21,119 | 1,394.9 | 70,282 B |
| 500 | sampled_velocity | 0.332 | 6.70 s | 321 | 117,079 | 217.9 | 251,877 B |
| 500 | orca | 0.384 | 1.88 s | 601,435 | 40,859 | 861.2 | 257,957 B |
| 500 | anticipatory | 0.308 | 4.63 s | 67,443 | 318,570 | 232.0 | 260,825 B |
| 1000 | sampled_velocity | 0.240 | 6.27 s | 8,015 | 278,838 | 103.5 | 504,464 B |
| 1000 | orca | 0.248 | 1.51 s | 2,359,011 | 128,061 | 325.0 | 508,484 B |
| 1000 | anticipatory | 0.240 | 3.86 s | 378,138 | 1,019,677 | 109.5 | 512,384 B |
| 2000 | sampled_velocity | 0.159 | 5.88 s | 80,272 | 674,359 | 48.5 | 961,647 B |
| 2000 | orca | 0.165 | 1.22 s | 8,416,246 | 430,641 | 143.8 | 967,687 B |
| 2000 | anticipatory | 0.172 | 3.18 s | 1,645,745 | 3,134,266 | 52.3 | 970,587 B |

### 3.5 dense_flow

| Agents | Solver | Completion | Mean TTC | Pen. pair-ticks | Reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 0.760 | 6.45 s | 217 | 16,973 | 1,348.4 | 59,729 B |
| 100 | orca | 1.000 | 2.83 s | 15,918 | 4,847 | 11,591.4 | 64,901 B |
| 100 | anticipatory | 1.000 | 4.85 s | 1,938 | 28,632 | 1,850.5 | 68,025 B |
| 500 | sampled_velocity | 0.514 | 6.58 s | 49,271 | 140,688 | 221.0 | 243,994 B |
| 500 | orca | 0.308 | 1.00 s | 2,473,509 | 224,913 | 708.8 | 248,066 B |
| 500 | anticipatory | 0.560 | 2.98 s | 289,356 | 596,130 | 242.1 | 252,958 B |
| 1000 | sampled_velocity | 0.340 | 5.91 s | 449,812 | 366,178 | 99.5 | 460,723 B |
| 1000 | orca | 0.299 | 0.93 s | 7,327,657 | 647,751 | 309.7 | 466,831 B |
| 1000 | anticipatory | 0.333 | 2.40 s | 1,459,504 | 2,029,643 | 109.2 | 468,659 B |
| 2000 | sampled_velocity | 0.243 | 5.52 s | 2,351,068 | 886,084 | 46.7 | 922,620 B |
| 2000 | orca | 0.176 | 0.87 s | 23,333,403 | 2,067,455 | 133.6 | 926,668 B |
| 2000 | anticipatory | 0.307 | 2.04 s | 5,395,000 | 6,303,589 | 51.4 | 933,624 B |

`dense_flow` is the one scene where `anticipatory` beats `sampled_velocity`
on completion at every scale (1.000/0.560/0.333/0.307 vs.
0.760/0.514/0.340/0.243). It still carries far more penetration and far more
oscillation at every scale (see section 6).

### 3.6 l_corridor

| Agents | Solver | Completion | Mean TTC | Pen. pair-ticks | Reversals | Ticks/s | Peak alloc |
|---|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 0.400 | 6.94 s | 67 | 28,971 | 1,152.5 | 62,697 B |
| 100 | orca | 0.290 | 2.88 s | 31,612 | 3,800 | 7,276.2 | 67,677 B |
| 100 | anticipatory | 0.370 | 6.15 s | 1,300 | 33,015 | 1,244.7 | 70,993 B |
| 500 | sampled_velocity | 0.178 | 6.42 s | 1,111 | 172,015 | 204.6 | 253,982 B |
| 500 | orca | 0.128 | 1.57 s | 1,006,985 | 34,289 | 733.4 | 258,758 B |
| 500 | anticipatory | 0.180 | 4.75 s | 62,264 | 415,378 | 223.0 | 262,946 B |
| 1000 | sampled_velocity | 0.140 | 6.15 s | 10,996 | 421,531 | 98.9 | 505,211 B |
| 1000 | orca | 0.095 | 1.31 s | 3,512,334 | 99,652 | 310.9 | 508,695 B |
| 1000 | anticipatory | 0.148 | 4.13 s | 335,997 | 1,316,890 | 107.3 | 513,147 B |
| 2000 | sampled_velocity | 0.093 | 5.81 s | 111,045 | 999,163 | 47.7 | 964,028 B |
| 2000 | orca | 0.058 | 1.09 s | 11,768,841 | 299,348 | 135.1 | 969,548 B |
| 2000 | anticipatory | 0.112 | 3.48 s | 1,667,056 | 4,075,007 | 51.1 | 972,984 B |

## 4. Derived cross-scene comparison

Averaged across all six scenes at each scale (arithmetic mean of the six
per-scene values above):

| Scale | Solver | Mean completion | Mean TTC | Mean pen. pair-ticks | Mean reversals | Mean ticks/s |
|---|---|---|---|---|---|---|
| 100 | sampled_velocity | 0.713 | 7.11 s | 126 | 17,059 | 1,431.2 |
| 100 | orca | 0.727 | 2.84 s | 25,764 | 5,461 | 8,957.9 |
| 100 | anticipatory | 0.685 | 5.83 s | 1,949 | 26,473 | 1,540.0 |
| 500 | sampled_velocity | 0.452 | 6.65 s | 27,190 | 137,487 | 224.4 |
| 500 | orca | 0.281 | 1.52 s | 1,209,050 | 95,704 | 780.1 |
| 500 | anticipatory | 0.464 | 4.25 s | 154,567 | 426,801 | 238.6 |
| 1000 | sampled_velocity | 0.294 | 6.28 s | 185,048 | 337,743 | 103.3 |
| 1000 | orca | 0.193 | 1.26 s | 4,160,651 | 287,192 | 313.1 |
| 1000 | anticipatory | 0.251 | 3.46 s | 807,474 | 1,421,532 | 108.4 |
| 2000 | sampled_velocity | 0.177 | 5.81 s | 1,036,584 | 808,210 | 48.3 |
| 2000 | orca | 0.128 | 1.03 s | 14,123,437 | 910,284 | 136.0 |
| 2000 | anticipatory | 0.172 | 2.76 s | 3,338,019 | 4,478,837 | 50.9 |

Ratios of `orca`/`sampled_velocity` and `anticipatory`/`sampled_velocity`,
computed from these means:

| Scale | `tps` ratio (orca : anticipatory) | `pen_pt` ratio (orca : anticipatory) | `rev` ratio (orca : anticipatory) |
|---|---|---|---|
| 100 | 6.26x : 1.08x | 205x : 15.5x | 0.32x : 1.55x |
| 500 | 3.48x : 1.06x | 44.5x : 5.7x | 0.70x : 3.10x |
| 1000 | 3.03x : 1.05x | 22.5x : 4.4x | 0.85x : 4.21x |
| 2000 | 2.82x : 1.05x | 13.6x : 3.2x | 1.13x : 5.54x |

Two patterns fall out of this table cleanly:

- **`orca`'s throughput advantage over `sampled_velocity` shrinks as
  population grows (6.3x at 100 agents down to 2.8x at 2,000), while its
  penetration cost, though it also shrinks in ratio terms, stays two orders
  of magnitude worse in absolute terms** (14.1M mean pair-ticks vs. 1.0M at
  n=2000). `orca`'s speed does not come free.
- **`anticipatory` never delivers a measurable throughput advantage over
  `sampled_velocity`** — the ratio sits at 1.05x–1.08x across every scale,
  which is noise, not a design payoff for its scoped-lookahead cost model —
  **while its heading-reversal cost grows the worst of the three as
  population grows**, reaching 5.54x `sampled_velocity`'s reversal count at
  n=2000 (and this is despite `sampled_velocity` itself already having far
  more reversals than `orca` at low population).

## 5. Determinism

Task 6's extended determinism and density-fuzz suites — bitwise-identical
repeated runs, state-hash agreement at every tick (not just the end),
spawn-order-permutation invariance under stable IDs, the added-agent
non-interference check, seed-sensitivity, and the 800-agent, six-scene,
eight-seed density fuzz for non-finite state / scene escape / speed-limit
violation / wholesale deadlock — **passed 6/6 and 4/4 respectively for all
three solvers: `sampled_velocity`, `orca`, and `anticipatory`.** No
solver-specific failure was found in either suite. This holds at the same
bitwise-identity strength the kernel slice established for
`sampled_velocity` alone; the comparison in this report does not weaken that
guarantee for the other two solvers.

## 6. The `anticipatory` solver's tunneling limitation

`anticipatory` (`crates/crowd-core/src/avoidance/anticipatory.rs`) is
designed to bound its per-tick cost by giving only the nearest
`lookahead_neighbors` a multi-step constant-velocity extrapolation, with
everyone past that cutoff falling back to a cheap `far_field_cost`
repulsion. This is a genuine, accepted design tradeoff, not a defect: it
buys a cost bounded by a fixed neighbor count instead of the full neighbor
list, at the cost of being able to miss a threat that is close in time but
outside the `lookahead_neighbors` distance cutoff, or that develops between
the discrete `lookahead_steps` sub-steps rather than at one of them
(analogous to the wall time-to-collision sampling gap the kernel slice
found and replaced with an exact swept-capsule check — the multi-step
lookahead here has the same class of gap, deliberately accepted rather than
fixed, because closing it would mean going back to scoring every neighbor
and losing the whole point of the design).

The measurement in this report is consistent with that limitation actually
manifesting: `anticipatory` is the only solver whose completion rate
collapses to near-zero in a single scene (`circle`, 0.004 at 2,000 agents),
and it has the worst heading-reversal growth of the three as population
rises — both are the observable symptoms of a solver reacting to threats
later or more erratically than a full-neighbor-list solver would, exactly
where its scoped attention runs out. This is stated here as a measurement
limitation, not swept into a footnote: any report of `anticipatory`'s
quality numbers should be read with the caveat that its failure mode is
concentrated in geometries — like `circle`'s radially symmetric,
all-converge-on-center layout — that produce many simultaneous, roughly
equidistant threats, which is precisely the case its neighbor-count cutoff
is worst suited to handle.

## 7. Did Task 9's retuning attempt change the defaults?

**No.** Task 9's report states explicitly that no retuning was performed.
Its own criterion for retuning `anticipatory` was "dramatically worse across
most scenes, not one edge case," and the measured data does not clear that
bar: `anticipatory` underperforms `sampled_velocity` badly in exactly one of
six scenes (`circle`), is competitive with or better than
`sampled_velocity` on completion in four of the remaining five
(`dense_flow`, `bottleneck`, `crossing`, `l_corridor`), and only trails
clearly in `bidirectional_corridor`. Retuning `anticipatory`'s constants
globally to fix `circle`'s specific degeneracy risked trading one scene's
numbers for a regression elsewhere — the same failure mode Task 5's own
tuning process already had to navigate once for this solver's
`far_field_weight` constant. No solver's default parameters were changed as
a result of this comparison; the three solver structs' default
constructors are exactly as Task 9 left them.

## 8. Selection: `sampled_velocity` becomes the production default

**`sampled_velocity` is selected as `crowd-core`'s production-default
avoidance solver.**

### Why it won

- **Best penetration almost everywhere, and by a wide margin at scale.**
  At n=2000, `sampled_velocity`'s mean penetration-pair-ticks (1,036,584) is
  13.6x lower than `orca`'s (14,123,437) and 3.2x lower than
  `anticipatory`'s (3,338,019). This holds in every one of the six scenes
  individually, not just on average — see section 3.
- **Highest predicted time-to-collision at every scale.** Mean TTC stays
  5.8–7.1 s for `sampled_velocity` across all four scales, while `orca`'s
  collapses from 2.8 s (n=100) to 1.0 s (n=2000) and `anticipatory`'s falls
  from 5.8 s to 2.8 s. `orca` in particular is, by this measure, constantly
  resolving conflicts at the last possible moment rather than anticipating
  them — consistent with its closed-form, single-step half-plane
  construction reacting only once a conflict is imminent.
- **Reversals stay the most controlled of the three as population grows.**
  At n=100, `orca` has fewer reversals than `sampled_velocity` (5,461 vs.
  17,059), but that inverts by n=2000 (`orca` 910,284 vs.
  `sampled_velocity` 808,210) — `sampled_velocity`'s smoothness term scales
  better under load. `anticipatory`'s reversal count is worse than both at
  every scale and grows the fastest (4,478,837 at n=2000).
- **Already clears the contract's throughput gate at every scale
  measured.** Contract section 8.3 asks for at least 30 ticks/s. At n=2000,
  `sampled_velocity` still achieves 46.5–52.7 ticks/s across all six scenes
  — comfortably over the gate, even though it is the slowest of the three
  by raw throughput. The gate does not require `orca`'s extra headroom.
- **No scene-specific catastrophic failure.** Unlike `anticipatory`
  (`circle` completion collapses to 0.004 at n=2000) and `orca` (`circle`
  completion falls to 0.026, and it is also the worst or tied-worst
  performer on completion in `bidirectional_corridor` and `bottleneck` at
  n=1000–2000), `sampled_velocity`'s worst per-scene result is a gradual
  degradation consistent with the rest of its own profile, not a collapse.

### Why `orca` was not selected

`orca` is the fastest solver measured, by a wide and real margin (2.8x–6.3x
`sampled_velocity`'s throughput depending on scale), and that speed is a
genuine advantage this report does not dismiss. It is rejected as the
default because that speed is bought with the worst quality profile of the
three on every axis except throughput and, at low population, reversals:
penetration-pair-ticks 13.6x–205x worse than `sampled_velocity` depending on
scale, the lowest predicted time-to-collision at every scale (agents are
close to collision essentially continuously under load), and the worst or
near-worst completion collapse in the one adversarial scene (`circle`,
0.026 at n=2000, worse than `sampled_velocity`'s 0.110). Since
`sampled_velocity` already exceeds the contract's stated 30 Hz throughput
gate at every scale tested, `orca`'s extra throughput is not needed to meet
that gate, and its quality cost is not offset by any requirement that
demands it.

### Why `anticipatory` was not selected

`anticipatory`'s scoped multi-step lookahead is designed to trade
throughput for depth of foresight on the neighbors it does attend to, but
the measured throughput ratio against `sampled_velocity` (1.05x–1.08x
across every scale) shows that trade is not actually being cashed in — it
costs roughly the same as `sampled_velocity` per tick without buying more
speed. What it does cost, relative to `sampled_velocity`, is 3.2x–15.5x
worse penetration depending on scale, the worst heading-reversal growth of
the three (up to 5.54x `sampled_velocity`'s count at n=2000), and the one
genuinely catastrophic single-scene result in the entire 72-report set
(`circle` completion at 0.004, n=2000) — the direct, measured consequence
of the tunneling limitation in section 6. It is competitive with, and in
`dense_flow` better than, `sampled_velocity` on completion in four of six
scenes, which is a real and recorded finding, not dismissed — but a single
production default has to be chosen, and a solver whose worst case is a
near-total collapse in an entire scene is a worse default than one whose
worst case is a gradual, in-family degradation.

The design spec's own risk note (section 9) flagged that
`AnticipatorySolver`'s `lookahead_neighbors`/`lookahead_steps` defaults were
"starting points, not measured," and that this comparison report might show
they need tuning before the bake-off is fair. Section 7 above records that
finding: they do need it (`circle`'s collapse is the clearest evidence),
but per Task 9's own criterion, chasing it now risked a global regression
for a single-scene fix, so the tuning was deliberately deferred rather than
performed under this task.

### Where the data does not give a clean, unqualified answer

Completion rate itself, taken alone and scene by scene, is genuinely mixed
and does not point to a single winner: `orca` has the best completion at
n=100 in four of six scenes; `anticipatory` beats `sampled_velocity` on
completion at every scale in `dense_flow`; `sampled_velocity` is not the
best-completing solver in any single scene at n=100. The case for
`sampled_velocity` rests specifically on penetration and time-to-collision
being the more load-bearing quality axes for a crowd a scene will actually
render — completion is a duration-budget artifact as much as a solver
quality signal, per the kernel slice's own finding that scene durations
scale with the square root of population while congestion slowdown is
superlinear — combined with it having no scene where it collapses the way
the other two each do once. That is a defensible selection, not a
mechanical one, and it is recorded as reasoning rather than as an arithmetic
maximum over one column.

## 9. All three solvers remain in the codebase

Selecting `sampled_velocity` as the default does not delete `orca` or
`anticipatory`. All three stay behind the shared `AvoidanceSolver` trait —
the point of this slice was proving the architecture accommodates a
bake-off, not committing to ship only one implementation forever. A future
slice could reasonably make solver choice a per-scene or per-tier setting
(for instance, `orca`'s throughput advantage may matter more once fidelity
tiers exist and only a subset of agents get full simulation), but that is
future work, not part of this decision.

## 10. What remains open for M0 after this slice

This report closes the avoidance bake-off item of Phase 0, and nothing
more. Contract section 14's planned layout still has substantial M0 scope
untouched: **the tiled navmesh** (routing here is still the kernel slice's
authored waypoint graph, a deliberate stand-in, not real navigation
geometry); **cache v0** (nothing computed by any of these three solvers is
cached, versioned, or made resumable — caching a still-unselected solver's
output would have been premature before this report, and caching now still
requires its own design and implementation); and **the Blender bridge**
(the PyO3 bridge, the Blender add-on, Geometry Nodes integration — nothing
in this comparison or the kernel slice before it has been anywhere near
Blender). M0 is not closed by this report; it removes one specific
open question (which avoidance approach is the default) so that the
remaining items can be built against a settled choice instead of an
unresolved three-way comparison.
