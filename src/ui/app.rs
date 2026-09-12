//! eframe application: shell chrome around the live [`World`].

use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::entity::EntityId;
use crate::event::EventKind;
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
        Self {
            world,
            paused: true,
            increment: 1,
            camera: cam,
            selected: None,
            viewpoint: Viewpoint::Operator,
            tick_accum: 0.0,
            inspector_open: false,
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
                    "seed={}  tick={}  systems={}  lod={:?}  dt={}",
                    self.world.seed(),
                    self.world.master_tick(),
                    self.world.ledger().len(),
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

        // System inspector as a floating egui window (opens on map select).
        if self.inspector_open {
            let mut open = true;
            let title = match self.selected {
                Some(id) => format!("System inspector — {id}"),
                None => "System inspector".into(),
            };
            egui::Window::new(title)
                .open(&mut open)
                .default_width(320.0)
                .show(ctx, |ui| {
                    if let Some(id) = self.selected {
                        if let Some(sys) = self.world.ledger().get(id) {
                            let state = sky::map_state(sys);
                            ui.monospace(format!("id:              {id}"));
                            ui.monospace(format!("x/y:             {:.2} / {:.2}", sys.x, sys.y));
                            ui.monospace(format!(
                                "binding_remainder:{:.3}",
                                sys.binding_remainder
                            ));
                            ui.monospace(format!("depleted:        {}", sys.depleted));
                            ui.monospace(format!("fuse_end_tick:   {:?}", sys.fuse_end_tick));
                            ui.monospace(format!("fuse_remaining:  {:?}", sys.fuse_remaining));
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
                    } else {
                        ui.label("Select a system on the map.");
                    }
                });
            if !open {
                self.inspector_open = false;
            }
        }

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
