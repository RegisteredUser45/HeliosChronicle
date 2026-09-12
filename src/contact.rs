//! Phase G — Contact: per-empire fog, treaties, first_contact (emit + fog).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::entity::{EmpireId, EntityId};
use crate::event::EventKind;
use crate::lod::{hot_until_tick, LodHint, DEFAULT_HOT_TTL_TICKS};
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


/// Per-fleet fog entry (last fix + uncertainty). H sensors upgrade this.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetFogEntry {
    pub last_known_tick: u64,
    pub uncertainty: f64,
    pub last_system: Option<EntityId>,
}

impl FleetFogEntry {
    pub fn fresh(tick: u64, system: Option<EntityId>) -> Self {
        Self {
            last_known_tick: tick,
            uncertainty: 1.0,
            last_system: system,
        }
    }
}

/// Fog is knowledge state per empire, not a map shader.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FogState {
    pub known_systems: BTreeMap<EntityId, SystemFogEntry>,
    #[serde(default)]
    pub known_fleets: BTreeMap<EntityId, FleetFogEntry>,
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



/// Narrower than treaties: freight / salvage / hire / survey charter (G owns record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    Freight,
    SalvageRights,
    MercenaryHire,
    SurveyCharter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: EntityId,
    pub a: EmpireId,
    pub b: EmpireId,
    pub kind: ContractKind,
    pub start_tick: u64,
    pub end_tick: Option<u64>,
    pub defaulted: bool,
}

impl EmpireContact {
    /// Active (non-defaulted) contracts for this empire.
    pub fn active_contracts(&self) -> impl Iterator<Item = &Contract> {
        self.contracts.iter().filter(|c| !c.defaulted)
    }
}

/// Per-empire contact registry entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmpireContact {
    pub fog: FogState,
    pub treaties: Vec<Treaty>,
    #[serde(default)]
    pub contracts: Vec<Contract>,
}

/// Empire contact / fog registry on World.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EmpireContactStore {
    pub empires: BTreeMap<EmpireId, EmpireContact>,
    next_treaty_id: u64,
    next_contract_id: u64,
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
    let now = world.master_tick();
    if let Some(sys) = world.ledger.get_mut(system_id) {
        sys.lod_hint = LodHint::Hot;
        sys.hot_until = Some(hot_until_tick(now, DEFAULT_HOT_TTL_TICKS));
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

/// Diplomatic fog share: copy `from`'s known-system entry to `to` (Lock 9 knowledge path).
///
/// Fails closed if `from` does not know `system`. Recipient gets the shared entry
/// (or keeps lower uncertainty if already better). Pushes fine-hot.
pub fn share_fog(
    world: &mut World,
    from: EmpireId,
    to: EmpireId,
    system: EntityId,
) -> bool {
    let Some(src) = world
        .contact
        .get(from)
        .and_then(|c| c.fog.known_systems.get(&system))
        .cloned()
    else {
        return false;
    };
    let tick = world.master_tick();
    let entry = world
        .contact
        .ensure(to)
        .fog
        .known_systems
        .entry(system)
        .or_insert_with(|| SystemFogEntry::fresh(tick));
    entry.last_known_tick = entry.last_known_tick.max(src.last_known_tick).max(tick);
    entry.uncertainty = entry.uncertainty.min(src.uncertainty);
    if src.surveyed_fuse {
        entry.surveyed_fuse = true;
    }
    push_fine_hot(world, system);
    world.recompute_outcome_hash();
    true
}

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
    let open_passage = clauses.iter().any(|c| matches!(c, TreatyClause::OpenPassage));
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
    // OpenPassage: mutual diplomatic fog on each other's capitals (if any).
    if open_passage {
        if let Some(cap_a) = world.ledger.capital_of(a) {
            grant_fog(world, b, cap_a);
        }
        if let Some(cap_b) = world.ledger.capital_of(b) {
            grant_fog(world, a, cap_b);
        }
    }
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
    let mut found: Option<(EmpireId, EmpireId, bool)> = None;
    for (_eid, contact) in world.contact.empires.iter_mut() {
        if let Some(pos) = contact.treaties.iter().position(|t| t.id == treaty_id) {
            let t = contact.treaties.remove(pos);
            let had_reparations = t.clauses.iter().any(|c| *c == TreatyClause::ReparationsStub);
            found = Some((t.a, t.b, had_reparations));
            // Remove from both sides — continue scan
        }
    }
    // Clean residual copies on the other party
    if let Some((a, b, had_reparations)) = found {
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
        if had_reparations {
            crate::politics::apply_reparations_breach(world, a, b, chronicle.seq);
        }
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


/// Sign a contract between two empires (G instrument; Matter/Hulls fulfill later).
pub fn sign_contract(
    world: &mut World,
    a: EmpireId,
    b: EmpireId,
    kind: ContractKind,
) -> EntityId {
    let tick = world.master_tick();
    let id = EntityId(world.contact.next_contract_id);
    world.contact.next_contract_id = world.contact.next_contract_id.saturating_add(1);
    let contract = Contract {
        id,
        a,
        b,
        kind,
        start_tick: tick,
        end_tick: None,
        defaulted: false,
    };
    world.contact.ensure(a).contracts.push(contract.clone());
    world.contact.ensure(b).contracts.push(contract);
    push_contact_hot_empires(world, a, b);
    world.recompute_outcome_hash();
    id
}

/// Mark contract defaulted; emit `ContractDefault` for P standing.
pub fn default_contract(world: &mut World, contract_id: EntityId) -> bool {
    let tick = world.master_tick();
    let mut parties: Option<(EmpireId, EmpireId)> = None;
    for contact in world.contact.empires.values_mut() {
        if let Some(c) = contact.contracts.iter_mut().find(|c| c.id == contract_id) {
            if c.defaulted {
                return false;
            }
            c.defaulted = true;
            c.end_tick = Some(tick);
            parties = Some((c.a, c.b));
        }
    }
    if let Some((a, b)) = parties {
        // Mirror defaulted flag on both copies
        for party in [a, b] {
            if let Some(c) = world.contact.empires.get_mut(&party) {
                if let Some(con) = c.contracts.iter_mut().find(|c| c.id == contract_id) {
                    con.defaulted = true;
                    con.end_tick = Some(tick);
                }
            }
        }
        push_contact_hot_empires(world, a, b);
        let ev = world.log.append(
            tick,
            EventKind::ContractDefault {
                a,
                b,
                contract_id,
            },
        );
        let chronicle = ev.clone();
        apply_event_for_standing(world, &chronicle);
        world.recompute_outcome_hash();
        true
    } else {
        false
    }
}


/// Record / refresh a fleet last-known fix in observer fog (G knowledge state).

/// Lose fleet contact: drop observer's last-known fleet fog entry (sensor/intel gap).
pub fn lose_fleet_contact(world: &mut World, observer: EmpireId, fleet: EntityId) -> bool {
    let Some(contact) = world.contact.empires.get_mut(&observer) else {
        return false;
    };
    let removed = contact.fog.known_fleets.remove(&fleet).is_some();
    if removed {
        world.recompute_outcome_hash();
    }
    removed
}

pub fn sense_fleet(
    world: &mut World,
    observer: EmpireId,
    fleet: EntityId,
    at_system: Option<EntityId>,
) {
    let tick = world.master_tick();
    let entry = world
        .contact
        .ensure(observer)
        .fog
        .known_fleets
        .entry(fleet)
        .or_insert_with(|| FleetFogEntry::fresh(tick, at_system));
    entry.last_known_tick = tick;
    entry.uncertainty = (entry.uncertainty * 0.5).max(0.0);
    if at_system.is_some() {
        entry.last_system = at_system;
    }
    if let Some(sys) = at_system {
        push_fine_hot(world, sys);
    }
    world.recompute_outcome_hash();
}


/// True if a and b share an active treaty containing `clause`.
pub fn has_clause(world: &World, a: EmpireId, b: EmpireId, clause: TreatyClause) -> bool {
    let Some(ca) = world.contact.get(a) else {
        return false;
    };
    ca.treaties.iter().any(|t| {
        ((t.a == a && t.b == b) || (t.a == b && t.b == a))
            && t.end_tick.is_none()
            && t.clauses.iter().any(|cl| *cl == clause)
    })
}


/// Mark a known system as surveyed (fuse product known). Creates fog entry if missing.
pub fn mark_system_surveyed(world: &mut World, empire: EmpireId, system: EntityId) -> bool {
    let tick = world.master_tick();
    let entry = world
        .contact
        .ensure(empire)
        .fog
        .known_systems
        .entry(system)
        .or_insert_with(|| SystemFogEntry::fresh(tick));
    let was = entry.surveyed_fuse;
    entry.surveyed_fuse = true;
    entry.last_known_tick = tick;
    push_fine_hot(world, system);
    world.recompute_outcome_hash();
    !was
}


/// Fulfill an active SurveyCharter: both parties mark `system` surveyed (fuse known).
pub fn fulfill_survey_charter(
    world: &mut World,
    contract_id: EntityId,
    system: EntityId,
) -> bool {
    let mut parties: Option<(EmpireId, EmpireId)> = None;
    for contact in world.contact.empires.values() {
        if let Some(con) = contact.contracts.iter().find(|c| c.id == contract_id) {
            if con.defaulted || con.end_tick.is_some() {
                return false;
            }
            if con.kind != ContractKind::SurveyCharter {
                return false;
            }
            parties = Some((con.a, con.b));
            break;
        }
    }
    let Some((a, b)) = parties else {
        return false;
    };
    mark_system_surveyed(world, a, system);
    mark_system_surveyed(world, b, system);
    // Slight fog upgrade for both (survey product).
    upgrade_fog(world, a, system);
    upgrade_fog(world, b, system);
    push_contact_hot_empires(world, a, b);
    world.recompute_outcome_hash();
    true
}

/// Grow fleet-fog uncertainty for one observer (quiet ticks without a fresh fix).

/// Grow system-fog uncertainty for one observer (quiet ticks without upgrade).
pub fn decay_system_fog(world: &mut World, observer: EmpireId, factor: f64) {
    let factor = factor.clamp(1.0, 4.0);
    let Some(contact) = world.contact.empires.get_mut(&observer) else {
        return;
    };
    for entry in contact.fog.known_systems.values_mut() {
        entry.uncertainty = (entry.uncertainty * factor).min(1.0);
    }
    world.recompute_outcome_hash();
}

pub fn decay_fleet_fog(world: &mut World, observer: EmpireId, factor: f64) {
    let factor = factor.clamp(1.0, 4.0);
    let Some(contact) = world.contact.empires.get_mut(&observer) else {
        return;
    };
    for entry in contact.fog.known_fleets.values_mut() {
        entry.uncertainty = (entry.uncertainty * factor).min(1.0);
    }
    world.recompute_outcome_hash();
}


/// True if a and b share a non-defaulted active contract of `kind`.
pub fn has_active_contract(
    world: &World,
    a: EmpireId,
    b: EmpireId,
    kind: ContractKind,
) -> bool {
    let Some(ca) = world.contact.get(a) else {
        return false;
    };
    ca.contracts.iter().any(|c| {
        !c.defaulted
            && c.end_tick.is_none()
            && c.kind == kind
            && ((c.a == a && c.b == b) || (c.a == b && c.b == a))
    })
}

/// OpenPassage: traveler may freely enter systems associated with sovereign's fog capitals.
/// Stub: true when an active OpenPassage treaty exists between the pair (symmetric).
pub fn open_passage_allows(world: &World, traveler: EmpireId, sovereign: EmpireId) -> bool {
    if traveler == sovereign {
        return true;
    }
    has_clause(world, traveler, sovereign, TreatyClause::OpenPassage)
}

/// Fulfill SalvageRights: client (`a`) gets wreck KO via salvage_contact; both get fog on system.
pub fn fulfill_salvage_rights(
    world: &mut World,
    contract_id: EntityId,
    wreck_system: EntityId,
    claim: impl Into<String>,
) -> Option<EntityId> {
    let claim = claim.into();
    let mut parties: Option<(EmpireId, EmpireId)> = None;
    for contact in world.contact.empires.values() {
        if let Some(con) = contact.contracts.iter().find(|c| c.id == contract_id) {
            if con.defaulted || con.end_tick.is_some() || con.kind != ContractKind::SalvageRights {
                return None;
            }
            parties = Some((con.a, con.b));
            break;
        }
    }
    let (a, b) = parties?;
    let ko = crate::knowledge::salvage_contact(world, a, wreck_system, claim);
    grant_fog(world, a, wreck_system);
    grant_fog(world, b, wreck_system);
    push_contact_hot_empires(world, a, b);
    world.recompute_outcome_hash();
    Some(ko)
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

    #[test]
    fn sign_and_default_contract() {
        let mut w = World::new(20);
        let a = EmpireId(1);
        let b = EmpireId(2);
        let id = sign_contract(&mut w, a, b, ContractKind::Freight);
        assert_eq!(w.contact.get(a).unwrap().active_contracts().count(), 1);
        assert!(default_contract(&mut w, id));
        assert_eq!(w.contact.get(a).unwrap().active_contracts().count(), 0);
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::ContractDefault { contract_id, .. } if *contract_id == id
        )));
    }


    #[test]
    fn open_passage_grants_capital_fog() {
        let mut w = World::new(30);
        let sys = *w.ledger().systems().next().unwrap().0;
        let a = EmpireId(1);
        let b = EmpireId(2);
        // Mark system as A's capital
        if let Some(s) = w.ledger.get_mut(sys) {
            s.is_home_capital = true;
            s.home_empire = Some(a);
        }
        sign_treaty(&mut w, a, b, vec![TreatyClause::OpenPassage]);
        assert!(w
            .contact
            .get(b)
            .unwrap()
            .fog
            .known_systems
            .contains_key(&sys));
    }


    #[test]
    fn sense_fleet_updates_fog() {
        let mut w = World::new(50);
        let sys = *w.ledger().systems().next().unwrap().0;
        let obs = EmpireId(1);
        let fleet = EntityId(99);
        sense_fleet(&mut w, obs, fleet, Some(sys));
        let e = w.contact.get(obs).unwrap().fog.known_fleets.get(&fleet).unwrap();
        assert_eq!(e.last_known_tick, w.master_tick());
        assert_eq!(e.last_system, Some(sys));
        assert!(e.uncertainty < 1.0);
    }


    #[test]
    fn has_clause_non_aggression() {
        let mut w = World::new(60);
        let a = EmpireId(1);
        let b = EmpireId(2);
        assert!(!has_clause(&w, a, b, TreatyClause::NonAggression));
        sign_treaty(&mut w, a, b, vec![TreatyClause::NonAggression]);
        assert!(has_clause(&w, a, b, TreatyClause::NonAggression));
        assert!(has_clause(&w, b, a, TreatyClause::NonAggression));
    }


    #[test]
    fn reparations_breach_extra_standing() {
        let mut w = World::new(71);
        let a = EmpireId(1);
        let b = EmpireId(2);
        let id = sign_treaty(
            &mut w,
            a,
            b,
            vec![TreatyClause::NonAggression, TreatyClause::ReparationsStub],
        );
        // After sign: +STANDING_FIRST_CONTACT each way.
        assert!(break_treaty(&mut w, id, false));
        // Base break -2, plus reparations -10 each way => net from sign+break = -10.
        let st = w.standing.get(a, b);
        assert_eq!(st, -crate::politics::STANDING_REPARATIONS_BREACH);
    }

    #[test]
    fn mark_system_surveyed_sets_fuse() {
        let mut w = World::new(72);
        let system = *w.ledger().systems().next().unwrap().0;
        let e = EmpireId(3);
        assert!(mark_system_surveyed(&mut w, e, system));
        let fog = w.contact.get(e).unwrap().fog.known_systems.get(&system).unwrap();
        assert!(fog.surveyed_fuse);
        assert!(!mark_system_surveyed(&mut w, e, system));
    }


    #[test]
    fn fulfill_survey_charter_marks_both() {
        let mut w = World::new(90);
        let system = *w.ledger().systems().next().unwrap().0;
        let a = EmpireId(1);
        let b = EmpireId(2);
        let id = sign_contract(&mut w, a, b, ContractKind::SurveyCharter);
        assert!(fulfill_survey_charter(&mut w, id, system));
        for e in [a, b] {
            let fog = &w.contact.get(e).unwrap().fog.known_systems[&system];
            assert!(fog.surveyed_fuse);
        }
        // Freight contract cannot fulfill as survey.
        let freight = sign_contract(&mut w, a, b, ContractKind::Freight);
        assert!(!fulfill_survey_charter(&mut w, freight, system));
    }

    #[test]
    fn decay_fleet_fog_raises_uncertainty() {
        let mut w = World::new(91);
        let e = EmpireId(3);
        let fleet = EntityId(99);
        sense_fleet(&mut w, e, fleet, None);
        let u0 = w.contact.get(e).unwrap().fog.known_fleets[&fleet].uncertainty;
        decay_fleet_fog(&mut w, e, 2.0);
        let u1 = w.contact.get(e).unwrap().fog.known_fleets[&fleet].uncertainty;
        assert!(u1 > u0);
        assert!(u1 <= 1.0);
    }


    #[test]
    fn open_passage_and_salvage_rights() {
        let mut w = World::new(100);
        let system = *w.ledger().systems().next().unwrap().0;
        let a = EmpireId(1);
        let b = EmpireId(2);
        assert!(!open_passage_allows(&w, a, b));
        sign_treaty(&mut w, a, b, vec![TreatyClause::OpenPassage]);
        assert!(open_passage_allows(&w, a, b));
        assert!(open_passage_allows(&w, b, a));
        assert!(open_passage_allows(&w, a, a));

        let id = sign_contract(&mut w, a, b, ContractKind::SalvageRights);
        assert!(has_active_contract(&w, a, b, ContractKind::SalvageRights));
        let ko = fulfill_salvage_rights(&mut w, id, system, "rights salvage").unwrap();
        assert!(w.knowledge.get(ko).is_some());
        assert!(w.contact.get(a).unwrap().fog.known_systems.contains_key(&system));
        assert!(w.contact.get(b).unwrap().fog.known_systems.contains_key(&system));
        // Wrong kind rejected.
        let freight = sign_contract(&mut w, a, b, ContractKind::Freight);
        assert!(fulfill_salvage_rights(&mut w, freight, system, "nope").is_none());
    }


    #[test]
    fn push_fine_hot_stamps_ttl() {
        let mut w = World::new(110);
        let system = *w.ledger().systems().next().unwrap().0;
        let now = w.master_tick();
        push_fine_hot(&mut w, system);
        let sys = w.ledger().get(system).unwrap();
        assert_eq!(sys.lod_hint, crate::lod::LodHint::Hot);
        assert_eq!(
            sys.hot_until,
            Some(crate::lod::hot_until_tick(now, crate::lod::DEFAULT_HOT_TTL_TICKS))
        );
    }


    #[test]
    fn decay_system_fog_raises_uncertainty() {
        let mut w = World::new(120);
        let system = *w.ledger().systems().next().unwrap().0;
        let e = EmpireId(8);
        grant_fog(&mut w, e, system);
        let u0 = w.contact.get(e).unwrap().fog.known_systems[&system].uncertainty;
        decay_system_fog(&mut w, e, 2.0);
        let u1 = w.contact.get(e).unwrap().fog.known_systems[&system].uncertainty;
        assert!(u1 > u0);
        assert!(u1 <= 1.0);
    }


    #[test]
    fn share_fog_copies_known_system() {
        let mut w = World::new(130);
        let system = *w.ledger().systems().next().unwrap().0;
        let a = EmpireId(1);
        let b = EmpireId(2);
        assert!(!share_fog(&mut w, a, b, system));
        grant_fog(&mut w, a, system);
        mark_system_surveyed(&mut w, a, system);
        assert!(share_fog(&mut w, a, b, system));
        let fog_b = &w.contact.get(b).unwrap().fog.known_systems[&system];
        assert!(fog_b.surveyed_fuse);
        assert!(fog_b.uncertainty <= 0.25);
    }


    #[test]
    fn lose_fleet_contact_drops_entry() {
        let mut w = World::new(160);
        let e = EmpireId(3);
        let fleet = EntityId(77);
        sense_fleet(&mut w, e, fleet, None);
        assert!(w.contact.get(e).unwrap().fog.known_fleets.contains_key(&fleet));
        assert!(lose_fleet_contact(&mut w, e, fleet));
        assert!(!w.contact.get(e).unwrap().fog.known_fleets.contains_key(&fleet));
        assert!(!lose_fleet_contact(&mut w, e, fleet));
    }

}
