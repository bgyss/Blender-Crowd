# M5 100K scale gate — second attempt, and a defect in the gate itself

Date: 2026-08-15
Milestone: [M5 — Scale, GPU tiers, and procedural rendering](../milestones/M5-scale-rendering.md)
Runbook: [10K and 100K scale-gate runbook](../runbooks/m5-scale-gates.md)
Status: **the throughput failure is fixed and confirmed; two of the four
quality failures were the gate's fault and are corrected; the proposed solver
fix for the rest is refuted. No 100K run of the corrected code exists yet.**

Supersedes the diagnosis in
[2026-08-14-m5-100k-failed.md](2026-08-14-m5-100k-failed.md), whose
specification finding was right and whose solver finding was wrong.

## The run

The optimised code from the first attempt, rerun at 100,000 agents.

| Check | First attempt | Second attempt | Limit | |
| --- | ---: | ---: | ---: | --- |
| `ticks_per_second_achieved` | 5.27 | **13.04** | ≥ 10 | now **pass** |
| S1 `max_penetration_depth_m` | 0.063148 | 0.063148 | ≤ 0.02 | FAIL |
| S1 `stalled_agent_share` | 0.238608 | 0.238608 | ≤ 0.20 | FAIL |
| S2 `max_penetration_depth_m` | 0.135480 | 0.135480 | ≤ 0.05 | FAIL |
| S2 `stalled_agent_share` | 0.259884 | 0.259884 | ≤ 0.20 | FAIL |

Wall time 10,910 s (3h 02m) for 142,302 ticks, against 26,999 s for the same
work in the first attempt — the 3.28x steering speedup landing at 100K, and the
first 100K measurement of it rather than the projection the previous report was
careful to label as such.

The four quality figures are **bit-identical** across the two runs. That is the
expected result, not a coincidence: the optimisations were verified to
reproduce the sequential path bitwise, so the second attempt is the same
simulation measured on a faster machine path. It also means these four numbers
are a deterministic reproduction, not a sampling fluctuation to be re-rolled.

## Two independent problems, which must not be used to excuse each other

### 1. Two of the four gates were not scale-invariant

The threshold file claimed its limits were scale-free because they were rates
per observed agent-tick. Two of them were not rates.

- **`stalled_agent_share`** is `agents_ever_stalled / population`
  (`crates/crowd-core/src/metrics.rs`, `summarize_tiers`). It is a lifetime
  cumulative probability. At a perfectly constant blocking rate per metre it
  still rises toward 1.0 as routes lengthen, and this fixture's routes grow
  with the square root of population.

  The arithmetic is decisive. Fitting `1 - exp(-λL)` to the 10K value of 0.0998
  gives `λL₁ = 0.1051`; at 100K the route is 3.17x longer, predicting 0.283.
  The run measured **0.258** — *better* than a constant blocking rate per
  metre, while failing a 0.20 bar.

- **`max_penetration_depth_m`** is a running maximum over samples. Its expected
  value grows with the number of draws, and 100K observes 5.24e9 agent-ticks
  against 10K's 1.65e8 — 32x the draws.

Neither can be held to a fixed number across scales that change route length
and sample count. Both are now **reported and not gated**, and the gate decides
on rate-shaped replacements instead:

| was gated | replaced by |
| --- | --- |
| `stalled_agent_share` | `stall_episodes_per_agent_km` |
| `max_penetration_depth_m` | `mean_penetration_depth_fraction` |

This is a real correction, not a loosened bar, and it is worth being explicit
about why it is not the latter: the replacements divide out the exposure that
was contaminating the originals, and every genuinely rate-shaped limit in the
file is unchanged. The change is behaviour-neutral — the 10K run under schema
v6 reproduces `final_state_hash 16004330017290778013`, the value the accepted
10K report records — and all six checked-in baselines still report OK.

An intermediate design was discarded during this work and is worth recording.
`deep_penetration_agent_ticks_per_agent_tick`, counting overlaps past 10% of
the pair's combined radius, is rate-shaped and looks like the natural
replacement for peak depth. It measures **exactly zero at every scale and
configuration yet run**. Setting a threshold from it would have been setting a
threshold from an event nobody has observed — the same defect as the metrics it
replaced, in a new form. It is reported, ungated, until a run produces a
non-zero value.

### 2. The proposed solver fix is refuted

The previous report proposed a minimum-progress floor on the density feedback
`1/(1 + 0.18 x crowding)`, reasoning that the term "has no floor, so dense
clusters slow toward a crawl and clear slowly". It expected this to invalidate
all six baselines and require re-validating the M0–M4 evidence resting on them.

**The premise does not hold for this fixture.** Three measurements, all from
probes checked in with this change:

**The crowd is sparse at every scale.** `m5_crowding_distribution` counts
neighbours inside the solver's own clearance, excluding arrived agents exactly
as `phases::perceive` does:

| population | max crowding | ≤2 neighbours | ≥5.6 (0.50 floor binds) | ≥10.3 (0.35 floor binds) |
| ---: | ---: | ---: | ---: | ---: |
| 10,000 | 6 | 99.94% | 0.000153% | **0%** |
| 100,000 | **4** | 99.90% | **0%** | **0%** |

100K is *marginally sparser* than 10K. There are no dense clusters. The density
term is also capped from above regardless of crowd density:
`PerceiveConfig::budget` keeps 16 neighbours, so `crowding` saturates there and
the strongest slowdown the term can ever produce is 0.26.

**The floor is therefore a no-op where it would be needed.**
`m5_density_floor_sweep` compares `final_state_hash` directly:

| floor | 10K `final_state_hash` | |
| ---: | --- | --- |
| 0.00 | 16004330017290778013 | control |
| 0.35 | 16004330017290778013 | **identical — never binds** |
| 0.50 | 6294499437395297802 | binds |
| 0.70 | 13392418256402237161 | binds |

**Where it does bind, it makes the thing it targets worse.** A 0.35 floor moves
only the two densest scenes, and on `dense_flow` it cut stall episodes 19%
(21,464 to 17,289) while growing stall agent-ticks 40% (1.82M to 2.55M) —
lengthening the mean episode from 84.8 to **147.6 ticks**. Holding back in
front of a blockage is doing useful work; flooring the speed drives agents into
it and they then sit there.

`min_density_speed_fraction` is kept as a parameter, defaulted to `0.0` (off),
so the sweep can still exercise it. It is not recommended.

An earlier version of the crowding probe reported max crowding of 73 at 10K and
was wrong: it counted agents that had already arrived. Those park on their
destination node and accumulate, and `phases::perceive` drops them precisely so
they do not become a permanent plug. The corrected figure is 6. The wrong
number is recorded here because it briefly looked like evidence *for* the floor.

## A metric defect found while investigating the S1/S2 result (2026-08-17)

Investigating why S1's contact advantage over S2 disappears at scale turned up
a measurement bug that affected every per-tier contact figure in this
milestone.

`perceive_scheduled` recorded a schedule-skipped agent with
`arena.push(slot, &[])` — an empty neighbour list, indistinguishable from
"queried, and genuinely alone". `observe_tick` reads that list for every active
agent every tick. A background agent on a skipped tick therefore registered **no
contact however deeply it was overlapping**, while `agent_ticks` — the
denominator of every contact rate — still counted the tick.

Background-tier contact was undercounted by exactly 2x at the declared 2-tick
cadence. Measured after the fix, the share of ticks on which contact could be
observed is **1.000 for S1 and 0.500 for S2**, at every scale.

The fingerprint that exposed it: at 40,000 agents the same S1-S2 contacts are
seen 65 times from the S1 side, which perceives every tick, and 35 times from
the S2 side.

The fix records in the arena whether a slot was actually queried, and divides
contact rates by observed ticks rather than by all ticks. Stall, oscillation,
and distance counters read world state and stay on the full denominator. The
skip schedule is an ID hash independent of whether anyone is overlapping, so
the observed ticks are an unbiased sample and the ratio estimates the true
rate; measuring contact exactly instead would need a neighbour query every tick
for every agent, costing roughly a third of the perception phase against a
throughput budget that clears its bar by 32%. `max_penetration_depth` still
misses peaks on unobserved ticks, so S2's true peak may exceed its reported
figure — it is ungated for the separate reason that it is an extremum.

Corrected values, all still inside their bars, so no previously-passing check
becomes a failure:

| measure | tier | before | after |
| --- | --- | ---: | ---: |
| contact rate, 10K | S2 | 6.549e-7 | **1.310e-6** |
| contact rate, 40K | S2 | 5.334e-7 | **1.067e-6** |
| mean severity, 10K | S2 | 5.186e-9 | **1.037e-8** |

S1 perceives every tick and is unchanged, so the failing S1 check is untouched
by this fix. That is the correct outcome: it was a correctness repair, not a
route to a passing gate.

### What the corrected comparison shows

Contacts last **exactly 1.0 ticks** at every scale and tier — instantaneous
grazes at closest approach, not sustained overlap. The solver never lets an
overlap persist, which peak depth alone never showed. It also means contact
agent-ticks are independent events, so rates built on them are sound.

With both tiers honestly measured:

- **S1 agents never contact each other.** Zero S1-S1 contacts at 10K, 20K, 40K
  and 100K, against a tier-blind expectation of ~10% of S1's contacts —
  `P(0 | 6.5) ~ 0.0015` at 40K alone. Per-tick steering works when both parties
  have it.
- **Every one of S1's contacts is with an S2 partner.** The foreground tier
  pays for the background tier's stale steering.
- **The convergence is progressive**, saturating by ~40K. Corrected S1/S2 rate
  ratio: 0.048 at 10K, 0.449 at 40K, 0.445 at 100K.

### The S1 severity threshold is unsound, independently of the 100K run

`max_mean_penetration_depth_fraction` for S1 is set to 1e-9, derived from a 10K
measurement of **one contact**, with the 1K scale recording **zero**. That is
not a calibration. S1 breaches it at **40K** too — 3.412e-9 on 65 contacts —
so this is not a 100K-specific regression.

The 100x split between the tiers' severity bars is not supported either. At 40K,
where both tiers are properly sampled, S1 and S2 differ by 2.8x (3.412e-9
against 9.620e-9), not 100x. The apparent 211x gap at 10K was a single
observation on one side and a halved detector on the other.

Recalibrating would convert the failing check to a pass, so it is deliberately
**not** done here.

### Still unexplained

S1's contact rate rises 13x from 10K to 100K (6.298e-8 to 8.272e-7) on
unbiased measurement, entirely from S2 partners, with density constant and
S1-S1 contact at zero throughout. No mechanism is established for this.

## What is left, and what it probably is

Stalling at 100K is genuinely higher after correct normalisation, and no solver
change here addresses it. Measured `stall_episodes_per_agent_km`:

| tier | 1K | 10K | 100K (estimated) |
| --- | ---: | ---: | ---: |
| S1 | 0.2735 | 0.1603 | — |
| S2 | 0.2917 | 0.2202 | — |
| population-wide | 0.290 | 0.214 | ≈0.495 |

Note the metric is **non-monotonic**: 1K is worse than 10K. Measured against
the worse of the two calibration scales rather than against 10K alone, the 100K
figure is 1.70x, not the 2.3x that comparing only to 10K suggests. Any claim
about regression at scale has to name which baseline it is measured from.

The 100K estimate above is derived from the first-attempt run's episode count
and a route length scaled from the 10K measurement; it is **not** a measured
per-tier figure, because that run predates `distance_travelled_m`. It is
labelled an estimate for that reason and must be replaced by a schema-v6 100K
run before it is quoted.

The likely mechanism is fixture geometry rather than solver behaviour. The
scene holds *density* constant with population but not *platoon length*:

| | 10K | 100K | ratio |
| --- | ---: | ---: | ---: |
| lanes | 120 | 380 | 3.17 |
| agents per lane | 83 | 263 | **3.16** |
| lane length | 720 m | 2277 m | 3.16 |
| linear density | 0.116/m | 0.116/m | **1.00** |

Because `lanes = 12 x scale` while `agents = 100 x scale²`, agents-per-lane
grows with the linear scale. A blockage propagates back through the platoon, so
a longer platoon puts more followers into a stall per blocking event. That
predicts stall episodes per agent-km rising roughly with agents-per-lane, which
is the right order for what is measured.

If that is the mechanism, it is a property of how the fixture is built, and
"fixing" it means changing the fixture — which, done in response to a failed
gate, is hard to distinguish from moving the goalposts. That is a contract
decision and is deliberately left open here.

## Thresholds

`benchmarks/thresholds/m5-city-flow.json` now sets the two new limits from the
measured schema-v6 per-tier baselines, by a stated rule rather than by what
admits any particular run:

- `stall_episodes_per_agent_km`: **0.9** for both tiers, 3x the worse
  calibration point (S2's 0.2917/km at 1K). A well-sampled rate — 1,455
  episodes at 10K — so it does not need the large headroom the contact rates
  carry.
- `mean_penetration_depth_fraction`: **1e-9** (S1) and **1e-7** (S2), ~20x the
  worse calibration point. Deliberately loose: the fixture produces 99
  penetration agent-ticks in 1.65e8 at 10K, so no tight severity bar can be
  justified from measurement. It is a blowup detector. The meaningful contact
  gate remains `max_penetration_agent_ticks_per_agent_tick`.

The 10K gate passes under these thresholds.

## What this report does not establish

- **No 100K run of the corrected code exists.** Every 100K figure here comes
  from the schema-v5 run, and the per-tier stall rate is an estimate. The gate
  must be rerun before any 100K claim is made.
- No 100K headline follows from this run. It failed the file it was judged
  against, and two of those four failures were the file's fault rather than the
  run's.
- Blender evidence was deliberately not gathered: it supports a passing gate.
- The accepted [10K report](2026-08-14-m5-10k.md) is unchanged, and is
  reproduced bitwise by this code.
