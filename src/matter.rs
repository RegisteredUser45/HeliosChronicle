//! Phase C — Matter: deposits, C7 binding_remainder aggregation, Lock-8 drains.
//!
//! C owns deposits + remainder aggregation. B owns depleted/fuse/home/wilderness;
//! dry-threshold crossing is a B transition via `sky::check_depletion` after C
//! updates remainder. Catalog v4 dotted ids; non-binding `stock.ore_common`
//! never counts toward remainder.

use serde::{Deserialize, Serialize};

use crate::cosmology::embedded_catalog;
use crate::entity::{EntityId, SystemEntity};
use crate::sky;
use crate::world::World;

/// Who is extracting (Lock 8 — all four drain binding remainder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractorKind {
    State,
    Civilian,
    Foreign,
    AbandonedAuto,
}

impl ExtractorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Civilian => "civilian",
            Self::Foreign => "foreign",
            Self::AbandonedAuto => "abandoned_auto",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "state" => Some(Self::State),
            "civilian" => Some(Self::Civilian),
            "foreign" => Some(Self::Foreign),
            "abandoned_auto" | "abandoned" | "abandonedautomation" => Some(Self::AbandonedAuto),
            _ => None,
        }
    }
}

impl std::fmt::Display for ExtractorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One deposit vein / extraction line on a system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deposit {
    /// Dotted catalog stock id (e.g. `stock.ore_binding`).
    pub stock_id: String,
    pub quantity: f64,
    /// 0..1 typical; extractable value = quantity × accessibility.
    pub accessibility: f64,
    /// First-class drain attribution (state / civilian / foreign / abandoned_auto).
    pub extractor: ExtractorKind,
}

impl Deposit {
    pub fn new(
        stock_id: impl Into<String>,
        quantity: f64,
        accessibility: f64,
        extractor: ExtractorKind,
    ) -> Self {
        Self {
            stock_id: stock_id.into(),
            quantity: quantity.max(0.0),
            accessibility: accessibility.clamp(0.0, 1.0),
            extractor,
        }
    }

    /// Extractable value still in the ground for this deposit.
    pub fn extractable(&self) -> f64 {
        self.quantity.max(0.0) * self.accessibility.clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatterError {
    SystemNotFound(EntityId),
    UnknownStock(String),
    InvalidAmount,
}

impl std::fmt::Display for MatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemNotFound(id) => write!(f, "system {id} not found"),
            Self::UnknownStock(id) => write!(f, "unknown catalog stock {id}"),
            Self::InvalidAmount => write!(f, "amount must be finite and >= 0"),
        }
    }
}

impl std::error::Error for MatterError {}

/// Lock 1: catalog `binding:true` AND extractable × accessibility ≥ floor.
pub fn deposit_counts_as_binding(deposit: &Deposit, binding_floor: f64) -> bool {
    let cat = embedded_catalog();
    let Some(row) = cat.stocks.iter().find(|s| s.id == deposit.stock_id) else {
        // Rares may also be binding; treat rare rows similarly.
        if let Some(rare) = cat.rares.iter().find(|r| r.id == deposit.stock_id) {
            return rare.binding && deposit.extractable() >= binding_floor;
        }
        return false;
    };
    row.binding && deposit.extractable() >= binding_floor
}

/// Sum extractable value over binding deposits only (C7).
pub fn compute_binding_remainder(deposits: &[Deposit], binding_floor: f64) -> f64 {
    deposits
        .iter()
        .filter(|d| deposit_counts_as_binding(d, binding_floor))
        .map(|d| d.extractable())
        .sum()
}

/// Sync `binding_stocks` map from deposit quantities (additive bookkeeping).
fn sync_binding_stocks(sys: &mut SystemEntity) {
    sys.binding_stocks.clear();
    for d in &sys.deposits {
        *sys.binding_stocks.entry(d.stock_id.clone()).or_insert(0.0) += d.quantity.max(0.0);
    }
}

/// Reaggregate system `binding_remainder` from deposits; does **not** call sky.
pub fn reaggregate_remainder(sys: &mut SystemEntity, binding_floor: f64) -> f64 {
    let rem = compute_binding_remainder(&sys.deposits, binding_floor);
    sys.binding_remainder = rem;
    sync_binding_stocks(sys);
    rem
}

/// Write remainder from deposits and ask B to react at dry threshold.
pub fn reaggregate_and_check(world: &mut World, system: EntityId) -> Result<f64, MatterError> {
    let floor = world.globals.binding_floor;
    {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        reaggregate_remainder(sys, floor);
    }
    sky::check_depletion(world, system);
    let rem = world
        .ledger
        .get(system)
        .map(|s| s.binding_remainder)
        .unwrap_or(0.0);
    Ok(rem)
}

/// Add a deposit to a system and reaggregate → check_depletion.
pub fn add_deposit(world: &mut World, system: EntityId, deposit: Deposit) -> Result<(), MatterError> {
    if !embedded_catalog().contains_stock(&deposit.stock_id)
        && !embedded_catalog()
            .rares
            .iter()
            .any(|r| r.id == deposit.stock_id)
    {
        return Err(MatterError::UnknownStock(deposit.stock_id));
    }
    {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        sys.deposits.push(deposit);
    }
    reaggregate_and_check(world, system)?;
    Ok(())
}

/// Drain `amount` of **quantity** from deposits owned by `extractor`.
/// Reaggregates remainder and calls `sky::check_depletion`.
/// Returns quantity actually drained.
pub fn extract(
    world: &mut World,
    system: EntityId,
    extractor: ExtractorKind,
    amount: f64,
) -> Result<f64, MatterError> {
    if !amount.is_finite() || amount < 0.0 {
        return Err(MatterError::InvalidAmount);
    }
    if amount == 0.0 {
        return Ok(0.0);
    }
    let mut left = amount;
    let mut drained = 0.0;
    {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        for d in sys.deposits.iter_mut() {
            if left <= 0.0 {
                break;
            }
            if d.extractor != extractor {
                continue;
            }
            let take = d.quantity.min(left).max(0.0);
            d.quantity -= take;
            left -= take;
            drained += take;
        }
        // Drop empty veins.
        sys.deposits.retain(|d| d.quantity > 1e-12);
    }
    reaggregate_and_check(world, system)?;
    Ok(drained)
}

/// Convenience Lock-8 entry points.
pub fn extract_state(world: &mut World, system: EntityId, amount: f64) -> Result<f64, MatterError> {
    extract(world, system, ExtractorKind::State, amount)
}

pub fn extract_civilian(
    world: &mut World,
    system: EntityId,
    amount: f64,
) -> Result<f64, MatterError> {
    extract(world, system, ExtractorKind::Civilian, amount)
}

pub fn extract_foreign(
    world: &mut World,
    system: EntityId,
    amount: f64,
) -> Result<f64, MatterError> {
    extract(world, system, ExtractorKind::Foreign, amount)
}

pub fn extract_abandoned_auto(
    world: &mut World,
    system: EntityId,
    amount: f64,
) -> Result<f64, MatterError> {
    extract(world, system, ExtractorKind::AbandonedAuto, amount)
}

/// Salvage feed-pipe stub: adds to `salvage_stock` without refilling deposit veins.
/// Does not change binding_remainder.
pub fn salvage_into_feed(world: &mut World, system: EntityId, amount: f64) -> Result<f64, MatterError> {
    if !amount.is_finite() || amount < 0.0 {
        return Err(MatterError::InvalidAmount);
    }
    let sys = world
        .ledger
        .get_mut(system)
        .ok_or(MatterError::SystemNotFound(system))?;
    sys.salvage_stock = sys.salvage_stock.max(0.0) + amount;
    Ok(sys.salvage_stock)
}


/// Optional per-system civilian extraction line (quantity per master-tick).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CivilianLine {
    /// Quantity drained per master-tick while this line is active.
    pub rate: f64,
}

impl CivilianLine {
    pub fn new(rate: f64) -> Self {
        Self { rate: rate.max(0.0) }
    }
}

/// Drain civilian deposits for one LOD step (`dt` master ticks).
///
/// Per system: sum of `civilian_lines` rates, or `globals.civilian_extract_rate`
/// if no lines are configured but Civilian deposits exist.
pub fn tick_civilian_extractors(world: &mut World, dt: u64) {
    if dt == 0 {
        return;
    }
    let default_rate = world.globals.civilian_extract_rate;
    let systems: Vec<EntityId> = world.ledger.systems().map(|(id, _)| *id).collect();
    for id in systems {
        let rate = {
            let Some(sys) = world.ledger.get(id) else {
                continue;
            };
            if sys.ended {
                0.0
            } else {
                let has = sys
                    .deposits
                    .iter()
                    .any(|d| d.extractor == ExtractorKind::Civilian && d.quantity > 0.0);
                let line_rate: f64 = sys.civilian_lines.iter().map(|l| l.rate.max(0.0)).sum();
                if line_rate > 0.0 {
                    line_rate
                } else if has {
                    default_rate
                } else {
                    0.0
                }
            }
        };
        if rate <= 0.0 {
            continue;
        }
        let _ = extract_civilian(world, id, rate * dt as f64);
    }
}

/// Drain abandoned-auto deposits when any body on the system has automation_active.
pub fn tick_abandoned_automation(world: &mut World, dt: u64) {
    if dt == 0 {
        return;
    }
    let rate = world.globals.abandoned_auto_extract_rate;
    if rate <= 0.0 {
        return;
    }
    let systems: Vec<EntityId> = world.ledger.systems().map(|(id, _)| *id).collect();
    for id in systems {
        let active = world
            .ledger
            .bodies()
            .any(|(_, b)| b.system == id && b.automation_active);
        if !active {
            continue;
        }
        let has = world
            .ledger
            .get(id)
            .map(|s| {
                s.deposits
                    .iter()
                    .any(|d| d.extractor == ExtractorKind::AbandonedAuto && d.quantity > 0.0)
            })
            .unwrap_or(false);
        if !has {
            continue;
        }
        let _ = extract_abandoned_auto(world, id, rate * dt as f64);
    }
}

/// Salvage feed with optional per-stock bookkeeping (does not affect binding_remainder / veins).
pub fn salvage_into_feed_stock(
    world: &mut World,
    system: EntityId,
    stock_id: Option<&str>,
    amount: f64,
) -> Result<f64, MatterError> {
    if !amount.is_finite() || amount < 0.0 {
        return Err(MatterError::InvalidAmount);
    }
    let total = salvage_into_feed(world, system, amount)?;
    if let Some(sid) = stock_id {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        *sys.salvage_by_stock.entry(sid.to_string()).or_insert(0.0) += amount;
    }
    Ok(total)
}

fn consume_from_deposits(sys: &mut SystemEntity, stock: &str, need: f64) -> f64 {
    let mut left = need;
    let mut got = 0.0;
    for d in sys.deposits.iter_mut() {
        if left <= 0.0 {
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
    got
}

fn consume_from_salvage_by_stock(sys: &mut SystemEntity, stock: &str, need: f64) -> f64 {
    let entry = sys.salvage_by_stock.entry(stock.to_string()).or_insert(0.0);
    let take = (*entry).min(need).max(0.0);
    *entry -= take;
    if *entry <= 1e-12 {
        sys.salvage_by_stock.remove(stock);
    }
    sys.salvage_stock = (sys.salvage_stock - take).max(0.0);
    take
}

/// Run a cosmology catalog recipe BOM at a system (C chain stub).
///
/// Consumes BOM from deposits first, then salvage_by_stock. Does **not** refill
/// veins. Outputs go to `outputs_stock`. Day-one: no E unlock gate.
pub fn try_run_recipe(
    world: &mut World,
    system: EntityId,
    recipe_id: &str,
) -> Result<bool, MatterError> {
    let cat = embedded_catalog();
    let Some(recipe) = cat.recipes.iter().find(|r| r.id == recipe_id) else {
        return Err(MatterError::UnknownStock(recipe_id.to_string()));
    };

    let mut needs: Vec<(String, f64)> = Vec::new();
    for entry in &recipe.bom {
        let qty = if entry.qty > 0.0 { entry.qty } else { 1.0 };
        if let Some(stock) = &entry.stock {
            needs.push((stock.clone(), qty));
        } else if let Some(rare) = &entry.rare {
            needs.push((rare.clone(), qty));
        }
    }

    {
        let sys = world
            .ledger
            .get(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        for (stock, need) in &needs {
            let in_dep: f64 = sys
                .deposits
                .iter()
                .filter(|d| d.stock_id == *stock)
                .map(|d| d.quantity)
                .sum();
            let in_sal = sys.salvage_by_stock.get(stock).copied().unwrap_or(0.0);
            if in_dep + in_sal + 1e-12 < *need {
                return Ok(false);
            }
        }
    }

    {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        for (stock, need) in &needs {
            let mut left = *need;
            let got = consume_from_deposits(sys, stock, left);
            left -= got;
            if left > 1e-12 {
                let _ = consume_from_salvage_by_stock(sys, stock, left);
            }
        }
    }
    reaggregate_and_check(world, system)?;

    {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(MatterError::SystemNotFound(system))?;
        for entry in &recipe.outputs {
            let qty = if entry.qty > 0.0 { entry.qty } else { 1.0 };
            let key = entry
                .fuel
                .clone()
                .or_else(|| entry.module.clone())
                .or_else(|| entry.stock.clone())
                .unwrap_or_else(|| format!("{recipe_id}:output"));
            *sys.outputs_stock.entry(key).or_insert(0.0) += qty;
        }
        if recipe.outputs.is_empty() {
            *sys.outputs_stock
                .entry(format!("{recipe_id}:done"))
                .or_insert(0.0) += 1.0;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod matter_tests {
    use super::*;
    use crate::entity::EmpireId;
    use crate::event::EventKind;
    use crate::globals::Globals;
    use crate::sky::{self, claim_system};
    use crate::world::World;

    fn claimed_system(w: &mut World) -> EntityId {
        let id = w.ledger.spawn_system();
        // Clear default remainder so C aggregation owns the number.
        if let Some(s) = w.ledger.get_mut(id) {
            s.binding_remainder = 0.0;
            s.deposits.clear();
        }
        id
    }

    #[test]
    fn binding_aggregation_ignores_non_binding() {
        let mut w = World::new(200);
        let id = claimed_system(&mut w);
        let floor = w.globals.binding_floor;
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.ore_common", 10_000.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.ore_binding", 50.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        let rem = w.ledger.get(id).unwrap().binding_remainder;
        assert!(
            (rem - 50.0).abs() < 1e-9,
            "non-binding ore_common must not count; got {rem}"
        );
        assert!(deposit_counts_as_binding(
            &Deposit::new("stock.ore_binding", 50.0, 1.0, ExtractorKind::State),
            floor
        ));
        assert!(!deposit_counts_as_binding(
            &Deposit::new("stock.ore_common", 10_000.0, 1.0, ExtractorKind::State),
            floor
        ));
    }

    #[test]
    fn all_four_extractor_kinds_drain_remainder() {
        let mut w = World::new(201);
        let id = claimed_system(&mut w);
        for (kind, stock) in [
            (ExtractorKind::State, "stock.ore_binding"),
            (ExtractorKind::Civilian, "stock.volatiles"),
            (ExtractorKind::Foreign, "stock.silicates"),
            (ExtractorKind::AbandonedAuto, "stock.fissiles"),
        ] {
            add_deposit(&mut w, id, Deposit::new(stock, 100.0, 1.0, kind)).unwrap();
        }
        let before = w.ledger.get(id).unwrap().binding_remainder;
        assert!((before - 400.0).abs() < 1e-9, "got {before}");

        assert_eq!(extract_state(&mut w, id, 10.0).unwrap(), 10.0);
        assert_eq!(extract_civilian(&mut w, id, 10.0).unwrap(), 10.0);
        assert_eq!(extract_foreign(&mut w, id, 10.0).unwrap(), 10.0);
        assert_eq!(extract_abandoned_auto(&mut w, id, 10.0).unwrap(), 10.0);

        let after = w.ledger.get(id).unwrap().binding_remainder;
        assert!(
            (after - 360.0).abs() < 1e-9,
            "all four must drain; got {after}"
        );
    }

    #[test]
    fn drain_below_dry_threshold_arms_fuse_via_sky() {
        let mut g = Globals::default();
        g.dry_threshold = 10.0;
        g.binding_floor = 1.0;
        g.master_era_length = 1000;
        g.fuse_length_ratio = 0.25;
        let mut w = World::with_globals(202, g);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.ore_binding", 15.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        assert!(!w.ledger.get(id).unwrap().depleted);
        extract_state(&mut w, id, 10.0).unwrap(); // remainder 5 < 10
        let sys = w.ledger.get(id).unwrap();
        assert!(sys.depleted, "must deplete below dry_threshold");
        assert_eq!(sys.fuse_end_tick, Some(250));
        assert!(w.log.events().iter().any(|e| matches!(
            e.kind,
            EventKind::Deplete { system } if system == id
        )));
        assert!(w.log.events().iter().any(|e| matches!(
            &e.kind,
            EventKind::FuseArmed { system, end_tick: 250 } if *system == id
        )));
    }

    #[test]
    fn abandoned_automation_drains() {
        let mut w = World::new(203);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new(
                "stock.organics",
                80.0,
                1.0,
                ExtractorKind::AbandonedAuto,
            ),
        )
        .unwrap();
        // Mark a body as ash automation still running (D field; C drains).
        let body = w.ledger.spawn_body(id);
        w.ledger.get_body_mut(body).unwrap().pops = 0.0;
        w.ledger.get_body_mut(body).unwrap().automation_active = true;

        let before = w.ledger.get(id).unwrap().binding_remainder;
        assert!((before - 80.0).abs() < 1e-9);
        let drained = extract_abandoned_auto(&mut w, id, 30.0).unwrap();
        assert_eq!(drained, 30.0);
        let after = w.ledger.get(id).unwrap().binding_remainder;
        assert!((after - 50.0).abs() < 1e-9);
    }

    #[test]
    fn wilderness_immortal_blocks_deplete_even_if_remainder_low() {
        let mut w = World::new(204);
        let id = w.ledger.spawn_wilderness_system();
        if let Some(s) = w.ledger.get_mut(id) {
            s.deposits.clear();
            s.binding_remainder = 0.0;
        }
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.ore_binding", 5.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        // Drain below dry_threshold while still immortal.
        extract_state(&mut w, id, 5.0).unwrap();
        assert!(
            w.ledger.get(id).unwrap().binding_remainder < w.globals.dry_threshold
        );
        assert!(
            !w.ledger.get(id).unwrap().depleted,
            "wilderness immortal must block deplete until claim/survey"
        );
        assert!(w.ledger.get(id).unwrap().is_wilderness_immortal());

        claim_system(&mut w, id);
        assert!(sky::check_depletion(&mut w, id));
        assert!(w.ledger.get(id).unwrap().depleted);
    }

    #[test]
    fn salvage_feed_does_not_refill_veins() {
        let mut w = World::new(205);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.rare_earth", 40.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        extract_state(&mut w, id, 20.0).unwrap();
        let qty_before: f64 = w
            .ledger
            .get(id)
            .unwrap()
            .deposits
            .iter()
            .map(|d| d.quantity)
            .sum();
        let rem_before = w.ledger.get(id).unwrap().binding_remainder;
        salvage_into_feed(&mut w, id, 100.0).unwrap();
        let qty_after: f64 = w
            .ledger
            .get(id)
            .unwrap()
            .deposits
            .iter()
            .map(|d| d.quantity)
            .sum();
        assert!((qty_after - qty_before).abs() < 1e-9, "veins must not refill");
        assert!(
            (w.ledger.get(id).unwrap().binding_remainder - rem_before).abs() < 1e-9
        );
        assert!((w.ledger.get(id).unwrap().salvage_stock - 100.0).abs() < 1e-9);
        let _ = EmpireId(0); // silence unused in case
    }

    #[test]
    fn accessibility_floor_excludes_specks() {
        let mut w = World::new(206);
        let id = claimed_system(&mut w);
        // qty*acc = 0.5 < floor 1.0 → does not bind
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.ore_binding", 1.0, 0.4, ExtractorKind::State),
        )
        .unwrap();
        assert!(
            (w.ledger.get(id).unwrap().binding_remainder - 0.0).abs() < 1e-9,
            "specks below floor must not count"
        );
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.volatiles", 10.0, 0.5, ExtractorKind::Civilian),
        )
        .unwrap(); // extractable 5.0 >= 1.0
        assert!((w.ledger.get(id).unwrap().binding_remainder - 5.0).abs() < 1e-9);
    }

    #[test]
    fn civilian_tick_drains_without_operator_extract() {
        let mut w = World::new(301);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.volatiles", 50.0, 1.0, ExtractorKind::Civilian),
        )
        .unwrap();
        w.globals.civilian_extract_rate = 5.0;
        let before = w.ledger.get(id).unwrap().binding_remainder;
        tick_civilian_extractors(&mut w, 2);
        let after = w.ledger.get(id).unwrap().binding_remainder;
        assert!((before - after - 10.0).abs() < 1e-9, "before={before} after={after}");
    }

    #[test]
    fn abandoned_auto_tick_requires_automation_active() {
        let mut w = World::new(302);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.fissiles", 40.0, 1.0, ExtractorKind::AbandonedAuto),
        )
        .unwrap();
        w.globals.abandoned_auto_extract_rate = 4.0;
        tick_abandoned_automation(&mut w, 1);
        assert!((w.ledger.get(id).unwrap().binding_remainder - 40.0).abs() < 1e-9);
        let body = w.ledger.spawn_body(id);
        w.ledger.get_body_mut(body).unwrap().automation_active = true;
        tick_abandoned_automation(&mut w, 1);
        assert!((w.ledger.get(id).unwrap().binding_remainder - 36.0).abs() < 1e-9);
    }

    #[test]
    fn try_run_recipe_consumes_bom_no_vein_refill() {
        let mut w = World::new(303);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.volatiles", 20.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        let ok = try_run_recipe(&mut w, id, "recipe.fuel_chem_refine").unwrap();
        assert!(ok);
        let qty: f64 = w
            .ledger
            .get(id)
            .unwrap()
            .deposits
            .iter()
            .filter(|d| d.stock_id == "stock.volatiles")
            .map(|d| d.quantity)
            .sum();
        assert!((qty - 15.0).abs() < 1e-9, "got {qty}");
        assert!(
            w.ledger
                .get(id)
                .unwrap()
                .outputs_stock
                .get("fuel.chemical")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
    }

    #[test]
    fn world_tick_runs_civilian_drains() {
        let mut w = World::new(304);
        let id = claimed_system(&mut w);
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.volatiles", 100.0, 1.0, ExtractorKind::Civilian),
        )
        .unwrap();
        w.globals.civilian_extract_rate = 3.0;
        let before = w.ledger.get(id).unwrap().binding_remainder;
        w.tick(5); // fine dt=1 → 5 ticks × 3 = 15
        let after = w.ledger.get(id).unwrap().binding_remainder;
        assert!(
            (before - after - 15.0).abs() < 1e-6,
            "world tick must drain civilians; before={before} after={after}"
        );
    }

    #[test]
    fn operator_run_recipe_and_civilian_line() {
        use crate::operator::Operator;
        let mut w = World::new(305);
        let id = claimed_system(&mut w);
        {
            let mut op = Operator::new(&mut w);
            op.add_deposit(id, "stock.volatiles", 30.0, 1.0, "state").unwrap();
            let ok = op.run_recipe(id, "recipe.fuel_chem_refine").unwrap();
            assert!(ok);
            op.add_civilian_line(id, 2.0).unwrap();
        }
        // After recipe, volatiles should be 25; add civilian deposit for tick drain
        add_deposit(
            &mut w,
            id,
            Deposit::new("stock.organics", 20.0, 1.0, ExtractorKind::Civilian),
        )
        .unwrap();
        assert_eq!(w.ledger.get(id).unwrap().civilian_lines.len(), 1);
        assert!(
            w.ledger
                .get(id)
                .unwrap()
                .outputs_stock
                .get("fuel.chemical")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
    }
}
