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
}

#[pymodule]
fn blender_crowd_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<Trace>()?;
    Ok(())
}
