# Helios Chronicle

A 2D, headless Aurora-class space-empire simulation. One species. Ships as module lists. Named research in incremental segments. Industry as chains, not sliders.

**Core loop:** systems stay stable while they hold catalog-binding matter; once that matter is gone, a surveyed extinction fuse starts. New systems and remnants keep throughput alive. Home/capital suns are politically anchored. Worlds have writable environment layers. Polities remember cruelty when they know about it. The operator may rewrite any field.

The simulation is the product. The operator is optional. The tick and the editor share **one ledger**.

Design notes: [docs/phase-bc-design.md](docs/phase-bc-design.md). See [STATEMENT.md](STATEMENT.md) for the full project statement, [LOCKS.md](LOCKS.md) for frozen issue resolutions, and [PHASES.md](PHASES.md) for the build order.

## Phase A — Kernel (landed)

Rust / Cargo library (`helios_chronicle`) + thin binary (`helios`).

| Capability | Status |
|---|---|
| Tick loop / master era | Done |
| Deterministic seed | Done |
| Append-only event log | Done |
| Save / load (JSON) | Done |
| Entity ledger (shared) | Done |
| Operator R/W hooks | Done |
| Stub LOD (Coarse \| Fine) | Done |
| Galaxy / sky hooks | Phase B headless slice (deplete/fuse/home/wilderness/spawn) |
| Cosmology catalog v4 | Frozen dotted ids (`stock.ore_binding`, …) |

## Phase J — Operator (stub slice)

| Capability | Status |
|---|---|
| Possess / release empire | Stub — `Operator::possess` / `release`; events + save field |
| AI skip while possessed | Done — `minds_tick_stub` skips possessed empire |
| Injectors (force dry/pause/nova) | Not yet (rumor inject exists via knowledge) |
| Chronicle / History | EventLog moments only (Issue 11) — no full replay |
| Headless `--possess <id>` | Done |

## Phase U — Operator shell (milestone 1)

Instrument panel over the live ledger (STATEMENT §11). Map never goes away; pan/zoom; time pause/step; System inspector; EventLog chronicle strip (not a replay viewer).

```bash
cargo run --features ui --bin helios -- ui --seed 42
```

Requires a display. Feature `ui` is **off by default** (keeps headless `cargo test --lib` free of egui). Enable with `--features ui`. Package `rust-version = "1.85"` + resolver fallback pins egui/eframe 0.28 so the window builds on rustc 1.85.

## Phase I — Minds (schema stub)

Doctrine fields + Order entities on the **same Kernel ledger**. Scoring and salt/punish emit are **feature-flagged off** until B map bits and H knowledge objects land.

| Capability | Status |
|---|---|
| Empire doctrine (salt / punish / evacuate bias) | Schema + defaults + operator mutate |
| Order entities on ledger | Schema + create via operator / `try_emit_order` |
| SaltWorld / PunishSalter / ProsecuteAtrocity | Reserved; emit blocked by default flags + KO stub |
| Capital same-tick re-score on HomeFlagClear | Hook + `CapitalRescore` event; `rescore_system` no-op |
| AI Expand/Plant scoring | Flagged off (`scoring_enabled`) |

Design notes: [docs/phase-i-minds.md](docs/phase-i-minds.md).


## Phase G/H/P

Contact (G) and Politics (P) stubs: knowledge objects, fog/first_contact, directed standing. Notes: [docs/GHP-design-notes.md](docs/GHP-design-notes.md). **H** layer-write path live (`violence::strike_layers` / `salt_world`); hull damage via ShipInstance.damage (F); sensor stubs (`sense_system` / `detect_strike` / `receive_signal` / `discover_wreck`).

### Build

```bash
cd /workspace/HeliosChronicle   # or your clone root
cargo build
```

### Test

```bash
cargo test
```

### Headless demo

Run 20 fine ticks from seed 42, demo an operator write, save:

```bash
cargo run -- run -n 20 --seed 42 --demo-operator --save saves/demo.json
```

Coarse LOD run:

```bash
cargo run -- run -n 10 --seed 42 --lod coarse
```

Load and continue:

```bash
cargo run -- run --load saves/demo.json -n 5 --save saves/demo2.json
```

### Verify determinism

Same seed must yield the same `outcome_hash` and world state:

```bash
cargo run -- verify --seed 42 -n 50
```

Or in tests: `same_seed_same_outcomes` / `helios_chronicle::verify_determinism(seed, ticks)`.

**Verify path:** construct two `World::new(seed)`, call `.tick(n)` on each, compare `outcome_hash()` (and full `PartialEq`). The CLI `verify` subcommand exits `0` on match, `1` on divergence.

### Crate layout

```
src/
  lib.rs          # public API + unit tests
  bin/helios.rs   # headless CLI
  globals.rs      # master era + editable ratios + doctrine galaxy defaults
  lod.rs          # Coarse | Fine, quiet/hot hints (Issue 10)
  entity.rs       # System / Empire / Order entities + EntityLedger
  event.rs        # typed EventKind + append-only EventLog
  world.rs        # seed/RNG, tick, outcome hash, minds flags
  minds.rs        # Phase I doctrine helpers, try_emit_order, capital rescore
  operator.rs     # inspect / set_field / arm_fuse / doctrine / issue_order
  save.rs         # JSON save_world / load_world
docs/
  phase-i-minds.md
```

### Fuse + coarse LOD note

Fuses use an **absolute** `fuse_end_tick` in master-tick units. A coarse step of size `dt` ends the fuse iff `before < end && before+dt >= end`, so jumps cannot skip the end without firing (monotonic, exactly once). Remaining is derived as `end.saturating_sub(master_tick)`.

## Phase D stub (Empire)

See [docs/DEF-design-notes.md](docs/DEF-design-notes.md). Ledger bodies expose five env layers + `automation_active`; species envelope is on `Globals`. Catalog ids: dotted v4 in `data/cosmology_catalog.json`.
