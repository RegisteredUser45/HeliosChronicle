//! Phase G — Contact: per-empire fog, treaties, first_contact (emit + fog).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::entity::{EmpireId, EntityId};
use crate::event::EventKind;
use crate::lod::LodHint;
use crate::politics::apply_event_for_standing;
use crate::world::World;

/// Per-system fog entry for one empire's knowledge state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemFogEntry {
    pub last_known_tick: u64,
    /// Sensor/diplomatic uncertainty stub (0.0 = certain).
    pub uncertainty: f64,
    /// Surveyed fuse product known? (Sky owns survey; Contact consumes).
    pub surveyed_fuse: bool,
}

impl SystemFogEntry {
    pub fn fresh(tick: u64) -> Self {
        Self {
            last_known_tick: tick,
            uncertainty: 1.0,
            surveyed_fuse: false,
        }
    }
}

/// Fog is knowledge state per empire, not a map shader.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FogState {
    pub known_systems: BTreeMap<EntityId, SystemFogEntry>,
}

/// Minimal v1 treaty clause enum (Lead Q3 — no opaque bags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatyClause {
    NonAggression,
    OpenPassage,
    ExtraditionStub,
    ReparationsStub,
}

/// Stub treaty record (G owns instrument; P owns standing score).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Treaty {
    pub id: EntityId,
    pub a: EmpireId,
    pub b: EmpireId,
    pub clauses: Vec<TreatyClause>,
    pub start_tick: u64,
    pub end_tick: Option<u64>,
}

/// Per-empire contact registry entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmpireContact {
    pub fog: FogState,
    pub treaties: Vec<Treaty>,
}

/// Empire contact / fog registry on World.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmpireContactStore {
    pub empires: BTreeMap<EmpireId, EmpireContact>,
    next_treaty_id: u64,
}

impl EmpireContactStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn empire_count(&self) -> usize {
        self.empires.len()
    }

    pub fn ensure(&mut self, id: EmpireId) -> &mut EmpireContact {
        self.empires.entry(id).or_default()
    }

    pub fn get(&self, id: EmpireId) -> Option<&EmpireContact> {
        self.empires.get(&id)
    }
}

/// Lock 10: Contact/Violence push fine-hot to Kernel for involved systems.
pub fn push_fine_hot(world: &mut World, system_id: EntityId) {
    if let Some(sys) = world.ledger.get_mut(system_id) {
        sys.lod_hint = LodHint::Hot;
    }
    world.recompute_outcome_hash();
}

/// G-owned first contact: mutual fog stubs + emit `FirstContact` (P consumes for standing).
///
/// Optional `at_system` receives a fine-hot push when contact occurs in/about a system.
pub fn first_contact(
    world: &mut World,
    a: EmpireId,
    b: EmpireId,
    at_system: Option<EntityId>,
) {
    let tick = world.master_tick();

    // Ensure both empires exist in the registry.
    world.contact.ensure(a);
    world.contact.ensure(b);

    // Mutual fog stubs: if a system is named, both learn a coarse entry.
    if let Some(sys) = at_system {
        let entry_a = SystemFogEntry {
            last_known_tick: tick,
            uncertainty: 0.5,
            surveyed_fuse: false,
        };
        let entry_b = entry_a.clone();
        world
            .contact
            .ensure(a)
            .fog
            .known_systems
            .insert(sys, entry_a);
        world
            .contact
            .ensure(b)
            .fog
            .known_systems
            .insert(sys, entry_b);
        push_fine_hot(world, sys);
    } else {
        // No system: still mark empires as having met via empty fog presence.
        let _ = world.contact.ensure(a);
        let _ = world.contact.ensure(b);
    }

    let ev = world
        .log
        .append(tick, EventKind::FirstContact { a, b });
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);
    world.recompute_outcome_hash();
}
