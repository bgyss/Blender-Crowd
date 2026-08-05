//! Fixed-order tick phases.
//!
//! Each phase is a free function taking immutable previous-state buffers and
//! mutable next-state buffers, so read and write sets are visible in the
//! signature and a later parallel pass needs no semantic change.

pub mod decide;
pub mod integrate;
pub mod perceive;
pub mod spawn;
pub mod steer;

pub use decide::{decide, DecideConfig};
pub use integrate::{integrate, IntegrateConfig, IntegrateReport, IntegrateScratch};
pub use perceive::{perceive, PerceiveConfig, PerceiveScratch};
pub use spawn::{apply_spawns, SpawnState};
pub use steer::{steer, SteerConfig, SteerReport, SteerScratch};
