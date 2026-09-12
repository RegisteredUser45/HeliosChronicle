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

/// Whether a system should skip fine per-tick work this step (Lock 10).
///
/// Coarse + quiet + no armed fuse → skip. Fine world never skips. Fuse-armed
/// systems stay on the path so absolute `fuse_end_tick` can still fire.
pub fn skip_quiet_fine_work(world_lod: LodMode, hint: LodHint, fuse_armed: bool) -> bool {
    matches!(world_lod, LodMode::Coarse) && matches!(hint, LodHint::Quiet) && !fuse_armed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_skip_only_when_coarse_quiet_and_unarmed() {
        assert!(skip_quiet_fine_work(LodMode::Coarse, LodHint::Quiet, false));
        assert!(!skip_quiet_fine_work(LodMode::Coarse, LodHint::Quiet, true));
        assert!(!skip_quiet_fine_work(LodMode::Coarse, LodHint::Hot, false));
        assert!(!skip_quiet_fine_work(LodMode::Fine, LodHint::Quiet, false));
        assert!(!skip_quiet_fine_work(LodMode::Fine, LodHint::Hot, true));
    }

    #[test]
    fn hot_ttl_cools_only_when_expired() {
        assert_eq!(hot_until_tick(10, DEFAULT_HOT_TTL_TICKS), 42);
        assert_eq!(
            cool_hot_if_expired(41, Some(42), LodHint::Hot),
            LodHint::Hot
        );
        assert_eq!(
            cool_hot_if_expired(42, Some(42), LodHint::Hot),
            LodHint::Quiet
        );
        assert_eq!(cool_hot_if_expired(100, None, LodHint::Hot), LodHint::Hot);
        assert_eq!(
            cool_hot_if_expired(100, Some(1), LodHint::Quiet),
            LodHint::Quiet
        );
    }

    #[test]
    fn stamp_hot_sets_hint_and_until() {
        let mut hint = LodHint::Quiet;
        let mut until = None;
        stamp_hot(&mut hint, &mut until, 10, DEFAULT_HOT_TTL_TICKS);
        assert_eq!(hint, LodHint::Hot);
        assert_eq!(until, Some(42));
        stamp_hot(&mut hint, &mut until, 20, 5);
        assert_eq!(until, Some(25));
    }
}

/// Default Hot dwell in master ticks before cool-down (Lock 10).
pub const DEFAULT_HOT_TTL_TICKS: u64 = 32;

/// Absolute tick when a Hot mark should expire (`now + ttl`).
pub fn hot_until_tick(now: u64, ttl: u64) -> u64 {
    now.saturating_add(ttl.max(1))
}

/// Cool Hot → Quiet once `now` reaches `hot_until`.
///
/// `None` means no TTL (stay Hot) so existing promotions keep working
/// until a caller stamps an expiry.
pub fn cool_hot_if_expired(now: u64, hot_until: Option<u64>, hint: LodHint) -> LodHint {
    match (hint, hot_until) {
        (LodHint::Hot, Some(until)) if now >= until => LodHint::Quiet,
        (hint, _) => hint,
    }
}


/// Stamp a system Hot and set/refresh its TTL (Lock 10).
pub fn stamp_hot(hint: &mut LodHint, hot_until: &mut Option<u64>, now: u64, ttl: u64) {
    *hint = LodHint::Hot;
    *hot_until = Some(hot_until_tick(now, ttl));
}
