//! Operator tag colors (STATEMENT §11) — chrome only; not standing/combat/knowledge.
//!
//! Stored in HeliosApp UI state (`BTreeMap`), not the World ledger.

use std::collections::BTreeMap;

use egui::{Color32, Ui};

use crate::entity::EntityId;

/// Kind of entity a display color can tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TagKind {
    Empire,
    Fleet,
    Body,
}

/// UI-side color overrides keyed by (kind, entity id).
#[derive(Debug, Clone, Default)]
pub struct TagColors {
    overrides: BTreeMap<(TagKind, u64), Color32>,
}

impl TagColors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default wheel color from a stable hash of the id (empire defaults).
    pub fn hash_color(id: EntityId) -> Color32 {
        hash_color_u64(id.0)
    }

    pub fn get(&self, kind: TagKind, id: EntityId) -> Option<Color32> {
        self.overrides.get(&(kind, id.0)).copied()
    }

    pub fn set(&mut self, kind: TagKind, id: EntityId, color: Color32) {
        self.overrides.insert((kind, id.0), color);
    }

    pub fn clear(&mut self, kind: TagKind, id: EntityId) {
        self.overrides.remove(&(kind, id.0));
    }

    /// Override if set, else hash default (suitable for empires).
    pub fn resolve(&self, kind: TagKind, id: EntityId) -> Color32 {
        self.get(kind, id).unwrap_or_else(|| Self::hash_color(id))
    }

}

fn hash_color_u64(id: u64) -> Color32 {
    // Simple mixed hash → pleasant saturated RGB (avoid near-black / near-white).
    let mut x = id.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
    x ^= x >> 30;
    let r = 70 + ((x & 0xFF) as u8 % 160);
    let g = 70 + (((x >> 8) & 0xFF) as u8 % 160);
    let b = 70 + (((x >> 16) & 0xFF) as u8 % 160);
    Color32::from_rgb(r, g, b)
}

/// Color-edit button (opaque wheel) writing into `color`.
pub fn color_wheel_button(ui: &mut Ui, color: &mut Color32) -> egui::Response {
    egui::color_picker::color_edit_button_srgba(ui, color, egui::color_picker::Alpha::Opaque)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_color_stable_and_nontrivial() {
        let a = TagColors::hash_color(EntityId(1));
        let b = TagColors::hash_color(EntityId(1));
        let c = TagColors::hash_color(EntityId(2));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn override_beats_hash() {
        let mut tags = TagColors::new();
        let id = EntityId(42);
        let custom = Color32::from_rgb(255, 0, 128);
        assert_eq!(tags.resolve(TagKind::Empire, id), TagColors::hash_color(id));
        tags.set(TagKind::Empire, id, custom);
        assert_eq!(tags.resolve(TagKind::Empire, id), custom);
        tags.clear(TagKind::Empire, id);
        assert_eq!(tags.resolve(TagKind::Empire, id), TagColors::hash_color(id));
    }
}
