//! Editable simulation globals (LOCKS Issue 5 — master era + ratios).

use serde::{Deserialize, Serialize};

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
}

impl Default for Globals {
    fn default() -> Self {
        Self {
            master_era_length: 10_000,
            dry_time_ratio: 1.0,
            fuse_length_ratio: 0.25,
            research_segment_ratio: 0.1,
            coarse_dt: 10,
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
}
