//! Phase D — Worlds: envelope deficits, env layers, automation-on-ash hooks.
//!
//! Body/layer records live on the shared entity ledger (`BodyEntity`).
//! Species envelope globals live on `Globals::envelope`.
//! Catalog life-support v1 drains `stock.organics` + `stock.volatiles` (C applies).

use serde::{Deserialize, Serialize};

use crate::entity::{BodyEntity, EnvLayers};
use crate::globals::SpeciesEnvelope;

/// Continuous deficit overflow after structure/power/upkeep soak.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct DeficitReport {
    pub mortality: f64,
    pub fertility: f64,
    pub labor: f64,
}

fn band_deficit(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min - value
    } else if value > max {
        value - max
    } else {
        0.0
    }
}

/// Continuous bands vs envelope. Soak structure/power/upkeep first; overflow →
/// mortality / fertility / labor. Pops ≤ 0 → no life-support bill (zeros).
pub fn compute_deficits(body: &BodyEntity, envelope: &SpeciesEnvelope) -> DeficitReport {
    if body.pops <= 0.0 {
        return DeficitReport::default();
    }
    let layers = &body.layers;
    let raw = band_deficit(layers.atmosphere_pressure, envelope.pressure.min, envelope.pressure.max)
        + band_deficit(layers.temperature, envelope.temperature.min, envelope.temperature.max)
        + band_deficit(layers.radiation, envelope.radiation.min, envelope.radiation.max)
        // toxins: envelope has no toxin band — any positive toxins_fallout is hostile
        + layers.toxins_fallout.max(0.0)
        // biosphere shortfall vs ideal 1.0 (simple continuous stub)
        + (1.0 - layers.biosphere).max(0.0);

    let soak = (body.structure_soak + body.power_soak + body.upkeep_soak).max(0.0);
    let overflow = (raw - soak).max(0.0);
    // Split overflow across the three channels (equal stub weights).
    let third = overflow / 3.0;
    DeficitReport {
        mortality: third,
        fertility: third,
        labor: third,
    }
}

/// Optional fuse-end / weapon layer burst (B/H write the same columns).

/// Apply deficit mortality to pops for one tick (`dt` scales lightly). Returns new pops.

/// Day-one life-support bill (DEF): when pops>0, need organics+volatiles from a system.
/// Returns (organics_need, volatiles_need) for `dt` ticks; caller (C/matter) may consume.
pub fn life_support_bill(body: &BodyEntity, envelope: &SpeciesEnvelope, dt: u64) -> (f64, f64) {
    if body.pops <= 0.0 {
        return (0.0, 0.0);
    }
    let scale = body.pops.max(0.0) * dt as f64;
    (
        envelope.calories_need.max(0.0) * scale * 0.01,
        envelope.water_need.max(0.0) * scale * 0.01,
    )
}

/// Apply life-support by consuming `stock.organics` + `stock.volatiles` at the body's system.
/// Shortfall adds mortality-style pop loss (no separate supply.* in catalog v4).
pub fn apply_life_support_drain(
    world: &mut crate::world::World,
    body_id: crate::entity::EntityId,
    dt: u64,
) -> f64 {
    let (system, pops, bill) = {
        let Some(body) = world.ledger.get_body(body_id) else {
            return 0.0;
        };
        if body.pops <= 0.0 {
            return 0.0;
        }
        let bill = life_support_bill(body, &world.globals.envelope, dt);
        (body.system, body.pops, bill)
    };
    let (need_org, need_vol) = bill;
    // Consume from deposits/salvage via try_run style peek+consume helpers inline
    let mut shortfall = 0.0;
    for (stock, need) in [("stock.organics", need_org), ("stock.volatiles", need_vol)] {
        if need <= 0.0 {
            continue;
        }
        let got = consume_stock_at_system(world, system, stock, need);
        shortfall += (need - got).max(0.0);
    }
    if shortfall > 0.0 {
        if let Some(body) = world.ledger.get_body_mut(body_id) {
            let loss = (shortfall * 0.1).min(body.pops);
            body.pops = (body.pops - loss).max(0.0);
            return loss;
        }
    }
    let _ = pops;
    0.0
}

fn consume_stock_at_system(
    world: &mut crate::world::World,
    system: crate::entity::EntityId,
    stock: &str,
    need: f64,
) -> f64 {
    let Some(sys) = world.ledger.get_mut(system) else {
        return 0.0;
    };
    let mut left = need;
    let mut got = 0.0;
    for d in sys.deposits.iter_mut() {
        if left <= 1e-12 {
            break;
        }
        if d.stock_id != stock {
            continue;
        }
        let take = d.quantity.min(left).max(0.0);
        d.quantity -= take;
        left -= take;
        got += take;
    }
    sys.deposits.retain(|d| d.quantity > 1e-12);
    if left > 1e-12 {
        let have = sys.salvage_by_stock.get(stock).copied().unwrap_or(0.0);
        let take = have.min(left).max(0.0);
        if take > 0.0 {
            let e = sys.salvage_by_stock.entry(stock.to_string()).or_insert(0.0);
            *e = (*e - take).max(0.0);
            if *e <= 1e-12 {
                sys.salvage_by_stock.remove(stock);
            }
            got += take;
        }
    }
    got
}

pub fn apply_pop_deficits(body: &mut BodyEntity, envelope: &SpeciesEnvelope, dt: u64) -> f64 {
    if body.pops <= 0.0 {
        return 0.0;
    }
    let d = compute_deficits(body, envelope);
    let loss = (d.mortality * 0.01 * dt as f64).min(body.pops);
    body.pops = (body.pops - loss).max(0.0);
    body.pops
}

pub fn apply_layer_burst(body: &mut BodyEntity, delta: &EnvLayers) {
    body.layers.atmosphere_pressure += delta.atmosphere_pressure;
    body.layers.temperature += delta.temperature;
    body.layers.radiation += delta.radiation;
    body.layers.toxins_fallout += delta.toxins_fallout;
    body.layers.biosphere += delta.biosphere;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{BodyEntity, EntityId, EnvLayers};
    use crate::globals::SpeciesEnvelope;

    #[test]
    fn pops_zero_no_life_support_bill() {
        let mut body = BodyEntity::new(EntityId(1), EntityId(0));
        body.pops = 0.0;
        body.automation_active = true;
        body.layers.radiation = 99.0;
        let d = compute_deficits(&body, &SpeciesEnvelope::default());
        assert_eq!(d, DeficitReport::default());
    }

    #[test]
    fn soak_then_overflow() {
        let mut body = BodyEntity::new(EntityId(1), EntityId(0));
        body.pops = 100.0;
        body.structure_soak = 5.0;
        body.power_soak = 5.0;
        body.upkeep_soak = 0.0;
        // default envelope radiation max 1.0; set radiation to 21 → deficit 20; soak 10 → overflow 10
        body.layers.radiation = 21.0;
        let d = compute_deficits(&body, &SpeciesEnvelope::default());
        assert!((d.mortality + d.fertility + d.labor - 10.0).abs() < 1e-9);
    }

    #[test]
    fn layer_burst_writes_columns() {
        let mut body = BodyEntity::new(EntityId(1), EntityId(0));
        apply_layer_burst(
            &mut body,
            &EnvLayers {
                atmosphere_pressure: 0.0,
                temperature: 0.0,
                radiation: 3.0,
                toxins_fallout: 1.0,
                biosphere: -0.5,
            },
        );
        assert!((body.layers.radiation - 3.0).abs() < f64::EPSILON);
        assert!((body.layers.toxins_fallout - 1.0).abs() < f64::EPSILON);
        assert!((body.layers.biosphere - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn deficit_tick_reduces_pops() {
        let mut body = BodyEntity::new(EntityId(3), EntityId(0));
        body.pops = 100.0;
        body.layers.radiation = 50.0;
        let before = body.pops;
        apply_pop_deficits(&mut body, &SpeciesEnvelope::default(), 10);
        assert!(body.pops < before);
        assert!(body.pops > 0.0);
    }

    #[test]
    fn life_support_bill_zero_when_no_pops() {
        let body = BodyEntity::new(EntityId(9), EntityId(0));
        assert_eq!(life_support_bill(&body, &SpeciesEnvelope::default(), 5), (0.0, 0.0));
    }

    #[test]
    fn life_support_drain_consumes_stocks() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::world::World;
        let mut w = World::new(31);
        let system = *w.ledger.systems().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        {
            let b = w.ledger.get_body_mut(body_id).unwrap();
            b.pops = 50.0;
        }
        add_deposit(&mut w, system, Deposit::new("stock.organics", 100.0, 1.0, ExtractorKind::State)).unwrap();
        add_deposit(&mut w, system, Deposit::new("stock.volatiles", 100.0, 1.0, ExtractorKind::State)).unwrap();
        let before_org: f64 = w.ledger.get(system).unwrap().deposits.iter().filter(|d| d.stock_id == "stock.organics").map(|d| d.quantity).sum();
        apply_life_support_drain(&mut w, body_id, 10);
        let after_org: f64 = w.ledger.get(system).unwrap().deposits.iter().filter(|d| d.stock_id == "stock.organics").map(|d| d.quantity).sum();
        assert!(after_org < before_org);
        assert!(w.ledger.get_body(body_id).unwrap().pops > 0.0);
    }
}
