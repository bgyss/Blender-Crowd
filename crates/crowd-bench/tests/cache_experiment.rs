use crowd_bench::cache_bench::{run_experiment, ExperimentOptions};
use crowd_cache::{PositionEncoding, CACHE_V1_DEFAULTS};

#[test]
fn matrix_contains_every_candidate() {
    let temp = tempfile::tempdir().expect("temporary experiment directory");
    let report = run_experiment(&ExperimentOptions {
        agents: 24,
        frames: 20,
        seed: 2_026,
        out_dir: temp.path().to_path_buf(),
    })
    .expect("cache experiment succeeds");

    assert_eq!(report.results.len(), 9);
    for chunk_ticks in [30, 60, 120] {
        for encoding in ["affine_i16", "millimeter_i32", "f32"] {
            let result = report
                .results
                .iter()
                .find(|result| {
                    result.chunk_ticks == chunk_ticks && result.position_encoding == encoding
                })
                .unwrap_or_else(|| panic!("missing {chunk_ticks}/{encoding} candidate"));
            assert!(result.bytes > 0);
            assert!(result.write_duration_ns > 0);
            assert!(result.read_duration_ns > 0);
            assert!(result.write_frames_per_second.is_finite());
            assert!(result.write_frames_per_second > 0.0);
            assert!(result.read_frames_per_second.is_finite());
            assert!(result.read_frames_per_second > 0.0);
            assert!(result.max_position_error_m.is_finite());
            assert!(result.max_position_error_m <= result.declared_error_bound_m);
            assert!(result.cancel_latency_ns > 0);
            assert!(result.recovered_chunks > 0);
        }
    }
}

#[test]
fn checked_report_selection_matches_cache_defaults() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/benchmarks/2026-08-10-cache-v0-experiment.json");
    let text = std::fs::read_to_string(path).expect("checked cache experiment report");
    let report: crowd_bench::cache_bench::CacheExperimentReport =
        serde_json::from_str(&text).expect("valid cache experiment report");

    assert_eq!(report.selected.chunk_ticks, CACHE_V1_DEFAULTS.chunk_ticks);
    assert_eq!(
        report.selected.position_encoding,
        match CACHE_V1_DEFAULTS.position_encoding {
            PositionEncoding::AffineI16 => "affine_i16",
            PositionEncoding::MillimeterI32 => "millimeter_i32",
            PositionEncoding::F32 => "f32",
        }
    );
}
