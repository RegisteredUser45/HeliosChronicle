//! Cosmology binding catalog (Lock 1 / Issue 6) — catalog v4.
//!
//! Galaxy owns `data/cosmology_catalog.json`. Ids use Lead-locked dotted
//! `kind.snake_name` (e.g. `stock.ore_binding`, `fuel.chemical`). Binding is a
//! **row boolean**, not an id prefix. Spawn rolls `spawn_binding_table.entries`
//! in full.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const EMBEDDED_CATALOG_JSON: &str = include_str!("../data/cosmology_catalog.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogStock {
    pub id: String,
    pub name: String,
    /// Lock 1: binding is a field on the row, not implied by id prefix.
    pub binding: bool,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogFuel {
    pub id: String,
    pub name: String,
    pub tier: u32,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogRare {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub binding: bool,
    #[serde(default)]
    pub stock_ref: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeIoEntry {
    #[serde(default)]
    pub stock: Option<String>,
    #[serde(default)]
    pub rare: Option<String>,
    #[serde(default)]
    pub fuel: Option<String>,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub qty: f64,
    #[serde(default)]
    pub per_burn: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogRecipe {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub bom: Vec<RecipeIoEntry>,
    #[serde(default)]
    pub outputs: Vec<RecipeIoEntry>,
    #[serde(default)]
    pub operating: Vec<RecipeIoEntry>,
    #[serde(default)]
    pub fuel_tier: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogNamed {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub fuel_tier: Option<String>,
    #[serde(default)]
    pub bom_recipe: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnBindingEntry {
    pub stock: String,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnBindingTable {
    #[serde(default)]
    pub notes: String,
    pub entries: Vec<SpawnBindingEntry>,
}

/// Full cosmology catalog document (v4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CosmologyCatalog {
    pub version: u32,
    #[serde(default)]
    pub binding_floor_default: f64,
    pub stocks: Vec<CatalogStock>,
    #[serde(default)]
    pub fuels: Vec<CatalogFuel>,
    #[serde(default)]
    pub rares: Vec<CatalogRare>,
    #[serde(default)]
    pub recipes: Vec<CatalogRecipe>,
    #[serde(default)]
    pub modules: Vec<CatalogNamed>,
    #[serde(default)]
    pub supplies: Vec<CatalogNamed>,
    #[serde(default)]
    pub facilities: Vec<CatalogNamed>,
    pub spawn_binding_table: SpawnBindingTable,
}

impl CosmologyCatalog {
    pub fn binding_stocks(&self) -> impl Iterator<Item = &CatalogStock> {
        self.stocks.iter().filter(|s| s.binding)
    }

    pub fn binding_stock_ids(&self) -> Vec<&str> {
        self.binding_stocks().map(|s| s.id.as_str()).collect()
    }

    /// Lock 6 day-one spawn table stock ids (all binding stocks eligible).
    pub fn spawn_stock_ids(&self) -> Vec<&str> {
        self.spawn_binding_table
            .entries
            .iter()
            .map(|e| e.stock.as_str())
            .collect()
    }

    pub fn spawn_weights(&self) -> Vec<(String, f64)> {
        self.spawn_binding_table
            .entries
            .iter()
            .map(|e| (e.stock.clone(), e.weight))
            .collect()
    }

    pub fn contains_stock(&self, id: &str) -> bool {
        self.stocks.iter().any(|s| s.id == id)
    }
}

pub fn parse_catalog(json: &str) -> Result<CosmologyCatalog, serde_json::Error> {
    serde_json::from_str(json)
}

/// Embedded day-one catalog (compiled from frozen `data/cosmology_catalog.json`).
pub fn embedded_catalog() -> &'static CosmologyCatalog {
    static CATALOG: OnceLock<CosmologyCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        parse_catalog(EMBEDDED_CATALOG_JSON).expect("embedded cosmology_catalog.json must parse")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_v4_frozen_ids() {
        let c = embedded_catalog();
        assert_eq!(c.version, 4);
        let spawn = c.spawn_stock_ids();
        for id in [
            "stock.ore_binding",
            "stock.volatiles",
            "stock.silicates",
            "stock.fissiles",
            "stock.organics",
            "stock.rare_earth",
            "stock.antimatter_precursor",
        ] {
            assert!(spawn.contains(&id), "missing spawn entry {id}");
            assert!(c.contains_stock(id));
            assert!(
                c.stocks.iter().find(|s| s.id == id).unwrap().binding,
                "{id} must be binding:true"
            );
        }
        assert!(c.contains_stock("stock.ore_common"));
        assert!(!c.stocks.iter().find(|s| s.id == "stock.ore_common").unwrap().binding);
        assert_eq!(spawn.len(), 7);
        assert!(c.fuels.iter().any(|f| f.id == "fuel.chemical"));
        assert!(c.rares.iter().any(|r| r.id == "rare.catalyst"));
        for id in [
            "recipe.yard_mk1",
            "recipe.habitat_seal_mk1",
            "recipe.mine_auto_mk1",
            "recipe.tankage_mk1",
        ] {
            assert!(c.recipes.iter().any(|r| r.id == id), "missing {id}");
        }
        assert!(c.modules.iter().any(|m| m.id == "module.engine_chem"));
        assert!(c.modules.iter().any(|m| m.id == "module.tankage"));
        assert!(c.facilities.iter().any(|f| f.id == "facility.lab"));
    }
}
