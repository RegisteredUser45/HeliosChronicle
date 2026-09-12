//! Named map waypoints (STATEMENT §11) — operator pins on the glass.
//!
//! Persist in HeliosApp across viewpoint swaps. Not binding stock, not fleets.

use egui::Color32;

use super::tags::TagColors;

/// Operator-dropped pin on the galaxy / jump-graph map.
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub id: u64,
    /// World-space x (same coordinates as system.x).
    pub x: f32,
    /// World-space y (same coordinates as system.y).
    pub y: f32,
    pub name: String,
    pub color: Color32,
    pub note: String,
}

impl Waypoint {
    pub fn new(id: u64, x: f32, y: f32, name: impl Into<String>, color: Color32) -> Self {
        Self {
            id,
            x,
            y,
            name: name.into(),
            color,
            note: String::new(),
        }
    }
}

/// UI-side waypoint roster (survives viewpoint Operator ↔ Empire swaps).
#[derive(Debug, Clone, Default)]
pub struct WaypointStore {
    next_id: u64,
    items: Vec<Waypoint>,
}

impl WaypointStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[Waypoint] {
        &self.items
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Waypoint> {
        self.items.iter_mut().find(|w| w.id == id)
    }

    pub fn add(&mut self, x: f32, y: f32, name: impl Into<String>, color: Color32) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.items.push(Waypoint::new(id, x, y, name, color));
        id
    }

    /// Drop a waypoint with a default name and hash-derived color.
    pub fn add_at(&mut self, x: f32, y: f32) -> u64 {
        let id = self.next_id;
        let color = TagColors::hash_color(crate::entity::EntityId(id.wrapping_add(0xA11)));
        let name = format!("Waypoint {id}");
        self.add(x, y, name, color)
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|w| w.id != id);
        self.items.len() < before
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_edit_remove() {
        let mut store = WaypointStore::new();
        let id = store.add_at(10.0, -20.0);
        assert_eq!(store.len(), 1);
        {
            let w = store.get_mut(id).unwrap();
            w.name = "Chokepoint".into();
            w.note = "watch fuse".into();
            assert!((w.x - 10.0).abs() < f32::EPSILON);
        }
        assert!(store.remove(id));
        assert!(store.is_empty());
    }
}
