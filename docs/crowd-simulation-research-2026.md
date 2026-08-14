# Crowd simulation research synthesis

Status: research note supporting the Blender Crowd 1.0 contract  
Evidence reviewed: four supplied papers plus selected primary research
available through August 4, 2026

## Decision summary

The research strengthens the existing architecture rather than overturning it.
Blender Crowd should keep the deterministic 1,000-agent CPU vertical slice as
the 1.0 gate and make one research-backed capability an **MVP stretch goal**:
attention-aware social locomotion whose steering, group behavior, gaze, and
upper-body presentation share the same deterministic state.

Four other ideas are valuable only behind measured gates:

1. Add a time-to-collision-inspired candidate to the Phase 0 avoidance
   comparison, without assuming a global implicit solver will win.
2. Use observed trajectories to evaluate and fit explainable crowd profiles
   offline; do not make a learned controller authoritative in 1.0.
3. Develop a GPU continuous-flow simulation tier only for the 10K/100K
   background roadmap, through the existing tier and cache boundaries.
4. Explore camera- or gaze-prioritized animation updates after the R2
   presentation contract exists; never couple this optimization to simulation
   truth.

Per-agent language or vision-language models remain out of scope. Recent work
makes that direction interesting for experiments, but its runtime cost,
reproducibility, explainability, and packaging requirements conflict with the
1.0 contract.

Reactive neural **animation** for a small number of promoted interaction groups
is a distinct post-1.0 opportunity. It may generate a validated animation layer
without replacing the deterministic brain, navigation, contact outcome, or base
cache. ARDY and paired-interaction alternatives are assessed in the dedicated
[reactive neural interaction animation research track](reactive-neural-interaction-animation-2026.md).

## Evidence assessment

| Evidence | Strongest relevant result | Product implication | Important caveat |
|---|---|---|---|
| Lombardo, Gadia, and Maggiorini, 2024 | A GPU hybrid of potential fields, continuous density/velocity fields, pressure correction, and pairwise repair reported 60 FPS for 20,000 agents on an M1 Pro test machine | Good reference architecture for an S2/S3 background tier | Primarily homogeneous flow; grid approximations produced congestion, clearance loss, and dead-end errors; Unity/HLSL timings do not transfer directly to Blender |
| Itatani and Pelechano, 2024 | Coupling attention, group-aware avoidance, gaze, and animation improved perceived movement/social behavior over basic or randomly animated baselines | Best candidate for an artist-visible MVP stretch | The online study had 12 participants, animation quality did not improve significantly, and several field-of-view values were empirical choices |
| Charalambous et al., 2023 | Example-guided deep RL recovered groups, wandering, stops, and other trajectory details that goal-and-collision rewards omit | Use real data to define plausibility metrics and profile presets | Training and nearest-neighbor reward construction add data, robustness, determinism, and explainability risks |
| Karamouzas et al., 2017 | A smoothed time-to-collision potential and implicit optimization produced smooth collision-free motion over a broad time-step range | Test anticipatory objectives in the avoidance proving ground | The global L-BFGS solve was 2-10 times slower than comparison solvers and conflicts with independent tier scheduling |
| CEDRL, 2025 | Multiple datasets and a controllable complexity signal expanded example-driven crowd behavior beyond one policy/data source | Supports a future controllable profile layer | Still a learned runtime policy with training and validation obligations |
| MPACT, 2025 | Unlabelled trajectories were mapped to explainable, controllable behavior profiles for analysis and authoring | Strong basis for offline “match this reference crowd” tools | Its learned mapping is tied to a modified underlying simulator and fixed spatial abstractions |
| Foveated Animations, 2025 | User studies found that peripheral animation updates could be reduced sharply, with a best-case 99.3% operation reduction | Promising R1/R2 animation scheduling policy | The result is prototype- and viewing-condition-specific, not a Blender production benchmark |
| CrowdVLA, 2026 preprint | A vision-language-action agent uses semantic observations, language instructions, and motion skills | Confirms the value of semantic actions and consequence-aware authoring research | Version 1 is a preprint and its per-agent model is contrary to the 1.0 runtime contract |

Performance numbers above describe the cited authors' configurations, not
Blender Crowd targets.

## Findings from the supplied papers

### GPU continuous-flow simulation belongs in a background tier

*Massive Crowd Simulation With Parallel Computing on GPU* combines a discrete
agent representation with continuous density and average-velocity fields. Its
pipeline uses potential fields for global motion, a predicted future-density
field, pressure projection for dense regions, and a pairwise pass to repair
grid-level approximations. It also demonstrates vertex-animation textures for
large animated crowds. The paper reports roughly linear scaling in its tests,
including 60 FPS at 20,000 agents on its M1 Pro system and about 22.8 FPS for a
20,000-agent animated Manhattan example.

This is not a reason to move the 1K authoritative simulator to the GPU. The
same paper reports limitations that matter to Blender Crowd: homogeneous or
goal-grouped behavior, crowd congestion where opposing flows meet, obstacle
clearance lost to spatial approximation, and potential-field errors around
complex obstacles and dead ends. Its final all-pairs repair also deserves a
fresh complexity and spatial-index review rather than direct adoption.

The reusable idea is the representation boundary: splat S2/S3 agents into
density and velocity fields, solve shared flow cheaply, then discretize back to
stable agents and write the normal cache. That fits the current simulation-tier
architecture and keeps Blender, Geometry Nodes, and render extraction isolated
from the solver backend.

Source: [Lombardo, Gadia, and Maggiorini, 2024](https://doi.org/10.1109/ACCESS.2024.3501093).

### Attention should connect behavior to visible character intent

*Social Crowd Simulation: Improving Realism with Social Rules and Gaze
Behavior* separates walking direction from gaze direction. Attention changes
the anticipatory field of view; time to collision selects threats; members can
share anticipatory perception; an incoming pedestrian treats a nearby social
group as a larger obstacle; and agents use gaze to make avoidance-side
coordination visible. Upper-body states such as conversation or smartphone use
both express and influence attention rather than being random decoration.

This fills a real gap in the current product contract. Blender Crowd already
has perception, groups, inspectable decisions, animation state, and a stable
cache, but it does not require those layers to agree about what an agent is
attending to. A small, typed attention state can bridge them without adding a
general behavior language.

The paper should not be copied literally. Production safety should retain a
gaze-independent hard separation envelope: distraction may shorten
anticipation or create a late reaction, but it must not disable the final
non-penetration fallback. Field-of-view angles should be authorable presets and
validated, not treated as universal constants. The study supports the direction
but is not a final product validation: it used 12 online participants, and
animation realism itself showed no significant improvement.

Source: [Itatani and Pelechano, MIG 2024](https://doi.org/10.1145/3677388.3696337).
An expanded 2025 journal version also reports a VR evaluation, but it is an
extension of the same research rather than independent replication:
[Computers & Graphics 131](https://doi.org/10.1016/j.cag.2025.104286).

### Learned behavior is most useful first as an offline critic and authoring aid

*GREIL-Crowds* represents each pedestrian using its speed, desired-velocity
error, and temporally smoothed relative observations of nearby agents. It
learns a novelty-based reward from real trajectories, then trains a
Double-DQN policy over sampled accelerations. Its most useful product lesson is
that goal completion and collision counts do not measure ambient-crowd
plausibility: grouping, hesitation, wandering, sudden stops, and persistent
action choices also shape the result.

The direct runtime design is a poor MVP fit. A trained policy introduces model
and dataset provenance, out-of-distribution behavior, platform reproducibility,
and weaker per-decision explanations. The paper also identifies sensitivity to
limited data and scaling costs in its nearest-neighbor novelty evaluation.

Blender Crowd can capture most of the near-term value without that risk:

- import normalized 2D trajectory samples in an offline tool;
- compare simulated and reference distributions for speed versus density,
  nearest-neighbor distance, path deviation, stops, and group persistence;
- fit or recommend explainable population and behavior-graph parameters;
- save the source hash, fitted values, metrics, and confidence with the preset;
- keep the compiled graph and deterministic solver authoritative at bake time.

Source: [Charalambous et al., 2023](https://doi.org/10.1145/3592459).

### Time-to-collision behavior deserves a benchmark, not an early commitment

*Implicit Crowds* reformulates anticipatory interaction as a smoothed
time-to-collision potential and solves all agents' next velocities through
implicit optimization. It is compelling where ORCA becomes visibly
conservative: agents negotiate over several steps instead of making an abrupt
one-step correction. The paper also reports collision-free trajectories, close
agreement with measured crowd statistics, and stability at relatively large
time steps.

The complete solver is not an obvious MVP default. Its global synchronized
optimization was slower than ORCA and the explicit PowerLaw comparison, makes
variable update rates difficult, and cannot implicitly express every
asymmetric interaction such as following or fleeing. The right experiment is
to add a time-to-collision-inspired candidate to Phase 0 and measure whether a
soft anticipatory objective, dense-region repair, or full implicit solve earns
its cost.

Source: [Karamouzas et al., 2017](https://doi.org/10.1145/3072959.3073705).

## What newer work changes

Recent results reinforce three trends.

First, learned crowd behavior is becoming more controllable. CEDRL combines
multiple datasets with a behavior-complexity control, while MPACT maps
unlabelled trajectories into explainable behavior profiles that users can
inspect and blend. This makes a post-1.0 trajectory-to-preset authoring tool
more credible than an opaque “realism” toggle. It does not remove the need for
licensed data, held-out scenarios, failure reporting, or deterministic bake
semantics. Sources: [CEDRL](https://doi.org/10.1111/cgf.70015) and
[MPACT](https://doi.org/10.1111/cgf.70156).

Second, presentation scheduling can use perception as well as geometric LOD.
The 2025 foveated-animation study reduced animation work based on visual focus
without detected loss under its tested conditions. Blender Crowd can eventually
generalize this into a deterministic camera/focus importance schedule for
armature or deformation updates while continuing to sample every cached root
trajectory. This is a presentation optimization, not permission to freeze
simulation. Source: [Stancu, Weiss, and dos Anjos,
2025](https://doi.org/10.1145/3728306).

Third, the 2026 CrowdVLA preprint pushes semantic, consequence-aware decisions
into a vision-language-action agent. Its “motion skill” boundary is compatible
with Blender Crowd's typed actions, but running such a model for each agent is
not. The appropriate future experiment is offline authoring or sparse hero-agent
planning that compiles to a reviewable graph or action schedule. Runtime
promotion would require peer-reviewed evidence, a fixed model artifact,
latency/cost and determinism measurements, failure traces, and a fallback that
still produces a valid bake. Source: [CrowdVLA version
1](https://arxiv.org/abs/2604.05525).

## Recommended product experiments

### A. MVP stretch: deterministic social attention and gaze

Add this only after the base 1K vertical slice is healthy.

The smallest useful implementation has:

- typed attention states such as `travel`, `social`, `distraction`,
  `avoidance`, and `recovery`;
- a stable-ID gaze target or semantic target plus bounded head-look output;
- attention-dependent anticipatory range and time horizon;
- a group-aware collider and shared *anticipatory* observations;
- stable-ID tie-breaking for avoidance-side coordination;
- a gaze-independent hard separation fallback;
- optional versioned cache channels for attention, gaze target, and head look;
- an overlay showing attention cone, selected threat, group extent, and the
  reason for a coordination change.

The evaluation scene should contain individuals, a conversational pair, a
three-person group, opposing traffic, and a distracted walker. Compare the
stretch against the base solver using penetration, near misses, avoidance-side
reversals, group intrusions, group splits, travel time, stalls, and deterministic
rebake. Any public realism claim also needs a recorded blinded preference test
with the protocol and sample reported in advance.

### B. Phase 0: anticipatory avoidance candidate

Add one time-to-collision candidate to the existing ORCA-versus-sampled solver
comparison. Record minimum predicted time to collision, acceleration/jerk,
oscillation, penetration, stalls, throughput, wall time, and memory in crossing,
antipodal-circle, three-agent, bottleneck, and dense bidirectional scenes. A
full implicit solver remains an experiment unless it meets the 1K/30 Hz budget
and the tier scheduler can preserve its guarantees.

### C. Post-1.0: trajectory-informed profiles

Build an offline, optional analysis path before considering a learned runtime
policy. A useful first version accepts documented trajectories, computes a
standard metric report, searches bounded explainable parameters, and writes a
reviewable preset with provenance and uncertainty. It must never silently
train on or redistribute user footage.

### D. 10K/100K: GPU shared-flow tier

Prototype S2/S3 density, velocity, future-density, and discomfort fields behind
the Rust core facade. Preserve stable agent IDs and write the same cache schema
as the CPU backend. Benchmark complex navigation, heterogeneous destinations,
backend determinism, transfer/readback cost, and quality together. Do not claim
the supplied paper's Unity/HLSL numbers as Blender performance.

### E. Post-R2: view-prioritized animation updates

Use camera distance, projected size, focus region, motion salience, and artist
pins to schedule armature/deformation work. Keep root transforms and cache
sampling current. Validate flat-screen viewport, final render, motion blur,
reflections, shadows, multiple cameras, and agents moving into focus before
shipping any skipped-update policy.

## Rejected shortcuts

- Do not replace tiled navmesh corridors with a single global potential field
  for the 1K heterogeneous MVP.
- Do not treat GPU agent count or frames per second as a quality result.
- Do not let visual gaze disable the hard safety envelope.
- Do not call random upper-body animation “social behavior.”
- Do not ship a learned policy without dataset/model provenance and held-out
  scenario tests.
- Do not add a per-agent LLM or VLA loop to 1.0.
- Do not make foveated animation state part of authoritative simulation or the
  portable cache.

## Source list

### Supplied papers

- Vincenzo Lombardo, Davide Gadia, and Dario Maggiorini. “Massive Crowd
  Simulation With Parallel Computing on GPU.” *IEEE Access* 12 (2024).
  [DOI](https://doi.org/10.1109/ACCESS.2024.3501093)
- Reiya Itatani and Nuria Pelechano. “Social Crowd Simulation: Improving
  Realism with Social Rules and Gaze Behavior.” *MIG 2024*.
  [DOI](https://doi.org/10.1145/3677388.3696337)
- Panayiotis Charalambous et al. “GREIL-Crowds: Crowd Simulation with Deep
  Reinforcement Learning and Examples.” *ACM Transactions on Graphics* 42.4
  (2023). [DOI](https://doi.org/10.1145/3592459)
- Ioannis Karamouzas et al. “Implicit Crowds: Optimization Integrator for
  Robust Crowd Simulation.” *ACM Transactions on Graphics* 36.4 (2017).
  [DOI](https://doi.org/10.1145/3072959.3073705)

### Recent primary research

- Reiya Itatani and Nuria Pelechano. “Social crowd simulation: Improving
  realism with social rules and gaze behavior.” *Computers & Graphics* 131
  (2025). [DOI](https://doi.org/10.1016/j.cag.2025.104286)
- Andreas Panayiotou, Andreas Aristidou, and Panayiotis Charalambous. “CEDRL:
  Simulating Diverse Crowds with Example-Driven Deep Reinforcement Learning.”
  *Computer Graphics Forum* 44.2 (2025).
  [DOI](https://doi.org/10.1111/cgf.70015)
- Marilena Lemonari et al. “MPACT: Mesoscopic Profiling and Abstraction of
  Crowd Trajectories.” *Computer Graphics Forum* 44.6 (2025).
  [DOI](https://doi.org/10.1111/cgf.70156)
- Florin-Vladimir Stancu, Tomer Weiss, and Rafael Kuffner dos Anjos. “Foveated
  Animations for Efficient Crowd Simulation.” *Proceedings of the ACM on
  Computer Graphics and Interactive Techniques* 8.1 (2025).
  [DOI](https://doi.org/10.1145/3728306)
- Juyeong Hwang et al. “CrowdVLA: Embodied Vision-Language-Action Agents for
  Context-Aware Crowd Simulation.” arXiv version 1 (April 2026).
  [Preprint](https://arxiv.org/abs/2604.05525)
