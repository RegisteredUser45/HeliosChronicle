//! Phase E — Research schema stubs: lines, segments, labs, empire unlock flags.
//!
//! Cosmology rows are Galaxy-owned; empire unlocks live on progress records only.
//! No prototype failure RNG in v1. Segment duration uses `Globals::research_segment_ticks`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::cosmology::embedded_catalog;
use crate::entity::{EmpireEntity, EntityId};
use crate::globals::Globals;

/// Research line category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechCategory {
    Hull,
    Industry,
    Population,
    Automation,
}

/// One segment inside a named tech line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TechSegment {
    pub id: String,
    pub line_id: String,
    pub index: u32,
    pub display_name: String,
    /// Catalog ids gated before research can complete (stocks/rares/recipes/facilities).
    pub material_gates: Vec<String>,
    /// Catalog module/facility/recipe ids unlocked when researched.
    pub unlocks: Vec<String>,
    pub salvage_skip_allowed: bool,
}

/// Named tech line with ordered segments (no final segment — book can grow).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TechLine {
    pub id: String,
    pub name: String,
    pub category: TechCategory,
    pub segment_ids: Vec<String>,
}

/// Lab capacity stub (RP per research-segment-tick unit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lab {
    pub id: EntityId,
    pub empire_id: EntityId,
    pub capacity: f64,
    /// Queued segment id, if any.
    pub assigned_segment: Option<String>,
    pub progress_rp: f64,
}

/// Day-one stub tech book (refs catalog v4 dotted ids only).
pub fn stub_tech_book() -> (Vec<TechLine>, Vec<TechSegment>) {
    let lines = vec![
        TechLine {
            id: "line.propulsion".into(),
            name: "Propulsion".into(),
            category: TechCategory::Hull,
            segment_ids: vec![
                "seg.chem_drive".into(),
                "seg.fission_drive".into(),
                "seg.fusion_drive".into(),
                "seg.antimatter_drive".into(),
            ],
        },
        TechLine {
            id: "line.industry".into(),
            name: "Industry".into(),
            category: TechCategory::Industry,
            segment_ids: vec![
                "seg.basic_lab".into(),
                "seg.yard".into(),
                "seg.tankage".into(),
                "seg.mine_auto".into(),
            ],
        },
        TechLine {
            id: "line.habitat".into(),
            name: "Habitat".into(),
            category: TechCategory::Population,
            segment_ids: vec!["seg.habitat_seal".into(), "seg.crew_habitat_mod".into()],
        },
    ];

    let segments = vec![
        TechSegment {
            id: "seg.chem_drive".into(),
            line_id: "line.propulsion".into(),
            index: 0,
            display_name: "Chemical drive".into(),
            material_gates: vec!["recipe.fuel_chem_refine".into(), "stock.volatiles".into()],
            unlocks: vec!["module.engine_chem".into()],
            salvage_skip_allowed: true,
        },
        TechSegment {
            id: "seg.fission_drive".into(),
            line_id: "line.propulsion".into(),
            index: 1,
            display_name: "Fission drive".into(),
            material_gates: vec!["recipe.fuel_fission_pellet".into(), "stock.fissiles".into()],
            unlocks: vec!["module.engine_fission".into()],
            salvage_skip_allowed: true,
        },
        TechSegment {
            id: "seg.fusion_drive".into(),
            line_id: "line.propulsion".into(),
            index: 2,
            display_name: "Fusion drive".into(),
            material_gates: vec![
                "recipe.fuel_fusion_pellet".into(),
                "rare.catalyst".into(),
            ],
            unlocks: vec!["module.engine_fusion".into()],
            salvage_skip_allowed: true,
        },
        TechSegment {
            id: "seg.antimatter_drive".into(),
            line_id: "line.propulsion".into(),
            index: 3,
            display_name: "Antimatter drive".into(),
            material_gates: vec![
                "recipe.antimatter_synth".into(),
                "stock.antimatter_precursor".into(),
            ],
            unlocks: vec!["module.engine_antimatter".into()],
            salvage_skip_allowed: true,
        },
        TechSegment {
            id: "seg.basic_lab".into(),
            line_id: "line.industry".into(),
            index: 0,
            display_name: "Basic lab".into(),
            material_gates: vec!["stock.organics".into(), "stock.rare_earth".into()],
            unlocks: vec!["facility.lab".into()],
            salvage_skip_allowed: false,
        },
        TechSegment {
            id: "seg.yard".into(),
            line_id: "line.industry".into(),
            index: 1,
            display_name: "Yard".into(),
            material_gates: vec!["recipe.yard_mk1".into()],
            unlocks: vec!["facility.yard".into()],
            salvage_skip_allowed: false,
        },
        TechSegment {
            id: "seg.tankage".into(),
            line_id: "line.industry".into(),
            index: 2,
            display_name: "Tankage".into(),
            material_gates: vec!["recipe.tankage_mk1".into()],
            unlocks: vec!["module.tankage".into()],
            salvage_skip_allowed: false,
        },
        TechSegment {
            id: "seg.mine_auto".into(),
            line_id: "line.industry".into(),
            index: 3,
            display_name: "Automated mine".into(),
            material_gates: vec!["recipe.mine_auto_mk1".into()],
            unlocks: vec!["facility.mine_auto".into()],
            salvage_skip_allowed: false,
        },
        TechSegment {
            id: "seg.habitat_seal".into(),
            line_id: "line.habitat".into(),
            index: 0,
            display_name: "Habitat seal".into(),
            material_gates: vec!["recipe.habitat_seal_mk1".into()],
            unlocks: vec!["facility.habitat_seal".into()],
            salvage_skip_allowed: false,
        },
        TechSegment {
            id: "seg.crew_habitat_mod".into(),
            line_id: "line.habitat".into(),
            index: 1,
            display_name: "Crew habitat module".into(),
            material_gates: vec!["stock.organics".into()],
            unlocks: vec!["module.crew_habitat".into()],
            salvage_skip_allowed: true,
        },
    ];

    (lines, segments)
}

/// Validate material_gates / unlocks reference known catalog ids (stocks, rares, recipes, modules, facilities).
pub fn gates_ref_catalog(segment: &TechSegment) -> Result<(), String> {
    let cat = embedded_catalog();
    let mut known: BTreeSet<&str> = BTreeSet::new();
    for s in &cat.stocks {
        known.insert(s.id.as_str());
    }
    for r in &cat.rares {
        known.insert(r.id.as_str());
    }
    for r in &cat.recipes {
        known.insert(r.id.as_str());
    }
    for m in &cat.modules {
        known.insert(m.id.as_str());
    }
    for f in &cat.facilities {
        known.insert(f.id.as_str());
    }
    for id in segment.material_gates.iter().chain(segment.unlocks.iter()) {
        if !known.contains(id.as_str()) {
            return Err(format!("unknown catalog id {id}"));
        }
    }
    Ok(())
}

/// Mark a segment researched on the empire progress record (does not mutate cosmology).
pub fn unlock_segment(empire: &mut EmpireEntity, segment: &TechSegment) -> Result<(), String> {
    gates_ref_catalog(segment)?;
    empire.unlocked_segments.insert(segment.id.clone());
    for u in &segment.unlocks {
        empire.unlocked_catalog_ids.insert(u.clone());
    }
    Ok(())
}

/// RP cost stub: one full `research_segment_ticks` worth at capacity 1.0.
pub fn segment_rp_cost(globals: &Globals) -> f64 {
    globals.research_segment_ticks() as f64
}

/// Salvage jump: unlock with incomplete stats flag (no RNG).
pub fn salvage_unlock_segment(
    empire: &mut EmpireEntity,
    segment: &TechSegment,
) -> Result<(), String> {
    if !segment.salvage_skip_allowed {
        return Err("salvage skip not allowed".into());
    }
    unlock_segment(empire, segment)?;
    empire.incomplete_stat_segments.insert(segment.id.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityId;
    use crate::globals::Globals;

    #[test]
    fn stub_book_gates_are_catalog_v4() {
        let (_lines, segs) = stub_tech_book();
        for s in &segs {
            gates_ref_catalog(s).unwrap_or_else(|e| panic!("{}: {e}", s.id));
        }
    }

    #[test]
    fn unlock_writes_empire_progress_not_catalog() {
        let mut empire = EmpireEntity::from_defaults(EntityId(1), &Globals::default(), None);
        let (_l, segs) = stub_tech_book();
        let seg = segs.iter().find(|s| s.id == "seg.chem_drive").unwrap();
        unlock_segment(&mut empire, seg).unwrap();
        assert!(empire.unlocked_segments.contains("seg.chem_drive"));
        assert!(empire.unlocked_catalog_ids.contains("module.engine_chem"));
        // catalog unchanged
        assert!(embedded_catalog()
            .modules
            .iter()
            .any(|m| m.id == "module.engine_chem"));
    }

    #[test]
    fn no_prototype_rng_path() {
        // Compile-time documentation: designed → tooled → buildable only.
        let cost = segment_rp_cost(&Globals::default());
        assert!(cost > 0.0);
    }
}
