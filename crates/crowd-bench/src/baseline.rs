//! Measured baselines and relative regression checking.
//!
//! Contract section 12.3 fixes thresholds only after a baseline is measured,
//! so nothing here asserts an absolute quality bar. It asserts only that today
//! matches what was measured and reviewed.
//!
//! Because the simulation is deterministic, quality metrics are exactly
//! reproducible on the same machine, so their tolerance is zero: any drift is
//! a real behavior change, not noise. Only timing and memory need a band.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::report::Report;
use crowd_core::metrics::MetricsSummary;

/// Fractional tolerance for wall-clock and throughput figures.
const TIMING_TOLERANCE: f64 = 0.5;
/// Fractional tolerance for peak allocation, which varies with allocator
/// behavior but not nearly as much as timing.
const MEMORY_TOLERANCE: f64 = 0.15;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BaselineMetric {
    pub value: f64,
    /// Fractional tolerance. Zero demands an exact match.
    pub tolerance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    pub scene: String,
    pub agents: u32,
    pub seed: u64,
    pub scene_hash: u64,
    pub solver: String,
    pub metrics: BTreeMap<String, BaselineMetric>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drift {
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    pub tolerance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub passed: bool,
    pub drifts: Vec<Drift>,
    /// `Some((baseline, current))` when the baseline was captured with a
    /// different solver than the one just run. A cross-solver comparison is
    /// not a metric drift — the numbers were never expected to agree — so
    /// this is reported separately rather than folded into `drifts`.
    pub solver_mismatch: Option<(String, String)>,
}

/// Flatten a summary into comparable named values.
///
/// Written out by hand rather than derived: an explicit list makes adding a
/// metric a deliberate act, and the test above catches omissions.
pub fn metric_map(summary: &MetricsSummary) -> BTreeMap<String, f64> {
    // Built as an array first rather than through a closure that captures the
    // map: a `FnMut` holding `&mut map` would still be alive at the return,
    // and the borrow checker rejects that.
    [
        ("ticks", summary.ticks as f64),
        ("agents_spawned", summary.agents_spawned as f64),
        ("agents_arrived", summary.agents_arrived as f64),
        ("agents_unrouted", summary.agents_unrouted as f64),
        ("completion_rate", summary.completion_rate as f64),
        (
            "median_travel_seconds",
            summary.median_travel_seconds as f64,
        ),
        ("p95_travel_seconds", summary.p95_travel_seconds as f64),
        (
            "penetration_pair_ticks",
            summary.penetration_pair_ticks as f64,
        ),
        (
            "max_penetration_depth",
            summary.max_penetration_depth as f64,
        ),
        (
            "penetration_agent_ticks",
            summary.penetration_agent_ticks as f64,
        ),
        (
            "min_time_to_collision",
            summary.min_time_to_collision as f64,
        ),
        (
            "near_miss_agent_ticks",
            summary.near_miss_agent_ticks as f64,
        ),
        (
            "mean_time_to_collision",
            summary.mean_time_to_collision as f64,
        ),
        ("wall_corrections", summary.wall_corrections as f64),
        (
            "nonfinite_corrections",
            summary.nonfinite_corrections as f64,
        ),
        ("agents_ever_stalled", summary.agents_ever_stalled as f64),
        ("stall_episodes", summary.stall_episodes as f64),
        ("stall_agent_ticks", summary.stall_agent_ticks as f64),
        ("distance_travelled_m", summary.distance_travelled_m),
        ("penetration_episodes", summary.penetration_episodes as f64),
        (
            "deep_penetration_agent_ticks",
            summary.deep_penetration_agent_ticks as f64,
        ),
        (
            "penetration_depth_fraction_sum",
            summary.penetration_depth_fraction_sum,
        ),
        ("heading_reversals", summary.heading_reversals as f64),
        ("abrupt_turns", summary.abrupt_turns as f64),
        ("gate_crossings", summary.gate_crossings as f64),
        ("wall_time_seconds", summary.wall_time_seconds),
        (
            "ticks_per_second_achieved",
            summary.ticks_per_second_achieved,
        ),
        ("peak_allocated_bytes", summary.peak_allocated_bytes as f64),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .chain(summary.per_tier.iter().flat_map(tier_metric_entries))
    .collect()
}

/// Flatten one tier's metrics into `per_tier.<TIER>.<field>` keys.
///
/// Per-tier values are compared like any other quality metric rather than
/// being treated as commentary: a change that leaves the population-wide
/// totals intact while moving work between tiers is exactly the kind of
/// regression the M5 tier scheduler can introduce.
fn tier_metric_entries(tier: &crowd_core::metrics::TierMetrics) -> Vec<(String, f64)> {
    let prefix = format!("per_tier.{}", tier.tier);
    [
        ("agents_final", tier.agents_final as f64),
        ("agents_arrived", tier.agents_arrived as f64),
        ("completion_rate", tier.completion_rate as f64),
        ("agent_ticks", tier.agent_ticks as f64),
        (
            "contact_observed_agent_ticks",
            tier.contact_observed_agent_ticks as f64,
        ),
        ("penetration_pair_ticks", tier.penetration_pair_ticks as f64),
        (
            "penetration_agent_ticks",
            tier.penetration_agent_ticks as f64,
        ),
        (
            "penetration_agent_ticks_per_agent_tick",
            tier.penetration_agent_ticks_per_agent_tick as f64,
        ),
        ("max_penetration_depth", tier.max_penetration_depth as f64),
        ("penetration_episodes", tier.penetration_episodes as f64),
        // Compared per partner tier: a shift in *who* a tier collides with is
        // the signal these carry, and a summed value would hide it.
        (
            "penetration_with_s0_partner",
            tier.penetration_with_s0_partner as f64,
        ),
        (
            "penetration_with_s1_partner",
            tier.penetration_with_s1_partner as f64,
        ),
        (
            "penetration_with_s2_partner",
            tier.penetration_with_s2_partner as f64,
        ),
        (
            "penetration_with_s3_partner",
            tier.penetration_with_s3_partner as f64,
        ),
        (
            "deep_penetration_agent_ticks",
            tier.deep_penetration_agent_ticks as f64,
        ),
        (
            "deep_penetration_agent_ticks_per_agent_tick",
            tier.deep_penetration_agent_ticks_per_agent_tick as f64,
        ),
        (
            "mean_penetration_depth_fraction",
            tier.mean_penetration_depth_fraction as f64,
        ),
        ("agents_ever_stalled", tier.agents_ever_stalled as f64),
        ("stalled_agent_share", tier.stalled_agent_share as f64),
        ("stall_episodes", tier.stall_episodes as f64),
        ("distance_travelled_m", tier.distance_travelled_m),
        (
            "stall_episodes_per_agent_km",
            tier.stall_episodes_per_agent_km as f64,
        ),
        ("stall_agent_ticks", tier.stall_agent_ticks as f64),
        (
            "stall_agent_ticks_per_agent_tick",
            tier.stall_agent_ticks_per_agent_tick as f64,
        ),
        ("heading_reversals", tier.heading_reversals as f64),
        (
            "heading_reversals_per_agent_tick",
            tier.heading_reversals_per_agent_tick as f64,
        ),
        ("abrupt_turns", tier.abrupt_turns as f64),
        (
            "abrupt_turns_per_agent_tick",
            tier.abrupt_turns_per_agent_tick as f64,
        ),
        ("animation_evaluations", tier.animation_evaluations as f64),
        ("animation_agent_ticks", tier.animation_agent_ticks as f64),
        (
            "animation_evaluation_share",
            tier.animation_evaluation_share as f64,
        ),
    ]
    .into_iter()
    .map(|(key, value)| (format!("{prefix}.{key}"), value))
    .collect()
}

fn tolerance_for(metric: &str) -> f64 {
    match metric {
        "wall_time_seconds" | "ticks_per_second_achieved" => TIMING_TOLERANCE,
        "peak_allocated_bytes" => MEMORY_TOLERANCE,
        _ => 0.0,
    }
}

pub fn from_report(report: &Report) -> Baseline {
    Baseline {
        scene: report.scene.clone(),
        agents: report.requested_agents,
        seed: report.seed,
        scene_hash: report.scene_hash,
        solver: report.solver.clone(),
        metrics: metric_map(&report.metrics)
            .into_iter()
            .map(|(key, value)| {
                let tolerance = tolerance_for(&key);
                (key, BaselineMetric { value, tolerance })
            })
            .collect(),
    }
}

pub fn compare(baseline: &Baseline, report: &Report) -> Comparison {
    let mut drifts = Vec::new();

    // A baseline recorded from different geometry cannot be meaningfully
    // compared, so say that rather than emitting twenty confusing drifts.
    if baseline.scene_hash != report.scene_hash {
        drifts.push(Drift {
            metric: "scene_hash".to_string(),
            baseline: baseline.scene_hash as f64,
            current: report.scene_hash as f64,
            tolerance: 0.0,
        });
        return Comparison {
            passed: false,
            drifts,
            solver_mismatch: None,
        };
    }

    // A different solver was never expected to reproduce the same numbers, so
    // report that distinctly rather than as twenty confusing metric drifts.
    if baseline.solver != report.solver {
        return Comparison {
            passed: false,
            drifts,
            solver_mismatch: Some((baseline.solver.clone(), report.solver.clone())),
        };
    }

    let current = metric_map(&report.metrics);
    for (key, expected) in &baseline.metrics {
        let Some(&actual) = current.get(key) else {
            drifts.push(Drift {
                metric: key.clone(),
                baseline: expected.value,
                current: f64::NAN,
                tolerance: expected.tolerance,
            });
            continue;
        };

        let allowed = if expected.tolerance == 0.0 {
            0.0
        } else {
            expected.value.abs() * expected.tolerance
        };
        if (actual - expected.value).abs() > allowed {
            drifts.push(Drift {
                metric: key.clone(),
                baseline: expected.value,
                current: actual,
                tolerance: expected.tolerance,
            });
        }
    }

    Comparison {
        passed: drifts.is_empty(),
        drifts,
        solver_mismatch: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{run_scene, RunOptions};
    use std::path::PathBuf;

    fn report() -> crate::report::Report {
        run_scene(&RunOptions {
            scene: "crossing".to_string(),
            agents: 40,
            seed: 2026,
            svg: false,
            frames: false,
            frame_interval: crate::frames::DEFAULT_FRAME_INTERVAL_TICKS,
            trace_interval: 1,
            max_ticks: 0,
            out_dir: std::env::temp_dir().join("crowd_bench_baseline_test"),
            solver: crate::report::SolverKind::SampledVelocity,
            trace: false,
        })
        .unwrap()
    }

    #[test]
    fn a_report_compared_against_its_own_baseline_passes() {
        let report = report();
        let baseline = from_report(&report);
        let comparison = compare(&baseline, &report);
        assert!(comparison.passed, "drift: {:?}", comparison.drifts);
    }

    #[test]
    fn quality_metrics_have_zero_tolerance() {
        let baseline = from_report(&report());
        let metric = baseline.metrics.get("penetration_pair_ticks").unwrap();
        assert_eq!(metric.tolerance, 0.0);
    }

    #[test]
    fn timing_metrics_have_a_tolerance_band() {
        let baseline = from_report(&report());
        assert!(baseline.metrics.get("wall_time_seconds").unwrap().tolerance > 0.0);
    }

    #[test]
    fn a_changed_quality_metric_fails_the_check() {
        let report = report();
        let mut baseline = from_report(&report);
        baseline.metrics.get_mut("agents_arrived").unwrap().value += 5.0;
        let comparison = compare(&baseline, &report);
        assert!(!comparison.passed);
        assert!(comparison
            .drifts
            .iter()
            .any(|d| d.metric == "agents_arrived"));
    }

    #[test]
    fn timing_noise_within_tolerance_passes() {
        let report = report();
        let mut baseline = from_report(&report);
        let metric = baseline.metrics.get_mut("wall_time_seconds").unwrap();
        // Ten percent slower, well inside the timing band.
        metric.value *= 1.1;
        assert!(compare(&baseline, &report).passed);
    }

    #[test]
    fn a_scene_hash_mismatch_fails_immediately() {
        let report = report();
        let mut baseline = from_report(&report);
        baseline.scene_hash = 12345;
        let comparison = compare(&baseline, &report);
        assert!(!comparison.passed);
        assert!(
            comparison.drifts.iter().any(|d| d.metric == "scene_hash"),
            "a baseline from a different scene must be rejected, not compared"
        );
    }

    #[test]
    fn a_solver_mismatch_fails_immediately_without_metric_drifts() {
        let report = report();
        let mut baseline = from_report(&report);
        baseline.solver = "orca".to_string();
        let comparison = compare(&baseline, &report);
        assert!(!comparison.passed);
        assert_eq!(
            comparison.solver_mismatch,
            Some(("orca".to_string(), report.solver.clone()))
        );
        assert!(
            comparison.drifts.is_empty(),
            "a solver mismatch is not a metric drift: {:?}",
            comparison.drifts
        );
    }

    #[test]
    fn a_baseline_round_trips_through_json() {
        let baseline = from_report(&report());
        let json = serde_json::to_string_pretty(&baseline).unwrap();
        let parsed: Baseline = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, baseline);
    }

    #[test]
    fn checked_in_baselines_match_the_current_scene_identity_contract() {
        let baseline_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/baselines");

        for scene_name in crowd_core::scenes::SCENE_NAMES {
            let path = baseline_dir.join(format!("{scene_name}.json"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let baseline: Baseline = serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));
            let scene = crowd_core::scenes::build(scene_name, baseline.agents, baseline.seed)
                .unwrap_or_else(|| panic!("cannot build baseline scene {scene_name}"));
            let compiled = scene
                .compile()
                .unwrap_or_else(|errors| panic!("cannot compile {scene_name}: {errors:?}"));

            assert_eq!(
                baseline.scene_hash,
                compiled.scene_hash(),
                "{} was measured against an obsolete scene identity; regenerate its baseline",
                path.display()
            );
        }
    }

    #[test]
    fn every_summary_field_appears_in_the_metric_map() {
        // Guards against a metric being added to the summary but silently
        // never compared.
        //
        // Derived from the serialised summary rather than a hardcoded list:
        // a hardcoded list cannot detect the very thing this test is for,
        // since adding a field leaves the list — and the test — unchanged.
        let summary = report().metrics;
        let map = metric_map(&summary);
        let serialised = serde_json::to_value(&summary).expect("summary serialises");
        let object = serialised.as_object().expect("summary is a JSON object");

        for (key, value) in object {
            // Phase timings are a nested array of per-phase shares, not a
            // scalar quality metric, and they are excluded from comparison on
            // purpose: they are wall-clock derived and vary between runs.
            if key == "phase_time_shares" {
                continue;
            }
            // Per-tier metrics are a nested array, but unlike phase timings
            // they *are* compared — flattened to `per_tier.<TIER>.<field>`.
            // Check every field of every reported tier reached the map, so a
            // field added to `TierMetrics` cannot go silently uncompared.
            if key == "per_tier" {
                for entry in value.as_array().expect("per_tier is a JSON array") {
                    let entry = entry.as_object().expect("a tier is a JSON object");
                    let tier = entry["tier"].as_str().expect("tier is named");
                    for (field, field_value) in entry {
                        if field == "tier" {
                            continue;
                        }
                        assert!(
                            field_value.is_number(),
                            "unexpected non-scalar tier metric {field}: {field_value}"
                        );
                        let flattened = format!("per_tier.{tier}.{field}");
                        assert!(
                            map.contains_key(&flattened),
                            "tier metric `{flattened}` is in TierMetrics but never compared"
                        );
                    }
                }
                continue;
            }
            assert!(
                value.is_number(),
                "unexpected non-scalar metric {key}: {value}"
            );
            assert!(
                map.contains_key(key.as_str()),
                "metric `{key}` is in MetricsSummary but never compared against a baseline"
            );
        }
    }
}
