//! Fixed per-tier adjudication of an M5 scale report.
//!
//! The M5 10K gate requires destination, penetration, stall, throughput, and
//! oscillation metrics to stay "within declared thresholds for the fidelity
//! assigned to each tier". A subjective reading of an improvement is not a
//! gate, so the thresholds live in a checked-in file that is compiled into
//! this binary: a report is adjudicated against the reviewed thresholds, not
//! against whatever file happens to be on disk beside it.
//!
//! Thresholds are expressed as rates per observed agent-tick wherever the raw
//! counter grows with population or duration. That is what lets one file gate
//! the 1K confirmation, the 10K gate, and the 100K gate without being rewritten
//! at each scale — a rewrite at each scale would make the threshold a
//! description of the last run rather than a bar it has to clear.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crowd_core::metrics::{MetricsSummary, TierMetrics};

/// The reviewed thresholds, compiled in so a gate run cannot be pointed at a
/// loosened copy.
const CITY_FLOW_THRESHOLDS: &str = include_str!("../../../benchmarks/thresholds/m5-city-flow.json");

/// Minimum report schema the gate accepts. Schema 5 introduced
/// `metrics.per_tier`; an earlier report has no per-tier evidence at all and
/// must be rerun rather than adjudicated on its population-wide totals.
pub const MINIMUM_REPORT_SCHEMA: u32 = 5;

/// A deserialise-only view of the fields the gate reads from a report.
///
/// Deliberately a subset of `crate::report::Report`, which lives in the binary
/// rather than this library. `report::tests` asserts a freshly produced report
/// parses as one of these, which is what keeps the two in step.
#[derive(Clone, Debug, Deserialize)]
pub struct GatedReport {
    pub schema_version: u32,
    pub scene: String,
    pub requested_agents: u32,
    pub fidelity_profile: Option<GatedFidelityProfile>,
    pub metrics: MetricsSummary,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GatedFidelityProfile {
    pub mode: String,
    pub s1_agents: u32,
    pub s2_agents: u32,
    pub s2_perception_interval_ticks: u32,
    pub s2_steering_interval_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    pub schema_version: u32,
    pub scene: String,
    /// Rationale for the numbers below, carried in the file so a reviewer
    /// reading the threshold sees why it was set where it is.
    pub basis: String,
    /// The profile mode string the report must declare. A report produced by a
    /// different assignment policy is not comparable and is rejected outright
    /// rather than scored.
    pub declared_profile_mode: String,
    /// Target share of the population assigned to background S2.
    pub declared_background_share: f64,
    /// Allowed deviation from that share. A stable hash partitioning a finite
    /// population lands near the target, not exactly on it.
    pub declared_share_tolerance: f64,
    /// Declared S2 perception and steering cadence, in ticks. Cadence is part
    /// of the quality/cost tradeoff the thresholds were set against, so a run
    /// at a different cadence is not gated by this file.
    pub s2_update_interval_ticks: u32,
    /// Engineering simulation-throughput budget.
    pub min_ticks_per_second: f64,
    /// Population-wide destination budget.
    pub min_completion_rate: f64,
    /// Tiers that must be present and populated in the report.
    pub required_tiers: Vec<String>,
    pub tiers: BTreeMap<String, TierThresholds>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierThresholds {
    pub min_completion_rate: f64,
    /// Share of this tier's observed agent-ticks spent overlapping anyone.
    pub max_penetration_agent_ticks_per_agent_tick: f64,
    /// Deepest overlap in metres, from either side of the pair.
    pub max_penetration_depth_m: f64,
    /// Share of the tier's population that entered a stall at least once.
    pub max_stalled_agent_share: f64,
    /// Share of the tier's observed agent-ticks spent stalled.
    pub max_stall_agent_ticks_per_agent_tick: f64,
    /// Signed-turn reversals per observed agent-tick. This counter is
    /// sensitive by construction — it fires on alternating corrections above
    /// 0.001 rad — so it carries a declared tolerance rather than a target of
    /// zero, and stays reported either way.
    pub max_heading_reversals_per_agent_tick: f64,
    /// Turns above the abrupt threshold per observed agent-tick.
    pub max_abrupt_turns_per_agent_tick: f64,
    /// Share of this tier's presentation classifications actually performed.
    /// `1.0` for camera-focused tiers, which are evaluated every tick by
    /// design; below `1.0` for background tiers, where it is the gate on M5
    /// item 4 — animation scheduling must demonstrably reduce evaluation cost
    /// rather than merely be configured.
    pub max_animation_evaluation_share: f64,
}

/// Whether a check compares upward or downward, so the printed line reads
/// correctly without the caller having to know which metric it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bound {
    AtLeast,
    AtMost,
    /// Exact string or structural equality; `measured`/`threshold` are unused.
    Equals,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    /// `None` for a population-wide or structural check.
    pub tier: Option<String>,
    pub bound: Bound,
    pub measured: f64,
    pub threshold: f64,
    /// Populated for `Equals` checks, where the numbers say nothing useful.
    pub detail: Option<String>,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adjudication {
    pub passed: bool,
    pub scene: String,
    pub requested_agents: u32,
    pub thresholds_basis: String,
    pub checks: Vec<Check>,
}

impl Adjudication {
    pub fn failures(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|check| !check.passed)
    }
}

/// The reviewed thresholds for `m5_city_flow`.
pub fn city_flow_thresholds() -> Thresholds {
    serde_json::from_str(CITY_FLOW_THRESHOLDS)
        .expect("the compiled-in M5 thresholds must parse; the parse test guards this")
}

fn at_least(name: &str, tier: Option<&str>, measured: f64, threshold: f64) -> Check {
    Check {
        name: name.to_string(),
        tier: tier.map(str::to_string),
        bound: Bound::AtLeast,
        measured,
        threshold,
        detail: None,
        passed: measured >= threshold,
    }
}

fn at_most(name: &str, tier: Option<&str>, measured: f64, threshold: f64) -> Check {
    Check {
        name: name.to_string(),
        tier: tier.map(str::to_string),
        bound: Bound::AtMost,
        measured,
        threshold,
        detail: None,
        passed: measured <= threshold,
    }
}

fn equals(name: &str, expected: &str, actual: &str) -> Check {
    Check {
        name: name.to_string(),
        tier: None,
        bound: Bound::Equals,
        measured: 0.0,
        threshold: 0.0,
        detail: Some(format!("expected {expected}, found {actual}")),
        passed: expected == actual,
    }
}

/// Adjudicate one report. Every check is evaluated and recorded, including
/// after a failure: an operator repeating a multi-minute run needs the whole
/// list, not the first thing that broke.
pub fn adjudicate(report: &GatedReport, thresholds: &Thresholds) -> Adjudication {
    let mut checks = Vec::new();

    checks.push(equals("scene", &thresholds.scene, &report.scene));
    checks.push(at_least(
        "report_schema_version",
        None,
        f64::from(report.schema_version),
        f64::from(MINIMUM_REPORT_SCHEMA),
    ));

    match &report.fidelity_profile {
        None => checks.push(equals(
            "declared_fidelity_profile",
            &thresholds.declared_profile_mode,
            "none declared",
        )),
        Some(profile) => {
            checks.push(equals(
                "declared_fidelity_profile",
                &thresholds.declared_profile_mode,
                &profile.mode,
            ));
            // Cadence is checked before quality: a run at a different cadence
            // is a different tradeoff, and passing it against these numbers
            // would be a category error rather than a pass.
            checks.push(at_most(
                "s2_perception_interval_ticks",
                Some("S2"),
                f64::from(profile.s2_perception_interval_ticks),
                f64::from(thresholds.s2_update_interval_ticks),
            ));
            checks.push(at_most(
                "s2_steering_interval_ticks",
                Some("S2"),
                f64::from(profile.s2_steering_interval_ticks),
                f64::from(thresholds.s2_update_interval_ticks),
            ));

            let committed = u64::from(profile.s1_agents) + u64::from(profile.s2_agents);
            let share = if committed > 0 {
                f64::from(profile.s2_agents) / committed as f64
            } else {
                0.0
            };
            checks.push(at_most(
                "background_share_deviation",
                None,
                (share - thresholds.declared_background_share).abs(),
                thresholds.declared_share_tolerance,
            ));
        }
    }

    checks.push(at_least(
        "completion_rate",
        None,
        f64::from(report.metrics.completion_rate),
        thresholds.min_completion_rate,
    ));
    checks.push(at_least(
        "ticks_per_second_achieved",
        None,
        report.metrics.ticks_per_second_achieved,
        thresholds.min_ticks_per_second,
    ));
    // Not a tunable budget: a non-finite correction means the integrator
    // rescued a NaN, which is a correctness fault at any tier.
    checks.push(at_most(
        "nonfinite_corrections",
        None,
        report.metrics.nonfinite_corrections as f64,
        0.0,
    ));

    // Every agent must be accounted for by a required tier.
    //
    // Without this, a run that quietly left part of the population outside the
    // declared mix — at S0, say — could still pass: the declared-share check
    // divides S2 by S1+S2, so it stays near the target while the remainder
    // goes ungated. This makes the tiers a partition of the population rather
    // than a sample of it.
    let tiered: u64 = report
        .metrics
        .per_tier
        .iter()
        .filter(|entry| thresholds.required_tiers.contains(&entry.tier))
        .map(|entry| entry.agents_final)
        .sum();
    checks.push(at_least(
        "population_covered_by_declared_tiers",
        None,
        tiered as f64,
        report.metrics.agents_spawned as f64,
    ));

    for name in &thresholds.required_tiers {
        let Some(tier) = report
            .metrics
            .per_tier
            .iter()
            .find(|entry| &entry.tier == name)
        else {
            checks.push(equals(
                &format!("{name}_present"),
                "populated",
                "missing from metrics.per_tier",
            ));
            continue;
        };
        let Some(limits) = thresholds.tiers.get(name) else {
            checks.push(equals(
                &format!("{name}_has_thresholds"),
                "declared",
                "no thresholds declared for a required tier",
            ));
            continue;
        };
        checks.extend(check_tier(tier, limits));
    }

    Adjudication {
        passed: checks.iter().all(|check| check.passed),
        scene: report.scene.clone(),
        requested_agents: report.requested_agents,
        thresholds_basis: thresholds.basis.clone(),
        checks,
    }
}

fn check_tier(tier: &TierMetrics, limits: &TierThresholds) -> Vec<Check> {
    let name = Some(tier.tier.as_str());
    vec![
        at_least(
            "completion_rate",
            name,
            f64::from(tier.completion_rate),
            limits.min_completion_rate,
        ),
        at_most(
            "penetration_agent_ticks_per_agent_tick",
            name,
            f64::from(tier.penetration_agent_ticks_per_agent_tick),
            limits.max_penetration_agent_ticks_per_agent_tick,
        ),
        at_most(
            "max_penetration_depth_m",
            name,
            f64::from(tier.max_penetration_depth),
            limits.max_penetration_depth_m,
        ),
        at_most(
            "stalled_agent_share",
            name,
            f64::from(tier.stalled_agent_share),
            limits.max_stalled_agent_share,
        ),
        at_most(
            "stall_agent_ticks_per_agent_tick",
            name,
            f64::from(tier.stall_agent_ticks_per_agent_tick),
            limits.max_stall_agent_ticks_per_agent_tick,
        ),
        at_most(
            "heading_reversals_per_agent_tick",
            name,
            f64::from(tier.heading_reversals_per_agent_tick),
            limits.max_heading_reversals_per_agent_tick,
        ),
        at_most(
            "abrupt_turns_per_agent_tick",
            name,
            f64::from(tier.abrupt_turns_per_agent_tick),
            limits.max_abrupt_turns_per_agent_tick,
        ),
        at_most(
            "animation_evaluation_share",
            name,
            f64::from(tier.animation_evaluation_share),
            limits.max_animation_evaluation_share,
        ),
    ]
}

/// One line per check, aligned so a failure is visible without reading values.
pub fn render(adjudication: &Adjudication) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "M5 gate: {} at {} agents\nthresholds: {}\n\n",
        adjudication.scene, adjudication.requested_agents, adjudication.thresholds_basis
    ));
    for check in &adjudication.checks {
        let status = if check.passed { "pass" } else { "FAIL" };
        let scope = check.tier.as_deref().unwrap_or("all");
        let body = match (&check.detail, check.bound) {
            (Some(detail), _) => detail.clone(),
            (None, Bound::AtLeast) => {
                format!("{:.6} >= {:.6}", check.measured, check.threshold)
            }
            (None, Bound::AtMost) => format!("{:.6} <= {:.6}", check.measured, check.threshold),
            (None, Bound::Equals) => String::new(),
        };
        out.push_str(&format!(
            "  {status}  {scope:<3} {:<42} {body}\n",
            check.name
        ));
    }
    out.push_str(if adjudication.passed {
        "\nresult: PASS\n"
    } else {
        "\nresult: FAIL\n"
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier_metrics(name: &str) -> TierMetrics {
        TierMetrics {
            tier: name.to_string(),
            agents_final: 900,
            agents_arrived: 900,
            completion_rate: 1.0,
            agent_ticks: 4_000_000,
            penetration_pair_ticks: 0,
            penetration_agent_ticks: 0,
            penetration_agent_ticks_per_agent_tick: 0.0,
            max_penetration_depth: 0.0,
            agents_ever_stalled: 0,
            stalled_agent_share: 0.0,
            stall_episodes: 0,
            stall_agent_ticks: 0,
            stall_agent_ticks_per_agent_tick: 0.0,
            heading_reversals: 0,
            heading_reversals_per_agent_tick: 0.0,
            abrupt_turns: 0,
            abrupt_turns_per_agent_tick: 0.0,
            // Background tiers must show a scheduling saving, so the passing
            // fixture reports a halved share rather than a full one.
            animation_evaluations: 2_000_000,
            animation_agent_ticks: 4_000_000,
            animation_evaluation_share: 0.5,
        }
    }

    /// A report that clears every threshold, so each test can break exactly
    /// one thing and see only that failure.
    fn passing_report(thresholds: &Thresholds) -> GatedReport {
        let mut metrics: MetricsSummary = serde_json::from_str(
            // `agents_spawned` matches the two 900-agent tiers below, so the
            // population-coverage check is exercised rather than trivially
            // satisfied by an empty population.
            r#"{"ticks":0,"agents_spawned":1800,"agents_arrived":1800,
                "agents_unrouted":0,"completion_rate":1.0,"median_travel_seconds":0.0,
                "p95_travel_seconds":0.0,"penetration_pair_ticks":0,
                "max_penetration_depth":0.0,"penetration_agent_ticks":0,
                "min_time_to_collision":-1.0,"mean_time_to_collision":-1.0,
                "near_miss_agent_ticks":0,"wall_corrections":0,"nonfinite_corrections":0,
                "agents_ever_stalled":0,"stall_episodes":0,"stall_agent_ticks":0,
                "heading_reversals":0,"abrupt_turns":0,"gate_crossings":0,
                "wall_time_seconds":1.0,"ticks_per_second_achieved":1000.0,
                "peak_allocated_bytes":0,"phase_time_shares":[]}"#,
        )
        .unwrap();
        metrics.per_tier = thresholds
            .required_tiers
            .iter()
            .map(|name| tier_metrics(name))
            .collect();
        GatedReport {
            schema_version: MINIMUM_REPORT_SCHEMA,
            scene: thresholds.scene.clone(),
            requested_agents: 10_000,
            fidelity_profile: Some(GatedFidelityProfile {
                mode: thresholds.declared_profile_mode.clone(),
                s1_agents: 1_000,
                s2_agents: 9_000,
                s2_perception_interval_ticks: thresholds.s2_update_interval_ticks,
                s2_steering_interval_ticks: thresholds.s2_update_interval_ticks,
            }),
            metrics,
        }
    }

    fn failing_check_names(adjudication: &Adjudication) -> Vec<String> {
        adjudication
            .failures()
            .map(|check| match &check.tier {
                Some(tier) => format!("{tier}.{}", check.name),
                None => check.name.clone(),
            })
            .collect()
    }

    #[test]
    fn the_checked_in_thresholds_parse_and_cover_every_required_tier() {
        let thresholds = city_flow_thresholds();
        assert_eq!(thresholds.scene, "m5_city_flow");
        assert!(!thresholds.required_tiers.is_empty());
        for name in &thresholds.required_tiers {
            assert!(
                thresholds.tiers.contains_key(name),
                "required tier {name} has no declared thresholds"
            );
        }
    }

    #[test]
    fn a_clean_report_passes_every_check() {
        let thresholds = city_flow_thresholds();
        let adjudication = adjudicate(&passing_report(&thresholds), &thresholds);
        assert!(
            adjudication.passed,
            "unexpected failures: {:?}",
            failing_check_names(&adjudication)
        );
    }

    #[test]
    fn a_tier_over_its_penetration_budget_fails_that_tier_only() {
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        let s2 = report
            .metrics
            .per_tier
            .iter_mut()
            .find(|tier| tier.tier == "S2")
            .unwrap();
        s2.max_penetration_depth = thresholds.tiers["S2"].max_penetration_depth_m as f32 + 1.0;

        let adjudication = adjudicate(&report, &thresholds);
        assert!(!adjudication.passed);
        assert_eq!(
            failing_check_names(&adjudication),
            ["S2.max_penetration_depth_m"]
        );
    }

    #[test]
    fn a_report_without_per_tier_metrics_cannot_pass() {
        // The pre-schema-5 case: population-wide totals only. It must fail as
        // missing evidence rather than pass by having nothing to check.
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        report.schema_version = 4;
        report.metrics.per_tier.clear();

        let adjudication = adjudicate(&report, &thresholds);
        assert!(!adjudication.passed);
        for name in &thresholds.required_tiers {
            assert!(
                failing_check_names(&adjudication).contains(&format!("{name}_present")),
                "{name} must be reported as missing"
            );
        }
    }

    #[test]
    fn a_run_at_a_different_s2_cadence_is_not_gated_by_this_file() {
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        let profile = report.fidelity_profile.as_mut().unwrap();
        profile.s2_steering_interval_ticks = thresholds.s2_update_interval_ticks + 1;

        let adjudication = adjudicate(&report, &thresholds);
        assert!(!adjudication.passed);
        assert_eq!(
            failing_check_names(&adjudication),
            ["S2.s2_steering_interval_ticks"]
        );
    }

    #[test]
    fn a_profile_mix_outside_the_declared_tolerance_fails() {
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        let profile = report.fidelity_profile.as_mut().unwrap();
        profile.s1_agents = 5_000;
        profile.s2_agents = 5_000;

        let adjudication = adjudicate(&report, &thresholds);
        assert!(!adjudication.passed);
        assert_eq!(
            failing_check_names(&adjudication),
            ["background_share_deviation"]
        );
    }

    #[test]
    fn a_report_with_no_declared_profile_is_rejected_outright() {
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        report.fidelity_profile = None;

        let adjudication = adjudicate(&report, &thresholds);
        assert!(!adjudication.passed);
        assert!(
            failing_check_names(&adjudication).contains(&"declared_fidelity_profile".to_string())
        );
    }

    /// A threshold that only the current run can clear is a description, not
    /// a gate. This replays the rejected fixed-lane two-tick 10K candidate
    /// (docs/benchmarks/2026-08-13-m5-10k-failed-baseline.md) through the same
    /// file and requires it to fail on the reasons it was actually rejected
    /// for: 87.6% of the background tier entered a continuous braking state,
    /// and peak overlap reached 0.216 m.
    #[test]
    fn the_rejected_fixed_lane_candidate_fails_these_thresholds() {
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        let s2 = report
            .metrics
            .per_tier
            .iter_mut()
            .find(|tier| tier.tier == "S2")
            .unwrap();
        s2.stalled_agent_share = 0.876;
        s2.max_penetration_depth = 0.216;
        // 3,682,282 stall agent-ticks over roughly 150M S2 agent-ticks.
        s2.stall_agent_ticks_per_agent_tick = 0.0246;

        let failures = failing_check_names(&adjudicate(&report, &thresholds));
        for expected in [
            "S2.stalled_agent_share",
            "S2.max_penetration_depth_m",
            "S2.stall_agent_ticks_per_agent_tick",
        ] {
            assert!(
                failures.contains(&expected.to_string()),
                "{expected} did not fail: {failures:?}"
            );
        }
    }

    #[test]
    fn agents_outside_the_declared_tiers_fail_the_gate() {
        // The declared-share check divides S2 by S1+S2, so a population that
        // partly escaped the declared mix would keep that ratio on target
        // while going ungated. Coverage is what closes that.
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        report.metrics.agents_spawned += 500;

        let failures = failing_check_names(&adjudicate(&report, &thresholds));
        assert_eq!(failures, ["population_covered_by_declared_tiers"]);
    }

    #[test]
    fn every_check_is_evaluated_after_the_first_failure() {
        let thresholds = city_flow_thresholds();
        let mut report = passing_report(&thresholds);
        report.scene = "not_the_scale_fixture".to_string();
        let s1 = report
            .metrics
            .per_tier
            .iter_mut()
            .find(|tier| tier.tier == "S1")
            .unwrap();
        s1.completion_rate = 0.0;

        let adjudication = adjudicate(&report, &thresholds);
        let failures = failing_check_names(&adjudication);
        assert!(failures.contains(&"scene".to_string()));
        assert!(
            failures.contains(&"S1.completion_rate".to_string()),
            "adjudication must not stop at the first failure: {failures:?}"
        );
    }
}
