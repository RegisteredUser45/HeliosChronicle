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
    YardMaterials,
    InsufficientFuel,
    SystemNotFound,
    UnknownAmmo(String),
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
            Self::YardMaterials => write!(f, "yard materials missing (hull_plate recipe)"),
            Self::InsufficientFuel => write!(f, "insufficient fuel"),
            Self::SystemNotFound => write!(f, "system not found"),
            Self::UnknownAmmo(id) => write!(f, "unknown ammo {id}"),
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


/// Burn fuel for a move (B calls after can_move). No RNG.

/// Draw `fuel_tier` from a system's `outputs_stock` (C recipe outputs) into the ship tank.
pub fn refuel_from_system(
    world: &mut World,
    ship_id: EntityId,
    system: EntityId,
    qty: f64,
) -> Result<f64, HullError> {
    if !qty.is_finite() || qty <= 0.0 {
        return Err(HullError::BadFuel);
    }
    let tier = world
        .ships
        .get(&ship_id)
        .ok_or(HullError::ShipNotFound)?
        .fuel_tier
        .clone();
    {
        let sys = world
            .ledger
            .get_mut(system)
            .ok_or(HullError::SystemNotFound)?;
        let have = sys.outputs_stock.get(&tier).copied().unwrap_or(0.0);
        if have + 1e-12 < qty {
            return Err(HullError::InsufficientFuel);
        }
        *sys.outputs_stock.get_mut(&tier).unwrap() = (have - qty).max(0.0);
        if sys.outputs_stock.get(&tier).copied().unwrap_or(0.0) <= 1e-12 {
            sys.outputs_stock.remove(&tier);
        }
    }
    let ship = world.ships.get_mut(&ship_id).ok_or(HullError::ShipNotFound)?;
    refuel(ship, &tier, qty)?;
    Ok(ship.fuel_qty)
}

/// Refine `recipe.fuel_chem_refine` (or matching tier recipe) then tank the produced fuel.
pub fn refine_fuel_at_system(
    world: &mut World,
    ship_id: EntityId,
    system: EntityId,
    recipe_id: &str,
) -> Result<f64, HullError> {
    let ran = crate::matter::try_run_recipe(world, system, recipe_id)
        .map_err(|_| HullError::YardMaterials)?;
    if !ran {
        return Err(HullError::YardMaterials);
    }
    // Pull 1.0 unit of the ship's fuel tier from outputs (chem refine outputs qty 1).
    refuel_from_system(world, ship_id, system, 1.0)
}

pub fn spend_fuel(ship: &mut ShipInstance, amount: f64) -> Result<f64, HullError> {
    if !amount.is_finite() || amount < 0.0 {
        return Err(HullError::BadFuel);
    }
    if ship.fuel_qty + 1e-12 < amount {
        return Err(HullError::InsufficientFuel);
    }
    ship.fuel_qty = (ship.fuel_qty - amount).max(0.0);
    Ok(ship.fuel_qty)
}

/// Build at a system yard: consume `recipe.hull_plate` via C matter, then spawn ship.

/// Tool yard after consuming `recipe.yard_mk1` at the system (industry chain).
pub fn tool_yard_at_system(
    world: &mut World,
    empire_id: EntityId,
    design_id: EntityId,
    system: EntityId,
) -> Result<(), HullError> {
    let ran = crate::matter::try_run_recipe(world, system, "recipe.yard_mk1")
        .map_err(|_| HullError::YardMaterials)?;
    if !ran {
        return Err(HullError::YardMaterials);
    }
    tool_yard(world, empire_id, design_id)
}

/// Load magazine ammo on a ship (catalog id → qty). Additive.

/// Spend magazine ammo; errors if empty/unknown.

/// True if ship has `module.weapon_kinetic` on its design and magazine ammo remaining.

/// True if design lists the catalog module id.

/// True if design has `module.sensor_basic` and ship is not wrecked.

/// Stub cargo capacity: 10 per `module.cargo_hold` on the design.
pub fn cargo_capacity(design: &ShipDesign) -> f64 {
    design
        .modules
        .iter()
        .filter(|m| *m == "module.cargo_hold")
        .count() as f64
        * 10.0
}

pub fn can_sense(design: &ShipDesign, ship: &ShipInstance) -> bool {
    design_has_module(design, "module.sensor_basic") && ship.damage < 1.0
}

pub fn design_has_module(design: &ShipDesign, module_id: &str) -> bool {
    design.modules.iter().any(|m| m == module_id)
}

pub fn can_fire(design: &ShipDesign, ship: &ShipInstance, ammo_id: &str) -> bool {
    design.modules.iter().any(|m| m == "module.weapon_kinetic")
        && ship.magazines.get(ammo_id).copied().unwrap_or(0.0) > 0.0
        && ship.damage < 1.0
}

/// Fire once: requires can_fire, spends 1.0 ammo.
pub fn fire_kinetic(design: &ShipDesign, ship: &mut ShipInstance, ammo_id: &str) -> Result<f64, HullError> {
    if !can_fire(design, ship, ammo_id) {
        return Err(HullError::InsufficientFuel);
    }
    spend_magazine(ship, ammo_id, 1.0)
}

pub fn spend_magazine(ship: &mut ShipInstance, ammo_id: &str, qty: f64) -> Result<f64, HullError> {
    if ammo_id.is_empty() {
        return Err(HullError::UnknownAmmo(ammo_id.into()));
    }
    if !qty.is_finite() || qty < 0.0 {
        return Err(HullError::BadFuel);
    }
    let have = ship.magazines.get(ammo_id).copied().unwrap_or(0.0);
    if have + 1e-12 < qty {
        return Err(HullError::InsufficientFuel);
    }
    let left = (have - qty).max(0.0);
    if left <= 1e-12 {
        ship.magazines.remove(ammo_id);
        Ok(0.0)
    } else {
        ship.magazines.insert(ammo_id.to_string(), left);
        Ok(left)
    }
}

pub fn load_magazine(ship: &mut ShipInstance, ammo_id: &str, qty: f64) -> Result<f64, HullError> {
    if ammo_id.is_empty() {
        return Err(HullError::UnknownAmmo(ammo_id.into()));
    }
    if !qty.is_finite() || qty < 0.0 {
        return Err(HullError::BadFuel);
    }
    let e = ship.magazines.entry(ammo_id.to_string()).or_insert(0.0);
    *e = (*e + qty).max(0.0);
    Ok(*e)
}

/// Catalog recipe that produces this fuel tier (day-one map).
pub fn fuel_recipe_for_tier(fuel_tier: &str) -> Option<&'static str> {
    match fuel_tier {
        "fuel.chemical" => Some("recipe.fuel_chem_refine"),
        "fuel.fission" => Some("recipe.fuel_fission_pellet"),
        "fuel.fusion" => Some("recipe.fuel_fusion_pellet"),
        "fuel.antimatter" => Some("recipe.antimatter_synth"),
        _ => None,
    }
}

/// Refine the ship's fuel-tier recipe at system, then tank 1.0 unit.
pub fn refine_ship_tier_fuel(
    world: &mut World,
    ship_id: EntityId,
    system: EntityId,
) -> Result<f64, HullError> {
    let tier = world
        .ships
        .get(&ship_id)
        .ok_or(HullError::ShipNotFound)?
        .fuel_tier
        .clone();
    let recipe = fuel_recipe_for_tier(&tier).ok_or(HullError::BadFuel)?;
    refine_fuel_at_system(world, ship_id, system, recipe)
}

pub fn build_ship_at_system(
    world: &mut World,
    empire_id: EntityId,
    design_id: EntityId,
    system: EntityId,
    fuel_qty: f64,
) -> Result<EntityId, HullError> {
    let ran = crate::matter::try_run_recipe(world, system, "recipe.hull_plate")
        .map_err(|_| HullError::YardMaterials)?;
    if !ran {
        return Err(HullError::YardMaterials);
    }
    build_ship(world, empire_id, design_id, fuel_qty)
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

    #[test]
    fn spend_fuel_blocks_empty_tank() {
        let d = make_design(EntityId(20), "boat", vec!["module.engine_chem".into()]).unwrap();
        let mut s = spawn_instance(EntityId(21), &d, 2.0);
        assert!((spend_fuel(&mut s, 1.5).unwrap() - 0.5).abs() < 1e-9);
        assert!(matches!(spend_fuel(&mut s, 1.0).unwrap_err(), HullError::InsufficientFuel));
        assert!(!can_move(&d, &s) || s.fuel_qty < 1.0);
        s.fuel_qty = 0.0;
        assert!(!can_move(&d, &s));
    }

    #[test]
    fn build_ship_at_system_consumes_hull_plate() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(9);
        let empire = *w.ledger.empires().next().unwrap().0;
        let system = *w.ledger.systems().next().unwrap().0;
        for sid in ["seg.chem_drive", "seg.tankage", "seg.basic_lab", "seg.yard"] {
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment(sid).unwrap()).unwrap();
        }
        // seed BOM for recipe.hull_plate (8 ore_binding + 4 silicates)
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.ore_binding", 8.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.silicates", 4.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        let did = register_design(
            &mut w,
            empire,
            "scout",
            vec!["module.engine_chem".into(), "module.tankage".into()],
        )
        .unwrap();
        tool_yard(&mut w, empire, did).unwrap();
        let sid = build_ship_at_system(&mut w, empire, did, system, 4.0).unwrap();
        assert!(w.ships.contains_key(&sid));
        // second build without restock should fail materials
        assert!(matches!(
            build_ship_at_system(&mut w, empire, did, system, 1.0).unwrap_err(),
            HullError::YardMaterials
        ));
    }

    #[test]
    fn refine_chem_fuel_into_tank() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(13);
        let empire = *w.ledger.empires().next().unwrap().0;
        let system = *w.ledger.systems().next().unwrap().0;
        for sid in ["seg.chem_drive", "seg.tankage", "seg.basic_lab", "seg.yard"] {
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment(sid).unwrap()).unwrap();
        }
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.volatiles", 10.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.ore_binding", 8.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        add_deposit(
            &mut w,
            system,
            Deposit::new("stock.silicates", 4.0, 1.0, ExtractorKind::State),
        )
        .unwrap();
        let did = register_design(
            &mut w,
            empire,
            "tanker",
            vec!["module.engine_chem".into(), "module.tankage".into()],
        )
        .unwrap();
        tool_yard(&mut w, empire, did).unwrap();
        let sid = build_ship_at_system(&mut w, empire, did, system, 0.0).unwrap();
        assert!(!can_move(w.ship_designs.get(&did).unwrap(), w.ships.get(&sid).unwrap()));
        let qty = refine_fuel_at_system(&mut w, sid, system, "recipe.fuel_chem_refine").unwrap();
        assert!((qty - 1.0).abs() < 1e-9);
        assert!(can_move(w.ship_designs.get(&did).unwrap(), w.ships.get(&sid).unwrap()));
        spend_fuel(w.ships.get_mut(&sid).unwrap(), 1.0).unwrap();
        assert!(!can_move(w.ship_designs.get(&did).unwrap(), w.ships.get(&sid).unwrap()));
    }

    #[test]
    fn fuel_recipe_map_covers_tiers() {
        assert_eq!(fuel_recipe_for_tier("fuel.chemical"), Some("recipe.fuel_chem_refine"));
        assert_eq!(fuel_recipe_for_tier("fuel.fusion"), Some("recipe.fuel_fusion_pellet"));
        assert!(fuel_recipe_for_tier("fuel.nope").is_none());
    }

    #[test]
    fn load_magazine_and_tool_yard_at_system() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(17);
        let empire = *w.ledger.empires().next().unwrap().0;
        let system = *w.ledger.systems().next().unwrap().0;
        for sid in ["seg.chem_drive", "seg.tankage", "seg.basic_lab", "seg.yard"] {
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment(sid).unwrap()).unwrap();
        }
        let did = register_design(
            &mut w,
            empire,
            "gunboat",
            vec!["module.engine_chem".into(), "module.tankage".into()],
        )
        .unwrap();
        add_deposit(&mut w, system, Deposit::new("stock.ore_binding", 20.0, 1.0, ExtractorKind::State)).unwrap();
        add_deposit(&mut w, system, Deposit::new("stock.silicates", 8.0, 1.0, ExtractorKind::State)).unwrap();
        tool_yard_at_system(&mut w, empire, did, system).unwrap();
        let sid = build_ship(&mut w, empire, did, 2.0).unwrap();
        let ship = w.ships.get_mut(&sid).unwrap();
        assert!((load_magazine(ship, "ammo.kinetic", 10.0).unwrap() - 10.0).abs() < 1e-9);
        let did2 = register_design(
            &mut w,
            empire,
            "gunboat2",
            vec!["module.engine_chem".into(), "module.tankage".into()],
        )
        .unwrap();
        assert!(matches!(
            tool_yard_at_system(&mut w, empire, did2, system).unwrap_err(),
            HullError::YardMaterials
        ));
    }

    #[test]
    fn spend_magazine_empty_errors() {
        let d = make_design(EntityId(30), "g", vec!["module.engine_chem".into()]).unwrap();
        let mut s = spawn_instance(EntityId(31), &d, 1.0);
        load_magazine(&mut s, "ammo.kinetic", 5.0).unwrap();
        assert!((spend_magazine(&mut s, "ammo.kinetic", 2.0).unwrap() - 3.0).abs() < 1e-9);
        assert!(matches!(spend_magazine(&mut s, "ammo.kinetic", 9.0).unwrap_err(), HullError::InsufficientFuel));
    }

    #[test]
    fn fire_kinetic_needs_weapon_and_ammo() {
        use crate::research::{find_segment, unlock_segment};
        use crate::world::World;
        let mut w = World::new(41);
        let empire = *w.ledger.empires().next().unwrap().0;
        for sid in ["seg.chem_drive", "seg.weapon_kinetic", "seg.basic_lab", "seg.yard", "seg.tankage"] {
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment(sid).unwrap()).unwrap();
        }
        let did = register_design(
            &mut w,
            empire,
            "gun",
            vec!["module.engine_chem".into(), "module.weapon_kinetic".into()],
        )
        .unwrap();
        tool_yard(&mut w, empire, did).unwrap();
        let sid = build_ship(&mut w, empire, did, 2.0).unwrap();
        let design = w.ship_designs.get(&did).unwrap().clone();
        {
            let ship = w.ships.get_mut(&sid).unwrap();
            assert!(!can_fire(&design, ship, "ammo.kinetic"));
            load_magazine(ship, "ammo.kinetic", 3.0).unwrap();
            assert!(can_fire(&design, ship, "ammo.kinetic"));
            fire_kinetic(&design, ship, "ammo.kinetic").unwrap();
            assert!((ship.magazines.get("ammo.kinetic").copied().unwrap() - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn design_has_module_checks_list() {
        let d = make_design(
            EntityId(50),
            "s",
            vec!["module.engine_chem".into(), "module.sensor_basic".into()],
        )
        .unwrap();
        assert!(design_has_module(&d, "module.sensor_basic"));
        assert!(!design_has_module(&d, "module.weapon_kinetic"));
    }

    #[test]
    fn can_sense_requires_sensor_module() {
        let d = make_design(
            EntityId(60),
            "scout",
            vec!["module.engine_chem".into(), "module.sensor_basic".into()],
        )
        .unwrap();
        let mut s = spawn_instance(EntityId(61), &d, 1.0);
        assert!(can_sense(&d, &s));
        apply_ship_damage(&mut s, 1.0);
        assert!(!can_sense(&d, &s));
        let d2 = make_design(EntityId(62), "blind", vec!["module.engine_chem".into()]).unwrap();
        let s2 = spawn_instance(EntityId(63), &d2, 1.0);
        assert!(!can_sense(&d2, &s2));
    }

    #[test]
    fn cargo_capacity_counts_holds() {
        let d0 = make_design(EntityId(70), "a", vec!["module.engine_chem".into()]).unwrap();
        assert_eq!(cargo_capacity(&d0), 0.0);
        let d1 = make_design(
            EntityId(71),
            "b",
            vec![
                "module.engine_chem".into(),
                "module.cargo_hold".into(),
                "module.cargo_hold".into(),
            ],
        )
        .unwrap();
        assert!((cargo_capacity(&d1) - 20.0).abs() < 1e-9);
    }
}
