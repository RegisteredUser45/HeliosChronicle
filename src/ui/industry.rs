//! Industry inspector (STATEMENT §11) — deposits / civilian lines /
//! outputs_stock / salvage on a system, rows from live `SystemEntity` matter.

use std::collections::BTreeSet;

use eframe::egui::{self, RichText, ScrollArea};

use crate::entity::EntityId;
use crate::world::World;

/// Show one Industry window per open system id (several at once OK).
/// Reads SystemEntity matter fields by live ref — no deposit/line Vec clones.
pub fn show_industry_inspectors(
    ctx: &egui::Context,
    world: &World,
    open: &mut BTreeSet<EntityId>,
) {
    let ids: Vec<EntityId> = open.iter().copied().collect();
    let mut closed: Vec<EntityId> = Vec::new();

    for sid in ids {
        let mut win_open = true;
        let title = format!("Industry inspector — {sid}");

        egui::Window::new(title)
            .id(egui::Id::new(("helios_industry", sid.0)))
            .open(&mut win_open)
            .default_width(460.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!("breadcrumb: System {sid} → Industry")).weak(),
                );
                ui.separator();

                match world.ledger().get(sid) {
                    Some(sys) => {
                        ui.monospace(format!(
                            "binding_remainder={:.3}  depleted={}  salvage_stock={:.3}",
                            sys.binding_remainder, sys.depleted, sys.salvage_stock
                        ));
                        ui.separator();

                        // Mines = deposit veins (live rows).
                        ui.label(RichText::new("Mines / deposits").strong());
                        ui.label(format!("deposits = {}", sys.deposits.len()));
                        ScrollArea::vertical()
                            .id_source(("ind_deposits", sid.0))
                            .max_height(160.0)
                            .show(ui, |ui| {
                                if sys.deposits.is_empty() {
                                    ui.label("(no deposits)");
                                } else {
                                    for (i, d) in sys.deposits.iter().enumerate() {
                                        ui.monospace(format!(
                                            "[{i}] {stock}  qty={qty:.3}  acc={acc:.3}  extractable={ex:.3}  extractor={ext}",
                                            stock = d.stock_id,
                                            qty = d.quantity,
                                            acc = d.accessibility,
                                            ex = d.extractable(),
                                            ext = d.extractor.as_str(),
                                        ));
                                    }
                                }
                            });

                        ui.separator();
                        ui.label(RichText::new("Civilian lines").strong());
                        ui.label(format!("civilian_lines = {}", sys.civilian_lines.len()));
                        if sys.civilian_lines.is_empty() {
                            ui.label(
                                RichText::new(
                                    "(empty — tick uses globals.civilian_extract_rate if Civilian deposits exist)",
                                )
                                .weak()
                                .small(),
                            );
                        } else {
                            for (i, line) in sys.civilian_lines.iter().enumerate() {
                                ui.monospace(format!("[{i}] rate={:.6} / tick", line.rate));
                            }
                        }

                        ui.separator();
                        ui.label(RichText::new("Chain outputs (outputs_stock)").strong());
                        if sys.outputs_stock.is_empty() {
                            ui.label("(empty)");
                        } else {
                            ScrollArea::vertical()
                                .id_source(("ind_outputs", sid.0))
                                .max_height(120.0)
                                .show(ui, |ui| {
                                    for (stock, qty) in &sys.outputs_stock {
                                        ui.monospace(format!("{stock}: {qty:.6}"));
                                    }
                                });
                        }

                        ui.separator();
                        ui.label(RichText::new("Salvage").strong());
                        egui::Grid::new(("ind_salvage", sid.0))
                            .num_columns(2)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("salvage_stock");
                                ui.monospace(format!("{:.6}", sys.salvage_stock));
                                ui.end_row();
                                ui.label("salvage_by_stock entries");
                                ui.monospace(sys.salvage_by_stock.len().to_string());
                                ui.end_row();
                            });
                        if sys.salvage_by_stock.is_empty() {
                            ui.label("(no per-stock salvage)");
                        } else {
                            for (stock, qty) in &sys.salvage_by_stock {
                                ui.monospace(format!("{stock}: {qty:.6}"));
                            }
                        }

                        ui.separator();
                        ui.label(RichText::new("Binding stocks (aggregated)").strong());
                        if sys.binding_stocks.is_empty() {
                            ui.label("(empty)");
                        } else {
                            for (stock, qty) in &sys.binding_stocks {
                                ui.monospace(format!("{stock}: {qty:.6}"));
                            }
                        }
                    }
                    None => {
                        ui.label("System not found on ledger.");
                    }
                }
            });

        if !win_open {
            closed.push(sid);
        }
    }
    for id in closed {
        open.remove(&id);
    }
}
