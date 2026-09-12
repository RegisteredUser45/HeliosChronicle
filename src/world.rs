//! World state: master clock, seed/RNG, ledger, chronicle, LOD, G/P stores.

use std::collections::BTreeMap;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::contact::EmpireContactStore;
use crate::entity::{EntityId, EntityLedger};
use crate::event::{EventKind, EventLog};
use crate::globals::Globals;
use crate::hulls::{ShipDesign, ShipInstance};
use crate::research::Lab;
use crate::knowledge::KnowledgeStore;
use crate::lod::LodMode;
use crate::matter;
use crate::sky;
use crate::minds::MindsFlags;
use crate::politics::StandingStore;

/// Serializable snapshot of the world (RNG reconstructed from seed + draws).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub seed: u64,
    /// How many RNG draws have been consumed (for restore determinism).
    pub rng_draws: u64,
    pub master_tick: u64,
    pub globals: Globals,
    pub lod: LodMode,
    pub ledger: EntityLedger,
    pub log: EventLog,
    /// Knowledge objects (Lock 9) — G/H/P.
    #[serde(default)]
    pub knowledge: KnowledgeStore,
    /// Per-empire fog + treaty stubs (Phase G).
    #[serde(default)]
    pub contact: EmpireContactStore,
    /// Directed standing a→b (Phase P).
    #[serde(default)]
    pub standing: StandingStore,
    /// Phase I minds feature flags (default both off).
    #[serde(default)]
    pub minds_flags: MindsFlags,
    /// Phase F design book (instances ref these).
    #[serde(default)]
    pub ship_designs: BTreeMap<EntityId, ShipDesign>,
    /// Phase F ship instances.
    #[serde(default)]
    pub ships: BTreeMap<EntityId, ShipInstance>,
    #[serde(default)]
    pub labs: BTreeMap<EntityId, Lab>,
    /// Phase J: empire currently possessed by the operator (`None` = none).
    #[serde(default)]
    pub possessed_empire: Option<EntityId>,
    /// Fingerprint of ledger + tick for cheap determinism checks.
    pub outcome_hash: u64,
}

impl World {
    /// Create a new world from seed. Spawns a couple of stub systems.
    pub fn new(seed: u64) -> Self {
        Self::with_globals(seed, Globals::default())
    }

    pub fn with_globals(seed: u64, globals: Globals) -> Self {
        let mut world = Self {
            seed,
            rng_draws: 0,
            master_tick: 0,
            globals,
            lod: LodMode::Fine,
            ledger: EntityLedger::new(),
            log: EventLog::new(),
            knowledge: KnowledgeStore::new(),
            contact: EmpireContactStore::new(),
            standing: StandingStore::new(),
            minds_flags: MindsFlags::default(),
            ship_designs: BTreeMap::new(),
            ships: BTreeMap::new(),
            labs: BTreeMap::new(),
            possessed_empire: None,
            outcome_hash: 0,
        };
        world.log.append(0, EventKind::WorldCreated { seed });

        // Seed one empire from galaxy doctrine defaults (Phase I).
        {
            let eid = world.ledger.spawn_empire(&world.globals, Some("seed".into()));
            world
                .log
                .append(0, EventKind::EmpireSpawned { empire: eid });
        }

        // Lock 6: catalog spawn (full binding table + deposits), not a rolled remainder.
        let n = 2 + (world.rng_u64() % 3); // 2..4 systems
        world.seed_initial_catalog_systems(n);

        world.recompute_outcome_hash();
        world
    }


    /// Lock 6: seed day-one systems via existing B/C catalog spawn APIs.
    ///
    /// Remainder comes from deposits (`seed_deposits_from_binding_stocks`),
    /// not a rolled orphan number.
    pub fn seed_initial_catalog_systems(&mut self, count: u64) {
        for _ in 0..count {
            let id = sky::spawn_system_with_catalog(self, false);
            let _body = self.ledger.spawn_body(id);
        }
    }

    /// Mark a system Hot and stamp `hot_until` from the default TTL.
    pub fn stamp_system_hot(&mut self, id: crate::entity::EntityId) -> bool {
        let now = self.master_tick;
        if let Some(sys) = self.ledger.get_mut(id) {
            crate::lod::stamp_hot(
                &mut sys.lod_hint,
                &mut sys.hot_until,
                now,
                crate::lod::DEFAULT_HOT_TTL_TICKS,
            );
            return true;
        }
        false
    }

    /// Issue 10: set world LOD from current per-system Hot/Quiet hints.
    pub fn reconcile_lod_from_hot_hints(&mut self) {
        self.lod = crate::lod::reconcile_lod_from_hot_hints(
            self.ledger.systems().map(|(_, sys)| sys.lod_hint),
        );
    }

    /// Lock 10: expire Hot hints whose `hot_until` has been reached.
    pub fn apply_hot_ttl_cooldown(&mut self) {
        let now = self.master_tick;
        for (_, sys) in self.ledger.systems_mut() {
            let next = crate::lod::cool_hot_if_expired(now, sys.hot_until, sys.lod_hint);
            if next != sys.lod_hint {
                sys.lod_hint = next;
                sys.hot_until = None;
            }
        }
    }

    pub fn master_tick(&self) -> u64 {
        self.master_tick
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn ledger(&self) -> &EntityLedger {
        &self.ledger
    }

    pub fn ledger_mut(&mut self) -> &mut EntityLedger {
        &mut self.ledger
    }

    pub fn log(&self) -> &EventLog {
        &self.log
    }

    pub fn log_mut(&mut self) -> &mut EventLog {
        &mut self.log
    }

    pub fn globals(&self) -> &Globals {
        &self.globals
    }

    pub fn globals_mut(&mut self) -> &mut Globals {
        &mut self.globals
    }

    pub fn lod(&self) -> LodMode {
        self.lod
    }

    pub fn set_lod(&mut self, mode: LodMode) {
        if self.lod != mode {
            let from = format!("{:?}", self.lod);
            let to = format!("{:?}", mode);
            self.lod = mode;
            self.log.append(
                self.master_tick,
                EventKind::LodSwitched { from, to },
            );
        }
    }

    /// Current tick dt from LOD mode.
    pub fn current_dt(&self) -> u64 {
        self.lod.dt(self.globals.coarse_dt)
    }

    /// Empire currently possessed by the operator, if any (Phase J).
    pub fn possessed_empire(&self) -> Option<EntityId> {
        self.possessed_empire
    }

    /// Whether `empire_id` is the currently possessed empire.
    pub fn is_possessed(&self, empire_id: EntityId) -> bool {
        self.possessed_empire == Some(empire_id)
    }

    /// Rebuild RNG at the current draw cursor (deterministic).
    fn rng_at_cursor(&self) -> StdRng {
        let mut rng = StdRng::seed_from_u64(self.seed);
        for _ in 0..self.rng_draws {
            let _: u64 = rng.gen();
        }
        rng
    }

    /// Draw next u64 from the seeded RNG (advances cursor).
    pub fn rng_u64(&mut self) -> u64 {
        let mut rng = self.rng_at_cursor();
        let v: u64 = rng.gen();
        self.rng_draws += 1;
        v
    }

    /// Run `n` LOD steps (each step advances master_tick by current_dt).
    pub fn tick(&mut self, n: u64) {
        for _ in 0..n {
            self.tick_once();
        }
    }

    fn tick_once(&mut self) {
        let dt = self.current_dt();
        let from = self.master_tick;
        let to = from.saturating_add(dt);

        // C: Lock-8 civilian + abandoned-auto drains.
        matter::tick_civilian_extractors(self, dt);
        matter::tick_abandoned_automation(self, dt);

        // B: slide absolute fuse_end_tick while capital home-paused (freeze countdown).
        sky::apply_fuse_pause_slide(self, dt);

        // Quiet vs hot placeholder: under coarse LOD, skip fine work on quiet
        // systems; under fine, process everyone. Fuse uses absolute end ticks.
        let lod = self.lod;
        let mut fuse_ends: Vec<EntityId> = Vec::new();
        let mut deplete_checks: Vec<EntityId> = Vec::new();

        for (_, sys) in self.ledger.systems_mut() {
            let process = !crate::lod::skip_quiet_fine_work(
                lod,
                sys.lod_hint,
                sys.fuse_end_tick.is_some(),
            );

            if process {
                // Placeholder "work": Phase C owns real drain; B reacts at threshold.
                if matches!(lod, LodMode::Fine) && !sys.depleted && !sys.ended {
                    deplete_checks.push(sys.id);
                }
                if sys.fuse_crossed_end(from, dt) {
                    fuse_ends.push(sys.id);
                } else if let Some(end) = sys.fuse_end_tick {
                    if to < end {
                        // Sync remaining for operator views; do not chronicle FuseTick
                        // every step (Issue 11 — notable moments only; FuseArmed/End stay).
                        sys.fuse_remaining = Some(end - to);
                    }
                }
            } else {
                // Quiet + coarse: still sync remaining so operator views stay consistent.
                sys.sync_fuse_remaining(to);
            }
        }

        self.master_tick = to;
        self.apply_hot_ttl_cooldown();

        // Issue 11 / lean: do not append FuseTick or TickAdvanced (tick spam).
        // EventKind variants remain for serde + old saves; FuseEnd stays notable.
        for id in fuse_ends {
            sky::apply_fuse_end(self, id);
            self.log.append(to, EventKind::FuseEnd { system: id });
        }

        for id in deplete_checks {
            sky::check_depletion(self, id);
        }

        sky::maintain_live_band(self);
        // Phase E: advance labs (segment RP; completions emit SegmentResearched).
        crate::research::tick_all_labs(self, dt);

        // Phase D: apply envelope deficit mortality to pops.
        {
            let env = self.globals.envelope.clone();
            let body_ids: Vec<_> = self.ledger.bodies().map(|(id, _)| *id).collect();
            for bid in body_ids {
                if let Some(body) = self.ledger.get_body_mut(bid) {
                    crate::worlds::apply_pop_deficits(body, &env, dt);
                }
            }
        }

        // Phase D: life-support drains organics+volatiles (pops>0).
        {
            let body_ids: Vec<_> = self.ledger.bodies().map(|(id, _)| *id).collect();
            for bid in body_ids {
                crate::worlds::apply_life_support_drain(self, bid, dt);
            }
        }

        // Phase D: facility unlocks soft-boost soaks on bodies (day-one: all empires).
        {
            let empire_ids: Vec<_> = self.ledger.empires().map(|(id, _)| *id).collect();
            let body_ids: Vec<_> = self.ledger.bodies().map(|(id, _)| *id).collect();
            for eid in empire_ids {
                let Some(empire) = self.ledger.get_empire(eid).cloned() else { continue; };
                for bid in &body_ids {
                    if let Some(body) = self.ledger.get_body_mut(*bid) {
                        crate::worlds::apply_facility_soaks(body, &empire);
                    }
                }
            }
        }

        // Phase I: score known feed/fuse (no-op unless scoring_enabled).
        crate::minds::minds_tick_stub(self);
        self.recompute_outcome_hash();
    }

    /// Cheap deterministic fingerprint of world outcomes (for verify path).
    pub fn recompute_outcome_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h = DefaultHasher::new();
        self.seed.hash(&mut h);
        self.master_tick.hash(&mut h);
        self.rng_draws.hash(&mut h);
        self.ledger.len().hash(&mut h);
        for (id, sys) in self.ledger.systems() {
            id.0.hash(&mut h);
            // bit-stable f64 via to_bits
            sys.binding_remainder.to_bits().hash(&mut h);
            sys.depleted.hash(&mut h);
            sys.fuse_end_tick.hash(&mut h);
            sys.is_home_capital.hash(&mut h);
            sys.home_flag.hash(&mut h);
        }
        self.ledger.bodies_len().hash(&mut h);
        for (id, body) in self.ledger.bodies() {
            id.0.hash(&mut h);
            body.system.0.hash(&mut h);
            body.pops.to_bits().hash(&mut h);
            body.automation_active.hash(&mut h);
            body.layers.atmosphere_pressure.to_bits().hash(&mut h);
            body.layers.temperature.to_bits().hash(&mut h);
            body.layers.radiation.to_bits().hash(&mut h);
            body.layers.toxins_fallout.to_bits().hash(&mut h);
            body.layers.biosphere.to_bits().hash(&mut h);
        }
        // Phase I ledger fingerprint (empires / orders).
        self.ledger.empires_len().hash(&mut h);
        for (id, emp) in self.ledger.empires() {
            id.0.hash(&mut h);
            emp.salt_willingness.to_bits().hash(&mut h);
            emp.punishment_willingness.to_bits().hash(&mut h);
            emp.evacuate_vs_die_in_place.to_bits().hash(&mut h);
            emp.unlocked_segments.len().hash(&mut h);
        }
        self.ledger.orders_len().hash(&mut h);
        for (id, ord) in self.ledger.orders() {
            id.0.hash(&mut h);
            ord.empire_id.0.hash(&mut h);
            ord.intent.as_str().hash(&mut h);
            ord.status.as_str().hash(&mut h);
            ord.created_tick.hash(&mut h);
        }
        self.globals.envelope.pressure.min.to_bits().hash(&mut h);
        self.globals.envelope.pressure.max.to_bits().hash(&mut h);
        self.log.len().hash(&mut h);
        self.log.dropped().hash(&mut h);
        // G/P fingerprint: KO count + standing + fog empire count
        self.knowledge.len().hash(&mut h);
        self.standing.fingerprint().hash(&mut h);
        self.contact.empire_count().hash(&mut h);
        self.ship_designs.len().hash(&mut h);
        self.ships.len().hash(&mut h);
        self.labs.len().hash(&mut h);
        self.outcome_hash = h.finish();
    }

    pub fn outcome_hash(&self) -> u64 {
        self.outcome_hash
    }

    /// Phase I: same-tick capital re-score hook after HomeFlagClear.
    pub fn handle_home_flag_clear(&mut self, system_id: crate::entity::EntityId) {
        crate::minds::on_home_flag_clear(self, system_id);
    }
}


#[cfg(test)]
mod lock6_seed_tests {
    use super::*;

    #[test]
    fn new_seeds_catalog_deposits_matching_remainder() {
        let w = World::new(7);
        let spawn_ids = crate::cosmology::embedded_catalog().spawn_stock_ids();
        let mut n = 0usize;
        for (_, sys) in w.ledger.systems() {
            n += 1;
            assert!(
                !sys.deposits.is_empty(),
                "Lock 6: seeded system {} must have catalog deposits",
                sys.id
            );
            for sid in &spawn_ids {
                assert!(
                    sys.binding_stocks.contains_key(*sid),
                    "Lock 6: missing spawn-table stock {sid} on {}",
                    sys.id
                );
            }
            let rem = matter::compute_binding_remainder(&sys.deposits, w.globals.binding_floor);
            assert!(
                (sys.binding_remainder - rem).abs() < 1e-9,
                "remainder must follow deposits, not a rolled orphan; got {} vs {rem}",
                sys.binding_remainder
            );
        }
        assert!(n >= 2 && n <= 4, "seed surface count {n}");
    }

    #[test]
    fn seed_initial_catalog_systems_adds_one() {
        let mut w = World::new(9);
        let before = w.ledger.systems().count();
        w.seed_initial_catalog_systems(1);
        assert_eq!(w.ledger.systems().count(), before + 1);
        let id = *w.ledger.systems().last().unwrap().0;
        let sys = w.ledger.get(id).unwrap();
        assert!(!sys.deposits.is_empty());
    }


    #[test]
    fn world_reconcile_lod_follows_hot_hints() {
        let mut w = World::new(11);
        w.minds_flags.scoring_enabled = false;
        for (_, sys) in w.ledger.systems_mut() {
            sys.lod_hint = crate::lod::LodHint::Quiet;
        }
        w.reconcile_lod_from_hot_hints();
        assert_eq!(w.lod, crate::lod::LodMode::Coarse);
        let id = *w.ledger.systems().next().unwrap().0;
        assert!(w.stamp_system_hot(id));
        w.reconcile_lod_from_hot_hints();
        assert_eq!(w.lod, crate::lod::LodMode::Fine);
    }
}
