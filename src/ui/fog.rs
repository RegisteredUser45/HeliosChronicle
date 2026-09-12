//! Empire-viewpoint fog filter helpers — read-only over live `world.contact`.
//!
//! Viewpoint is a filter (STATEMENT §11), not a turn. Operator (all) passes
//! `None` fog and keeps full ledger rows. Empire viewpoint reads
//! `FogState.known_systems` / `SystemFogEntry.surveyed_fuse` without cloning.

use crate::contact::FogState;
use crate::entity::{EmpireId, EntityId};
use crate::world::World;

/// What the selected empire knows about one system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKnowledge {
    /// Absent from `known_systems` — map placeholder / inspector "unknown".
    Unknown,
    /// Present in fog; fuse numbers only when `surveyed_fuse`.
    Known { surveyed_fuse: bool },
}

/// Live fog ref for an empire (no clone).
#[inline]
pub fn fog_of(world: &World, empire: EmpireId) -> Option<&FogState> {
    world.contact.get(empire).map(|c| &c.fog)
}

#[inline]
pub fn system_knowledge(fog: Option<&FogState>, system: EntityId) -> SystemKnowledge {
    match fog.and_then(|f| f.known_systems.get(&system)) {
        None => SystemKnowledge::Unknown,
        Some(entry) => SystemKnowledge::Known {
            surveyed_fuse: entry.surveyed_fuse,
        },
    }
}

#[inline]
pub fn is_known(fog: Option<&FogState>, system: EntityId) -> bool {
    fog.is_some_and(|f| f.known_systems.contains_key(&system))
}

/// Resolve ledger empire id → `EmpireId` for contact lookups.
#[inline]
pub fn as_empire_id(ledger_id: EntityId) -> EmpireId {
    EmpireId(ledger_id.0)
}
