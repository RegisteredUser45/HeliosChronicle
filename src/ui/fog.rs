//! Empire-viewpoint fog filter helpers — read-only over live `world.contact`.
//!
//! Viewpoint is a filter (STATEMENT §11), not a turn. Operator (all) turns the
//! filter **off**. Empire turns the filter **on**; missing contact / empty fog
//! means every system is unknown — never fall back to Operator-all paint.
//! Reads `FogState.known_systems` / `SystemFogEntry.surveyed_fuse` without cloning.

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
    // None fog under an active Empire filter = empty contact → unknown.
    fog.is_some_and(|f| f.known_systems.contains_key(&system))
}

/// Whether Industry/Colony (and similar) may open full ledger for `system`.
#[inline]
pub fn may_open_system_detail(fog_filter: bool, fog: Option<&FogState>, system: EntityId) -> bool {
    !fog_filter || is_known(fog, system)
}

/// Resolve ledger empire id → `EmpireId` for contact lookups.
#[inline]
pub fn as_empire_id(ledger_id: EntityId) -> EmpireId {
    EmpireId(ledger_id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityId;

    #[test]
    fn missing_fog_under_filter_is_unknown() {
        let sid = EntityId(1);
        assert!(!is_known(None, sid));
        assert_eq!(system_knowledge(None, sid), SystemKnowledge::Unknown);
        assert!(!may_open_system_detail(true, None, sid));
        assert!(may_open_system_detail(false, None, sid));
    }
}
