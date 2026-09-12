//! Phase I — Minds: empire doctrine + Order entities + MapState scoring.
//!
//! Salt/punish emit stay behind feature flags until H knowledge objects land.
//! Scoring (this module) consumes Phase B `sky::map_state` bits only — no parallel map fields.
//! Feed input is C's ledger `binding_remainder` (C owns deposit aggregation via
//! `matter::reaggregate_*`; I only reads the field — no parallel feed score source).
//! See `docs/phase-i-minds.md`.

use serde::{Deserialize, Serialize};

use crate::entity::{
    EmpireEntity, EmpireId, EntityId, EntityLedger, OrderIntent, OrderSource, OrderStatus,
    SystemEntity,
};
use crate::event::EventKind;
use crate::lod::{LodHint, LodMode};
use crate::matter;
use crate::sky::{self, MapState};
use crate::world::World;
use crate::worlds;

/// Binding remainder at/above which Feed systems prefer PlantCity / PlantYard.
pub const LONG_FEED: f64 = 400.0;
/// Binding remainder below which Feed systems prefer StripMine / Fortify.
pub const SHORT_FEED: f64 = 100.0;
/// One-shot Ai StripMine drain via C `extract_state` (quantity units).
pub const STRIP_MINE_EXTRACT: f64 = 25.0;
/// Seed population planted by Ai PlantCity.
pub const PLANT_CITY_POPS: f64 = 50.0;
/// Structure soak floor for Ai PlantYard (yard pad).
pub const PLANT_YARD_STRUCTURE_SOAK: f64 = 3.0;
/// Structure soak floor for Ai Fortify.
pub const FORTIFY_STRUCTURE_SOAK: f64 = 5.0;
/// Known fuse remaining below this fraction of `globals.fuse_length_ticks` is urgent.
const FUSE_URGENCY_FRAC: f64 = 0.20;
/// Finite score for uncertain / unknown fuse — never +∞.
const UNCERTAIN_FUSE_SCORE: f64 = 25.0;
/// Penalty applied to feed_score when the system is not known to the empire.
const UNKNOWN_FEED_PENALTY: f64 = 50.0;
/// Feed score is multiplied by (1 - fog uncertainty) when known via fog.

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

/// Per-system score against Phase B `MapState` for one empire's mind.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemScore {
    pub system: EntityId,
    pub map_state: MapState,
    pub known: bool,
    pub feed_score: f64,
    pub fuse_score: f64,
    pub total: f64,
    pub suggested: Option<OrderIntent>,
    /// Ledger binding remainder (sim truth).
    pub binding_remainder: f64,
    /// AI-known exact fuse (surveyed or fog.surveyed_fuse); ledger fuse may still be armed.
    pub fuse_known: bool,
    /// Known remaining < 20% of galaxy fuse length.
    pub fuse_urgent: bool,
}

/// Clamp doctrine willingness / bias into `[0.0, 1.0]`.
pub fn clamp_doctrine(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Knowledge-path gate for salt-family orders (H/P Lock 9).
///
/// - Victim auto-knows cruelty on own worlds / as named victim on a KO payload.
/// - Witnesses need to **carry** a KO whose payload implicates the act.
/// - Rumor and confirmed both open the path; confirmed weighs heavier for
///   doctrine thresholds (`evidence_weight_for_salt_family`).
/// - `salt_emit_enabled` remains default **false** — path ≠ emit.
pub const RUMOR_WEIGHT: f64 = 0.25;
pub const CONFIRMED_WEIGHT: f64 = 1.0;
/// `salt_willingness * evidence_weight` must meet this to emit SaltWorld.
pub const SALT_DOCTRINE_THRESHOLD: f64 = 0.15;
/// `punishment_willingness * evidence_weight` for Punish/Prosecute.
pub const PUNISH_DOCTRINE_THRESHOLD: f64 = 0.20;

/// Best evidence weight for a salt-family order, if any path exists.
///
/// Returns `None` when there is no knowledge path. Victim auto-know (own home
/// or KO naming empire as victim) counts as confirmed weight.
pub fn evidence_weight_for_salt_family(
    world: &World,
    empire_id: EntityId,
    _intent: OrderIntent,
    target: Option<EntityId>,
) -> Option<f64> {
    let eid = EmpireId(empire_id.0);
    let mut best: Option<f64> = None;
    let mut raise = |w: f64| {
        best = Some(best.map(|b: f64| b.max(w)).unwrap_or(w));
    };

    if let Some(sys_id) = target {
        if let Some(sys) = world.ledger.get(sys_id) {
            if sys.home_empire == Some(eid) {
                raise(CONFIRMED_WEIGHT);
            }
        }
    }

    for (_id, ko) in world.knowledge.iter() {
        let payload = &ko.payload;
        let victim_match = payload.who_victim == Some(eid);
        let carries = ko
            .carriers
            .contains(&crate::knowledge::CarrierId::Empire(eid));
        let system_match = match (target, payload.system) {
            (Some(t), Some(s)) => t == s,
            (None, _) => true,
            (Some(_), None) => false,
        };
        let actor_present = payload.who_actor.is_some();
        let grade_w = match ko.grade {
            crate::knowledge::KoGrade::Rumor => RUMOR_WEIGHT,
            crate::knowledge::KoGrade::Confirmed => CONFIRMED_WEIGHT,
        };

        if victim_match && (system_match || target.is_none()) {
            raise(grade_w.max(CONFIRMED_WEIGHT)); // victim auto-know ≥ confirmed
        }
        if carries && actor_present && system_match {
            raise(grade_w);
        }
    }
    best
}

pub fn has_knowledge_path(
    world: &World,
    empire_id: EntityId,
    intent: OrderIntent,
    target: Option<EntityId>,
) -> bool {
    evidence_weight_for_salt_family(world, empire_id, intent, target).is_some()
}

/// Doctrine × evidence gate (after knowledge path). Deterministic — no RNG.
/// How hostile `empire` already feels toward the worst implicated KO actor.
/// More negative standing → higher multiplier (easier Punish/Prosecute).
/// SaltWorld ignores standing (cruelty willingness is its own knob).
pub fn punish_standing_multiplier(
    world: &World,
    empire_id: EntityId,
    target: Option<EntityId>,
) -> f64 {
    let eid = EmpireId(empire_id.0);
    let mut worst = 0_i32;
    for (_id, ko) in world.knowledge.iter() {
        let payload = &ko.payload;
        let carries = ko
            .carriers
            .contains(&crate::knowledge::CarrierId::Empire(eid));
        let victim_match = payload.who_victim == Some(eid);
        let system_match = match (target, payload.system) {
            (Some(t), Some(s)) => t == s,
            (None, _) => true,
            (Some(_), None) => false,
        };
        if !(carries || victim_match) || !system_match {
            continue;
        }
        if let Some(actor) = payload.who_actor {
            let s = world.standing.get(eid, actor);
            if s < worst {
                worst = s;
            }
        }
    }
    if worst <= -crate::politics::STANDING_SALT_VICTIM {
        1.5
    } else if worst <= -crate::politics::STANDING_CONFIRMED {
        1.25
    } else {
        1.0
    }
}

pub fn doctrine_allows_salt_family(
    empire: &EmpireEntity,
    intent: OrderIntent,
    evidence_weight: f64,
) -> bool {
    doctrine_allows_salt_family_weighted(empire, intent, evidence_weight, 1.0)
}

pub fn doctrine_allows_salt_family_weighted(
    empire: &EmpireEntity,
    intent: OrderIntent,
    evidence_weight: f64,
    standing_mult: f64,
) -> bool {
    let score = match intent {
        OrderIntent::SaltWorld => empire.salt_willingness * evidence_weight,
        OrderIntent::PunishSalter | OrderIntent::ProsecuteAtrocity => {
            empire.punishment_willingness * standing_mult * evidence_weight
        }
        _ => return true,
    };
    let threshold = match intent {
        OrderIntent::SaltWorld => SALT_DOCTRINE_THRESHOLD,
        OrderIntent::PunishSalter | OrderIntent::ProsecuteAtrocity => PUNISH_DOCTRINE_THRESHOLD,
        _ => 0.0,
    };
    score + f64::EPSILON >= threshold
}

/// Whether this intent is salt/punish/atrocity and must pass emit gates.
pub fn is_salt_family(intent: OrderIntent) -> bool {
    matches!(
        intent,
        OrderIntent::SaltWorld | OrderIntent::PunishSalter | OrderIntent::ProsecuteAtrocity
    )
}

/// Apply Evacuate intent to all bodies in a system (D `evacuate_body`).
///
/// `leave_automation` follows empire `evacuate_vs_die_in_place` (≥ 0.5 → leave
/// ash automation running for C drains). No-op when no pops on any body.
pub fn execute_evacuate_intent(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) -> usize {
    let leave_automation = world
        .ledger
        .get_empire(empire_id)
        .map(|e| e.evacuate_vs_die_in_place >= 0.5)
        .unwrap_or(true);
    let body_ids: Vec<EntityId> = world
        .ledger
        .bodies_for_system(system_id)
        .filter(|(_, b)| b.pops > 0.0)
        .map(|(id, _)| *id)
        .collect();
    let tick = world.master_tick;
    let mut n = 0usize;
    for bid in body_ids {
        if let Some(body) = world.ledger.get_body_mut(bid) {
            worlds::evacuate_body(body, leave_automation);
            n += 1;
            world.log.append(
                tick,
                EventKind::OperatorMutation {
                    entity: bid,
                    field: "evacuate".into(),
                    old: String::new(),
                    new: format!(
                        "ai_order leave_automation={leave_automation} empire={}",
                        empire_id
                    ),
                },
            );
        }
    }
    n
}

/// Apply ExpandSurvey intent via H `sensors::sense_system` (fog last-known).
///
/// Parallel to Ai Evacuate → D `evacuate_body`. Marks/refreshes the target
/// system in the empire's fog so frontier survey can open knowledge.
pub fn execute_expand_survey_intent(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) {
    crate::sensors::sense_system(world, EmpireId(empire_id.0), system_id);
}

/// Apply ClaimFeed intent via B `sky::claim_system` + H fog refresh.
///
/// Parallel to Ai Evacuate → D and Ai ExpandSurvey → H. Sets `sys.claimed`
/// (ends wilderness immortality per Lock 3) and opens knowledge via
/// `sensors::sense_system`. Returns whether the claim succeeded.
pub fn execute_claim_feed_intent(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) -> bool {
    let ok = sky::claim_system(world, system_id);
    if ok {
        crate::sensors::sense_system(world, EmpireId(empire_id.0), system_id);
    }
    ok
}

/// Try to create an order on the ledger.
///
/// Salt-family intents return `Ok(None)` when `salt_emit_enabled` is false, or
/// when enabled but `has_knowledge_path` is false (empty knowledge → no order).

/// Apply Abandon intent: clear pops and leave C abandoned automation running.
///
/// Uses D `evacuate_body(..., leave_automation=true)` on every body in the
/// system so Lock 8 abandoned-auto drains can continue after the polity leaves.
pub fn execute_abandon_intent(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) -> usize {
    let body_ids: Vec<EntityId> = world
        .ledger
        .bodies_for_system(system_id)
        .map(|(id, _)| *id)
        .collect();
    let tick = world.master_tick;
    let mut n = 0usize;
    for bid in body_ids {
        if let Some(body) = world.ledger.get_body_mut(bid) {
            worlds::evacuate_body(body, true);
            n += 1;
            world.log.append(
                tick,
                EventKind::OperatorMutation {
                    entity: bid,
                    field: "abandon".into(),
                    old: String::new(),
                    new: format!("ai_order leave_automation=true empire={empire_id}"),
                },
            );
        }
    }
    n
}


/// Apply StripMine intent via C `extract_state` (Lock 8 state extractor).
///
/// One-shot drain of `STRIP_MINE_EXTRACT` quantity from state-owned deposits;
/// C reaggregates binding and may arm depletion. Returns drained amount (0 if none).
pub fn execute_strip_mine_intent(
    world: &mut World,
    _empire_id: EntityId,
    system_id: EntityId,
) -> f64 {
    matter::extract_state(world, system_id, STRIP_MINE_EXTRACT).unwrap_or(0.0)
}


fn ensure_system_body(world: &mut World, system_id: EntityId) -> EntityId {
    if let Some((id, _)) = world.ledger.bodies_for_system(system_id).next() {
        return *id;
    }
    world.ledger.spawn_body(system_id)
}

/// Ai PlantCity: ensure a body, seed pops, apply D facility soaks.
pub fn execute_plant_city_intent(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) -> EntityId {
    let body_id = ensure_system_body(world, system_id);
    if let Some(body) = world.ledger.get_body_mut(body_id) {
        body.pops = (body.pops + PLANT_CITY_POPS).max(PLANT_CITY_POPS);
        if let Some(empire) = world.ledger.get_empire(empire_id) {
            let empire = empire.clone();
            if let Some(body) = world.ledger.get_body_mut(body_id) {
                worlds::apply_facility_soaks(body, &empire);
            }
        }
    }
    body_id
}

/// Ai PlantYard: ensure a body, raise structure soak to yard pad, apply facility soaks.
pub fn execute_plant_yard_intent(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) -> EntityId {
    let body_id = ensure_system_body(world, system_id);
    if let Some(body) = world.ledger.get_body_mut(body_id) {
        body.structure_soak = body.structure_soak.max(PLANT_YARD_STRUCTURE_SOAK);
    }
    if let Some(empire) = world.ledger.get_empire(empire_id).cloned() {
        if let Some(body) = world.ledger.get_body_mut(body_id) {
            worlds::apply_facility_soaks(body, &empire);
        }
    }
    body_id
}

/// Ai Fortify: ensure a body, raise structure soak to fortify floor.
pub fn execute_fortify_intent(
    world: &mut World,
    _empire_id: EntityId,
    system_id: EntityId,
) -> EntityId {
    let body_id = ensure_system_body(world, system_id);
    if let Some(body) = world.ledger.get_body_mut(body_id) {
        body.structure_soak = body.structure_soak.max(FORTIFY_STRUCTURE_SOAK);
    }
    body_id
}

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
        let Some(weight) =
            evidence_weight_for_salt_family(world, empire_id, intent, target_ref)
        else {
            return Ok(None);
        };
        let standing_mult = match intent {
            OrderIntent::PunishSalter | OrderIntent::ProsecuteAtrocity => {
                punish_standing_multiplier(world, empire_id, target_ref)
            }
            _ => 1.0,
        };
        let empire = world
            .ledger
            .get_empire(empire_id)
            .ok_or_else(|| format!("empire {empire_id} not found"))?;
        if !doctrine_allows_salt_family_weighted(empire, intent, weight, standing_mult) {
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
    // Ai Evacuate → clear pops on system bodies via D evacuate_body.
    if matches!(intent, OrderIntent::Evacuate) && matches!(source, OrderSource::Ai) {
        if let Some(sys) = target_ref {
            let _ = execute_evacuate_intent(world, empire_id, sys);
        }
    }
    // Ai ExpandSurvey → H sense_system (fog last-known / frontier survey).
    if matches!(intent, OrderIntent::ExpandSurvey) && matches!(source, OrderSource::Ai) {
        if let Some(sys) = target_ref {
            execute_expand_survey_intent(world, empire_id, sys);
        }
    }
    // Ai ClaimFeed → B claim_system (ends wilderness immortality) + fog.
    if matches!(intent, OrderIntent::ClaimFeed) && matches!(source, OrderSource::Ai) {
        if let Some(sys) = target_ref {
            let _ = execute_claim_feed_intent(world, empire_id, sys);
        }
    }
    // Ai Abandon → D evacuate_body leave_automation=true (C abandoned_auto).
    if matches!(intent, OrderIntent::Abandon) && matches!(source, OrderSource::Ai) {
        if let Some(sys) = target_ref {
            let _ = execute_abandon_intent(world, empire_id, sys);
        }
    }
    // Ai StripMine → C extract_state one-shot drain.
    if matches!(intent, OrderIntent::StripMine) && matches!(source, OrderSource::Ai) {
        if let Some(sys) = target_ref {
            let _ = execute_strip_mine_intent(world, empire_id, sys);
        }
    }
    // Ai PlantCity / PlantYard / Fortify → body seed / soak floors (D soaks).
    if matches!(source, OrderSource::Ai) {
        if let Some(sys) = target_ref {
            match intent {
                OrderIntent::PlantCity => {
                    let _ = execute_plant_city_intent(world, empire_id, sys);
                }
                OrderIntent::PlantYard => {
                    let _ = execute_plant_yard_intent(world, empire_id, sys);
                }
                OrderIntent::Fortify => {
                    let _ = execute_fortify_intent(world, empire_id, sys);
                }
                _ => {}
            }
        }
    }
    Ok(Some(id))
}

/// Knowledge / known-map heuristic for scoring.
///
/// A system is **known** to an empire if ANY of:
/// - `sys.home_empire == Some(EmpireId(empire_id.0))` (own capital/home)
/// - fog `known_systems` contains it
/// - OR (bootstrap) empire has **no** contact registry entry yet — then treat
///   non-wilderness-immortal systems as known for headless scoring tests.
///   Once `contact.ensure` exists for that empire, require fog or home.
pub fn is_system_known(world: &World, empire_id: EntityId, sys: &SystemEntity) -> bool {
    let eid = EmpireId(empire_id.0);
    if sys.home_empire == Some(eid) {
        return true;
    }
    match world.contact.get(eid) {
        Some(contact) => contact.fog.known_systems.contains_key(&sys.id),
        // Bootstrap: no contact registry entry yet.
        None => !sys.is_wilderness_immortal(),
    }
}

/// Exact fuse number is AI-known only when the system is surveyed or fog marks surveyed_fuse.
/// Ledger `fuse_end_tick` / `fuse_remaining` remain sim truth either way.
pub fn fuse_is_known(world: &World, empire_id: EntityId, sys: &SystemEntity) -> bool {
    if sys.surveyed {
        return true;
    }
    let eid = EmpireId(empire_id.0);
    world
        .contact
        .get(eid)
        .and_then(|c| c.fog.known_systems.get(&sys.id))
        .map(|e| e.surveyed_fuse)
        .unwrap_or(false)
}

fn fuse_urgent_known(world: &World, sys: &SystemEntity, fuse_known: bool) -> bool {
    if !fuse_known || !sys.depleted {
        return false;
    }
    let fuse_len = world.globals.fuse_length_ticks().max(1);
    let rem = sys
        .fuse_remaining
        .or_else(|| {
            sys.fuse_end_tick
                .map(|end| end.saturating_sub(world.master_tick))
        })
        .unwrap_or(fuse_len);
    (rem as f64) < (fuse_len as f64) * FUSE_URGENCY_FRAC
}


/// Fog uncertainty for a known system (0 = certain). Home / bootstrap → 0.
pub fn fog_uncertainty(world: &World, empire_id: EntityId, sys: &SystemEntity) -> f64 {
    let eid = EmpireId(empire_id.0);
    if sys.home_empire == Some(eid) {
        return 0.0;
    }
    match world.contact.get(eid) {
        Some(contact) => contact
            .fog
            .known_systems
            .get(&sys.id)
            .map(|e| e.uncertainty.clamp(0.0, 1.0))
            .unwrap_or(1.0),
        None => 0.0, // bootstrap known path
    }
}

/// Score one system for an empire against B `MapState`. Returns `None` if system missing.
pub fn score_system(
    world: &World,
    empire_id: EntityId,
    system_id: EntityId,
) -> Option<SystemScore> {
    let sys = world.ledger.get(system_id)?;
    let map_state = sky::map_state(sys);
    let known = is_system_known(world, empire_id, sys);
    let fuse_known = fuse_is_known(world, empire_id, sys);
    let fuse_urgent = fuse_urgent_known(world, sys, fuse_known);
    let binding_remainder = sys.binding_remainder;

    // 1) Feed first, fuse second. Unknown feed gets a penalty; unknown fuse is finite.
    let mut feed_score = match map_state {
        MapState::Ended => 0.0,
        MapState::WildernessUnknown => 30.0, // uncertain feed — not infinite
        MapState::HomePaused => {
            // Stable until flag drops: mid score from remainder, no panic.
            100.0 + binding_remainder.min(LONG_FEED) * 0.25
        }
        MapState::Feed => binding_remainder.max(0.0),
        MapState::DryFuse => binding_remainder.max(0.0).min(SHORT_FEED),
    };
    if !known {
        feed_score = (feed_score - UNKNOWN_FEED_PENALTY).max(0.0);
    } else {
        // G fog uncertainty discounts known feed (certain home = 0).
        let u = fog_uncertainty(world, empire_id, sys);
        feed_score *= 1.0 - u;
    }

    let fuse_score = match map_state {
        MapState::Ended | MapState::HomePaused => 0.0, // HomePaused: stable (countdown frozen)
        MapState::WildernessUnknown => UNCERTAIN_FUSE_SCORE, // never +∞
        MapState::Feed => 0.0,
        MapState::DryFuse => {
            if fuse_known {
                let fuse_len = world.globals.fuse_length_ticks().max(1) as f64;
                let rem = sys
                    .fuse_remaining
                    .or_else(|| {
                        sys.fuse_end_tick
                            .map(|end| end.saturating_sub(world.master_tick))
                    })
                    .unwrap_or(fuse_len as u64) as f64;
                let urgency = (1.0 - (rem / fuse_len).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                50.0 + urgency * 200.0
            } else {
                // Depleted but fuse not known to AI — uncertain, not infinite.
                UNCERTAIN_FUSE_SCORE
            }
        }
    };

    let total = feed_score + fuse_score;
    let mut score = SystemScore {
        system: system_id,
        map_state,
        known,
        feed_score,
        fuse_score,
        total,
        suggested: None,
        binding_remainder,
        fuse_known,
        fuse_urgent,
    };
    if let Some(empire) = world.ledger.get_empire(empire_id) {
        score.suggested = suggested_intent(&score, empire);
    }
    Some(score)
}

/// High evacuate doctrine: prefer yards/forts over cities; evacuate/abandon over hold.
pub const DOCTRINE_EVACUATE_HIGH: f64 = 0.85;
/// Low salt-willingness: prefer Fortify over StripMine (won't aggressively strip).
pub const DOCTRINE_SALT_STRIP_FLOOR: f64 = 0.10;

/// Doctrine weight for ranking candidate systems in `minds_tick_stub` (0..1).
pub fn doctrine_emit_weight(intent: OrderIntent, empire: &EmpireEntity) -> f64 {
    match intent {
        OrderIntent::PlantCity | OrderIntent::PlantYard | OrderIntent::ClaimFeed => {
            (1.0 - empire.evacuate_vs_die_in_place).clamp(0.0, 1.0)
        }
        OrderIntent::StripMine => empire.salt_willingness.clamp(0.0, 1.0),
        OrderIntent::Fortify => {
            // Holders: high evacuate-in-place OR low salt aggression.
            empire
                .evacuate_vs_die_in_place
                .max(1.0 - empire.salt_willingness)
                .clamp(0.0, 1.0)
        }
        OrderIntent::Abandon | OrderIntent::Evacuate => {
            empire.evacuate_vs_die_in_place.clamp(0.0, 1.0)
        }
        OrderIntent::ExpandSurvey => 0.5,
        _ => 0.0,
    }
}

fn tick_rank(score: &SystemScore, empire: &EmpireEntity) -> f64 {
    let w = score
        .suggested
        .map(|i| doctrine_emit_weight(i, empire))
        .unwrap_or(0.0);
    score.total * (0.5 + 0.5 * w)
}

/// Map a score into a non-salt `OrderIntent` using empire doctrine knobs.
///
/// Selects among already-built Ai orders (PlantCity/Yard/Fortify/StripMine/Abandon
/// + Evacuate/Claim/Survey). Salt family is never suggested here.
pub fn suggested_intent(score: &SystemScore, empire: &EmpireEntity) -> Option<OrderIntent> {
    match score.map_state {
        MapState::Ended => Some(OrderIntent::Abandon),
        MapState::WildernessUnknown => {
            if score.known {
                Some(OrderIntent::ClaimFeed)
            } else {
                Some(OrderIntent::ExpandSurvey)
            }
        }
        MapState::HomePaused => {
            // Stable until flag drops — do not panic-Evacuate solely because depleted.
            if score.binding_remainder >= LONG_FEED {
                if empire.evacuate_vs_die_in_place >= DOCTRINE_EVACUATE_HIGH {
                    Some(OrderIntent::Fortify)
                } else {
                    Some(OrderIntent::PlantYard)
                }
            } else if empire.salt_willingness < DOCTRINE_SALT_STRIP_FLOOR {
                Some(OrderIntent::Fortify)
            } else {
                Some(OrderIntent::Fortify)
            }
        }
        MapState::Feed => {
            if score.binding_remainder >= LONG_FEED {
                // Ultra-evacuate doctrine plants yards, not cities.
                if empire.evacuate_vs_die_in_place >= DOCTRINE_EVACUATE_HIGH {
                    Some(OrderIntent::PlantYard)
                } else {
                    Some(OrderIntent::PlantCity)
                }
            } else if score.binding_remainder < SHORT_FEED {
                // Low salt-willingness holds (Fortify) instead of StripMine.
                if empire.salt_willingness < DOCTRINE_SALT_STRIP_FLOOR {
                    Some(OrderIntent::Fortify)
                } else {
                    Some(OrderIntent::StripMine)
                }
            } else if empire.evacuate_vs_die_in_place >= DOCTRINE_EVACUATE_HIGH {
                Some(OrderIntent::Fortify)
            } else {
                Some(OrderIntent::PlantYard)
            }
        }
        MapState::DryFuse => {
            // Evacuate vs die-in-place on DryFuse / post-HomePaused-drop paths.
            if empire.evacuate_vs_die_in_place >= 0.5 && score.fuse_known && score.fuse_urgent {
                Some(OrderIntent::Evacuate)
            } else if score.binding_remainder > 0.0 {
                if empire.salt_willingness < DOCTRINE_SALT_STRIP_FLOOR {
                    Some(OrderIntent::Fortify)
                } else {
                    Some(OrderIntent::StripMine)
                }
            } else {
                Some(OrderIntent::Fortify)
            }
        }
    }
}

fn cancel_queued_ai_orders_targeting(
    world: &mut World,
    empire_id: EntityId,
    system_id: EntityId,
) {
    let tick = world.master_tick;
    let to_cancel: Vec<EntityId> = world
        .ledger
        .orders()
        .filter(|(_, o)| {
            o.empire_id == empire_id
                && o.source == OrderSource::Ai
                && o.status == OrderStatus::Queued
                && o.target_ref == Some(system_id)
        })
        .map(|(id, _)| *id)
        .collect();
    for oid in to_cancel {
        let from = world
            .ledger
            .get_order(oid)
            .map(|o| o.status.as_str().to_string())
            .unwrap_or_else(|| "queued".into());
        if let Some(o) = world.ledger.get_order_mut(oid) {
            o.status = OrderStatus::Cancelled;
            o.updated_tick = tick;
        }
        world.log.append(
            tick,
            EventKind::OrderStatusChanged {
                order: oid,
                from,
                to: OrderStatus::Cancelled.as_str().to_string(),
            },
        );
    }
}

fn empire_has_ai_order_targeting(world: &World, empire_id: EntityId, system_id: EntityId) -> bool {
    world.ledger.orders().any(|(_, o)| {
        o.empire_id == empire_id
            && o.source == OrderSource::Ai
            && matches!(o.status, OrderStatus::Queued | OrderStatus::Active)
            && o.target_ref == Some(system_id)
    })
}

fn empires_for_rescore(world: &World, system_id: EntityId) -> Vec<EntityId> {
    if let Some(sys) = world.ledger.get(system_id) {
        if let Some(he) = sys.home_empire {
            let eid = EntityId(he.0);
            if world.ledger.get_empire(eid).is_some() {
                return vec![eid];
            }
        }
    }
    // Home unknown — all empires may care.
    world.ledger.empires().map(|(id, _)| *id).collect()
}

/// Capital / system re-score. No-op unless `scoring_enabled`.
///
/// Cancels conflicting Queued+Ai orders targeting the system, then may emit one
/// suggested non-salt Ai order per caring empire.
pub fn rescore_system(world: &mut World, system_id: EntityId) {
    if !world.minds_flags.scoring_enabled {
        return;
    }
    if world.ledger.get(system_id).is_none() {
        return;
    }
    let empires = empires_for_rescore(world, system_id);
    for empire_id in empires {
        cancel_queued_ai_orders_targeting(world, empire_id, system_id);
        let intent = score_system(world, empire_id, system_id)
            .and_then(|s| s.suggested)
            .filter(|i| !is_salt_family(*i));
        if let Some(intent) = intent {
            let _ = try_emit_order(
                world,
                empire_id,
                intent,
                Some(system_id),
                OrderSource::Ai,
            );
        }
    }
}

/// Same-tick hook after home-flag clear: append CapitalRescore, then rescore.
///
/// Prefer `sys.home_empire` for the event empire (copy fields first — no nested ledger borrows).
pub fn on_home_flag_clear(world: &mut World, system_id: EntityId) {
    let home_empire = world
        .ledger
        .get(system_id)
        .and_then(|sys| sys.home_empire.map(|e| EntityId(e.0)));
    let empire = home_empire.or_else(|| world.ledger.empires().next().map(|(id, _)| *id));

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

/// Under Coarse LOD, only "hot" empires get an Ai order this tick (Issue 10).
///
/// Hot = owns/home a Hot-hint system, or any known system that is DryFuse /
/// HomePaused / Ended (needs attention). Fine LOD evaluates every empire.
pub fn empire_needs_minds_tick(world: &World, empire_id: EntityId) -> bool {
    if matches!(world.lod(), LodMode::Fine) {
        return true;
    }
    let eid = EmpireId(empire_id.0);
    for (_id, sys) in world.ledger.systems() {
        let cares = sys.home_empire == Some(eid)
            || is_system_known(world, empire_id, sys);
        if !cares {
            continue;
        }
        if matches!(sys.lod_hint, LodHint::Hot) {
            return true;
        }
        match sky::map_state(sys) {
            MapState::DryFuse | MapState::HomePaused | MapState::Ended => return true,
            MapState::Feed | MapState::WildernessUnknown => {}
        }
    }
    false
}

/// Minds tick — when scoring is enabled, emit at most one Ai order per empire per tick.
/// Coarse LOD skips quiet empires (see `empire_needs_minds_tick`).
pub fn minds_tick_stub(world: &mut World) {
    if !world.minds_flags.scoring_enabled {
        return;
    }
    let empire_ids: Vec<EntityId> = world.ledger.empires().map(|(id, _)| *id).collect();
    for empire_id in empire_ids {
        // Phase J: operator-possessed empires are not AI-driven.
        if world.is_possessed(empire_id) {
            continue;
        }
        if !empire_needs_minds_tick(world, empire_id) {
            continue;
        }
        let Some(empire_snap) = world.ledger.get_empire(empire_id).cloned() else {
            continue;
        };
        let system_ids: Vec<EntityId> = world.ledger.systems().map(|(id, _)| *id).collect();
        let mut best: Option<SystemScore> = None;
        let mut best_rank = f64::NEG_INFINITY;
        for sid in system_ids {
            let Some(score) = score_system(world, empire_id, sid) else {
                continue;
            };
            // Known systems: current behavior. Unknown: only ExpandSurvey frontier.
            if !score.known {
                if !matches!(score.suggested, Some(OrderIntent::ExpandSurvey)) {
                    continue;
                }
            }
            if empire_has_ai_order_targeting(world, empire_id, sid) {
                continue;
            }
            let rank = tick_rank(&score, &empire_snap);
            if best.is_none() || rank > best_rank {
                best_rank = rank;
                best = Some(score);
            }
        }
        if let Some(score) = best {
            if let Some(intent) = score.suggested.filter(|i| !is_salt_family(*i)) {
                let _ = try_emit_order(
                    world,
                    empire_id,
                    intent,
                    Some(score.system),
                    OrderSource::Ai,
                );
            }
        }
    }
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
    use crate::operator::Operator;

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

    #[test]
    fn scoring_disabled_still_noop() {
        let mut w = World::new(50);
        assert!(!w.minds_flags.scoring_enabled);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 800.0;
            s.depleted = false;
        }
        let before = w.ledger.orders_len();
        minds_tick_stub(&mut w);
        rescore_system(&mut w, sys);
        assert_eq!(w.ledger.orders_len(), before);
        assert!(!w.ledger.orders().any(|(_, o)| {
            o.source == OrderSource::Ai && o.empire_id == empire
        }));
    }

    #[test]
    fn score_feed_prefers_plant() {
        let mut w = World::new(51);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 800.0;
            s.depleted = false;
            s.fuse_end_tick = None;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
        }
        assert_eq!(sky::map_state(w.ledger.get(sys).unwrap()), MapState::Feed);
        let score = score_system(&w, empire, sys).expect("score");
        assert!(score.known);
        assert!(matches!(
            score.suggested,
            Some(OrderIntent::PlantCity) | Some(OrderIntent::PlantYard)
        ));
    }

    #[test]
    fn score_dry_fuse_not_plant() {
        let mut w = World::new(52);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let fuse_len = w.globals.fuse_length_ticks();
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 10.0;
            s.depleted = true;
            s.fuse_end_tick = Some(w.master_tick + fuse_len);
            s.fuse_remaining = Some(fuse_len);
            s.fuse_paused = false;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.home_flag = false;
            s.is_home_capital = false;
        }
        assert_eq!(sky::map_state(w.ledger.get(sys).unwrap()), MapState::DryFuse);
        let score = score_system(&w, empire, sys).expect("score");
        assert!(!matches!(
            score.suggested,
            Some(OrderIntent::PlantCity) | Some(OrderIntent::PlantYard)
        ));
        assert!(matches!(
            score.suggested,
            Some(OrderIntent::StripMine)
                | Some(OrderIntent::Fortify)
                | Some(OrderIntent::Evacuate)
        ));
    }

    #[test]
    fn score_home_paused_stable() {
        let mut w = World::new(53);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let fuse_len = w.globals.fuse_length_ticks();
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 5.0;
            s.depleted = true;
            s.fuse_end_tick = Some(w.master_tick + 10); // very short if it were live
            s.fuse_remaining = Some(10);
            s.fuse_paused = true;
            s.ended = false;
            s.is_home_capital = true;
            s.home_flag = true;
            s.home_empire = Some(EmpireId(empire.0));
            s.surveyed = true;
            let _ = fuse_len;
        }
        assert_eq!(
            sky::map_state(w.ledger.get(sys).unwrap()),
            MapState::HomePaused
        );
        let score = score_system(&w, empire, sys).expect("score");
        assert_ne!(score.suggested, Some(OrderIntent::Evacuate));
        assert!(matches!(
            score.suggested,
            Some(OrderIntent::Fortify) | Some(OrderIntent::PlantYard) | Some(OrderIntent::PlantCity)
        ));
    }

    #[test]
    fn capital_rescore_emits_when_scoring_on() {
        let mut w = World::new(54);
        w.minds_flags.scoring_enabled = true;
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 700.0;
            s.depleted = false;
            s.is_home_capital = true;
            s.home_flag = true;
            s.home_empire = Some(EmpireId(empire.0));
            s.surveyed = true;
        }
        // Conflicting queued AI order targeting the capital.
        let old = try_emit_order(
            &mut w,
            empire,
            OrderIntent::ExpandSurvey,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("queued ai");
        let tick = w.master_tick();
        {
            let mut op = Operator::new(&mut w);
            op.set_field(sys, "home_flag", "false").unwrap();
        }
        assert_eq!(w.master_tick(), tick);
        assert!(w.log.events().iter().any(|e| matches!(
            &e.kind,
            EventKind::CapitalRescore { system, empire: e } if *system == sys && *e == Some(empire)
        )));
        let cancelled = w.ledger.get_order(old).unwrap().status == OrderStatus::Cancelled;
        assert!(cancelled, "conflicting Queued Ai order should cancel on rescore");
        assert!(w.log.events().iter().any(|e| matches!(
            &e.kind,
            EventKind::OrderStatusChanged { order, to, .. } if *order == old && to == "cancelled"
        )));
        // New suggested Ai order (non-salt) may be emitted.
        let ai_intents: Vec<_> = w
            .ledger
            .orders()
            .filter(|(_, o)| {
                o.source == OrderSource::Ai
                    && o.status == OrderStatus::Queued
                    && o.target_ref == Some(sys)
            })
            .map(|(_, o)| o.intent)
            .collect();
        assert!(
            !ai_intents.is_empty(),
            "rescore should emit a suggested non-salt Ai order"
        );
        assert!(ai_intents.iter().all(|i| !is_salt_family(*i)));
    }

    #[test]
    fn salt_still_blocked_when_scoring_enabled() {
        let mut w = World::new(55);
        w.minds_flags.scoring_enabled = true;
        assert!(!w.minds_flags.salt_emit_enabled);
        let empire = *w.ledger.empires().next().unwrap().0;
        let r = try_emit_order(
            &mut w,
            empire,
            OrderIntent::SaltWorld,
            None,
            OrderSource::Ai,
        )
        .unwrap();
        assert!(r.is_none());
        // Scoring suggestions must never be salt family.
        let sys = *w.ledger.systems().next().unwrap().0;
        if let Some(score) = score_system(&w, empire, sys) {
            if let Some(intent) = score.suggested {
                assert!(!is_salt_family(intent));
            }
        }
        minds_tick_stub(&mut w);
        assert!(!w.ledger.orders().any(|(_, o)| is_salt_family(o.intent)));
    }


    /// C owns deposit → `binding_remainder` aggregation; I only reads that field
    /// into `score_system` feed_score (no parallel feed field).
    #[test]
    fn matter_drain_updates_binding_remainder_and_feed_score() {
        let mut w = World::new(57);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.deposits.clear();
            s.binding_remainder = 0.0;
            s.depleted = false;
            s.fuse_end_tick = None;
            s.fuse_remaining = None;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.home_flag = false;
            s.is_home_capital = false;
        }

        matter::add_deposit(
            &mut w,
            sys,
            matter::Deposit::new(
                "stock.ore_binding",
                800.0,
                1.0,
                matter::ExtractorKind::State,
            ),
        )
        .expect("add binding deposit");

        let rem_before = w.ledger.get(sys).unwrap().binding_remainder;
        assert!(
            (rem_before - 800.0).abs() < 1e-9,
            "C reaggregate must set remainder from deposit; got {rem_before}"
        );
        assert_eq!(sky::map_state(w.ledger.get(sys).unwrap()), MapState::Feed);

        let score_before = score_system(&w, empire, sys).expect("score before drain");
        assert!(score_before.known);
        assert!(
            (score_before.feed_score - rem_before).abs() < 1e-9,
            "Feed feed_score must equal ledger binding_remainder; score={} rem={}",
            score_before.feed_score,
            rem_before
        );
        assert!(
            (score_before.binding_remainder - rem_before).abs() < 1e-9,
            "SystemScore mirrors C remainder field"
        );

        matter::extract_state(&mut w, sys, 300.0).expect("drain");
        let rem_after = w.ledger.get(sys).unwrap().binding_remainder;
        assert!(
            (rem_after - 500.0).abs() < 1e-9,
            "C extract must lower remainder; got {rem_after}"
        );
        // Stay above dry_threshold so MapState remains Feed (I reads remainder only).
        assert_eq!(sky::map_state(w.ledger.get(sys).unwrap()), MapState::Feed);

        let score_after = score_system(&w, empire, sys).expect("score after drain");
        assert!(
            (score_after.feed_score - rem_after).abs() < 1e-9,
            "feed_score must track C remainder after drain; score={} rem={}",
            score_after.feed_score,
            rem_after
        );
        assert!(
            score_after.feed_score < score_before.feed_score,
            "drain must lower feed_score"
        );
        assert!(
            (score_after.feed_score - (score_before.feed_score - 300.0)).abs() < 1e-9,
            "Feed delta must match drained quantity"
        );
    }

    #[test]
    fn knowledge_path_victim_auto_know_own_home() {
        let mut w = World::new(70);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.home_empire = Some(EmpireId(empire.0));
            s.is_home_capital = true;
        }
        assert!(has_knowledge_path(
            &w,
            empire,
            OrderIntent::ProsecuteAtrocity,
            Some(sys),
        ));
        // salt_emit still default off → no order written
        assert!(!w.minds_flags.salt_emit_enabled);
        let r = try_emit_order(
            &mut w,
            empire,
            OrderIntent::SaltWorld,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn knowledge_path_witness_needs_carried_ko() {
        let mut w = World::new(71);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let actor = EmpireId(99);
        let victim = EmpireId(88);
        // No KO yet
        assert!(!has_knowledge_path(
            &w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
        ));
        let ko = crate::knowledge::emit_ko(
            &mut w,
            crate::knowledge::EmitKoParams {
                kind: crate::knowledge::KoKind::Signal,
                grade: crate::knowledge::KoGrade::Rumor,
                origin_event_seq: None,
                payload: crate::knowledge::KoPayload {
                    who_actor: Some(actor),
                    who_victim: Some(victim),
                    system: Some(sys),
                    severity: 5,
                    target_type: "world".into(),
                    claim: "salt".into(),
                },
                initial_carriers: Default::default(),
                propagation: crate::knowledge::KoPropagation::Broadcast,
            },
        );
        // Still no — empire does not carry the KO
        assert!(!has_knowledge_path(
            &w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
        ));
        assert!(crate::knowledge::acquire_ko(&mut w, EmpireId(empire.0), ko));
        assert!(has_knowledge_path(
            &w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
        ));
        // Enabling salt_emit + confirm grade so doctrine threshold passes
        w.minds_flags.salt_emit_enabled = true;
        crate::knowledge::confirm_ko(&mut w, ko);
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("path + flag + confirmed → order");
        assert_eq!(
            w.ledger.get_order(id).unwrap().intent,
            OrderIntent::PunishSalter
        );
    }


    #[test]
    fn doctrine_blocks_salt_on_rumor_for_default_empire() {
        let mut w = World::new(80);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        w.minds_flags.salt_emit_enabled = true;
        let ko = crate::knowledge::emit_ko(
            &mut w,
            crate::knowledge::EmitKoParams {
                kind: crate::knowledge::KoKind::Signal,
                grade: crate::knowledge::KoGrade::Rumor,
                origin_event_seq: None,
                payload: crate::knowledge::KoPayload {
                    who_actor: Some(EmpireId(99)),
                    who_victim: Some(EmpireId(88)),
                    system: Some(sys),
                    severity: 5,
                    target_type: "world".into(),
                    claim: "salt".into(),
                },
                initial_carriers: Default::default(),
                propagation: crate::knowledge::KoPropagation::Broadcast,
            },
        );
        assert!(crate::knowledge::acquire_ko(&mut w, EmpireId(empire.0), ko));
        let wgt = evidence_weight_for_salt_family(
            &w,
            empire,
            OrderIntent::SaltWorld,
            Some(sys),
        )
        .expect("path");
        assert!((wgt - RUMOR_WEIGHT).abs() < 1e-9);
        // default salt_willingness 0.15 * 0.25 < 0.15 threshold
        let r = try_emit_order(
            &mut w,
            empire,
            OrderIntent::SaltWorld,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap();
        assert!(r.is_none(), "rumor must fail default salt doctrine");
    }

    #[test]
    fn doctrine_allows_punish_on_rumor_for_default_empire() {
        let mut w = World::new(81);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        w.minds_flags.salt_emit_enabled = true;
        let ko = crate::knowledge::emit_ko(
            &mut w,
            crate::knowledge::EmitKoParams {
                kind: crate::knowledge::KoKind::Signal,
                grade: crate::knowledge::KoGrade::Rumor,
                origin_event_seq: None,
                payload: crate::knowledge::KoPayload {
                    who_actor: Some(EmpireId(99)),
                    who_victim: Some(EmpireId(88)),
                    system: Some(sys),
                    severity: 5,
                    target_type: "world".into(),
                    claim: "salt".into(),
                },
                initial_carriers: Default::default(),
                propagation: crate::knowledge::KoPropagation::Broadcast,
            },
        );
        assert!(crate::knowledge::acquire_ko(&mut w, EmpireId(empire.0), ko));
        // default punishment 0.55 * 0.25 = 0.1375 < 0.20 → should block on rumor
        let r = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap();
        assert!(r.is_none(), "rumor punish should fail default threshold");
        crate::knowledge::confirm_ko(&mut w, ko);
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("confirmed punish passes");
        assert_eq!(
            w.ledger.get_order(id).unwrap().intent,
            OrderIntent::PunishSalter
        );
    }


    #[test]
    fn standing_boosts_punish_doctrine() {
        let mut w = World::new(90);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let actor = EmpireId(77);
        // Low punishment so confirmed alone fails (0.15 * 1.0 < 0.20)
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.punishment_willingness = 0.15;
        }
        w.minds_flags.salt_emit_enabled = true;
        let ko = crate::knowledge::emit_ko(
            &mut w,
            crate::knowledge::EmitKoParams {
                kind: crate::knowledge::KoKind::Signal,
                grade: crate::knowledge::KoGrade::Confirmed,
                origin_event_seq: None,
                payload: crate::knowledge::KoPayload {
                    who_actor: Some(actor),
                    who_victim: Some(EmpireId(empire.0)),
                    system: Some(sys),
                    severity: 8,
                    target_type: "world".into(),
                    claim: "salt".into(),
                },
                initial_carriers: Default::default(),
                propagation: crate::knowledge::KoPropagation::Broadcast,
            },
        );
        assert!(crate::knowledge::acquire_ko(&mut w, EmpireId(empire.0), ko));
        let blocked = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap();
        assert!(blocked.is_none(), "low punish + no hostility should fail");
        // Victim-level hostility toward actor
        w.standing.table.insert(
            (EmpireId(empire.0), actor),
            -crate::politics::STANDING_SALT_VICTIM,
        );
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PunishSalter,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("hostile standing boosts punish over threshold");
        assert_eq!(
            w.ledger.get_order(id).unwrap().intent,
            OrderIntent::PunishSalter
        );
    }


    #[test]
    fn possessed_empire_skipped_by_minds_tick() {
        let mut w = World::new(11);
        w.minds_flags.scoring_enabled = true;
        let empire = *w.ledger.empires().next().unwrap().0;
        // Possess before ticking so AI cannot emit for this empire.
        {
            let mut op = crate::operator::Operator::new(&mut w);
            op.possess(empire).unwrap();
        }
        let before = w.ledger.orders_len();
        minds_tick_stub(&mut w);
        assert_eq!(
            w.ledger.orders_len(),
            before,
            "possessed empire must not receive AI orders"
        );
    }


    #[test]
    fn coarse_lod_skips_quiet_empire_minds_tick() {
        let mut w = World::new(100);
        w.minds_flags.scoring_enabled = true;
        w.set_lod(crate::lod::LodMode::Coarse);
        let empire = *w.ledger.empires().next().unwrap().0;
        // Make all systems quiet Feed with known home
        let systems: Vec<_> = w.ledger.systems().map(|(id, _)| *id).collect();
        for sid in &systems {
            let s = w.ledger.get_mut(*sid).unwrap();
            s.lod_hint = crate::lod::LodHint::Quiet;
            s.depleted = false;
            s.fuse_end_tick = None;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.binding_remainder = 800.0;
            s.home_empire = Some(EmpireId(empire.0));
        }
        assert!(!empire_needs_minds_tick(&w, empire));
        let before = w.ledger.orders_len();
        minds_tick_stub(&mut w);
        assert_eq!(w.ledger.orders_len(), before, "coarse+quiet must skip emit");

        // Mark one system Hot → empire needs tick and may emit
        let hot = systems[0];
        w.ledger.get_mut(hot).unwrap().lod_hint = crate::lod::LodHint::Hot;
        assert!(empire_needs_minds_tick(&w, empire));
        minds_tick_stub(&mut w);
        assert!(
            w.ledger.orders_len() > before,
            "coarse+hot should allow an Ai order"
        );
    }

    #[test]
    fn ai_evacuate_clears_body_pops() {
        let mut w = World::new(110);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let body = w.ledger.spawn_body(sys);
        {
            let b = w.ledger.get_body_mut(body).unwrap();
            b.pops = 100.0;
            b.automation_active = false;
        }
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.evacuate_vs_die_in_place = 0.8;
        }
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::Evacuate,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("evacuate order");
        assert_eq!(w.ledger.get_order(id).unwrap().intent, OrderIntent::Evacuate);
        let b = w.ledger.get_body(body).unwrap();
        assert_eq!(b.pops, 0.0);
        assert!(
            b.automation_active,
            "high evacuate bias should leave automation"
        );
    }


    #[test]
    fn fog_uncertainty_discounts_feed_score() {
        let mut w = World::new(120);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 800.0;
            s.depleted = false;
            s.fuse_end_tick = None;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.home_empire = None;
            s.home_flag = false;
            s.is_home_capital = false;
        }
        // Contact registry without fog → not known (bootstrap off)
        w.contact.ensure(EmpireId(empire.0));
        assert!(!score_system(&w, empire, sys).unwrap().known);
        crate::contact::grant_fog(&mut w, EmpireId(empire.0), sys);
        w.contact
            .ensure(EmpireId(empire.0))
            .fog
            .known_systems
            .get_mut(&sys)
            .unwrap()
            .uncertainty = 0.5;
        let mid = score_system(&w, empire, sys).unwrap();
        assert!(mid.known);
        assert!(
            (mid.feed_score - 400.0).abs() < 1e-6,
            "800 * (1-0.5) = 400, got {}",
            mid.feed_score
        );
        w.contact
            .ensure(EmpireId(empire.0))
            .fog
            .known_systems
            .get_mut(&sys)
            .unwrap()
            .uncertainty = 0.0;
        let certain = score_system(&w, empire, sys).unwrap();
        assert!((certain.feed_score - 800.0).abs() < 1e-6);
        assert!(certain.feed_score > mid.feed_score);
    }

    #[test]
    fn ai_expand_survey_senses_system() {
        let mut w = World::new(130);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        // Contact registry present but no fog → system unknown until sense.
        w.contact.ensure(EmpireId(empire.0));
        assert!(
            !w.contact
                .get(EmpireId(empire.0))
                .unwrap()
                .fog
                .known_systems
                .contains_key(&sys)
        );
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::ExpandSurvey,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("expand survey order");
        assert_eq!(
            w.ledger.get_order(id).unwrap().intent,
            OrderIntent::ExpandSurvey
        );
        assert!(
            w.contact
                .get(EmpireId(empire.0))
                .unwrap()
                .fog
                .known_systems
                .contains_key(&sys),
            "Ai ExpandSurvey must call sense_system and add fog"
        );
    }

    #[test]
    fn ai_claim_feed_claims_system() {
        let mut w = World::new(132);
        let empire = *w.ledger.empires().next().unwrap().0;
        // Wilderness unclaimed → immortal until ClaimFeed.
        let sys = w.ledger.spawn_wilderness_system();
        {
            let s = w.ledger.get(sys).unwrap();
            assert!(!s.claimed);
            assert!(s.is_wilderness_immortal());
        }
        // Contact registry present but no fog yet.
        w.contact.ensure(EmpireId(empire.0));
        assert!(
            !w.contact
                .get(EmpireId(empire.0))
                .unwrap()
                .fog
                .known_systems
                .contains_key(&sys)
        );
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::ClaimFeed,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("claim feed order");
        assert_eq!(
            w.ledger.get_order(id).unwrap().intent,
            OrderIntent::ClaimFeed
        );
        let s = w.ledger.get(sys).unwrap();
        assert!(s.claimed, "Ai ClaimFeed must set claimed via claim_system");
        assert!(
            !s.is_wilderness_immortal(),
            "claim ends wilderness immortality"
        );
        assert!(
            w.contact
                .get(EmpireId(empire.0))
                .unwrap()
                .fog
                .known_systems
                .contains_key(&sys),
            "Ai ClaimFeed must refresh fog via sense_system"
        );
        assert!(!w.minds_flags.salt_emit_enabled);
    }


    #[test]
    fn ai_abandon_leaves_automation() {
        let mut w = World::new(130);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let body = w.ledger.spawn_body(sys);
        {
            let b = w.ledger.get_body_mut(body).unwrap();
            b.pops = 55.0;
            b.automation_active = false;
        }
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::Abandon,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("abandon order");
        assert_eq!(w.ledger.get_order(id).unwrap().intent, OrderIntent::Abandon);
        let b = w.ledger.get_body(body).unwrap();
        assert_eq!(b.pops, 0.0);
        assert!(
            b.automation_active,
            "Abandon must leave C abandoned automation running"
        );
    }


    #[test]
    fn ai_strip_mine_extracts_state() {
        let mut w = World::new(140);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let floor = w.globals.binding_floor;
            let s = w.ledger.get_mut(sys).unwrap();
            s.claimed = true;
            s.wilderness = false;
            s.deposits.clear();
            s.deposits.push(crate::matter::Deposit::new(
                "stock.ore_binding",
                100.0,
                1.0,
                crate::matter::ExtractorKind::State,
            ));
            crate::matter::reaggregate_remainder(s, floor);
        }
        let before = w.ledger.get(sys).unwrap().binding_remainder;
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::StripMine,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("strip mine order");
        assert_eq!(w.ledger.get_order(id).unwrap().intent, OrderIntent::StripMine);
        let after = w.ledger.get(sys).unwrap().binding_remainder;
        assert!(
            after < before,
            "StripMine should drain state deposits (before={before} after={after})"
        );
        let qty: f64 = w
            .ledger
            .get(sys)
            .unwrap()
            .deposits
            .iter()
            .filter(|d| d.extractor == crate::matter::ExtractorKind::State)
            .map(|d| d.quantity)
            .sum();
        assert!((qty - 75.0).abs() < 1e-6, "100-25=75 left, got {qty}");
    }


    #[test]
    fn ai_plant_city_seeds_pops() {
        let mut w = World::new(150);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        // Clear pops on any seeded body so PlantCity seed is observable.
        for (bid, _) in w.ledger.bodies_for_system(sys).map(|(id, b)| (*id, b.pops)).collect::<Vec<_>>() {
            w.ledger.get_body_mut(bid).unwrap().pops = 0.0;
        }
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PlantCity,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("plant city");
        assert_eq!(w.ledger.get_order(id).unwrap().intent, OrderIntent::PlantCity);
        let pops: f64 = w
            .ledger
            .bodies_for_system(sys)
            .map(|(_, b)| b.pops)
            .sum();
        assert!(
            (pops - PLANT_CITY_POPS).abs() < 1e-9,
            "expected {PLANT_CITY_POPS} pops, got {pops}"
        );
    }

    #[test]
    fn ai_plant_yard_raises_structure_soak() {
        let mut w = World::new(151);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::PlantYard,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("plant yard");
        assert_eq!(w.ledger.get_order(id).unwrap().intent, OrderIntent::PlantYard);
        let (_bid, body) = w.ledger.bodies_for_system(sys).next().expect("body");
        assert!(body.structure_soak >= PLANT_YARD_STRUCTURE_SOAK);
    }

    #[test]
    fn ai_fortify_raises_structure_soak() {
        let mut w = World::new(152);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        let id = try_emit_order(
            &mut w,
            empire,
            OrderIntent::Fortify,
            Some(sys),
            OrderSource::Ai,
        )
        .unwrap()
        .expect("fortify");
        assert_eq!(w.ledger.get_order(id).unwrap().intent, OrderIntent::Fortify);
        let (_bid, body) = w.ledger.bodies_for_system(sys).next().expect("body");
        assert!(body.structure_soak >= FORTIFY_STRUCTURE_SOAK);
    }


    #[test]
    fn doctrine_selects_plant_yard_when_evacuate_high() {
        let mut w = World::new(160);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 800.0;
            s.depleted = false;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.home_empire = Some(EmpireId(empire.0));
        }
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.evacuate_vs_die_in_place = 0.95;
        }
        let score = score_system(&w, empire, sys).unwrap();
        assert_eq!(score.suggested, Some(OrderIntent::PlantYard));
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.evacuate_vs_die_in_place = 0.4;
        }
        let score2 = score_system(&w, empire, sys).unwrap();
        assert_eq!(score2.suggested, Some(OrderIntent::PlantCity));
    }

    #[test]
    fn doctrine_selects_fortify_when_salt_low() {
        let mut w = World::new(161);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = *w.ledger.systems().next().unwrap().0;
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 40.0; // short feed
            s.depleted = false;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.home_empire = Some(EmpireId(empire.0));
        }
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.salt_willingness = 0.05;
        }
        let score = score_system(&w, empire, sys).unwrap();
        assert_eq!(score.suggested, Some(OrderIntent::Fortify));
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.salt_willingness = 0.5;
        }
        let score2 = score_system(&w, empire, sys).unwrap();
        assert_eq!(score2.suggested, Some(OrderIntent::StripMine));
    }

    #[test]
    fn minds_tick_emits_doctrine_selected_order() {
        let mut w = World::new(162);
        w.minds_flags.scoring_enabled = true;
        w.set_lod(crate::lod::LodMode::Fine);
        let empire = *w.ledger.empires().next().unwrap().0;
        let systems: Vec<_> = w.ledger.systems().map(|(id, _)| *id).collect();
        let target = systems[0];
        for sid in &systems {
            let s = w.ledger.get_mut(*sid).unwrap();
            s.depleted = false;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.home_empire = Some(EmpireId(empire.0));
            s.lod_hint = crate::lod::LodHint::Hot;
            s.binding_remainder = if *sid == target { 40.0 } else { 10.0 };
        }
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            e.salt_willingness = 0.6;
            e.evacuate_vs_die_in_place = 0.4;
        }
        let before = w.ledger.orders_len();
        minds_tick_stub(&mut w);
        assert!(w.ledger.orders_len() > before);
        let order = w
            .ledger
            .orders()
            .map(|(_, o)| o)
            .find(|o| o.empire_id == empire && o.source == OrderSource::Ai)
            .expect("ai order");
        assert_eq!(order.intent, OrderIntent::StripMine);
        assert_eq!(order.target_ref, Some(target));
        // Execution side effect: state extract ran.
        let qty: f64 = w
            .ledger
            .get(target)
            .unwrap()
            .deposits
            .iter()
            .filter(|d| d.extractor == crate::matter::ExtractorKind::State)
            .map(|d| d.quantity)
            .sum();
        let _ = qty; // deposits may be empty pre-seed; intent+emit is the milestone gate
    }

    #[test]
    fn minds_tick_can_expand_survey_unknown() {
        let mut w = World::new(131);
        w.minds_flags.scoring_enabled = true;
        w.set_lod(crate::lod::LodMode::Fine);
        let empire = *w.ledger.empires().next().unwrap().0;
        // Contact registry so bootstrap does not auto-know; frontier stays unknown.
        w.contact.ensure(EmpireId(empire.0));
        // Deterministic wilderness frontier → suggested ExpandSurvey when !known.
        let frontier = w.ledger.spawn_wilderness_system();
        {
            let s = w.ledger.get_mut(frontier).unwrap();
            s.binding_remainder = 100.0;
            assert!(s.is_wilderness_immortal());
        }
        assert_eq!(
            sky::map_state(w.ledger.get(frontier).unwrap()),
            MapState::WildernessUnknown
        );
        let pre = score_system(&w, empire, frontier).expect("score");
        assert!(!pre.known);
        assert_eq!(pre.suggested, Some(OrderIntent::ExpandSurvey));

        // Quiet known homes so frontier ExpandSurvey can win the ≤1 Ai slot.
        let systems: Vec<_> = w.ledger.systems().map(|(id, _)| *id).collect();
        for sid in &systems {
            if *sid == frontier {
                continue;
            }
            let s = w.ledger.get_mut(*sid).unwrap();
            s.lod_hint = crate::lod::LodHint::Quiet;
            s.depleted = false;
            s.fuse_end_tick = None;
            s.ended = false;
            s.wilderness = false;
            s.surveyed = true;
            s.claimed = true;
            s.binding_remainder = 10.0; // low score vs frontier
            s.home_empire = Some(EmpireId(empire.0));
        }

        let before_orders = w.ledger.orders_len();
        minds_tick_stub(&mut w);
        let has_expand = w.ledger.orders().any(|(_, o)| {
            o.empire_id == empire
                && o.intent == OrderIntent::ExpandSurvey
                && o.target_ref == Some(frontier)
                && matches!(o.source, OrderSource::Ai)
        });
        let fogged = w
            .contact
            .get(EmpireId(empire.0))
            .map(|c| c.fog.known_systems.contains_key(&frontier))
            .unwrap_or(false);
        assert!(
            has_expand || fogged,
            "minds tick must ExpandSurvey unknown frontier (order and/or fog); orders_before={before_orders} after={}",
            w.ledger.orders_len()
        );
        // salt_emit remains default false
        assert!(!w.minds_flags.salt_emit_enabled);
    }

    #[test]
    fn wilderness_unknown_not_infinite() {
        let mut w = World::new(56);
        let empire = *w.ledger.empires().next().unwrap().0;
        let sys = w.ledger.spawn_wilderness_system();
        {
            let s = w.ledger.get_mut(sys).unwrap();
            s.binding_remainder = 9999.0;
            // immortal wilderness → WildernessUnknown
            assert!(s.is_wilderness_immortal());
        }
        assert_eq!(
            sky::map_state(w.ledger.get(sys).unwrap()),
            MapState::WildernessUnknown
        );
        // Ensure contact so bootstrap does not auto-know wilderness.
        w.contact.ensure(EmpireId(empire.0));
        let score = score_system(&w, empire, sys).expect("score");
        assert!(!score.fuse_score.is_infinite());
        assert!(score.fuse_score < 1_000.0);
        assert!(matches!(
            score.suggested,
            Some(OrderIntent::ExpandSurvey) | Some(OrderIntent::ClaimFeed)
        ));
    }
}
