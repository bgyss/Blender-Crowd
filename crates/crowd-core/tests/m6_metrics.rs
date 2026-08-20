use crowd_core::formation::FormationReportV1;
use crowd_core::ids::AgentId;
use crowd_core::m6_metrics::{
    compare_group_readability, compare_motion_quality, GroupReadabilityMetricsV1,
    MotionQualitySampleV1,
};
use crowd_core::motion::MotionCorrectionV1;

fn report(split: bool, separation: f32, intruders: usize, missing: usize) -> FormationReportV1 {
    FormationReportV1 {
        split,
        missing_members: missing,
        maximum_separation_m: separation,
        farthest_member: Some(AgentId(3)),
        intruder_agent_ids: (0..intruders).map(|id| AgentId(id as u64 + 50)).collect(),
    }
}

#[test]
fn group_metrics_are_deterministic_and_detect_readability_improvement() {
    let baseline = GroupReadabilityMetricsV1::from_reports(&[
        report(true, 5.0, 1, 1),
        report(false, 2.0, 0, 0),
    ]);
    let candidate = GroupReadabilityMetricsV1::from_reports(&[
        report(false, 2.0, 0, 0),
        report(false, 2.0, 0, 0),
    ]);
    assert!(compare_group_readability(&baseline, &candidate).improved);
    assert_eq!(candidate.split_rate_millionths, 0);
    assert_eq!(candidate.intrusion_rate_millionths, 0);
}

#[test]
fn motion_metrics_report_feasibility_fallbacks_and_hard_safety_thresholds() {
    let baseline = vec![MotionQualitySampleV1 {
        feasible: true,
        root_deviation_millionths: 100,
        foot_slide_millionths: 50,
        required_contacts: 1,
        observed_contacts: 1,
        transition_discontinuity_millionths: 100,
        correction: MotionCorrectionV1::None,
    }];
    let candidate = vec![MotionQualitySampleV1 {
        feasible: true,
        root_deviation_millionths: 80,
        foot_slide_millionths: 40,
        required_contacts: 1,
        observed_contacts: 1,
        transition_discontinuity_millionths: 90,
        correction: MotionCorrectionV1::None,
    }];
    let comparison = compare_motion_quality(&baseline, &candidate, 1_000);
    assert!(comparison.improved);
    assert_eq!(comparison.candidate.fallback_rate_millionths, 0);
    assert!(comparison.candidate.required_contact_precision_millionths >= 1_000_000);
}
