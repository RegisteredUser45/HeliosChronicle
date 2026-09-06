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
}
