//! Quality and performance metrics, contract section 12.3.
//!
//! Accumulated during simulation and summarised at the end. `MetricsSummary`
//! field names are the keys baselines compare on, so treat them as a schema:
//! renaming one silently invalidates every checked-in baseline.

use serde::{Deserialize, Serialize};

use crate::arena::NeighborArena;
use crate::clock::Clock;
use crate::fidelity::SimulationTier;
use crate::geometry::Segment;
use crate::phases::animate::AnimateReport;
use crate::phases::integrate::IntegrateReport;
use crate::phases::steer::SteerReport;
use crate::scene::CompiledScene;
use crate::units::{wrap_angle, Vec2};
use crate::world::{SolverStatus, World};

/// Which pipeline phase a timing sample belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Inputs,
    Spawn,
    Index,
    Perceive,
    Plan,
    Decide,
    Steer,
    Integrate,
    Animate,
    Metrics,
}

impl Phase {
    pub const ALL: [Phase; 10] = [
        Phase::Inputs,
        Phase::Spawn,
        Phase::Index,
        Phase::Perceive,
        Phase::Plan,
        Phase::Decide,
        Phase::Steer,
        Phase::Integrate,
        Phase::Animate,
        Phase::Metrics,
    ];

    const fn index(self) -> usize {
        match self {
            Phase::Inputs => 0,
            Phase::Spawn => 1,
            Phase::Index => 2,
            Phase::Perceive => 3,
            Phase::Plan => 4,
            Phase::Decide => 5,
            Phase::Steer => 6,
            Phase::Integrate => 7,
            Phase::Animate => 8,
            Phase::Metrics => 9,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Phase::Inputs => "inputs",
            Phase::Spawn => "spawn",
            Phase::Index => "index",
            Phase::Perceive => "perceive",
            Phase::Plan => "plan",
            Phase::Decide => "decide",
            Phase::Steer => "steer",
            Phase::Integrate => "integrate",
            Phase::Animate => "animate",
            Phase::Metrics => "metrics",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MetricsConfig {
    /// Consecutive braking ticks before an agent counts as stalled.
    pub stall_ticks_threshold: u16,
    /// Heading change in one tick that counts as abrupt.
    pub abrupt_turn_radians: f32,
    /// Optional line agents are counted crossing, for throughput.
    pub throughput_gate: Option<Segment>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            stall_ticks_threshold: 15,
            abrupt_turn_radians: 0.9,
            throughput_gate: None,
        }
    }
}

/// `SimulationTier` has four variants, and per-tier accumulators are indexed
/// by `tier as usize`. A fifth tier would have to be added here deliberately.
const TIER_COUNT: usize = 4;

const TIER_NAMES: [&str; TIER_COUNT] = ["S0", "S1", "S2", "S3"];

// Indexing by `tier as usize` is only sound while the discriminants stay
// contiguous from zero and end at S3. Adding a tier fails the build here
// rather than silently panicking mid-run.
const _: () = assert!(SimulationTier::S3 as usize + 1 == TIER_COUNT);

/// Per-tier accumulation, so the M5 contract's requirement that quality stay
/// "within declared thresholds for the fidelity assigned to each tier" can be
/// adjudicated instead of inferred from a population-wide total.
///
/// Attribution rules, which the thresholds depend on:
///
/// - Every counter is attributed to the agent's tier *at the observed tick*.
///   An agent promoted mid-run therefore contributes its early ticks to the
///   tier that actually produced them.
/// - `penetration_pair_ticks` is counted once per pair from the lower-ID side
///   (as in the population-wide counter) and attributed to that agent's tier.
///   `penetration_agent_ticks` is per-agent and so attributes cleanly to both
///   sides; prefer it when comparing tiers.
/// - `agents_ever_stalled` is attributed to the tier the agent held when it
///   first stalled, so the per-tier counts sum to the population-wide count.
/// - `agent_ticks` counts each observed, still-active agent once per tick. It
///   is the exposure denominator that makes the derived rates comparable
///   across 1K, 10K, and 100K runs of the same fixture.
#[derive(Clone, Debug, Default)]
struct TierAccumulator {
    agent_ticks: u64,
    arrived: u64,
    penetration_pair_ticks: u64,
    penetration_agent_ticks: u64,
    max_penetration_depth: f32,
    agents_ever_stalled: u64,
    stall_episodes: u64,
    stall_agent_ticks: u64,
    heading_reversals: u64,
    abrupt_turns: u64,
    /// Full presentation classifications actually performed, against
    /// `animation_agent_ticks` opportunities to perform one.
    animation_evaluations: u64,
    animation_agent_ticks: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    ticks: u64,

    penetration_pair_ticks: u64,
    max_penetration_depth: f32,
    penetration_agent_ticks: u64,

    min_time_to_collision: f32,
    time_to_collision_sum: f64,
    risk_samples: u64,
    near_miss_agent_ticks: u64,

    wall_corrections: u64,
    nonfinite_corrections: u64,

    stall_episodes: u64,
    agents_ever_stalled: u64,
    stall_agent_ticks: u64,

    heading_reversals: u64,
    abrupt_turns: u64,

    gate_crossings: u64,

    arrived: u64,
    travel_seconds: Vec<f32>,

    /// Per-agent state carried between ticks, indexed by slot.
    previous_heading: Vec<f32>,
    previous_turn_sign: Vec<i8>,
    counted_arrival: Vec<bool>,
    counted_stall: Vec<bool>,
    /// Never reset, so an agent is counted once no matter how many separate
    /// stalls it has.
    ever_stalled: Vec<bool>,
    previous_gate_side: Vec<i8>,

    phase_nanos: [u64; 10],

    tiers: [TierAccumulator; TIER_COUNT],
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            min_time_to_collision: f32::INFINITY,
            ..Default::default()
        }
    }

    pub fn begin_tick(&mut self) {
        self.ticks += 1;
    }

    pub fn record_phase(&mut self, phase: Phase, nanos: u64) {
        self.phase_nanos[phase.index()] += nanos;
    }

    pub fn phase_nanos(&self, phase: Phase) -> u64 {
        self.phase_nanos[phase.index()]
    }

    pub fn record_steer(&mut self, report: &SteerReport) {
        if report.min_time_to_collision < self.min_time_to_collision {
            self.min_time_to_collision = report.min_time_to_collision;
        }
        self.time_to_collision_sum += report.time_to_collision_sum as f64;
        self.risk_samples += report.risk_samples as u64;
        self.near_miss_agent_ticks += report.near_miss_agents as u64;
    }

    /// Record what the presentation phase actually evaluated this tick.
    ///
    /// Counted from the phase's own report rather than re-derived in
    /// `observe_tick`: the scheduler decides this, and a second copy of the
    /// decision rule here could agree with the report while both were wrong.
    pub fn record_animate(&mut self, report: &AnimateReport) {
        for tier in 0..TIER_COUNT {
            self.tiers[tier].animation_evaluations += report.evaluated_by_tier[tier];
            self.tiers[tier].animation_agent_ticks += report.agents_by_tier[tier];
        }
    }

    pub fn record_integrate(&mut self, report: &IntegrateReport) {
        self.wall_corrections += report.wall_corrections as u64;
        self.nonfinite_corrections += report.nonfinite_corrections as u64;
    }

    /// Grow per-agent tracking to cover newly spawned slots.
    fn ensure_capacity(&mut self, agent_count: usize) {
        self.previous_heading.resize(agent_count, f32::NAN);
        self.previous_turn_sign.resize(agent_count, 0);
        self.counted_arrival.resize(agent_count, false);
        self.counted_stall.resize(agent_count, false);
        self.ever_stalled.resize(agent_count, false);
        self.previous_gate_side.resize(agent_count, 0);
    }

    pub fn observe_tick(
        &mut self,
        world: &World,
        arena: &NeighborArena,
        _clock: &Clock,
        config: &MetricsConfig,
    ) {
        self.ensure_capacity(world.len());

        for slot in 0..world.len() {
            // Agents that have left the scene are not part of the crowd any
            // more. They park on a destination and every later agent routes to
            // that same point, so counting them would pile up phantom
            // penetration in proportion to how many agents actually finished —
            // penalising a solver for succeeding.
            if world.arrived[slot] || world.unrouted[slot] {
                continue;
            }

            let position = world.position(slot as u32);
            let velocity = world.velocity(slot as u32);
            // Read once per slot: every per-tier counter below must attribute
            // to the same tier, even if a later phase reschedules this agent.
            let tier = world.simulation_tier[slot] as usize;
            self.tiers[tier].agent_ticks += 1;

            // Penetration. Counted once per pair by only considering
            // neighbors with a higher agent ID, so the pair is not
            // double-counted from both sides.
            let mut penetrating = false;
            for neighbor in arena.neighbors(slot) {
                let other = neighbor.slot as usize;
                if world.agent_id[other] <= world.agent_id[slot] {
                    continue;
                }
                let combined = world.radius[slot] + world.radius[other];
                let distance = neighbor.dist_sq.sqrt();
                if distance < combined {
                    let depth = combined - distance;
                    self.penetration_pair_ticks += 1;
                    self.tiers[tier].penetration_pair_ticks += 1;
                    if depth > self.max_penetration_depth {
                        self.max_penetration_depth = depth;
                    }
                }
            }
            // Separate pass so an agent overlapping several others still
            // contributes one agent-tick, not several.
            //
            // The per-tier peak depth is taken here rather than in the pair
            // pass above: the pair pass only looks at higher-ID neighbors, so
            // a tier whose agent was the higher-ID side of the deepest overlap
            // would never see it. This pass is symmetric, so both tiers
            // involved in an overlap observe its true depth.
            let mut deepest = 0.0f32;
            for neighbor in arena.neighbors(slot) {
                let other = neighbor.slot as usize;
                let combined = world.radius[slot] + world.radius[other];
                let distance = neighbor.dist_sq.sqrt();
                if distance < combined {
                    penetrating = true;
                    deepest = deepest.max(combined - distance);
                }
            }
            if penetrating {
                self.penetration_agent_ticks += 1;
                self.tiers[tier].penetration_agent_ticks += 1;
                if deepest > self.tiers[tier].max_penetration_depth {
                    self.tiers[tier].max_penetration_depth = deepest;
                }
            }

            // Stalls.
            if world.stall_ticks[slot] >= config.stall_ticks_threshold {
                self.stall_agent_ticks += 1;
                self.tiers[tier].stall_agent_ticks += 1;
                if !self.counted_stall[slot] {
                    self.counted_stall[slot] = true;
                    self.stall_episodes += 1;
                    self.tiers[tier].stall_episodes += 1;
                    if !self.ever_stalled[slot] {
                        self.ever_stalled[slot] = true;
                        self.agents_ever_stalled += 1;
                        self.tiers[tier].agents_ever_stalled += 1;
                    }
                }
            } else if world.solver_status[slot] != SolverStatus::Braking {
                self.counted_stall[slot] = false;
            }

            // Oscillation. A reversal is a change in the *sign* of turning,
            // which is what reads as jitter; a steady arc is not a reversal
            // however sharp it is.
            let speed_sq = velocity.length_squared();
            if speed_sq > 1e-4 {
                let heading = velocity.to_yaw();
                let previous = self.previous_heading[slot];
                if previous.is_finite() {
                    let delta = wrap_angle(heading - previous);
                    if delta.abs() > config.abrupt_turn_radians {
                        self.abrupt_turns += 1;
                        self.tiers[tier].abrupt_turns += 1;
                    }
                    let sign = if delta > 1e-3 {
                        1
                    } else if delta < -1e-3 {
                        -1
                    } else {
                        0
                    };
                    if sign != 0 {
                        if self.previous_turn_sign[slot] != 0
                            && sign != self.previous_turn_sign[slot]
                        {
                            self.heading_reversals += 1;
                            self.tiers[tier].heading_reversals += 1;
                        }
                        self.previous_turn_sign[slot] = sign;
                    }
                }
                self.previous_heading[slot] = heading;
            }

            // Throughput gate. Counts only forward crossings that actually
            // pass through the gate's extent.
            //
            // The side test alone is a test against the *infinite* line, so an
            // agent crossing that line anywhere in a 126 m scene used to
            // count as passing through a 5 m doorway. And counting any sign
            // change let an agent jittering at the threshold contribute over
            // and over, in both directions.
            if let Some(gate) = config.throughput_gate {
                let side = gate_side(&gate, position);
                let previous = self.previous_gate_side[slot];
                // Forward is positive-to-negative. Gate endpoints must be
                // ordered so the incoming flow starts on the positive side —
                // see `scenes::throughput_gate`, which owns that convention.
                let forward = previous > 0 && side < 0;
                if forward && within_gate_extent(&gate, position) {
                    self.gate_crossings += 1;
                }
                if side != 0 {
                    self.previous_gate_side[slot] = side;
                }
            }
        }
    }

    /// Count newly arrived agents and record their travel time.
    pub fn record_arrivals(&mut self, world: &World, clock: &Clock) {
        self.ensure_capacity(world.len());
        for slot in 0..world.len() {
            if world.arrived[slot] && !self.counted_arrival[slot] {
                self.counted_arrival[slot] = true;
                self.arrived += 1;
                self.tiers[world.simulation_tier[slot] as usize].arrived += 1;
                let ticks = clock.tick().saturating_sub(world.spawn_tick[slot]);
                self.travel_seconds
                    .push(ticks as f32 / clock.ticks_per_second() as f32);
            }
        }
    }

    pub fn penetration_pair_ticks(&self) -> u64 {
        self.penetration_pair_ticks
    }

    pub fn max_penetration_depth(&self) -> f32 {
        self.max_penetration_depth
    }

    pub fn penetration_agent_ticks(&self) -> u64 {
        self.penetration_agent_ticks
    }

    pub fn gate_crossings(&self) -> u64 {
        self.gate_crossings
    }

    pub fn agents_ever_stalled(&self) -> u64 {
        self.agents_ever_stalled
    }

    pub fn heading_reversals(&self) -> u64 {
        self.heading_reversals
    }

    pub fn abrupt_turns(&self) -> u64 {
        self.abrupt_turns
    }

    pub fn arrived(&self) -> u64 {
        self.arrived
    }

    /// `(median, p95)` travel time in seconds. Zero when nobody arrived.
    pub fn summarize_travel_times(&self) -> (f32, f32) {
        if self.travel_seconds.is_empty() {
            return (0.0, 0.0);
        }
        let mut sorted = self.travel_seconds.clone();
        sorted.sort_by(f32::total_cmp);
        let median = sorted[sorted.len() / 2];
        let p95_index = ((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1);
        (median, sorted[p95_index])
    }

    pub fn summarize(
        &self,
        world: &World,
        scene: &CompiledScene,
        wall_time_seconds: f64,
        peak_allocated_bytes: u64,
    ) -> MetricsSummary {
        let (median_travel, p95_travel) = self.summarize_travel_times();
        let total = scene.total_agents().max(1) as f32;
        let phase_total: u64 = self.phase_nanos.iter().sum();
        let per_tier = self.summarize_tiers(world);

        MetricsSummary {
            ticks: self.ticks,
            agents_spawned: world.len() as u64,
            agents_arrived: self.arrived,
            agents_unrouted: (0..world.len()).filter(|s| world.unrouted[*s]).count() as u64,
            completion_rate: self.arrived as f32 / total,
            median_travel_seconds: median_travel,
            p95_travel_seconds: p95_travel,

            penetration_pair_ticks: self.penetration_pair_ticks,
            max_penetration_depth: self.max_penetration_depth,
            penetration_agent_ticks: self.penetration_agent_ticks,

            min_time_to_collision: if self.min_time_to_collision.is_finite() {
                self.min_time_to_collision
            } else {
                // JSON has no infinity, and a sentinel that survives a round
                // trip beats a value that silently becomes null.
                -1.0
            },
            near_miss_agent_ticks: self.near_miss_agent_ticks,
            mean_time_to_collision: if self.risk_samples > 0 {
                (self.time_to_collision_sum / self.risk_samples as f64) as f32
            } else {
                -1.0
            },

            wall_corrections: self.wall_corrections,
            nonfinite_corrections: self.nonfinite_corrections,

            agents_ever_stalled: self.agents_ever_stalled,
            stall_episodes: self.stall_episodes,
            stall_agent_ticks: self.stall_agent_ticks,

            heading_reversals: self.heading_reversals,
            abrupt_turns: self.abrupt_turns,
            gate_crossings: self.gate_crossings,

            wall_time_seconds,
            ticks_per_second_achieved: if wall_time_seconds > 0.0 {
                self.ticks as f64 / wall_time_seconds
            } else {
                0.0
            },
            peak_allocated_bytes,

            phase_time_shares: Phase::ALL
                .iter()
                .map(|phase| PhaseShare {
                    phase: phase.name().to_string(),
                    nanos: self.phase_nanos[phase.index()],
                    share: if phase_total > 0 {
                        self.phase_nanos[phase.index()] as f32 / phase_total as f32
                    } else {
                        0.0
                    },
                })
                .collect(),
            per_tier,
        }
    }

    /// Per-tier rollup, one entry per tier that carried any agent.
    ///
    /// `agents_final` is the population holding the tier at the end of the
    /// run. For a declared stable-ID profile — the M5 scale fixture — tier
    /// membership never changes, so it is also the population that produced
    /// every counter below. Under a camera-driven policy an agent can move
    /// between tiers, so compare the exposure-weighted rates rather than
    /// `completion_rate`, which then divides arrivals by an end-of-run
    /// population that did not necessarily hold the tier while travelling.
    fn summarize_tiers(&self, world: &World) -> Vec<TierMetrics> {
        let mut agents_final = [0u64; TIER_COUNT];
        for slot in 0..world.len() {
            agents_final[world.simulation_tier[slot] as usize] += 1;
        }

        (0..TIER_COUNT)
            .filter(|tier| agents_final[*tier] > 0 || self.tiers[*tier].agent_ticks > 0)
            .map(|tier| {
                let accumulated = &self.tiers[tier];
                // Rates are per agent-tick of actual exposure, which is what
                // makes one threshold file valid at 1K, 10K, and 100K.
                let exposure = accumulated.agent_ticks.max(1) as f32;
                let population = agents_final[tier].max(1) as f32;
                TierMetrics {
                    tier: TIER_NAMES[tier].to_string(),
                    agents_final: agents_final[tier],
                    agents_arrived: accumulated.arrived,
                    completion_rate: accumulated.arrived as f32 / population,
                    agent_ticks: accumulated.agent_ticks,
                    penetration_pair_ticks: accumulated.penetration_pair_ticks,
                    penetration_agent_ticks: accumulated.penetration_agent_ticks,
                    penetration_agent_ticks_per_agent_tick: accumulated.penetration_agent_ticks
                        as f32
                        / exposure,
                    max_penetration_depth: accumulated.max_penetration_depth,
                    agents_ever_stalled: accumulated.agents_ever_stalled,
                    stalled_agent_share: accumulated.agents_ever_stalled as f32 / population,
                    stall_episodes: accumulated.stall_episodes,
                    stall_agent_ticks: accumulated.stall_agent_ticks,
                    stall_agent_ticks_per_agent_tick: accumulated.stall_agent_ticks as f32
                        / exposure,
                    heading_reversals: accumulated.heading_reversals,
                    heading_reversals_per_agent_tick: accumulated.heading_reversals as f32
                        / exposure,
                    abrupt_turns: accumulated.abrupt_turns,
                    abrupt_turns_per_agent_tick: accumulated.abrupt_turns as f32 / exposure,
                    animation_evaluations: accumulated.animation_evaluations,
                    animation_agent_ticks: accumulated.animation_agent_ticks,
                    animation_evaluation_share: accumulated.animation_evaluations as f32
                        / accumulated.animation_agent_ticks.max(1) as f32,
                }
            })
            .collect()
    }
}

/// Which side of the gate line a point is on: `-1`, `0`, or `1`.
/// Whether `p` projects onto the gate segment rather than past either end.
fn within_gate_extent(gate: &Segment, p: Vec2) -> bool {
    let along = gate.b - gate.a;
    let len_sq = along.length_squared();
    if len_sq <= f32::MIN_POSITIVE {
        return false;
    }
    let t = (p - gate.a).dot(along) / len_sq;
    (0.0..=1.0).contains(&t)
}

/// Which side of the gate's infinite line `p` lies on: `-1`, `0`, or `1`.
fn gate_side(gate: &Segment, p: Vec2) -> i8 {
    let along = gate.b - gate.a;
    let to_point = p - gate.a;
    let cross = along.x * to_point.y - along.y * to_point.x;
    if cross > 0.0 {
        1
    } else if cross < 0.0 {
        -1
    } else {
        0
    }
}

/// One tier's slice of the quality metrics.
///
/// The M5 10K gate requires quality to stay within declared thresholds "for
/// the fidelity assigned to each tier", so a background S2 agent evaluated on
/// a sparse cadence is judged against a different bar than an S1 agent
/// evaluated every tick. The `_per_agent_tick` rates are the scale-independent
/// form: the same checked-in threshold applies at 1K, 10K, and 100K.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierMetrics {
    /// `S0`..`S3`.
    pub tier: String,
    /// Agents holding this tier at the end of the run.
    pub agents_final: u64,
    pub agents_arrived: u64,
    pub completion_rate: f32,
    /// Observed active agent-ticks; the denominator for every rate here.
    pub agent_ticks: u64,

    pub penetration_pair_ticks: u64,
    pub penetration_agent_ticks: u64,
    pub penetration_agent_ticks_per_agent_tick: f32,
    pub max_penetration_depth: f32,

    pub agents_ever_stalled: u64,
    pub stalled_agent_share: f32,
    pub stall_episodes: u64,
    pub stall_agent_ticks: u64,
    pub stall_agent_ticks_per_agent_tick: f32,

    pub heading_reversals: u64,
    pub heading_reversals_per_agent_tick: f32,
    pub abrupt_turns: u64,
    pub abrupt_turns_per_agent_tick: f32,

    /// Full presentation classifications performed for this tier.
    pub animation_evaluations: u64,
    /// Opportunities to perform one: agents in this tier, summed over ticks.
    pub animation_agent_ticks: u64,
    /// Share of those opportunities that were paid for. 1.0 means every agent
    /// was re-classified every tick; a lower value is the measured saving from
    /// camera/focus animation scheduling.
    pub animation_evaluation_share: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseShare {
    pub phase: String,
    pub nanos: u64,
    pub share: f32,
}

/// The report schema. Field names are baseline keys — renaming one invalidates
/// every checked-in baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub ticks: u64,
    pub agents_spawned: u64,
    pub agents_arrived: u64,
    /// Agents that never got a usable route. Reported separately because a
    /// routing regression would otherwise surface only as an unexplained drop
    /// in completion.
    pub agents_unrouted: u64,
    pub completion_rate: f32,
    pub median_travel_seconds: f32,
    pub p95_travel_seconds: f32,

    /// Pair-ticks of overlap, not distinct penetration episodes: a pair that
    /// stays overlapped for 100 ticks contributes 100.
    pub penetration_pair_ticks: u64,
    pub max_penetration_depth: f32,
    pub penetration_agent_ticks: u64,

    /// `-1.0` means no collision was ever predicted. Expect this to saturate
    /// at zero in a dense scene — one overlapping pair pins it for the whole
    /// run — which is why `mean_time_to_collision` exists beside it.
    pub min_time_to_collision: f32,
    /// Mean per-agent predicted time to collision, capped at the risk horizon.
    /// Unlike the minimum, this responds to how the whole population is doing.
    pub mean_time_to_collision: f32,
    /// Agent-ticks spent under the near-miss threshold. Scales with the
    /// population rather than saturating at the tick count.
    pub near_miss_agent_ticks: u64,

    pub wall_corrections: u64,
    pub nonfinite_corrections: u64,

    /// Distinct agents that stalled at least once.
    pub agents_ever_stalled: u64,
    /// Stall episodes, which is larger: one agent can stall repeatedly.
    pub stall_episodes: u64,
    pub stall_agent_ticks: u64,

    pub heading_reversals: u64,
    pub abrupt_turns: u64,
    pub gate_crossings: u64,

    pub wall_time_seconds: f64,
    pub ticks_per_second_achieved: f64,
    pub peak_allocated_bytes: u64,

    pub phase_time_shares: Vec<PhaseShare>,

    /// Quality broken down by simulation tier, in `S0`..`S3` order, omitting
    /// tiers no agent ever held. Defaulted on read so reports written before
    /// per-tier accounting existed still parse — they are simply not eligible
    /// for a per-tier gate.
    #[serde(default)]
    pub per_tier: Vec<TierMetrics>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NeighborArena;
    use crate::grid::UniformGrid;
    use crate::ids::AgentId;
    use crate::phases::perceive::{perceive, PerceiveConfig, PerceiveScratch};
    use crate::units::{Aabb, Vec2};
    use crate::world::{AgentSpawn, World, NO_ROUTE};

    fn world_at(points: &[Vec2], radius: f32) -> World {
        let mut world = World::new();
        for (i, p) in points.iter().enumerate() {
            world
                .spawn(
                    AgentSpawn {
                        agent_id: AgentId(i as u64 + 1),
                        population_id: 0,
                        position: *p,
                        yaw: 0.0,
                        radius,
                        max_speed: 2.0,
                        preferred_speed: 1.35,
                        route: NO_ROUTE,
                        destination: 0,
                    },
                    0,
                )
                .unwrap();
        }
        world
    }

    fn observe(world: &World, metrics: &mut Metrics, clock: &Clock) {
        let mut grid = UniformGrid::new(
            Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0)),
            5.0,
        );
        grid.rebuild(&world.pos_x, &world.pos_y);
        let mut arena = NeighborArena::new();
        perceive(
            world,
            &grid,
            &PerceiveConfig::default(),
            &mut PerceiveScratch::default(),
            &mut arena,
        );
        metrics.begin_tick();
        metrics.observe_tick(world, &arena, clock, &MetricsConfig::default());
    }

    #[test]
    fn separated_agents_record_no_penetration() {
        let world = world_at(&[Vec2::ZERO, Vec2::new(5.0, 0.0)], 0.3);
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());
        assert_eq!(metrics.penetration_pair_ticks(), 0);
        assert_eq!(metrics.max_penetration_depth(), 0.0);
    }

    #[test]
    fn overlapping_agents_record_penetration_depth() {
        // Radii 0.3 each, centres 0.4 apart: overlap of 0.2.
        let world = world_at(&[Vec2::ZERO, Vec2::new(0.4, 0.0)], 0.3);
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());
        assert_eq!(
            metrics.penetration_pair_ticks(),
            1,
            "one pair, counted once"
        );
        assert!((metrics.max_penetration_depth() - 0.2).abs() < 1e-5);
    }

    #[test]
    fn penetration_duration_accumulates_across_ticks() {
        let world = world_at(&[Vec2::ZERO, Vec2::new(0.4, 0.0)], 0.3);
        let mut metrics = Metrics::new();
        let mut clock = Clock::default();
        for _ in 0..3 {
            observe(&world, &mut metrics, &clock);
            clock.advance();
        }
        assert_eq!(metrics.penetration_agent_ticks(), 6, "2 agents x 3 ticks");
    }

    /// Overlapping pair split across tiers, which is the M5 city-flow case:
    /// a background S2 agent brushing a midground S1 agent.
    fn mixed_tier_overlap() -> World {
        let mut world = world_at(&[Vec2::ZERO, Vec2::new(0.4, 0.0)], 0.3);
        world.simulation_tier[0] = SimulationTier::S1;
        world.simulation_tier[1] = SimulationTier::S2;
        world
    }

    fn tier<'a>(summary: &'a [TierMetrics], name: &str) -> &'a TierMetrics {
        summary
            .iter()
            .find(|entry| entry.tier == name)
            .unwrap_or_else(|| panic!("{name} missing from {summary:?}"))
    }

    #[test]
    fn per_tier_penetration_attributes_to_each_agents_own_tier() {
        let world = mixed_tier_overlap();
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());

        let summary = metrics.summarize_tiers(&world);
        assert_eq!(tier(&summary, "S1").penetration_agent_ticks, 1);
        assert_eq!(tier(&summary, "S2").penetration_agent_ticks, 1);
        assert_eq!(
            tier(&summary, "S1").penetration_agent_ticks
                + tier(&summary, "S2").penetration_agent_ticks,
            metrics.penetration_agent_ticks(),
            "per-tier agent-ticks must sum to the population total"
        );
    }

    #[test]
    fn per_tier_peak_penetration_is_visible_to_the_higher_id_side() {
        // Pair-ticks are counted only from the lower-ID side. If peak depth
        // were taken from that same pass, S2 here — holding the higher ID —
        // would report a 0 m peak while actually overlapping by 0.2 m.
        let world = mixed_tier_overlap();
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());

        let summary = metrics.summarize_tiers(&world);
        assert_eq!(tier(&summary, "S1").penetration_pair_ticks, 1);
        assert_eq!(
            tier(&summary, "S2").penetration_pair_ticks,
            0,
            "the pair is counted once, from the lower-ID side"
        );
        for name in ["S1", "S2"] {
            assert!(
                (tier(&summary, name).max_penetration_depth - 0.2).abs() < 1e-5,
                "{name} must observe the true overlap depth"
            );
        }
    }

    #[test]
    fn per_tier_agent_ticks_are_the_exposure_denominator() {
        let world = mixed_tier_overlap();
        let mut metrics = Metrics::new();
        let mut clock = Clock::default();
        for _ in 0..4 {
            observe(&world, &mut metrics, &clock);
            clock.advance();
        }

        let summary = metrics.summarize_tiers(&world);
        for name in ["S1", "S2"] {
            let entry = tier(&summary, name);
            assert_eq!(entry.agent_ticks, 4, "one agent observed over four ticks");
            assert!(
                (entry.penetration_agent_ticks_per_agent_tick - 1.0).abs() < 1e-6,
                "{name} overlapped on every observed tick"
            );
        }
    }

    #[test]
    fn tiers_no_agent_holds_are_omitted() {
        let world = mixed_tier_overlap();
        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());

        let names: Vec<String> = metrics
            .summarize_tiers(&world)
            .into_iter()
            .map(|entry| entry.tier)
            .collect();
        assert_eq!(names, ["S1".to_string(), "S2".to_string()]);
    }

    #[test]
    fn per_tier_stall_counts_sum_to_the_population_total() {
        let mut world = mixed_tier_overlap();
        let config = MetricsConfig::default();
        world.stall_ticks[0] = config.stall_ticks_threshold;
        world.stall_ticks[1] = config.stall_ticks_threshold;

        let mut metrics = Metrics::new();
        observe(&world, &mut metrics, &Clock::default());

        let summary = metrics.summarize_tiers(&world);
        assert_eq!(tier(&summary, "S1").agents_ever_stalled, 1);
        assert_eq!(tier(&summary, "S2").agents_ever_stalled, 1);
        assert_eq!(
            summary
                .iter()
                .map(|entry| entry.agents_ever_stalled)
                .sum::<u64>(),
            metrics.agents_ever_stalled()
        );
    }

    #[test]
    fn stalled_agents_are_counted_once_past_the_threshold() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let config = MetricsConfig::default();
        world.stall_ticks[0] = config.stall_ticks_threshold;
        metrics.begin_tick();
        let arena = NeighborArena::new();
        metrics.observe_tick(&world, &arena, &Clock::default(), &config);
        assert_eq!(metrics.agents_ever_stalled(), 1);
    }

    #[test]
    fn a_reversing_agent_records_a_heading_reversal() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let arena = NeighborArena::new();
        let config = MetricsConfig::default();
        let clock = Clock::default();

        world.vel_x[0] = 1.0;
        world.vel_y[0] = 0.5;
        metrics.begin_tick();
        metrics.observe_tick(&world, &arena, &clock, &config);

        world.vel_y[0] = -0.5;
        metrics.begin_tick();
        metrics.observe_tick(&world, &arena, &clock, &config);

        world.vel_y[0] = 0.5;
        metrics.begin_tick();
        metrics.observe_tick(&world, &arena, &clock, &config);

        assert!(metrics.heading_reversals() >= 1);
    }

    #[test]
    fn a_straight_walker_records_no_reversals() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let arena = NeighborArena::new();
        world.vel_x[0] = 1.35;
        for _ in 0..10 {
            metrics.begin_tick();
            metrics.observe_tick(&world, &arena, &Clock::default(), &MetricsConfig::default());
        }
        assert_eq!(metrics.heading_reversals(), 0);
        assert_eq!(metrics.abrupt_turns(), 0);
    }

    #[test]
    fn arrivals_record_travel_time() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let mut clock = Clock::default();
        for _ in 0..45 {
            clock.advance();
        }
        world.arrived[0] = true;
        metrics.record_arrivals(&world, &clock);
        assert_eq!(metrics.arrived(), 1);
        // 45 ticks at 30 Hz is 1.5 s.
        let summary = metrics.summarize_travel_times();
        assert!((summary.0 - 1.5).abs() < 1e-4, "median was {}", summary.0);
    }

    #[test]
    fn an_agent_is_only_counted_as_arriving_once() {
        let mut world = world_at(&[Vec2::ZERO], 0.3);
        let mut metrics = Metrics::new();
        let clock = Clock::default();
        world.arrived[0] = true;
        metrics.record_arrivals(&world, &clock);
        metrics.record_arrivals(&world, &clock);
        assert_eq!(metrics.arrived(), 1);
    }

    #[test]
    fn phase_timings_accumulate() {
        let mut metrics = Metrics::new();
        metrics.record_phase(Phase::Steer, 1000);
        metrics.record_phase(Phase::Steer, 500);
        assert_eq!(metrics.phase_nanos(Phase::Steer), 1500);
    }
}
