//! Fixed-point M6 quality summaries and baseline comparisons.
//!
//! These functions make local fixture improvements measurable without treating
//! a synthetic/reference run as production or human-preference evidence.

use crate::formation::FormationReportV1;
use crate::motion::MotionCorrectionV1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupReadabilityMetricsV1 {
    pub sample_count: u64,
    pub split_rate_millionths: u32,
    pub intrusion_rate_millionths: u32,
    pub missing_member_rate_millionths: u32,
    pub mean_separation_millionths: u32,
}

impl GroupReadabilityMetricsV1 {
    pub fn from_reports(reports: &[FormationReportV1]) -> Self {
        if reports.is_empty() {
            return Self::default();
        }
        let sample_count = reports.len() as u64;
        let split_count = reports.iter().filter(|report| report.split).count() as u64;
        let intrusion_count = reports
            .iter()
            .filter(|report| !report.intruder_agent_ids.is_empty())
            .count() as u64;
        let missing_count = reports
            .iter()
            .filter(|report| report.missing_members > 0)
            .count() as u64;
        let mean_separation = reports
            .iter()
            .map(|report| (report.maximum_separation_m.max(0.0) * 1_000_000.0).round() as u64)
            .sum::<u64>()
            / sample_count;
        Self {
            sample_count,
            split_rate_millionths: rate(split_count, sample_count),
            intrusion_rate_millionths: rate(intrusion_count, sample_count),
            missing_member_rate_millionths: rate(missing_count, sample_count),
            mean_separation_millionths: mean_separation.min(u64::from(u32::MAX)) as u32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupReadabilityComparisonV1 {
    pub baseline: GroupReadabilityMetricsV1,
    pub candidate: GroupReadabilityMetricsV1,
    pub improved: bool,
    pub hard_safety_preserved: bool,
}

pub fn compare_group_readability(
    baseline: &GroupReadabilityMetricsV1,
    candidate: &GroupReadabilityMetricsV1,
) -> GroupReadabilityComparisonV1 {
    let hard_safety_preserved = candidate.intrusion_rate_millionths
        <= baseline.intrusion_rate_millionths
        && candidate.missing_member_rate_millionths <= baseline.missing_member_rate_millionths;
    let no_regression = candidate.split_rate_millionths <= baseline.split_rate_millionths
        && candidate.mean_separation_millionths <= baseline.mean_separation_millionths;
    let strict_improvement = candidate.split_rate_millionths < baseline.split_rate_millionths
        || candidate.intrusion_rate_millionths < baseline.intrusion_rate_millionths
        || candidate.missing_member_rate_millionths < baseline.missing_member_rate_millionths
        || candidate.mean_separation_millionths < baseline.mean_separation_millionths;
    GroupReadabilityComparisonV1 {
        baseline: baseline.clone(),
        candidate: candidate.clone(),
        improved: hard_safety_preserved && no_regression && strict_improvement,
        hard_safety_preserved,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionQualitySampleV1 {
    pub feasible: bool,
    pub root_deviation_millionths: u32,
    pub foot_slide_millionths: u32,
    pub required_contacts: u32,
    pub observed_contacts: u32,
    pub transition_discontinuity_millionths: u32,
    pub correction: MotionCorrectionV1,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MotionQualityMetricsV1 {
    pub sample_count: u64,
    pub feasible_rate_millionths: u32,
    pub fallback_rate_millionths: u32,
    pub mean_root_deviation_millionths: u32,
    pub mean_foot_slide_millionths: u32,
    pub required_contact_precision_millionths: u32,
    pub mean_transition_discontinuity_millionths: u32,
    pub hard_safety_violations: u32,
}

impl MotionQualityMetricsV1 {
    pub fn from_samples(samples: &[MotionQualitySampleV1]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let sample_count = samples.len() as u64;
        let feasible_count = samples.iter().filter(|sample| sample.feasible).count() as u64;
        let fallback_count = samples
            .iter()
            .filter(|sample| sample.correction == MotionCorrectionV1::FallbackClip)
            .count() as u64;
        let required = samples
            .iter()
            .map(|sample| u64::from(sample.required_contacts))
            .sum::<u64>();
        let observed = samples
            .iter()
            .map(|sample| u64::from(sample.observed_contacts.min(sample.required_contacts)))
            .sum::<u64>();
        Self {
            sample_count,
            feasible_rate_millionths: rate(feasible_count, sample_count),
            fallback_rate_millionths: rate(fallback_count, sample_count),
            mean_root_deviation_millionths: mean_u32(
                samples
                    .iter()
                    .map(|sample| sample.root_deviation_millionths),
            ),
            mean_foot_slide_millionths: mean_u32(
                samples.iter().map(|sample| sample.foot_slide_millionths),
            ),
            required_contact_precision_millionths: if required == 0 {
                1_000_000
            } else {
                rate(observed, required)
            },
            mean_transition_discontinuity_millionths: mean_u32(
                samples
                    .iter()
                    .map(|sample| sample.transition_discontinuity_millionths),
            ),
            hard_safety_violations: samples.iter().filter(|sample| !sample.feasible).count() as u32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MotionQualityComparisonV1 {
    pub baseline: MotionQualityMetricsV1,
    pub candidate: MotionQualityMetricsV1,
    pub improved: bool,
    pub hard_safety_preserved: bool,
}

pub fn compare_motion_quality(
    baseline_samples: &[MotionQualitySampleV1],
    candidate_samples: &[MotionQualitySampleV1],
    minimum_contact_precision_millionths: u32,
) -> MotionQualityComparisonV1 {
    let baseline = MotionQualityMetricsV1::from_samples(baseline_samples);
    let candidate = MotionQualityMetricsV1::from_samples(candidate_samples);
    let hard_safety_preserved = candidate.hard_safety_violations <= baseline.hard_safety_violations
        && candidate.required_contact_precision_millionths >= minimum_contact_precision_millionths;
    let no_regression = candidate.feasible_rate_millionths >= baseline.feasible_rate_millionths
        && candidate.mean_root_deviation_millionths <= baseline.mean_root_deviation_millionths
        && candidate.mean_foot_slide_millionths <= baseline.mean_foot_slide_millionths
        && candidate.mean_transition_discontinuity_millionths
            <= baseline.mean_transition_discontinuity_millionths;
    let strict_improvement = candidate.feasible_rate_millionths > baseline.feasible_rate_millionths
        || candidate.mean_root_deviation_millionths < baseline.mean_root_deviation_millionths
        || candidate.mean_foot_slide_millionths < baseline.mean_foot_slide_millionths
        || candidate.mean_transition_discontinuity_millionths
            < baseline.mean_transition_discontinuity_millionths;
    MotionQualityComparisonV1 {
        baseline,
        candidate,
        improved: hard_safety_preserved && no_regression && strict_improvement,
        hard_safety_preserved,
    }
}

fn rate(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(1_000_000) / denominator).min(1_000_000)) as u32
}

fn mean_u32(values: impl Iterator<Item = u32>) -> u32 {
    let mut count = 0u64;
    let mut sum = 0u64;
    for value in values {
        count += 1;
        sum += u64::from(value);
    }
    if count == 0 {
        0
    } else {
        (sum / count).min(u64::from(u32::MAX)) as u32
    }
}
