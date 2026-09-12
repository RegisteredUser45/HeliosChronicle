//! Galaxy / jump-graph map painter (never goes away).

use egui::{Color32, Pos2, Response, Sense, Stroke, Ui, Vec2};

use crate::entity::EntityId;
use crate::sky::{self, MapState};
use crate::world::World;

/// Screen-space camera over world (x, y) coordinates.
#[derive(Debug, Clone)]
pub struct MapCamera {
    /// World-space point shown at the center of the map rect.
    pub center: Pos2,
    /// World units → screen pixels.
    pub zoom: f32,
}

impl Default for MapCamera {
    fn default() -> Self {
        Self {
            center: Pos2::new(0.0, 0.0),
            zoom: 1.0,
        }
    }
}

impl MapCamera {
    pub fn world_to_screen(&self, world: Pos2, rect_center: Pos2) -> Pos2 {
        let d = world - self.center;
        rect_center + Vec2::new(d.x * self.zoom, d.y * self.zoom)
    }

    pub fn screen_to_world(&self, screen: Pos2, rect_center: Pos2) -> Pos2 {
        let d = screen - rect_center;
        self.center + Vec2::new(d.x / self.zoom, d.y / self.zoom)
    }
}

pub fn map_state_color(state: MapState) -> Color32 {
    match state {
        MapState::Feed => Color32::from_rgb(80, 180, 120),
        MapState::DryFuse => Color32::from_rgb(220, 160, 60),
        MapState::HomePaused => Color32::from_rgb(100, 160, 230),
        MapState::Ended => Color32::from_rgb(90, 90, 90),
        MapState::WildernessUnknown => Color32::from_rgb(160, 120, 200),
    }
}

/// If every system sits at the origin, place a ring + jump cycle on the live ledger
/// so the map has geometry without inventing a parallel sim.
pub fn ensure_map_layout(world: &mut World) {
    let systems: Vec<(EntityId, f64, f64)> = world
        .ledger()
        .systems()
        .map(|(id, s)| (*id, s.x, s.y))
        .collect();
    if systems.is_empty() {
        return;
    }
    let all_origin = systems.iter().all(|(_, x, y)| *x == 0.0 && *y == 0.0);
    if !all_origin {
        return;
    }
    let n = systems.len() as f64;
    let radius = 180.0_f64.max(40.0 * n.sqrt());
    let ids: Vec<EntityId> = systems.iter().map(|(id, _, _)| *id).collect();
    for (i, id) in ids.iter().enumerate() {
        let a = (i as f64) * std::f64::consts::TAU / n;
        if let Some(sys) = world.ledger_mut().get_mut(*id) {
            sys.x = a.cos() * radius;
            sys.y = a.sin() * radius;
        }
    }
    if ids.len() >= 2 {
        for i in 0..ids.len() {
            let a = ids[i];
            let b = ids[(i + 1) % ids.len()];
            if let Some(sys) = world.ledger_mut().get_mut(a) {
                if !sys.jump_links.contains(&b) {
                    sys.jump_links.push(b);
                }
            }
            if let Some(sys) = world.ledger_mut().get_mut(b) {
                if !sys.jump_links.contains(&a) {
                    sys.jump_links.push(a);
                }
            }
        }
    }
}

/// Draw jump graph + systems; returns click selection (nearest system under cursor).
pub fn draw_map(
    ui: &mut Ui,
    world: &World,
    camera: &mut MapCamera,
    selected: Option<EntityId>,
    empire_filter_soft: bool,
) -> (Response, Option<EntityId>) {
    let desired = Vec2::new(ui.available_width(), ui.available_height().max(240.0));
    let (response, painter) = ui.allocate_painter(desired, Sense::click_and_drag());
    let rect = response.rect;
    let rect_center = rect.center();

    painter.rect_filled(rect, 0.0, Color32::from_rgb(12, 14, 22));

    // Pan
    if response.dragged() {
        let delta = response.drag_delta();
        camera.center -= Vec2::new(delta.x / camera.zoom, delta.y / camera.zoom);
    }

    // Zoom (scroll), keep cursor world point stable when possible
    let scroll = ui.input(|i| i.raw_scroll_delta.y);
    if scroll.abs() > 0.0 && response.hovered() {
        let before = ui
            .input(|i| i.pointer.hover_pos())
            .map(|p| camera.screen_to_world(p, rect_center));
        let factor = if scroll > 0.0 { 1.12 } else { 1.0 / 1.12 };
        camera.zoom = (camera.zoom * factor).clamp(0.05, 40.0);
        if let Some(wpos) = before {
            if let Some(screen) = ui.input(|i| i.pointer.hover_pos()) {
                // Recenter so wpos stays under cursor
                let new_screen_of_w = camera.world_to_screen(wpos, rect_center);
                let fix = screen - new_screen_of_w;
                camera.center -= Vec2::new(fix.x / camera.zoom, fix.y / camera.zoom);
            }
        }
    }

    let positions: Vec<(EntityId, Pos2, MapState)> = world
        .ledger()
        .systems()
        .map(|(id, sys)| {
            (
                *id,
                Pos2::new(sys.x as f32, sys.y as f32),
                sky::map_state(sys),
            )
        })
        .collect();

    // Jump links (draw once per undirected edge via id order)
    let stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(90, 110, 140, 160));
    for (id, sys) in world.ledger().systems() {
        let from = Pos2::new(sys.x as f32, sys.y as f32);
        for link in &sys.jump_links {
            if link.0 < id.0 {
                continue;
            }
            if let Some(other) = world.ledger().get(*link) {
                let to = Pos2::new(other.x as f32, other.y as f32);
                painter.line_segment(
                    [
                        camera.world_to_screen(from, rect_center),
                        camera.world_to_screen(to, rect_center),
                    ],
                    stroke,
                );
            }
        }
    }

    let radius = (6.0 * camera.zoom.sqrt()).clamp(4.0, 14.0);
    for (id, wpos, state) in &positions {
        let screen = camera.world_to_screen(*wpos, rect_center);
        if !rect.expand(20.0).contains(screen) {
            continue;
        }
        let mut color = map_state_color(*state);
        if empire_filter_soft && matches!(state, MapState::WildernessUnknown) {
            // Soft empire chrome: wilderness-unknown reads muted under empire viewpoint.
            color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90);
        }
        painter.circle_filled(screen, radius, color);
        if selected == Some(*id) {
            painter.circle_stroke(screen, radius + 3.0, Stroke::new(2.0_f32, Color32::WHITE));
        }
        painter.text(
            screen + Vec2::new(radius + 2.0, -radius),
            egui::Align2::LEFT_BOTTOM,
            format!("{id}"),
            egui::FontId::proportional(11.0),
            Color32::from_gray(200),
        );
    }

    // Legend
    let legend = [
        (MapState::Feed, "Feed"),
        (MapState::DryFuse, "Dry+Fuse"),
        (MapState::HomePaused, "HomePaused"),
        (MapState::Ended, "Ended"),
        (MapState::WildernessUnknown, "Wilderness"),
    ];
    let mut ly = rect.top() + 8.0;
    for (st, label) in legend {
        let c = map_state_color(st);
        painter.circle_filled(Pos2::new(rect.left() + 14.0, ly), 4.0, c);
        painter.text(
            Pos2::new(rect.left() + 24.0, ly),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            Color32::from_gray(180),
        );
        ly += 14.0;
    }

    let mut clicked = None;
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let world_click = camera.screen_to_world(pos, rect_center);
            let mut best: Option<(EntityId, f32)> = None;
            let thresh = (radius + 8.0) / camera.zoom;
            for (id, wpos, _) in &positions {
                let d = (*wpos - world_click).length();
                if d <= thresh {
                    if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                        best = Some((*id, d));
                    }
                }
            }
            clicked = best.map(|(id, _)| id);
        }
    }

    (response, clicked)
}
