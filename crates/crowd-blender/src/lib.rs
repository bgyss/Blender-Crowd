//! PyO3 bridge from trace v0 to Blender.
//!
//! This module decides nothing. It performs no simulation, applies no policy,
//! and holds no state beyond an open file and its parsed header. Anything
//! requiring a decision belongs in `crowd-core` or in the addon. Keeping this
//! rule is what keeps the FFI surface small enough to audit.
//!
//! Every failure is raised as `OSError`. A trace error is always something
//! about a file on disk — missing, truncated, wrong format, tick past the
//! end — so the addon has one exception type to catch rather than a bespoke
//! hierarchy it would have to import before it could handle anything.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crowd_cache::{
    compose_frame, compose_layout_frame_v1, write_usda_crowd_profile_v1, AgentStatic,
    AnimationLayerV1, BakeSpec, BehaviorEventCompactor, BehaviorEventKindV1, BehaviorEventV1,
    CacheReader, CacheStatus, CacheWriter, ChannelDef, Frame as CacheFrame, FrameRecord,
    LayoutLayerV1, LocalResimulationRequestV1, OverrideLayerV1, PhysicsHandoffSpecV1,
    RecoveryInspector, RecoveryReport, ScalarType, CACHE_V1_DEFAULTS,
};
use crowd_core::authoring::{
    compile_authorable_project as compile_core_authorable_project, migrate_project_v1,
    AuthorableProjectV2, CompiledAuthorableProject,
};
use crowd_core::avoidance::SampledVelocitySolver;
use crowd_core::behavior::{compile_graph, BehaviorGraphV1};
use crowd_core::interaction::{
    ContactConstraintV1, InteractionBudgetsV1, InteractionModeV1, InteractionMotionV1,
    InteractionParticipantV1, InteractionProvenanceV1, InteractionRequestV1, RootConstraintV1,
    RootSampleV1,
};
use crowd_core::project::{
    compile_project as compile_core_project, CompiledAgentSpawn,
    CompiledProject as CoreCompiledProject, Diagnostic,
};
use crowd_core::{compile_concourse, FidelityPolicy, SimConfig, Simulation};
use crowd_trace::{AgentRecord, TraceReader};
use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

#[pyclass(name = "CompiledProject", frozen)]
struct PyCompiledProject {
    inner: Arc<CoreCompiledProject>,
}

/// A compiled M2 project retains its graph and social controllers for a
/// native-only bake.  It deliberately has the same narrow session boundary as
/// the M1 project, so Blender never becomes an authoritative simulator.
#[pyclass(name = "AuthorableProject", frozen)]
struct PyAuthorableProject {
    base: Arc<CoreCompiledProject>,
    inner: Arc<CompiledAuthorableProject>,
}

#[pymethods]
impl PyAuthorableProject {
    #[getter]
    fn agent_count(&self) -> usize {
        self.base.agent_spawns().len()
    }

    #[pyo3(signature = (agent_count=None))]
    fn create_session(&self, agent_count: Option<u32>) -> PyResult<Session> {
        let requested = agent_count.unwrap_or(self.base.agent_spawns().len() as u32);
        if requested == 0 || requested as usize > self.base.agent_spawns().len() {
            return Err(PyValueError::new_err(format!(
                "E_AGENT_COUNT: requested {requested}, project contains {}",
                self.base.agent_spawns().len()
            )));
        }
        Session::create_authorable(Arc::clone(&self.base), Arc::clone(&self.inner), requested)
    }
}

#[pymethods]
impl PyCompiledProject {
    #[getter]
    fn agent_count(&self) -> usize {
        self.inner.agent_spawns().len()
    }

    #[getter]
    fn project_id(&self) -> &str {
        &self.inner.ir().project_id
    }

    #[getter]
    fn source_hash(&self) -> String {
        self.inner.source_hash_hex()
    }

    fn agent_ids(&self) -> Vec<u64> {
        self.inner
            .agent_spawns()
            .iter()
            .map(|spawn| spawn.agent_id.0)
            .collect()
    }

    /// `fidelity_profile` names a declared M5 tier mix. Omitting it leaves
    /// every agent at S0/R0, which is the pre-M5 behavior and what every
    /// existing caller gets.
    #[pyo3(signature = (agent_count=None, fidelity_profile=None))]
    fn create_session(
        &self,
        agent_count: Option<u32>,
        fidelity_profile: Option<&str>,
    ) -> PyResult<Session> {
        let requested = agent_count.unwrap_or(self.inner.agent_spawns().len() as u32);
        if requested == 0 || requested as usize > self.inner.agent_spawns().len() {
            return Err(PyValueError::new_err(format!(
                "E_AGENT_COUNT: requested {requested}, project contains {}",
                self.inner.agent_spawns().len()
            )));
        }
        Session::create(
            Arc::clone(&self.inner),
            requested,
            parse_fidelity_profile(fidelity_profile)?,
        )
    }
}

/// Resolve a declared M5 tier-mix name to a policy.
///
/// Names rather than raw radii/shares: the mix a scale report claims must be
/// one the repository can reproduce by name, and an arbitrary caller-supplied
/// share would make a cache's declared profile unverifiable.
fn parse_fidelity_profile(name: Option<&str>) -> PyResult<Option<FidelityPolicy>> {
    match name {
        None => Ok(None),
        Some("m5_background_10_90") => Ok(Some(FidelityPolicy::m5_10k_profile())),
        Some(other) => Err(PyValueError::new_err(format!(
            "E_FIDELITY_PROFILE: unknown profile {other}; known profiles: m5_background_10_90"
        ))),
    }
}

#[pyclass(name = "CancelToken", frozen)]
struct PyCancelToken {
    inner: crowd_cache::CancelToken,
}

#[pymethods]
impl PyCancelToken {
    #[new]
    fn new() -> Self {
        Self {
            inner: crowd_cache::CancelToken::new(),
        }
    }

    fn cancel(&self) {
        self.inner.cancel();
    }

    #[getter]
    fn is_canceled(&self) -> bool {
        self.inner.is_canceled()
    }
}

#[pyclass]
struct Session {
    project: Arc<CoreCompiledProject>,
    authorable_bake: bool,
    simulation: Simulation,
    agent_count: u32,
}

#[derive(Clone, Debug)]
struct BakeOutcome {
    status: CacheStatus,
    last_complete_tick: Option<u64>,
}

#[pymethods]
impl Session {
    #[getter]
    fn agent_count(&self) -> u32 {
        self.agent_count
    }

    #[getter]
    fn tick(&self) -> u64 {
        self.simulation.clock().tick()
    }

    #[getter]
    fn state_hash(&self) -> u64 {
        self.simulation.state_hash()
    }

    #[pyo3(signature = (ticks=1))]
    fn step(&mut self, py: Python<'_>, ticks: u64) {
        py.detach(|| self.simulation.run(ticks));
    }

    fn query_agent<'py>(&self, py: Python<'py>, agent_id: u64) -> PyResult<Bound<'py, PyDict>> {
        let compiled = self
            .selected_agents()
            .iter()
            .find(|spawn| spawn.agent_id.0 == agent_id)
            .ok_or_else(|| PyValueError::new_err(format!("E_AGENT_NOT_FOUND: {agent_id}")))?;
        let frame = self.frame_record(compiled);
        let out = PyDict::new(py);
        out.set_item("agent_id", frame.agent_id)?;
        out.set_item("position", [frame.position[0], frame.position[1], 0.0])?;
        out.set_item("velocity", [frame.velocity[0], frame.velocity[1], 0.0])?;
        out.set_item("orientation", frame.orientation)?;
        out.set_item("destination_id", frame.destination_id)?;
        out.set_item("behavior_state", frame.behavior_state)?;
        out.set_item("decision_reason", frame.decision_reason)?;
        out.set_item("visible", frame.visible)?;
        Ok(out)
    }

    #[pyo3(signature = (path, ticks, cancel_token))]
    fn bake<'py>(
        &mut self,
        py: Python<'py>,
        path: PathBuf,
        ticks: u64,
        cancel_token: PyRef<'py, PyCancelToken>,
    ) -> PyResult<Bound<'py, PyDict>> {
        if ticks == 0 {
            return Err(PyValueError::new_err(
                "E_BAKE_TICKS: ticks must be positive",
            ));
        }
        let token = cancel_token.inner.clone();
        drop(cancel_token);
        let outcome = py
            .detach(|| self.bake_native(&path, ticks, &token))
            .map_err(|error| {
                PyOSError::new_err(format!("E_CACHE_BAKE {}: {error}", path.display()))
            })?;
        bake_outcome_dict(py, &path, outcome)
    }
}

impl Session {
    fn create(
        project: Arc<CoreCompiledProject>,
        agent_count: u32,
        fidelity: Option<FidelityPolicy>,
    ) -> PyResult<Self> {
        let scene = facade_scene(&project, agent_count)
            .map_err(|error| PyValueError::new_err(format!("E_SESSION_COMPILE: {error}")))?;
        Ok(Self {
            project,
            authorable_bake: false,
            simulation: Simulation::new(
                scene,
                Box::new(SampledVelocitySolver::default()),
                SimConfig {
                    fidelity,
                    ..SimConfig::default()
                },
            ),
            agent_count,
        })
    }

    fn create_authorable(
        project: Arc<CoreCompiledProject>,
        authorable: Arc<CompiledAuthorableProject>,
        agent_count: u32,
    ) -> PyResult<Self> {
        let scene = facade_scene(&project, agent_count)
            .map_err(|error| PyValueError::new_err(format!("E_SESSION_COMPILE: {error}")))?;
        let mut simulation = Simulation::new(
            scene,
            Box::new(SampledVelocitySolver::default()),
            SimConfig::default(),
        );
        simulation.enable_authorable_behavior(authorable.runtime_controller());
        Ok(Self {
            project,
            authorable_bake: true,
            simulation,
            agent_count,
        })
    }

    fn selected_agents(&self) -> &[CompiledAgentSpawn] {
        &self.project.agent_spawns()[..self.agent_count as usize]
    }

    fn frame_record(&self, compiled: &CompiledAgentSpawn) -> FrameRecord {
        let Some(snapshot) = self.simulation.query_agent(compiled.agent_id) else {
            return FrameRecord {
                agent_id: compiled.agent_id.0,
                scale: compiled.scale,
                population_id: compiled.population_id,
                variant_id: compiled.appearance_id,
                destination_id: compiled.destination_id,
                visible: false,
                ..FrameRecord::default()
            };
        };
        FrameRecord {
            agent_id: compiled.agent_id.0,
            position: [snapshot.position.x, snapshot.position.y],
            orientation: snapshot.orientation,
            scale: snapshot.scale,
            population_id: snapshot.population_id,
            variant_id: snapshot.variant_id,
            clip_id: snapshot.clip_state.clip_id,
            phase: snapshot.clip_state.phase,
            playback_rate: snapshot.clip_state.playback_rate,
            behavior_state: snapshot.commuter_state as u16,
            decision_reason: snapshot.decision_reason as u16,
            destination_id: snapshot.destination_id,
            velocity: [snapshot.velocity.x, snapshot.velocity.y],
            visible: snapshot.visible,
            render_tier: snapshot.render_tier,
        }
    }

    fn cache_frame(&self) -> CacheFrame {
        CacheFrame {
            records: self
                .selected_agents()
                .iter()
                .map(|compiled| self.frame_record(compiled))
                .collect(),
        }
    }

    fn bake_native(
        &mut self,
        path: &Path,
        ticks: u64,
        cancel_token: &crowd_cache::CancelToken,
    ) -> Result<BakeOutcome, crowd_cache::CacheError> {
        let tick_start = self.simulation.clock().tick();
        let tick_end =
            tick_start
                .checked_add(ticks - 1)
                .ok_or(crowd_cache::CacheError::InvalidBakeSpec(
                    "tick range overflow",
                ))?;
        let mut writer = CacheWriter::create(
            path,
            BakeSpec {
                engine_version: env!("CARGO_PKG_VERSION").to_string(),
                project_id: self.project.ir().project_id.clone(),
                source_hash: self.project.source_hash_hex(),
                tick_start,
                tick_end,
                ticks_per_second: self.simulation.clock().ticks_per_second(),
                agent_count: self.agent_count,
                channels: cache_channel_defs(),
                chunk_ticks: CACHE_V1_DEFAULTS.chunk_ticks,
                position_encoding: CACHE_V1_DEFAULTS.position_encoding,
            },
        )?;
        let agents: Vec<AgentStatic> = self
            .selected_agents()
            .iter()
            .map(|spawn| AgentStatic {
                agent_id: spawn.agent_id.0,
                population_id: spawn.population_id,
                archetype_id: spawn.archetype_id,
                variant_id: spawn.appearance_id,
                base_scale: spawn.scale,
                spawn_ordinal: spawn.spawn_ordinal,
            })
            .collect();
        writer.write_agents(&agents)?;

        let mut behavior_events = BehaviorEventCompactor::default();
        for offset in 0..ticks {
            if cancel_token.is_canceled() {
                if self.authorable_bake {
                    writer.write_behavior_events(&behavior_events.into_events())?;
                }
                let manifest = writer.cancel("canceled by caller")?;
                return Ok(BakeOutcome {
                    status: manifest.status,
                    last_complete_tick: manifest.last_complete_tick,
                });
            }
            writer.push_tick(tick_start + offset, self.cache_frame())?;
            self.simulation.step();
            for event in self.simulation.drain_behavior_events() {
                behavior_events.push(BehaviorEventV1 {
                    tick: event.tick,
                    agent_id: event.agent_id.0,
                    kind: behavior_event_kind(event.kind),
                    detail: event.detail,
                    graph_id: event.graph_id,
                    decisive_node: event.decisive_node,
                    utility_scores: event.utility_scores,
                    fuzzy_scores: event.fuzzy_scores,
                    perception_channels: event.perception_channels,
                    blackboard_values: event.blackboard_values,
                    degraded_evidence: event.degraded_evidence,
                });
            }
        }
        if self.authorable_bake {
            writer.write_behavior_events(&behavior_events.into_events())?;
        }
        let manifest = writer.finish()?;
        Ok(BakeOutcome {
            status: manifest.status,
            last_complete_tick: manifest.last_complete_tick,
        })
    }
}

enum CacheBacking {
    Complete(Box<CacheReader>),
    Inspection(RecoveryReport),
}

#[pyclass(name = "Cache")]
struct PyCache {
    path: PathBuf,
    backing: CacheBacking,
    base_cache_hash: String,
    override_layers: Vec<OverrideLayerV1>,
    layout_layers: Vec<LayoutLayerV1>,
    cached_behavior_query: Option<(u64, u64, Option<String>)>,
}

/// Select the event bundle that most recently explains an agent's state at a
/// timeline tick. Decision events are sparse by design, so exact-tick lookup
/// leaves the debugger blank for almost every frame.
fn inspection_behavior_events(
    events: &[BehaviorEventV1],
    agent_id: u64,
    tick: u64,
) -> Vec<BehaviorEventV1> {
    let latest_tick = events
        .iter()
        .filter(|event| event.agent_id == agent_id && event.tick <= tick)
        .map(|event| event.tick)
        .max();
    latest_tick.map_or_else(Vec::new, |selected_tick| {
        events
            .iter()
            .filter(|event| event.agent_id == agent_id && event.tick == selected_tick)
            .cloned()
            .collect()
    })
}

#[pymethods]
impl PyCache {
    #[new]
    #[pyo3(signature = (path, require_complete=true))]
    fn new(path: PathBuf, require_complete: bool) -> PyResult<Self> {
        let backing = if require_complete {
            CacheBacking::Complete(Box::new(CacheReader::open_complete(&path).map_err(
                |error| PyOSError::new_err(format!("E_CACHE_OPEN {}: {error}", path.display())),
            )?))
        } else {
            match CacheReader::open_complete(&path) {
                Ok(reader) => CacheBacking::Complete(Box::new(reader)),
                Err(_) => {
                    CacheBacking::Inspection(RecoveryInspector::open(&path).map_err(|error| {
                        PyOSError::new_err(format!("E_CACHE_INSPECT {}: {error}", path.display()))
                    })?)
                }
            }
        };
        let base_cache_hash = match &backing {
            CacheBacking::Complete(reader) => reader.base_cache_hash_hex().map_err(|error| {
                PyOSError::new_err(format!("E_CACHE_IDENTITY {}: {error}", path.display()))
            })?,
            CacheBacking::Inspection(_) => String::new(),
        };
        Ok(Self {
            path,
            backing,
            base_cache_hash,
            override_layers: Vec::new(),
            layout_layers: Vec::new(),
            cached_behavior_query: None,
        })
    }

    #[getter]
    fn status(&self) -> &'static str {
        match &self.backing {
            CacheBacking::Complete(_) => "complete",
            CacheBacking::Inspection(report) => cache_status_name(report.status),
        }
    }

    #[getter]
    fn agent_count(&self) -> PyResult<u32> {
        Ok(self.complete()?.manifest().agent_count)
    }

    #[getter]
    fn tick_start(&self) -> PyResult<u64> {
        Ok(self.complete()?.manifest().tick_start)
    }

    #[getter]
    fn tick_end(&self) -> PyResult<u64> {
        Ok(self.complete()?.manifest().tick_end)
    }

    #[getter]
    fn ticks_per_second(&self) -> PyResult<u32> {
        Ok(self.complete()?.manifest().ticks_per_second)
    }

    #[getter]
    fn source_hash(&self) -> PyResult<&str> {
        Ok(&self.complete()?.manifest().source_hash)
    }

    #[getter]
    fn base_cache_hash(&self) -> &str {
        &self.base_cache_hash
    }

    fn read_tick<'py>(&self, py: Python<'py>, tick: u64) -> PyResult<Bound<'py, PyDict>> {
        let frame = self.complete()?.read_tick(tick).map_err(|error| {
            PyOSError::new_err(format!("E_CACHE_READ {}: {error}", self.path.display()))
        })?;
        let reader = self.complete()?;
        let mut packed = pack_cache(&frame.records, reader.agents());
        if !self.override_layers.is_empty() {
            let composed = compose_frame(&frame, tick, &self.override_layers)
                .map_err(|error| PyValueError::new_err(format!("E_OVERRIDE: {error}")))?;
            packed.position.clear();
            for record in composed.records {
                push_f32x3(
                    &mut packed.position,
                    record.position[0],
                    record.position[1],
                    record.position[2],
                );
            }
        }
        if !self.layout_layers.is_empty() {
            let composed =
                compose_layout_frame_v1(&frame, tick, &self.base_cache_hash, &self.layout_layers)
                    .map_err(|error| PyValueError::new_err(format!("E_LAYOUT: {error}")))?;
            apply_layout_records(&mut packed, &composed.records);
        }
        packed_cache_dict(py, &packed)
    }

    fn read_agents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        packed_agent_static_dict(py, &pack_agent_static(self.complete()?.agents()))
    }

    fn scan_ticks(&self) -> PyResult<usize> {
        self.complete()?
            .read_all_frames()
            .map(|frames| frames.len())
            .map_err(|error| {
                PyOSError::new_err(format!("E_CACHE_SCAN {}: {error}", self.path.display()))
            })
    }

    fn set_override_layers(&mut self, layers_json: &str) -> PyResult<()> {
        self.override_layers = serde_json::from_str(layers_json)
            .map_err(|error| PyValueError::new_err(format!("E_OVERRIDE_JSON: {error}")))?;
        Ok(())
    }

    fn clear_override_layers(&mut self) {
        self.override_layers.clear();
    }

    fn set_layout_layers(&mut self, layers_json: &str) -> PyResult<()> {
        self.layout_layers = serde_json::from_str(layers_json)
            .map_err(|error| PyValueError::new_err(format!("E_LAYOUT_JSON: {error}")))?;
        Ok(())
    }

    fn clear_layout_layers(&mut self) {
        self.layout_layers.clear();
    }

    fn inspect_layout<'py>(&self, py: Python<'py>, tick: u64) -> PyResult<Bound<'py, PyDict>> {
        let reader = self.complete()?;
        let frame = reader.read_tick(tick).map_err(|error| {
            PyOSError::new_err(format!("E_CACHE_READ {}: {error}", self.path.display()))
        })?;
        let composed =
            compose_layout_frame_v1(&frame, tick, &self.base_cache_hash, &self.layout_layers)
                .map_err(|error| PyValueError::new_err(format!("E_LAYOUT: {error}")))?;
        let out = PyDict::new(py);
        out.set_item("base_cache_hash", &self.base_cache_hash)?;
        out.set_item("active_layer_ids", composed.active_layer_ids)?;
        out.set_item(
            "conflicts",
            composed
                .conflicts
                .iter()
                .map(|conflict| {
                    format!(
                        "agent {} {}: {} -> {}",
                        conflict.agent_id,
                        conflict.channel,
                        conflict.earlier_layer_id,
                        conflict.later_layer_id
                    )
                })
                .collect::<Vec<_>>(),
        )?;
        out.set_item(
            "warnings",
            vec![
                "USD profile v1 exports point instancer identity, position, and variant only; animation, physics, guides, groups, and unresolved conflicts are not representable.".to_owned(),
            ],
        )?;
        Ok(out)
    }

    fn flatten_layout(&self, tick: u64, path: PathBuf) -> PyResult<()> {
        let reader = self.complete()?;
        let frame = reader.read_tick(tick).map_err(|error| {
            PyOSError::new_err(format!("E_CACHE_READ {}: {error}", self.path.display()))
        })?;
        let composed =
            compose_layout_frame_v1(&frame, tick, &self.base_cache_hash, &self.layout_layers)
                .map_err(|error| PyValueError::new_err(format!("E_LAYOUT: {error}")))?;
        let document = serde_json::json!({
            "schema_version": 1,
            "source_base_hash": self.base_cache_hash,
            "tick": tick,
            "composed": composed,
        });
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|error| PyValueError::new_err(format!("E_FLATTEN_JSON: {error}")))?;
        fs::write(&path, bytes).map_err(|error| {
            PyOSError::new_err(format!("E_FLATTEN_WRITE {}: {error}", path.display()))
        })
    }

    fn export_usda(&self, tick: u64, path: PathBuf) -> PyResult<()> {
        let reader = self.complete()?;
        let frame = reader.read_tick(tick).map_err(|error| {
            PyOSError::new_err(format!("E_CACHE_READ {}: {error}", self.path.display()))
        })?;
        let composed =
            compose_layout_frame_v1(&frame, tick, &self.base_cache_hash, &self.layout_layers)
                .map_err(|error| PyValueError::new_err(format!("E_LAYOUT: {error}")))?;
        let usda = write_usda_crowd_profile_v1(&composed.records, &self.base_cache_hash)
            .map_err(|error| PyValueError::new_err(format!("E_USD: {error}")))?;
        fs::write(&path, usda)
            .map_err(|error| PyOSError::new_err(format!("E_USD_WRITE {}: {error}", path.display())))
    }

    fn inspect_agent<'py>(
        &mut self,
        py: Python<'py>,
        agent_id: u64,
        tick: u64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let frame = self.complete()?.read_tick(tick).map_err(|error| {
            PyOSError::new_err(format!("E_CACHE_READ {}: {error}", self.path.display()))
        })?;
        let record = frame
            .records
            .iter()
            .find(|record| record.agent_id == agent_id)
            .ok_or_else(|| {
                PyValueError::new_err(format!("E_AGENT_NOT_FOUND: {agent_id} at tick {tick}"))
            })?;
        let override_composed = compose_frame(&frame, tick, &self.override_layers)
            .map_err(|error| PyValueError::new_err(format!("E_OVERRIDE: {error}")))?;
        let override_position = override_composed
            .records
            .iter()
            .find(|candidate| candidate.agent_id == agent_id)
            .expect("composition preserves base agent IDs")
            .position;
        let layout_composed = if self.layout_layers.is_empty() {
            None
        } else {
            Some(
                compose_layout_frame_v1(&frame, tick, &self.base_cache_hash, &self.layout_layers)
                    .map_err(|error| PyValueError::new_err(format!("E_LAYOUT: {error}")))?,
            )
        };
        let layout_record = layout_composed.as_ref().and_then(|composed| {
            composed
                .records
                .iter()
                .find(|candidate| candidate.agent_id == agent_id)
        });
        let position = layout_record
            .map(|composed| composed.position)
            .unwrap_or(override_position);
        let out = PyDict::new(py);
        out.set_item("agent_id", agent_id)?;
        out.set_item("tick", tick)?;
        out.set_item("position", position)?;
        out.set_item(
            "solved_velocity",
            layout_record.map(|composed| composed.velocity).unwrap_or([
                record.velocity[0],
                record.velocity[1],
                0.0,
            ]),
        )?;
        out.set_item(
            "destination_id",
            layout_record
                .map(|composed| composed.destination_id)
                .unwrap_or(record.destination_id),
        )?;
        out.set_item("behavior_state", record.behavior_state)?;
        out.set_item("decision_reason", record.decision_reason)?;
        out.set_item(
            "clip_id",
            layout_record
                .map(|composed| composed.clip_id)
                .unwrap_or(record.clip_id),
        )?;
        out.set_item(
            "clip_phase",
            layout_record
                .map(|composed| composed.phase)
                .unwrap_or(record.phase),
        )?;
        out.set_item(
            "playback_rate",
            layout_record
                .map(|composed| composed.playback_rate)
                .unwrap_or(record.playback_rate),
        )?;
        out.set_item(
            "visible",
            layout_record
                .map(|composed| composed.visible)
                .unwrap_or(record.visible),
        )?;
        out.set_item(
            "physics_active",
            layout_record.is_some_and(|composed| composed.physics_active),
        )?;

        let cache_hit = self.cached_behavior_query.as_ref().is_some_and(
            |(cached_agent, cached_tick, _trace)| *cached_agent == agent_id && *cached_tick == tick,
        );
        let cached_trace_json = if cache_hit {
            self.cached_behavior_query
                .as_ref()
                .and_then(|(_agent, _tick, trace)| trace.clone())
        } else {
            let behavior_events = self.complete()?.read_behavior_events().map_err(|error| {
                PyOSError::new_err(format!("E_CACHE_EVENTS {}: {error}", self.path.display()))
            })?;
            let selected = inspection_behavior_events(&behavior_events, agent_id, tick);
            let trace = if selected.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&selected).map_err(|error| {
                    PyOSError::new_err(format!("E_CACHE_EVENTS {}: {error}", self.path.display()))
                })?)
            };
            self.cached_behavior_query = Some((agent_id, tick, trace.clone()));
            trace
        };
        let legacy_evidence_path = self.path.join("debug/selected-agent.json");
        let legacy_trace_json = fs::read_to_string(&legacy_evidence_path)
            .ok()
            .filter(|text| {
                serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .and_then(|value| {
                        Some((
                            value.get("agent_id")?.as_u64()?,
                            value.get("tick")?.as_u64()?,
                        ))
                    })
                    == Some((agent_id, tick))
            });
        let decision_trace_json = if cached_trace_json.is_none() {
            legacy_trace_json
        } else {
            cached_trace_json
        };
        out.set_item("decision_trace_json", decision_trace_json)?;
        Ok(out)
    }
}

impl PyCache {
    fn complete(&self) -> PyResult<&CacheReader> {
        match &self.backing {
            CacheBacking::Complete(reader) => Ok(reader.as_ref()),
            CacheBacking::Inspection(report) => Err(PyOSError::new_err(format!(
                "E_CACHE_NOT_COMPLETE {}: {}",
                self.path.display(),
                cache_status_name(report.status)
            ))),
        }
    }
}

#[pyfunction(name = "compile_project")]
fn compile_project_py(project_json: &str) -> PyResult<PyCompiledProject> {
    let ir = serde_json::from_str(project_json)
        .map_err(|error| PyValueError::new_err(format!("E_PROJECT_JSON: {error}")))?;
    let compiled = compile_core_project(&ir)
        .map_err(|diagnostics| PyValueError::new_err(format_diagnostics(&diagnostics)))?;
    Ok(PyCompiledProject {
        inner: Arc::new(compiled),
    })
}

/// Compile a typed authoring graph without entering Blender or the simulator.
///
/// Keeping this as a coarse Rust call makes graph validation identical in the
/// node editor, headless validation, and eventual bake preflight.
#[pyfunction(name = "compile_behavior_graph")]
fn compile_behavior_graph_py<'py>(
    py: Python<'py>,
    graph_json: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let graph: BehaviorGraphV1 = serde_json::from_str(graph_json)
        .map_err(|error| PyValueError::new_err(format!("E_GRAPH_JSON: {error}")))?;
    let program = compile_graph(&graph).map_err(|diagnostics| {
        let message = diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "E_GRAPH_{:?} {}: {}",
                    diagnostic.code, diagnostic.node_id, diagnostic.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        PyValueError::new_err(message)
    })?;
    let out = PyDict::new(py);
    out.set_item("id", program.id())?;
    out.set_item("entry_index", program.entry_index())?;
    out.set_item("node_count", program.node_count())?;
    Ok(out)
}

#[pyfunction(name = "migrate_project_v1")]
fn migrate_project_v1_py(project_json: &str) -> PyResult<String> {
    let project = serde_json::from_str(project_json)
        .map_err(|error| PyValueError::new_err(format!("E_PROJECT_JSON: {error}")))?;
    serde_json::to_string(&migrate_project_v1(project))
        .map_err(|error| PyValueError::new_err(format!("E_PROJECT_V2_JSON: {error}")))
}

#[pyfunction(name = "compile_authorable_project")]
fn compile_authorable_project_py<'py>(
    py: Python<'py>,
    project_json: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let project: AuthorableProjectV2 = serde_json::from_str(project_json)
        .map_err(|error| PyValueError::new_err(format!("E_PROJECT_V2_JSON: {error}")))?;
    let compiled = compile_core_authorable_project(&project).map_err(|diagnostics| {
        PyValueError::new_err(
            diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "E_AUTHORING_{:?} {}: {}",
                        diagnostic.code, diagnostic.entity_id, diagnostic.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })?;
    let out = PyDict::new(py);
    out.set_item("agent_count", compiled.base().agent_spawns().len())?;
    out.set_item("base_source_hash", compiled.base().source_hash_hex())?;
    out.set_item("behavior_program_count", compiled.behavior_program_count())?;
    Ok(out)
}

/// Build an M2 project that can create a native authorable bake session.
#[pyfunction(name = "compile_authorable_runtime")]
fn compile_authorable_runtime_py(project_json: &str) -> PyResult<PyAuthorableProject> {
    let project: AuthorableProjectV2 = serde_json::from_str(project_json)
        .map_err(|error| PyValueError::new_err(format!("E_PROJECT_V2_JSON: {error}")))?;
    let base = compile_core_project(&project.base)
        .map_err(|diagnostics| PyValueError::new_err(format_diagnostics(&diagnostics)))?;
    let compiled = compile_core_authorable_project(&project).map_err(|diagnostics| {
        PyValueError::new_err(
            diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "E_AUTHORING_{:?} {}: {}",
                        diagnostic.code, diagnostic.entity_id, diagnostic.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })?;
    Ok(PyAuthorableProject {
        base: Arc::new(base),
        inner: Arc::new(compiled),
    })
}

/// Build a deterministic physics interval for a selected M4 layer. Blender
/// receives JSON cache samples only; it does not become a hidden simulator.
#[pyfunction(name = "simulate_physics_handoff")]
fn simulate_physics_handoff_py(spec_json: &str) -> PyResult<String> {
    let spec: PhysicsHandoffSpecV1 = serde_json::from_str(spec_json)
        .map_err(|error| PyValueError::new_err(format!("E_PHYSICS_SPEC_JSON: {error}")))?;
    let samples = crowd_cache::simulate_physics_handoff_v1(&spec)
        .map_err(|error| PyValueError::new_err(format!("E_PHYSICS_SPEC: {error}")))?;
    serde_json::to_string(&samples)
        .map_err(|error| PyValueError::new_err(format!("E_PHYSICS_SERIALIZE: {error}")))
}

fn validate_interaction_motion_attachment_json(
    layer_json: &str,
    motion_json: &str,
) -> Result<InteractionMotionV1, String> {
    let layer: AnimationLayerV1 = serde_json::from_str(layer_json)
        .map_err(|error| format!("interaction layer JSON is invalid: {error}"))?;
    layer
        .validate()
        .map_err(|error| format!("interaction layer is invalid: {error}"))?;
    let motion: InteractionMotionV1 = serde_json::from_str(motion_json)
        .map_err(|error| format!("interaction motion JSON is invalid: {error}"))?;
    if motion.contacts.is_empty() {
        return Err("interaction motion must contain validated contact evidence".to_owned());
    }

    let request = InteractionRequestV1 {
        schema_version: 1,
        request_id: layer.interaction_id.clone(),
        group_id: format!("attachment-{}", layer.layer_id),
        participants: layer
            .target_agent_ids
            .iter()
            .enumerate()
            .map(|(index, agent_id)| InteractionParticipantV1 {
                agent_id: *agent_id,
                role: format!("attachment-participant-{index}"),
                retarget_profile_id: "attachment-validated".to_owned(),
            })
            .collect(),
        tick_start: layer.tick_start,
        tick_end: layer.tick_end,
        seed: motion.provenance.seed,
        mode: InteractionModeV1::Strict,
        action: "attach-validated-paired-motion".to_owned(),
        outcome: "cache-bound-layer".to_owned(),
        root_constraints: motion
            .participants
            .iter()
            .map(|participant| RootConstraintV1 {
                agent_id: participant.agent_id,
                samples: participant
                    .root_samples
                    .iter()
                    .map(|sample| RootSampleV1 {
                        tick: sample.tick,
                        position: sample.translation,
                        yaw: sample.yaw,
                    })
                    .collect(),
            })
            .collect(),
        contact_constraints: motion
            .contacts
            .iter()
            .map(|contact| ContactConstraintV1 {
                contact_id: contact.contact_id.clone(),
                owner_agent_id: contact.owner_agent_id,
                other_agent_id: contact.other_agent_id,
                label: contact.label,
                tick_start: contact.tick,
                tick_end: contact.tick,
                required: true,
            })
            .collect(),
        provenance: InteractionProvenanceV1 {
            base_cache_hash: layer.base_cache_hash.clone(),
            graph_hash: "0".repeat(64),
            worker_protocol: layer.provenance.clone(),
        },
        budgets: InteractionBudgetsV1 {
            max_latency_ms: 1,
            max_memory_bytes: 1,
            max_output_bytes: 1,
        },
    };
    motion.validate_against(&request).map_err(|issues| {
        issues
            .into_iter()
            .map(|issue| issue.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    Ok(motion)
}

#[pyfunction(name = "validate_interaction_motion_attachment")]
fn validate_interaction_motion_attachment_py(
    layer_json: &str,
    motion_json: &str,
) -> PyResult<String> {
    let motion = validate_interaction_motion_attachment_json(layer_json, motion_json)
        .map_err(|error| PyValueError::new_err(format!("E_INTERACTION_MOTION: {error}")))?;
    serde_json::to_string(&motion)
        .map_err(|error| PyValueError::new_err(format!("E_INTERACTION_SERIALIZE: {error}")))
}

#[pyfunction(name = "resimulate_local_kinematic")]
fn resimulate_local_kinematic_py(request_json: &str) -> PyResult<String> {
    let request: LocalResimulationRequestV1 = serde_json::from_str(request_json)
        .map_err(|error| PyValueError::new_err(format!("E_RESIM_REQUEST_JSON: {error}")))?;
    let samples = crowd_cache::resimulate_local_kinematic_v1(&request)
        .map_err(|error| PyValueError::new_err(format!("E_RESIM_REQUEST: {error}")))?;
    serde_json::to_string(&samples)
        .map_err(|error| PyValueError::new_err(format!("E_RESIM_SERIALIZE: {error}")))
}

#[pyfunction]
fn inspect_cache<'py>(py: Python<'py>, path: PathBuf) -> PyResult<Bound<'py, PyDict>> {
    let report = RecoveryInspector::open(&path).map_err(|error| {
        PyOSError::new_err(format!("E_CACHE_INSPECT {}: {error}", path.display()))
    })?;
    // Recovery inspection intentionally stops at a readable prefix. A cache
    // that claims completion needs the stricter reader pass as well, otherwise
    // a corrupt final chunk or optional event file could be presented as safe.
    let integrity_error = if report.status == CacheStatus::Complete {
        CacheReader::open_complete(&path)
            .err()
            .map(|error| error.to_string())
    } else {
        None
    };
    let out = PyDict::new(py);
    out.set_item(
        "status",
        if integrity_error.is_some() {
            "corrupt"
        } else {
            cache_status_name(report.status)
        },
    )?;
    out.set_item("integrity_error", integrity_error)?;
    out.set_item("cancellation_reason", report.cancellation_reason)?;
    out.set_item("last_complete_tick", report.last_complete_tick)?;
    out.set_item("valid_chunk_count", report.valid_chunk_count)?;
    let readable_start = report
        .readable_tick_range
        .as_ref()
        .map(|range| *range.start());
    let readable_end = report
        .readable_tick_range
        .as_ref()
        .map(|range| *range.end());
    out.set_item("readable_tick_start", readable_start)?;
    out.set_item("readable_tick_end", readable_end)?;
    Ok(out)
}

fn facade_scene(
    project: &CoreCompiledProject,
    agent_count: u32,
) -> Result<crowd_core::CompiledScene, String> {
    let mut scene = compile_concourse(project).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "E_{:?} {}: {}",
                    diagnostic.code, diagnostic.entity_id, diagnostic.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let selected: BTreeSet<_> = project.agent_spawns()[..agent_count as usize]
        .iter()
        .map(|spawn| spawn.agent_id)
        .collect();
    for (spawn, specs) in scene.spawns.iter_mut().zip(&mut scene.agent_specs_by_spawn) {
        specs.retain(|spec| selected.contains(&spec.agent_id));
        spawn.count = specs.len() as u32;
    }
    Ok(scene)
}

fn cache_channel_defs() -> Vec<ChannelDef> {
    vec![
        channel("agent_id", ScalarType::U64, 1, None),
        channel("position", ScalarType::F32, 2, Some(0.0)),
        channel("orientation", ScalarType::F32, 1, None),
        channel("scale", ScalarType::F32, 1, None),
        channel("population_id", ScalarType::U32, 1, None),
        channel("variant_id", ScalarType::U32, 1, None),
        channel("clip_id", ScalarType::U16, 1, None),
        channel("phase", ScalarType::F32, 1, None),
        channel("playback_rate", ScalarType::F32, 1, None),
        channel("behavior_state", ScalarType::U16, 1, None),
        channel("decision_reason", ScalarType::U16, 1, None),
        channel("destination_id", ScalarType::U32, 1, None),
        channel("velocity", ScalarType::F32, 2, None),
        channel("visible", ScalarType::U8, 1, None),
        channel("render_tier", ScalarType::U8, 1, None),
    ]
}

fn channel(
    name: &str,
    scalar_type: ScalarType,
    arity: u8,
    quantization_error: Option<f32>,
) -> ChannelDef {
    ChannelDef {
        name: name.to_string(),
        scalar_type,
        arity,
        quantization_error,
    }
}

fn bake_outcome_dict<'py>(
    py: Python<'py>,
    path: &Path,
    outcome: BakeOutcome,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    out.set_item("path", path.display().to_string())?;
    out.set_item("status", cache_status_name(outcome.status))?;
    out.set_item("last_complete_tick", outcome.last_complete_tick)?;
    Ok(out)
}

fn cache_status_name(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Incomplete => "incomplete",
        CacheStatus::Canceled => "canceled",
        CacheStatus::Complete => "complete",
    }
}

fn behavior_event_kind(kind: crowd_core::BehaviorRuntimeEventKind) -> BehaviorEventKindV1 {
    match kind {
        crowd_core::BehaviorRuntimeEventKind::Decision => BehaviorEventKindV1::Decision,
        crowd_core::BehaviorRuntimeEventKind::QueueRequested => BehaviorEventKindV1::QueueRequested,
        crowd_core::BehaviorRuntimeEventKind::QueueAdmitted => BehaviorEventKindV1::QueueAdmitted,
        crowd_core::BehaviorRuntimeEventKind::QueueReleased => BehaviorEventKindV1::QueueReleased,
        crowd_core::BehaviorRuntimeEventKind::GroupSplit => BehaviorEventKindV1::GroupSplit,
        crowd_core::BehaviorRuntimeEventKind::GroupRegrouped => BehaviorEventKindV1::GroupRegrouped,
        crowd_core::BehaviorRuntimeEventKind::ActivityGranted => {
            BehaviorEventKindV1::ActivityGranted
        }
        crowd_core::BehaviorRuntimeEventKind::ActivityWaiting => {
            BehaviorEventKindV1::ActivityWaiting
        }
        crowd_core::BehaviorRuntimeEventKind::ActivityReleased => {
            BehaviorEventKindV1::ActivityReleased
        }
        crowd_core::BehaviorRuntimeEventKind::ActivityFailed => BehaviorEventKindV1::ActivityFailed,
    }
}

fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "E_{:?} {}: {}",
                diagnostic.code, diagnostic.entity_id, diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Default, PartialEq, Debug)]
struct PackedCacheChannels {
    position: Vec<u8>,
    orientation: Vec<u8>,
    scale: Vec<u8>,
    agent_id_lo: Vec<u8>,
    agent_id_hi: Vec<u8>,
    population_id: Vec<u8>,
    archetype_id: Vec<u8>,
    variant_id: Vec<u8>,
    spawn_ordinal: Vec<u8>,
    clip_id: Vec<u8>,
    phase: Vec<u8>,
    playback_rate: Vec<u8>,
    behavior_state: Vec<u8>,
    decision_reason: Vec<u8>,
    destination_id: Vec<u8>,
    velocity: Vec<u8>,
    visible: Vec<u8>,
    render_tier: Vec<u8>,
}

#[derive(Default, PartialEq, Debug)]
struct PackedAgentStaticChannels {
    agent_id_lo: Vec<u8>,
    agent_id_hi: Vec<u8>,
    population_id: Vec<u8>,
    archetype_id: Vec<u8>,
    variant_id: Vec<u8>,
    base_scale: Vec<u8>,
    spawn_ordinal: Vec<u8>,
}

fn pack_agent_static(agents: &[AgentStatic]) -> PackedAgentStaticChannels {
    let n = agents.len();
    let mut out = PackedAgentStaticChannels {
        agent_id_lo: Vec::with_capacity(n * 4),
        agent_id_hi: Vec::with_capacity(n * 4),
        population_id: Vec::with_capacity(n * 4),
        archetype_id: Vec::with_capacity(n * 4),
        variant_id: Vec::with_capacity(n * 4),
        base_scale: Vec::with_capacity(n * 4),
        spawn_ordinal: Vec::with_capacity(n * 4),
    };
    for agent in agents {
        push_u32(&mut out.agent_id_lo, agent.agent_id as u32);
        push_u32(&mut out.agent_id_hi, (agent.agent_id >> 32) as u32);
        push_u32(&mut out.population_id, agent.population_id);
        push_u32(&mut out.archetype_id, agent.archetype_id);
        push_u32(&mut out.variant_id, agent.variant_id);
        push_f32(&mut out.base_scale, agent.base_scale);
        push_u32(&mut out.spawn_ordinal, agent.spawn_ordinal);
    }
    out
}

fn pack_cache(records: &[FrameRecord], agents: &[AgentStatic]) -> PackedCacheChannels {
    debug_assert_eq!(records.len(), agents.len());
    let n = records.len();
    let mut out = PackedCacheChannels {
        position: Vec::with_capacity(n * 12),
        orientation: Vec::with_capacity(n * 4),
        scale: Vec::with_capacity(n * 4),
        agent_id_lo: Vec::with_capacity(n * 4),
        agent_id_hi: Vec::with_capacity(n * 4),
        population_id: Vec::with_capacity(n * 4),
        archetype_id: Vec::with_capacity(n * 4),
        variant_id: Vec::with_capacity(n * 4),
        spawn_ordinal: Vec::with_capacity(n * 4),
        clip_id: Vec::with_capacity(n * 4),
        phase: Vec::with_capacity(n * 4),
        playback_rate: Vec::with_capacity(n * 4),
        behavior_state: Vec::with_capacity(n * 4),
        decision_reason: Vec::with_capacity(n * 4),
        destination_id: Vec::with_capacity(n * 4),
        velocity: Vec::with_capacity(n * 12),
        visible: Vec::with_capacity(n * 4),
        render_tier: Vec::with_capacity(n * 4),
    };
    for (record, agent) in records.iter().zip(agents) {
        push_f32x3(
            &mut out.position,
            record.position[0],
            record.position[1],
            0.0,
        );
        push_f32(&mut out.orientation, record.orientation);
        push_f32(&mut out.scale, record.scale);
        push_u32(&mut out.agent_id_lo, record.agent_id as u32);
        push_u32(&mut out.agent_id_hi, (record.agent_id >> 32) as u32);
        push_u32(&mut out.population_id, record.population_id);
        push_u32(&mut out.archetype_id, agent.archetype_id);
        push_u32(&mut out.variant_id, record.variant_id);
        push_u32(&mut out.spawn_ordinal, agent.spawn_ordinal);
        push_u32(&mut out.clip_id, u32::from(record.clip_id));
        push_f32(&mut out.phase, record.phase);
        push_f32(&mut out.playback_rate, record.playback_rate);
        push_u32(&mut out.behavior_state, u32::from(record.behavior_state));
        push_u32(&mut out.decision_reason, u32::from(record.decision_reason));
        push_u32(&mut out.destination_id, record.destination_id);
        push_f32x3(
            &mut out.velocity,
            record.velocity[0],
            record.velocity[1],
            0.0,
        );
        push_u32(&mut out.visible, u32::from(record.visible));
        push_u32(&mut out.render_tier, u32::from(record.render_tier));
    }
    out
}

/// Replace only presentation channels with a composed M4 layer frame.  The
/// source `Frame` and the cache reader remain immutable, so a viewport scrub or
/// USD export can never turn a directed result into a rebake.
fn apply_layout_records(packed: &mut PackedCacheChannels, records: &[crowd_cache::LayoutRecordV1]) {
    packed.position.clear();
    packed.velocity.clear();
    packed.variant_id.clear();
    packed.clip_id.clear();
    packed.phase.clear();
    packed.playback_rate.clear();
    packed.destination_id.clear();
    packed.visible.clear();
    packed.render_tier.clear();
    for record in records {
        push_f32x3(
            &mut packed.position,
            record.position[0],
            record.position[1],
            record.position[2],
        );
        push_f32x3(
            &mut packed.velocity,
            record.velocity[0],
            record.velocity[1],
            record.velocity[2],
        );
        push_u32(&mut packed.variant_id, record.variant_id);
        push_u32(&mut packed.clip_id, u32::from(record.clip_id));
        push_f32(&mut packed.phase, record.phase);
        push_f32(&mut packed.playback_rate, record.playback_rate);
        push_u32(&mut packed.destination_id, record.destination_id);
        push_u32(&mut packed.visible, u32::from(record.visible));
        push_u32(&mut packed.render_tier, u32::from(record.render_tier));
    }
}

fn packed_cache_dict<'py>(
    py: Python<'py>,
    packed: &PackedCacheChannels,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (name, bytes) in [
        ("position", &packed.position),
        ("orientation", &packed.orientation),
        ("scale", &packed.scale),
        ("agent_id_lo", &packed.agent_id_lo),
        ("agent_id_hi", &packed.agent_id_hi),
        ("population_id", &packed.population_id),
        ("archetype_id", &packed.archetype_id),
        ("variant_id", &packed.variant_id),
        ("spawn_ordinal", &packed.spawn_ordinal),
        ("clip_id", &packed.clip_id),
        ("phase", &packed.phase),
        ("playback_rate", &packed.playback_rate),
        ("behavior_state", &packed.behavior_state),
        ("decision_reason", &packed.decision_reason),
        ("destination_id", &packed.destination_id),
        ("velocity", &packed.velocity),
        ("visible", &packed.visible),
        ("render_tier", &packed.render_tier),
    ] {
        out.set_item(name, PyBytes::new(py, bytes))?;
    }
    Ok(out)
}

fn packed_agent_static_dict<'py>(
    py: Python<'py>,
    packed: &PackedAgentStaticChannels,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (name, bytes) in [
        ("agent_id_lo", &packed.agent_id_lo),
        ("agent_id_hi", &packed.agent_id_hi),
        ("population_id", &packed.population_id),
        ("archetype_id", &packed.archetype_id),
        ("variant_id", &packed.variant_id),
        ("base_scale", &packed.base_scale),
        ("spawn_ordinal", &packed.spawn_ordinal),
    ] {
        out.set_item(name, PyBytes::new(py, bytes))?;
    }
    Ok(out)
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_f32(target: &mut Vec<u8>, value: f32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_f32x3(target: &mut Vec<u8>, x: f32, y: f32, z: f32) {
    push_f32(target, x);
    push_f32(target, y);
    push_f32(target, z);
}

/// A read-only handle to a trace v0 file.
#[pyclass]
struct Trace {
    reader: TraceReader,
    // Reused across `read_tick` calls: scrubbing the Blender timeline calls
    // this once per frame, and a fresh allocation per frame is a cost with
    // no upside.
    scratch: Vec<AgentRecord>,
}

#[pymethods]
impl Trace {
    #[new]
    fn new(path: PathBuf) -> PyResult<Self> {
        let reader = TraceReader::open(&path)
            .map_err(|e| PyOSError::new_err(format!("{}: {e}", path.display())))?;
        Ok(Self {
            reader,
            scratch: Vec::new(),
        })
    }

    #[getter]
    fn tick_count(&self) -> u64 {
        self.reader.header().tick_count
    }

    #[getter]
    fn agent_count(&self) -> u32 {
        self.reader.header().agent_count
    }

    #[getter]
    fn ticks_per_second(&self) -> u32 {
        self.reader.header().ticks_per_second
    }

    #[getter]
    fn world_to_meter(&self) -> f32 {
        self.reader.header().world_to_meter
    }

    /// Read one tick as flat per-channel byte buffers.
    ///
    /// Buffers are shaped for `numpy.frombuffer` followed by `foreach_set`,
    /// which is the only path into Blender point attributes that avoids a
    /// per-element Python round trip. Every integer channel is widened or
    /// split to 32 bits because that is the only integer width a Blender
    /// point attribute has.
    ///
    /// `agent_id_lo`/`agent_id_hi` carry the *unsigned* 32-bit halves of the
    /// 64-bit stable agent ID. Blender's INT point attribute is signed, so
    /// numpy and Blender both reinterpret each half as a signed `i32` the
    /// moment it crosses `foreach_set` — the bit pattern survives, the
    /// printed number does not. A consumer MUST mask both halves back to
    /// unsigned before recombining them:
    /// `(lo & 0xFFFFFFFF) | ((hi & 0xFFFFFFFF) << 32)`. Skipping the mask
    /// silently corrupts roughly half of all IDs (any ID whose high word has
    /// its top bit set becomes sign-extended before the shift) and the
    /// corrupted result is still a plausible, still-unique-looking 64-bit
    /// number — nothing about it fails loudly, it is just the wrong agent.
    ///
    /// `tick` is `i64`, not `u64`, purely so a negative tick can be rejected
    /// as a normal `OSError` instead of pyo3's `OverflowError` on the u64
    /// conversion: a Blender frame number minus `frame_start` lands negative
    /// the moment an artist scrubs before the start, and an addon that
    /// catches only `OSError` (per this module's contract) must not crash on
    /// that.
    fn read_tick<'py>(&mut self, py: Python<'py>, tick: i64) -> PyResult<Bound<'py, PyDict>> {
        let tick = u64::try_from(tick)
            .map_err(|_| PyOSError::new_err(format!("tick {tick} is negative")))?;
        self.reader
            .read_tick(tick, &mut self.scratch)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;

        let packed = pack(&self.scratch);

        let out = PyDict::new(py);
        out.set_item("position", PyBytes::new(py, &packed.position))?;
        out.set_item("orientation", PyBytes::new(py, &packed.orientation))?;
        out.set_item("agent_id_lo", PyBytes::new(py, &packed.agent_id_lo))?;
        out.set_item("agent_id_hi", PyBytes::new(py, &packed.agent_id_hi))?;
        out.set_item("flags", PyBytes::new(py, &packed.flags))?;
        out.set_item("clip_index", PyBytes::new(py, &packed.clip_index))?;
        out.set_item("phase", PyBytes::new(py, &packed.phase))?;
        out.set_item("playback_rate", PyBytes::new(py, &packed.playback_rate))?;
        out.set_item("render_tier", PyBytes::new(py, &packed.render_tier))?;
        Ok(out)
    }
}

/// The nine flat per-channel byte buffers `read_tick` returns, before they
/// cross into Python. A plain struct of `Vec<u8>` so the packing loop below
/// can be unit-tested without pyo3, a `Trace`, or a file on disk.
#[derive(Default, PartialEq, Debug)]
struct PackedChannels {
    position: Vec<u8>,
    orientation: Vec<u8>,
    agent_id_lo: Vec<u8>,
    agent_id_hi: Vec<u8>,
    flags: Vec<u8>,
    clip_index: Vec<u8>,
    phase: Vec<u8>,
    playback_rate: Vec<u8>,
    render_tier: Vec<u8>,
}

/// Pack `records` into the nine little-endian channel buffers `read_tick`
/// returns. Pure Rust, no pyo3, so this — the crate's only real logic — can
/// be tested directly instead of only through a hand-run wheel script.
fn pack(records: &[AgentRecord]) -> PackedChannels {
    let n = records.len();
    let mut out = PackedChannels {
        // `position` is 3 floats per agent: Blender's `position` attribute is
        // FLOAT_VECTOR, and the simulation is planar, so z is always 0.
        position: Vec::with_capacity(n * 12),
        orientation: Vec::with_capacity(n * 4),
        agent_id_lo: Vec::with_capacity(n * 4),
        agent_id_hi: Vec::with_capacity(n * 4),
        flags: Vec::with_capacity(n * 4),
        clip_index: Vec::with_capacity(n * 4),
        phase: Vec::with_capacity(n * 4),
        playback_rate: Vec::with_capacity(n * 4),
        render_tier: Vec::with_capacity(n * 4),
    };

    for r in records {
        out.position.extend_from_slice(&r.position[0].to_le_bytes());
        out.position.extend_from_slice(&r.position[1].to_le_bytes());
        out.position.extend_from_slice(&0.0f32.to_le_bytes());
        out.orientation
            .extend_from_slice(&r.orientation.to_le_bytes());
        // Split rather than narrow: a truncated stable ID is not stable.
        out.agent_id_lo
            .extend_from_slice(&(r.agent_id as u32).to_le_bytes());
        out.agent_id_hi
            .extend_from_slice(&((r.agent_id >> 32) as u32).to_le_bytes());
        out.flags.extend_from_slice(&r.flags.to_le_bytes());
        out.clip_index
            .extend_from_slice(&u32::from(r.clip_index).to_le_bytes());
        out.phase.extend_from_slice(&r.phase.to_le_bytes());
        out.playback_rate
            .extend_from_slice(&r.playback_rate.to_le_bytes());
        out.render_tier
            .extend_from_slice(&u32::from(r.render_tier).to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn checked_interaction_attachment() -> (String, String) {
        (
            include_str!("../../../assets/reference/m6/interaction-animation-layer-v1.json")
                .to_owned(),
            include_str!("../../../assets/reference/m6/interaction-motion-v1.json").to_owned(),
        )
    }

    #[test]
    fn native_attachment_accepts_interval_roots_contacts_and_provenance() {
        let (layer, motion) = checked_interaction_attachment();
        let validated = validate_interaction_motion_attachment_json(&layer, &motion)
            .expect("checked paired motion must pass the Rust authority");

        assert_eq!(validated.request_id, "request-pair-7-9");
        assert_eq!(validated.participants.len(), 2);
        assert_eq!(validated.contacts.len(), 1);
        assert_eq!(validated.provenance.backend, "authored-paired-clip");
    }

    #[test]
    fn native_attachment_rejects_motion_without_complete_root_samples() {
        let (layer, motion) = checked_interaction_attachment();
        let mut motion: serde_json::Value = serde_json::from_str(&motion).unwrap();
        motion["participants"][0]["root_samples"] = serde_json::json!([]);

        let error = validate_interaction_motion_attachment_json(&layer, &motion.to_string())
            .expect_err("empty roots must not reach native layout lowering");
        assert!(
            error.contains("root"),
            "unexpected validation error: {error}"
        );
    }

    #[test]
    fn native_attachment_rejects_invalid_contact_and_provenance() {
        let (layer, motion) = checked_interaction_attachment();
        let mut bad_contact: serde_json::Value = serde_json::from_str(&motion).unwrap();
        bad_contact["contacts"][0]["distance_m"] = serde_json::json!(-1.0);
        let error = validate_interaction_motion_attachment_json(&layer, &bad_contact.to_string())
            .expect_err("invalid contact must not reach native layout lowering");
        assert!(
            error.contains("contact"),
            "unexpected contact validation error: {error}"
        );

        let mut bad_provenance: serde_json::Value = serde_json::from_str(&motion).unwrap();
        bad_provenance["provenance"]["backend"] = serde_json::json!("");
        let error =
            validate_interaction_motion_attachment_json(&layer, &bad_provenance.to_string())
                .expect_err("invalid provenance must not reach native layout lowering");
        assert!(
            error.contains("provenance"),
            "unexpected provenance validation error: {error}"
        );
    }

    #[test]
    fn only_declared_fidelity_profiles_are_accepted() {
        assert!(parse_fidelity_profile(None).unwrap().is_none());
        let declared = parse_fidelity_profile(Some("m5_background_10_90"))
            .unwrap()
            .expect("the declared M5 mix must resolve to a policy");
        assert_eq!(
            declared.background_permyriad,
            FidelityPolicy::m5_10k_profile().background_permyriad
        );
        // An arbitrary share would make a cache's declared profile
        // unverifiable, so an unknown name is an error rather than a default.
        // Only `is_err` is asserted: rendering the message would fetch the
        // Python exception value, and this unit test runs with no interpreter.
        assert!(parse_fidelity_profile(Some("90_percent_background")).is_err());
    }

    fn record_with_id(agent_id: u64) -> AgentRecord {
        AgentRecord {
            agent_id,
            position: [0.0, 0.0],
            orientation: 0.0,
            flags: 0,
            clip_index: 0,
            phase: 0.0,
            playback_rate: 0.0,
            render_tier: 0,
        }
    }

    fn reassemble(lo: &[u8], hi: &[u8], index: usize) -> u64 {
        let lo = u32::from_le_bytes(lo[index * 4..index * 4 + 4].try_into().unwrap());
        let hi = u32::from_le_bytes(hi[index * 4..index * 4 + 4].try_into().unwrap());
        (u64::from(lo)) | (u64::from(hi) << 32)
    }

    #[test]
    fn inspection_uses_the_latest_cached_event_at_or_before_the_selected_tick() {
        let events = vec![
            BehaviorEventV1::decision(0, 7, "leave", "join_queue", "population:0"),
            BehaviorEventV1::new(0, 7, BehaviorEventKindV1::QueueRequested, "east_queue"),
            BehaviorEventV1::decision(3, 7, "leave", "visit_kiosk", "population:0"),
            BehaviorEventV1::decision(6, 8, "leave", "join_queue", "population:0"),
        ];

        let selected = inspection_behavior_events(&events, 7, 5);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].tick, 3);
        assert_eq!(selected[0].decisive_node.as_deref(), Some("visit_kiosk"));
    }

    #[test]
    fn id_splits_reassemble_across_full_range() {
        let ids = [
            u64::MAX,
            1u64 << 63,
            0xFFFF_FFFFu64,
            0x1_0000_0000u64,
            // High word's top bit set: this is the case that goes wrong
            // without masking, because Blender/numpy read the u32 bit
            // pattern back as a signed i32.
            0x8000_0001_0000_0002u64,
        ];
        let records: Vec<AgentRecord> = ids.iter().copied().map(record_with_id).collect();
        let packed = pack(&records);

        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(
                reassemble(&packed.agent_id_lo, &packed.agent_id_hi, i),
                id,
                "agent {i} did not reassemble to {id:#x}"
            );
        }
    }

    #[test]
    fn position_is_three_floats_with_zero_z() {
        let records = vec![AgentRecord {
            position: [1.5, -2.5],
            ..record_with_id(1)
        }];
        let packed = pack(&records);

        assert_eq!(packed.position.len(), 12);
        let x = f32::from_le_bytes(packed.position[0..4].try_into().unwrap());
        let y = f32::from_le_bytes(packed.position[4..8].try_into().unwrap());
        let z = f32::from_le_bytes(packed.position[8..12].try_into().unwrap());
        assert_eq!((x, y, z), (1.5, -2.5, 0.0));
    }

    #[test]
    fn authorable_bake_persists_live_graph_evidence_in_the_cache() {
        let base = serde_json::from_str(include_str!(
            "../../../assets/reference/concourse-project-v1.json"
        ))
        .unwrap();
        let mut project = migrate_project_v1(base);
        project.behavior_graphs = vec![serde_json::from_str(include_str!(
            "../../../addon/blender_crowd/reference/leave-concourse-v1.json"
        ))
        .unwrap()];
        project.semantics = serde_json::from_str(include_str!(
            "../../../addon/blender_crowd/reference/concourse-authoring-v2.json"
        ))
        .unwrap();
        for assignment in &mut project.population_behaviors {
            assignment.graph_id = "leave_concourse".to_string();
        }
        let base = Arc::new(compile_core_project(&project.base).unwrap());
        let authorable = Arc::new(compile_core_authorable_project(&project).unwrap());
        let mut session = Session::create_authorable(base, authorable, 1).unwrap();
        let temp = tempdir().unwrap();
        let cache_path = temp.path().join("authorable-cache");

        let result = session
            .bake_native(&cache_path, 2, &crowd_cache::CancelToken::new())
            .unwrap();
        assert_eq!(result.status, CacheStatus::Complete);
        let reader = CacheReader::open_complete(&cache_path).unwrap();
        assert!(reader.manifest().behavior_events.is_some());
        assert!(reader
            .read_behavior_events()
            .unwrap()
            .iter()
            .any(|event| event.kind == BehaviorEventKindV1::Decision));
    }

    #[test]
    fn multi_agent_offsets_do_not_collapse_to_agent_zero() {
        let records = vec![
            AgentRecord {
                agent_id: 10,
                position: [1.0, 1.0],
                orientation: 0.1,
                flags: 1,
                clip_index: 1,
                phase: 0.1,
                playback_rate: 1.0,
                render_tier: 1,
            },
            AgentRecord {
                agent_id: 20,
                position: [2.0, 2.0],
                orientation: 0.2,
                flags: 2,
                clip_index: 2,
                phase: 0.2,
                playback_rate: 2.0,
                render_tier: 2,
            },
            AgentRecord {
                agent_id: 30,
                position: [3.0, 3.0],
                orientation: 0.3,
                flags: 3,
                clip_index: 3,
                phase: 0.3,
                playback_rate: 3.0,
                render_tier: 3,
            },
        ];
        let packed = pack(&records);

        for (i, r) in records.iter().enumerate() {
            assert_eq!(
                reassemble(&packed.agent_id_lo, &packed.agent_id_hi, i),
                r.agent_id,
                "agent {i}"
            );
            let x = f32::from_le_bytes(packed.position[i * 12..i * 12 + 4].try_into().unwrap());
            let y = f32::from_le_bytes(packed.position[i * 12 + 4..i * 12 + 8].try_into().unwrap());
            assert_eq!((x, y), (r.position[0], r.position[1]), "agent {i} position");
            let orientation =
                f32::from_le_bytes(packed.orientation[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(orientation, r.orientation, "agent {i} orientation");
            let flags = u32::from_le_bytes(packed.flags[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(flags, r.flags, "agent {i} flags");
            let clip_index =
                u32::from_le_bytes(packed.clip_index[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(clip_index, u32::from(r.clip_index), "agent {i} clip_index");
            let phase = f32::from_le_bytes(packed.phase[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(phase, r.phase, "agent {i} phase");
            let playback_rate =
                f32::from_le_bytes(packed.playback_rate[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(playback_rate, r.playback_rate, "agent {i} playback_rate");
            let render_tier =
                u32::from_le_bytes(packed.render_tier[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(
                render_tier,
                u32::from(r.render_tier),
                "agent {i} render_tier"
            );
        }
    }

    #[test]
    fn channel_byte_lengths_scale_with_agent_count() {
        let records: Vec<AgentRecord> = (0..7).map(|i| record_with_id(i as u64)).collect();
        let packed = pack(&records);
        let n = records.len();

        assert_eq!(packed.position.len(), n * 12);
        assert_eq!(packed.orientation.len(), n * 4);
        assert_eq!(packed.agent_id_lo.len(), n * 4);
        assert_eq!(packed.agent_id_hi.len(), n * 4);
        assert_eq!(packed.flags.len(), n * 4);
        assert_eq!(packed.clip_index.len(), n * 4);
        assert_eq!(packed.phase.len(), n * 4);
        assert_eq!(packed.playback_rate.len(), n * 4);
        assert_eq!(packed.render_tier.len(), n * 4);
    }

    #[test]
    fn cache_channels_pack_three_agents_without_losing_values() {
        let records: Vec<FrameRecord> = (0..3u32)
            .map(|index| FrameRecord {
                agent_id: 0x8000_0001_0000_0002 + u64::from(index),
                position: [index as f32 + 1.25, index as f32 - 2.5],
                orientation: index as f32 * 0.1,
                scale: 0.9 + index as f32 * 0.05,
                population_id: 10 + index,
                variant_id: 20 + index,
                clip_id: 30 + index as u16,
                phase: 0.2 + index as f32 * 0.1,
                playback_rate: 0.8 + index as f32 * 0.1,
                behavior_state: 40 + index as u16,
                decision_reason: 50 + index as u16,
                destination_id: 60 + index,
                velocity: [index as f32 + 0.5, index as f32 - 0.25],
                visible: index != 1,
                render_tier: index as u8,
            })
            .collect();
        let agents: Vec<AgentStatic> = records
            .iter()
            .enumerate()
            .map(|(index, record)| AgentStatic {
                agent_id: record.agent_id,
                population_id: record.population_id,
                archetype_id: 70 + index as u32,
                variant_id: record.variant_id,
                base_scale: record.scale,
                spawn_ordinal: 80 + index as u32,
            })
            .collect();
        let packed = pack_cache(&records, &agents);
        let n = records.len();

        assert_eq!(packed.position.len(), n * 12);
        assert_eq!(packed.velocity.len(), n * 12);
        for bytes in [
            &packed.orientation,
            &packed.scale,
            &packed.agent_id_lo,
            &packed.agent_id_hi,
            &packed.population_id,
            &packed.archetype_id,
            &packed.variant_id,
            &packed.spawn_ordinal,
            &packed.clip_id,
            &packed.phase,
            &packed.playback_rate,
            &packed.behavior_state,
            &packed.decision_reason,
            &packed.destination_id,
            &packed.visible,
            &packed.render_tier,
        ] {
            assert_eq!(bytes.len(), n * 4);
        }
        for (index, record) in records.iter().enumerate() {
            assert_eq!(
                reassemble(&packed.agent_id_lo, &packed.agent_id_hi, index),
                record.agent_id
            );
            assert_eq!(
                f32::from_le_bytes(
                    packed.position[index * 12..index * 12 + 4]
                        .try_into()
                        .unwrap()
                ),
                record.position[0]
            );
            assert_eq!(
                u32::from_le_bytes(
                    packed.archetype_id[index * 4..index * 4 + 4]
                        .try_into()
                        .unwrap()
                ),
                agents[index].archetype_id
            );
            assert_eq!(
                u32::from_le_bytes(packed.visible[index * 4..index * 4 + 4].try_into().unwrap()),
                u32::from(record.visible)
            );
        }
    }

    #[test]
    fn static_agent_table_packs_ids_and_authored_choices_once() {
        let agents = vec![
            AgentStatic {
                agent_id: 0x8000_0001_0000_0002,
                population_id: 11,
                archetype_id: 12,
                variant_id: 13,
                base_scale: 0.95,
                spawn_ordinal: 14,
            },
            AgentStatic {
                agent_id: u64::MAX,
                population_id: 21,
                archetype_id: 22,
                variant_id: 23,
                base_scale: 1.05,
                spawn_ordinal: 24,
            },
        ];
        let packed = pack_agent_static(&agents);

        for (index, agent) in agents.iter().enumerate() {
            assert_eq!(
                reassemble(&packed.agent_id_lo, &packed.agent_id_hi, index),
                agent.agent_id
            );
            assert_eq!(
                u32::from_le_bytes(
                    packed.population_id[index * 4..index * 4 + 4]
                        .try_into()
                        .unwrap()
                ),
                agent.population_id
            );
            assert_eq!(
                f32::from_le_bytes(
                    packed.base_scale[index * 4..index * 4 + 4]
                        .try_into()
                        .unwrap()
                ),
                agent.base_scale
            );
        }
    }
}

#[pymodule]
fn blender_crowd_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<Trace>()?;
    m.add_class::<PyCompiledProject>()?;
    m.add_class::<PyAuthorableProject>()?;
    m.add_class::<Session>()?;
    m.add_class::<PyCancelToken>()?;
    m.add_class::<PyCache>()?;
    m.add_function(wrap_pyfunction!(compile_project_py, m)?)?;
    m.add_function(wrap_pyfunction!(compile_behavior_graph_py, m)?)?;
    m.add_function(wrap_pyfunction!(migrate_project_v1_py, m)?)?;
    m.add_function(wrap_pyfunction!(compile_authorable_project_py, m)?)?;
    m.add_function(wrap_pyfunction!(compile_authorable_runtime_py, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_physics_handoff_py, m)?)?;
    m.add_function(wrap_pyfunction!(
        validate_interaction_motion_attachment_py,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(resimulate_local_kinematic_py, m)?)?;
    m.add_function(wrap_pyfunction!(inspect_cache, m)?)?;
    Ok(())
}
