//! Phase H — Sensor stubs: last-known fix, strike detection, wreck/signal receive.
//!
//! Reserves envelopes so G fog and P KO paths are not retrofit. Range is a
//! coarse stub (`SENSOR_RANGE_STUB`); B/F can later bind real sensor modules.

use crate::contact::{push_fine_hot, SystemFogEntry};
use crate::entity::{EmpireId, EntityId};
use crate::knowledge::{acquire_ko, salvage_contact, KoKind};
use crate::world::World;

/// Coarse sensor range stub (system-graph distance units; unused until B wires
/// adjacency — for now "in range" means observer has fog on the system OR
/// `force_range` is true).
pub const SENSOR_RANGE_STUB: f64 = 1.0;

/// Refresh last-known fix for `system` in the observer's fog.
///
/// Creates a fog entry if missing. Reduces uncertainty toward 0.
pub fn sense_system(world: &mut World, observer: EmpireId, system: EntityId) {
    let tick = world.master_tick();
    let fog = &mut world.contact.ensure(observer).fog.known_systems;
    let entry = fog.entry(system).or_insert_with(|| SystemFogEntry::fresh(tick));
    entry.last_known_tick = tick;
    entry.uncertainty = (entry.uncertainty * 0.5).max(0.0);
    push_fine_hot(world, system);
}

/// Whether the observer's sensors cover `system` (stub).
///
/// True if they already have a fog entry, or if `force_in_range` is set (tests /
/// operator). Real adjacency comes later from B.
pub fn in_sensor_range(world: &World, observer: EmpireId, system: EntityId, force_in_range: bool) -> bool {
    if force_in_range {
        return true;
    }
    world
        .contact
        .get(observer)
        .map(|c| c.fog.known_systems.contains_key(&system))
        .unwrap_or(false)
}

/// Detect a strike / violence event at `system` if in range: refresh fog fix.
///
/// Returns true when detection occurred.
pub fn detect_strike(
    world: &mut World,
    observer: EmpireId,
    system: EntityId,
    force_in_range: bool,
) -> bool {
    if !in_sensor_range(world, observer, system, force_in_range) {
        return false;
    }
    sense_system(world, observer, system);
    true
}

/// Receive a signal KO if in range of its payload system (or force).
///
/// On success, acquires the KO for the observer (Lock 9 witness path).
pub fn receive_signal(
    world: &mut World,
    observer: EmpireId,
    ko_id: EntityId,
    force_in_range: bool,
) -> bool {
    let (kind, system) = match world.knowledge.get(ko_id) {
        Some(ko) => (ko.kind, ko.payload.system),
        None => return false,
    };
    if !matches!(kind, KoKind::Signal) {
        return false;
    }
    if let Some(sys) = system {
        if !in_sensor_range(world, observer, sys, force_in_range) {
            return false;
        }
        sense_system(world, observer, sys);
    } else if !force_in_range {
        return false;
    }
    acquire_ko(world, observer, ko_id)
}

/// Discover a wreck at `system` via sensors → salvage_contact (always grants wreck KO).
pub fn discover_wreck(
    world: &mut World,
    observer: EmpireId,
    system: EntityId,
    force_in_range: bool,
) -> Option<EntityId> {
    if !in_sensor_range(world, observer, system, force_in_range) {
        return None;
    }
    sense_system(world, observer, system);
    Some(salvage_contact(
        world,
        observer,
        system,
        "sensor wreck discovery",
    ))
}


/// For each Signal KO whose payload.system is in sensor range, try receive_signal.
/// Returns how many KOs were newly acquired.
pub fn poll_signal_envelope(
    world: &mut World,
    observer: EmpireId,
    force_in_range: bool,
) -> usize {
    let candidates: Vec<EntityId> = world
        .knowledge
        .iter()
        .filter(|(_, ko)| matches!(ko.kind, KoKind::Signal))
        .map(|(id, _)| *id)
        .collect();
    let mut n = 0;
    for ko_id in candidates {
        if receive_signal(world, observer, ko_id, force_in_range) {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EmpireId;
    use crate::knowledge::{emit_ko, EmitKoParams, KoGrade, KoKind, KoPayload, KoPropagation};
    use crate::world::World;
    use std::collections::BTreeSet;

    #[test]
    fn sense_system_updates_fog_last_known() {
        let mut w = World::new(1);
        let sys = *w.ledger().systems().next().unwrap().0;
        let obs = EmpireId(1);
        sense_system(&mut w, obs, sys);
        let e = w.contact.get(obs).unwrap().fog.known_systems.get(&sys).unwrap();
        assert_eq!(e.last_known_tick, w.master_tick());
        assert!(e.uncertainty < 1.0);
    }

    #[test]
    fn detect_strike_requires_range_unless_forced() {
        let mut w = World::new(2);
        let sys = *w.ledger().systems().next().unwrap().0;
        let obs = EmpireId(2);
        assert!(!detect_strike(&mut w, obs, sys, false));
        assert!(detect_strike(&mut w, obs, sys, true));
        assert!(w.contact.get(obs).unwrap().fog.known_systems.contains_key(&sys));
    }

    #[test]
    fn receive_signal_acquires_when_in_range() {
        let mut w = World::new(3);
        let sys = *w.ledger().systems().next().unwrap().0;
        let obs = EmpireId(3);
        let ko = emit_ko(
            &mut w,
            EmitKoParams {
                kind: KoKind::Signal,
                grade: KoGrade::Rumor,
                origin_event_seq: None,
                payload: KoPayload {
                    who_actor: None,
                    who_victim: None,
                    system: Some(sys),
                    severity: 1,
                    target_type: "signal".into(),
                    claim: "ping".into(),
                },
                initial_carriers: BTreeSet::new(),
                propagation: KoPropagation::Broadcast,
            },
        );
        assert!(!receive_signal(&mut w, obs, ko, false));
        assert!(receive_signal(&mut w, obs, ko, true));
        assert!(w
            .knowledge
            .get(ko)
            .unwrap()
            .carriers
            .iter()
            .any(|c| matches!(c, crate::knowledge::CarrierId::Empire(e) if *e == obs)));
    }

    #[test]
    fn discover_wreck_grants_ko() {
        let mut w = World::new(4);
        let sys = *w.ledger().systems().next().unwrap().0;
        let obs = EmpireId(4);
        let ko = discover_wreck(&mut w, obs, sys, true).expect("wreck ko");
        assert_eq!(w.knowledge.get(ko).unwrap().kind, KoKind::Wreck);
    }

    #[test]
    fn poll_signal_envelope_acquires() {
        let mut w = World::new(5);
        let sys = *w.ledger().systems().next().unwrap().0;
        let obs = EmpireId(5);
        sense_system(&mut w, obs, sys);
        let ko = emit_ko(
            &mut w,
            EmitKoParams {
                kind: KoKind::Signal,
                grade: KoGrade::Rumor,
                origin_event_seq: None,
                payload: KoPayload {
                    who_actor: None,
                    who_victim: None,
                    system: Some(sys),
                    severity: 1,
                    target_type: "signal".into(),
                    claim: "burst".into(),
                },
                initial_carriers: BTreeSet::new(),
                propagation: KoPropagation::Broadcast,
            },
        );
        assert_eq!(poll_signal_envelope(&mut w, obs, false), 1);
        assert!(w
            .knowledge
            .get(ko)
            .unwrap()
            .carriers
            .iter()
            .any(|c| matches!(c, crate::knowledge::CarrierId::Empire(e) if *e == obs)));
    }

}
