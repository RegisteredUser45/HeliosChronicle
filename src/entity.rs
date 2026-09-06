//! Entity ledger — single shared store for tick + operator.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::lod::LodHint;

/// Stable entity identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// System entity schema stub (Phase B owns real sky).
///
/// Fuse design: prefer `fuse_end_tick` (absolute master tick). Remaining is
/// derived as `fuse_end_tick.saturating_sub(master_tick)`. Coarse dt can jump
/// without incorrectly skipping the end — when `master_tick >= fuse_end_tick`,
/// the fuse ends exactly once (monotonic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemEntity {
    pub id: EntityId,
    /// Extractable catalog-binding value still in the ground (placeholder).
    pub binding_remainder: f64,
    pub depleted: bool,
    /// Absolute master tick when the fuse ends, if armed.
    pub fuse_end_tick: Option<u64>,
    /// Placeholder extinction/fuse remaining in master ticks (derived cache).
    pub fuse_remaining: Option<u64>,
    pub is_home_capital: bool,
    pub home_flag: bool,
    pub lod_hint: LodHint,
}

impl SystemEntity {
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            binding_remainder: 1000.0,
            depleted: false,
            fuse_end_tick: None,
            fuse_remaining: None,
            is_home_capital: false,
            home_flag: false,
            lod_hint: LodHint::Quiet,
        }
    }

    /// Sync derived remaining from absolute end tick and current master tick.
    pub fn sync_fuse_remaining(&mut self, master_tick: u64) {
        self.fuse_remaining = self
            .fuse_end_tick
            .map(|end| end.saturating_sub(master_tick));
    }

    /// Advance fuse awareness after a tick of size `dt`.
    /// Uses absolute end tick; returns true if the fuse ends this step.
    pub fn fuse_crossed_end(&self, master_tick_before: u64, dt: u64) -> bool {
        match self.fuse_end_tick {
            Some(end) => {
                let before = master_tick_before;
                let after = master_tick_before.saturating_add(dt);
                before < end && after >= end
            }
            None => false,
        }
    }
}

/// Single shared entity store — tick loop and operator both use this.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityLedger {
    next_id: u64,
    systems: BTreeMap<EntityId, SystemEntity>,
}

impl EntityLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn_system(&mut self) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id += 1;
        self.systems.insert(id, SystemEntity::new(id));
        id
    }

    pub fn insert(&mut self, entity: SystemEntity) {
        if entity.id.0 >= self.next_id {
            self.next_id = entity.id.0 + 1;
        }
        self.systems.insert(entity.id, entity);
    }

    pub fn get(&self, id: EntityId) -> Option<&SystemEntity> {
        self.systems.get(&id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut SystemEntity> {
        self.systems.get_mut(&id)
    }

    pub fn systems(&self) -> impl Iterator<Item = (&EntityId, &SystemEntity)> {
        self.systems.iter()
    }

    pub fn systems_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut SystemEntity)> {
        self.systems.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.systems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }

    pub fn next_id(&self) -> u64 {
        self.next_id
    }
}
