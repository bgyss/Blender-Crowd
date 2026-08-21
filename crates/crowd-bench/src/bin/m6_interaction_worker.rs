//! Out-of-process model-independent R0 interaction worker.
//!
//! The worker deliberately accepts only a versioned request and writes a
//! versioned response. It has no Blender, model, accelerator, or base-cache
//! mutation path; the deterministic paired-clip adapter is the fallback and
//! reference protocol implementation.

use std::path::PathBuf;
use std::process::ExitCode;

use crowd_cache::{
    AnimationEditV1, AnimationLayerV1, FallbackClipV1, INTERACTION_LAYER_SCHEMA_VERSION,
};
use crowd_core::interaction::{deterministic_paired_clip, InteractionRequestV1};

fn usage() -> &'static str {
    "usage: m6-interaction-worker --request REQUEST.json --out MOTION.json [--layer-out LAYER.json]"
}

fn run(raw: &[String]) -> Result<(), String> {
    let mut request = None;
    let mut output = None;
    let mut layer_output = None;
    let mut index = 0;
    while index < raw.len() {
        match raw[index].as_str() {
            "--request" => {
                index += 1;
                request = Some(PathBuf::from(
                    raw.get(index).ok_or("--request needs a value")?,
                ));
            }
            "--out" => {
                index += 1;
                output = Some(PathBuf::from(raw.get(index).ok_or("--out needs a value")?));
            }
            "--layer-out" => {
                index += 1;
                layer_output = Some(PathBuf::from(
                    raw.get(index).ok_or("--layer-out needs a value")?,
                ));
            }
            other => return Err(format!("unknown argument {other}\n{}", usage())),
        }
        index += 1;
    }
    let request_path = request.ok_or_else(|| format!("missing --request\n{}", usage()))?;
    let output_path = output.ok_or_else(|| format!("missing --out\n{}", usage()))?;
    let request: InteractionRequestV1 = serde_json::from_str(
        &std::fs::read_to_string(&request_path)
            .map_err(|error| format!("read {}: {error}", request_path.display()))?,
    )
    .map_err(|error| format!("parse {}: {error}", request_path.display()))?;
    let motion = deterministic_paired_clip(&request)
        .map_err(|issues| format!("interaction request rejected: {}", format_issues(&issues)))?;
    let json = serde_json::to_string_pretty(&motion).map_err(|error| error.to_string())?;
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    std::fs::write(&output_path, format!("{json}\n"))
        .map_err(|error| format!("write {}: {error}", output_path.display()))?;
    println!("wrote {}", output_path.display());
    if let Some(layer_path) = layer_output {
        let layer = interaction_layer(&request, &motion)?;
        write_json(&layer_path, &layer)?;
        println!("wrote {}", layer_path.display());
    }
    Ok(())
}

fn interaction_layer(
    request: &InteractionRequestV1,
    motion: &crowd_core::interaction::InteractionMotionV1,
) -> Result<AnimationLayerV1, String> {
    let layer = AnimationLayerV1 {
        schema_version: INTERACTION_LAYER_SCHEMA_VERSION,
        layer_id: format!("interaction-{}", request.group_id),
        interaction_id: request.request_id.clone(),
        base_cache_hash: request.provenance.base_cache_hash.clone(),
        target_agent_ids: request
            .participants
            .iter()
            .map(|participant| participant.agent_id)
            .collect(),
        tick_start: request.tick_start,
        tick_end: request.tick_end,
        priority: 40,
        enabled: true,
        provenance: motion.provenance.backend.clone(),
        edits: motion
            .participants
            .iter()
            .flat_map(|participant| {
                participant
                    .root_samples
                    .iter()
                    .map(|sample| AnimationEditV1 {
                        agent_id: participant.agent_id,
                        tick: sample.tick,
                        clip_id: 1,
                        phase_millionths: 0,
                    })
            })
            .collect(),
        fallback: FallbackClipV1 {
            clip_set_id: motion.fallback.clip_set_id.clone(),
            clip_id: motion.fallback.clip_id.clone(),
            reason: motion.fallback.reason.clone(),
        },
    };
    layer
        .validate()
        .map_err(|error| format!("interaction layer rejected: {error}"))?;
    Ok(layer)
}

fn write_json<T: serde::Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    std::fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("write {}: {error}", path.display()))
}

fn format_issues(issues: &[crowd_core::interaction::InteractionIssue]) -> String {
    issues
        .iter()
        .map(|issue| format!("{:?}: {}", issue.code, issue.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    match run(&raw) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
