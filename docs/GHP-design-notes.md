# Helios Conflict — Phases G / H / P design notes
Pinned: HeliosChronicle @ edf8ced451e8f120e02f42f4b7b769ad17395888
Law: STATEMENT / LOCKS / PHASES (locks override). Phase A gate lifted — G/P stubs implement against this pin; H waits on D/F.

Lead accepted notes 2026-09-06; Q1–Q8 answered below (locked for this lane).

---

## Scope (this lane)

| Phase | Contract |
|-------|----------|
| **G Contact** | Fog, treaties, contracts |
| **H Violence** | Sensors → combat → planetary layer writes; refugees / wrecks / signals as knowledge objects |
| **P Politics** | Standing driven by typed chronicle events + knowledge paths |

Depends on earlier ledger surfaces (A–F): entities, systems, worlds/layers, hulls, sensors inputs. Does **not** own doctrines/orders (I) or operator injectors (J), but must expose fields those phases read.

---

## Lock constraints that bind this lane

- **Lock 9 (Atrocity knowledge):** Refugees, wrecks, and signals are **real knowledge objects**. Standing moves **only** along knowledge paths for **witnesses**. Victims auto-know and auto-react for atrocities on their own pops/worlds (STATEMENT: victims react hard) — refugees/signals are for third parties and confirmation chains, not a gate on the victim empire.
- **Lock 7:** No sick/dying *appearance* stages for stars. (Does not ban fog/sensor uncertainty for fleets/polities.)
- **Lock 10:** LOD mandatory early — contact/violence/politics must not force fine ticks on quiet systems; hot contact zones stay fine. Contact/Violence **push** fine-hot to Kernel (do not rely on Kernel polling).
- **Lock 4 (adjacent):** Capital home-pause is Sky/politics-adjacent; conquest re-flag is OK — G treaties / P standing may reference home flag transfers but do not redefine pause physics.
- STATEMENT pillars: *Acts have witnesses*; *Writable worlds*; *Same rules for AI and hand*; typed cruelty events; no Geneva in physics.

---

## Shared primitives (G ∩ H ∩ P)

### Knowledge object (KO)
First-class ledger entity (not a log string). Minimum fields (design):

- `ko_id`, `ko_kind` ∈ {`refugee_wave`, `wreck`, `signal`, `confessed_event`, `leaked_event`, …}
- `grade` ∈ {`rumor`, `confirmed`} — different standing weights (Lead Q2). Sources: sensors, refugees, wrecks, signals, `confessed` / `leaked` as confirmation paths. Operator rumor inject emits **rumor-grade** KO (same rules as hand).
- `origin_event_id` — typed chronicle event that produced it (or null if salvage/sensor-origin without prior typed event)
- `payload` — structured claim (who did what, where, severity class, target type, evidence quality)
- `carriers` — who currently holds/knows this KO (empire ids, ship ids, colony ids)
- `propagation` — how it moves (evac convoy, derelict scan, broadcast, diplomatic reveal, salvage yard leak)
- `created_tick`, `last_transfer_tick`

Witness standing/treaty reactions **subscribe to KO acquisition / confirmation**, never to raw violence alone. Victim empire is the exception: auto-know / auto-react without needing a third-party KO path (Lead Q1).

### Typed chronicle event (cruelty / contact)
Closed type enum (v1):

- Violence: `orbital_strike`, `bombardment_layer_write`, `surface_combat`, `glass_attempt`, **`salt`** (first-class cruelty event — Lead Q4; *causes* high-rate layer writes; doctrine `salt_willingness` and P standing key off this typed event)
- Contact: `first_contact` (emitted by **G only** — Lead Q8), `treaty_signed`, `treaty_broken`, `contract_default`
- Knowledge: `ko_emitted`, `ko_acquired`, `ko_confirmed`, `confession`, `leak`

Operator mutations that fake atrocity or rumor emit KOs through the same APIs (no operator-only diplo channel — Lead Q7).

### Knowledge path
Directed: event → KO emission → carrier chain → empire knowledge set → standing/treaty delta.

- **Victims:** auto-know and auto-react for atrocities on their own pops/worlds.
- **Witnesses:** only along knowledge paths (Lock 9). Refugees/signals serve third parties and confirmation chains.

---

## Phase G — Contact

### Fog
- Fog is **knowledge state per empire**, not a map shader: known systems, known fleets (last fix + uncertainty), known deposits/fuses only when surveyed (Sky owns fuse survey product; Contact consumes “known?”).
- Sensor contact (H) upgrades fog entries; diplomatic exchange can grant fog without sensors.
- **`first_contact`:** G owns fog upgrade + emits the typed chronicle event; P consumes that event for standing. Single emission source in G; P does not invent a parallel contact event (Lead Q8).
- LOD: quiet empires with no foreign contact stay coarse; fog updates batch on coarse ticks. G **pushes** fine-hot when contact goes hot (Lead Q6).

### Treaties
Ledger objects, not free text. **Minimal v1 clause enum frozen** (Lead Q3 — no opaque `{type,params}` bag):

- `non_aggression`
- `open_passage`
- `extradition_stub`
- `reparations_stub`

Also: parties, start/end ticks, break conditions. Breach emits typed event + optional KO (if third parties can learn of the breach). Same rules for AI and operator-forced sign/break.

### Contracts
Narrower than treaties: freight, salvage rights, mercenary/hire, survey charter. Default/fulfillment is Matter/Hulls-adjacent; Contact owns the agreement record and default event typing so P can score trust.

### Out of G
Doctrine “willingness to honor” → I. Standing math → P. Actual sensor resolution → H.

---

## Phase H — Violence

### Pipeline
Sensors (detect) → engagement / strike orders (I later; H simulates resolution when ordered) → damage on hulls + **planetary layer writes** (same columns as star stages: atmosphere/pressure, temperature, radiation, toxins/fallout, biosphere) → mortality / infrastructure bills (D) → **KO emission** (refugees, wrecks, signals).

### Planetary layer writes
- Strike is ordinary damage + layer deltas. Uninhabitable ≠ unclaimable ≠ unmineable (STATEMENT).
- **`salt` is a first-class typed chronicle event** that *causes* high-rate layer writes (Lead Q4). Not a tag on bombardment. No legal subsystem.
- Remediation cost/time owned with D; H only writes layers and emits events/KOs.

### Knowledge object emission (Lock 9)
| Source | KO kind | Typical carriers |
|--------|---------|------------------|
| Evac from struck world | `refugee_wave` | convoys, destination colonies |
| Destroyed/abandoned hull | `wreck` | salvage ships, nearby sensors |
| Broadcast / beacon / leak | `signal` | anyone in receive envelope |
| Captured logs / confession | `confessed_event` / `leaked_event` | holding empire |

Emission is physics/ledger truth. **Who learns it** is path + sensors + diplomacy (except victim auto-know).

**Salvage (Lead Q5):** salvage contact **always** grants the wreck KO + basic payload claim. Never salvage-blind. Deep blueprint / segment stats may be E incomplete-stats gated.

### Sensors
Enough to: last-known fix, detection of strikes in range, wreck discovery, signal receive envelope. Exact combat model can stay thin until F hulls exist; H schema should already reserve sensor envelopes so fog (G) and KO paths (P) are not retrofit.

### LOD (Lead Q6)
Contact/Violence **push** fine-hot to Kernel for involved systems (active fights + fresh KOs). Do not rely on Kernel polling — coarse ticks must not miss fight/KO edges (Lock 10; aligns Galaxy monotonic fuse rule). After quieting, drop back. No galaxy-wide fine combat ticks.

---

## Phase P — Politics

### Standing
- Scalar (or small vector) per empire-pair, driven by **typed events filtered by knowledge**.
- Formula inputs: severity, target type, doctrine flags (I), relation to victim, repetition, KO grade (`rumor` vs `confirmed`).
- **Witnesses:** no standing delta without a knowledge path (Lock 9).
- **Victims:** auto-react (Lead Q1).
- Same standing/diplo model for AI and hand from day one; operator mutates via ledger/events/KOs only (Lead Q7).

### Grudges / memory
Chronicle-backed: store event ids + KO ids that moved standing, not opaque decay alone. Decay/forgiveness is a later knob; v1 can be sticky.

### Treaty feedback
Standing gates AI willingness (I) and can auto-break or refuse renewal (G). P owns the score; G owns the instrument.

### first_contact consumption
P consumes G-emitted `first_contact` for standing; does not emit a parallel contact event (Lead Q8).

### Internal factions (STATEMENT)
May punish successful salting even if external standing does not. Design as optional internal standing channels keyed off the same typed `salt` events — stub until I, but event types must already exist so factions are not a schema break later.

---

## Slice 5 alignment
War, salt, witnesses, god tools: H writes layers + emits KOs; P moves standing when knowledge moves (victims immediately); G supplies fog/treaties so “witness” is not global; J later injects rumors/possession using the same KO/event APIs (rumor-grade).

---

## Non-goals (this lane, v1)
- Legal/Geneva subsystem
- Omniscient news ticker
- 3D spectacle / Aurora UI
- Full doctrine AI (I)
- Appearance-based “sick star” tells (Lock 7)
- Opaque treaty `{type,params}` bags
- Operator-only diplo channel
- Salvage-blind wrecks (no KO)

---

## Dependencies / wait-for (agreed with Lead)
1. **A Kernel** scaffold — cleared at pin edf8ced. G/P stubs may land; H resolution still waits on D/F.
2. D layer columns + F hull damage fields before H resolution is real
3. B sensor/range / system graph for fog envelopes
4. I doctrines consume P standing + H `salt` events but do not define them

---

## Lead decisions (2026-09-06) — locked

1. **Victim privilege** — victims auto-know and auto-react for atrocities on own pops/worlds. Witnesses only along knowledge paths. Refugees/signals = third parties + confirmation, not a victim gate.
2. **KO grades** — `rumor` → `confirmed` (confessed/leaked as sources); different standing weights. Operator rumor inject → rumor-grade KO.
3. **Treaty clauses** — freeze minimal v1 enum: `non_aggression`, `open_passage`, `extradition_stub`, `reparations_stub`. No opaque bag.
4. **`salt`** — first-class typed chronicle event (cruelty); causes high-rate layer writes. Doctrine + P key off the typed event.
5. **Salvage** — always wreck KO + basic payload on salvage contact; deep blueprint/segment stats may be E-gated. Never salvage-blind.
6. **LOD** — Contact/Violence **push** fine-hot to Kernel for involved systems. No Kernel polling for fight/KO edges.
7. **Symmetry** — same standing/diplo for AI and hand; no operator-only diplo channel.
8. **`first_contact`** — split OK: G emits + fog upgrade; P consumes for standing. Single emission in G.

---

## Status
Design notes copied into repo `docs/`. Phase A gate lifted at pin edf8ced; G/P stubs land in Kernel; H violence resolution still waits on D/F.
