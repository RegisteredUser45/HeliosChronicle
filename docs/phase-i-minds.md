# Phase I — Minds (schema stub / v2.1)

Status: **scoring wired against Phase B `sky::MapState` + Phase C `binding_remainder`**. Salt/punish emit remain behind feature flags (default off) until Lead clears real H knowledge paths. CoS owns push; this doc matches locked design decisions. Baseline tip lineage: `a60ce17` / `79fdd49`.

## Scope

Phase I owns **orders and doctrines**: empire operator-writable doctrine fields, Order entities on the one Kernel ledger, capital same-tick re-score on home-flag drop, and reserved salt/punish/atrocity order types gated by knowledge.

Out of scope / still gated:

- Salt/punish emit spam (needs H knowledge objects; Conflict owns typed cruelty events later).
- Parallel standing math (P owns standing; I **consumes** standing, does not invent a second model).
- Multiple minds per empire (v1: **one mind per empire**).

## Doctrine schema + defaults

Galaxy defaults live on `Globals` (editable). Per-empire fields live on `EmpireEntity` (operator-writable). All clamped to `[0.0, 1.0]`.

| Field | Default | Meaning (stub) |
|---|---|---|
| `salt_willingness` | `0.15` | Willingness to issue SaltWorld when gated |
| `punishment_willingness` | `0.55` | Willingness to PunishSalter / ProsecuteAtrocity |
| `evacuate_vs_die_in_place` | `0.6` | Bias toward Evacuate vs hold |

`EmpireEntity::from_defaults(id, &Globals)` copies galaxy defaults onto a new empire. Operator may mutate per-empire values via `set_empire_doctrine` / `set_field`.

## Order ledger shape

Orders are **entities on the ONE Kernel `EntityLedger`** — same id space as systems and empires. There is **no parallel Orders store**.

- `OrderIntent`: `ExpandSurvey`, `ClaimFeed`, `PlantCity`, `PlantYard`, `StripMine`, `Fortify`, `Evacuate`, `Abandon`, `ProsecuteAtrocity`, `SaltWorld`, `PunishSalter`
- `OrderStatus`: `Queued`, `Active`, `Done`, `Cancelled`
- `OrderSource`: `Ai`, `Operator`
- `OrderEntity`: `id`, `empire_id`, `intent`, `target_ref: Option<EntityId>` (system/body stub), `status`, `created_tick`, `updated_tick`, `source`

Chronicle events: `EmpireSpawned`, `OrderCreated`, `OrderStatusChanged`, `CapitalRescore`.

## Emit gates (salt / punish / atrocity)

1. **Feature flag** `MindsFlags.salt_emit_enabled` (default `false`) — until H lands, do not emit salt/punish spam. `try_emit_order` returns `Ok(None)` for SaltWorld / PunishSalter / ProsecuteAtrocity when the flag is off.
2. When the flag is on, `has_knowledge_path` checks real H/P objects:
   - Victim auto-knows own-home systems (`home_empire`) and KOs naming them as victim.
   - Witnesses must **carry** a KO with actor+system matching the order target.
3. **KO grades** rumor (0.25) → confirmed (1.0): both open the path; emit also requires doctrine×weight ≥ threshold (`SaltWorld` 0.15, Punish/Prosecute 0.20). Punish/Prosecute multiplies willingness by P standing hostility toward the KO actor (≤−40 → ×1.5, ≤−15 → ×1.25).
4. Empty knowledge or failed doctrine → **no order**. `salt_emit_enabled` stays **default false**.

`SaltWorld` maps to a typed cruelty event (reserved; Conflict owns event typing later).

## Capital same-tick re-score

On `HomeFlagClear` (operator or future B collapse path):

1. Append `HomeFlagClear`.
2. **Same tick**, call `World::handle_home_flag_clear(system)` → `on_home_flag_clear`:
   - Clear pause awareness (stub: home-flag clear is the signal today; B will expose a fuse-pause bit).
   - Append `CapitalRescore { system, empire }`.
   - Call `rescore_system` — **no-op** unless `MindsFlags.scoring_enabled`; when on, cancels Queued+Ai orders targeting the system and may emit a suggested non-salt Ai order.

Re-score runs **before further AI orders** in that tick once scoring is wired.


## LOD-aware minds tick (Issue 10)

Under `LodMode::Coarse`, `minds_tick_stub` only emits for empires that need attention (`empire_needs_minds_tick`: Hot-hint system, or known DryFuse / HomePaused / Ended). Fine LOD still evaluates every empire. ≤1 Ai order per empire per tick; salt family never from scoring.

## Scoring (B + C)

- Coarse bucket: `sky::map_state(sys)` only — no parallel map bits, no Dying variant.
- Feed: ledger `binding_remainder` (Phase C aggregates deposits via `matter::reaggregate_*`; I never maintains a second feed number).
- Fuse: ledger `fuse_*` is physics truth once armed; AI *known* fuse number gated on `surveyed` / fog `surveyed_fuse`.
- HomePaused: treated as stable until home-flag drop (same-tick `CapitalRescore`).
- WildernessUnknown: uncertain feed, not infinite.
- When `scoring_enabled`: cancel Queued+Ai orders on rescore target; emit ≤1 suggested non-salt Ai order per empire per tick.

## Feature flags

`World.minds_flags: MindsFlags` (default both false):

| Flag | Default | Effect |
|---|---|---|
| `scoring_enabled` | `false` | `rescore_system` / minds tick no-op; when on, scores known feed/fuse via B MapState |
| `salt_emit_enabled` | `false` | SaltWorld / PunishSalter / ProsecuteAtrocity not written |

## Dependence on A / B / H / P

| Phase | What I needs |
|---|---|
| **A** Kernel | Ledger, events, operator, tick — **done**; this stub extends them |
| **B** Sky | `sky::map_state` buckets (Feed / DryFuse / HomePaused / Ended / WildernessUnknown) — **wired** |
| **C** Matter | Ledger `binding_remainder` (C aggregates deposits; I **reads only**) — **wired** |
| **H** Violence | Knowledge objects + grades for salt/punish gates — **path wired**; emit still default-off via `salt_emit_enabled` |
| **P** Politics | Standing from typed events + KO paths; I consumes standing only |

## Lock compliance

- One ledger for tick + operator (no parallel order store).
- Operator may rewrite doctrine and issue orders; mutations are events.
- Home/capital suns politically anchored; home-flag drop triggers same-tick capital re-score.
- Polities remember cruelty when they **know** about it (KO gates; flags off until H).
- Same standing model for AI and hand — I does not invent parallel math.
- One mind per empire in v1.

## Stubbed vs done

**Done:** docs, doctrine defaults on Globals + EmpireEntity, Order entities on ledger, events, operator doctrine/order hooks, home-flag → CapitalRescore same tick, feature-flagged emit helpers, **scoring against B `MapState`**, **feed score from C `binding_remainder`** (matter drain updates score), same-tick capital rescore cancel+suggest, minds tick ≤1 Ai order/empire, tests.

**Stubbed / gated:** `salt_emit_enabled` default off (Lead: don’t spam salt until cleared), SaltWorld → typed cruelty event body (Conflict later). Knowledge path + willingness×grade thresholds are wired.
