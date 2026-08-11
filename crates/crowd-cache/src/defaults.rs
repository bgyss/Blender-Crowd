//! Defaults selected by the checked M0 cache experiment.

use crate::PositionEncoding;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheDefaults {
    pub chunk_ticks: u32,
    pub position_encoding: PositionEncoding,
}

// This literal is updated only from the checked cache experiment report. The
// report-selection test prevents it from drifting from measured evidence.
pub const CACHE_V1_DEFAULTS: CacheDefaults = CacheDefaults {
    chunk_ticks: 120,
    position_encoding: PositionEncoding::AffineI16,
};
