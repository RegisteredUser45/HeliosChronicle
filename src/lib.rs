//! Helios Chronicle — Phase A kernel + G/P stubs + Phase I minds + B sky.
//!
//! Tick loop, deterministic seed, event log, save/load, entity ledger,
//! operator R/W, stub LOD, knowledge objects, contact fog, standing,
//! Phase I doctrine/orders, Phase B sky, Phase D worlds, Phase E/F schema stubs,
//! Phase H layer-writes, sensors stubs, F hull damage + wreck KO.

pub mod contact;
pub mod cosmology;
pub mod entity;
pub mod event;
pub mod globals;
pub mod knowledge;
pub mod lod;
pub mod matter;
pub mod minds;
pub mod operator;
pub mod politics;
pub mod save;
pub mod sky;
pub mod world;
pub mod worlds;
pub mod violence;
pub mod sensors;
pub mod research;
pub mod hulls;

pub use contact::{
    break_treaty, default_contract, empire_contact_active, first_contact, grant_fog, push_fine_hot,
    sign_contract, sign_treaty, upgrade_fog, Contract, ContractKind, EmpireContact,
    EmpireContactStore, FogState, SystemFogEntry, Treaty, TreatyClause,
};
pub use entity::{
    BodyEntity, EmpireEntity, EmpireId, EntityId, EntityLedger, EnvLayers, OrderEntity,
    OrderIntent, OrderSource, OrderStatus, SystemEntity,
};
pub use event::{ChronicleEvent, EventKind, EventLog, DEFAULT_MAX_LOG_EVENTS};
pub use globals::Globals;
pub use knowledge::{
    acquire_ko, confirm_ko, emit_ko, inject_rumor, salvage_contact, CarrierId, EmitKoParams,
    KnowledgeObject, KnowledgeStore, KoGrade, KoKind, KoPayload, KoPropagation,
};
pub use lod::{LodHint, LodMode};
pub use matter::{
    add_deposit, compute_binding_remainder, deposit_counts_as_binding, extract,
    extract_abandoned_auto, extract_civilian, extract_foreign, extract_state,
    reaggregate_and_check, salvage_into_feed, salvage_into_feed_stock, seed_deposits_from_binding_stocks, tick_abandoned_automation,
    tick_civilian_extractors, try_run_recipe, CivilianLine, Deposit, ExtractorKind, MatterError,
};
pub use minds::{clamp_doctrine, MindsFlags};
pub use operator::{Operator, OperatorError};
pub use politics::{
    apply_event_for_standing, emit_salt, StandingStore, STANDING_CONFIRMED, STANDING_FIRST_CONTACT,
    STANDING_RUMOR, STANDING_SALT_VICTIM, STANDING_VIOLENCE_VICTIM,
};
pub use save::{load_world, save_world, SaveError};
pub use world::World;
pub use worlds::{apply_layer_burst, apply_life_support_drain, apply_pop_deficits, compute_deficits, evacuate_body, life_support_bill, DeficitReport};
pub use sensors::{
    detect_strike, discover_wreck, in_sensor_range, receive_signal, sense_system,
    SENSOR_RANGE_STUB,
};
pub use violence::{
    apply_hull_damage, salt_layer_delta, salt_world, strike_layers, strike_with_ship, StrikeKind,
    ViolenceError, ViolenceOutcome, KO_SEVERITY_THRESHOLD, SALT_BIOSPHERE_DELTA,
    SALT_RADIATION_DELTA, SALT_SEVERITY, SALT_TEMPERATURE_DELTA, SALT_TOXINS_DELTA,
};
pub use research::{assign_lab, material_gates_met, set_lab_site, empire_has_unlock, find_segment, gates_ref_catalog, line_prereqs_met, make_lab, salvage_unlock_segment, segment_rp_cost, stub_tech_book, tick_all_labs, tick_lab, tick_lab_on_world, unlock_segment, Lab, TechCategory, TechLine, TechSegment};
pub use hulls::{apply_ship_damage, build_ship, build_ship_at_system, can_move, derive_fuel_tier, fuel_recipe_for_tier, load_magazine, spend_magazine, make_design, refine_fuel_at_system, refine_ship_tier_fuel, refuel, refuel_from_system, register_design, spend_fuel, spawn_instance, tool_yard, tool_yard_at_system, HullError, ShipDesign, ShipInstance};
pub use globals::{Band, SpeciesEnvelope};

/// Run two worlds from the same seed for `ticks` and return whether outcomes match.
///
/// **Verify path:** `helios verify --seed S -n N` or call this helper / assert
/// `World::new(s).tick(n).outcome_hash()` equality across two instances.
pub fn verify_determinism(seed: u64, ticks: u64) -> (bool, u64, u64) {
    let mut a = World::new(seed);
    let mut b = World::new(seed);
    a.tick(ticks);
    b.tick(ticks);
    let ha = a.outcome_hash();
    let hb = b.outcome_hash();
    (ha == hb && a == b, ha, hb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lod::LodHint;

    #[test]
    fn tick_advances_master_time() {
        let mut w = World::new(1);
        assert_eq!(w.master_tick(), 0);
        w.tick(5);
        assert_eq!(w.master_tick(), 5);
        assert!(w
            .log()
            .events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::TickAdvanced { .. })));
    }

    #[test]
    fn same_seed_same_outcomes() {
        let (ok, ha, hb) = verify_determinism(42, 25);
        assert!(ok, "hashes diverged: {ha} vs {hb}");
        let mut a = World::new(1);
        let mut b = World::new(2);
        a.tick(10);
        b.tick(10);
        assert_ne!(a.outcome_hash(), b.outcome_hash());
    }

    #[test]
    fn event_log_is_append_only() {
        let mut w = World::new(7);
        let before = w.log().len();
        w.tick(3);
        assert!(w.log().len() > before);
        let seqs: Vec<_> = w.log().events().iter().map(|e| e.seq).collect();
        let mut sorted = seqs.clone();
        sorted.sort_unstable();
        assert_eq!(seqs, sorted);
        assert_eq!(seqs.first().copied().unwrap_or(0), 0);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slot.json");

        let mut w = World::new(99);
        w.tick(8);
        let tick_before = w.master_tick();
        let systems_before = w.ledger().len();
        let draws_before = w.rng_draws;

        save_world(&mut w, &path).unwrap();

        // File contains post-save world; deserialize without the load-side event.
        let raw = std::fs::read_to_string(&path).unwrap();
        let restored: World = serde_json::from_str(&raw).unwrap();
        assert_eq!(restored.seed(), 99);
        assert_eq!(restored.master_tick(), tick_before);
        assert_eq!(restored.ledger().len(), systems_before);
        assert_eq!(restored.rng_draws, draws_before);
        assert_eq!(restored.ledger, w.ledger);
        assert_eq!(restored.ledger.empires_len(), w.ledger.empires_len());
        assert_eq!(restored.ledger.orders_len(), w.ledger.orders_len());
        assert_eq!(restored.globals, w.globals);
        assert_eq!(restored.knowledge, w.knowledge);
        assert_eq!(restored.contact, w.contact);
        assert_eq!(restored.standing, w.standing);

        // Continuing from two independent restores stays deterministic.
        let mut a: World = serde_json::from_str(&raw).unwrap();
        let mut b: World = serde_json::from_str(&raw).unwrap();
        a.recompute_outcome_hash();
        b.recompute_outcome_hash();
        a.tick(5);
        b.tick(5);
        assert_eq!(a.outcome_hash(), b.outcome_hash());
        assert_eq!(a, b);

        // load_world adds a WorldLoaded event but preserves ledger/tick.
        let loaded = load_world(&path).unwrap();
        assert_eq!(loaded.master_tick(), tick_before);
        assert_eq!(loaded.ledger, restored.ledger);
        assert!(loaded
            .log()
            .events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::WorldLoaded { .. })));
    }

    #[test]
    fn operator_mutation_is_event() {
        let mut w = World::new(5);
        let id = *w.ledger().systems().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            op.set_field(id, "binding_remainder", "42.5").unwrap();
        }
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::OperatorMutation { field, new, .. }
                if field == "binding_remainder" && new == "42.5"
        )));
        assert!((w.ledger().get(id).unwrap().binding_remainder - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn shared_ledger_tick_and_operator() {
        let mut w = World::new(11);
        let id = *w.ledger().systems().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            op.set_field(id, "lod_hint", "hot").unwrap();
        }
        assert_eq!(w.ledger().get(id).unwrap().lod_hint, LodHint::Hot);
        w.tick(1);
        assert_eq!(w.ledger().get(id).unwrap().lod_hint, LodHint::Hot);
    }

    #[test]
    fn coarse_lod_fuse_end_is_monotonic() {
        let mut globals = Globals::default();
        globals.coarse_dt = 10;
        let mut w = World::with_globals(3, globals);
        w.set_lod(LodMode::Coarse);
        let id = w.ledger.spawn_system();
        // Arm fuse to end at tick 25; coarse steps 0→10→20→30 fire once at 30.
        {
            let mut op = Operator::new(&mut w);
            op.arm_fuse(id, 25).unwrap();
        }
        assert_eq!(w.master_tick(), 0);
        w.tick(2); // 0→10→20
        assert_eq!(w.master_tick(), 20);
        assert!(w.ledger().get(id).unwrap().fuse_end_tick.is_some());
        w.tick(1); // 20→30 crosses 25
        assert_eq!(w.master_tick(), 30);
        assert!(w.ledger().get(id).unwrap().fuse_end_tick.is_none());
        let ends = w
            .log()
            .events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::FuseEnd { system } if system == id))
            .count();
        assert_eq!(ends, 1, "fuse must end exactly once under coarse dt");
    }

    #[test]
    fn fine_vs_coarse_placeholder() {
        let mut w = World::new(8);
        assert_eq!(w.current_dt(), 1);
        w.set_lod(LodMode::Coarse);
        assert_eq!(w.current_dt(), w.globals().coarse_dt);
        let from = w.master_tick();
        w.tick(1);
        assert_eq!(w.master_tick(), from + w.globals().coarse_dt);
    }

    // --- Phase I stubs ---

    #[test]
    fn phase_i_doctrine_defaults() {
        let w = World::new(42);
        assert!((w.globals().salt_willingness - 0.15).abs() < f64::EPSILON);
        assert!((w.globals().punishment_willingness - 0.55).abs() < f64::EPSILON);
        assert!((w.globals().evacuate_vs_die_in_place - 0.6).abs() < f64::EPSILON);
        assert_eq!(w.ledger().empires_len(), 1);
        let emp = w.ledger().empires().next().unwrap().1;
        assert!((emp.salt_willingness - 0.15).abs() < f64::EPSILON);
        assert!((emp.punishment_willingness - 0.55).abs() < f64::EPSILON);
        assert!((emp.evacuate_vs_die_in_place - 0.6).abs() < f64::EPSILON);
        assert!(w.log().events().iter().any(|e| matches!(
            e.kind,
            EventKind::EmpireSpawned { .. }
        )));
        assert!(!w.minds_flags.scoring_enabled);
        assert!(!w.minds_flags.salt_emit_enabled);
    }

    #[test]
    fn event_log_soft_cap_drops_oldest_keeps_seq() {
        let mut log = EventLog::with_max_events(3);
        for i in 0..5 {
            log.append(i, EventKind::TickAdvanced {
                from: i,
                to: i + 1,
                dt: 1,
                lod: "Fine".into(),
            });
        }
        assert_eq!(log.len(), 3);
        assert_eq!(log.dropped(), 2);
        assert_eq!(log.next_seq(), 5);
        let seqs: Vec<_> = log.events().iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![2, 3, 4]);
        // Unlimited still grows.
        let mut open = EventLog::with_max_events(0);
        for i in 0..10 {
            open.append(i, EventKind::WorldCreated { seed: i });
        }
        assert_eq!(open.len(), 10);
        assert_eq!(open.dropped(), 0);
    }

    #[test]
    fn outcome_hash_covers_empire_doctrine() {
        let mut a = World::new(21);
        let mut b = World::new(21);
        a.recompute_outcome_hash();
        b.recompute_outcome_hash();
        assert_eq!(a.outcome_hash(), b.outcome_hash());
        let empire = *a.ledger().empires().next().unwrap().0;
        // Mutate ledger directly (no log append) so only the empire fingerprint moves.
        a.ledger
            .get_empire_mut(empire)
            .unwrap()
            .salt_willingness = 0.91;
        a.recompute_outcome_hash();
        b.recompute_outcome_hash();
        assert_eq!(a.log().len(), b.log().len());
        assert_ne!(
            a.outcome_hash(),
            b.outcome_hash(),
            "empire doctrine must affect outcome_hash"
        );
    }

    #[test]
    fn save_load_preserves_orders() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orders.json");
        let mut w = World::new(77);
        let empire = *w.ledger().empires().next().unwrap().0;
        let order_id = {
            let mut op = Operator::new(&mut w);
            op.issue_order(empire, "fortify", None)
                .unwrap()
                .expect("order written")
        };
        save_world(&mut w, &path).unwrap();
        let loaded = load_world(&path).unwrap();
        assert_eq!(loaded.ledger.orders_len(), 1);
        let ord = loaded.ledger().get_order(order_id).unwrap();
        assert_eq!(ord.intent, OrderIntent::Fortify);
        assert_eq!(ord.empire_id, empire);
        assert_eq!(loaded.ledger.empires_len(), w.ledger.empires_len());
    }

    #[test]
    fn phase_i_order_on_ledger() {
        let mut w = World::new(10);
        let empire = *w.ledger().empires().next().unwrap().0;
        let order_id = {
            let mut op = Operator::new(&mut w);
            op.issue_order(empire, "expand_survey", None)
                .unwrap()
                .expect("non-salt order written")
        };
        assert_eq!(w.ledger().orders_len(), 1);
        let ord = w.ledger().get_order(order_id).unwrap();
        assert_eq!(ord.intent, OrderIntent::ExpandSurvey);
        assert_eq!(ord.status, OrderStatus::Queued);
        assert_eq!(ord.source, OrderSource::Operator);
    }

    #[test]
    fn phase_i_salt_blocked_by_default_flags() {
        let mut w = World::new(12);
        let empire = *w.ledger().empires().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            let r = op.issue_order(empire, "salt_world", None).unwrap();
            assert!(r.is_none());
            let r = op.issue_order(empire, "punish_salter", None).unwrap();
            assert!(r.is_none());
            let r = op.issue_order(empire, "prosecute_atrocity", None).unwrap();
            assert!(r.is_none());
        }
        assert_eq!(w.ledger().orders_len(), 0);
    }

    #[test]
    fn phase_i_home_flag_clear_capital_rescore_same_tick() {
        let mut w = World::new(13);
        let sys = *w.ledger().systems().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            op.set_field(sys, "is_home_capital", "true").unwrap();
            op.set_field(sys, "home_flag", "true").unwrap();
        }
        let tick_before = w.master_tick();
        {
            let mut op = Operator::new(&mut w);
            op.set_field(sys, "home_flag", "false").unwrap();
        }
        assert_eq!(w.master_tick(), tick_before);
        let events = w.log().events();
        let clear_pos = events
            .iter()
            .position(|e| matches!(e.kind, EventKind::HomeFlagClear { system } if system == sys))
            .expect("HomeFlagClear");
        let rescore_pos = events
            .iter()
            .position(|e| {
                matches!(
                    &e.kind,
                    EventKind::CapitalRescore { system, .. } if *system == sys
                )
            })
            .expect("CapitalRescore");
        assert!(
            rescore_pos > clear_pos,
            "CapitalRescore must follow HomeFlagClear same tick"
        );
        assert_eq!(events[clear_pos].at_tick, events[rescore_pos].at_tick);
    }

    #[test]
    fn phase_i_operator_mutates_empire_doctrine() {
        let mut w = World::new(14);
        let empire = *w.ledger().empires().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            op.set_empire_doctrine(empire, "salt_willingness", "0.9")
                .unwrap();
        }
        assert!(
            (w.ledger().get_empire(empire).unwrap().salt_willingness - 0.9).abs() < f64::EPSILON
        );
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::OperatorMutation { entity, field, new, .. }
                if *entity == empire && field == "salt_willingness" && new == "0.9"
        )));
    }

    // --- Phase G/P stub tests ---

    #[test]
    fn ko_emit_acquire_confirm_grades() {
        use std::collections::BTreeSet;
        let mut w = World::new(100);
        let actor = EmpireId(1);
        let victim = EmpireId(2);
        let witness = EmpireId(3);
        let sys = *w.ledger().systems().next().unwrap().0;

        let ko = emit_ko(
            &mut w,
            EmitKoParams {
                kind: KoKind::Signal,
                grade: KoGrade::Rumor,
                origin_event_seq: None,
                payload: KoPayload {
                    who_actor: Some(actor),
                    who_victim: Some(victim),
                    system: Some(sys),
                    severity: 2,
                    target_type: "colony".into(),
                    claim: "strike reported".into(),
                },
                initial_carriers: BTreeSet::new(),
                propagation: KoPropagation::Broadcast,
            },
        );
        assert_eq!(w.knowledge.get(ko).unwrap().grade, KoGrade::Rumor);
        assert!(acquire_ko(&mut w, witness, ko));
        assert!(confirm_ko(&mut w, ko));
        assert_eq!(w.knowledge.get(ko).unwrap().grade, KoGrade::Confirmed);
    }

    #[test]
    fn first_contact_emits_and_updates_fog() {
        let mut w = World::new(101);
        let a = EmpireId(10);
        let b = EmpireId(20);
        let sys = *w.ledger().systems().next().unwrap().0;
        first_contact(&mut w, a, b, Some(sys));
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::FirstContact { a: x, b: y } if *x == a && *y == b
        )));
        assert!(w.contact.get(a).unwrap().fog.known_systems.contains_key(&sys));
        assert_eq!(w.standing.get(a, b), STANDING_FIRST_CONTACT);
        assert_eq!(w.standing.get(b, a), STANDING_FIRST_CONTACT);
    }

    #[test]
    fn victim_standing_on_salt_without_ko_witness_needs_acquire() {
        use std::collections::BTreeSet;
        let mut w = World::new(102);
        let actor = EmpireId(1);
        let victim = EmpireId(2);
        let witness = EmpireId(3);
        let sys = *w.ledger().systems().next().unwrap().0;
        emit_salt(&mut w, actor, victim, sys);
        assert_eq!(w.standing.get(victim, actor), -STANDING_SALT_VICTIM);
        assert_eq!(w.standing.get(witness, actor), 0);
        let ko = emit_ko(
            &mut w,
            EmitKoParams {
                kind: KoKind::RefugeeWave,
                grade: KoGrade::Rumor,
                origin_event_seq: None,
                payload: KoPayload {
                    who_actor: Some(actor),
                    who_victim: Some(victim),
                    system: Some(sys),
                    severity: 0,
                    target_type: "world".into(),
                    claim: "salt refugees".into(),
                },
                initial_carriers: BTreeSet::new(),
                propagation: KoPropagation::EvacConvoy,
            },
        );
        assert_eq!(w.standing.get(witness, actor), 0);
        acquire_ko(&mut w, witness, ko);
        assert_eq!(w.standing.get(witness, actor), -STANDING_RUMOR);
        confirm_ko(&mut w, ko);
        assert_eq!(w.standing.get(witness, actor), -STANDING_CONFIRMED);
    }

    #[test]
    fn push_fine_hot_sets_lod_hint() {
        let mut w = World::new(103);
        let sys = *w.ledger().systems().next().unwrap().0;
        w.ledger.get_mut(sys).unwrap().lod_hint = LodHint::Quiet;
        push_fine_hot(&mut w, sys);
        assert_eq!(w.ledger().get(sys).unwrap().lod_hint, LodHint::Hot);
    }

    #[test]
    fn salvage_always_grants_wreck_ko() {
        let mut w = World::new(104);
        let salvager = EmpireId(7);
        let sys = *w.ledger().systems().next().unwrap().0;
        let ko = salvage_contact(&mut w, salvager, sys, "derelict hull");
        let obj = w.knowledge.get(ko).unwrap();
        assert_eq!(obj.kind, KoKind::Wreck);
        assert_eq!(obj.grade, KoGrade::Confirmed);
        assert!(obj.carriers.contains(&CarrierId::Empire(salvager)));
        assert_eq!(w.ledger().get(sys).unwrap().lod_hint, LodHint::Hot);
    }

    #[test]
    fn inject_rumor_is_rumor_grade() {
        let mut w = World::new(105);
        let empire = EmpireId(9);
        let ko = inject_rumor(
            &mut w,
            KoKind::LeakedEvent,
            KoPayload {
                who_actor: Some(EmpireId(1)),
                who_victim: Some(EmpireId(2)),
                system: None,
                severity: 1,
                target_type: "fleet".into(),
                claim: "operator rumor".into(),
            },
            Some(empire),
        );
        assert_eq!(w.knowledge.get(ko).unwrap().grade, KoGrade::Rumor);
        assert_eq!(
            w.knowledge.get(ko).unwrap().propagation,
            KoPropagation::OperatorInject
        );
    }
}
