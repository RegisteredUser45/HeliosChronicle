//! eframe application: shell chrome around the live [`World`].

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::entity::EntityId;
use crate::event::EventKind;
use crate::hulls::{self};
use crate::research::{self, TechLine, TechSegment};
use crate::sky;
use crate::world::World;

use super::map::{self, MapCamera};

const EVENT_STRIP_MAX: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewpoint {
    Operator,
    Empire,
}

/// Operator shell state — one client of the same ledger.
pub struct HeliosApp {
    pub world: World,
    pub paused: bool,
    pub increment: u64,
    pub camera: MapCamera,
    pub selected: Option<EntityId>,
    pub viewpoint: Viewpoint,
    /// Wall-clock accumulator for unpaused ticking (seconds).
    tick_accum: f32,
    inspector_open: bool,
    /// Body inspector windows currently open (several at once OK).
    open_bodies: BTreeSet<EntityId>,
    /// Ship / fleet-detail inspector windows currently open.
    open_ships: BTreeSet<EntityId>,
    /// Fleet list window (ShipInstance roster; no TaskGroup type yet).
    fleet_open: bool,
    /// Research inspector (labs / tech lines / unlocks).
    research_open: bool,
    /// Selected lab in Research inspector detail pane.
    research_lab: Option<EntityId>,
    /// Design inspector (world.ship_designs roster + detail).
    designs_open: bool,
    /// Selected design in Design inspector detail pane.
    selected_design: Option<EntityId>,
    /// Cached stub tech book — avoid per-frame TechLine/TechSegment rebuild clones.
    tech_lines: Vec<TechLine>,
    tech_segments: Vec<TechSegment>,
}

impl HeliosApp {
    pub fn new(seed: u64) -> Self {
        let mut world = World::new(seed);
        map::ensure_map_layout(&mut world);
        // Fit camera to systems
        let mut cam = MapCamera::default();
        let pts: Vec<_> = world
            .ledger()
            .systems()
            .map(|(_, s)| (s.x as f32, s.y as f32))
            .collect();
        if let Some((min_x, max_x, min_y, max_y)) = pts.iter().fold(
            None::<(f32, f32, f32, f32)>,
            |acc, &(x, y)| {
                Some(match acc {
                    None => (x, x, y, y),
                    Some((a, b, c, d)) => (a.min(x), b.max(x), c.min(y), d.max(y)),
                })
            },
        ) {
            cam.center = egui::Pos2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            let span = ((max_x - min_x).max(max_y - min_y)).max(80.0_f32);
            cam.zoom = (420.0_f32 / span).clamp(0.2, 4.0);
        }
        let (tech_lines, tech_segments) = research::stub_tech_book();
        Self {
            world,
            paused: true,
            increment: 1,
            camera: cam,
            selected: None,
            viewpoint: Viewpoint::Operator,
            tick_accum: 0.0,
            inspector_open: false,
            open_bodies: BTreeSet::new(),
            open_ships: BTreeSet::new(),
            fleet_open: false,
            research_open: false,
            research_lab: None,
            designs_open: false,
            selected_design: None,
            tech_lines,
            tech_segments,
        }
    }

    fn step(&mut self) {
        self.world.tick(self.increment.max(1));
    }

    /// Issue 11 lean moment: kind discriminant name.
    fn kind_name(kind: &EventKind) -> String {
        let dbg = format!("{kind:?}");
        dbg.split_once(' ')
            .map(|(head, _)| head.to_string())
            .unwrap_or(dbg)
            .chars()
            .take(40)
            .collect()
    }

    /// Optional system id for chronicle row (unknown ok).
    fn kind_system(kind: &EventKind) -> Option<EntityId> {
        match kind {
            EventKind::Deplete { system }
            | EventKind::FuseArmed { system, .. }
            | EventKind::FuseTick { system, .. }
            | EventKind::FuseEnd { system }
            | EventKind::Spawn { system }
            | EventKind::HomeFlagSet { system }
            | EventKind::HomeFlagClear { system }
            | EventKind::CapitalRescore { system, .. }
            | EventKind::OrbitalStrike { system, .. }
            | EventKind::BombardmentLayerWrite { system, .. }
            | EventKind::SurfaceCombat { system, .. }
            | EventKind::GlassAttempt { system, .. }
            | EventKind::Salt { system, .. } => Some(*system),
            EventKind::OperatorMutation { entity, .. } => Some(*entity),
            _ => None,
        }
    }

    fn kind_one_line(kind: &EventKind) -> String {
        let s = format!("{kind:?}");
        s.chars().take(72).collect()
    }

    fn show_system_inspector(&mut self, ctx: &egui::Context) {
        if !self.inspector_open {
            return;
        }
        let mut open = true;
        let title = match self.selected {
            Some(id) => format!("System inspector — {id}"),
            None => "System inspector".into(),
        };
        let mut open_body: Option<EntityId> = None;
        let mut open_ship: Option<EntityId> = None;
        let mut open_fleet = false;

        egui::Window::new(title)
            .id(egui::Id::new("helios_system_inspector"))
            .open(&mut open)
            .default_width(340.0)
            .show(ctx, |ui| {
                if let Some(id) = self.selected {
                    if let Some(sys) = self.world.ledger().get(id) {
                        ui.label(RichText::new("breadcrumb: System").weak());
                        ui.separator();
                        ui.monospace(format!("id:              {id}"));
                        ui.monospace(format!("x/y:             {:.2} / {:.2}", sys.x, sys.y));
                        ui.monospace(format!(
                            "binding_remainder:{:.3}",
                            sys.binding_remainder
                        ));
                        ui.monospace(format!("depleted:        {}", sys.depleted));
                        ui.monospace(format!("fuse_end_tick:   {:?}", sys.fuse_end_tick));
                        ui.monospace(format!("fuse_remaining:  {:?}", sys.fuse_remaining));
                        let state = sky::map_state(sys);
                        ui.monospace(format!("map_state:       {}", state.as_str()));
                        ui.monospace(format!(
                            "wilderness:      {}  surveyed: {}  claimed: {}",
                            sys.wilderness, sys.surveyed, sys.claimed
                        ));
                        ui.monospace(format!(
                            "home:            capital={} flag={} paused={}",
                            sys.is_home_capital, sys.home_flag, sys.fuse_paused
                        ));
                        ui.monospace(format!("ended:           {}", sys.ended));
                        ui.monospace(format!("jump_links:      {}", sys.jump_links.len()));
                        ui.monospace(format!("lod_hint:        {:?}", sys.lod_hint));
                    } else {
                        ui.label("System not found on ledger.");
                    }

                    ui.separator();
                    ui.label(RichText::new("Bodies in system (System → Body)").strong());
                    let bodies: Vec<(EntityId, f64, bool)> = self
                        .world
                        .ledger()
                        .bodies_for_system(id)
                        .map(|(bid, b)| (*bid, b.pops, b.automation_active))
                        .collect();
                    if bodies.is_empty() {
                        ui.label("(no bodies)");
                    } else {
                        for (bid, pops, auto) in bodies {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("body {bid}  pops={pops:.0} auto={auto}"));
                                if ui.button("Open Body").clicked() {
                                    open_body = Some(bid);
                                }
                            });
                        }
                    }

                    ui.separator();
                    ui.label(RichText::new("Fleet (System → Ship)").strong());
                    ui.label(
                        RichText::new(
                            "ShipInstance has no system field yet — listing all world.ships",
                        )
                        .weak()
                        .small(),
                    );
                    let ships: Vec<EntityId> = self.world.ships.keys().copied().collect();
                    if ships.is_empty() {
                        ui.label("(no ships in world.ships)");
                    } else {
                        for sid in ships.iter().take(24) {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("ship {sid}"));
                                if ui.button("Open Ship").clicked() {
                                    open_ship = Some(*sid);
                                }
                            });
                        }
                        if ships.len() > 24 {
                            ui.label(format!("… +{} more (see Fleet list)", ships.len() - 24));
                        }
                    }
                    if ui.button("Open Fleet list…").clicked() {
                        open_fleet = true;
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Research…").clicked() {
                            self.research_open = true;
                        }
                        if ui.button("Designs…").clicked() {
                            self.designs_open = true;
                        }
                    });
                } else {
                    ui.label("Select a system on the map.");
                }
            });

        if let Some(bid) = open_body {
            self.open_bodies.insert(bid);
        }
        if let Some(sid) = open_ship {
            self.open_ships.insert(sid);
        }
        if open_fleet {
            self.fleet_open = true;
        }
        if !open {
            self.inspector_open = false;
        }
    }

    fn show_body_inspectors(&mut self, ctx: &egui::Context) {
        let ids: Vec<EntityId> = self.open_bodies.iter().copied().collect();
        let mut closed: Vec<EntityId> = Vec::new();
        for bid in ids {
            let mut open = true;
            let parent_sys = self
                .world
                .ledger()
                .get_body(bid)
                .map(|b| b.system);
            let title = format!("Body inspector — {bid}");
            egui::Window::new(title)
                .id(egui::Id::new(("helios_body", bid.0)))
                .open(&mut open)
                .default_width(360.0)
                .show(ctx, |ui| {
                    if let Some(sys) = parent_sys {
                        ui.label(
                            RichText::new(format!("breadcrumb: System {sys} → Body {bid}")).weak(),
                        );
                    } else {
                        ui.label(RichText::new(format!("breadcrumb: Body {bid}")).weak());
                    }
                    ui.separator();
                    match self.world.ledger().get_body(bid) {
                        Some(body) => {
                            // Tabular rows from BodyEntity fields on the live ledger.
                            egui::Grid::new(("body_grid", bid.0))
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("id");
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
                                    ui.label("structure_soak");
                                    ui.monospace(format!("{:.6}", body.structure_soak));
                                    ui.end_row();
                                    ui.label("power_soak");
                                    ui.monospace(format!("{:.6}", body.power_soak));
                                    ui.end_row();
                                    ui.label("upkeep_soak");
                                    ui.monospace(format!("{:.6}", body.upkeep_soak));
                                    ui.end_row();
                                    ui.label("layers.atmosphere_pressure");
                                    ui.monospace(format!(
                                        "{:.6}",
                                        body.layers.atmosphere_pressure
                                    ));
                                    ui.end_row();
                                    ui.label("layers.temperature");
                                    ui.monospace(format!("{:.6}", body.layers.temperature));
                                    ui.end_row();
                                    ui.label("layers.radiation");
                                    ui.monospace(format!("{:.6}", body.layers.radiation));
                                    ui.end_row();
                                    ui.label("layers.toxins_fallout");
                                    ui.monospace(format!("{:.6}", body.layers.toxins_fallout));
                                    ui.end_row();
                                    ui.label("layers.biosphere");
                                    ui.monospace(format!("{:.6}", body.layers.biosphere));
                                    ui.end_row();
                                });
                            ui.separator();
                            ui.label(
                                RichText::new(
                                    "(colonies/outposts: no separate types on BodyEntity yet)",
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
            if !open {
                closed.push(bid);
            }
        }
        for id in closed {
            self.open_bodies.remove(&id);
        }
    }

    fn show_fleet_list(&mut self, ctx: &egui::Context) {
        if !self.fleet_open {
            return;
        }
        let mut open = true;
        let mut open_ship: Option<EntityId> = None;
        let ships: Vec<(EntityId, EntityId, f64, f64, f64)> = self
            .world
            .ships
            .iter()
            .map(|(id, s)| (*id, s.design_id, s.fuel_qty, s.damage, s.cargo_qty))
            .collect();

        egui::Window::new("Fleet inspector — roster")
            .id(egui::Id::new("helios_fleet_list"))
            .open(&mut open)
            .default_width(420.0)
            .default_height(280.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("breadcrumb: System → Ship (fleet = list of ShipInstances)")
                        .weak(),
                );
                ui.label(format!(
                    "world.ships = {}   ship_designs = {}",
                    ships.len(),
                    self.world.ship_designs.len()
                ));
                if let Some(sys) = self.selected {
                    ui.label(
                        RichText::new(format!(
                            "selected system {sys}: no ShipInstance.system field to filter"
                        ))
                        .weak()
                        .small(),
                    );
                }
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    if ships.is_empty() {
                        ui.label("(no ships — build via hulls / operator)");
                    } else {
                        for (sid, did, fuel, dmg, cargo) in &ships {
                            ui.horizontal(|ui| {
                                ui.monospace(format!(
                                    "ship {sid}  design={did}  fuel={fuel:.2}  dmg={dmg:.3}  cargo={cargo:.2}"
                                ));
                                if ui.button("Inspect").clicked() {
                                    open_ship = Some(*sid);
                                }
                            });
                        }
                    }
                });
            });

        if let Some(sid) = open_ship {
            self.open_ships.insert(sid);
        }
        if !open {
            self.fleet_open = false;
        }
    }

    fn show_ship_inspectors(&mut self, ctx: &egui::Context) {
        let ids: Vec<EntityId> = self.open_ships.iter().copied().collect();
        let mut closed: Vec<EntityId> = Vec::new();
        let mut open_design: Option<EntityId> = None;
        for sid in ids {
            let mut open = true;
            let title = format!("Ship inspector — {sid}");
            // Display from live refs / scalars — no per-frame ShipInstance/ShipDesign clones.
            let world = &self.world;
            egui::Window::new(title)
                .id(egui::Id::new(("helios_ship", sid.0)))
                .open(&mut open)
                .default_width(380.0)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(format!("breadcrumb: System → Ship {sid}")).weak(),
                    );
                    ui.separator();
                    match world.ships.get(&sid) {
                        Some(s) => {
                            let design = world.ship_designs.get(&s.design_id);
                            let cap = design.map(hulls::cargo_capacity).unwrap_or(0.0);
                            let can_mv = design.map(|d| hulls::can_move(d, s)).unwrap_or(false);
                            egui::Grid::new(("ship_grid", sid.0))
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("id");
                                    ui.monospace(s.id.to_string());
                                    ui.end_row();
                                    ui.label("system location");
                                    ui.monospace(
                                        "(none on ShipInstance — location not tracked yet)",
                                    );
                                    ui.end_row();
                                    ui.label("design_id");
                                    ui.monospace(s.design_id.to_string());
                                    ui.end_row();
                                    ui.label("design name");
                                    ui.monospace(
                                        design.map(|d| d.name.as_str()).unwrap_or("(missing)"),
                                    );
                                    ui.end_row();
                                    ui.label("fuel_tier");
                                    ui.monospace(s.fuel_tier.as_str());
                                    ui.end_row();
                                    ui.label("fuel_qty");
                                    ui.monospace(format!("{:.6}", s.fuel_qty));
                                    ui.end_row();
                                    ui.label("damage");
                                    ui.monospace(format!("{:.6}", s.damage));
                                    ui.end_row();
                                    ui.label("crew");
                                    ui.monospace(format!("{:.6}", s.crew));
                                    ui.end_row();
                                    ui.label("maintenance");
                                    ui.monospace(format!("{:.6}", s.maintenance));
                                    ui.end_row();
                                    ui.label("cargo_qty");
                                    ui.monospace(format!("{:.6}", s.cargo_qty));
                                    ui.end_row();
                                    ui.label("cargo_capacity");
                                    ui.monospace(format!("{cap:.6}"));
                                    ui.end_row();
                                    ui.label("can_move");
                                    ui.monospace(can_mv.to_string());
                                    ui.end_row();
                                    ui.label("planetary_strike");
                                    ui.monospace(s.planetary_strike.to_string());
                                    ui.end_row();
                                    ui.label("sidearm_caliber");
                                    ui.monospace(format!("{:.6}", s.sidearm_caliber));
                                    ui.end_row();
                                });
                            ui.separator();
                            ui.label(RichText::new("magazines").strong());
                            if s.magazines.is_empty() {
                                ui.label("(empty)");
                            } else {
                                for (ammo, qty) in &s.magazines {
                                    ui.monospace(format!("{ammo}: {qty:.3}"));
                                }
                            }
                            if let Some(d) = design {
                                ui.separator();
                                ui.label(RichText::new("design modules").strong());
                                ui.monospace(format!(
                                    "crew_req={:.1} mass={:.1} fuel_tier={}",
                                    d.crew_req, d.mass, d.fuel_tier
                                ));
                                for m in &d.modules {
                                    ui.monospace(format!("  {m}"));
                                }
                                if ui.button("Open Design inspector…").clicked() {
                                    open_design = Some(s.design_id);
                                }
                            }
                        }
                        None => {
                            ui.label("Ship not found in world.ships.");
                        }
                    }
                });
            if !open {
                closed.push(sid);
            }
        }
        for id in closed {
            self.open_ships.remove(&id);
        }
        if let Some(did) = open_design {
            self.selected_design = Some(did);
            self.designs_open = true;
        }
    }

    fn show_research_inspector(&mut self, ctx: &egui::Context) {
        if !self.research_open {
            return;
        }
        let mut open = true;
        let mut pick_lab: Option<EntityId> = None;
        let mut open_designs = false;
        let rp_cost = research::segment_rp_cost(self.world.globals());
        let lab_ids: Vec<EntityId> = self.world.labs.keys().copied().collect();

        egui::Window::new("Research inspector")
            .id(egui::Id::new("helios_research_inspector"))
            .open(&mut open)
            .default_width(520.0)
            .default_height(420.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("breadcrumb: Catalog/Research").weak());
                ui.label(format!(
                    "world.labs = {}   tech lines = {}   segments = {}   rp_cost/seg = {:.1}",
                    lab_ids.len(),
                    self.tech_lines.len(),
                    self.tech_segments.len(),
                    rp_cost
                ));
                if ui.button("Open Designs…").clicked() {
                    open_designs = true;
                }
                ui.separator();

                ui.label(RichText::new("Labs (roster)").strong());
                ScrollArea::vertical()
                    .id_source("research_lab_roster")
                    .max_height(140.0)
                    .show(ui, |ui| {
                        if lab_ids.is_empty() {
                            ui.label("(no labs — spawn via operator research)");
                        } else {
                            for lid in &lab_ids {
                                // Live Lab ref — no Lab clone.
                                let Some(lab) = self.world.labs.get(lid) else {
                                    continue;
                                };
                                let assigned = lab
                                    .assigned_segment
                                    .as_deref()
                                    .unwrap_or("(idle)");
                                ui.horizontal(|ui| {
                                    ui.monospace(format!(
                                        "lab {lid}  empire={}  cap={:.1}  prog={:.1}  site={:?}  queue={assigned}",
                                        lab.empire_id, lab.capacity, lab.progress_rp, lab.site_system
                                    ));
                                    if ui.button("Detail").clicked() {
                                        pick_lab = Some(*lid);
                                    }
                                });
                            }
                        }
                    });

                ui.separator();
                ui.label(RichText::new("Lab detail").strong());
                match self.research_lab {
                    Some(lid) => match self.world.labs.get(&lid) {
                        Some(lab) => {
                            ui.label(
                                RichText::new(format!(
                                    "breadcrumb: Catalog/Research → Lab {lid}"
                                ))
                                .weak(),
                            );
                            egui::Grid::new(("lab_grid", lid.0))
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("id");
                                    ui.monospace(lab.id.to_string());
                                    ui.end_row();
                                    ui.label("empire_id");
                                    ui.monospace(lab.empire_id.to_string());
                                    ui.end_row();
                                    ui.label("capacity");
                                    ui.monospace(format!("{:.6}", lab.capacity));
                                    ui.end_row();
                                    ui.label("progress_rp");
                                    ui.monospace(format!("{:.6}", lab.progress_rp));
                                    ui.end_row();
                                    ui.label("assigned_segment");
                                    ui.monospace(
                                        lab.assigned_segment
                                            .as_deref()
                                            .unwrap_or("(none)"),
                                    );
                                    ui.end_row();
                                    ui.label("site_system");
                                    ui.monospace(format!("{:?}", lab.site_system));
                                    ui.end_row();
                                    ui.label("rp remaining (est)");
                                    let rem = (rp_cost - lab.progress_rp).max(0.0);
                                    ui.monospace(format!("{rem:.6} / cost {rp_cost:.1}"));
                                    ui.end_row();
                                });
                            if let Some(seg_id) = lab.assigned_segment.as_deref() {
                                if let Some(seg) =
                                    self.tech_segments.iter().find(|s| s.id == seg_id)
                                {
                                    ui.label(RichText::new("queued segment (catalog)").strong());
                                    ui.monospace(format!(
                                        "{} — {} (line {} idx {})",
                                        seg.id, seg.display_name, seg.line_id, seg.index
                                    ));
                                    ui.monospace(format!("gates: {:?}", seg.material_gates));
                                    ui.monospace(format!("unlocks: {:?}", seg.unlocks));
                                }
                            }
                            // Empire unlocks for this lab's empire (live ledger refs).
                            if let Some(emp) = self.world.ledger().get_empire(lab.empire_id) {
                                ui.separator();
                                ui.label(RichText::new("empire unlocks").strong());
                                ui.monospace(format!(
                                    "unlocked_segments ({})",
                                    emp.unlocked_segments.len()
                                ));
                                for s in emp.unlocked_segments.iter().take(32) {
                                    ui.monospace(format!("  {s}"));
                                }
                                if emp.unlocked_segments.len() > 32 {
                                    ui.label(format!(
                                        "… +{} more",
                                        emp.unlocked_segments.len() - 32
                                    ));
                                }
                                ui.monospace(format!(
                                    "unlocked_catalog_ids ({})",
                                    emp.unlocked_catalog_ids.len()
                                ));
                                for c in emp.unlocked_catalog_ids.iter().take(32) {
                                    ui.monospace(format!("  {c}"));
                                }
                                if emp.unlocked_catalog_ids.len() > 32 {
                                    ui.label(format!(
                                        "… +{} more",
                                        emp.unlocked_catalog_ids.len() - 32
                                    ));
                                }
                            }
                        }
                        None => {
                            ui.label("Lab not found in world.labs.");
                        }
                    },
                    None => {
                        ui.label("(select a lab from the roster)");
                    }
                }

                ui.separator();
                ui.label(RichText::new("Tech lines / segments (cached catalog)").strong());
                ScrollArea::vertical()
                    .id_source("research_tech_book")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for line in &self.tech_lines {
                            ui.label(RichText::new(format!(
                                "{} — {} ({:?})",
                                line.id, line.name, line.category
                            )).strong());
                            for sid in &line.segment_ids {
                                if let Some(seg) =
                                    self.tech_segments.iter().find(|s| s.id == *sid)
                                {
                                    ui.monospace(format!(
                                        "  [{}] {}  {}",
                                        seg.index, seg.id, seg.display_name
                                    ));
                                } else {
                                    ui.monospace(format!("  {sid}"));
                                }
                            }
                        }
                    });
            });

        if let Some(lid) = pick_lab {
            self.research_lab = Some(lid);
        }
        if open_designs {
            self.designs_open = true;
        }
        if !open {
            self.research_open = false;
        }
    }

    fn show_design_inspector(&mut self, ctx: &egui::Context) {
        if !self.designs_open {
            return;
        }
        let mut open = true;
        let mut pick: Option<EntityId> = None;
        let design_ids: Vec<EntityId> = self.world.ship_designs.keys().copied().collect();

        egui::Window::new("Design inspector")
            .id(egui::Id::new("helios_design_inspector"))
            .open(&mut open)
            .default_width(480.0)
            .default_height(360.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("breadcrumb: Catalog/Designs").weak());
                ui.label(format!("world.ship_designs = {}", design_ids.len()));
                ui.separator();
                ui.label(RichText::new("Designs (roster)").strong());
                ScrollArea::vertical()
                    .id_source("design_roster")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        if design_ids.is_empty() {
                            ui.label("(no designs — register via hulls / operator)");
                        } else {
                            for did in &design_ids {
                                // Live ShipDesign ref — no clone.
                                let Some(d) = self.world.ship_designs.get(did) else {
                                    continue;
                                };
                                ui.horizontal(|ui| {
                                    ui.monospace(format!(
                                        "design {did}  {}  fuel={}  crew={:.1}  mass={:.1}  mods={}",
                                        d.name,
                                        d.fuel_tier,
                                        d.crew_req,
                                        d.mass,
                                        d.modules.len()
                                    ));
                                    if ui.button("Detail").clicked() {
                                        pick = Some(*did);
                                    }
                                });
                            }
                        }
                    });

                ui.separator();
                ui.label(RichText::new("Design detail").strong());
                match self.selected_design {
                    Some(did) => match self.world.ship_designs.get(&did) {
                        Some(d) => {
                            ui.label(
                                RichText::new(format!(
                                    "breadcrumb: Catalog/Designs → {did}"
                                ))
                                .weak(),
                            );
                            egui::Grid::new(("design_grid", did.0))
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("id");
                                    ui.monospace(d.id.to_string());
                                    ui.end_row();
                                    ui.label("name");
                                    ui.monospace(d.name.as_str());
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
                                    ui.label("cargo_capacity");
                                    ui.monospace(format!("{:.6}", hulls::cargo_capacity(d)));
                                    ui.end_row();
                                });
                            ui.separator();
                            ui.label(RichText::new("modules").strong());
                            if d.modules.is_empty() {
                                ui.label("(none)");
                            } else {
                                for m in &d.modules {
                                    ui.monospace(format!("  {m}"));
                                }
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
            });

        if let Some(did) = pick {
            self.selected_design = Some(did);
        }
        if !open {
            self.designs_open = false;
        }
    }
}

impl eframe::App for HeliosApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Time: when unpaused, advance live World at ~20 Hz × increment.
        if !self.paused {
            let dt = ctx.input(|i| i.unstable_dt);
            self.tick_accum += dt;
            let period = 0.05_f32;
            while self.tick_accum >= period {
                self.tick_accum -= period;
                self.step();
            }
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("helios_time_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Helios Chronicle");
                ui.separator();
                ui.label(format!(
                    "seed={}  tick={}  systems={}  bodies={}  ships={}  lod={:?}  dt={}",
                    self.world.seed(),
                    self.world.master_tick(),
                    self.world.ledger().len(),
                    self.world.ledger().bodies_len(),
                    self.world.ships.len(),
                    self.world.lod(),
                    self.world.current_dt()
                ));
            });
            ui.horizontal(|ui| {
                if ui
                    .button(if self.paused { "▶ Run" } else { "⏸ Pause" })
                    .clicked()
                {
                    self.paused = !self.paused;
                    self.tick_accum = 0.0;
                }
                if ui.button("Step").clicked() {
                    self.paused = true;
                    self.step();
                }
                ui.label("Increment:");
                for n in [1_u64, 10, 100] {
                    let selected = self.increment == n;
                    if ui.selectable_label(selected, n.to_string()).clicked() {
                        self.increment = n;
                    }
                }
                ui.separator();
                ui.label("Viewpoint:");
                if ui
                    .selectable_label(self.viewpoint == Viewpoint::Operator, "Operator (all)")
                    .clicked()
                {
                    self.viewpoint = Viewpoint::Operator;
                }
                let empire_label = self
                    .world
                    .ledger()
                    .empires()
                    .next()
                    .map(|(id, e)| {
                        format!(
                            "Empire: {}",
                            e.name.clone().unwrap_or_else(|| id.to_string())
                        )
                    })
                    .unwrap_or_else(|| "Empire: (none)".into());
                if ui
                    .selectable_label(self.viewpoint == Viewpoint::Empire, empire_label)
                    .clicked()
                {
                    self.viewpoint = Viewpoint::Empire;
                }
                if self.viewpoint == Viewpoint::Empire {
                    ui.colored_label(
                        Color32::from_rgb(200, 180, 100),
                        "(filter chrome stub — knowledge soft)",
                    );
                }
                ui.separator();
                if ui.button("Fleet").clicked() {
                    self.fleet_open = true;
                }
                if ui.button("Research").clicked() {
                    self.research_open = true;
                }
                if ui.button("Designs").clicked() {
                    self.designs_open = true;
                }
            });
        });

        // Issue 11: Chronicle/History panel — EventLog moments only (not a replay viewer).
        egui::TopBottomPanel::bottom("helios_event_strip")
            .resizable(true)
            .default_height(140.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Chronicle / History (EventLog moments, read-only)")
                        .strong(),
                );
                let events = self.world.log().events();
                let start = events.len().saturating_sub(EVENT_STRIP_MAX);
                ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for ev in &events[start..] {
                            let sys = Self::kind_system(&ev.kind)
                                .map(|id| id.to_string())
                                .unwrap_or_else(|| "-".into());
                            ui.monospace(format!(
                                "seq={:<5} tick={:<7} sys={:<6} {:<22} {}",
                                ev.seq,
                                ev.at_tick,
                                sys,
                                Self::kind_name(&ev.kind),
                                Self::kind_one_line(&ev.kind)
                            ));
                        }
                    });
            });

        // Floating inspectors (several may be open at once — STATEMENT §11).
        self.show_system_inspector(ctx);
        self.show_fleet_list(ctx);
        self.show_body_inspectors(ctx);
        self.show_ship_inspectors(ctx);
        self.show_research_inspector(ctx);
        self.show_design_inspector(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Map (pan: drag · zoom: scroll · click: inspect)");
            let empire_soft = self.viewpoint == Viewpoint::Empire;
            let (_resp, clicked) =
                map::draw_map(ui, &self.world, &mut self.camera, self.selected, empire_soft);
            if let Some(id) = clicked {
                self.selected = Some(id);
                self.inspector_open = true;
            }
        });
    }
}

/// Launch the operator shell window.
pub fn run(seed: u64) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Helios Chronicle — Operator Shell"),
        ..Default::default()
    };
    eframe::run_native(
        "Helios Chronicle",
        options,
        Box::new(move |_cc| Ok(Box::new(HeliosApp::new(seed)))),
    )
}
