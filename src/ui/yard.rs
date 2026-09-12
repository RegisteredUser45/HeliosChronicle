//! Yard / fleet-yard inspector (STATEMENT §11) — designs tooled, yard unlock,
//! ship build hooks as readable state from live World (empire tooled_design_ids,
//! ship_designs + ships roster). No Yard entity type yet.

use eframe::egui::{self, RichText, ScrollArea};

use crate::entity::EntityId;
use crate::research::empire_has_unlock;
use crate::world::World;

/// Yard window over one empire (viewpoint or picker). Returns open-Design request.
pub fn show_yard_inspector(
    ctx: &egui::Context,
    world: &World,
    open: &mut bool,
    yard_empire: &mut Option<EntityId>,
    selected_design: &mut Option<EntityId>,
) -> YardAction {
    if !*open {
        return YardAction::None;
    }
    let mut win_open = true;
    let mut action = YardAction::None;
    let mut pick_design: Option<EntityId> = None;
    let mut cycle = false;

    // Snapshot empire ids for picker (avoid borrow fights).
    let empire_ids: Vec<EntityId> = world.ledger().empires().map(|(id, _)| *id).collect();
    if yard_empire.is_none() {
        *yard_empire = empire_ids.first().copied();
    } else if let Some(eid) = *yard_empire {
        if world.ledger().get_empire(eid).is_none() {
            *yard_empire = empire_ids.first().copied();
        }
    }

    let design_ids: Vec<EntityId> = world.ship_designs.keys().copied().collect();
    let ship_ids: Vec<EntityId> = world.ships.keys().copied().collect();

    egui::Window::new("Yard / fleet yard inspector")
        .id(egui::Id::new("helios_yard_inspector"))
        .open(&mut win_open)
        .default_width(520.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("breadcrumb: System/Design → Yard").weak());
            ui.horizontal(|ui| {
                let label = match *yard_empire {
                    Some(eid) => {
                        let name = world
                            .ledger()
                            .get_empire(eid)
                            .and_then(|e| e.name.clone())
                            .unwrap_or_else(|| eid.to_string());
                        format!("empire: {name} ({eid})")
                    }
                    None => "empire: (none)".into(),
                };
                ui.label(RichText::new(label).strong());
                if ui.small_button("Cycle empire").clicked() {
                    cycle = true;
                }
                if ui.button("Open Designs…").clicked() {
                    action = YardAction::OpenDesigns;
                }
            });
            ui.separator();

            match *yard_empire {
                Some(eid) => match world.ledger().get_empire(eid) {
                    Some(emp) => {
                        let has_yard = empire_has_unlock(emp, "facility.yard");
                        egui::Grid::new(("yard_emp", eid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("empire_id");
                                ui.monospace(eid.to_string());
                                ui.end_row();
                                ui.label("facility.yard unlocked");
                                ui.monospace(has_yard.to_string());
                                ui.end_row();
                                ui.label("tooled_design_ids");
                                ui.monospace(emp.tooled_design_ids.len().to_string());
                                ui.end_row();
                                ui.label("world.ship_designs");
                                ui.monospace(design_ids.len().to_string());
                                ui.end_row();
                                ui.label("world.ships");
                                ui.monospace(ship_ids.len().to_string());
                                ui.end_row();
                            });

                        ui.separator();
                        ui.label(RichText::new("Tooled designs (empire.tooled_design_ids)").strong());
                        if emp.tooled_design_ids.is_empty() {
                            ui.label("(none — tool via hulls::tool_yard / commission)");
                        } else {
                            ScrollArea::vertical()
                                .id_source(("yard_tooled", eid.0))
                                .max_height(100.0)
                                .show(ui, |ui| {
                                    for did_u in &emp.tooled_design_ids {
                                        let did = EntityId(*did_u);
                                        let name = world
                                            .ship_designs
                                            .get(&did)
                                            .map(|d| d.name.as_str())
                                            .unwrap_or("(missing design)");
                                        ui.horizontal(|ui| {
                                            ui.monospace(format!("design {did}  {name}"));
                                            if ui.small_button("Detail").clicked() {
                                                pick_design = Some(did);
                                            }
                                        });
                                    }
                                });
                        }

                        ui.separator();
                        ui.label(RichText::new("Design roster (ship_designs + tooling)").strong());
                        ScrollArea::vertical()
                            .id_source(("yard_designs", eid.0))
                            .max_height(140.0)
                            .show(ui, |ui| {
                                if design_ids.is_empty() {
                                    ui.label("(no designs in world.ship_designs)");
                                } else {
                                    for did in &design_ids {
                                        let Some(d) = world.ship_designs.get(did) else {
                                            continue;
                                        };
                                        let tooled = emp.tooled_design_ids.contains(&did.0);
                                        let ships_of = world
                                            .ships
                                            .values()
                                            .filter(|s| s.design_id == *did)
                                            .count();
                                        ui.horizontal(|ui| {
                                            ui.monospace(format!(
                                                "design {did}  {}  tooled={tooled}  ships={ships_of}  fuel={}  mods={}",
                                                d.name,
                                                d.fuel_tier,
                                                d.modules.len()
                                            ));
                                            if ui.small_button("Detail").clicked() {
                                                pick_design = Some(*did);
                                            }
                                        });
                                    }
                                }
                            });

                        ui.separator();
                        ui.label(RichText::new("Selected design / build hooks").strong());
                        match *selected_design {
                            Some(did) => match world.ship_designs.get(&did) {
                                Some(d) => {
                                    ui.label(
                                        RichText::new(format!(
                                            "breadcrumb: Yard → Design {did}"
                                        ))
                                        .weak(),
                                    );
                                    let tooled = emp.tooled_design_ids.contains(&did.0);
                                    egui::Grid::new(("yard_design_detail", did.0))
                                        .num_columns(2)
                                        .striped(true)
                                        .show(ui, |ui| {
                                            ui.label("id");
                                            ui.monospace(d.id.to_string());
                                            ui.end_row();
                                            ui.label("name");
                                            ui.monospace(d.name.as_str());
                                            ui.end_row();
                                            ui.label("tooled_for_empire");
                                            ui.monospace(tooled.to_string());
                                            ui.end_row();
                                            ui.label("fuel_tier");
                                            ui.monospace(d.fuel_tier.as_str());
                                            ui.end_row();
                                            ui.label("crew_req");
                                            ui.monospace(format!("{:.6}", d.crew_req));
                                            ui.end_row();
                                            ui.label("mass");
                                            ui.monospace(format!("{:.6}", d.mass));
                                            ui.end_row();
                                            ui.label("build hook");
                                            ui.monospace(
                                                "commission_design_at_system / yard_build_ship",
                                            );
                                            ui.end_row();
                                            ui.label("tool hook");
                                            ui.monospace(
                                                "tool_yard / tool_yard_at_system (recipe.yard_mk1)",
                                            );
                                            ui.end_row();
                                            ui.label("plate recipe");
                                            ui.monospace("recipe.hull_plate");
                                            ui.end_row();
                                        });
                                    ui.label(RichText::new("modules").strong());
                                    if d.modules.is_empty() {
                                        ui.label("(none)");
                                    } else {
                                        for m in &d.modules {
                                            ui.monospace(format!("  {m}"));
                                        }
                                    }
                                    ui.separator();
                                    ui.label(
                                        RichText::new("Ships of this design (world.ships)")
                                            .strong(),
                                    );
                                    let mut any = false;
                                    for (sid, ship) in &world.ships {
                                        if ship.design_id != did {
                                            continue;
                                        }
                                        any = true;
                                        ui.monospace(format!(
                                            "ship {sid}  fuel={:.3}  dmg={:.3}  crew={:.3}  cargo={:.3}",
                                            ship.fuel_qty, ship.damage, ship.crew, ship.cargo_qty
                                        ));
                                    }
                                    if !any {
                                        ui.label("(no ships built from this design)");
                                    }
                                }
                                None => {
                                    ui.label("Design not found in world.ship_designs.");
                                }
                            },
                            None => {
                                ui.label("(select a design from the roster)");
                            }
                        }
                    }
                    None => {
                        ui.label("Empire not found on ledger.");
                    }
                },
                None => {
                    ui.label("(no empires on ledger)");
                }
            }
        });

    if cycle {
        if let Some(cur) = *yard_empire {
            let mut first = None;
            let mut take_next = false;
            let mut next = None;
            for id in &empire_ids {
                if first.is_none() {
                    first = Some(*id);
                }
                if take_next {
                    next = Some(*id);
                    break;
                }
                if *id == cur {
                    take_next = true;
                }
            }
            *yard_empire = next.or(first);
        } else {
            *yard_empire = empire_ids.first().copied();
        }
    }
    if let Some(did) = pick_design {
        *selected_design = Some(did);
    }
    if !win_open {
        *open = false;
    }
    action
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YardAction {
    None,
    OpenDesigns,
}
