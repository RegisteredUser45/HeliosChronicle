//! Catalog inspector (STATEMENT §11) — tech book lines / segments roster +
//! detail. Read-only over cached `stub_tech_book` rows (same book Research uses).

use eframe::egui::{self, RichText, ScrollArea};

use crate::research::{self, TechLine, TechSegment};

/// Catalog window: lines → segments roster; detail via segment id.
/// Prefers cached book slices (no per-frame `stub_tech_book` rebuild).
/// Returns whether Research should open when the operator clicks through.
pub fn show_catalog_inspector(
    ctx: &egui::Context,
    open: &mut bool,
    lines: &[TechLine],
    segments: &[TechSegment],
    selected_segment: &mut Option<String>,
) -> CatalogAction {
    if !*open {
        return CatalogAction::None;
    }
    let mut win_open = true;
    let mut action = CatalogAction::None;
    let mut pick: Option<String> = None;

    egui::Window::new("Catalog inspector")
        .id(egui::Id::new("helios_catalog_inspector"))
        .open(&mut win_open)
        .default_width(520.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("breadcrumb: Catalog").weak());
            ui.label(format!(
                "tech lines = {}   segments = {}   (stub_tech_book)",
                lines.len(),
                segments.len()
            ));
            ui.horizontal(|ui| {
                if ui.button("Open Research…").clicked() {
                    action = CatalogAction::OpenResearch;
                }
            });
            ui.separator();

            ui.label(RichText::new("Tech lines / segments (roster)").strong());
            ScrollArea::vertical()
                .id_source("catalog_line_roster")
                .max_height(220.0)
                .show(ui, |ui| {
                    if lines.is_empty() {
                        ui.label("(empty tech book)");
                    } else {
                        for line in lines {
                            ui.label(
                                RichText::new(format!(
                                    "{} — {} ({:?})",
                                    line.id, line.name, line.category
                                ))
                                .strong(),
                            );
                            for sid in &line.segment_ids {
                                match segments.iter().find(|s| s.id == *sid) {
                                    Some(seg) => {
                                        ui.horizontal(|ui| {
                                            ui.monospace(format!(
                                                "  [{}] {}  {}",
                                                seg.index, seg.id, seg.display_name
                                            ));
                                            if ui.small_button("Detail").clicked() {
                                                pick = Some(seg.id.clone());
                                            }
                                        });
                                    }
                                    None => {
                                        ui.horizontal(|ui| {
                                            ui.monospace(format!("  {sid}  (missing row)"));
                                            if ui.small_button("find_segment").clicked() {
                                                pick = Some(sid.clone());
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                });

            ui.separator();
            ui.label(RichText::new("Segment detail").strong());
            match selected_segment.as_deref() {
                Some(sid) => {
                    ui.label(
                        RichText::new(format!("breadcrumb: Catalog → {sid}")).weak(),
                    );
                    // Prefer cached book; fall back to research::find_segment once.
                    let cached = segments.iter().find(|s| s.id == sid);
                    let owned;
                    let seg: Option<&TechSegment> = match cached {
                        Some(s) => Some(s),
                        None => {
                            owned = research::find_segment(sid);
                            owned.as_ref()
                        }
                    };
                    match seg {
                        Some(seg) => {
                            egui::Grid::new(("catalog_seg", sid))
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("id");
                                    ui.monospace(seg.id.as_str());
                                    ui.end_row();
                                    ui.label("line_id");
                                    ui.monospace(seg.line_id.as_str());
                                    ui.end_row();
                                    ui.label("index");
                                    ui.monospace(seg.index.to_string());
                                    ui.end_row();
                                    ui.label("display_name");
                                    ui.monospace(seg.display_name.as_str());
                                    ui.end_row();
                                    ui.label("salvage_skip_allowed");
                                    ui.monospace(seg.salvage_skip_allowed.to_string());
                                    ui.end_row();
                                });
                            ui.separator();
                            ui.label(RichText::new("material_gates").strong());
                            if seg.material_gates.is_empty() {
                                ui.label("(none)");
                            } else {
                                for g in &seg.material_gates {
                                    ui.monospace(format!("  {g}"));
                                }
                            }
                            ui.label(RichText::new("unlocks").strong());
                            if seg.unlocks.is_empty() {
                                ui.label("(none)");
                            } else {
                                for u in &seg.unlocks {
                                    ui.monospace(format!("  {u}"));
                                }
                            }
                        }
                        None => {
                            ui.label(format!("segment `{sid}` not found (find_segment miss)"));
                        }
                    }
                }
                None => {
                    ui.label("(select a segment from the roster)");
                }
            }
        });

    if let Some(sid) = pick {
        *selected_segment = Some(sid);
    }
    if !win_open {
        *open = false;
    }
    action
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogAction {
    None,
    OpenResearch,
}
