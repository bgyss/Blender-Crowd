//! Pooled per-tick neighbor storage.
//!
//! Contract section 5.2 requires the hot loop not to allocate. Buffers are
//! cleared and refilled each tick rather than freed, so steady-state ticks do
//! no allocator work at all.

/// One observed neighbor: its slot and squared distance at observation time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbor {
    pub slot: u32,
    pub dist_sq: f32,
}

/// Flat per-agent neighbor lists with `(start, len)` indexing.
#[derive(Clone, Debug, Default)]
pub struct NeighborArena {
    entries: Vec<Neighbor>,
    start: Vec<u32>,
    len: Vec<u32>,
    /// Whether this slot's neighbours were actually looked up this tick.
    ///
    /// An empty list is ambiguous on its own: it means either "queried, and
    /// genuinely alone" or "not due this tick, so nothing was looked at". Any
    /// reader deriving a *rate* has to tell those apart, or it divides real
    /// observations by an exposure that includes ticks nothing was observed
    /// on. That is exactly how background-tier contact came to be undercounted
    /// ~2x; see `metrics::observe_tick`.
    observed: Vec<bool>,
}

impl NeighborArena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset for a tick with `agent_count` agents, keeping allocated capacity.
    pub fn begin(&mut self, agent_count: usize) {
        self.entries.clear();
        self.start.clear();
        self.len.clear();
        self.start.resize(agent_count, 0);
        self.len.resize(agent_count, 0);
        self.observed.clear();
        self.observed.resize(agent_count, false);
    }

    /// Record `neighbors` as belonging to `slot_owner`.
    ///
    /// Entries are indexed by owner, so call order does not affect the result.
    /// Calling twice for one owner in a tick is still wrong — the first batch
    /// is stranded in the arena rather than freed — but it is not a
    /// correctness hazard for readers.
    pub fn push(&mut self, slot_owner: usize, neighbors: &[Neighbor]) {
        self.start[slot_owner] = self.entries.len() as u32;
        self.len[slot_owner] = neighbors.len() as u32;
        self.observed[slot_owner] = true;
        self.entries.extend_from_slice(neighbors);
    }

    /// Record that `slot_owner` was *not* queried this tick.
    ///
    /// Distinct from `push(slot, &[])`, which asserts the agent was looked at
    /// and found alone. Use this for an agent its schedule skipped.
    pub fn push_unobserved(&mut self, slot_owner: usize) {
        self.start[slot_owner] = self.entries.len() as u32;
        self.len[slot_owner] = 0;
        self.observed[slot_owner] = false;
    }

    /// Whether `slot`'s neighbours were looked up this tick. False for a slot
    /// that was skipped, and for one past what `begin` sized for.
    pub fn is_observed(&self, slot: usize) -> bool {
        self.observed.get(slot).copied().unwrap_or(false)
    }

    pub fn neighbors(&self, slot: usize) -> &[Neighbor] {
        // A slot past what `begin` sized for has no recorded neighbors —
        // treat it the same as an empty list rather than panicking. Callers
        // (metrics observation in particular) may hold a `NeighborArena`
        // that was never `begin`-ed for the current world.
        if slot >= self.start.len() {
            return &[];
        }
        let start = self.start[slot] as usize;
        let len = self.len[slot] as usize;
        &self.entries[start..start + len]
    }

    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_arena_returns_empty_slices() {
        let mut arena = NeighborArena::new();
        arena.begin(3);
        assert!(arena.neighbors(0).is_empty());
        assert!(arena.neighbors(2).is_empty());
    }

    #[test]
    fn pushed_neighbors_round_trip_per_agent() {
        let mut arena = NeighborArena::new();
        arena.begin(2);
        arena.push(
            0,
            &[Neighbor {
                slot: 5,
                dist_sq: 1.0,
            }],
        );
        arena.push(
            1,
            &[
                Neighbor {
                    slot: 6,
                    dist_sq: 2.0,
                },
                Neighbor {
                    slot: 7,
                    dist_sq: 3.0,
                },
            ],
        );
        assert_eq!(arena.neighbors(0).len(), 1);
        assert_eq!(arena.neighbors(0)[0].slot, 5);
        assert_eq!(arena.neighbors(1).len(), 2);
        assert_eq!(arena.neighbors(1)[1].slot, 7);
    }

    #[test]
    fn begin_reuses_capacity_without_reallocating() {
        let mut arena = NeighborArena::new();
        arena.begin(4);
        arena.push(
            0,
            &[Neighbor {
                slot: 1,
                dist_sq: 1.0,
            }; 32],
        );
        let capacity = arena.capacity();
        arena.begin(4);
        assert_eq!(arena.capacity(), capacity);
        assert!(arena.neighbors(0).is_empty());
    }
}
