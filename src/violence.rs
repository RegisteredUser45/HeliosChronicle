//! Phase H — Violence: planetary layer writes, Salt, KO emission, hull damage.
//!
//! Writes the same five EnvLayers columns as D (`apply_layer_burst`).
//! Ship instance damage uses F [`ShipInstance::damage`] via [`apply_hull_damage`].

use std::collections::BTreeSet;

use crate::contact::push_fine_hot;
use crate::entity::{EmpireId, EntityId, EnvLayers};
use crate::event::EventKind;
use crate::knowledge::{
    emit_ko, EmitKoParams, KoGrade, KoKind, KoPayload, KoPropagation,
};
use crate::politics::apply_event_for_standing;
use crate::world::World;
use crate::worlds::apply_layer_burst;

/// Stub Salt layer deltas (Lead: salt causes high-rate layer writes).
/// Rates are placeholders until balance pass — not final design numbers.
pub const SALT_RADIATION_DELTA: f64 = 20.0;
pub const SALT_TOXINS_DELTA: f64 = 10.0;
pub const SALT_BIOSPHERE_DELTA: f64 = -0.8;
pub const SALT_TEMPERATURE_DELTA: f64 = 50.0;
/// Salt severity stub (drives RefugeeWave / Signal emission).
pub const SALT_SEVERITY: u8 = 10;
/// Minimum severity for RefugeeWave / Signal KO emission on strikes.
pub const KO_SEVERITY_THRESHOLD: u8 = 1;

/// Which typed violence event to append for a layer strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrikeKind {
    OrbitalStrike,
    BombardmentLayerWrite,
    SurfaceCombat,
    GlassAttempt,
}

impl StrikeKind {
    fn to_event(self, actor: EmpireId, victim: EmpireId, system: EntityId) -> EventKind {
        match self {
            StrikeKind::OrbitalStrike => EventKind::OrbitalStrike {
                actor,
                victim,
                system,
            },
            StrikeKind::BombardmentLayerWrite => EventKind::BombardmentLayerWrite {
                actor,
                victim,
                system,
            },
            StrikeKind::SurfaceCombat => EventKind::SurfaceCombat {
                actor,
                victim,
                system,
            },
            StrikeKind::GlassAttempt => EventKind::GlassAttempt {
                actor,
                victim,
                system,
            },
        }
    }

    fn target_type(self) -> &'static str {
        match self {
            StrikeKind::OrbitalStrike => "orbital_strike",
            StrikeKind::BombardmentLayerWrite => "bombardment",
            StrikeKind::SurfaceCombat => "surface_combat",
            StrikeKind::GlassAttempt => "glass_attempt",
        }
    }
}

/// Outcome of a successful layer-write violence action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViolenceOutcome {
    pub event_seq: u64,
    pub body_id: EntityId,
    pub refugee_ko: Option<EntityId>,
    pub signal_ko: Option<EntityId>,
}

/// Errors from violence resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolenceError {
    BodyNotFound {
        body_id: EntityId,
    },
    BodySystemMismatch {
        body_id: EntityId,
        expected_system: EntityId,
        actual_system: EntityId,
    },
    ShipNotFound {
        ship_id: EntityId,
    },
}

/// High-rate Salt EnvLayers delta (stub constants).
pub fn salt_layer_delta() -> EnvLayers {
    EnvLayers {
        atmosphere_pressure: 0.0,
        temperature: SALT_TEMPERATURE_DELTA,
        radiation: SALT_RADIATION_DELTA,
        toxins_fallout: SALT_TOXINS_DELTA,
        biosphere: SALT_BIOSPHERE_DELTA,
    }
}

fn strike_severity(delta: &EnvLayers) -> u8 {
    let mag = delta.radiation.abs()
        + delta.toxins_fallout.abs()
        + delta.biosphere.abs() * 10.0
        + delta.temperature.abs() / 10.0
        + delta.atmosphere_pressure.abs();
    mag.round().clamp(0.0, 255.0) as u8
}

fn resolve_body(
    world: &World,
    system: EntityId,
    body_id: EntityId,
) -> Result<(), ViolenceError> {
    let Some(body) = world.ledger.get_body(body_id) else {
        return Err(ViolenceError::BodyNotFound { body_id });
    };
    if body.system != system {
        return Err(ViolenceError::BodySystemMismatch {
            body_id,
            expected_system: system,
            actual_system: body.system,
        });
    }
    Ok(())
}

fn emit_violence_kos(
    world: &mut World,
    actor: EmpireId,
    victim: EmpireId,
    system: EntityId,
    origin_event_seq: u64,
    severity: u8,
    pops_before: f64,
    target_type: &str,
    claim: &str,
    emit_refugee: bool,
) -> (Option<EntityId>, Option<EntityId>) {
    if severity < KO_SEVERITY_THRESHOLD {
        return (None, None);
    }

    let base_payload = KoPayload {
        who_actor: Some(actor),
        who_victim: Some(victim),
        system: Some(system),
        severity,
        target_type: target_type.into(),
        claim: claim.into(),
    };

    // Empty initial carriers: victims auto-know via standing; witnesses need acquire_ko.
    let refugee_ko = if emit_refugee && pops_before > 0.0 {
        Some(emit_ko(
            world,
            EmitKoParams {
                kind: KoKind::RefugeeWave,
                grade: KoGrade::Rumor,
                origin_event_seq: Some(origin_event_seq),
                payload: KoPayload {
                    claim: format!("{claim} refugees"),
                    ..base_payload.clone()
                },
                initial_carriers: BTreeSet::new(),
                propagation: KoPropagation::EvacConvoy,
            },
        ))
    } else {
        None
    };

    let signal_ko = Some(emit_ko(
        world,
        EmitKoParams {
            kind: KoKind::Signal,
            grade: KoGrade::Rumor,
            origin_event_seq: Some(origin_event_seq),
            payload: base_payload,
            initial_carriers: BTreeSet::new(),
            propagation: KoPropagation::Broadcast,
        },
    ));

    (refugee_ko, signal_ko)
}


fn notify_sensors_of_strike(world: &mut World, system: EntityId) {
    let observers: Vec<_> = world
        .contact
        .empires
        .iter()
        .filter(|(_, c)| c.fog.known_systems.contains_key(&system))
        .map(|(id, _)| *id)
        .collect();
    for obs in observers {
        let _ = crate::sensors::detect_strike(world, obs, system, false);
    }
}

/// Apply a planetary layer strike: write EnvLayers, chronicle event, fine-hot,
/// victim standing, and rumor KOs (no auto-acquire for witnesses).
pub fn strike_layers(
    world: &mut World,
    actor: EmpireId,
    victim: EmpireId,
    system: EntityId,
    body_id: EntityId,
    kind: StrikeKind,
    delta: EnvLayers,
) -> Result<ViolenceOutcome, ViolenceError> {
    resolve_body(world, system, body_id)?;

    let pops_before = world
        .ledger
        .get_body(body_id)
        .map(|b| b.pops)
        .unwrap_or(0.0);
    let severity = strike_severity(&delta);

    {
        let body = world
            .ledger
            .get_body_mut(body_id)
            .expect("body validated");
        apply_layer_burst(body, &delta);
    }

    let tick = world.master_tick();
    let ev = world
        .log
        .append(tick, kind.to_event(actor, victim, system));
    let event_seq = ev.seq;
    let chronicle = ev.clone();

    push_fine_hot(world, system);
    apply_event_for_standing(world, &chronicle);
    notify_sensors_of_strike(world, system);

    let (refugee_ko, signal_ko) = emit_violence_kos(
        world,
        actor,
        victim,
        system,
        event_seq,
        severity,
        pops_before,
        kind.target_type(),
        kind.target_type(),
        true,
    );

    // Hull damage is ship-targeted — use [`apply_hull_damage`] separately.

    world.recompute_outcome_hash();
    Ok(ViolenceOutcome {
        event_seq,
        body_id,
        refugee_ko,
        signal_ko,
    })
}

/// First-class Salt: high-rate layer writes + Salt event + standing + KOs.
pub fn salt_world(
    world: &mut World,
    actor: EmpireId,
    victim: EmpireId,
    system: EntityId,
    body_id: EntityId,
) -> Result<ViolenceOutcome, ViolenceError> {
    resolve_body(world, system, body_id)?;

    let pops_before = world
        .ledger
        .get_body(body_id)
        .map(|b| b.pops)
        .unwrap_or(0.0);
    let delta = salt_layer_delta();

    {
        let body = world
            .ledger
            .get_body_mut(body_id)
            .expect("body validated");
        apply_layer_burst(body, &delta);
    }

    let tick = world.master_tick();
    let ev = world.log.append(
        tick,
        EventKind::Salt {
            actor,
            victim,
            system,
        },
    );
    let event_seq = ev.seq;
    let chronicle = ev.clone();

    push_fine_hot(world, system);
    apply_event_for_standing(world, &chronicle);
    notify_sensors_of_strike(world, system);

    let (refugee_ko, signal_ko) = emit_violence_kos(
        world,
        actor,
        victim,
        system,
        event_seq,
        SALT_SEVERITY,
        pops_before,
        "salt",
        "salt",
        true,
    );

    // Hull damage is ship-targeted — use [`apply_hull_damage`] separately.

    world.recompute_outcome_hash();
    Ok(ViolenceOutcome {
        event_seq,
        body_id,
        refugee_ko,
        signal_ko,
    })
}

/// Apply damage to an F [`crate::hulls::ShipInstance`] on `world.ships`.
///
/// `amount` is added to `ShipInstance.damage` and clamped to `[0.0, 1.0]`
/// (`can_move` treats `damage >= 1.0` as immobilized). Returns the new damage.
/// Layer strikes do not auto-call this — pass an explicit ship id.
pub fn apply_hull_damage(
    world: &mut World,
    ship_id: EntityId,
    amount: f64,
) -> Result<f64, ViolenceError> {
    let ship = world
        .ships
        .get_mut(&ship_id)
        .ok_or(ViolenceError::ShipNotFound { ship_id })?;
    let new = crate::hulls::apply_ship_damage(ship, amount);
    // Immobilized / wrecked: emit a wreck KO (Lock 9 knowledge object).
    if new >= 1.0 {
        let _ = crate::knowledge::emit_ko(
            world,
            crate::knowledge::EmitKoParams {
                kind: crate::knowledge::KoKind::Wreck,
                grade: crate::knowledge::KoGrade::Confirmed,
                origin_event_seq: None,
                payload: crate::knowledge::KoPayload {
                    who_actor: None,
                    who_victim: None,
                    system: None,
                    severity: 5,
                    target_type: "wrecked_ship".into(),
                    claim: format!("ship {ship_id} wrecked"),
                },
                initial_carriers: Default::default(),
                propagation: crate::knowledge::KoPropagation::DerelictScan,
            },
        );
    }
    world.recompute_outcome_hash();
    Ok(new)
}

/// Backward-compatible alias — prefer [`apply_hull_damage`].

/// Layer strike plus optional ship hull damage (F).
pub fn strike_with_ship(
    world: &mut World,
    actor: EmpireId,
    victim: EmpireId,
    system: EntityId,
    body_id: EntityId,
    kind: StrikeKind,
    delta: EnvLayers,
    ship_id: Option<EntityId>,
    ship_damage: f64,
) -> Result<(ViolenceOutcome, Option<f64>), ViolenceError> {
    let out = strike_layers(world, actor, victim, system, body_id, kind, delta)?;
    let dmg = if let Some(sid) = ship_id {
        Some(apply_hull_damage(world, sid, ship_damage)?)
    } else {
        None
    };
    Ok((out, dmg))
}

#[deprecated(note = "use apply_hull_damage")]
pub fn apply_hull_damage_stub(
    world: &mut World,
    ship_id: EntityId,
    amount: f64,
) -> Result<(), ViolenceError> {
    apply_hull_damage(world, ship_id, amount).map(|_| ())
}

/// System-only Salt path used when no body is available (backward-compatible).
pub(crate) fn salt_system_only(
    world: &mut World,
    actor: EmpireId,
    victim: EmpireId,
    system: EntityId,
) {
    let tick = world.master_tick();
    push_fine_hot(world, system);
    let ev = world.log.append(
        tick,
        EventKind::Salt {
            actor,
            victim,
            system,
        },
    );
    let chronicle = ev.clone();
    apply_event_for_standing(world, &chronicle);
    world.recompute_outcome_hash();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{acquire_ko, KoKind};
    use crate::lod::LodHint;
    use crate::politics::{STANDING_RUMOR, STANDING_SALT_VICTIM, STANDING_VIOLENCE_VICTIM};
    use crate::worlds::compute_deficits;

    fn setup_with_body(pops: f64) -> (World, EmpireId, EmpireId, EntityId, EntityId) {
        let mut w = World::new(201);
        let actor = EmpireId(1);
        let victim = EmpireId(2);
        let system = *w.ledger().systems().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        w.ledger.get_body_mut(body_id).unwrap().pops = pops;
        (w, actor, victim, system, body_id)
    }

    #[test]
    fn layer_strike_writes_all_five_columns() {
        let (mut w, actor, victim, system, body_id) = setup_with_body(100.0);
        let before = w.ledger.get_body(body_id).unwrap().layers.clone();
        let delta = EnvLayers {
            atmosphere_pressure: -0.2,
            temperature: 15.0,
            radiation: 4.0,
            toxins_fallout: 2.5,
            biosphere: -0.3,
        };
        let out = strike_layers(
            &mut w,
            actor,
            victim,
            system,
            body_id,
            StrikeKind::BombardmentLayerWrite,
            delta.clone(),
        )
        .unwrap();
        let after = &w.ledger.get_body(body_id).unwrap().layers;
        assert!((after.atmosphere_pressure - (before.atmosphere_pressure + delta.atmosphere_pressure)).abs() < 1e-9);
        assert!((after.temperature - (before.temperature + delta.temperature)).abs() < 1e-9);
        assert!((after.radiation - (before.radiation + delta.radiation)).abs() < 1e-9);
        assert!((after.toxins_fallout - (before.toxins_fallout + delta.toxins_fallout)).abs() < 1e-9);
        assert!((after.biosphere - (before.biosphere + delta.biosphere)).abs() < 1e-9);
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::BombardmentLayerWrite { .. }
        )));
        assert_eq!(out.body_id, body_id);
        // Optional post-write deficit check still runs.
        let _ = compute_deficits(
            w.ledger.get_body(body_id).unwrap(),
            &w.globals.envelope,
        );
    }

    #[test]
    fn salt_high_rate_writes_event_and_victim_standing_no_ko_acquire() {
        let (mut w, actor, victim, system, body_id) = setup_with_body(50.0);
        let witness = EmpireId(3);
        let before = w.ledger.get_body(body_id).unwrap().layers.clone();
        let out = salt_world(&mut w, actor, victim, system, body_id).unwrap();
        let after = &w.ledger.get_body(body_id).unwrap().layers;
        assert!((after.radiation - (before.radiation + SALT_RADIATION_DELTA)).abs() < 1e-9);
        assert!((after.toxins_fallout - (before.toxins_fallout + SALT_TOXINS_DELTA)).abs() < 1e-9);
        assert!((after.biosphere - (before.biosphere + SALT_BIOSPHERE_DELTA)).abs() < 1e-9);
        assert!((after.temperature - (before.temperature + SALT_TEMPERATURE_DELTA)).abs() < 1e-9);
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::Salt { actor: a, victim: v, system: s }
                if *a == actor && *v == victim && *s == system
        )));
        assert_eq!(w.standing.get(victim, actor), -STANDING_SALT_VICTIM);
        // Witness has not acquired KO — no standing change.
        assert_eq!(w.standing.get(witness, actor), 0);
        assert!(out.refugee_ko.is_some());
        assert!(out.signal_ko.is_some());
    }

    #[test]
    fn ko_emitted_witness_standing_only_after_acquire() {
        let (mut w, actor, victim, system, body_id) = setup_with_body(80.0);
        let witness = EmpireId(9);
        let out = salt_world(&mut w, actor, victim, system, body_id).unwrap();
        let refugee = out.refugee_ko.expect("RefugeeWave");
        let signal = out.signal_ko.expect("Signal");
        assert_eq!(w.knowledge.get(refugee).unwrap().kind, KoKind::RefugeeWave);
        assert_eq!(w.knowledge.get(signal).unwrap().kind, KoKind::Signal);
        assert!(w.knowledge.get(refugee).unwrap().carriers.is_empty());
        assert_eq!(w.standing.get(witness, actor), 0);
        assert!(acquire_ko(&mut w, witness, refugee));
        assert_eq!(w.standing.get(witness, actor), -STANDING_RUMOR - SALT_SEVERITY as i32);
    }

    #[test]
    fn strike_push_fine_hot_and_violence_standing() {
        let (mut w, actor, victim, system, body_id) = setup_with_body(10.0);
        w.ledger.get_mut(system).unwrap().lod_hint = LodHint::Quiet;
        let delta = EnvLayers {
            atmosphere_pressure: 0.0,
            temperature: 0.0,
            radiation: 5.0,
            toxins_fallout: 1.0,
            biosphere: -0.1,
        };
        strike_layers(
            &mut w,
            actor,
            victim,
            system,
            body_id,
            StrikeKind::OrbitalStrike,
            delta,
        )
        .unwrap();
        assert_eq!(w.ledger().get(system).unwrap().lod_hint, LodHint::Hot);
        assert_eq!(
            w.standing.get(victim, actor),
            -STANDING_VIOLENCE_VICTIM
        );
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::OrbitalStrike { .. }
        )));
    }

    #[test]
    fn hull_damage_applies_to_ship_instance() {
        use crate::hulls::{can_move, make_design, spawn_instance};

        let mut w = World::new(202);
        let design = make_design(
            EntityId(900),
            "gunboat",
            vec![
                "module.engine_chem".into(),
                "module.crew_habitat".into(),
            ],
        )
        .unwrap();
        let ship_id = EntityId(901);
        let inst = spawn_instance(ship_id, &design, 5.0);
        assert!(can_move(&design, &inst));
        w.ship_designs.insert(design.id, design.clone());
        w.ships.insert(ship_id, inst);

        let new = apply_hull_damage(&mut w, ship_id, 0.4).unwrap();
        assert!((new - 0.4).abs() < f64::EPSILON);
        assert!((w.ships.get(&ship_id).unwrap().damage - 0.4).abs() < f64::EPSILON);
        assert!(can_move(&design, w.ships.get(&ship_id).unwrap()));

        let new = apply_hull_damage(&mut w, ship_id, 0.7).unwrap();
        assert!((new - 1.0).abs() < f64::EPSILON); // clamped
        assert!(!can_move(&design, w.ships.get(&ship_id).unwrap()));

        let missing = EntityId(9999);
        assert_eq!(
            apply_hull_damage(&mut w, missing, 0.1).unwrap_err(),
            ViolenceError::ShipNotFound { ship_id: missing }
        );
    }

    #[test]
    #[test]
    fn hull_damage_emits_wreck_ko_when_immobilized() {
        use crate::hulls::{make_design, spawn_instance};
        use crate::knowledge::KoKind;

        let mut w = World::new(204);
        let design = make_design(
            EntityId(910),
            "target",
            vec!["module.engine_chem".into(), "module.crew_habitat".into()],
        )
        .unwrap();
        let ship_id = EntityId(911);
        w.ship_designs.insert(design.id, design.clone());
        w.ships.insert(ship_id, spawn_instance(ship_id, &design, 5.0));
        let before = w.knowledge.len();
        let dmg = apply_hull_damage(&mut w, ship_id, 1.0).unwrap();
        assert!((dmg - 1.0).abs() < f64::EPSILON);
        assert!(w.knowledge.len() > before);
        assert!(w
            .knowledge
            .iter()
            .any(|(_, ko)| ko.kind == KoKind::Wreck));
    }


    #[test]
    fn strike_refreshes_observer_fog() {
        use crate::contact::grant_fog;

        let mut w = World::new(40);
        let system = *w.ledger().systems().next().unwrap().0;
        let body = w.ledger.spawn_body(system);
        let observer = EmpireId(9);
        grant_fog(&mut w, observer, system);
        let before = w.contact.get(observer).unwrap().fog.known_systems[&system].last_known_tick;
        w.tick(1);
        strike_layers(
            &mut w,
            EmpireId(1),
            EmpireId(2),
            system,
            body,
            StrikeKind::OrbitalStrike,
            EnvLayers {
                atmosphere_pressure: 0.0,
                temperature: 0.0,
                radiation: 2.0,
                toxins_fallout: 0.0,
                biosphere: 0.0,
            },
        )
        .unwrap();
        let after = w.contact.get(observer).unwrap().fog.known_systems[&system].last_known_tick;
        assert!(after >= before);
        assert_eq!(after, w.master_tick());
    }
    fn body_system_mismatch_errors() {
        let mut w = World::new(203);
        let systems: Vec<_> = w.ledger().systems().map(|(id, _)| *id).collect();
        assert!(systems.len() >= 2);
        let body_id = w.ledger.spawn_body(systems[0]);
        let err = strike_layers(
            &mut w,
            EmpireId(1),
            EmpireId(2),
            systems[1],
            body_id,
            StrikeKind::SurfaceCombat,
            EnvLayers::default(),
        )
        .unwrap_err();
        assert!(matches!(err, ViolenceError::BodySystemMismatch { .. }));
    }

    #[test]
    fn zero_pops_skips_refugee_ko_still_emits_signal() {
        let (mut w, actor, victim, system, body_id) = setup_with_body(0.0);
        let out = salt_world(&mut w, actor, victim, system, body_id).unwrap();
        assert!(out.refugee_ko.is_none());
        assert!(out.signal_ko.is_some());
    }
}
