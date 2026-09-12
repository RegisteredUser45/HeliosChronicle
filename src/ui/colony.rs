//! Colony inspector (STATEMENT §11) — pops / structure soaks / deficit bill
//! over live `BodyEntity` + related ledger/globals fields.
//!
//! No separate Colony or Structure entity yet: soaks on BodyEntity are the
//! structure ledger rows; deficit/life-support derived from worlds helpers.

use std::collections::BTreeSet;

use eframe::egui::{self, RichText};

use crate::entity::EntityId;
use crate::world::World;
use crate::worlds;

/// Show one Colony window per open body id (several at once OK).
/// Display from live BodyEntity / globals refs — no per-frame body clones.
pub fn show_colony_inspectors(
    ctx: &egui::Context,
    world: &World,
    open: &mut BTreeSet<EntityId>,
) {
    let ids: Vec<EntityId> = open.iter().copied().collect();
    let mut closed: Vec<EntityId> = Vec::new();
    let envelope = &world.globals().envelope;

    for bid in ids {
        let mut win_open = true;
        let parent_sys = world.ledger().get_body(bid).map(|b| b.system);
        let title = format!("Colony inspector — {bid}");

        egui::Window::new(title)
            .id(egui::Id::new(("helios_colony", bid.0)))
            .open(&mut win_open)
            .default_width(400.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                match parent_sys {
                    Some(sys) => {
                        ui.label(
                            RichText::new(format!(
                                "breadcrumb: System {sys} → Body {bid} → Colony"
                            ))
                            .weak(),
                        );
                    }
                    None => {
                        ui.label(
                            RichText::new(format!("breadcrumb: Body {bid} → Colony")).weak(),
                        );
                    }
                }
                ui.separator();

                match world.ledger().get_body(bid) {
                    Some(body) => {
                        let deficit = worlds::compute_deficits(body, envelope);
                        let (org_need, vol_need) =
                            worlds::life_support_bill(body, envelope, 1);
                        let soak_sum =
                            (body.structure_soak + body.power_soak + body.upkeep_soak).max(0.0);

                        egui::Grid::new(("colony_grid", bid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("body_id");
                                ui.monospace(body.id.to_string());
                                ui.end_row();
                                ui.label("system");
                                ui.monospace(body.system.to_string());
                                ui.end_row();
                                ui.label("pops");
                                ui.monospace(format!("{:.6}", body.pops));
                                ui.end_row();
                                ui.label("automation_active");
                                ui.monospace(body.automation_active.to_string());
                                ui.end_row();
                            });

                        ui.separator();
                        ui.label(RichText::new("Structures (BodyEntity soaks)").strong());
                        ui.label(
                            RichText::new(
                                "(no Structure entity yet — soaks are the structure ledger)",
                            )
                            .weak()
                            .small(),
                        );
                        egui::Grid::new(("colony_soaks", bid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("structure_soak");
                                ui.monospace(format!("{:.6}", body.structure_soak));
                                ui.end_row();
                                ui.label("power_soak");
                                ui.monospace(format!("{:.6}", body.power_soak));
                                ui.end_row();
                                ui.label("upkeep_soak");
                                ui.monospace(format!("{:.6}", body.upkeep_soak));
                                ui.end_row();
                                ui.label("soak_sum");
                                ui.monospace(format!("{:.6}", soak_sum));
                                ui.end_row();
                            });
                        ui.label(RichText::new("Inferred structure rows").strong());
                        let mut any_struct = false;
                        if body.structure_soak > 0.0 {
                            any_struct = true;
                            ui.monospace(format!(
                                "habitat/structure  soak={:.3}",
                                body.structure_soak
                            ));
                        }
                        if body.power_soak > 0.0 {
                            any_struct = true;
                            ui.monospace(format!("lab/power         soak={:.3}", body.power_soak));
                        }
                        if body.upkeep_soak > 0.0 {
                            any_struct = true;
                            ui.monospace(format!(
                                "mine/upkeep       soak={:.3}",
                                body.upkeep_soak
                            ));
                        }
                        if !any_struct {
                            ui.label("(no soaks — empty colony pad)");
                        }

                        ui.separator();
                        ui.label(RichText::new("Deficit bill (envelope vs layers)").strong());
                        egui::Grid::new(("colony_deficit", bid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("mortality");
                                ui.monospace(format!("{:.6}", deficit.mortality));
                                ui.end_row();
                                ui.label("fertility");
                                ui.monospace(format!("{:.6}", deficit.fertility));
                                ui.end_row();
                                ui.label("labor");
                                ui.monospace(format!("{:.6}", deficit.labor));
                                ui.end_row();
                            });

                        ui.separator();
                        ui.label(RichText::new("Life-support bill (dt=1)").strong());
                        egui::Grid::new(("colony_ls", bid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("organics_need");
                                ui.monospace(format!("{:.6}", org_need));
                                ui.end_row();
                                ui.label("volatiles_need");
                                ui.monospace(format!("{:.6}", vol_need));
                                ui.end_row();
                                ui.label("envelope.calories_need");
                                ui.monospace(format!("{:.6}", envelope.calories_need));
                                ui.end_row();
                                ui.label("envelope.water_need");
                                ui.monospace(format!("{:.6}", envelope.water_need));
                                ui.end_row();
                            });

                        ui.separator();
                        ui.label(RichText::new("Env layers (live)").strong());
                        egui::Grid::new(("colony_layers", bid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("atmosphere_pressure");
                                ui.monospace(format!("{:.6}", body.layers.atmosphere_pressure));
                                ui.end_row();
                                ui.label("temperature");
                                ui.monospace(format!("{:.6}", body.layers.temperature));
                                ui.end_row();
                                ui.label("radiation");
                                ui.monospace(format!("{:.6}", body.layers.radiation));
                                ui.end_row();
                                ui.label("toxins_fallout");
                                ui.monospace(format!("{:.6}", body.layers.toxins_fallout));
                                ui.end_row();
                                ui.label("biosphere");
                                ui.monospace(format!("{:.6}", body.layers.biosphere));
                                ui.end_row();
                            });

                        ui.separator();
                        ui.label(
                            RichText::new(
                                "(stockpiles / build queue: no BodyEntity fields yet)",
                            )
                            .weak()
                            .small(),
                        );
                    }
                    None => {
                        ui.label("Body not found on ledger.");
                    }
                }
            });

        if !win_open {
            closed.push(bid);
        }
    }
    for id in closed {
        open.remove(&id);
    }
}
