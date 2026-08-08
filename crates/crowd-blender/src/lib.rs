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

use std::path::PathBuf;

use crowd_trace::{AgentRecord, TraceReader};
use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

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
    fn read_tick<'py>(&mut self, py: Python<'py>, tick: u64) -> PyResult<Bound<'py, PyDict>> {
        self.reader
            .read_tick(tick, &mut self.scratch)
            .map_err(|e| PyOSError::new_err(format!("{e}")))?;

        let n = self.scratch.len();
        // `position` is 3 floats per agent: Blender's `position` attribute is
        // FLOAT_VECTOR, and the simulation is planar, so z is always 0.
        let mut position = Vec::with_capacity(n * 12);
        let mut orientation = Vec::with_capacity(n * 4);
        let mut agent_id_lo = Vec::with_capacity(n * 4);
        let mut agent_id_hi = Vec::with_capacity(n * 4);
        let mut flags = Vec::with_capacity(n * 4);
        let mut clip_index = Vec::with_capacity(n * 4);
        let mut phase = Vec::with_capacity(n * 4);
        let mut playback_rate = Vec::with_capacity(n * 4);
        let mut render_tier = Vec::with_capacity(n * 4);

        for r in &self.scratch {
            position.extend_from_slice(&r.position[0].to_le_bytes());
            position.extend_from_slice(&r.position[1].to_le_bytes());
            position.extend_from_slice(&0.0f32.to_le_bytes());
            orientation.extend_from_slice(&r.orientation.to_le_bytes());
            // Split rather than narrow: a truncated stable ID is not stable.
            agent_id_lo.extend_from_slice(&(r.agent_id as u32).to_le_bytes());
            agent_id_hi.extend_from_slice(&((r.agent_id >> 32) as u32).to_le_bytes());
            flags.extend_from_slice(&r.flags.to_le_bytes());
            clip_index.extend_from_slice(&u32::from(r.clip_index).to_le_bytes());
            phase.extend_from_slice(&r.phase.to_le_bytes());
            playback_rate.extend_from_slice(&r.playback_rate.to_le_bytes());
            render_tier.extend_from_slice(&u32::from(r.render_tier).to_le_bytes());
        }

        let out = PyDict::new(py);
        out.set_item("position", PyBytes::new(py, &position))?;
        out.set_item("orientation", PyBytes::new(py, &orientation))?;
        out.set_item("agent_id_lo", PyBytes::new(py, &agent_id_lo))?;
        out.set_item("agent_id_hi", PyBytes::new(py, &agent_id_hi))?;
        out.set_item("flags", PyBytes::new(py, &flags))?;
        out.set_item("clip_index", PyBytes::new(py, &clip_index))?;
        out.set_item("phase", PyBytes::new(py, &phase))?;
        out.set_item("playback_rate", PyBytes::new(py, &playback_rate))?;
        out.set_item("render_tier", PyBytes::new(py, &render_tier))?;
        Ok(out)
    }
}

#[pymodule]
fn blender_crowd_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<Trace>()?;
    Ok(())
}
