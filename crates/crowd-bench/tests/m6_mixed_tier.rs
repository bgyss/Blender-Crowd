use std::collections::BTreeMap;
use std::process::Command;

use crowd_bench::m6_mixed_tier::{run_fixture, MixedTierFixture, PHASE_NAMES};

#[test]
fn checked_fixture_has_exact_m5_mix_and_debug_evidence_boundaries() {
    let fixture = MixedTierFixture::checked_10k();
    assert_eq!(fixture.agent_count, 10_000);
    assert_eq!(fixture.ticks, 30);
    assert_eq!(fixture.tier_counts.get("S0"), Some(&10));
    assert_eq!(fixture.tier_counts.get("S1"), Some(&990));
    assert_eq!(fixture.tier_counts.get("S2"), Some(&9_000));
    assert_eq!(fixture.promoted_agent_count(), 1_000);
    assert_eq!(fixture.min_ticks_per_second, 10.0);
    assert_eq!(
        fixture.debug_evidence.get("S0").map(String::as_str),
        Some("full")
    );
    assert_eq!(
        fixture.debug_evidence.get("S1").map(String::as_str),
        Some("reduced")
    );
    assert_eq!(
        fixture.debug_evidence.get("S2").map(String::as_str),
        Some("aggregate_only")
    );
}

#[test]
fn mixed_tier_run_reports_each_authoritative_phase_and_hard_safety() {
    let report = run_fixture(&MixedTierFixture::checked_10k()).expect("fixed fixture should run");
    let timings = report
        .phase_timings
        .iter()
        .map(|timing| (timing.phase.as_str(), timing))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(timings.keys().copied().collect::<Vec<_>>(), PHASE_NAMES);
    for phase in PHASE_NAMES {
        assert!(timings[phase].nanos > 0, "{phase} timing was not measured");
        assert!(
            timings[phase].operations > 0,
            "{phase} executed no authoritative work"
        );
    }
    assert_eq!(report.tier_counts.get("S0"), Some(&10));
    assert_eq!(report.tier_counts.get("S1"), Some(&990));
    assert_eq!(report.tier_counts.get("S2"), Some(&9_000));
    assert_eq!(report.hard_safety_failures, 0);
    assert_eq!(report.unrelated_agent_mutations, 0);
    assert!(report.working_set_bytes > 0);
    assert!(report.cache_payload_bytes > 0);
    assert_eq!(report.cache_records, 300_000);
    assert_eq!(report.tier_counts_source, "runtime_state");
    assert_eq!(
        report.cache_payload_bytes,
        report.cache_records * u64::from(report.cache_record_bytes)
    );
    assert!(
        report.cache_record_bytes > 16,
        "cache evidence must cover authoritative phase outputs, not only tier and motion state"
    );
    assert_eq!(
        report.working_set_components.values().sum::<u64>(),
        report.working_set_bytes
    );
    for component in [
        "agent_state",
        "blackboards",
        "cache_payload",
        "group_runtime",
        "interaction_requests",
        "world",
    ] {
        assert!(
            report.working_set_components[component] > 0,
            "{component} memory was not derived from the runtime allocation"
        );
    }
    assert!(report.fallbacks.iter().all(|item| !item.reason.is_empty()));
    assert_eq!(
        report
            .fallbacks
            .iter()
            .map(|item| item.phase.as_str())
            .collect::<Vec<_>>(),
        PHASE_NAMES
    );
    assert!(report.fallbacks.iter().all(|item| item.count == 0));
    assert_eq!(
        report.motion_source.candidate_id,
        "cmu-mocap-subjects-35-36-m6-v1"
    );
    assert_eq!(report.motion_source.candidate_joint_limit_violations, 3_587);
    assert_eq!(report.motion_source.joint_limit_violation_limit, 0);
    assert_eq!(report.motion_source.candidate_decision, "rejected");
    assert_eq!(
        report.motion_source.accepted_database_id,
        "reference-humanoid-motion"
    );
    assert_eq!(report.motion_source.accepted_license_id, "CC0-1.0");
    assert_eq!(report.motion_source.accepted_decision, "accepted");
    assert_eq!(report.motion_source.database_hash.len(), 64);
    assert_eq!(report.motion_source.provenance_hash.len(), 64);
    assert!(report
        .unsupported_claims
        .iter()
        .any(|claim| claim.contains("production")));
    assert!(
        report.passed,
        "fixed report failed: {:?}",
        report.failure_reasons
    );
}

#[test]
fn every_tier_runs_authoritative_work_and_s2_evidence_is_aggregated_from_outputs() {
    let report = run_fixture(&MixedTierFixture::checked_10k()).expect("fixed fixture should run");
    let evidence = report
        .tier_evidence
        .iter()
        .map(|item| (item.tier.as_str(), item))
        .collect::<BTreeMap<_, _>>();

    for (tier, agents) in [("S0", 10u64), ("S1", 990), ("S2", 9_000)] {
        let item = evidence[tier];
        assert_eq!(u64::from(item.agent_count), agents);
        assert_eq!(item.cache_records, agents * u64::from(report.ticks));
        assert_eq!(item.fallbacks, 0);
        assert_eq!(item.hard_safety_failures, 0);
        assert_eq!(item.unrelated_agent_mutations, 0);
        for phase in PHASE_NAMES {
            assert!(
                item.phase_operations[phase] > 0,
                "{tier} did not execute authoritative {phase} work"
            );
        }
    }

    assert_eq!(evidence["S0"].individual_records, 10);
    assert_eq!(evidence["S1"].individual_records, 990);
    assert_eq!(evidence["S2"].individual_records, 0);
    assert_eq!(evidence["S2"].aggregate_records, report.ticks);
    assert_eq!(
        evidence["S2"].phase_operations["perception"],
        9_000 * u64::from(report.ticks)
    );
    assert_eq!(
        evidence["S2"].phase_operations["brain"],
        9_000 * u64::from(report.ticks)
    );
    assert_eq!(
        evidence["S2"].phase_operations["activity"],
        9_000 * u64::from(report.ticks)
    );
    assert_eq!(
        evidence["S2"].phase_operations["group"],
        9_000 * u64::from(report.ticks)
    );
    assert_eq!(
        evidence["S2"].phase_operations["interaction"], 9_000,
        "every S2 agent must complete one authoritative interaction during the run"
    );
    assert_eq!(
        evidence["S2"].phase_operations["motion"],
        9_000 * u64::from(report.ticks) / 2
    );

    let timing_operations = report
        .phase_timings
        .iter()
        .map(|timing| (timing.phase.as_str(), timing.operations))
        .collect::<BTreeMap<_, _>>();
    for phase in PHASE_NAMES {
        assert_eq!(
            timing_operations[phase],
            evidence
                .values()
                .map(|item| item.phase_operations[phase])
                .sum::<u64>(),
            "{phase} timing operations were not derived from tier runtime outputs"
        );
    }
}

#[test]
fn replay_hash_excludes_measurement_noise_but_covers_output_state_and_accounting() {
    let fixture = MixedTierFixture::checked_10k();
    let first = run_fixture(&fixture).expect("first fixed run should succeed");
    let second = run_fixture(&fixture).expect("second fixed run should succeed");
    assert_eq!(
        first.deterministic_replay_hash,
        second.deterministic_replay_hash
    );
    assert_eq!(first.final_state_hash, second.final_state_hash);
    assert_eq!(first.fallbacks, second.fallbacks);
    assert_eq!(first.hard_safety_failures, second.hard_safety_failures);
    assert_eq!(
        first.unrelated_agent_mutations,
        second.unrelated_agent_mutations
    );
}

#[test]
fn report_binary_writes_the_fixed_fixture_and_rejects_unknown_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let report_path = directory.path().join("report.json");
    let output = Command::new(env!("CARGO_BIN_EXE_m6-mixed-tier"))
        .args(["--out", report_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
    assert_eq!(report["fixture_id"], "m6-mixed-tier-10k-v1");
    assert_eq!(report["agent_count"], 10_000);
    assert_eq!(report["passed"], true);

    let invalid_path = directory.path().join("invalid.json");
    let invalid = Command::new(env!("CARGO_BIN_EXE_m6-mixed-tier"))
        .args(["--unknown", invalid_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!invalid_path.exists());
}
