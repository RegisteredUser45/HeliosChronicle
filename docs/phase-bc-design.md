# Helios Galaxy — Phases B/C design notes (accepted)

Pinned: `4894b1ca5f1cfc33280b68314f1a8d1630ccf01f`
Law: STATEMENT / LOCKS / PHASES (locks override). Status: design notes only until Kernel Phase A cleared by Helios Lead.
Lead answers locked 2026-09-06. Do not reopen locks; flag conflicts only.

---

## Scope

**B — Sky:** orbits, transfers, jumps; depletion arms fuse; end + remnants; spawn with full binding table; map ledger of feed / dry+fuse / ended / wilderness-unknown / Home-paused.

**C — Matter:** deposits → chains; C7 binding remainder; civilians; salvage. All extraction drains (Lock 8).

---

## Lock map (B/C) — accepted

| Lock | Effect on B/C |
|------|----------------|
| 1 Binding = cosmology catalog | Binding remainder uses fixed catalog test, not price/AI interest |
| 2 Fuse = physics | Depletion + fuse run in sky physics; tech may later pause/cancel fuse only |
| 3 Wilderness immortal | No drain/fuse until surveyed OR claimed |
| 4 Capital-only Home pause | Only capital systems pause fuse while home flag held; paused capitals count in live-system band; re-flag OK; non-capitals never get pause |
| 5 Master era | Dry-time, fuse length, segment time = editable ratios of one master era |
| 6 Spawn full binding table | Day-one spawn rolls entire cosmology binding table (not research-gated) |
| 7 No sick appearance | After depletion: fuse timer → end/explode only. No sick/dying *appearance* stages |
| 8 All extraction drains | State, civilian, foreign, abandoned automation all decrement binding_remainder |
| 10 LOD early | Quiet systems coarse-tick; hot fine — B motion/fuse must survive coarse ticks |

---

## Resolved decisions (Lead 2026-09-06)

1. **Fuse fog:** B ledger always holds true `fuse_ticks_remaining` once armed (physics, Issue 2). Clock exists and counts down in sim. Empire knowledge of the number = survey product (G/I fog). Never hide the physics clock itself.

2. **“Dying”:** = dry+fuse ledger state, no visuals. Lock 7 bans appearance stages, not the ledger name. Prefer schema names `dry+fuse` / `ended`; “dying” only as PHASES synonym.

3. **Wilderness:** Claim alone ends immortality (surveyed OR claimed). No deposit survey required to start normal depletion/fuse after claim.

4. **Remainder authority:** C owns deposits + `binding_remainder` aggregation; B owns depleted/fuse/home/wilderness transitions; dry-threshold crossing is a B transition reacting to C’s remainder update. All Lock-8 drains flow through C into that remainder.

5. **Remnant:** Minimal last-harvest stub in B at fuse end. C deepens salvage types later; don’t block B on full salvage taxonomy.

6. **Kernel A needs (acceptance):**
   - Entity ids, tick dt / LOD hooks, save fields, operator mutate API
   - Lock-5 master era length + editable dry/fuse/research ratios as globals
   - Deterministic seed surface for spawn
   - System entity schema hooks for B fields below
   - Event types: `deplete` / `fuse_armed` / `fuse_tick` / `fuse_end` / `spawn` / `home_flag_set` | `home_flag_clear`
   - **Hard acceptance test:** coarse dt must still fire fuse ends monotonically

7. **Capital:** Exactly one home/capital flag per empire. Migration OK without conquest: abandon/clear old flag, then set new capital (political move). Conquest/successor re-flag also OK. Non-capitals never pause. Paused capitals count in live band.

8. **Spawn:** Full cosmology binding table from day one (Issue 6), never unlock-gated subsets. Weights / live-band / recipe table = operator settings over that fixed catalog.

---

## Phase B — Sky

### Spatial model
- 2D systems; bodies on orbits for transfer time, patrol radius, convoy ETA.
- Jump links exist; hidden until surveyed. B owns link existence + travel cost once known.
- Time scale via Lock-5 master era.

### System ledger fields (operator-visible; one ledger)
- `binding_remainder` — read from C aggregation (C writes)
- `depleted: bool`
- `fuse_ticks_remaining: Option<u64>` — armed only after depleted; always true physics once armed
- `home_flag: Option<EmpireId>` — at most one capital system per empire
- `fuse_paused: bool` — capital + home flag held + Home-pause setting on
- `wilderness: bool` — immortal until surveyed OR claimed
- `ended: bool` + remnant last-harvest stub when fuse hits 0
- Map presentation states: **feed | dry+fuse | ended | wilderness-unknown | Home-paused**

### Depletion → fuse (physics)
1. While not depleted and not wilderness-immortal: C extraction drains remainder.
2. Cross dry threshold → B sets `depleted`, arms fuse from Lock-5 fuse-length ratio × master era; emit `deplete` + `fuse_armed`.
3. Each tick (LOD-safe): if depleted and not fuse_paused, decrement fuse; emit `fuse_tick` as needed.
4. Fuse hits 0 → end system; minimal remnant harvest stub; emit `fuse_end`.
5. Stabilizer tech (later): pause/cancel fuse only — out of B v1 unless stub flag requested.

### Home pause (Lock 4 + Lead Q7)
- Capital systems only; one capital per empire.
- Migration: clear old flag → set new (political); conquest/successor re-flag OK.
- Depletion still occurs under pause; paused capitals in live-system band.
- Glassing ≠ kill star (D owns layers).

### Wilderness (Lock 3 + Lead Q3)
- Unsurveyed and unclaimed: immortal.
- Survey **or** claim → normal rules; claim alone is enough.

### Spawn (Locks 4, 6 + Lead Q8)
- Full cosmology binding table day one; weights/live-band/recipe = operator settings.
- Deterministic seed surface from Kernel A.
- Emit `spawn` events.

### Motion + LOD
- Orbit → ETA/transfer cost; jump pay + arrive.
- **Hard test:** coarse dt still fires fuse ends monotonically under LOD switches.

---

## Phase C — Matter

### Deposits + C7
- Deposit: stock id, quantity, accessibility.
- Binding (Lock 1): in fixed cosmology catalog AND extractable × accessibility ≥ floor.
- C aggregates `binding_remainder`; non-binding never stabilizes the sun.

### Drains (Lock 8)
State mines, civilian lines, foreign strip-mines, abandoned automation — all through C into remainder.

### Civilians / salvage
- Civilian lines first-class drainers.
- Abandoned automation still running → still drains.
- Salvage as feed-pipe stub; taxonomy deepens later (don’t block B remnant stub).

### Interface
C writes deposits + remainder → B reacts at dry threshold.

---

## Out of lane
D envelope/evac · E research · F hulls · G fog · H/P knowledge/standing · I minds · J injectors. LOD policy shared with Kernel A.

---

## Standby
No implementation until Helios Lead clears Kernel A. Fuller draft → repo notes when write path unblocks. Flag conflicts; don’t reopen locks.
