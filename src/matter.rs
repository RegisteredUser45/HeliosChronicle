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
}
