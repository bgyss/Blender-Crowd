use crowd_cache::{
    AgentStatic, BakeSpec, CacheError, CacheReader, CacheStatus, CacheWriter, CancelToken, Frame,
    FrameRecord, PositionEncoding, RecoveryInspector,
};
use std::fs;
use std::path::Path;

fn spec(agent_count: u32, tick_end: u64) -> BakeSpec {
    BakeSpec {
        engine_version: "0.1.0".to_owned(),
        project_id: "project:test".to_owned(),
        source_hash: "11".repeat(32),
        tick_start: 0,
        tick_end,
        ticks_per_second: 30,
        agent_count,
        channels: Vec::new(),
        chunk_ticks: 30,
        position_encoding: PositionEncoding::MillimeterI32,
    }
}

fn agents(count: u32) -> Vec<AgentStatic> {
    (0..count)
        .map(|index| AgentStatic {
            agent_id: 10_000 + u64::from(index),
            population_id: 1,
            archetype_id: index % 3,
            variant_id: index % 7,
            base_scale: 1.0,
            spawn_ordinal: index,
        })
        .collect()
}

fn frame(count: u32, tick: u64) -> Frame {
    Frame {
        records: agents(count)
            .into_iter()
            .map(|agent| FrameRecord {
                agent_id: agent.agent_id,
                position: [tick as f32, agent.spawn_ordinal as f32],
                orientation: 0.0,
                scale: agent.base_scale,
                population_id: agent.population_id,
                variant_id: agent.variant_id,
                clip_id: 1,
                phase: tick as f32 / 60.0,
                playback_rate: 1.0,
                behavior_state: 1,
                decision_reason: 2,
                destination_id: 3,
                velocity: [1.0, 0.0],
                visible: true,
                render_tier: 1,
            })
            .collect(),
    }
}

#[test]
fn canceled_cache_is_recoverable_but_never_complete() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("shot.crowd");
    let mut writer = CacheWriter::create(&target, spec(10, 119)).unwrap();
    writer.write_agents(&agents(10)).unwrap();
    for tick in 0..60 {
        writer.push_tick(tick, frame(10, tick)).unwrap();
    }
    writer.cancel("test cancellation").unwrap();

    assert!(matches!(
        CacheReader::open_complete(&target),
        Err(CacheError::NotComplete(CacheStatus::Canceled))
    ));
    let recovery = RecoveryInspector::open(&target).unwrap();
    assert_eq!(recovery.status, CacheStatus::Canceled);
    assert_eq!(
        recovery.cancellation_reason.as_deref(),
        Some("test cancellation")
    );
    assert_eq!(recovery.last_complete_tick, Some(59));
    assert_eq!(recovery.readable_tick_range, Some(0..=59));
}

fn write_complete_cache(target: &Path, count: u32, tick_end: u64) {
    let mut writer = CacheWriter::create(target, spec(count, tick_end)).unwrap();
    writer.write_agents(&agents(count)).unwrap();
    for tick in 0..=tick_end {
        writer.push_tick(tick, frame(count, tick)).unwrap();
    }
    let manifest = writer.finish().unwrap();
    assert_eq!(manifest.status, CacheStatus::Complete);
}

#[test]
fn writer_accepts_an_empty_user_created_cache_directory_but_rejects_nonempty_paths() {
    let temp = tempfile::tempdir().unwrap();
    let empty_target = temp.path().join("empty-selected-cache");
    fs::create_dir(&empty_target).unwrap();

    let writer = CacheWriter::create(&empty_target, spec(1, 0));
    assert!(
        writer.is_ok(),
        "an empty cache directory is safe to initialize"
    );
    drop(writer);

    let nonempty_target = temp.path().join("existing-cache");
    fs::create_dir(&nonempty_target).unwrap();
    fs::write(nonempty_target.join("keep.txt"), b"do not overwrite").unwrap();
    assert!(matches!(
        CacheWriter::create(&nonempty_target, spec(1, 0)),
        Err(CacheError::AlreadyExists(path)) if path == nonempty_target
    ));
}

#[test]
fn complete_cache_round_trips_in_nonsequential_tick_order() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("complete.crowd");
    write_complete_cache(&target, 4, 59);

    let reader = CacheReader::open_complete(&target).unwrap();
    assert_eq!(reader.agents(), agents(4));
    for tick in [0, 59, 30, 1] {
        assert_eq!(reader.read_tick(tick).unwrap(), frame(4, tick));
    }
}

#[test]
fn complete_cache_decodes_each_frame_in_sequential_order() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("sequential.crowd");
    write_complete_cache(&target, 4, 59);

    let reader = CacheReader::open_complete(&target).unwrap();
    let frames = reader.read_all_frames().unwrap();
    assert_eq!(frames.len(), 60);
    for (tick, actual) in frames.iter().enumerate() {
        assert_eq!(*actual, frame(4, tick as u64));
    }
}

#[test]
fn complete_reader_names_a_corrupt_chunk() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("corrupt.crowd");
    write_complete_cache(&target, 4, 59);
    let reader = CacheReader::open_complete(&target).unwrap();
    let relative = reader.manifest().chunks[0].path.clone();
    drop(reader);
    let path = target.join(&relative);
    let mut bytes = fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&path, bytes).unwrap();

    assert!(matches!(
        CacheReader::open_complete(&target),
        Err(CacheError::FileChecksum { path: error_path, .. }) if error_path == path
    ));
}

#[test]
fn complete_reader_names_a_missing_chunk() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("missing.crowd");
    write_complete_cache(&target, 4, 59);
    let reader = CacheReader::open_complete(&target).unwrap();
    let path = target.join(&reader.manifest().chunks[1].path);
    drop(reader);
    fs::remove_file(&path).unwrap();

    assert!(matches!(
        CacheReader::open_complete(&target),
        Err(CacheError::MissingFile(error_path)) if error_path == path
    ));
}

#[test]
fn dropped_writer_leaves_finalized_chunks_readable_as_incomplete() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("interrupted.crowd");
    let mut writer = CacheWriter::create(&target, spec(4, 59)).unwrap();
    writer.write_agents(&agents(4)).unwrap();
    for tick in 0..30 {
        writer.push_tick(tick, frame(4, tick)).unwrap();
    }
    drop(writer);

    let recovery = RecoveryInspector::open(&target).unwrap();
    assert_eq!(recovery.status, CacheStatus::Incomplete);
    assert_eq!(recovery.readable_tick_range, Some(0..=29));
    assert!(matches!(
        CacheReader::open_complete(&target),
        Err(CacheError::NotComplete(CacheStatus::Incomplete))
    ));
}

#[test]
fn cancel_token_is_observable_across_threads() {
    let token = CancelToken::new();
    let worker_token = token.clone();
    let worker = std::thread::spawn(move || worker_token.cancel());
    worker.join().unwrap();
    assert!(token.is_canceled());
}

#[test]
fn cancel_before_the_first_tick_records_no_fabricated_recovery_tick() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("immediate-cancel.crowd");
    let mut writer = CacheWriter::create(&target, spec(4, 59)).unwrap();
    writer.write_agents(&agents(4)).unwrap();
    writer.cancel("canceled before simulation").unwrap();

    let recovery = RecoveryInspector::open(&target).unwrap();
    assert_eq!(recovery.status, CacheStatus::Canceled);
    assert_eq!(recovery.last_complete_tick, None);
    assert_eq!(recovery.readable_tick_range, None);
}

#[test]
fn frame_ids_must_match_the_static_agent_slots() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("id-mismatch.crowd");
    let mut writer = CacheWriter::create(&target, spec(4, 59)).unwrap();
    writer.write_agents(&agents(4)).unwrap();
    let mut wrong = frame(4, 0);
    wrong.records[2].agent_id = 999_999;

    assert!(matches!(
        writer.push_tick(0, wrong),
        Err(CacheError::AgentIdMismatch {
            slot: 2,
            expected: 10_002,
            found: 999_999
        })
    ));
}

#[test]
fn recovery_ignores_orphan_temporary_files() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("orphan-temp.crowd");
    let mut writer = CacheWriter::create(&target, spec(4, 59)).unwrap();
    writer.write_agents(&agents(4)).unwrap();
    for tick in 0..30 {
        writer.push_tick(tick, frame(4, tick)).unwrap();
    }
    drop(writer);
    fs::write(target.join("frames/orphan.chunk.tmp"), b"partial").unwrap();

    let recovery = RecoveryInspector::open(&target).unwrap();
    assert_eq!(recovery.readable_tick_range, Some(0..=29));
    assert_eq!(recovery.valid_chunk_count, 1);
}

#[test]
fn zero_chunk_size_is_rejected_as_an_invalid_bake_spec() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("invalid.crowd");
    let mut invalid = spec(4, 59);
    invalid.chunk_ticks = 0;

    assert!(matches!(
        CacheWriter::create(&target, invalid),
        Err(CacheError::InvalidBakeSpec("chunk_ticks must be positive"))
    ));
}
