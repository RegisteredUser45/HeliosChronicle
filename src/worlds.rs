//! Phase D — Worlds: envelope deficits, env layers, automation-on-ash hooks.
//!
//! Body/layer records live on the shared entity ledger (`BodyEntity`).
//! Species envelope globals live on `Globals::envelope`.
//! Catalog life-support v1 drains `stock.organics` + `stock.volatiles` (C applies).

use serde::{Deserialize, Serialize};

use crate::entity::{BodyEntity, EmpireEntity, EnvLayers};
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

/// Evacuate: clear pops. If `leave_automation`, ash automation may keep draining via C (L8).

/// Facility unlocks soft-boost body soaks (D industry → world). Additive, no RNG.
pub fn apply_facility_soaks(body: &mut BodyEntity, empire: &EmpireEntity) {
    use crate::research::empire_has_unlock;
    if empire_has_unlock(empire, "facility.habitat_seal") {
        body.structure_soak = body.structure_soak.max(2.0);
    }
    if empire_has_unlock(empire, "facility.lab") {
        body.power_soak = body.power_soak.max(1.0);
    }
    if empire_has_unlock(empire, "facility.mine_auto") {
        body.upkeep_soak = body.upkeep_soak.max(1.0);
    }
}

/// Errors for body facility install (Phase D).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldsError {
    BodyNotFound,
    BodySystemMismatch,
    EmpireNotFound,
    FacilityNotUnlocked(String),
    FacilityNotBodyScoped(String),
    NoBomRecipe(String),
    RecipeFailed(String),
    ReaggregateFailed,
}

impl std::fmt::Display for WorldsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodyNotFound => write!(f, "body not found"),
            Self::BodySystemMismatch => write!(f, "body does not belong to system"),
            Self::EmpireNotFound => write!(f, "empire not found"),
            Self::FacilityNotUnlocked(id) => write!(f, "facility not unlocked {id}"),
            Self::FacilityNotBodyScoped(id) => write!(f, "facility not body-scoped {id}"),
            Self::NoBomRecipe(id) => write!(f, "facility has no bom_recipe {id}"),
            Self::RecipeFailed(id) => write!(f, "recipe failed {id}"),
            Self::ReaggregateFailed => write!(f, "binding reaggregate failed"),
        }
    }
}

impl std::error::Error for WorldsError {}

fn is_body_scoped_facility(facility_id: &str) -> bool {
    matches!(
        facility_id,
        "facility.habitat_seal" | "facility.mine_auto" | "facility.lab"
    )
}

/// Read `facility.bom_recipe` from the embedded cosmology catalog.
pub fn facility_bom_recipe(facility_id: &str) -> Option<&'static str> {
    crate::cosmology::embedded_catalog()
        .facilities
        .iter()
        .find(|f| f.id == facility_id)
        .and_then(|f| f.bom_recipe.as_deref())
}

/// Install a body-scoped facility: unlock gate + BOM recipe consume + soak / automation effects.
///
/// Body-scoped only: `facility.habitat_seal`, `facility.mine_auto`, `facility.lab`.
/// `facility.yard` and unknown ids return `FacilityNotBodyScoped`.
pub fn install_facility_on_body(
    world: &mut crate::world::World,
    empire_id: crate::entity::EntityId,
    system: crate::entity::EntityId,
    body_id: crate::entity::EntityId,
    facility_id: &str,
) -> Result<(), WorldsError> {
    use crate::research::empire_has_unlock;

    if !is_body_scoped_facility(facility_id) {
        return Err(WorldsError::FacilityNotBodyScoped(facility_id.to_string()));
    }

    {
        let body = world
            .ledger
            .get_body(body_id)
            .ok_or(WorldsError::BodyNotFound)?;
        if body.system != system {
            return Err(WorldsError::BodySystemMismatch);
        }
    }

    {
        let empire = world
            .ledger
            .get_empire(empire_id)
            .ok_or(WorldsError::EmpireNotFound)?;
        if !empire_has_unlock(empire, facility_id) {
            return Err(WorldsError::FacilityNotUnlocked(facility_id.to_string()));
        }
    }

    let recipe = facility_bom_recipe(facility_id)
        .ok_or_else(|| WorldsError::NoBomRecipe(facility_id.to_string()))?;

    let ran = crate::matter::try_run_recipe(world, system, recipe)
        .map_err(|_| WorldsError::RecipeFailed(recipe.to_string()))?;
    if !ran {
        return Err(WorldsError::RecipeFailed(recipe.to_string()));
    }

    let empire = world
        .ledger
        .get_empire(empire_id)
        .ok_or(WorldsError::EmpireNotFound)?
        .clone();
    let body = world
        .ledger
        .get_body_mut(body_id)
        .ok_or(WorldsError::BodyNotFound)?;
    apply_facility_soaks(body, &empire);
    if facility_id == "facility.mine_auto" {
        body.automation_active = true;
    }
    Ok(())
}

pub fn evacuate_body(body: &mut BodyEntity, leave_automation: bool) {
    body.pops = 0.0;
    if leave_automation {
        body.automation_active = true;
    } else {
        body.automation_active = false;
    }
}

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
/// Lock 8: after D drains binding stocks from deposits, reaggregate C7 remainder + dry check.
pub fn reaggregate_after_life_support_drain(
    world: &mut crate::world::World,
    system: crate::entity::EntityId,
) -> Result<f64, WorldsError> {
    crate::matter::reaggregate_and_check(world, system).map_err(|_| WorldsError::ReaggregateFailed)
}

/// Consume stock then reaggregate when any deposit quantity was taken (Lock 8 binding drain).
pub fn consume_stock_at_system(
    world: &mut crate::world::World,
    system: crate::entity::EntityId,
    stock: &str,
    need: f64,
) -> Result<f64, WorldsError> {
    let (got, from_deposits) = consume_stock_at_system_raw(world, system, stock, need);
    if from_deposits > 1e-12 {
        reaggregate_after_life_support_drain(world, system)?;
    }
    Ok(got)
}

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
    let mut shortfall = 0.0;
    let mut drained_deposits = 0.0;
    for (stock, need) in [("stock.organics", need_org), ("stock.volatiles", need_vol)] {
        if need <= 0.0 {
            continue;
        }
        let (got, from_dep) = consume_stock_at_system_raw(world, system, stock, need);
        drained_deposits += from_dep;
        shortfall += (need - got).max(0.0);
    }
    // One Lock-8 reaggregate after binding organics/volatiles deposit drains.
    if drained_deposits > 1e-12 {
        let _ = reaggregate_after_life_support_drain(world, system);
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

/// Returns `(total_got, amount_taken_from_deposits)`.
fn consume_stock_at_system_raw(
    world: &mut crate::world::World,
    system: crate::entity::EntityId,
    stock: &str,
    need: f64,
) -> (f64, f64) {
    let Some(sys) = world.ledger.get_mut(system) else {
        return (0.0, 0.0);
    };
    let mut left = need;
    let mut from_deposits = 0.0;
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
        from_deposits += take;
    }
    sys.deposits.retain(|d| d.quantity > 1e-12);
    let mut got = from_deposits;
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
    (got, from_deposits)
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

    #[test]
    fn life_support_drain_reaggregates_binding_remainder() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::world::World;
        let mut w = World::new(32);
        let system = *w.ledger.systems().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        {
            let b = w.ledger.get_body_mut(body_id).unwrap();
            b.pops = 50.0;
        }
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.organics", 100.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.volatiles", 100.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        let before_rem = w.ledger.get(system).unwrap().binding_remainder;
        assert!(before_rem > 0.0, "binding remainder should include organics/volatiles");
        apply_life_support_drain(&mut w, body_id, 10);
        let after_rem = w.ledger.get(system).unwrap().binding_remainder;
        assert!(
            after_rem < before_rem - 1e-9,
            "Lock 8: remainder must reaggregate after life-support drain; before={before_rem} after={after_rem}"
        );
    }

    #[test]
    fn evacuate_clears_pops_may_leave_automation() {
        let mut body = BodyEntity::new(EntityId(4), EntityId(0));
        body.pops = 40.0;
        evacuate_body(&mut body, true);
        assert_eq!(body.pops, 0.0);
        assert!(body.automation_active);
        evacuate_body(&mut body, false);
        assert!(!body.automation_active);
    }

    #[test]
    fn facility_unlocks_boost_soaks() {
        use crate::entity::EntityId;
        use crate::globals::Globals;
        use crate::research::{find_segment, unlock_segment};
        let mut body = BodyEntity::new(EntityId(5), EntityId(0));
        let mut empire = crate::entity::EmpireEntity::from_defaults(EntityId(1), &Globals::default(), None);
        apply_facility_soaks(&mut body, &empire);
        assert_eq!(body.structure_soak, 0.0);
        unlock_segment(&mut empire, &find_segment("seg.habitat_seal").unwrap()).unwrap();
        apply_facility_soaks(&mut body, &empire);
        assert!((body.structure_soak - 2.0).abs() < 1e-9);
    }

    #[test]
    fn facility_bom_recipe_from_catalog() {
        assert_eq!(
            facility_bom_recipe("facility.habitat_seal"),
            Some("recipe.habitat_seal_mk1")
        );
        assert_eq!(
            facility_bom_recipe("facility.mine_auto"),
            Some("recipe.mine_auto_mk1")
        );
        assert_eq!(facility_bom_recipe("facility.lab"), None);
        assert_eq!(
            facility_bom_recipe("facility.yard"),
            Some("recipe.yard_mk1")
        );
        assert_eq!(facility_bom_recipe("facility.unknown"), None);
    }

    #[test]
    fn install_habitat_seal_on_body_consumes_recipe_and_soaks() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(77);
        let system = *w.ledger.systems().next().unwrap().0;
        let empire = *w.ledger.empires().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        unlock_segment(
            w.ledger.get_empire_mut(empire).unwrap(),
            &find_segment("seg.habitat_seal").unwrap(),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.organics", 10.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.volatiles", 5.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.ore_binding", 4.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        assert_eq!(w.ledger.get_body(body_id).unwrap().structure_soak, 0.0);
        install_facility_on_body(&mut w, empire, system, body_id, "facility.habitat_seal").unwrap();
        let body = w.ledger.get_body(body_id).unwrap();
        assert!((body.structure_soak - 2.0).abs() < 1e-9);
        let org: f64 = w
            .ledger
            .get(system)
            .unwrap()
            .deposits
            .iter()
            .filter(|d| d.stock_id == "stock.organics")
            .map(|d| d.quantity)
            .sum();
        assert!(org < 1e-9, "BOM organics should be consumed");
    }

    #[test]
    fn install_facility_without_unlock_errors() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::world::World;
        let mut w = World::new(78);
        let system = *w.ledger.systems().next().unwrap().0;
        let empire = *w.ledger.empires().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.organics", 10.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.volatiles", 5.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.ore_binding", 4.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        let err =
            install_facility_on_body(&mut w, empire, system, body_id, "facility.habitat_seal")
                .unwrap_err();
        assert!(matches!(err, WorldsError::FacilityNotUnlocked(_)));
        assert_eq!(w.ledger.get_body(body_id).unwrap().structure_soak, 0.0);
    }

    #[test]
    fn install_mine_auto_sets_automation_and_soaks() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(79);
        let system = *w.ledger.systems().next().unwrap().0;
        let empire = *w.ledger.empires().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        unlock_segment(
            w.ledger.get_empire_mut(empire).unwrap(),
            &find_segment("seg.mine_auto").unwrap(),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.ore_binding", 15.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.rare_earth", 3.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        assert!(!w.ledger.get_body(body_id).unwrap().automation_active);
        install_facility_on_body(&mut w, empire, system, body_id, "facility.mine_auto").unwrap();
        let body = w.ledger.get_body(body_id).unwrap();
        assert!(body.automation_active);
        assert!((body.upkeep_soak - 1.0).abs() < 1e-9);
    }

    #[test]
    fn install_yard_is_not_body_scoped() {
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(80);
        let system = *w.ledger.systems().next().unwrap().0;
        let empire = *w.ledger.empires().next().unwrap().0;
        let body_id = w.ledger.spawn_body(system);
        unlock_segment(
            w.ledger.get_empire_mut(empire).unwrap(),
            &find_segment("seg.yard").unwrap(),
        )
        .unwrap();
        let err =
            install_facility_on_body(&mut w, empire, system, body_id, "facility.yard").unwrap_err();
        assert!(matches!(err, WorldsError::FacilityNotBodyScoped(_)));
    }
}
