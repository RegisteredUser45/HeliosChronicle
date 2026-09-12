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
    /// Optional system where material_gates are checked (stocks/rares present).
    #[serde(default)]
    pub site_system: Option<EntityId>,
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
        TechLine {
            id: "line.outfitting".into(),
            name: "Outfitting".into(),
            category: TechCategory::Hull,
            segment_ids: vec![
                "seg.sensor_basic".into(),
                "seg.weapon_kinetic".into(),
                "seg.cargo_hold".into(),
            ],
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
        TechSegment {
            id: "seg.sensor_basic".into(),
            line_id: "line.outfitting".into(),
            index: 0,
            display_name: "Basic sensors".into(),
            material_gates: vec!["stock.rare_earth".into(), "recipe.power_core_basic".into()],
            unlocks: vec!["module.sensor_basic".into()],
            salvage_skip_allowed: true,
        },
        TechSegment {
            id: "seg.weapon_kinetic".into(),
            line_id: "line.outfitting".into(),
            index: 1,
            display_name: "Kinetic weapon".into(),
            material_gates: vec!["stock.ore_binding".into(), "recipe.hull_plate".into()],
            unlocks: vec!["module.weapon_kinetic".into()],
            salvage_skip_allowed: true,
        },
        TechSegment {
            id: "seg.cargo_hold".into(),
            line_id: "line.outfitting".into(),
            index: 2,
            display_name: "Cargo hold".into(),
            material_gates: vec!["stock.silicates".into()],
            unlocks: vec!["module.cargo_hold".into()],
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
    // Full research (or re-research) clears salvage incomplete-stats flag.
    empire.incomplete_stat_segments.remove(&segment.id);
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



/// Pay full RP cost again to clear `incomplete_stat_segments` (no RNG). Segment must already be unlocked.
pub fn repair_salvage_segment(
    empire: &mut EmpireEntity,
    segment_id: &str,
    globals: &Globals,
    capacity: f64,
    progress_rp: &mut f64,
    dt: u64,
) -> Result<bool, String> {
    if !empire.unlocked_segments.contains(segment_id) {
        return Err("segment not unlocked".into());
    }
    if !empire.incomplete_stat_segments.contains(segment_id) {
        return Ok(true);
    }
    let cost = segment_rp_cost(globals);
    *progress_rp += capacity.max(0.0) * dt as f64;
    if *progress_rp + f64::EPSILON >= cost {
        empire.incomplete_stat_segments.remove(segment_id);
        *progress_rp = 0.0;
        return Ok(true);
    }
    Ok(false)
}

pub fn find_segment(segment_id: &str) -> Option<TechSegment> {
    stub_tech_book().1.into_iter().find(|s| s.id == segment_id)
}

pub fn line_prereqs_met(empire: &EmpireEntity, segment: &TechSegment) -> bool {
    let (lines, segs) = stub_tech_book();
    let Some(line) = lines.iter().find(|l| l.id == segment.line_id) else { return false; };
    for sid in &line.segment_ids {
        if let Some(s) = segs.iter().find(|s| s.id == *sid) {
            if s.index < segment.index && !empire.unlocked_segments.contains(&s.id) {
                return false;
            }
        }
    }
    true
}

pub fn empire_has_unlock(empire: &EmpireEntity, catalog_id: &str) -> bool {
    empire.unlocked_catalog_ids.contains(catalog_id)
}


pub fn set_lab_site(lab: &mut Lab, system: Option<EntityId>) {
    lab.site_system = system;
}

/// True when each material_gate is satisfiable at `system` without consuming:
/// - `stock.*` / `rare.*`: deposits + salvage_by_stock cover qty≥1
/// - `recipe.*` / `module.*` / `facility.*`: empire already unlocked, or recipe inputs available
pub fn material_gates_met(
    world: &crate::world::World,
    empire: &EmpireEntity,
    system: EntityId,
    segment: &TechSegment,
) -> bool {
    let Some(sys) = world.ledger.get(system) else {
        return false;
    };
    for gate in &segment.material_gates {
        if gate.starts_with("stock.") || gate.starts_with("rare.") {
            let in_dep: f64 = sys
                .deposits
                .iter()
                .filter(|d| d.stock_id == *gate)
                .map(|d| d.quantity)
                .sum();
            let in_sal = sys.salvage_by_stock.get(gate).copied().unwrap_or(0.0);
            if in_dep + in_sal + 1e-12 < 1.0 {
                return false;
            }
        } else if gate.starts_with("recipe.") {
            if empire_has_unlock(empire, gate) {
                continue;
            }
            // soft: treat as met if try_run_recipe would find inputs (peek via deposits)
            // fall through to unlocked-or-present check using catalog BOM
            let cat = embedded_catalog();
            let Some(recipe) = cat.recipes.iter().find(|r| r.id == *gate) else {
                return false;
            };
            for entry in &recipe.bom {
                let stock = entry.stock.as_ref().or(entry.rare.as_ref());
                let Some(stock) = stock else { continue; };
                let need = if entry.qty > 0.0 { entry.qty } else { 1.0 };
                let in_dep: f64 = sys
                    .deposits
                    .iter()
                    .filter(|d| d.stock_id == *stock)
                    .map(|d| d.quantity)
                    .sum();
                let in_sal = sys.salvage_by_stock.get(stock).copied().unwrap_or(0.0);
                if in_dep + in_sal + 1e-12 < need {
                    return false;
                }
            }
        } else if gate.starts_with("module.") || gate.starts_with("facility.") {
            if !empire_has_unlock(empire, gate) {
                return false;
            }
        } else {
            // unknown family — require unlock flag
            if !empire_has_unlock(empire, gate) {
                return false;
            }
        }
    }
    true
}

pub fn make_lab(id: EntityId, empire_id: EntityId, capacity: f64) -> Lab {
    Lab { id, empire_id, capacity: capacity.max(0.0), assigned_segment: None, progress_rp: 0.0, site_system: None }
}

pub fn assign_lab(lab: &mut Lab, segment_id: &str) -> Result<(), String> {
    let seg = find_segment(segment_id).ok_or_else(|| format!("unknown segment {segment_id}"))?;
    gates_ref_catalog(&seg)?;
    lab.assigned_segment = Some(segment_id.to_string());
    lab.progress_rp = 0.0;
    Ok(())
}

pub fn tick_lab(
    lab: &mut Lab,
    empire: &mut EmpireEntity,
    globals: &Globals,
    dt: u64,
) -> Result<Option<String>, String> {
    let Some(seg_id) = lab.assigned_segment.clone() else { return Ok(None); };
    if empire.unlocked_segments.contains(&seg_id) {
        lab.assigned_segment = None;
        lab.progress_rp = 0.0;
        return Ok(None);
    }
    let seg = find_segment(&seg_id).ok_or_else(|| format!("unknown segment {seg_id}"))?;
    if !line_prereqs_met(empire, &seg) {
        return Err(format!("line prereqs unmet for {seg_id}"));
    }
    let cost = segment_rp_cost(globals);
    lab.progress_rp += lab.capacity * dt as f64;
    if lab.progress_rp + f64::EPSILON >= cost {
        unlock_segment(empire, &seg)?;
        lab.assigned_segment = None;
        lab.progress_rp = 0.0;
        return Ok(Some(seg_id));
    }
    Ok(None)
}


/// Advance every lab on the world by `dt`. Completions emit SegmentResearched.
/// Errors on a single lab are skipped (prereq blocks) so the tick stays robust.
pub fn tick_all_labs(world: &mut crate::world::World, dt: u64) {
    let ids: Vec<_> = world.labs.keys().copied().collect();
    for id in ids {
        let _ = tick_lab_on_world(world, id, dt);
    }
}

pub fn tick_lab_on_world(world: &mut crate::world::World, lab_id: EntityId, dt: u64) -> Result<Option<String>, String> {
    use crate::event::EventKind;
    let empire_id = world.labs.get(&lab_id).map(|l| l.empire_id).ok_or_else(|| format!("lab {lab_id} not found"))?;
    let site = world.labs.get(&lab_id).and_then(|l| l.site_system);
    let assigned = world.labs.get(&lab_id).and_then(|l| l.assigned_segment.clone());
    if let (Some(system), Some(seg_id)) = (site, assigned) {
        if let Some(seg) = find_segment(&seg_id) {
            let empire_ref = world.ledger.get_empire(empire_id).ok_or_else(|| format!("empire {empire_id} not found"))?;
            if !material_gates_met(world, empire_ref, system, &seg) {
                return Ok(None); // soft stall — no RP progress
            }
        }
    }
    let globals = world.globals.clone();
    let done = {
        let lab = world.labs.get_mut(&lab_id).unwrap();
        let empire = world.ledger.get_empire_mut(empire_id).ok_or_else(|| format!("empire {empire_id} not found"))?;
        tick_lab(lab, empire, &globals, dt)?
    };
    if let Some(ref seg) = done {
        let tick = world.master_tick;
        world.log.append(tick, EventKind::SegmentResearched { empire: empire_id, segment: seg.clone() });
        // Lab completion → auto-register ShipDesign from module.* unlocks (idempotent).
        let unlocked = crate::hulls::unlock_design_from_completed_segment(world, empire_id, seg);
        // Best-effort auto-tool when lab has a site and empire already has facility.yard.
        if let Ok(Some(design_id)) = unlocked {
            if let Some(sys) = site {
                let _ = crate::hulls::auto_tool_design_if_yard_ready(world, empire_id, design_id, sys);
            }
        }
    }
    Ok(done)
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
        let cost = segment_rp_cost(&Globals::default());
        assert!(cost > 0.0);
    }

    #[test]
    fn lab_tick_completes_segment_without_rng() {
        let g = Globals::default();
        let mut empire = EmpireEntity::from_defaults(EntityId(1), &g, None);
        unlock_segment(&mut empire, &find_segment("seg.basic_lab").unwrap()).unwrap();
        let mut lab = make_lab(EntityId(2), EntityId(1), 1.0);
        assign_lab(&mut lab, "seg.yard").unwrap();
        let mut done = None;
        while done.is_none() {
            done = tick_lab(&mut lab, &mut empire, &g, 100).unwrap();
        }
        assert_eq!(done.as_deref(), Some("seg.yard"));
        assert!(empire.unlocked_catalog_ids.contains("facility.yard"));
    }

    #[test]
    fn line_prereq_blocks_out_of_order() {
        let g = Globals::default();
        let mut empire = EmpireEntity::from_defaults(EntityId(1), &g, None);
        let mut lab = make_lab(EntityId(2), EntityId(1), 1000.0);
        assign_lab(&mut lab, "seg.yard").unwrap();
        assert!(tick_lab(&mut lab, &mut empire, &g, 10_000).unwrap_err().contains("prereqs"));
    }

    #[test]
    fn world_tick_advances_assigned_labs() {
        use crate::event::EventKind;
        use crate::world::World;
        let mut w = World::new(11);
        let empire = *w.ledger.empires().next().unwrap().0;
        unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment("seg.basic_lab").unwrap()).unwrap();
        let lab_id = w.ledger.alloc_id();
        let mut lab = make_lab(lab_id, empire, 1.0);
        assign_lab(&mut lab, "seg.yard").unwrap();
        w.labs.insert(lab_id, lab);
        w.tick(1000);
        let emp = w.ledger.get_empire(empire).unwrap();
        assert!(emp.unlocked_segments.contains("seg.yard"), "yard should complete via world tick");
        assert!(w.log.events().iter().any(|e| matches!(&e.kind, EventKind::SegmentResearched { segment, .. } if segment == "seg.yard")));
    }

    #[test]
    fn outfitting_segments_ref_catalog() {
        for sid in ["seg.sensor_basic", "seg.weapon_kinetic", "seg.cargo_hold"] {
            let seg = find_segment(sid).unwrap();
            gates_ref_catalog(&seg).unwrap();
            assert!(!seg.unlocks.is_empty());
        }
        let (lines, _) = stub_tech_book();
        assert!(lines.iter().any(|l| l.id == "line.outfitting"));
    }

    #[test]
    fn material_gates_stall_without_stocks() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::world::World;
        let mut w = World::new(21);
        let empire = *w.ledger.empires().next().unwrap().0;
        let system = *w.ledger.systems().next().unwrap().0;
        unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment("seg.basic_lab").unwrap()).unwrap();
        let lab_id = w.ledger.alloc_id();
        let mut lab = make_lab(lab_id, empire, 1000.0);
        assign_lab(&mut lab, "seg.yard").unwrap();
        set_lab_site(&mut lab, Some(system));
        w.labs.insert(lab_id, lab);
        // no yard_mk1 BOM → stall
        w.tick(1000);
        assert!(!w.ledger.get_empire(empire).unwrap().unlocked_segments.contains("seg.yard"));
        // seed BOM and progress
        add_deposit(&mut w, system, Deposit::new("stock.ore_binding", 20.0, 1.0, ExtractorKind::State)).unwrap();
        add_deposit(&mut w, system, Deposit::new("stock.silicates", 8.0, 1.0, ExtractorKind::State)).unwrap();
        w.tick(1000);
        assert!(w.ledger.get_empire(empire).unwrap().unlocked_segments.contains("seg.yard"));
    }

    #[test]
    fn salvage_then_repair_clears_incomplete() {
        let g = Globals::default();
        let mut empire = EmpireEntity::from_defaults(EntityId(1), &g, None);
        let seg = find_segment("seg.chem_drive").unwrap();
        salvage_unlock_segment(&mut empire, &seg).unwrap();
        assert!(empire.incomplete_stat_segments.contains("seg.chem_drive"));
        let mut progress = 0.0;
        let mut done = false;
        while !done {
            done = repair_salvage_segment(&mut empire, "seg.chem_drive", &g, 1.0, &mut progress, 100).unwrap();
        }
        assert!(!empire.incomplete_stat_segments.contains("seg.chem_drive"));
    }

    #[test]
    fn unlock_segment_clears_incomplete_flag() {
        let g = Globals::default();
        let mut empire = EmpireEntity::from_defaults(EntityId(1), &g, None);
        let seg = find_segment("seg.chem_drive").unwrap();
        salvage_unlock_segment(&mut empire, &seg).unwrap();
        assert!(empire.incomplete_stat_segments.contains("seg.chem_drive"));
        unlock_segment(&mut empire, &seg).unwrap();
        assert!(!empire.incomplete_stat_segments.contains("seg.chem_drive"));
    }

    #[test]
    fn lab_completion_unlocks_ship_design() {
        use crate::event::EventKind;
        use crate::hulls::unlock_design_from_completed_segment;
        use crate::world::World;
        let mut w = World::new(42);
        let empire = *w.ledger.empires().next().unwrap().0;
        // seg.chem_drive is propulsion index 0 — no line prereqs; no site → no material-gate stall
        let lab_id = w.ledger.alloc_id();
        let mut lab = make_lab(lab_id, empire, 1.0);
        assign_lab(&mut lab, "seg.chem_drive").unwrap();
        w.labs.insert(lab_id, lab);

        let mut done = None;
        let mut guard = 0;
        while done.is_none() && guard < 10_000 {
            done = tick_lab_on_world(&mut w, lab_id, 100).unwrap();
            guard += 1;
        }
        assert_eq!(done.as_deref(), Some("seg.chem_drive"));
        assert!(w
            .log
            .events()
            .iter()
            .any(|e| matches!(&e.kind, EventKind::SegmentResearched { segment, .. } if segment == "seg.chem_drive")));

        let emp = w.ledger.get_empire(empire).unwrap();
        assert!(emp.unlocked_catalog_ids.contains("module.engine_chem"));

        let designs: Vec<_> = w
            .ship_designs
            .values()
            .filter(|d| d.name == "design.chem_drive")
            .collect();
        assert_eq!(designs.len(), 1, "exactly one auto design from chem_drive");
        assert!(designs[0].modules.iter().any(|m| m == "module.engine_chem"));
        assert_eq!(designs[0].fuel_tier, "fuel.chemical");
        let design_id = designs[0].id;

        // Idempotent: second unlock does not duplicate
        let again = unlock_design_from_completed_segment(&mut w, empire, "seg.chem_drive").unwrap();
        assert_eq!(again, Some(design_id));
        assert_eq!(
            w.ship_designs
                .values()
                .filter(|d| d.name == "design.chem_drive")
                .count(),
            1
        );
    }

    #[test]
    fn lab_completion_auto_tools_design_when_yard_ready() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::world::World;
        let mut w = World::new(43);
        let empire = *w.ledger.empires().next().unwrap().0;
        let system = *w.ledger.systems().next().unwrap().0;
        // Empire already has facility.yard (+ industry prereqs).
        for sid in ["seg.basic_lab", "seg.yard", "seg.tankage"] {
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment(sid).unwrap()).unwrap();
        }
        // chem_drive gates at site: stock.volatiles + recipe.fuel_chem_refine BOM (5 volatiles).
        // yard_mk1 BOM for auto-tool (20 ore + 8 silicates).
        add_deposit(&mut w, system, Deposit::new("stock.volatiles", 5.0, 1.0, ExtractorKind::State)).unwrap();
        add_deposit(&mut w, system, Deposit::new("stock.ore_binding", 20.0, 1.0, ExtractorKind::State)).unwrap();
        add_deposit(&mut w, system, Deposit::new("stock.silicates", 8.0, 1.0, ExtractorKind::State)).unwrap();

        let lab_id = w.ledger.alloc_id();
        let mut lab = make_lab(lab_id, empire, 1.0);
        assign_lab(&mut lab, "seg.chem_drive").unwrap();
        set_lab_site(&mut lab, Some(system));
        w.labs.insert(lab_id, lab);

        let mut done = None;
        let mut guard = 0;
        while done.is_none() && guard < 10_000 {
            done = tick_lab_on_world(&mut w, lab_id, 100).unwrap();
            guard += 1;
        }
        assert_eq!(done.as_deref(), Some("seg.chem_drive"));
        let designs: Vec<_> = w
            .ship_designs
            .values()
            .filter(|d| d.name == "design.chem_drive")
            .collect();
        assert_eq!(designs.len(), 1);
        let design_id = designs[0].id;
        assert!(
            w.ledger.get_empire(empire).unwrap().tooled_design_ids.contains(&design_id.0),
            "design should be auto-tooled when site_system + facility.yard + yard BOM present"
        );
    }

    #[test]
    fn lab_completion_soft_fails_auto_tool_without_yard_bom() {
        use crate::matter::{add_deposit, Deposit, ExtractorKind};
        use crate::world::World;
        let mut w = World::new(44);
        let empire = *w.ledger.empires().next().unwrap().0;
        let system = *w.ledger.systems().next().unwrap().0;
        for sid in ["seg.basic_lab", "seg.yard"] {
            unlock_segment(w.ledger.get_empire_mut(empire).unwrap(), &find_segment(sid).unwrap()).unwrap();
        }
        // Site gates for chem_drive met (volatiles), but NO yard_mk1 BOM → unlock + soft-fail tool.
        add_deposit(&mut w, system, Deposit::new("stock.volatiles", 5.0, 1.0, ExtractorKind::State)).unwrap();

        let lab_id = w.ledger.alloc_id();
        let mut lab = make_lab(lab_id, empire, 1.0);
        assign_lab(&mut lab, "seg.chem_drive").unwrap();
        set_lab_site(&mut lab, Some(system));
        w.labs.insert(lab_id, lab);

        let mut done = None;
        let mut guard = 0;
        while done.is_none() && guard < 10_000 {
            done = tick_lab_on_world(&mut w, lab_id, 100).unwrap();
            guard += 1;
        }
        assert_eq!(done.as_deref(), Some("seg.chem_drive"), "lab tick must not fail when auto-tool lacks BOM");
        let designs: Vec<_> = w
            .ship_designs
            .values()
            .filter(|d| d.name == "design.chem_drive")
            .collect();
        assert_eq!(designs.len(), 1, "design still registered");
        let design_id = designs[0].id;
        assert!(
            !w.ledger.get_empire(empire).unwrap().tooled_design_ids.contains(&design_id.0),
            "missing yard BOM → soft-fail; design not tooled"
        );
    }
}
