//! Helios Chronicle — Phase A kernel.
//!
//! Tick loop, deterministic seed, event log, save/load, entity ledger,
//! operator R/W, and stub LOD. Galaxy/sky hooks are stubs for Phase B.

pub mod entity;
pub mod event;
pub mod globals;
pub mod lod;
pub mod operator;
pub mod save;
pub mod world;

pub use entity::{EntityId, EntityLedger, SystemEntity};
pub use event::{ChronicleEvent, EventKind, EventLog};
pub use globals::Globals;
pub use lod::{LodHint, LodMode};
pub use operator::{Operator, OperatorError};
pub use save::{load_world, save_world, SaveError};
pub use world::World;

/// Run two worlds from the same seed for `ticks` and return whether outcomes match.
///
/// **Verify path:** `helios verify --seed S -n N` or call this helper / assert
/// `World::new(s).tick(n).outcome_hash()` equality across two instances.
pub fn verify_determinism(seed: u64, ticks: u64) -> (bool, u64, u64) {
    let mut a = World::new(seed);
    let mut b = World::new(seed);
    a.tick(ticks);
    b.tick(ticks);
    let ha = a.outcome_hash();
    let hb = b.outcome_hash();
    (ha == hb && a == b, ha, hb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lod::LodHint;

    #[test]
    fn tick_advances_master_time() {
        let mut w = World::new(1);
        assert_eq!(w.master_tick(), 0);
        w.tick(5);
        assert_eq!(w.master_tick(), 5);
        assert!(w
            .log()
            .events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::TickAdvanced { .. })));
    }

    #[test]
    fn same_seed_same_outcomes() {
        let (ok, ha, hb) = verify_determinism(42, 25);
        assert!(ok, "hashes diverged: {ha} vs {hb}");
        let mut a = World::new(1);
        let mut b = World::new(2);
        a.tick(10);
        b.tick(10);
        assert_ne!(a.outcome_hash(), b.outcome_hash());
    }

    #[test]
    fn event_log_is_append_only() {
        let mut w = World::new(7);
        let before = w.log().len();
        w.tick(3);
        assert!(w.log().len() > before);
        let seqs: Vec<_> = w.log().events().iter().map(|e| e.seq).collect();
        let mut sorted = seqs.clone();
        sorted.sort_unstable();
        assert_eq!(seqs, sorted);
        assert_eq!(seqs.first().copied().unwrap_or(0), 0);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("slot.json");

        let mut w = World::new(99);
        w.tick(8);
        let tick_before = w.master_tick();
        let systems_before = w.ledger().len();
        let draws_before = w.rng_draws;

        save_world(&mut w, &path).unwrap();

        // File contains post-save world; deserialize without the load-side event.
        let raw = std::fs::read_to_string(&path).unwrap();
        let restored: World = serde_json::from_str(&raw).unwrap();
        assert_eq!(restored.seed(), 99);
        assert_eq!(restored.master_tick(), tick_before);
        assert_eq!(restored.ledger().len(), systems_before);
        assert_eq!(restored.rng_draws, draws_before);
        assert_eq!(restored.ledger, w.ledger);
        assert_eq!(restored.globals, w.globals);

        // Continuing from two independent restores stays deterministic.
        let mut a: World = serde_json::from_str(&raw).unwrap();
        let mut b: World = serde_json::from_str(&raw).unwrap();
        a.recompute_outcome_hash();
        b.recompute_outcome_hash();
        a.tick(5);
        b.tick(5);
        assert_eq!(a.outcome_hash(), b.outcome_hash());
        assert_eq!(a, b);

        // load_world adds a WorldLoaded event but preserves ledger/tick.
        let loaded = load_world(&path).unwrap();
        assert_eq!(loaded.master_tick(), tick_before);
        assert_eq!(loaded.ledger, restored.ledger);
        assert!(loaded
            .log()
            .events()
            .iter()
            .any(|e| matches!(e.kind, EventKind::WorldLoaded { .. })));
    }

    #[test]
    fn operator_mutation_is_event() {
        let mut w = World::new(5);
        let id = *w.ledger().systems().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            op.set_field(id, "binding_remainder", "42.5").unwrap();
        }
        assert!(w.log().events().iter().any(|e| matches!(
            &e.kind,
            EventKind::OperatorMutation { field, new, .. }
                if field == "binding_remainder" && new == "42.5"
        )));
        assert!((w.ledger().get(id).unwrap().binding_remainder - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn shared_ledger_tick_and_operator() {
        let mut w = World::new(11);
        let id = *w.ledger().systems().next().unwrap().0;
        {
            let mut op = Operator::new(&mut w);
            op.set_field(id, "lod_hint", "hot").unwrap();
        }
        assert_eq!(w.ledger().get(id).unwrap().lod_hint, LodHint::Hot);
        w.tick(1);
        assert_eq!(w.ledger().get(id).unwrap().lod_hint, LodHint::Hot);
    }

    #[test]
    fn coarse_lod_fuse_end_is_monotonic() {
        let mut globals = Globals::default();
        globals.coarse_dt = 10;
        let mut w = World::with_globals(3, globals);
        w.set_lod(LodMode::Coarse);
        let id = w.ledger.spawn_system();
        // Arm fuse to end at tick 25; coarse steps 0→10→20→30 fire once at 30.
        {
            let mut op = Operator::new(&mut w);
            op.arm_fuse(id, 25).unwrap();
        }
        assert_eq!(w.master_tick(), 0);
        w.tick(2); // 0→10→20
        assert_eq!(w.master_tick(), 20);
        assert!(w.ledger().get(id).unwrap().fuse_end_tick.is_some());
        w.tick(1); // 20→30 crosses 25
        assert_eq!(w.master_tick(), 30);
        assert!(w.ledger().get(id).unwrap().fuse_end_tick.is_none());
        let ends = w
            .log()
            .events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::FuseEnd { system } if system == id))
            .count();
        assert_eq!(ends, 1, "fuse must end exactly once under coarse dt");
    }

    #[test]
    fn fine_vs_coarse_placeholder() {
        let mut w = World::new(8);
        assert_eq!(w.current_dt(), 1);
        w.set_lod(LodMode::Coarse);
        assert_eq!(w.current_dt(), w.globals().coarse_dt);
        let from = w.master_tick();
        w.tick(1);
        assert_eq!(w.master_tick(), from + w.globals().coarse_dt);
    }
}
