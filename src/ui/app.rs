//! eframe application: shell chrome around the live [`World`].

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::entity::EntityId;
use crate::event::EventKind;
use crate::hulls::{self};
use crate::research::{self, TechLine, TechSegment};
use crate::sky;
use crate::world::World;

use super::fog::{self, SystemKnowledge};
use super::map::{self, MapAction, MapCamera, MapChrome};
use super::tags::{self, TagColors, TagKind};
use super::waypoints::WaypointStore;

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
    /// Ledger empire id for Empire viewpoint (first empire / cycle pick).
    pub viewpoint_empire: Option<EntityId>,
    /// Wall-clock accumulator for unpaused ticking (seconds).
    tick_accum: f32,
    inspector_open: bool,
    /// Body inspector windows currently open (several at once OK).
    open_bodies: BTreeSet<EntityId>,
    /// Ship / fleet-detail inspector windows currently open.
    open_ships: BTreeSet<EntityId>,
    /// Colony inspector windows (keyed by body id; several at once OK).
    open_colonies: BTreeSet<EntityId>,
    /// Industry inspector windows (keyed by system id; several at once OK).
    open_industry: BTreeSet<EntityId>,
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
    /// Operator tag colors (empire / fleet / body) — UI chrome, not ledger.
    tag_colors: TagColors,
    /// Named waypoints on the map — persist across viewpoint swaps.
    waypoints: WaypointStore,
    tags_open: bool,
    waypoints_open: bool,
    /// Button+click place mode for waypoints (right-click also places).
    place_waypoint_mode: bool,
    /// Selected waypoint id in the Waypoints window editor.
    selected_waypoint: Option<u64>,
    /// Catalog inspector (tech book lines / segments).
    catalog_open: bool,
    /// Selected segment id in Catalog detail pane.
    selected_segment: Option<String>,
    /// Yard / fleet-yard inspector.
    yard_open: bool,
    /// Empire whose yard tooling is shown (picker / viewpoint).
    yard_empire: Option<EntityId>,
    /// Contact inspector (treaties / contracts / standing / fog).
    contact_open: bool,
    /// Empire selected in Contact inspector.
    contact_empire: Option<EntityId>,
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
        let viewpoint_empire = world.ledger().empires().next().map(|(id, _)| *id);
        Self {
            world,
            paused: true,
            increment: 1,
            camera: cam,
            selected: None,
            viewpoint: Viewpoint::Operator,
            viewpoint_empire,
            tick_accum: 0.0,
            inspector_open: false,
            open_bodies: BTreeSet::new(),
            open_ships: BTreeSet::new(),
            open_colonies: BTreeSet::new(),
            open_industry: BTreeSet::new(),
            fleet_open: false,
            research_open: false,
            research_lab: None,
            designs_open: false,
            selected_design: None,
            tech_lines,
            tech_segments,
            tag_colors: TagColors::new(),
            waypoints: WaypointStore::new(),
            tags_open: false,
            waypoints_open: false,
            place_waypoint_mode: false,
            selected_waypoint: None,
            catalog_open: false,
            selected_segment: None,
            yard_open: false,
            yard_empire: viewpoint_empire,
            contact_open: false,
            contact_empire: viewpoint_empire,
        }
    }

    fn step(&mut self) {
        self.world.tick(self.increment.max(1));
    }

    /// Keep Empire viewpoint pointed at a live ledger empire (first if unset).
    fn ensure_viewpoint_empire(&mut self) {
        let current_ok = self.viewpoint_empire.is_some_and(|id| {
            self.world.ledger().get_empire(id).is_some()
        });
        if current_ok {
            return;
        }
        self.viewpoint_empire = self.world.ledger().empires().next().map(|(id, _)| *id);
    }

    /// Cycle `viewpoint_empire` through ledger empires (cheap, no alloc beyond scan).
    fn cycle_viewpoint_empire(&mut self) {
        let mut first = None;
        let mut take_next = false;
        let mut next = None;
        for (id, _) in self.world.ledger().empires() {
            if first.is_none() {
                first = Some(*id);
            }
            if take_next {
                next = Some(*id);
                break;
            }
            if Some(*id) == self.viewpoint_empire {
                take_next = true;
            }
        }
        self.viewpoint_empire = next.or(first);
    }

    /// Live fog for Empire viewpoint; Operator → None (fog off).
    fn viewpoint_fog(&self) -> Option<&crate::contact::FogState> {
        if self.viewpoint != Viewpoint::Empire {
            return None;
        }
        let eid = self.viewpoint_empire?;
        fog::fog_of(&self.world, fog::as_empire_id(eid))
    }

    /// Empire display color (override or hash default).
    fn empire_color(&self, empire_id: EntityId) -> Color32 {
        self.tag_colors.resolve(TagKind::Empire, empire_id)
    }

    /// Color for a system's map/header name: body/fleet overrides unused;
    /// prefer home empire tag when present.
    fn system_name_color(&self, system_id: EntityId) -> Color32 {
        if let Some(sys) = self.world.ledger().get(system_id) {
            if let Some(emp) = sys.home_empire {
                return self.empire_color(EntityId(emp.0));
            }
        }
        Color32::from_gray(200)
    }

    fn body_header_color(&self, body_id: EntityId) -> Color32 {
        if let Some(c) = self.tag_colors.get(TagKind::Body, body_id) {
            return c;
        }
        // Fall back to parent system's home empire color when known.
        if let Some(body) = self.world.ledger().get_body(body_id) {
            if let Some(sys) = self.world.ledger().get(body.system) {
                if let Some(emp) = sys.home_empire {
                    return self.empire_color(EntityId(emp.0));
                }
            }
        }
        TagColors::hash_color(body_id)
    }

    fn fleet_header_color(&self, ship_id: EntityId) -> Color32 {
        self.tag_colors
            .get(TagKind::Fleet, ship_id)
            .unwrap_or_else(|| TagColors::hash_color(ship_id))
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
        let (title, title_color) = match self.selected {
            Some(id) => (
                format!("System inspector — {id}"),
                self.system_name_color(id),
            ),
            None => ("System inspector".into(), Color32::from_gray(220)),
        };
        let mut open_body: Option<EntityId> = None;
        let mut open_ship: Option<EntityId> = None;
        let mut open_colony: Option<EntityId> = None;
        let mut open_industry_sys: Option<EntityId> = None;
        let mut open_fleet = false;

        // Resolve fog to owned flags before UI mutably borrows self.
        let fog_on = self.viewpoint == Viewpoint::Empire;
        let knowledge = if fog_on {
            let fog = self.viewpoint_fog();
            self.selected.map(|sid| fog::system_knowledge(fog, sid))
        } else {
            None
        };
        let known_fleet_ids: Vec<EntityId> = if fog_on {
            self.viewpoint_fog()
                .map(|f| f.known_fleets.keys().copied().collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        egui::Window::new(RichText::new(title).color(title_color))
            .id(egui::Id::new("helios_system_inspector"))
            .open(&mut open)
            .default_width(340.0)
            .show(ctx, |ui| {
                if let Some(id) = self.selected {
                    match knowledge {
                        Some(SystemKnowledge::Unknown) if fog_on => {
                            ui.label(RichText::new("breadcrumb: System").weak());
                            ui.separator();
                            ui.label(
                                RichText::new("unknown — outside empire fog")
                                    .color(Color32::from_rgb(200, 180, 100)),
                            );
                            ui.monospace(format!("id:              {id}"));
                            ui.monospace("knowledge:       unknown");
                            ui.monospace("binding_remainder: unknown");
                            ui.monospace("fuse_end_tick:   unknown");
                            ui.monospace("fuse_remaining:  unknown");
                            ui.monospace("map_state:       unknown");
                            ui.label(
                                RichText::new("(limited rows — system not in known_systems)")
                                    .weak()
                                    .small(),
                            );
                        }
                        _ => {
                            let surveyed_fuse = matches!(
                                knowledge,
                                Some(SystemKnowledge::Known {
                                    surveyed_fuse: true
                                })
                            );
                            let show_fuse = !fog_on || surveyed_fuse;

                            if let Some(sys) = self.world.ledger().get(id) {
                                ui.label(RichText::new("breadcrumb: System").weak());
                                ui.separator();
                                ui.monospace(format!("id:              {id}"));
                                ui.monospace(format!(
                                    "x/y:             {:.2} / {:.2}",
                                    sys.x, sys.y
                                ));
                                ui.monospace(format!(
                                    "binding_remainder:{:.3}",
                                    sys.binding_remainder
                                ));
                                ui.monospace(format!("depleted:        {}", sys.depleted));
                                if show_fuse {
                                    ui.monospace(format!(
                                        "fuse_end_tick:   {:?}",
                                        sys.fuse_end_tick
                                    ));
                                    ui.monospace(format!(
                                        "fuse_remaining:  {:?}",
                                        sys.fuse_remaining
                                    ));
                                } else {
                                    ui.monospace("fuse_end_tick:   unknown");
                                    ui.monospace("fuse_remaining:  unknown");
                                    ui.label(
                                        RichText::new(
                                            "(fuse hidden — surveyed_fuse=false)",
                                        )
                                        .weak()
                                        .small(),
                                    );
                                }
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
                                        ui.monospace(format!(
                                            "body {bid}  pops={pops:.0} auto={auto}"
                                        ));
                                        if ui.button("Open Body").clicked() {
                                            open_body = Some(bid);
                                        }
                                        if ui.button("Colony").clicked() {
                                            open_colony = Some(bid);
                                        }
                                    });
                                }
                            }

                            ui.separator();
                            ui.label(RichText::new("Fleet (System → Ship)").strong());
                            if fog_on {
                                // Unseen fleets absent — only contact fog known_fleets.
                                if known_fleet_ids.is_empty() {
                                    ui.label("(no known fleets in empire fog)");
                                } else {
                                    for sid in known_fleet_ids.iter().take(24) {
                                        ui.horizontal(|ui| {
                                            ui.monospace(format!("ship {sid}"));
                                            if ui.button("Open Ship").clicked() {
                                                open_ship = Some(*sid);
                                            }
                                        });
                                    }
                                    if known_fleet_ids.len() > 24 {
                                        ui.label(format!(
                                            "… +{} more known",
                                            known_fleet_ids.len() - 24
                                        ));
                                    }
                                }
                            } else {
                                ui.label(
                                    RichText::new(
                                        "ShipInstance has no system field yet — listing all world.ships",
                                    )
                                    .weak()
                                    .small(),
                                );
                                let ships: Vec<EntityId> =
                                    self.world.ships.keys().copied().collect();
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
                                        ui.label(format!(
                                            "… +{} more (see Fleet list)",
                                            ships.len() - 24
                                        ));
                                    }
                                }
                            }
                            if ui.button("Open Fleet list…").clicked() {
                                open_fleet = true;
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button("Industry…").clicked() {
                                    open_industry_sys = Some(id);
                                }
                                if ui.button("Research…").clicked() {
                                    self.research_open = true;
                                }
                                if ui.button("Designs…").clicked() {
                                    self.designs_open = true;
                                }
                                if ui.button("Yard…").clicked() {
                                    self.yard_open = true;
                                    if let Some(eid) = self.viewpoint_empire {
                                        self.yard_empire = Some(eid);
                                    }
                                }
                                if ui.button("Catalog…").clicked() {
                                    self.catalog_open = true;
                                }
                                if ui.button("Contact…").clicked() {
                                    self.contact_open = true;
                                    if let Some(eid) = self.viewpoint_empire {
                                        self.contact_empire = Some(eid);
                                    }
                                }
                            });
                        }
                    }
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
        if let Some(bid) = open_colony {
            let fog_filter = self.viewpoint == Viewpoint::Empire;
            let fog = self.viewpoint_fog();
            let sys_ok = self
                .world
                .ledger()
                .get_body(bid)
                .map(|b| fog::may_open_system_detail(fog_filter, fog, b.system))
                .unwrap_or(false);
            if sys_ok {
                self.open_colonies.insert(bid);
            }
        }
        if let Some(sid) = open_industry_sys {
            let fog_filter = self.viewpoint == Viewpoint::Empire;
            let fog = self.viewpoint_fog();
            if fog::may_open_system_detail(fog_filter, fog, sid) {
                self.open_industry.insert(sid);
            }
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
            let title_color = self.body_header_color(bid);
            egui::Window::new(RichText::new(title).color(title_color))
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
                                    "breadcrumb stub: System → Body → Colony",
                                )
                                .weak()
                                .small(),
                            );
                            if ui.button("Open Colony…").clicked() {
                                let fog_filter = self.viewpoint == Viewpoint::Empire;
                                let fog = self.viewpoint_fog();
                                let sys_ok = self
                                    .world
                                    .ledger()
                                    .get_body(bid)
                                    .map(|b| fog::may_open_system_detail(fog_filter, fog, b.system))
                                    .unwrap_or(false);
                                if sys_ok {
                                    self.open_colonies.insert(bid);
                                }
                            }
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
            let title_color = self.fleet_header_color(sid);
            // Display from live refs / scalars — no per-frame ShipInstance/ShipDesign clones.
            let world = &self.world;
            egui::Window::new(RichText::new(title).color(title_color))
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
                if ui.button("Open Catalog…").clicked() {
                    self.catalog_open = true;
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
                if ui.button("Open Yard…").clicked() {
                    self.yard_open = true;
                    if let Some(eid) = self.viewpoint_empire {
                        self.yard_empire = Some(eid);
                    }
                }
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

    fn show_tag_colors(&mut self, ctx: &egui::Context) {
        if !self.tags_open {
            return;
        }
        let mut open = true;
        // Snapshot empire ids/names for the roster (avoid borrow fights).
        let empires: Vec<(EntityId, String)> = self
            .world
            .ledger()
            .empires()
            .map(|(id, e)| {
                (
                    *id,
                    e.name.clone().unwrap_or_else(|| id.to_string()),
                )
            })
            .collect();
        let bodies: Vec<EntityId> = self.open_bodies.iter().copied().collect();
        let ships: Vec<EntityId> = self.open_ships.iter().copied().collect();

        egui::Window::new("Tag colors")
            .id(egui::Id::new("helios_tag_colors"))
            .open(&mut open)
            .default_width(380.0)
            .default_height(320.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Operator chrome — color wheel for empires / selected fleets & bodies. Defaults hash empire id.",
                    )
                    .weak()
                    .small(),
                );
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("Empires").strong());
                    for (eid, name) in &empires {
                        ui.horizontal(|ui| {
                            let mut c = self.tag_colors.resolve(TagKind::Empire, *eid);
                            let before = c;
                            tags::color_wheel_button(ui, &mut c);
                            if c != before {
                                self.tag_colors.set(TagKind::Empire, *eid, c);
                            }
                            ui.label(RichText::new(name).color(c));
                            ui.monospace(format!("({eid})"));
                            if self.tag_colors.get(TagKind::Empire, *eid).is_some() {
                                if ui.small_button("Reset").clicked() {
                                    self.tag_colors.clear(TagKind::Empire, *eid);
                                }
                            }
                        });
                    }
                    if empires.is_empty() {
                        ui.label("(no empires on ledger)");
                    }

                    ui.separator();
                    ui.label(RichText::new("Open fleets (ships)").strong());
                    if ships.is_empty() {
                        ui.label(RichText::new("(open a ship inspector to tag it)").weak().small());
                    }
                    for sid in &ships {
                        ui.horizontal(|ui| {
                            let mut c = self.fleet_header_color(*sid);
                            let before = c;
                            tags::color_wheel_button(ui, &mut c);
                            if c != before {
                                self.tag_colors.set(TagKind::Fleet, *sid, c);
                            }
                            ui.label(RichText::new(format!("ship {sid}")).color(c));
                            if self.tag_colors.get(TagKind::Fleet, *sid).is_some() {
                                if ui.small_button("Reset").clicked() {
                                    self.tag_colors.clear(TagKind::Fleet, *sid);
                                }
                            }
                        });
                    }

                    ui.separator();
                    ui.label(RichText::new("Open bodies").strong());
                    if bodies.is_empty() {
                        ui.label(RichText::new("(open a body inspector to tag it)").weak().small());
                    }
                    for bid in &bodies {
                        ui.horizontal(|ui| {
                            let mut c = self.body_header_color(*bid);
                            let before = c;
                            tags::color_wheel_button(ui, &mut c);
                            if c != before {
                                self.tag_colors.set(TagKind::Body, *bid, c);
                            }
                            ui.label(RichText::new(format!("body {bid}")).color(c));
                            if self.tag_colors.get(TagKind::Body, *bid).is_some() {
                                if ui.small_button("Reset").clicked() {
                                    self.tag_colors.clear(TagKind::Body, *bid);
                                }
                            }
                        });
                    }
                });
            });
        if !open {
            self.tags_open = false;
        }
    }

    fn show_waypoints_window(&mut self, ctx: &egui::Context) {
        if !self.waypoints_open {
            return;
        }
        let mut open = true;
        let mut delete_id: Option<u64> = None;
        let mut focus_id: Option<u64> = None;

        egui::Window::new("Waypoints")
            .id(egui::Id::new("helios_waypoints"))
            .open(&mut open)
            .default_width(420.0)
            .default_height(300.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Pins on the glass — right-click map (or Place mode + click). Not fleets / not binding.",
                    )
                    .weak()
                    .small(),
                );
                ui.horizontal(|ui| {
                    let placing = self.place_waypoint_mode;
                    if ui
                        .selectable_label(
                            placing,
                            if placing {
                                "Place mode ON"
                            } else {
                                "Place mode"
                            },
                        )
                        .clicked()
                    {
                        self.place_waypoint_mode = !self.place_waypoint_mode;
                    }
                    ui.label(format!("{} waypoints", self.waypoints.len()));
                });
                ui.separator();

                let ids: Vec<u64> = self.waypoints.items().iter().map(|w| w.id).collect();
                ScrollArea::vertical().show(ui, |ui| {
                    if self.waypoints.is_empty() {
                        ui.label("(none — right-click the map to drop one)");
                    }
                    for id in ids {
                        let selected = self.selected_waypoint == Some(id);
                        let (mut name, mut note, mut color, x, y) = {
                            let w = match self.waypoints.get_mut(id) {
                                Some(w) => w,
                                None => continue,
                            };
                            (w.name.clone(), w.note.clone(), w.color, w.x, w.y)
                        };
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                if ui.selectable_label(selected, format!("#{id}")).clicked() {
                                    focus_id = Some(id);
                                }
                                if tags::color_wheel_button(ui, &mut color).changed() {
                                    if let Some(w) = self.waypoints.get_mut(id) {
                                        w.color = color;
                                    }
                                }
                                if ui.text_edit_singleline(&mut name).changed() {
                                    if let Some(w) = self.waypoints.get_mut(id) {
                                        w.name = name.clone();
                                    }
                                }
                                ui.label(
                                    RichText::new(format!("({x:.1}, {y:.1})"))
                                        .weak()
                                        .small(),
                                );
                                if ui.small_button("Go").clicked() {
                                    self.camera.center = egui::Pos2::new(x, y);
                                    focus_id = Some(id);
                                }
                                if ui.small_button("Delete").clicked() {
                                    delete_id = Some(id);
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("note");
                                if ui.text_edit_singleline(&mut note).changed() {
                                    if let Some(w) = self.waypoints.get_mut(id) {
                                        w.note = note.clone();
                                    }
                                }
                            });
                        });
                    }
                });
            });

        if let Some(id) = focus_id {
            self.selected_waypoint = Some(id);
        }
        if let Some(id) = delete_id {
            self.waypoints.remove(id);
            if self.selected_waypoint == Some(id) {
                self.selected_waypoint = None;
            }
        }
        if !open {
            self.waypoints_open = false;
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
                self.ensure_viewpoint_empire();
                let (empire_label, empire_color) = match self.viewpoint_empire {
                    Some(eid) => {
                        let name = self
                            .world
                            .ledger()
                            .get_empire(eid)
                            .and_then(|e| e.name.clone())
                            .unwrap_or_else(|| eid.to_string());
                        (format!("Empire: {name}"), self.empire_color(eid))
                    }
                    None => ("Empire: (none)".into(), Color32::from_gray(200)),
                };
                if ui
                    .selectable_label(
                        self.viewpoint == Viewpoint::Empire,
                        RichText::new(empire_label).color(empire_color),
                    )
                    .clicked()
                {
                    self.viewpoint = Viewpoint::Empire;
                    self.ensure_viewpoint_empire();
                }
                if self.viewpoint == Viewpoint::Empire {
                    if ui.small_button("Cycle empire").clicked() {
                        self.cycle_viewpoint_empire();
                    }
                    let known_n = self
                        .viewpoint_fog()
                        .map(|f| f.known_systems.len())
                        .unwrap_or(0);
                    ui.colored_label(
                        Color32::from_rgb(200, 180, 100),
                        format!("(fog filter — {known_n} known systems)"),
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
                if ui.button("Catalog").clicked() {
                    self.catalog_open = true;
                }
                if ui.button("Yard").clicked() {
                    self.yard_open = true;
                    if let Some(eid) = self.viewpoint_empire {
                        self.yard_empire = Some(eid);
                    }
                }
                if ui.button("Contact").clicked() {
                    self.contact_open = true;
                    if let Some(eid) = self.viewpoint_empire {
                        self.contact_empire = Some(eid);
                    }
                }
                if ui.button("Industry").clicked() {
                    if let Some(sid) = self.selected {
                        let fog_filter = self.viewpoint == Viewpoint::Empire;
                        let fog = self.viewpoint_fog();
                        if fog::may_open_system_detail(fog_filter, fog, sid) {
                            self.open_industry.insert(sid);
                        }
                    }
                }
                if ui.button("Colony").clicked() {
                    // Open colony for first open body, else first body of selected system.
                    // Empire + unknown/missing fog: do not open full Colony (same check as map).
                    let fog_filter = self.viewpoint == Viewpoint::Empire;
                    let fog = self.viewpoint_fog();
                    let pick = self.open_bodies.iter().next().copied().or_else(|| {
                        self.selected.and_then(|sid| {
                            if !fog::may_open_system_detail(fog_filter, fog, sid) {
                                return None;
                            }
                            self.world
                                .ledger()
                                .bodies_for_system(sid)
                                .next()
                                .map(|(id, _)| *id)
                        })
                    });
                    if let Some(bid) = pick {
                        let sys_ok = self
                            .world
                            .ledger()
                            .get_body(bid)
                            .map(|b| fog::may_open_system_detail(fog_filter, fog, b.system))
                            .unwrap_or(false);
                        if sys_ok {
                            self.open_colonies.insert(bid);
                        }
                    }
                }
                if ui.button("Tags").clicked() {
                    self.tags_open = true;
                }
                if ui.button("Waypoints").clicked() {
                    self.waypoints_open = true;
                }
                let placing = self.place_waypoint_mode;
                if ui
                    .selectable_label(
                        placing,
                        if placing {
                            "Place WP: ON"
                        } else {
                            "Place WP"
                        },
                    )
                    .on_hover_text("Click empty map to drop a waypoint (right-click always works)")
                    .clicked()
                {
                    self.place_waypoint_mode = !self.place_waypoint_mode;
                    if self.place_waypoint_mode {
                        self.waypoints_open = true;
                    }
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
        super::colony::show_colony_inspectors(ctx, &self.world, &mut self.open_colonies);
        super::industry::show_industry_inspectors(ctx, &self.world, &mut self.open_industry);
        {
            let cat = super::catalog::show_catalog_inspector(
                ctx,
                &mut self.catalog_open,
                &self.tech_lines,
                &self.tech_segments,
                &mut self.selected_segment,
            );
            if cat == super::catalog::CatalogAction::OpenResearch {
                self.research_open = true;
            }
        }
        {
            let ya = super::yard::show_yard_inspector(
                ctx,
                &self.world,
                &mut self.yard_open,
                &mut self.yard_empire,
                &mut self.selected_design,
            );
            if ya == super::yard::YardAction::OpenDesigns {
                self.designs_open = true;
            }
        }
        super::contacts::show_contact_inspector(
            ctx,
            &self.world,
            &mut self.contact_open,
            &mut self.contact_empire,
        );
        self.show_tag_colors(ctx);
        self.show_waypoints_window(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(
                "Map (pan: drag · zoom: scroll · click: inspect · right-click / Place WP: waypoint)",
            );
            // Empire viewpoint: live contact fog ref (no clone). Operator: fog off.
            // Borrow fog from world.contact only so camera can be mutably borrowed.
            let fog_filter = self.viewpoint == Viewpoint::Empire;
            // Missing contact under Empire → None fog with filter on = all unknown.
            let fog = match (self.viewpoint, self.viewpoint_empire) {
                (Viewpoint::Empire, Some(eid)) => self
                    .world
                    .contact
                    .get(fog::as_empire_id(eid))
                    .map(|c| &c.fog),
                _ => None,
            };
            let place_mode = self.place_waypoint_mode;
            // Precompute system→color map so the dyn Fn can close over owned data.
            let mut sys_colors: std::collections::BTreeMap<u64, Color32> =
                std::collections::BTreeMap::new();
            for (id, sys) in self.world.ledger().systems() {
                if let Some(emp) = sys.home_empire {
                    sys_colors.insert(id.0, self.empire_color(EntityId(emp.0)));
                }
            }
            let color_fn = |sid: EntityId| {
                sys_colors
                    .get(&sid.0)
                    .copied()
                    .unwrap_or(Color32::from_gray(200))
            };
            let chrome = MapChrome {
                waypoints: self.waypoints.items(),
                place_mode,
                system_name_color: Some(&color_fn),
            };
            let (_resp, action) =
                map::draw_map(ui, &self.world, &mut self.camera, self.selected, fog_filter, fog, chrome);
            match action {
                Some(MapAction::SelectSystem(id)) => {
                    self.selected = Some(id);
                    self.inspector_open = true;
                }
                Some(MapAction::PlaceWaypoint { x, y }) => {
                    let id = self.waypoints.add_at(x, y);
                    self.selected_waypoint = Some(id);
                    self.waypoints_open = true;
                    // Stay in place mode until toggled off.
                }
                None => {}
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
