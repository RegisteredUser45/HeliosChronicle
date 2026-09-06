//! Simulation LOD (LOCKS Issue 10) — coarse for quiet / fine for hot.

use serde::{Deserialize, Serialize};

/// World or per-system LOD mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LodMode {
    /// Fine ticks (dt = 1). Use for hot systems.
    #[default]
    Fine,
    /// Coarse ticks (dt = globals.coarse_dt). Use for quiet systems.
    Coarse,
}

/// Per-system hint for future sky/LOD schedulers (Phase B+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LodHint {
    #[default]
    Quiet,
    Hot,
}

impl LodMode {
    /// Resolve the master-tick delta for this mode.
    ///
    /// Fuse countdowns are stored as absolute end ticks (or remaining in
    /// master-era units advanced with `min(dt, remaining)`), so a coarse
    /// jump cannot skip past an end incorrectly when B wires real fuses.
    pub fn dt(self, coarse_dt: u64) -> u64 {
        match self {
            LodMode::Fine => 1,
            LodMode::Coarse => coarse_dt.max(1),
        }
    }
}
