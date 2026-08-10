//! Measured cache-format candidate matrix and deterministic selection.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crowd_cache::{
    content_hash, encode_chunk, AgentStatic, BakeSpec, CacheReader, CacheWriter, CancelToken,
    ChannelDef, Frame, FrameRecord, PositionEncoding, RecoveryInspector, ScalarType,
};
use serde::{Deserialize, Serialize};

const CHUNK_TICKS: [u32; 3] = [30, 60, 120];
const ENCODINGS: [PositionEncoding; 3] = [
    PositionEncoding::AffineI16,
    PositionEncoding::MillimeterI32,
    PositionEncoding::F32,
];

#[derive(Clone, Debug)]
pub struct ExperimentOptions {
    pub agents: u32,
    pub frames: u32,
    pub seed: u64,
    pub out_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheCandidateResult {
    pub chunk_ticks: u32,
    pub position_encoding: String,
    pub bytes: u64,
    pub chunk_count: usize,
    pub write_duration_ns: u64,
    pub read_duration_ns: u64,
    pub write_frames_per_second: f64,
    pub read_frames_per_second: f64,
    pub max_position_error_m: f32,
    pub declared_error_bound_m: f32,
    pub cancel_latency_ns: u64,
    pub recovered_chunks: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSelection {
    pub chunk_ticks: u32,
    pub position_encoding: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CacheExperimentReport {
    pub schema_version: u32,
    pub agents: u32,
    pub frames: u32,
    pub seed: u64,
    pub generated_at_unix_seconds: u64,
    pub os: String,
    pub arch: String,
    pub uname: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub rustc_version: String,
    pub git_commit: String,
    pub git_dirty: bool,
    pub input_hash: String,
    pub results: Vec<CacheCandidateResult>,
    pub selected: CacheSelection,
    pub selection_rule: String,
}

pub fn run_experiment(options: &ExperimentOptions) -> Result<CacheExperimentReport, String> {
    if options.agents == 0 {
        return Err("agents must be positive".to_string());
    }
    if options.frames == 0 {
        return Err("frames must be positive".to_string());
    }
    fs::create_dir_all(&options.out_dir).map_err(|error| error.to_string())?;

    let agents = fixture_agents(options.agents);
    let frames = fixture_frames(options.agents, options.frames, options.seed);
    let mut results = Vec::with_capacity(CHUNK_TICKS.len() * ENCODINGS.len());
    for chunk_ticks in CHUNK_TICKS {
        for position_encoding in ENCODINGS {
            results.push(measure_candidate(
                options,
                &agents,
                &frames,
                chunk_ticks,
                position_encoding,
            )?);
        }
    }
    let selected = select_candidate(&results)?;

    Ok(CacheExperimentReport {
        schema_version: 1,
        agents: options.agents,
        frames: options.frames,
        seed: options.seed,
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        uname: environment_value("CROWD_EXPERIMENT_UNAME", "unknown"),
        cpu: environment_value("CROWD_EXPERIMENT_CPU", "unknown"),
        ram_bytes: std::env::var("CROWD_EXPERIMENT_RAM_BYTES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        rustc_version: env!("CROWD_RUSTC_VERSION").to_string(),
        git_commit: environment_value("CROWD_EXPERIMENT_GIT_COMMIT", "unknown"),
        git_dirty: std::env::var("CROWD_EXPERIMENT_GIT_DIRTY").is_ok_and(|value| value == "true"),
        input_hash: fixture_input_hash(&agents, &frames)?,
        results,
        selected,
        selection_rule: "smallest bytes with <=0.001m error and read time <= matching f32 time * 1.10; ties prefer fewer chunks then affine_i16, millimeter_i32, f32".to_string(),
    })
}

pub fn write_report(report: &CacheExperimentReport, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|error| error.to_string())?;
    let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
    fs::write(out_dir.join("report.json"), format!("{json}\n"))
        .map_err(|error| error.to_string())?;
    fs::write(out_dir.join("report.md"), markdown_report(report)).map_err(|error| error.to_string())
}

fn measure_candidate(
    options: &ExperimentOptions,
    agents: &[AgentStatic],
    frames: &[Frame],
    chunk_ticks: u32,
    position_encoding: PositionEncoding,
) -> Result<CacheCandidateResult, String> {
    let label = encoding_name(position_encoding);
    let root = options.out_dir.join(format!("chunk-{chunk_ticks}-{label}"));
    let started = Instant::now();
    let mut writer = CacheWriter::create(&root, bake_spec(options, chunk_ticks, position_encoding))
        .map_err(|error| error.to_string())?;
    writer
        .write_agents(agents)
        .map_err(|error| error.to_string())?;
    for (tick, frame) in frames.iter().enumerate() {
        writer
            .push_tick(tick as u64, frame.clone())
            .map_err(|error| error.to_string())?;
    }
    let manifest = writer.finish().map_err(|error| error.to_string())?;
    let write_duration_ns = elapsed_ns(started);

    let started = Instant::now();
    let reader = CacheReader::open_complete(&root).map_err(|error| error.to_string())?;
    let mut max_position_error_m = 0.0f32;
    for (tick, expected) in frames.iter().enumerate() {
        let actual = reader
            .read_tick(tick as u64)
            .map_err(|error| error.to_string())?;
        for (actual, expected) in actual.records.iter().zip(&expected.records) {
            for axis in 0..2 {
                max_position_error_m = max_position_error_m
                    .max((actual.position[axis] - expected.position[axis]).abs());
            }
        }
    }
    let read_duration_ns = elapsed_ns(started);

    let cancel_root = options
        .out_dir
        .join(format!("cancel-{chunk_ticks}-{label}"));
    let mut cancel_writer = CacheWriter::create(
        &cancel_root,
        bake_spec(options, chunk_ticks, position_encoding),
    )
    .map_err(|error| error.to_string())?;
    cancel_writer
        .write_agents(agents)
        .map_err(|error| error.to_string())?;
    let cancel_frames = frames.len().min(chunk_ticks as usize);
    for (tick, frame) in frames.iter().take(cancel_frames).enumerate() {
        cancel_writer
            .push_tick(tick as u64, frame.clone())
            .map_err(|error| error.to_string())?;
    }
    let token = CancelToken::new();
    let started = Instant::now();
    token.cancel();
    if !token.is_canceled() {
        return Err("cancel token was not observed".to_string());
    }
    cancel_writer
        .cancel("cache experiment cancellation probe")
        .map_err(|error| error.to_string())?;
    let recovery = RecoveryInspector::open(&cancel_root).map_err(|error| error.to_string())?;
    let cancel_latency_ns = elapsed_ns(started);

    Ok(CacheCandidateResult {
        chunk_ticks,
        position_encoding: label.to_string(),
        bytes: directory_bytes(&root)?,
        chunk_count: manifest.chunks.len(),
        write_duration_ns,
        read_duration_ns,
        write_frames_per_second: rate(options.frames, write_duration_ns),
        read_frames_per_second: rate(options.frames, read_duration_ns),
        max_position_error_m,
        declared_error_bound_m: manifest
            .channels
            .iter()
            .find(|channel| channel.name == "position")
            .and_then(|channel| channel.quantization_error)
            .unwrap_or(0.0),
        cancel_latency_ns,
        recovered_chunks: recovery.valid_chunk_count,
    })
}

fn select_candidate(results: &[CacheCandidateResult]) -> Result<CacheSelection, String> {
    let mut eligible: Vec<&CacheCandidateResult> = results
        .iter()
        .filter(|candidate| candidate.max_position_error_m <= 0.001)
        .filter(|candidate| {
            results
                .iter()
                .find(|raw| {
                    raw.chunk_ticks == candidate.chunk_ticks && raw.position_encoding == "f32"
                })
                .is_some_and(|raw| {
                    candidate.read_duration_ns as f64 <= raw.read_duration_ns as f64 * 1.10
                })
        })
        .collect();
    eligible.sort_by_key(|candidate| {
        (
            candidate.bytes,
            candidate.chunk_count,
            encoding_rank(&candidate.position_encoding),
        )
    });
    let selected = eligible
        .first()
        .ok_or_else(|| "no cache candidate satisfies the selection rule".to_string())?;
    Ok(CacheSelection {
        chunk_ticks: selected.chunk_ticks,
        position_encoding: selected.position_encoding.clone(),
    })
}

fn bake_spec(
    options: &ExperimentOptions,
    chunk_ticks: u32,
    position_encoding: PositionEncoding,
) -> BakeSpec {
    BakeSpec {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        project_id: "cache-experiment-fixture".to_string(),
        source_hash: format!(
            "fixture-{}-{}-{}",
            options.agents, options.frames, options.seed
        ),
        tick_start: 0,
        tick_end: u64::from(options.frames - 1),
        ticks_per_second: 30,
        agent_count: options.agents,
        channels: cache_channels(position_encoding),
        chunk_ticks,
        position_encoding,
    }
}

fn cache_channels(position_encoding: PositionEncoding) -> Vec<ChannelDef> {
    let position_error = match position_encoding {
        PositionEncoding::F32 => 0.0,
        PositionEncoding::MillimeterI32 => 0.0005,
        // The exact affine bound is written in each chunk header. The fixture
        // spans at most 31 metres, giving a conservative sub-millimetre bound.
        PositionEncoding::AffineI16 => 0.0005,
    };
    vec![ChannelDef {
        name: "position".to_string(),
        scalar_type: ScalarType::F32,
        arity: 2,
        quantization_error: Some(position_error),
    }]
}

fn fixture_agents(count: u32) -> Vec<AgentStatic> {
    (0..count)
        .map(|ordinal| AgentStatic {
            agent_id: 0xc000_0000_0000_0000 | u64::from(ordinal),
            population_id: ordinal % 3,
            archetype_id: ordinal % 4,
            variant_id: ordinal % 7,
            base_scale: 0.9 + (ordinal % 11) as f32 * 0.02,
            spawn_ordinal: ordinal,
        })
        .collect()
}

fn fixture_frames(agent_count: u32, frame_count: u32, seed: u64) -> Vec<Frame> {
    (0..frame_count)
        .map(|tick| Frame {
            records: (0..agent_count)
                .map(|ordinal| {
                    let jitter = ((seed ^ u64::from(ordinal).wrapping_mul(0x9e37_79b9)) & 1023)
                        as f32
                        / 102_400.0;
                    FrameRecord {
                        agent_id: 0xc000_0000_0000_0000 | u64::from(ordinal),
                        position: [
                            ordinal as f32 * 0.03 + tick as f32 * 0.011 + jitter,
                            ((ordinal * 17 + tick * 3) % 251) as f32 * 0.02 - jitter,
                        ],
                        orientation: tick as f32 * 0.01,
                        scale: 0.9 + (ordinal % 11) as f32 * 0.02,
                        population_id: ordinal % 3,
                        variant_id: ordinal % 7,
                        clip_id: (ordinal % 4) as u16,
                        phase: (tick % 30) as f32 / 30.0,
                        playback_rate: 0.95 + (ordinal % 5) as f32 * 0.025,
                        behavior_state: (tick % 3) as u16,
                        decision_reason: (ordinal % 5) as u16,
                        destination_id: ordinal % 4,
                        velocity: [1.0 + ordinal as f32 * 0.0001, tick as f32 * 0.0002],
                        visible: true,
                        render_tier: (ordinal % 3) as u8,
                    }
                })
                .collect(),
        })
        .collect()
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos())
        .unwrap_or(u64::MAX)
        .max(1)
}

fn environment_value(name: &str, fallback: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| fallback.to_string())
}

fn fixture_input_hash(agents: &[AgentStatic], frames: &[Frame]) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(agents.len() * 28);
    for agent in agents {
        bytes.extend_from_slice(&agent.agent_id.to_le_bytes());
        bytes.extend_from_slice(&agent.population_id.to_le_bytes());
        bytes.extend_from_slice(&agent.archetype_id.to_le_bytes());
        bytes.extend_from_slice(&agent.variant_id.to_le_bytes());
        bytes.extend_from_slice(&agent.base_scale.to_le_bytes());
        bytes.extend_from_slice(&agent.spawn_ordinal.to_le_bytes());
    }
    bytes.extend_from_slice(
        &encode_chunk(0, frames, PositionEncoding::F32)
            .map_err(|error| error.to_string())?
            .bytes,
    );
    let digest = content_hash(&bytes);
    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(""))
}

fn rate(frames: u32, duration_ns: u64) -> f64 {
    f64::from(frames) * 1_000_000_000.0 / duration_ns as f64
}

fn directory_bytes(root: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| "cache byte count overflowed".to_string())?;
            }
        }
    }
    Ok(total)
}

fn encoding_name(encoding: PositionEncoding) -> &'static str {
    match encoding {
        PositionEncoding::AffineI16 => "affine_i16",
        PositionEncoding::MillimeterI32 => "millimeter_i32",
        PositionEncoding::F32 => "f32",
    }
}

fn encoding_rank(name: &str) -> u8 {
    match name {
        "affine_i16" => 0,
        "millimeter_i32" => 1,
        "f32" => 2,
        _ => u8::MAX,
    }
}

fn markdown_report(report: &CacheExperimentReport) -> String {
    let mut text = format!(
        "# Cache v0 experiment\n\n- Agents: {}\n- Frames: {}\n- Seed: {}\n- Platform: {}/{}\n- CPU: {}\n- RAM: {} bytes\n- Rust: {}\n- Git commit: `{}`\n- Git dirty: `{}`\n- Input hash: `{}`\n\n",
        report.agents,
        report.frames,
        report.seed,
        report.os,
        report.arch,
        report.cpu,
        report.ram_bytes,
        report.rustc_version,
        report.git_commit,
        report.git_dirty,
        report.input_hash,
    );
    text.push_str("| Chunk ticks | Encoding | Bytes | Write fps | Read fps | Max error (m) | Cancel (ns) | Recovered chunks |\n");
    text.push_str("| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for result in &report.results {
        text.push_str(&format!(
            "| {} | {} | {} | {:.1} | {:.1} | {:.6} | {} | {} |\n",
            result.chunk_ticks,
            result.position_encoding,
            result.bytes,
            result.write_frames_per_second,
            result.read_frames_per_second,
            result.max_position_error_m,
            result.cancel_latency_ns,
            result.recovered_chunks,
        ));
    }
    text.push_str(&format!(
        "\nSelected: `{}` with `{}`-tick chunks.\n\nSelection rule: {}.\n\nThis experiment does not establish 10,000- or 100,000-agent performance.\n",
        report.selected.position_encoding, report.selected.chunk_ticks, report.selection_rule
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        chunk_ticks: u32,
        encoding: &str,
        bytes: u64,
        read_duration_ns: u64,
        error: f32,
    ) -> CacheCandidateResult {
        CacheCandidateResult {
            chunk_ticks,
            position_encoding: encoding.to_string(),
            bytes,
            chunk_count: 120usize.div_ceil(chunk_ticks as usize),
            write_duration_ns: 1,
            read_duration_ns,
            write_frames_per_second: 1.0,
            read_frames_per_second: 1.0,
            max_position_error_m: error,
            declared_error_bound_m: error,
            cancel_latency_ns: 1,
            recovered_chunks: 1,
        }
    }

    #[test]
    fn selection_rejects_slow_or_over_budget_candidates() {
        let results = vec![
            candidate(30, "affine_i16", 50, 112, 0.0004),
            candidate(30, "millimeter_i32", 60, 100, 0.002),
            candidate(30, "f32", 100, 100, 0.0),
        ];

        assert_eq!(
            select_candidate(&results).unwrap(),
            CacheSelection {
                chunk_ticks: 30,
                position_encoding: "f32".to_string(),
            }
        );
    }

    #[test]
    fn selection_prefers_fewer_chunks_then_encoding_order_for_equal_sizes() {
        let results = vec![
            candidate(30, "f32", 100, 100, 0.0),
            candidate(60, "f32", 100, 100, 0.0),
            candidate(120, "f32", 100, 100, 0.0),
            candidate(120, "millimeter_i32", 100, 100, 0.0005),
            candidate(120, "affine_i16", 100, 100, 0.0005),
        ];

        assert_eq!(
            select_candidate(&results).unwrap(),
            CacheSelection {
                chunk_ticks: 120,
                position_encoding: "affine_i16".to_string(),
            }
        );
    }
}
