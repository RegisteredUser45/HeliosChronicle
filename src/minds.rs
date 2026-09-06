//! Phase I — Minds schema stubs: empire doctrine + Order entities on the Kernel ledger.
//!
//! Scoring and salt/punish emit stay behind feature flags until B map bits and H
//! knowledge objects land. See `docs/phase-i-minds.md`.

use serde::{Deserialize, Serialize};

use crate::entity::{EntityId, EntityLedger, OrderIntent, OrderSource};
use crate::event::EventKind;
use crate::world::World;

/// Feature flags for Phase I minds behavior (default both off).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MindsFlags {
    /// When false, `rescore_system` and the minds tick stub are no-ops.
    pub scoring_enabled: bool,
    /// When false, SaltWorld / PunishSalter / ProsecuteAtrocity are not written.
    pub salt_emit_enabled: bool,
}

impl Default for MindsFlags {
    fn default() -> Self {
        Self {
            scoring_enabled: false,
            salt_emit_enabled: false,
        }
    }
}

/// Clamp doctrine willingness / bias into `[0.0, 1.0]`.
pub fn clamp_doctrine(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Stub knowledge-path gate (H). Defaults to false until knowledge objects exist.
///
/// Future: victim auto-knows own-world; witnesses need a KO on a path.
/// Rumor→confirmed grades: confirmed heavier when gating (placeholder weights).
pub fn has_knowledge_path(
    _world: &World,
    _empire_id: EntityId,
    _intent: OrderIntent,
    _target: Option<EntityId>,
) -> bool {
    // Placeholder KO grade weights (unused until H wires real paths):
    // const RUMOR_WEIGHT: f64 = 0.25;
    // const CONFIRMED_WEIGHT: f64 = 1.0;
    false
}

/// Whether this intent is salt/punish/atrocity and must pass emit gates.
pub fn is_salt_family(intent: OrderIntent) -> bool {
    matches!(
        intent,
        OrderIntent::SaltWorld | OrderIntent::PunishSalter | OrderIntent::ProsecuteAtrocity
    )
}

/// Try to create an order on the ledger.
///
/// Salt-family intents return `Ok(None)` when `salt_emit_enabled` is false, or
/// when enabled but `has_knowledge_path` is false (empty knowledge → no order).
pub fn try_emit_order(
    world: &mut World,
    empire_id: EntityId,
    intent: OrderIntent,
    target_ref: Option<EntityId>,
    source: OrderSource,
) -> Result<Option<EntityId>, String> {
    if world.ledger.get_empire(empire_id).is_none() {
        return Err(format!("empire {empire_id} not found"));
    }

    if is_salt_family(intent) {
        if !world.minds_flags.salt_emit_enabled {
            return Ok(None);
        }
        if !has_knowledge_path(world, empire_id, intent, target_ref) {
            return Ok(None);
        }
    }

    let tick = world.master_tick;
    let id = world
        .ledger
        .spawn_order(empire_id, intent, target_ref, tick, source);
    let intent_s = intent.as_str().to_string();
    world.log.append(
        tick,
        EventKind::OrderCreated {
            order: id,
            empire: empire_id,
            intent: intent_s,
        },
    );
    Ok(Some(id))
}

/// Stub capital re-score. No-op unless `scoring_enabled`.
pub fn rescore_system(world: &mut World, _system_id: EntityId) {
    if !world.minds_flags.scoring_enabled {
        return;
    }
    // Future: clear fuse-pause awareness (B map bit) and re-rank orders / standing.
    let _ = world;
}

/// Same-tick hook after home-flag clear: append CapitalRescore, then stub rescore.
///
/// B will expose a fuse-pause bit; for now home_flag clear is the signal.
/// When scoring is enabled, re-score runs before further AI orders.
pub fn on_home_flag_clear(world: &mut World, system_id: EntityId) {
    let empire = world
        .ledger
        .get(system_id)
        .and_then(|sys| {
            // Stub: associate capital-ish systems with the first empire if any.
            if sys.is_home_capital || !sys.home_flag {
                world.ledger.empires().next().map(|(id, _)| *id)
            } else {
                None
            }
        })
        .or_else(|| world.ledger.empires().next().map(|(id, _)| *id));

    let tick = world.master_tick;
    world.log.append(
        tick,
        EventKind::CapitalRescore {
            system: system_id,
            empire,
        },
    );
    rescore_system(world, system_id);
}

/// Minds tick stub — no Expand/Plant emit while scoring is off.
pub fn minds_tick_stub(world: &mut World) {
    if !world.minds_flags.scoring_enabled {
        return;
    }
    let _ = world;
}

/// Convenience: spawn an empire from galaxy doctrine defaults and log it.
pub fn spawn_empire_with_event(world: &mut World, name: Option<String>) -> EntityId {
    let id = world.ledger.spawn_empire(&world.globals, name);
    let tick = world.master_tick;
    world
        .log
        .append(tick, EventKind::EmpireSpawned { empire: id });
    id
}

/// Apply a doctrine field mutation on an empire (clamped). Returns (old, new) strings.
pub fn set_empire_doctrine_field(
    ledger: &mut EntityLedger,
    empire_id: EntityId,
    field: &str,
    value: f64,
) -> Result<(String, String), String> {
    let empire = ledger
        .get_empire_mut(empire_id)
        .ok_or_else(|| format!("empire {empire_id} not found"))?;
    let v = clamp_doctrine(value);
    let (old, new) = match field {
        "salt_willingness" => {
            let old = empire.salt_willingness.to_string();
            empire.salt_willingness = v;
            (old, v.to_string())
        }
        "punishment_willingness" => {
            let old = empire.punishment_willingness.to_string();
            empire.punishment_willingness = v;
            (old, v.to_string())
        }
        "evacuate_vs_die_in_place" => {
            let old = empire.evacuate_vs_die_in_place.to_string();
            empire.evacuate_vs_die_in_place = v;
            (old, v.to_string())
        }
        other => return Err(format!("unknown doctrine field '{other}'")),
    };
    Ok((old, new))
}

#[cfg(test)]
mod minds_tests {
    use super::*;
    use crate::entity::{EmpireEntity, OrderStatus};
    use crate::globals::Globals;

    #[test]
    fn empire_defaults_match_globals() {
        let g = Globals::default();
        assert!((g.salt_willingness - 0.15).abs() < f64::EPSILON);
        assert!((g.punishment_willingness - 0.55).abs() < f64::EPSILON);
        assert!((g.evacuate_vs_die_in_place - 0.6).abs() < f64::EPSILON);
        let e = EmpireEntity::from_defaults(EntityId(7), &g, Some("stub".into()));
        assert_eq!(e.id, EntityId(7));
        assert!((e.salt_willingness - 0.15).abs() < f64::EPSILON);
        assert!((e.punishment_willingness - 0.55).abs() < f64::EPSILON);
        assert!((e.evacuate_vs_die_in_place - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn clamp_doctrine_bounds() {
        assert_eq!(clamp_doctrine(-1.0), 0.0);
        assert_eq!(clamp_doctrine(2.0), 1.0);
        assert_eq!(clamp_doctrine(0.42), 0.42);
        assert_eq!(clamp_doctrine(f64::NAN), 0.0);
    }

    #[test]
    fn order_create_on_ledger() {
        let mut w = World::new(1);
        let empire = w.ledger.empires().next().map(|(id, _)| *id).expect("seed empire");
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::ExpandSurvey,
            None,
            OrderSource::Operator,
        )
        .unwrap()
        .expect("order written");
        let order = w.ledger.get_order(id).unwrap();
        assert_eq!(order.empire_id, empire);
        assert_eq!(order.intent, OrderIntent::ExpandSurvey);
        assert_eq!(order.status, OrderStatus::Queued);
        assert!(w.log.events().iter().any(|e| matches!(
            &e.kind,
            EventKind::OrderCreated { order: o, .. } if *o == id
        )));
    }

    #[test]
    fn salt_emit_blocked_when_flag_off() {
        let mut w = World::new(2);
        let empire = w.ledger.empires().next().map(|(id, _)| *id).unwrap();
        assert!(!w.minds_flags.salt_emit_enabled);
        let r = try_emit_order(
            &mut w,
            empire,
            OrderIntent::SaltWorld,
            None,
            OrderSource::Ai,
        )
        .unwrap();
        assert!(r.is_none());
        assert_eq!(w.ledger.orders_len(), 0);
    }

    #[test]
    fn salt_emit_blocked_without_knowledge_even_when_flag_on() {
        let mut w = World::new(3);
        w.minds_flags.salt_emit_enabled = true;
        let empire = w.ledger.empires().next().map(|(id, _)| *id).unwrap();
        let r = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PunishSalter,
            None,
            OrderSource::Ai,
        )
        .unwrap();
        assert!(r.is_none(), "empty KO path must not emit");
        assert_eq!(w.ledger.orders_len(), 0);
    }

    #[test]
    fn home_flag_clear_triggers_capital_rescore_same_tick() {
        let mut w = World::new(4);
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.home_flag = true;
            s.is_home_capital = true;
        }
        let tick = w.master_tick();
        w.log
            .append(tick, EventKind::HomeFlagClear { system: sys });
        on_home_flag_clear(&mut w, sys);
        assert_eq!(w.master_tick(), tick, "must stay same tick");
        assert!(w.log.events().iter().any(|e| matches!(
            &e.kind,
            EventKind::CapitalRescore { system, .. } if *system == sys
        )));
    }
}
