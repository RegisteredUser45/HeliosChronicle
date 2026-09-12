//! Contact inspector (STATEMENT §11) — treaties / contracts / standing / fog
//! summary from live `world.contact` + `world.standing` for a selected empire.

use eframe::egui::{self, RichText, ScrollArea};

use crate::entity::{EmpireId, EntityId};
use crate::world::World;

use super::fog;

/// Contact window for selected empire (viewpoint or picker). Read-only.
pub fn show_contact_inspector(
    ctx: &egui::Context,
    world: &World,
    open: &mut bool,
    contact_empire: &mut Option<EntityId>,
) {
    if !*open {
        return;
    }
    let mut win_open = true;
    let mut cycle = false;

    let empire_ids: Vec<EntityId> = world.ledger().empires().map(|(id, _)| *id).collect();
    if contact_empire.is_none() {
        *contact_empire = empire_ids.first().copied();
    } else if let Some(eid) = *contact_empire {
        if world.ledger().get_empire(eid).is_none() {
            *contact_empire = empire_ids.first().copied();
        }
    }

    egui::Window::new("Contact inspector")
        .id(egui::Id::new("helios_contact_inspector"))
        .open(&mut win_open)
        .default_width(520.0)
        .default_height(500.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("breadcrumb: Contact").weak());
            ui.horizontal(|ui| {
                let label = match *contact_empire {
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
            });
            ui.separator();

            match *contact_empire {
                Some(eid) => {
                    let empire_id = fog::as_empire_id(eid);
                    let contact = world.contact.get(empire_id);
                    let emp_name = world
                        .ledger()
                        .get_empire(eid)
                        .and_then(|e| e.name.clone())
                        .unwrap_or_else(|| eid.to_string());

                    ui.label(
                        RichText::new(format!("breadcrumb: Contact → {emp_name}")).weak(),
                    );

                    // Fog summary (live FogState ref — no clone).
                    ui.label(RichText::new("Fog summary (world.contact.fog)").strong());
                    match contact {
                        Some(c) => {
                            egui::Grid::new(("contact_fog", eid.0))
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("known_systems");
                                    ui.monospace(c.fog.known_systems.len().to_string());
                                    ui.end_row();
                                    ui.label("known_fleets");
                                    ui.monospace(c.fog.known_fleets.len().to_string());
                                    ui.end_row();
                                    ui.label("treaties");
                                    ui.monospace(c.treaties.len().to_string());
                                    ui.end_row();
                                    ui.label("contracts (all)");
                                    ui.monospace(c.contracts.len().to_string());
                                    ui.end_row();
                                    ui.label("contracts (active)");
                                    ui.monospace(c.active_contracts().count().to_string());
                                    ui.end_row();
                                });

                            ui.separator();
                            ui.label(RichText::new("Known systems (sample)").strong());
                            ScrollArea::vertical()
                                .id_source(("contact_known_sys", eid.0))
                                .max_height(100.0)
                                .show(ui, |ui| {
                                    if c.fog.known_systems.is_empty() {
                                        ui.label("(none)");
                                    } else {
                                        for (sid, entry) in c.fog.known_systems.iter().take(32) {
                                            ui.monospace(format!(
                                                "sys {sid}  tick={}  unc={:.3}  surveyed={}",
                                                entry.last_known_tick,
                                                entry.uncertainty,
                                                entry.surveyed_fuse
                                            ));
                                        }
                                        if c.fog.known_systems.len() > 32 {
                                            ui.label(format!(
                                                "… +{} more",
                                                c.fog.known_systems.len() - 32
                                            ));
                                        }
                                    }
                                });

                            ui.separator();
                            ui.label(RichText::new("Known fleets (sample)").strong());
                            ScrollArea::vertical()
                                .id_source(("contact_known_fleet", eid.0))
                                .max_height(80.0)
                                .show(ui, |ui| {
                                    if c.fog.known_fleets.is_empty() {
                                        ui.label("(none)");
                                    } else {
                                        for (fid, entry) in c.fog.known_fleets.iter().take(24) {
                                            ui.monospace(format!(
                                                "fleet {fid}  tick={}  unc={:.3}  sys={:?}",
                                                entry.last_known_tick,
                                                entry.uncertainty,
                                                entry.last_system
                                            ));
                                        }
                                        if c.fog.known_fleets.len() > 24 {
                                            ui.label(format!(
                                                "… +{} more",
                                                c.fog.known_fleets.len() - 24
                                            ));
                                        }
                                    }
                                });

                            ui.separator();
                            ui.label(RichText::new("Treaties").strong());
                            if c.treaties.is_empty() {
                                ui.label("(none)");
                            } else {
                                ScrollArea::vertical()
                                    .id_source(("contact_treaties", eid.0))
                                    .max_height(120.0)
                                    .show(ui, |ui| {
                                        for t in &c.treaties {
                                            ui.monospace(format!(
                                                "treaty {}  a={} b={}  start={} end={:?}  clauses={:?}",
                                                t.id, t.a.0, t.b.0, t.start_tick, t.end_tick, t.clauses
                                            ));
                                        }
                                    });
                            }

                            ui.separator();
                            ui.label(RichText::new("Contracts").strong());
                            if c.contracts.is_empty() {
                                ui.label("(none)");
                            } else {
                                ScrollArea::vertical()
                                    .id_source(("contact_contracts", eid.0))
                                    .max_height(120.0)
                                    .show(ui, |ui| {
                                        for con in &c.contracts {
                                            ui.monospace(format!(
                                                "contract {}  {:?}  a={} b={}  start={} end={:?}  defaulted={}",
                                                con.id,
                                                con.kind,
                                                con.a.0,
                                                con.b.0,
                                                con.start_tick,
                                                con.end_tick,
                                                con.defaulted
                                            ));
                                        }
                                    });
                            }
                        }
                        None => {
                            ui.label(
                                RichText::new(
                                    "(no EmpireContact row yet — contact.ensure on first_contact)",
                                )
                                .weak(),
                            );
                        }
                    }

                    ui.separator();
                    ui.label(RichText::new("Standing (world.standing, from this empire)").strong());
                    // Directed standing: how this empire feels about others (eid → *).
                    let from = EmpireId(eid.0);
                    let mut rows: Vec<(EmpireId, i32)> = world
                        .standing
                        .table
                        .iter()
                        .filter_map(|((a, b), v)| {
                            if *a == from {
                                Some((*b, *v))
                            } else {
                                None
                            }
                        })
                        .collect();
                    // Also show inbound (others → eid) briefly.
                    let mut inbound: Vec<(EmpireId, i32)> = world
                        .standing
                        .table
                        .iter()
                        .filter_map(|((a, b), v)| {
                            if *b == from {
                                Some((*a, *v))
                            } else {
                                None
                            }
                        })
                        .collect();
                    rows.sort_by_key(|(id, _)| id.0);
                    inbound.sort_by_key(|(id, _)| id.0);

                    ui.label(format!("outbound ({} → *) = {}", eid, rows.len()));
                    if rows.is_empty() {
                        ui.label("(no outbound standing rows)");
                    } else {
                        for (other, v) in rows.iter().take(48) {
                            let name = world
                                .ledger()
                                .get_empire(EntityId(other.0))
                                .and_then(|e| e.name.clone())
                                .unwrap_or_else(|| format!("empire {}", other.0));
                            ui.monospace(format!("  → {name} ({}): {v}", other.0));
                        }
                    }
                    ui.label(format!("inbound (* → {}) = {}", eid, inbound.len()));
                    if inbound.is_empty() {
                        ui.label("(no inbound standing rows)");
                    } else {
                        for (other, v) in inbound.iter().take(48) {
                            let name = world
                                .ledger()
                                .get_empire(EntityId(other.0))
                                .and_then(|e| e.name.clone())
                                .unwrap_or_else(|| format!("empire {}", other.0));
                            ui.monospace(format!("  ← {name} ({}): {v}", other.0));
                        }
                    }

                    // Cross-check ledger peers with get() for zero-default visibility.
                    ui.separator();
                    ui.label(RichText::new("Standing vs ledger empires (get default 0)").strong());
                    for oid in &empire_ids {
                        if *oid == eid {
                            continue;
                        }
                        let out = world.standing.get(from, EmpireId(oid.0));
                        let inn = world.standing.get(EmpireId(oid.0), from);
                        ui.monospace(format!(
                            "vs {oid}: out={out}  in={inn}"
                        ));
                    }
                }
                None => {
                    ui.label("(no empires on ledger)");
                }
            }
        });

    if cycle {
        if let Some(cur) = *contact_empire {
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
            *contact_empire = next.or(first);
        } else {
            *contact_empire = empire_ids.first().copied();
        }
    }
    if !win_open {
        *open = false;
    }
}
