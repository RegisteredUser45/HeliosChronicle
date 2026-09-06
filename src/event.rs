//! Append-only chronicle event log.

use serde::{Deserialize, Serialize};

use crate::entity::{EmpireId, EntityId};
use crate::knowledge::{KoGrade, KoKind};

/// Typed chronicle events. Phase A/B + I stubs; G/H/P contact/knowledge/violence/standing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    // --- Phase A ---
    TickAdvanced {
        from: u64,
        to: u64,
        dt: u64,
        lod: String,
    },
    OperatorMutation {
        entity: EntityId,
        field: String,
        old: String,
        new: String,
    },
    WorldCreated {
        seed: u64,
    },
    WorldSaved {
        path: String,
    },
    WorldLoaded {
        path: String,
    },
    LodSwitched {
        from: String,
        to: String,
    },
    // --- Phase B stubs (sky owns real behavior) ---
    Deplete {
        system: EntityId,
    },
    FuseArmed {
        system: EntityId,
        end_tick: u64,
    },
    FuseTick {
        system: EntityId,
        remaining: u64,
    },
    FuseEnd {
        system: EntityId,
    },
    Spawn {
        system: EntityId,
    },
    HomeFlagSet {
        system: EntityId,
    },
    HomeFlagClear {
        system: EntityId,
    },
    // --- Phase I stubs (minds) ---
    EmpireSpawned {
        empire: EntityId,
    },
    OrderCreated {
        order: EntityId,
        empire: EntityId,
        intent: String,
    },
    OrderStatusChanged {
        order: EntityId,
        from: String,
        to: String,
    },
    CapitalRescore {
        system: EntityId,
        empire: Option<EntityId>,
    },
    // --- Phase G: Contact ---
    FirstContact {
        a: EmpireId,
        b: EmpireId,
    },
    TreatySigned {
        a: EmpireId,
        b: EmpireId,
        treaty_id: EntityId,
    },
    TreatyBroken {
        a: EmpireId,
        b: EmpireId,
        treaty_id: EntityId,
    },
    ContractDefault {
        a: EmpireId,
        b: EmpireId,
        contract_id: EntityId,
    },
    // --- Phase G/H: Knowledge objects (Lock 9) ---
    KoEmitted {
        ko: EntityId,
        ko_kind: KoKind,
        grade: KoGrade,
    },
    KoAcquired {
        ko: EntityId,
        empire: EmpireId,
    },
    KoConfirmed {
        ko: EntityId,
    },
    Confession {
        ko: EntityId,
        by: EmpireId,
    },
    Leak {
        ko: EntityId,
        by: EmpireId,
    },
    // --- Phase H: Violence (layer writes via violence::strike_layers / salt_world) ---
    OrbitalStrike {
        actor: EmpireId,
        victim: EmpireId,
        system: EntityId,
    },
    BombardmentLayerWrite {
        actor: EmpireId,
        victim: EmpireId,
        system: EntityId,
    },
    SurfaceCombat {
        actor: EmpireId,
        victim: EmpireId,
        system: EntityId,
    },
    GlassAttempt {
        actor: EmpireId,
        victim: EmpireId,
        system: EntityId,
    },
    /// First-class cruelty event (Lead Q4). Causes high-rate layer writes in H.
    Salt {
        actor: EmpireId,
        victim: EmpireId,
        system: EntityId,
    },
    // --- Phase P: Politics ---
    StandingChanged {
        a: EmpireId,
        b: EmpireId,
        old: i32,
        new: i32,
        reason_seq: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChronicleEvent {
    /// Monotonic sequence within the chronicle.
    pub seq: u64,
    /// Master tick at which the event was recorded.
    pub at_tick: u64,
    pub kind: EventKind,
}

/// Append-only event log.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventLog {
    next_seq: u64,
    events: Vec<ChronicleEvent>,
}

impl EventLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, at_tick: u64, kind: EventKind) -> &ChronicleEvent {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(ChronicleEvent {
            seq,
            at_tick,
            kind,
        });
        self.events.last().expect("just pushed")
    }

    pub fn events(&self) -> &[ChronicleEvent] {
        &self.events
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }
}
