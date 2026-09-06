//! Phase P — Politics: directed standing driven by typed events + knowledge paths.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::entity::EmpireId;
use crate::event::{ChronicleEvent, EventKind};
use crate::knowledge::{CarrierId, KoGrade};
use crate::world::World;

/// Standing delta weights (simple stub constants).
pub const STANDING_RUMOR: i32 = 5;
pub const STANDING_CONFIRMED: i32 = 15;
pub const STANDING_SALT_VICTIM: i32 = 40;
pub const STANDING_FIRST_CONTACT: i32 = 2;
pub const STANDING_VIOLENCE_VICTIM: i32 = 25;
/// Extra mutual standing hit when a ReparationsStub treaty is broken.
pub const STANDING_REPARATIONS_BREACH: i32 = 10;
/// Standing delta multiplier numerator when NonAggression treaty holds (half impact).
pub const NON_AGGRESSION_SOFTEN_NUM: i32 = 1;
pub const NON_AGGRESSION_SOFTEN_DEN: i32 = 2;

/// Directed standing table: key `(a, b)` is how **a** feels about **b** (a → b).
///
/// Negative = hostility toward b; positive = favor. Not symmetrized — (a,b) and
/// (b,a) are independent. Documented directed model for Lock 9 / Lead Q1 paths.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StandingStore {
    /// Map `(from, to) -> standing`.
    pub table: BTreeMap<(EmpireId, EmpireId), i32>,
}

impl StandingStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, from: EmpireId, to: EmpireId) -> i32 {
        self.table.get(&(from, to)).copied().unwrap_or(0)
    }

    pub fn fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.table.len().hash(&mut h);
        for ((a, b), v) in &self.table {
            a.0.hash(&mut h);
            b.0.hash(&mut h);
            v.hash(&mut h);
        }
        h.finish()
    }
}

fn set_standing(world: &mut World, a: EmpireId, b: EmpireId, new: i32, reason_seq: u64) {
    let old = world.standing.get(a, b);
    if old == new {
        return;
    }
    world.standing.table.insert((a, b), new);
    let tick = world.master_tick();
    world.log.append(
        tick,
        EventKind::StandingChanged {
            a,
            b,
            old,
            new,
            reason_seq,
        },
    );
}

fn bump_standing(world: &mut World, a: EmpireId, b: EmpireId, delta: i32, reason_seq: u64) {
    if a == b || delta == 0 {
        return;
    }
    let old = world.standing.get(a, b);
    set_standing(world, a, b, old.saturating_add(delta), reason_seq);
}



/// Extra standing penalty when a treaty with ReparationsStub is broken (G/P).
pub fn apply_reparations_breach(world: &mut World, a: EmpireId, b: EmpireId, seq: u64) {
    bump_standing(world, a, b, -STANDING_REPARATIONS_BREACH, seq);
    bump_standing(world, b, a, -STANDING_REPARATIONS_BREACH, seq);
}

fn violence_standing_delta(world: &World, actor: EmpireId, victim: EmpireId, base: i32) -> i32 {
    if crate::contact::has_clause(world, actor, victim, crate::contact::TreatyClause::NonAggression) {
        (base * NON_AGGRESSION_SOFTEN_NUM) / NON_AGGRESSION_SOFTEN_DEN
    } else {
        base
    }
}

fn is_violence(kind: &EventKind) -> Option<(EmpireId, EmpireId)> {
    match kind {
        EventKind::OrbitalStrike { actor, victim, .. }
        | EventKind::BombardmentLayerWrite { actor, victim, .. }
        | EventKind::SurfaceCombat { actor, victim, .. }
        | EventKind::GlassAttempt { actor, victim, .. }
        | EventKind::Salt { actor, victim, .. } => Some((*actor, *victim)),
        _ => None,
    }
}

/// Apply standing rules for a chronicle event (P consumes; same model for AI and hand).
///
/// - Victims auto-react on Salt/violence against their pops/worlds (no KO gate).
/// - Witnesses only when they acquire/confirm a KO (Lock 9).
/// - Rumor ±5, confirmed ±15, salt victim ±40, first_contact +2 mutual.
pub fn apply_event_for_standing(world: &mut World, event: &ChronicleEvent) {
    let seq = event.seq;
    match &event.kind {
        EventKind::FirstContact { a, b } => {
            bump_standing(world, *a, *b, STANDING_FIRST_CONTACT, seq);
            bump_standing(world, *b, *a, STANDING_FIRST_CONTACT, seq);
        }
        EventKind::TreatySigned { a, b, .. } => {
            bump_standing(world, *a, *b, STANDING_FIRST_CONTACT, seq);
            bump_standing(world, *b, *a, STANDING_FIRST_CONTACT, seq);
        }
        EventKind::TreatyBroken { a, b, .. } => {
            bump_standing(world, *a, *b, -STANDING_FIRST_CONTACT, seq);
            bump_standing(world, *b, *a, -STANDING_FIRST_CONTACT, seq);
        }
        EventKind::ContractDefault { a, b, .. } => {
            bump_standing(world, *a, *b, -STANDING_FIRST_CONTACT, seq);
            bump_standing(world, *b, *a, -STANDING_FIRST_CONTACT, seq);
        }
        EventKind::Salt { actor, victim, .. } => {
            // Victim auto-react (Lead Q1) — hostility toward actor.
            let d = violence_standing_delta(world, *actor, *victim, STANDING_SALT_VICTIM);
            bump_standing(world, *victim, *actor, -d, seq);
        }
        EventKind::OrbitalStrike { .. }
        | EventKind::BombardmentLayerWrite { .. }
        | EventKind::SurfaceCombat { .. }
        | EventKind::GlassAttempt { .. } => {
            if let Some((actor, victim)) = is_violence(&event.kind) {
                let d = violence_standing_delta(world, actor, victim, STANDING_VIOLENCE_VICTIM);
                bump_standing(world, victim, actor, -d, seq);
            }
        }
        EventKind::KoAcquired { ko, empire } => {
            apply_witness_from_ko(world, *ko, *empire, seq, false);
        }
        EventKind::KoConfirmed { ko } => {
            // Every current empire carrier re-scores the upgrade (confirmed weight).
            let carriers: Vec<EmpireId> = world
                .knowledge
                .get(*ko)
                .map(|k| {
                    k.carriers
                        .iter()
                        .filter_map(|c| match c {
                            CarrierId::Empire(e) => Some(*e),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            for empire in carriers {
                apply_witness_from_ko(world, *ko, empire, seq, true);
            }
        }
        _ => {}
    }
}

/// Witness standing: only along knowledge paths; skip if empire is the victim.
fn apply_witness_from_ko(
    world: &mut World,
    ko_id: crate::entity::EntityId,
    empire: EmpireId,
    reason_seq: u64,
    on_confirm: bool,
) {
    let Some(ko) = world.knowledge.get(ko_id).cloned() else {
        return;
    };
    // Victim already auto-reacted — KO path is for third parties.
    if ko.payload.who_victim == Some(empire) {
        return;
    }
    let Some(actor) = ko.payload.who_actor else {
        return;
    };
    if actor == empire {
        return;
    }

    let weight = match ko.grade {
        KoGrade::Rumor => STANDING_RUMOR,
        KoGrade::Confirmed => STANDING_CONFIRMED,
    };

    // On confirm upgrade: apply the delta between confirmed and rumor so we do
    // not double-count the full confirmed weight on top of a prior rumor hit.
    let delta = if on_confirm {
        match ko.grade {
            KoGrade::Confirmed => -(STANDING_CONFIRMED - STANDING_RUMOR),
            KoGrade::Rumor => 0, // should not happen after confirm_ko
        }
    } else {
        -weight
    };

    // Severity scales slightly (stub): severity 0 keeps base, higher adds more.
    let scaled = delta.saturating_sub(ko.payload.severity as i32);
    bump_standing(world, empire, actor, scaled, reason_seq);
}

/// Emit a Salt cruelty event and apply victim standing.
///
/// When a body exists on `system`, delegates to [`crate::violence::salt_world`]
/// for high-rate layer writes + KO emission (Phase H). Otherwise keeps the
/// system-only path (Salt event + standing + fine-hot, no layer write).
pub fn emit_salt(
    world: &mut World,
    actor: EmpireId,
    victim: EmpireId,
    system: crate::entity::EntityId,
) {
    let body_id = world
        .ledger
        .bodies_for_system(system)
        .next()
        .map(|(id, _)| *id);
    if let Some(body_id) = body_id {
        let _ = crate::violence::salt_world(world, actor, victim, system, body_id);
        return;
    }
    crate::violence::salt_system_only(world, actor, victim, system);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contact::{sign_treaty, TreatyClause};
    use crate::entity::{EmpireId, EntityId, EnvLayers};
    use crate::violence::{strike_layers, StrikeKind};
    use crate::world::World;

    #[test]
    fn non_aggression_softens_violence_standing() {
        let mut w = World::new(70);
        let system = *w.ledger().systems().next().unwrap().0;
        let body = w.ledger.spawn_body(system);
        let actor = EmpireId(1);
        let victim = EmpireId(2);
        sign_treaty(&mut w, actor, victim, vec![TreatyClause::NonAggression]);
        strike_layers(
            &mut w,
            actor,
            victim,
            system,
            body,
            StrikeKind::OrbitalStrike,
            EnvLayers {
                atmosphere_pressure: 0.0,
                temperature: 0.0,
                radiation: 1.0,
                toxins_fallout: 0.0,
                biosphere: 0.0,
            },
        )
        .unwrap();
        let st = w.standing.get(victim, actor);
        // TreatySigned also bumps +STANDING_FIRST_CONTACT each way before the strike.
        assert_eq!(
            st,
            STANDING_FIRST_CONTACT - STANDING_VIOLENCE_VICTIM / 2
        );
    }
}
