//! World state: master clock, seed/RNG, ledger, chronicle, LOD.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::entity::{EntityId, EntityLedger};
use crate::event::{EventKind, EventLog};
use crate::globals::Globals;
use crate::lod::LodMode;

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
            outcome_hash: 0,
        };
        world.log.append(0, EventKind::WorldCreated { seed });

        // Seed surface: deterministic initial systems from RNG.
        let n = 2 + (world.rng_u64() % 3); // 2..4 systems
        for _ in 0..n {
            let id = world.ledger.spawn_system();
            // Vary binding remainder deterministically.
            let rem = 500.0 + (world.rng_u64() % 1000) as f64;
            let hot_roll = world.rng_u64() % 5;
            if let Some(sys) = world.ledger.get_mut(id) {
                sys.binding_remainder = rem;
                if hot_roll == 0 {
                    sys.lod_hint = crate::lod::LodHint::Hot;
                }
            }
            world
                .log
                .append(0, EventKind::Spawn { system: id });
        }

        world.recompute_outcome_hash();
        world
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

        // Quiet vs hot placeholder: under coarse LOD, skip fine work on quiet
        // systems; under fine, process everyone. Fuse uses absolute end ticks.
        let lod = self.lod;
        let mut fuse_ends: Vec<EntityId> = Vec::new();
        let mut fuse_ticks: Vec<(EntityId, u64)> = Vec::new();

        for (_, sys) in self.ledger.systems_mut() {
            let process = match lod {
                LodMode::Fine => true,
                LodMode::Coarse => matches!(sys.lod_hint, crate::lod::LodHint::Hot)
                    || sys.fuse_end_tick.is_some(),
            };

            if process {
                // Placeholder "work": nudge quiet binding by a tiny amount only when fine.
                if matches!(lod, LodMode::Fine) && !sys.depleted {
                    // no-op placeholder drain; Phase C owns real drain
                }
                if sys.fuse_crossed_end(from, dt) {
                    fuse_ends.push(sys.id);
                } else if let Some(end) = sys.fuse_end_tick {
                    if to < end {
                        let rem = end - to;
                        sys.fuse_remaining = Some(rem);
                        fuse_ticks.push((sys.id, rem));
                    }
                }
            } else {
                // Quiet + coarse: still sync remaining so operator views stay consistent.
                sys.sync_fuse_remaining(to);
            }
        }

        self.master_tick = to;

        for (id, rem) in fuse_ticks {
            self.log.append(
                to,
                EventKind::FuseTick {
                    system: id,
                    remaining: rem,
                },
            );
        }
        for id in fuse_ends {
            if let Some(sys) = self.ledger.get_mut(id) {
                sys.fuse_end_tick = None;
                sys.fuse_remaining = Some(0);
            }
            self.log.append(to, EventKind::FuseEnd { system: id });
        }

        self.log.append(
            to,
            EventKind::TickAdvanced {
                from,
                to,
                dt,
                lod: format!("{:?}", lod),
            },
        );
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
        self.log.len().hash(&mut h);
        self.outcome_hash = h.finish();
    }

    pub fn outcome_hash(&self) -> u64 {
        self.outcome_hash
    }
}
