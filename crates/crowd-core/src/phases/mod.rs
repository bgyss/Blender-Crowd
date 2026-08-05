//! Fixed-order tick phases.
//!
//! Each phase is a free function taking immutable previous-state buffers and
//! mutable next-state buffers, so read and write sets are visible in the
//! signature and a later parallel pass needs no semantic change.

pub mod spawn;

pub use spawn::{apply_spawns, SpawnState};
