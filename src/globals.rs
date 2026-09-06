//! Editable simulation globals (LOCKS Issue 5 — master era + ratios).

use serde::{Deserialize, Serialize};

use crate::cosmology::embedded_catalog;


/// Continuous [min, max] band for species envelope matching.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub min: f64,
    pub max: f64,
}

impl Default for Band {
    fn default() -> Self {
        Self { min: 0.0, max: 1.0 }
    }
}

/// One-sophont species envelope (Phase D). Continuous bands, not habitability grades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeciesEnvelope {
    pub pressure: Band,
    pub temperature: Band,
    pub gravity: Band,
    pub radiation: Band,
    /// Breathable mix stub (N2/O2 fraction sum target); operator-editable.
    pub breathable_o2_fraction: f64,
    pub calories_need: f64,
    pub water_need: f64,
    pub baseline_lifespan: f64,
    pub fertility: f64,
}

impl Default for SpeciesEnvelope {
    fn default() -> Self {
        Self {
            pressure: Band { min: 0.5, max: 1.5 },
            temperature: Band { min: 250.0, max: 320.0 },
            gravity: Band { min: 0.5, max: 1.5 },
            radiation: Band { min: 0.0, max: 1.0 },
            breathable_o2_fraction: 0.21,
            calories_need: 1.0,
            water_need: 1.0,
            baseline_lifespan: 80.0,
            fertility: 1.0,
        }
    }
}

/// One master era length; dry-time, fuse length, and research segment time
/// are editable ratios of that master length.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Globals {
    /// Master era length in master-tick units (the base clock).
    pub master_era_length: u64,
    /// Typical time-to-dry a worked system, as a ratio of master era.
    pub dry_time_ratio: f64,
    /// Countdown length after depletion, as a ratio of master era.
    pub fuse_length_ratio: f64,
    /// Time-to-complete a research segment, as a ratio of master era.
    pub research_segment_ratio: f64,
    /// Coarse LOD step size in master-tick units (Issue 10).
    pub coarse_dt: u64,

    // --- Phase B sky ---
    /// Binding remainder below this → B depletes and arms fuse.
    #[serde(default = "default_dry_threshold")]
    pub dry_threshold: f64,
    /// Binding floor for C (mirrors catalog default; operator-editable).
    #[serde(default = "default_binding_floor")]
    pub binding_floor: f64,
    /// Quantity drained per master-tick by civilian extractors (Lock 8).
    #[serde(default = "default_civilian_extract_rate")]
    pub civilian_extract_rate: f64,
    /// Quantity drained per master-tick by abandoned automation (Lock 8).
    #[serde(default = "default_abandoned_auto_extract_rate")]
    pub abandoned_auto_extract_rate: f64,
    /// Lock 4: capital home systems pause fuse while flag held.
    #[serde(default = "default_true")]
    pub home_pause_enabled: bool,
    /// Target count of live systems (incl. paused capitals; excl. ended).
    #[serde(default = "default_live_band")]
    pub live_system_band: u32,
    /// Operator-editable spawn weights over the fixed catalog spawn table.
    /// Empty → use embedded catalog `spawn_binding_table.entries`.
    #[serde(default)]
    pub spawn_weights: Vec<(String, f64)>,

    // --- Phase I doctrine galaxy defaults (operator-editable) ---
    /// Default salt willingness [0,1] (default 0.15).
    #[serde(default = "default_salt_willingness")]
    pub salt_willingness: f64,
    /// Default punishment willingness [0,1] (default 0.55).
    #[serde(default = "default_punishment_willingness")]
    pub punishment_willingness: f64,
    /// Default evacuate-vs-die-in-place bias [0,1] (default 0.6).
    #[serde(default = "default_evacuate_vs_die_in_place")]
    pub evacuate_vs_die_in_place: f64,

    // --- Phase D worlds ---
    /// Species envelope (continuous bands).
    #[serde(default)]
    pub envelope: SpeciesEnvelope,
}

fn default_dry_threshold() -> f64 {
    1.0
}
fn default_binding_floor() -> f64 {
    1.0
}

fn default_civilian_extract_rate() -> f64 {
    1.0
}

fn default_abandoned_auto_extract_rate() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_live_band() -> u32 {
    4
}
fn default_salt_willingness() -> f64 {
    0.15
}
fn default_punishment_willingness() -> f64 {
    0.55
}
fn default_evacuate_vs_die_in_place() -> f64 {
    0.6
}

impl Default for Globals {
    fn default() -> Self {
        let catalog = embedded_catalog();
        Self {
            master_era_length: 10_000,
            dry_time_ratio: 1.0,
            fuse_length_ratio: 0.25,
            research_segment_ratio: 0.1,
            coarse_dt: 10,
            dry_threshold: default_dry_threshold(),
            binding_floor: if catalog.binding_floor_default > 0.0 {
                catalog.binding_floor_default
            } else {
                default_binding_floor()
            },
            civilian_extract_rate: default_civilian_extract_rate(),
            abandoned_auto_extract_rate: default_abandoned_auto_extract_rate(),
            home_pause_enabled: true,
            live_system_band: default_live_band(),
            spawn_weights: catalog.spawn_weights(),
            salt_willingness: default_salt_willingness(),
            punishment_willingness: default_punishment_willingness(),
            evacuate_vs_die_in_place: default_evacuate_vs_die_in_place(),
            envelope: SpeciesEnvelope::default(),
        }
    }
}

impl Globals {
    pub fn dry_time_ticks(&self) -> u64 {
        ((self.master_era_length as f64) * self.dry_time_ratio).round() as u64
    }

    pub fn fuse_length_ticks(&self) -> u64 {
        ((self.master_era_length as f64) * self.fuse_length_ratio).round() as u64
    }

    pub fn research_segment_ticks(&self) -> u64 {
        ((self.master_era_length as f64) * self.research_segment_ratio).round() as u64
    }

    /// Effective spawn weight table (operator override or catalog default).
    pub fn effective_spawn_weights(&self) -> Vec<(String, f64)> {
        if self.spawn_weights.is_empty() {
            embedded_catalog().spawn_weights()
        } else {
            self.spawn_weights.clone()
        }
    }
}
