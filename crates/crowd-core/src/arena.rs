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
    }

    /// Record `neighbors` as belonging to `slot_owner`.
    ///
    /// Must be called at most once per agent per tick, in ascending slot
    /// order, which the perceive phase guarantees.
    pub fn push(&mut self, slot_owner: usize, neighbors: &[Neighbor]) {
        self.start[slot_owner] = self.entries.len() as u32;
        self.len[slot_owner] = neighbors.len() as u32;
        self.entries.extend_from_slice(neighbors);
    }

    pub fn neighbors(&self, slot: usize) -> &[Neighbor] {
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
