//! Knowledge objects (Lock 9) — first-class ledger entities for G/H/P.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::contact::push_fine_hot;
use crate::entity::{EmpireId, EntityId};
use crate::event::EventKind;
use crate::politics::apply_event_for_standing;
use crate::world::World;

/// Who/what currently carries a knowledge object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CarrierId {
    Empire(EmpireId),
    Ship(EntityId),
    Colony(EntityId),
}

/// Knowledge-object kind (Lock 9 spine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KoKind {
    RefugeeWave,
    Wreck,
    Signal,
    ConfessedEvent,
    LeakedEvent,
}

/// Evidence grade — rumor vs confirmed (different standing weights).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KoGrade {
    Rumor,
    Confirmed,
}

/// How a KO propagates between carriers (stub enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KoPropagation {
    EvacConvoy,
    DerelictScan,
    Broadcast,
    DiplomaticReveal,
    SalvageYardLeak,
    OperatorInject,
}

/// Structured claim payload on a KO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KoPayload {
    pub who_actor: Option<EmpireId>,
    pub who_victim: Option<EmpireId>,
    pub system: Option<EntityId>,
    pub severity: u8,
    pub target_type: String,
    pub claim: String,
}

impl Default for KoPayload {
    fn default() -> Self {
        Self {
            who_actor: None,
            who_victim: None,
            system: None,
            severity: 0,
            target_type: String::new(),
            claim: String::new(),
        }
    }
}

/// First-class knowledge object entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeObject {
    pub id: EntityId,
    pub kind: KoKind,
    pub grade: KoGrade,
    /// Chronicle sequence of the originating typed event, if any.
    pub origin_event_seq: Option<u64>,
    pub payload: KoPayload,
    pub carriers: BTreeSet<CarrierId>,
    pub propagation: KoPropagation,
    pub created_tick: u64,
    pub last_transfer_tick: u64,
}

/// Deterministic KO store (sibling to EntityLedger on World).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeStore {
    next_id: u64,
    objects: BTreeMap<EntityId, KnowledgeObject>,
}

impl KnowledgeStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn get(&self, id: EntityId) -> Option<&KnowledgeObject> {
        self.objects.get(&id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut KnowledgeObject> {
        self.objects.get_mut(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&EntityId, &KnowledgeObject)> {
        self.objects.iter()
    }

    fn alloc_id(&mut self) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id += 1;
        id
    }
}

/// Parameters for emitting a new knowledge object.
#[derive(Debug, Clone)]
pub struct EmitKoParams {
    pub kind: KoKind,
    pub grade: KoGrade,
    pub origin_event_seq: Option<u64>,
    pub payload: KoPayload,
    pub initial_carriers: BTreeSet<CarrierId>,
    pub propagation: KoPropagation,
}

/// Emit a KO, append `KoEmitted`, push fine-hot if payload names a system.
pub fn emit_ko(world: &mut World, params: EmitKoParams) -> EntityId {
    let tick = world.master_tick();
    let id = world.knowledge.alloc_id();
    let system = params.payload.system;
    let ko = KnowledgeObject {
        id,
        kind: params.kind,
        grade: params.grade,
        origin_event_seq: params.origin_event_seq,
        payload: params.payload,
        carriers: params.initial_carriers,
        propagation: params.propagation,
        created_tick: tick,
        last_transfer_tick: tick,
    };
    let kind = ko.kind;
    let grade = ko.grade;
    world.knowledge.objects.insert(id, ko);

    world.log.append(
        tick,
        EventKind::KoEmitted {
            ko: id,
            ko_kind: kind,
            grade,
        },
    );

    if let Some(sys) = system {
        push_fine_hot(world, sys);
    }

    world.recompute_outcome_hash();
    id
}

/// Empire acquires a KO (adds empire carrier); appends `KoAcquired` and applies witness standing.
pub fn acquire_ko(world: &mut World, empire: EmpireId, ko_id: EntityId) -> bool {
    let tick = world.master_tick();
    let Some(ko) = world.knowledge.get_mut(ko_id) else {
        return false;
    };
    let carrier = CarrierId::Empire(empire);
    if !ko.carriers.insert(carrier) {
        // Already held — idempotent; no duplicate event/standing.
        return true;
    }
    ko.last_transfer_tick = tick;

    let ev = world.log.append(
        tick,
        EventKind::KoAcquired {
            ko: ko_id,
            empire,
        },
    );
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);
    world.recompute_outcome_hash();
    true
}

/// Upgrade KO grade to Confirmed; appends `KoConfirmed` and applies standing deltas.
pub fn confirm_ko(world: &mut World, ko_id: EntityId) -> bool {
    let tick = world.master_tick();
    let Some(ko) = world.knowledge.get_mut(ko_id) else {
        return false;
    };
    if matches!(ko.grade, KoGrade::Confirmed) {
        return true;
    }
    ko.grade = KoGrade::Confirmed;

    let ev = world.log.append(tick, EventKind::KoConfirmed { ko: ko_id });
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);
    world.recompute_outcome_hash();
    true
}

/// Salvage contact always grants a wreck KO + basic payload (never salvage-blind).
pub fn salvage_contact(
    world: &mut World,
    salvager: EmpireId,
    wreck_system: EntityId,
    claim: impl Into<String>,
) -> EntityId {
    let ko_id = emit_ko(
        world,
        EmitKoParams {
            kind: KoKind::Wreck,
            grade: KoGrade::Confirmed,
            origin_event_seq: None,
            payload: KoPayload {
                who_actor: None,
                who_victim: None,
                system: Some(wreck_system),
                severity: 1,
                target_type: "wreck".into(),
                claim: claim.into(),
            },
            initial_carriers: BTreeSet::new(),
            propagation: KoPropagation::SalvageYardLeak,
        },
    );
    acquire_ko(world, salvager, ko_id);
    ko_id
}

/// Operator rumor inject — rumor-grade KO via the same emit API (Lead Q7).
pub fn inject_rumor(
    world: &mut World,
    kind: KoKind,
    payload: KoPayload,
    initial_empire: Option<EmpireId>,
) -> EntityId {
    let ko_id = emit_ko(
        world,
        EmitKoParams {
            kind,
            grade: KoGrade::Rumor,
            origin_event_seq: None,
            payload,
            initial_carriers: BTreeSet::new(),
            propagation: KoPropagation::OperatorInject,
        },
    );
    if let Some(e) = initial_empire {
        acquire_ko(world, e, ko_id);
    }
    ko_id
}

/// Empire confesses a held KO: grade→Confirmed, kind→ConfessedEvent, emits `Confession`.
/// ExtraditionStub partners of `by` auto-acquire the KO (diplomatic evidence transfer).
pub fn confess_ko(world: &mut World, ko_id: EntityId, by: EmpireId) -> bool {
    if world.knowledge.get(ko_id).is_none() {
        return false;
    }
    acquire_ko(world, by, ko_id);
    if let Some(ko) = world.knowledge.get_mut(ko_id) {
        ko.kind = KoKind::ConfessedEvent;
        ko.grade = KoGrade::Confirmed;
    }
    let tick = world.master_tick();
    let ev = world.log.append(tick, EventKind::Confession { ko: ko_id, by });
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);

    // ExtraditionStub: treaty partners of `by` receive the KO.
    let partners: Vec<EmpireId> = world
        .contact
        .get(by)
        .map(|c| {
            c.treaties
                .iter()
                .filter(|t| {
                    t.end_tick.is_none()
                        && t.clauses
                            .iter()
                            .any(|cl| *cl == crate::contact::TreatyClause::ExtraditionStub)
                })
                .map(|t| if t.a == by { t.b } else { t.a })
                .collect()
        })
        .unwrap_or_default();
    for partner in partners {
        acquire_ko(world, partner, ko_id);
    }
    world.recompute_outcome_hash();
    true
}

/// Empire leaks a held KO: grade→Confirmed, kind→LeakedEvent, emits `Leak`.
pub fn leak_ko(world: &mut World, ko_id: EntityId, by: EmpireId) -> bool {
    if world.knowledge.get(ko_id).is_none() {
        return false;
    }
    acquire_ko(world, by, ko_id);
    if let Some(ko) = world.knowledge.get_mut(ko_id) {
        ko.kind = KoKind::LeakedEvent;
        ko.grade = KoGrade::Confirmed;
    }
    let tick = world.master_tick();
    let ev = world.log.append(tick, EventKind::Leak { ko: ko_id, by });
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);
    world.recompute_outcome_hash();
    true
}

/// Hand off a KO from one empire carrier to another (Lock 9 carrier chain).
///
/// `from` must already hold the KO. Removes their empire carrier, then
/// [`acquire_ko`] for `to` (standing/events apply for the recipient).

/// Host a RefugeeWave KO: acquire for `host` and grant fog on the KO's system if named.
///
/// Fails closed unless the object exists and is `KoKind::RefugeeWave`.
pub fn host_refugees(world: &mut World, host: EmpireId, ko_id: EntityId) -> bool {
    let Some(ko) = world.knowledge.get(ko_id) else {
        return false;
    };
    if ko.kind != KoKind::RefugeeWave {
        return false;
    }
    let system = ko.payload.system;
    if !acquire_ko(world, host, ko_id) {
        return false;
    }
    if let Some(sys) = system {
        crate::contact::grant_fog(world, host, sys);
    }
    true
}

pub fn transfer_ko(
    world: &mut World,
    ko_id: EntityId,
    from: EmpireId,
    to: EmpireId,
) -> bool {
    if from == to {
        return acquire_ko(world, to, ko_id);
    }
    let tick = world.master_tick();
    let Some(ko) = world.knowledge.get_mut(ko_id) else {
        return false;
    };
    let from_carrier = CarrierId::Empire(from);
    if !ko.carriers.contains(&from_carrier) {
        return false;
    }
    ko.carriers.remove(&from_carrier);
    ko.last_transfer_tick = tick;
    drop(ko);
    acquire_ko(world, to, ko_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EmpireId;
    use crate::world::World;

    #[test]
    fn transfer_ko_moves_carrier() {
        let mut w = World::new(121);
        let from = EmpireId(1);
        let to = EmpireId(2);
        let actor = EmpireId(3);
        let ko = inject_rumor(
            &mut w,
            KoKind::Signal,
            KoPayload {
                who_actor: Some(actor),
                who_victim: None,
                system: None,
                severity: 0,
                target_type: "xfer".into(),
                claim: "hand off".into(),
            },
            Some(from),
        );
        assert!(w.knowledge.get(ko).unwrap().carriers.contains(&CarrierId::Empire(from)));
        assert!(transfer_ko(&mut w, ko, from, to));
        let carriers = &w.knowledge.get(ko).unwrap().carriers;
        assert!(!carriers.contains(&CarrierId::Empire(from)));
        assert!(carriers.contains(&CarrierId::Empire(to)));
        assert!(!transfer_ko(&mut w, ko, from, to)); // from no longer holds
    }

    #[test]
    fn host_refugees_grants_fog() {
        let mut w = World::new(140);
        let host = EmpireId(4);
        let system = *w.ledger().systems().next().unwrap().0;
        let ko = emit_ko(
            &mut w,
            EmitKoParams {
                kind: KoKind::RefugeeWave,
                grade: KoGrade::Rumor,
                origin_event_seq: None,
                payload: KoPayload {
                    who_actor: Some(EmpireId(1)),
                    who_victim: Some(EmpireId(2)),
                    system: Some(system),
                    severity: 3,
                    target_type: "refugees".into(),
                    claim: "evac".into(),
                },
                initial_carriers: Default::default(),
                propagation: KoPropagation::EvacConvoy,
            },
        );
        assert!(host_refugees(&mut w, host, ko));
        assert!(w
            .knowledge
            .get(ko)
            .unwrap()
            .carriers
            .contains(&CarrierId::Empire(host)));
        assert!(w
            .contact
            .get(host)
            .unwrap()
            .fog
            .known_systems
            .contains_key(&system));
        // Wrong kind fails.
        let signal = inject_rumor(
            &mut w,
            KoKind::Signal,
            KoPayload::default(),
            None,
        );
        assert!(!host_refugees(&mut w, host, signal));
    }

}
