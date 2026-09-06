# Helios Empire — Phases D/E/F design notes

Against: STATEMENT / LOCKS / PHASES @ 4894b1ca5f1cfc33280b68314f1a8d1630ccf01f  
Status: **accepted by Helios Lead** (2026-09-06). Design only — no implementation until Lead clears Kernel Phase A scaffold.  
Paste into repo docs once write path is up.  
Locks override STATEMENT where they conflict.

---

## Shared assumptions (from locks + statement + Lead answers)

- One ledger; operator R/W any field; every mutation is an event.
- Binding = fixed cosmology catalog test (recipes / fuel / rares + extractable×accessibility floor). Spot price never unbinds. (L1, L2)
- All extraction drains binding remainder, including abandoned automation still running. (L8)
- Research segment time is an editable ratio of one master era length. (L5)
- No sick/dying appearance stages for stars. (L7)
- Simulation LOD mandatory early; cut anything that breaks long headless runs. (L10)
- Spawn always includes full cosmology binding table from day one. (L6)

### Lead resolutions (frozen for Empire)

1. **Fuse-end** — B owns explode/end: mark system ended + remnant stub (Galaxy stubbed). Optional same-event layer burst writes into D’s five columns (reuse path H will use for weapons). D does **not** decide system delete vs sterilize. Prefer **end+remnant** as default; layer burst is an effect write, not a substitute for ending.
2. **Cosmology catalog** — shared frozen id space early (**required**). **C owns** cosmology/content file (stocks, deposits, recipes, binding rows, fuel tiers as matter). **E only refs** catalog ids; empire unlock flags live on empire/progress records, **not** mutations of cosmology rows. **F module/fuel ids** live in that same catalog; **F owns design schema**, **C owns matter rows**.
3. **Prototypes / fuel** — cut prototype failure RNG in v1 (LOD/L10). **Single required `fuel_tier` per design** in v1; wrong tier ⇒ no move (F gate, B motion).
4. **Civilian life support** — same envelope/deficit bills whenever pops are present (state or civilian). Abandoned automation at pops=0: **no** life-support bill; still drains binding via C (L8) when `automation_active`.
5. **Deficit model** — **continuous bands** vs envelope globals (pressure/temp/…), not discrete habitability grades. Structure/power/upkeep soak first; overflow → mortality/fertility/labor.

**Industry split:** D/E/F = consumers/schema; C = producers/deposits. Freeze shared catalog ids before leaving design-notes.

Dependencies we wait on:
- A: entity ledger, tick, log, save, operator R/W
- B: bodies in systems, Home/capital flag + fuse pause (capitals only — L4), fuse-end + remnant
- C: deposits, recipes/chains, C7 binding_remainder, civilians, salvage stock, cosmology catalog file

---

## Phase D — Worlds

### Contract (PHASES)
Envelope, deficits, drains, evac, automation on ash.

### Envelope
- One sophont type. Editable globals (not per-pop genes): pressure band, temperature band, gravity, radiation, breathable mix, calories, water, baseline lifespan, fertility.
- Colony = pops + infrastructure on a body.
- No gene track. Nukes/fallout change headcount/growth, not species definition.

### Environment layers (five body-level writable columns)
1. `atmosphere_pressure`
2. `temperature`
3. `radiation`
4. `toxins_fallout`
5. `biosphere`

Same columns receive optional fuse-end layer bursts (B event → D write path) and later weapon writes (H). D owns columns + colony response; does not own system end.

### Deficits & drains
- **Continuous** mismatch between layer state and envelope bands (plus calorie/water shortfall as life-support drains).
- Structure/power/upkeep facilities soak deficit capacity first; overflow → mortality, fertility, labor.
- Same bills for state and civilian whenever pops > 0.
- Extraction/binding drains are C’s; D owns life-support / remediation drains. Remediation slower and more expensive than break.

### Evac
- Evac moves pops (and optionally portable modules) off a body onto ships/stations with free berth + life support.
- Triggers: operator order, AI doctrine later (I), or optional auto-evac when mortality crosses editable threshold (default off until I).
- Evac does not clear claim; uninhabitable ≠ unclaimable ≠ unmineable.

### Automation on ash
- Automation research segments (E) gate whether a tomb still yields ore with zero living pops.
- D exposes `automation_active` / throughput on body; **C applies the binding drain** (L8).
- At pops=0: no life-support bill; automation may still run and drain.
- Tomb colony: pops=0, infra may remain, layers may be lethal; claim and mine flags independent.

### Out of D
- Standing / witnesses (P, H).
- Capital fuse pause is B/L4; D only cares Home bodies can be dry-imported-fed.

---

## Phase E — Research

### Contract
Lines, segments, labs, prototypes, salvage jumps.

### Schema sketch
- **TechLine**: id, name, category (hull | industry | population | automation | …), ordered segment ids.
- **Segment**: id, line_id, index, display_name, rp_cost (or rp_ratio_of_master_era — L5), material_gates[] (catalog ids), unlocks[] (ComponentDesign | Recipe | Facility | AutomationTier | StatMod), salvage_skip_allowed.
- **Lab**: capacity (RP/tick or RP/era-tick under LOD), assigned queue.
- Flow: segment researched → component designed → yard tooled → instance built.
- **No final segment** on a line.
- **Salvage jump**: unlock with incomplete stats (`stats_incomplete`); later research/reverse-eng fills stats.
- Empire unlock flags on empire/progress records — never mutate cosmology rows.

### Population / medicine / habitat
Same machine as other lines — more lines, not a parallel system.

### Material gates
Products appear when a segment needs them. Gate refs cosmology catalog ids (C-owned file). Late antimatter segment ⇒ antimatter recipe already in catalog.

### Prototypes
**Cut** prototype failure RNG in v1. Keep designed → tooled → buildable only.

### Clocks
Segment duration = `master_era_length * research_segment_ratio`. Labs parallelize by capacity.

### Soft prereqs
Cross-line prerequisites via material gates only in v1 (keep graph simple).

---

## Phase F — Hulls

### Contract
Schema through god fields and fuel-tier motion.

### Design vs instance
- **ShipDesign**: ordered module list, derived mass/crew_req/`fuel_tier`/magazine_types/power/maint_load. **Exactly one required `fuel_tier`** per design.
- **ShipInstance**: design_id, damage fields, magazines, fuel_qty + fuel_tier, crew, maintenance, operator god fields (incl. planetary strike flags / sidearm caliber).

### Modules
- Each module declares BOM (build) + operating consumable (incl. fuel burn class). Module/fuel ids live in shared cosmology catalog (C matter rows); F owns design schema.
- Wrong fuel_tier on instance ⇒ **no move** (F gate check; B motion).

### Fuel tiers
Discrete tiers in cosmology catalog. Single required tier per design; no mixing in v1.

### Layout
Free list + soft constraints (power, crew, hardpoints) over fixed slots for v1.

### God fields
Operator-writable: damage, planetary flags / strike weapons. Combat (H) reads them. Log records edit (A). F defines fields; H/J consume.

### Yard tooling
C facility “yard” with tooling flags unlocked by E segments that reference F designs.

---

## Industry chains (lane overlap with C)

- Short raw list; products appear when segments need them.
- Component = BOM + operating consumable.
- Rares = small natural veins or gated refineries on recipes.
- Freight = tonnes + time; wealth, labor, energy, maint supplies = constraints.
- **Consumers/schema (D/E/F)** vs **producers/deposits (C)**. Shared frozen catalog ids before implementation.

---

## Slice alignment

| Slice | Empire contribution |
|-------|---------------------|
| 2 People + fuel | D envelope/deficits; F fuel tier; chains for fuel (w/ C) |
| 3 Segment → ship that needs that fuel | E segment unlocks module+fuel; F design/instance; C tooling/build |
| 5 War, salt… | D layer columns for H to write; F god/planetary fields |

---

## Parked

Waiting on Helios Lead clear of Kernel A scaffold. No code, no PR, no cloud agent until cleared. Catalog id freeze with C before leaving design-notes phase.

---

## Frozen cosmology catalog v4 (Galaxy-owned, Lead-locked)

Path: `data/cosmology_catalog.json` + `src/cosmology.rs`. **FREEZE: dotted `kind.name`**. Binding = row boolean. Empire refs only; unlock flags on empire/progress.

**Fuels:** `fuel.chemical` · `fuel.fission` · `fuel.fusion` · `fuel.antimatter`

**Stocks:** `stock.ore_common` (non-binding) · `stock.ore_binding` · `stock.volatiles` · `stock.silicates` · `stock.fissiles` · `stock.organics` · `stock.rare_earth` · `stock.antimatter_precursor`

**Rare:** `rare.catalyst`

**Recipes:** `recipe.fuel_chem_refine` · `recipe.fuel_fission_pellet` · `recipe.fuel_fusion_pellet` · `recipe.antimatter_synth` · `recipe.hull_plate` · `recipe.power_core_basic` · `recipe.yard_mk1` · `recipe.habitat_seal_mk1` · `recipe.mine_auto_mk1` · `recipe.tankage_mk1`

**Modules:** `module.engine_chem` · `module.engine_fission` · `module.engine_fusion` · `module.engine_antimatter` · `module.tankage` · `module.crew_habitat` · `module.cargo_hold` · `module.sensor_basic` · `module.weapon_kinetic`

**Facilities:** `facility.lab` · `facility.yard` · `facility.habitat_seal` · `facility.mine_auto`

**D life-support v1:** drains `stock.organics` + `stock.volatiles` (no separate supply.* ids in v4).

**E material_gates:** prefer recipe.* / facility.* / stock.* ids. Option (a) locked: recipe.yard_mk1 etc. BOM into facility.* / module.tankage.

## Implementation status

- D: `src/worlds.rs` + ledger bodies + `Globals.envelope` (on main via CoS).
- E: `src/research.rs` stub tech book; unlocks on `EmpireEntity` progress fields.
- F: `src/hulls.rs` `ShipDesign`/`ShipInstance`; single `fuel_tier`; `can_move` gate; stores on `World`.
