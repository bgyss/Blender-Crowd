//! Stable identity contract.
//!
//! Agent IDs derive from the project seed, population, spawn source, and
//! spawn ordinal, per contract section 5.1. The mixing function is vendored
//! rather than taken from an external hasher: an external hash's exact output
//! is not a stability guarantee, and if it changed, every cache and baseline
//! in the project would silently break. Vendoring makes it part of the
//! versioned contract.

use serde::{Deserialize, Serialize};

/// SplitMix64 finalizer. Strong avalanche, no dependencies, permanently fixed.
pub const fn mix64(z: u64) -> u64 {
    let mut z = z;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Fold one value into a running hash.
pub const fn hash_combine(seed: u64, value: u64) -> u64 {
    mix64(seed ^ mix64(value.wrapping_add(0x9e3779b97f4a7c15)))
}

/// FNV-1a over bytes, then finalized through `mix64`.
///
/// Used only to turn authoring names into stable numeric IDs at scene compile
/// time. Never called inside the tick loop.
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in s.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    mix64(h)
}

/// A stable 64-bit agent identifier.
///
/// Stable across rebakes when unrelated authoring data changes, and the
/// tie-breaker for every ordering decision in the kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

/// Derive an agent's stable ID, per contract section 5.1.
///
/// Deliberately a pure function of its inputs: it never consults a counter,
/// the world, or call order, so IDs do not shift when unrelated agents are
/// added or removed.
pub fn derive_agent_id(
    project_seed: u64,
    population_id: u16,
    spawn_source_id: u16,
    ordinal: u32,
) -> AgentId {
    let mut h = mix64(project_seed);
    h = hash_combine(h, population_id as u64);
    h = hash_combine(h, spawn_source_id as u64);
    h = hash_combine(h, ordinal as u64);
    AgentId(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn mix64_output_is_pinned() {
        // These values are a contract. If this test fails after an
        // intentional change, every baseline and cache must be regenerated.
        assert_eq!(mix64(0), 0);
        assert_eq!(mix64(1), 6238072747940578789);
        assert_eq!(mix64(u64::MAX), 13029008266876403067);
    }

    #[test]
    fn derived_ids_are_unique_across_ordinals() {
        let mut seen = BTreeSet::new();
        for ordinal in 0..10_000u32 {
            assert!(seen.insert(derive_agent_id(42, 0, 0, ordinal)));
        }
    }

    #[test]
    fn derived_ids_are_unique_across_spawn_sources() {
        let a = derive_agent_id(42, 0, 0, 7);
        let b = derive_agent_id(42, 0, 1, 7);
        assert_ne!(a, b);
    }

    #[test]
    fn derived_ids_are_unique_across_populations() {
        assert_ne!(derive_agent_id(42, 0, 0, 7), derive_agent_id(42, 1, 0, 7));
    }

    #[test]
    fn derived_ids_depend_on_project_seed() {
        assert_ne!(derive_agent_id(42, 0, 0, 7), derive_agent_id(43, 0, 0, 7));
    }

    #[test]
    fn derived_ids_are_independent_of_call_order() {
        let forward: Vec<_> = (0..100).map(|i| derive_agent_id(1, 0, 0, i)).collect();
        let backward: Vec<_> = (0..100)
            .rev()
            .map(|i| derive_agent_id(1, 0, 0, i))
            .collect();
        let backward: Vec<_> = backward.into_iter().rev().collect();
        assert_eq!(forward, backward);
    }

    #[test]
    fn hash_str_is_stable_and_distinguishes_inputs() {
        assert_eq!(hash_str("platform_a"), hash_str("platform_a"));
        assert_ne!(hash_str("platform_a"), hash_str("platform_b"));
    }
}
