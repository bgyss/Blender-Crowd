use crowd_cache::{
    AgentStatic, BakeSpec, BehaviorEventKindV1, BehaviorEventV1, CacheReader, CacheWriter,
    ChannelDef, Frame, FrameRecord, PositionEncoding, ScalarType,
};
use tempfile::tempdir;

#[test]
fn behavior_events_round_trip_with_cache_integrity_and_tick_order() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("cache");
    let mut writer = CacheWriter::create(
        &path,
        BakeSpec {
            engine_version: "test".into(),
            project_id: "events".into(),
            source_hash: "00".repeat(32),
            tick_start: 0,
            tick_end: 1,
            ticks_per_second: 30,
            agent_count: 1,
            channels: vec![ChannelDef {
                name: "position".into(),
                scalar_type: ScalarType::F32,
                arity: 2,
                quantization_error: Some(0.0),
            }],
            chunk_ticks: 2,
            position_encoding: PositionEncoding::F32,
        },
    )
    .unwrap();
    writer
        .write_agents(&[AgentStatic {
            agent_id: 7,
            population_id: 0,
            archetype_id: 0,
            variant_id: 0,
            base_scale: 1.0,
            spawn_ordinal: 0,
        }])
        .unwrap();
    writer
        .write_behavior_events(&[
            BehaviorEventV1::decision(0, 7, "leave", "queue", "east_queue"),
            BehaviorEventV1::new(1, 7, BehaviorEventKindV1::QueueAdmitted, "east_queue"),
        ])
        .unwrap();
    for tick in 0..=1 {
        writer
            .push_tick(
                tick,
                Frame {
                    records: vec![FrameRecord {
                        agent_id: 7,
                        position: [tick as f32, 0.0],
                        ..FrameRecord::default()
                    }],
                },
            )
            .unwrap();
    }
    writer.finish().unwrap();

    let reader = CacheReader::open_complete(&path).unwrap();
    let events = reader.read_behavior_events().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].tick, 0);
    assert_eq!(events[0].kind, BehaviorEventKindV1::Decision);
    assert_eq!(events[1].kind, BehaviorEventKindV1::QueueAdmitted);
    assert!(reader.manifest().behavior_events.is_some());
}

#[test]
fn unordered_behavior_events_are_rejected_before_cache_publication() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("cache");
    let mut writer = CacheWriter::create(
        &path,
        BakeSpec {
            engine_version: "test".into(),
            project_id: "events".into(),
            source_hash: "00".repeat(32),
            tick_start: 0,
            tick_end: 0,
            ticks_per_second: 30,
            agent_count: 1,
            channels: vec![],
            chunk_ticks: 1,
            position_encoding: PositionEncoding::F32,
        },
    )
    .unwrap();
    let error = writer
        .write_behavior_events(&[
            BehaviorEventV1::new(2, 7, BehaviorEventKindV1::Decision, "late"),
            BehaviorEventV1::new(1, 7, BehaviorEventKindV1::Decision, "early"),
        ])
        .unwrap_err();
    assert!(error.to_string().contains("behavior events"));
}
