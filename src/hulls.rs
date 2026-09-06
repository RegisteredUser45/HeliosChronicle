//! Phase F — Hull schema stubs: designs as module lists, instances, fuel-tier motion gate.
//!
//! One required `fuel_tier` per design (catalog `fuel.*`). Wrong tier ⇒ no move.
//! No prototype failure RNG in v1.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::cosmology::embedded_catalog;
use crate::entity::{EmpireEntity, EntityId};
use crate::event::EventKind;
use crate::research::empire_has_unlock;
use crate::world::World;

/// Ship design: ordered catalog module ids + single required fuel tier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipDesign {
    pub id: EntityId,
    pub name: String,
    /// Catalog `module.*` ids.
    pub modules: Vec<String>,
    /// Exactly one required fuel tier (`fuel.chemical`, …).
    pub fuel_tier: String,
    pub crew_req: f64,
    pub mass: f64,
}

/// Ship instance on the ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipInstance {
    pub id: EntityId,
    pub design_id: EntityId,
    pub fuel_tier: String,
    pub fuel_qty: f64,
    pub damage: f64,
    pub crew: f64,
    pub maintenance: f64,
    /// Magazines by ammo/catalog id.
    #[serde(default)]
    pub magazines: BTreeMap<String, f64>,
    /// Operator god field stubs (H/J consume later).
    #[serde(default)]
    pub planetary_strike: bool,
    #[serde(default)]
    pub sidearm_caliber: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HullError {
    UnknownModule(String),
    MixedFuelTiers(String, String),
    NoEngine,
    DesignFuelMismatch,
    ModuleNotUnlocked(String),
    YardNotTooled,
    DesignNotFound,
    ShipNotFound,
    BadFuel,
    EmpireNotFound,
}

impl std::fmt::Display for HullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownModule(id) => write!(f, "unknown module {id}"),
            Self::MixedFuelTiers(a, b) => write!(f, "mixed fuel tiers {a} vs {b}"),
            Self::NoEngine => write!(f, "design has no engine module with fuel_tier"),
            Self::DesignFuelMismatch => write!(f, "instance fuel_tier != design fuel_tier"),
            Self::ModuleNotUnlocked(id) => write!(f, "module not unlocked {id}"),
            Self::YardNotTooled => write!(f, "yard not tooled for design"),
            Self::DesignNotFound => write!(f, "design not found"),
            Self::ShipNotFound => write!(f, "ship not found"),
            Self::BadFuel => write!(f, "fuel tier/qty invalid"),
            Self::EmpireNotFound => write!(f, "empire not found"),
        }
    }
}

/// Derive the single required fuel_tier from module list (catalog module fuel_tier fields).
pub fn derive_fuel_tier(modules: &[String]) -> Result<String, HullError> {
    let cat = embedded_catalog();
    let mut tier: Option<String> = None;
    let mut saw_engine = false;
    for mid in modules {
        let m = cat
            .modules
            .iter()
            .find(|m| m.id == *mid)
            .ok_or_else(|| HullError::UnknownModule(mid.clone()))?;
        if let Some(ft) = &m.fuel_tier {
            saw_engine = true;
            match &tier {
                None => tier = Some(ft.clone()),
                Some(existing) if existing == ft => {}
                Some(existing) => {
                    return Err(HullError::MixedFuelTiers(existing.clone(), ft.clone()))
                }
            }
        }
    }
    if !saw_engine {
        return Err(HullError::NoEngine);
    }
    Ok(tier.expect("engine implies tier"))
}

/// Build a design with validated single fuel_tier (no prototype RNG).
pub fn make_design(
    id: EntityId,
    name: impl Into<String>,
    modules: Vec<String>,
) -> Result<ShipDesign, HullError> {
    let fuel_tier = derive_fuel_tier(&modules)?;
    // crude stubs
    let crew_req = modules.len() as f64;
    let mass = modules.len() as f64 * 10.0;
    Ok(ShipDesign {
        id,
        name: name.into(),
        modules,
        fuel_tier,
        crew_req,
        mass,
    })
}

/// F motion gate: wrong fuel tier or empty tank ⇒ no move (B owns motion).
pub fn can_move(design: &ShipDesign, instance: &ShipInstance) -> bool {
    instance.design_id == design.id
        && instance.fuel_tier == design.fuel_tier
        && instance.fuel_qty > 0.0
        && instance.damage < 1.0
}

pub fn spawn_instance(id: EntityId, design: &ShipDesign, fuel_qty: f64) -> ShipInstance {
    ShipInstance {
        id,
        design_id: design.id,
        fuel_tier: design.fuel_tier.clone(),
        fuel_qty,
        damage: 0.0,
        crew: design.crew_req,
        maintenance: 1.0,
        magazines: BTreeMap::new(),
        planetary_strike: false,
        sidearm_caliber: 0.0,
    }
}


pub fn apply_ship_damage(ship: &mut ShipInstance, amount: f64) -> f64 {
    let amount = if amount.is_finite() { amount.max(0.0) } else { 0.0 };
    ship.damage = (ship.damage + amount).clamp(0.0, 1.0);
    ship.damage
}

pub fn refuel(ship: &mut ShipInstance, fuel_tier: &str, qty: f64) -> Result<(), HullError> {
    if qty < 0.0 || fuel_tier != ship.fuel_tier { return Err(HullError::BadFuel); }
    ship.fuel_qty += qty;
    Ok(())
}

fn empire_modules_unlocked(empire: &EmpireEntity, modules: &[String]) -> Result<(), HullError> {
    for m in modules {
        if !empire_has_unlock(empire, m) { return Err(HullError::ModuleNotUnlocked(m.clone())); }
    }
    Ok(())
}

pub fn register_design(world: &mut World, empire_id: EntityId, name: impl Into<String>, modules: Vec<String>) -> Result<EntityId, HullError> {
    {
        let empire = world.ledger.get_empire(empire_id).ok_or(HullError::EmpireNotFound)?;
        empire_modules_unlocked(empire, &modules)?;
    }
    let id = world.ledger.alloc_id();
    let design = make_design(id, name, modules)?;
    world.ship_designs.insert(id, design);
    let tick = world.master_tick;
    world.log.append(tick, EventKind::DesignRegistered { empire: empire_id, design: id });
    Ok(id)
}

pub fn tool_yard(world: &mut World, empire_id: EntityId, design_id: EntityId) -> Result<(), HullError> {
    if !world.ship_designs.contains_key(&design_id) { return Err(HullError::DesignNotFound); }
    {
        let empire = world.ledger.get_empire(empire_id).ok_or(HullError::EmpireNotFound)?;
        if !empire_has_unlock(empire, "facility.yard") { return Err(HullError::YardNotTooled); }
    }
    world.ledger.get_empire_mut(empire_id).ok_or(HullError::EmpireNotFound)?.tooled_design_ids.insert(design_id.0);
    let tick = world.master_tick;
    world.log.append(tick, EventKind::YardTooled { empire: empire_id, design: design_id });
    Ok(())
}

pub fn build_ship(world: &mut World, empire_id: EntityId, design_id: EntityId, fuel_qty: f64) -> Result<EntityId, HullError> {
    let tooled = world.ledger.get_empire(empire_id).map(|e| e.tooled_design_ids.contains(&design_id.0)).unwrap_or(false);
    if !tooled { return Err(HullError::YardNotTooled); }
    let design = world.ship_designs.get(&design_id).ok_or(HullError::DesignNotFound)?.clone();
    let id = world.ledger.alloc_id();
    world.ships.insert(id, spawn_instance(id, &design, fuel_qty.max(0.0)));
    let tick = world.master_tick;
    world.log.append(tick, EventKind::ShipBuilt { empire: empire_id, ship: id, design: design_id });
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_fuel_tier_chem_design() {
        let d = make_design(
            EntityId(10),
            "scout",
            vec![
                "module.engine_chem".into(),
                "module.crew_habitat".into(),
                "module.cargo_hold".into(),
            ],
        )
        .unwrap();
        assert_eq!(d.fuel_tier, "fuel.chemical");
    }

    #[test]
    fn mixed_engine_tiers_rejected() {
        let err = make_design(
            EntityId(11),
            "bad",
            vec![
                "module.engine_chem".into(),
                "module.engine_fission".into(),
            ],
        )
        .unwrap_err();
        assert!(matches!(err, HullError::MixedFuelTiers(_, _)));
    }

    #[test]
    fn wrong_fuel_blocks_move() {
        let d = make_design(
            EntityId(12),
            "boat",
            vec!["module.engine_chem".into(), "module.tankage".into()],
        )
        .unwrap();
        let mut inst = spawn_instance(EntityId(13), &d, 5.0);
        assert!(can_move(&d, &inst));
        inst.fuel_tier = "fuel.fusion".into();
        assert!(!can_move(&d, &inst));
        inst.fuel_tier = d.fuel_tier.clone();
        inst.fuel_qty = 0.0;
        assert!(!can_move(&d, &inst));
    }

    #[test]
    fn designed_tooled_built_pipeline() {
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(7);
        let empire = *w.ledger.empires().next().unwrap().0;
        for sid in ["seg.chem_drive", "seg.tankage", "seg.basic_lab", "seg.yard"] {
            let seg = find_segment(sid).unwrap();
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &seg).unwrap();
        }
        let did = register_design(&mut w, empire, "scout", vec!["module.engine_chem".into(), "module.tankage".into()]).unwrap();
        tool_yard(&mut w, empire, did).unwrap();
        let sid = build_ship(&mut w, empire, did, 3.0).unwrap();
        let design = w.ship_designs.get(&did).unwrap().clone();
        let ship = w.ships.get_mut(&sid).unwrap();
        assert!(can_move(&design, ship));
        apply_ship_damage(ship, 1.0);
        assert!(!can_move(&design, ship));
    }

    #[test]
    fn ship_damage_api_for_conflict() {
        let d = make_design(EntityId(1), "x", vec!["module.engine_chem".into()]).unwrap();
        let mut s = spawn_instance(EntityId(2), &d, 1.0);
        assert!((apply_ship_damage(&mut s, 0.25) - 0.25).abs() < 1e-9);
        assert!(can_move(&d, &s));
        apply_ship_damage(&mut s, 0.8);
        assert!(!can_move(&d, &s));
    }
}
