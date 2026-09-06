//! Phase F — Hull schema stubs: designs as module lists, instances, fuel-tier motion gate.
//!
//! One required `fuel_tier` per design (catalog `fuel.*`). Wrong tier ⇒ no move.
//! No prototype failure RNG in v1.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::cosmology::embedded_catalog;
use crate::entity::EntityId;

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
}

impl std::fmt::Display for HullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownModule(id) => write!(f, "unknown module {id}"),
            Self::MixedFuelTiers(a, b) => write!(f, "mixed fuel tiers {a} vs {b}"),
            Self::NoEngine => write!(f, "design has no engine module with fuel_tier"),
            Self::DesignFuelMismatch => write!(f, "instance fuel_tier != design fuel_tier"),
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
}
