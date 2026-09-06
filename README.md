# Helios Chronicle

A 2D, headless Aurora-class space-empire simulation. One species. Ships as module lists. Named research in incremental segments. Industry as chains, not sliders.

**Core loop:** systems stay stable while they hold catalog-binding matter; once that matter is gone, a surveyed extinction fuse starts. New systems and remnants keep throughput alive. Home/capital suns are politically anchored. Worlds have writable environment layers. Polities remember cruelty when they know about it. The operator may rewrite any field.

The simulation is the product. The operator is optional. The tick and the editor share **one ledger**.

See [STATEMENT.md](STATEMENT.md) for the full project statement, [LOCKS.md](LOCKS.md) for frozen issue resolutions, and [PHASES.md](PHASES.md) for the build order.

## Phase A — Kernel (current)

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
| Galaxy / sky hooks | Stubs only (Phase B) |

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
  globals.rs      # master era + editable ratios (Issue 5)
  lod.rs          # Coarse | Fine, quiet/hot hints (Issue 10)
  entity.rs       # SystemEntity stubs + EntityLedger
  event.rs        # typed EventKind + append-only EventLog
  world.rs        # seed/RNG, tick, outcome hash
  operator.rs     # inspect / set_field / arm_fuse
  save.rs         # JSON save_world / load_world
```

### Fuse + coarse LOD note

Fuses use an **absolute** `fuse_end_tick` in master-tick units. A coarse step of size `dt` ends the fuse iff `before < end && before+dt >= end`, so jumps cannot skip the end without firing (monotonic, exactly once). Remaining is derived as `end.saturating_sub(master_tick)`.
