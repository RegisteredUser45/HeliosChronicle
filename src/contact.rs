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


/// Diplomatic fog grant: observer learns `system` without sensors (uncertainty 0.25).
pub fn grant_fog(
    world: &mut World,
    observer: EmpireId,
    system: EntityId,
) {
    let tick = world.master_tick();
    world.contact.ensure(observer).fog.known_systems.insert(
        system,
        SystemFogEntry {
            last_known_tick: tick,
            uncertainty: 0.25,
            surveyed_fuse: false,
        },
    );
    push_fine_hot(world, system);
    world.recompute_outcome_hash();
}

/// Upgrade an existing fog entry (or create one): lower uncertainty, refresh tick.
pub fn upgrade_fog(world: &mut World, observer: EmpireId, system: EntityId) {
    let tick = world.master_tick();
    let entry = world
        .contact
        .ensure(observer)
        .fog
        .known_systems
        .entry(system)
        .or_insert_with(|| SystemFogEntry::fresh(tick));
    entry.last_known_tick = tick;
    entry.uncertainty = (entry.uncertainty * 0.25).max(0.0);
    push_fine_hot(world, system);
    world.recompute_outcome_hash();
}

/// Sign a v1 treaty (clause enum only). Emits `TreatySigned`; both parties get standing via P.
pub fn sign_treaty(
    world: &mut World,
    a: EmpireId,
    b: EmpireId,
    clauses: Vec<TreatyClause>,
) -> EntityId {
    let tick = world.master_tick();
    let id = EntityId(world.contact.next_treaty_id);
    world.contact.next_treaty_id += 1;
    let treaty = Treaty {
        id,
        a,
        b,
        clauses,
        start_tick: tick,
        end_tick: None,
    };
    world.contact.ensure(a).treaties.push(treaty.clone());
    world.contact.ensure(b).treaties.push(treaty);
    push_contact_hot_empires(world, a, b);
    let ev = world.log.append(
        tick,
        EventKind::TreatySigned {
            a,
            b,
            treaty_id: id,
        },
    );
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);
    world.recompute_outcome_hash();
    id
}

/// Break a treaty by id. Emits `TreatyBroken`; optional rumor KO for third-party witnesses.
pub fn break_treaty(
    world: &mut World,
    treaty_id: EntityId,
    emit_breach_ko: bool,
) -> bool {
    let tick = world.master_tick();
    let mut found: Option<(EmpireId, EmpireId)> = None;
    for (_eid, contact) in world.contact.empires.iter_mut() {
        if let Some(pos) = contact.treaties.iter().position(|t| t.id == treaty_id) {
            let t = contact.treaties.remove(pos);
            found = Some((t.a, t.b));
            // Remove from both sides — continue scan
        }
    }
    // Clean residual copies on the other party
    if let Some((a, b)) = found {
        for party in [a, b] {
            if let Some(c) = world.contact.empires.get_mut(&party) {
                c.treaties.retain(|t| t.id != treaty_id);
            }
        }
        push_contact_hot_empires(world, a, b);
        let ev = world.log.append(
            tick,
            EventKind::TreatyBroken {
                a,
                b,
                treaty_id,
            },
        );
        let chronicle = ev.clone();
        apply_event_for_standing(world, &chronicle);
        if emit_breach_ko {
            let _ = crate::knowledge::emit_ko(
                world,
                crate::knowledge::EmitKoParams {
                    kind: crate::knowledge::KoKind::Signal,
                    grade: crate::knowledge::KoGrade::Rumor,
                    origin_event_seq: Some(chronicle.seq),
                    payload: crate::knowledge::KoPayload {
                        who_actor: Some(a),
                        who_victim: Some(b),
                        system: None,
                        severity: 2,
                        target_type: "treaty_breach".into(),
                        claim: format!("treaty {treaty_id} broken"),
                    },
                    initial_carriers: Default::default(),
                    propagation: crate::knowledge::KoPropagation::DiplomaticReveal,
                },
            );
        }
        world.recompute_outcome_hash();
        true
    } else {
        false
    }
}

/// Contact LOD: push fine-hot on all capital/home systems of the two empires (stub),
/// or any systems in either fog map — keeps Lock 10 push model (no Kernel poll).
fn push_contact_hot_empires(world: &mut World, a: EmpireId, b: EmpireId) {
    let mut systems = std::collections::BTreeSet::new();
    for party in [a, b] {
        if let Some(c) = world.contact.get(party) {
            systems.extend(c.fog.known_systems.keys().copied());
        }
        if let Some(cap) = world.ledger.capital_of(party) {
            systems.insert(cap);
        }
    }
    for sys in systems {
        push_fine_hot(world, sys);
    }
}

/// True if empire has any foreign treaty or non-empty fog (contact-hot hint).
pub fn empire_contact_active(world: &World, empire: EmpireId) -> bool {
    world
        .contact
        .get(empire)
        .map(|c| !c.treaties.is_empty() || !c.fog.known_systems.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn grant_and_upgrade_fog() {
        let mut w = World::new(10);
        let sys = *w.ledger().systems().next().unwrap().0;
        let e = EmpireId(1);
        grant_fog(&mut w, e, sys);
        let u0 = w.contact.get(e).unwrap().fog.known_systems[&sys].uncertainty;
        upgrade_fog(&mut w, e, sys);
        let u1 = w.contact.get(e).unwrap().fog.known_systems[&sys].uncertainty;
        assert!(u1 < u0);
    }

    #[test]
    fn sign_and_break_treaty() {
        let mut w = World::new(11);
        let a = EmpireId(1);
        let b = EmpireId(2);
        let id = sign_treaty(
            &mut w,
            a,
            b,
            vec![TreatyClause::NonAggression, TreatyClause::OpenPassage],
        );
        assert!(w.contact.get(a).unwrap().treaties.iter().any(|t| t.id == id));
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::TreatySigned { treaty_id, .. } if *treaty_id == id
        )));
        assert!(break_treaty(&mut w, id, true));
        assert!(w.contact.get(a).unwrap().treaties.is_empty());
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::TreatyBroken { treaty_id, .. } if *treaty_id == id
        )));
        assert!(w.knowledge.iter().any(|(_, ko)| ko.payload.target_type == "treaty_breach"));
    }

    #[test]
    fn contact_active_and_lod_push() {
        let mut w = World::new(12);
        let sys = *w.ledger().systems().next().unwrap().0;
        let a = EmpireId(3);
        assert!(!empire_contact_active(&w, a));
        grant_fog(&mut w, a, sys);
        assert!(empire_contact_active(&w, a));
        assert_eq!(w.ledger().get(sys).unwrap().lod_hint, crate::lod::LodHint::Hot);
    }
}
