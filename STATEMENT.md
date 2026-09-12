# Helios Chronicle — Project Statement

## 1. What this is

Helios Chronicle is a **2D space-empire simulation**. It is built to run without a player.

Empires mine, research, design ships from modules, trade, ally, fight, salt worlds, collapse, and get replaced. The operator may watch, or reach in and change any number on any entity — including a deck gun that destroys a planet.

It is Aurora-shaped in grain (components, scarce matter, labs, logistics, variable time, scenario editing). It is not an Aurora clone. It is not a physics thesis. Distance and orbits exist so freight and war have a clock.

Empires do not print mass. The only new tonnes in the universe come from **feed systems** already on the ledger: unmined crust, new systems attaching to the jump graph, remnants after a light dies, salvage of wrecks and works that already existed. Every mind — algorithmic, neural, or possessed — **harvests**. It refines, ships, spends, and loses. It does not create matter and it does not void matter except by using it or destroying objects that were already there.

**The simulation is the product. The operator is optional. One ledger feeds both.**

---

## 2. Locked terms

| Term | Meaning |
| --- | --- |
| **Ledger** | The authoritative set of records the tick reads and the editor writes. |
| **Operator** | The human. Spectator by default. Editor when they choose. |
| **Empire** | A polity: pops, industry, fleets, tech book slice, treaties, standing, doctrines, officers. |
| **Species envelope** | One set of survival numbers for every sophont. Not per-race. Not genetic. |
| **Body** | Star, planet, moon, asteroid, remnant. Has orbits and environment layers. |
| **System** | One star and its bodies, deposits, jump links, and two resource-era fields: remaining feed and (if dry) an extinction clock. |
| **Catalog / tech book** | Global list of named lines, segments, and recipes. Grows when anyone unlocks a segment. |
| **Binding stock** | A deposit of a material that the catalog still uses, large enough to count. |
| **Remaining feed** | How much binding stock is still in the ground in that system. |
| **Depleted** | Remaining feed has fallen below the dry threshold. |
| **Extinction clock** | Starts when the system is depleted. Exact time left is hidden until surveyed. |
| **Home flag** | A living empire’s claim that this system is its home. Pauses an armed extinction clock. |
| **Layer** | One habitable property of a body (pressure, temperature, radiation, toxins, biosphere). |
| **Segment** | One increment on a named technology line. |
| **Module** | One designed component installed on a hull. Has mass, power, crew, bill of materials, fuel type, and combat fields. |
| **Knowledge** | What an empire has actually detected or been told. AI and diplomacy use only this. |
| **Shell** | The main window: map always visible, time controls, event strip, and the root of the window tree. |
| **Window** | A structured inspector for one record type. Windows stack and stay open; they do not replace the map. |
| **Tree path** | How you drill down: Galaxy → System → Body → Colony / Fleet / Deposit → Module / Stock / Layer. |
| **Viewpoint** | Whose knowledge the shell is filtered through: Operator (all) or one empire. |
| **Tag color** | Operator-chosen display color on an empire or entity. Chrome only. |
| **Waypoint** | Operator pin on the map: name, color, location. Not a sim object unless later tied to orders. |
| **Empire API** | External client that reads one empire’s knowledge and submits orders. Same rights as possession. |
| **Algorithmic mind** | The default empire brain: scored rules and doctrines on known ledger fields. |
| **Neural mind** | An external policy bound through the Empire API. No extra vision, no extra matter. |
| **War trial** | A canned scenario that scores whether a bound mind won, lost, or stalled a war. |


### Frozen issue locks (override this file on conflict)

1. Binding = cosmology catalog + extractable × accessibility. Price never unbinds.
2. Depletion / fuse = physics. Optional later stabilizer may only pause / cancel fuse.
3. Wilderness immortal until surveyed **or** claimed. No silent refill.
4. Capital-only home pause. Re-flag OK. Outposts cannot pause. Paused capitals count in the live-system band.
5. One master era length; dry / fuse / research are ratios.
6. Full binding table from day one.
7. No sick look — fuse timer, then explode / end.
8. All extraction drains the well.
9. Refugees / wrecks / signals are real knowledge; standing only along those paths.
10. LOD mandatory early; cut what breaks long headless runs.

11. **Chronicle / History, not full replay** — notable EventLog moments only (tick, system, kind, one-line); shell history panel; no tick-perfect recording.

---

## 3. Rules that do not move

1. If a number exists, it sits on a record and the operator can change it. The tick honors the new number. The log records the edit.
2. **No mind creates or destroys matter.** Algorithmic AI, neural clients, and a possessed empire all use the same recipes, fuels, sensors, and costs. Stockpiles rise only by harvest, freight, salvage, or trade of tonnes that already exist on the ledger. Stockpiles fall only by build, fuel burn, maintenance, loss, or transfer. There is no shadow production, no AI-only spawn, no delete-ore cheat. New mass enters the universe only through the feed systems (unmined binding crust, spawned systems, remnants, salvage of existing wrecks and works). The operator may still write a pile by hand; that is a logged edit, not an AI right.
3. There is one species. Better living comes from buildings and research, not genomes.
4. A star does not begin dying at birth. It begins dying when the system is economically dry.
5. Dry does not mean “every pebble is gone.” It means binding stock is gone.
6. Binding is a **cosmology catalog** test (recipes / fuel / rares in the fixed game catalog + extractable × accessibility floor). Spot price never unbinds; it only affects who digs.
7. Depletion and fuse are **physics**, not empire- or tech-gated. Optional later star-stabilizer tech may pause or cancel a fuse only.
8. Wilderness suns are immortal until **surveyed or claimed**; then normal drain / fuse rules. No silent refill.
9. **Capital systems only** may pause an armed fuse while the home flag is held. Paused capitals count in the live-system band. Re-flag on conquest is OK. Non-capitals cannot home-pause.
10. One **master era length**. Time-to-dry, fuse length, and research-segment time are editable **ratios** of that length.
11. Spawn always includes the **full cosmology binding table from day one**.
12. **No sick / dying appearance stages.** After depletion: fuse timer, then explode / end.
13. **All extraction drains** the remainder: state, civilian, foreign, abandoned automation.
14. Refugees, wrecks, and signals are **real knowledge objects**. Standing only moves along knowledge paths.
15. Simulation **LOD is mandatory early** (coarse quiet / fine hot). Cut anything that breaks long headless runs.
16. Worked-out veins do not refill. New matter enters as new systems, remnants, and salvage.
17. Planetary bombardment writes environment layers. There is no legal ban. Witnesses may still punish the act if they know about it.

---

## 4. Species and worlds

### Envelope
Every person shares one editable envelope: pressure band, temperature band, gravity tolerance, radiation tolerance, breathable mix, calorie and water need, baseline lifespan, baseline fertility.

### How pops get “better”
Only through **structures** and **segments**: medicine, training, habitat density, sealed living, radiation shelters, processors. After a nuclear strike, headcount and growth change. The species record does not.

### Environment layers
Each body stores a short stack, compared against the envelope:

- atmosphere / pressure
- temperature
- surface radiation
- toxins / fallout
- biosphere / food viability

A colony pays build cost and upkeep for every deficit. Whatever is not covered kills growth, labor, and people.

Layers change when:

- a fuse expiry / end event may write layers (at the end, not as a sick-look campaign)
- industry or remediation works (slow)
- weapons hit the world (fast)
- the operator writes a field (immediate)

**Uninhabitable is not gone.** The body stays on the map. It can still be claimed. It can still be mined if automation segments exist. Fixing a salted world is slower and more expensive than breaking it.

---

## 5. Sky, feed, and death

### Motion
2D systems. Orbits advance. Transfers cost time and the right fuel. Jump points are hidden until surveyed. The clock can step from seconds (combat, sensors) to months (industry, research).

### What keeps a sun stable
Each system tracks **remaining feed**: extractable binding stock still in the ground.

A stock is **binding** when both are true (cosmology catalog test, not this week’s market):

1. The material appears on any recipe, fuel chain, or rare-good gate in the **fixed cosmology catalog** (full table from day one; later unlocks do not redefine what was already binding).
2. Extractable quantity × accessibility is above a global floor. Dust and pebbles do not count.

Spot price never unbinds a stock. Price only changes whether anyone bothers to dig.

Depletion and fuse are **physics**. Optional later **star-stabilizer** tech may pause or cancel a fuse only. It does not redefine binding.

**All extraction counts** toward remaining feed: state mines, civilians, foreign strip-mines, forgotten automation still running.

When remaining feed drops below the dry threshold, the system is **Depleted**. The extinction clock **arms**.

### What the clock does
Until depleted, the star is stable.

After depleted there are **no sick or dying appearance stages**. There is only a game-served **fuse timer**. When it hits zero the system explodes / ends (it may write layers at that end). Remnants may leave a short last harvest.

**Wilderness is immortal until surveyed or claimed.** After either happens, normal drain / fuse rules apply. No silent refill.

### Survey
A geological survey always reports deposits, accessibility, and remaining feed.

After depletion, a survey also reports the **exact fuse ticks**. Without that survey an empire knows a fuse is running if they have the game-served timer, not because the star looks sick. There is no staged sick light.

### Homes
Home is a **capital flag**, not a special physics class.

- Capitals mine out like anywhere else.
- Capitals can be glassed like anywhere else.
- When a **capital** depletes, the fuse arms and **pauses** only while a living empire holds the capital home flag.
- The pause ends if that empire is destroyed or abandons the flag.
- A conqueror or successor may re-flag the capital and pause the fuse again.
- **Mining outposts and other non-capitals cannot receive home-pause.**
- Paused capitals **count in the live-system band** so spawn does not balloon around immortal seats.

So a capital remains a prize. It does not nova on the victory tick unless the operator forces it.

No other system an empire owns is immortal.

### How matter keeps flowing
Forbidden: topping up empty veins; infinite home crust; AI-only ore.

Allowed:

- whatever binding stock is still in living ground
- new unsurveyed systems added to the jump map when the count of useful systems sags
- remnants after a star dies
- salvage and recycling

Spawn always includes the **full cosmology binding table from day one**, not a research- or era-gated slice. A late map that only spawns dirt cannot feed antimatter chains.

Paused dry **capitals** still count toward the live-system band so spawn does not balloon around immortal seats.

### One master era, three ratios
There is one **master era length**. Dry-time, fuse length, and research-segment time are editable **ratios** of that length.

If research outruns freight, every empire looks hungry and stupid. If fuses last forever, depletion is flavor text.

---

## 6. Industry

A short list of raws. Products appear when a segment needs them.

Every module and facility has a bill of materials. Many also have an **operating consumable**. Antimatter engines require antimatter production, tankage, and tankers. No chain, no movement.

Rare goods are either tiny natural veins or refinery outputs that only exist after specific segments. They sit on recipes. They are not flavor icons.

Freight is tonnes, time, and risk. Maintenance and fuel are ongoing drains. You may strip a rock with machines and never settle it.

---

## 7. Research

Technology is **named lines** with **segments**.

Examples of lines: a drive family, a sensor family, a spinal family, a lab process, a habitat method, a medical line.

A segment changes a few explicit stats (mass, power, efficiency, reliability, resolution, yield) and may raise the material or fuel gate. There is no last segment. Cost and gates stretch.

Work is lab capacity on a queue: “advance this line one segment” or “unlock this named line.”

Order of fielding:

1. Segment exists in the catalog.
2. A concrete component is designed from it.
3. A yard is tooled.
4. Instances are built.

Salvage and espionage may skip a segment with missing or wrong stats.

---

## 8. Ships

A **design** is a list of modules. An **instance** is that list plus damage, magazines, fuel of the declared tier, crew, and maintenance.

The editor shows mass, power, crew, build cost, rare cost, yard size, and fuel type.

If the fuel on board is the wrong tier, the ship does not move.

Combat and planetary strike read the fields on the modules. If the operator sets a sidearm’s damage and “hits as planetary” flag high enough to crack a mantle, that is what happens. The log stores the edit. AI does not get those numbers unless they are actually on the module.

---

## 9. Empires, cruelty, and knowledge

An empire is pops, stockpiles, yards, fleets, officers, doctrines, treaties, and standing.

### Cruelty
World-attacks are typed events: orbital strike, population strike, biosphere kill, sterilization.

Physics does not care.

**Victims care.** Standing collapses. Grudges last. War aims update.

**Others care only if they know.** Knowledge arrives through sensors, wrecks, refugees, or leaked/injected events. Then standing may move, scaled by:

- how total the act was
- what was hit (fleet, colony, biosphere, already-dead rock)
- the witness’s doctrine
- whether the victim mattered to them
- whether this looks like a habit

Some doctrines salt. Some punish salt. Some only fear the precedent.

A hidden strike becomes political when the refugees or the wreckage exist. Those are real objects or guaranteed log events, not implied flavor.

Internal factions may split a winner who salted a world.

---

## 10. Minds

Every empire has a **mind slot**. The tick asks that slot for orders. The ledger does not care whether the answer came from a script, a window, or a network.

### Algorithmic mind (default)
This is what plays the chronicle when nobody is bound.

It is scored rules on **knowledge only**:

1. Remaining feed (survey).
2. Distance, lift, threat, jump position, material gates.
3. Exact fuse — only if depletion is known and surveyed.

Prefer long feed for cities, yards, labs, and Home flags. Use short feed and dry lights for mines, salvage, rares, and forts. Re-score a former home when its flag drops. Unknown feed and unknown fuses are uncertain, not infinite.

Doctrines are knobs on the same mind: salt-willingness, punishment-willingness, evacuate vs hold, research bias. Full policy tables come later. The contract does not: no hidden ore, no omniscient fuse.

Unbound empires always run this mind.

### Neural mind (trial)
A neural policy is **not** the galaxy’s default. It is a client.

Bind it through the Empire API to one empire (`possess`). It receives that empire’s knowledge (the same JSON the observe endpoint already owes) and returns typed orders. If it outputs an illegal order, the validator rejects it the same as a bad click.

It does not get Operator (all). It does not get extra fuel. It does not step the clock unless the operator allows a trial harness to.

You may leave every other empire on the algorithmic mind and put the network on one side of a war.

### War trial
The whole sim has no victory screen. A **trial** may.

A war trial is a saved scenario plus a score function the operator can read:

- two (or more) empires, already in contact or forced to contact
- a clock limit
- win / lose / stall checks, for example: enemy Home flag dropped, enemy yards gone, own Home still held, enemy fleet tonnage under a line, or operator-written conditions

The harness runs headless, binds the neural mind to empire A, leaves empire B on algorithmic (or another client), and writes a result record: outcome, ticks used, losses, whether the network starved itself, whether it salted, whether it cheated the API.

Training loops, if you run them, must use only the possess observation. Using Operator (all) as a training input is allowed only as an explicit “cheating baseline” flagged in the result. That is how you tell “the net is good” from “the net saw the fuse.”

### Mix
Legal mixes:

- all algorithmic (default chronicle)
- one empire possessed by you in the shell, rest algorithmic
- one empire bound to a neural client, rest algorithmic
- two neural clients in a trial (later)

Illegal mix: a neural mind that reads fields its empire has not surveyed.

---

## 11. Interface

The UI is an **instrument panel**, not a cinematic. Aurora’s habit is the model: a map that never goes away, and a tree of structured windows you open from it.

### Shell
The main window is always:

- a **map** that can show one system or the galaxy / jump graph without replacing the shell
- **pan and zoom** — drag the view; zoom in to a body or task group; zoom out to the jump network. Same map, different scale.
- **time controls** (pause, step, increment size)
- an **event strip** / history strip (notable chronicle moments plus operator edits)
- a **viewpoint control**

The map is the root of the tree. Clicking a thing does not “enter a different game.” It opens or focuses a window.

### Viewpoint
The shell can show **Operator (all)** or **Empire: [name]**.

- **Operator (all)** — every deposit, fuse number, fleet, and treaty the ledger has. Fog off.
- **Empire viewpoint** — the same windows and map, filtered to that empire’s **knowledge**. Unsurveyed fuses read unknown. Unseen fleets are absent. Standing and atrocities appear only if that empire knows them.

Swap at any time. Viewpoint does not pause the tick and does not change whose turn it is. It is a filter. You can page through empires to see who sees whom, then return to all.

Possessing an empire for orders is separate: viewpoint is “what they know”; possession is “what you may command.” You can look as them without commanding, or command them while still flipping back to all.

### Tag colors
Any empire, and any other tagged entity (fleet, body, waypoint, colony), may have a **display color** picked from a color wheel.

- Default can follow empire color.
- You may override per entity.
- Colors are operator chrome. They do not alter standing, combat, or knowledge.
- Names on the map and in window headers use that color.

### Waypoints
You may drop **waypoints** on the system map or galaxy graph.

Each waypoint has: position, name, color (wheel), optional note.

They exist for the operator: mark a chokepoint, a dying light, a rare vein, a promised battlefield. They persist across viewpoint swaps unless you hide them. They are not binding stock, not fleets, and not required for the AI. Later, if useful, a possessed empire may be allowed to treat a waypoint as an order target. Until then they are pins on the glass.

### Comfort (shell)
These do not add simulation rules. They keep a long chronicle readable.

- **Find** — type a name or id; jump the map and open the tree path. Empires, systems, bodies, fleets, designs, waypoints.
- **Follow** — lock the camera to a body, ship, or task group. Zoom and viewpoint still work. Cancel follow to free-pan again.
- **Go to** — last selection, focused empire’s Home flag, previous system.
- **Hover sheet** — short ledger on linger (feed, dry, known fuse, owner, color) without opening a window.
- **Overlays** — orbits, jump links, sensor bubbles, civilian vs military, feed/dry/dying badges, waypoints, tag colors. All can be off.
- **Measure** — two points or objects: distance and a transfer-time estimate if the viewpoint knows a drive.
- **Event filters** — chronicle by type, empire, system, severity. Click through to the records. Pin event types on the strip.
- **Pause on** — first contact, Home depleted, fuse armed, atrocity entering knowledge, watched waypoint’s system changing. Stops the tick; does not rewind.
- **Watchlist** — pin a field (stock, fuse, standing, ship). Nudge when it crosses a threshold you set.
- **Layout memory** — windows, scale, overlays, viewpoint restore on load. One-click clean map.
- **Edit undo** — operator writes undo in order and stay in the chronicle. The tick is not undone.
- **Color plus mark** — shape or letter with the wheel so close colors still split.

Also in the shell when cheap: galaxy inset while inside a system, design-vs-design split window, viewpoint diff (“what A sees that B does not”), screenshot / map export.

### Tree
Navigation is always the same direction:

**Galaxy → System → Body → Site → Detail**

Examples:

- Galaxy → System (feed / dry / dying / Home pause / unknown fuse)
- System → Body (layers, deposits, orbits)
- Body → Colony (pops, structures, upkeep vs envelope)
- Colony → Stockpile, factory, mine, lab, yard
- System → Task group → Ship instance → Module list
- Catalog → Named line → Segment → Designed component → Design that uses it
- Empire → Treaty / standing / doctrine
- Event → the records it touched

Every window has a **breadcrumb** of that path. Closing a child returns to the parent. Parents stay open.

### Window rules
1. Windows are tabular and inspectable. If the ledger has a field, the window can show it.
2. Several windows may be open at once (colony, design, research queue, event log).
3. **Chronicle view** is read-only plus time controls.
4. **Intervention** is the same windows with fields unlocked. No separate “cheat screen” that bypasses the tree. Editing a module damage value happens on that module’s row.
5. Fog is a filter, not a different UI. Empire viewpoint hides unknown fuse numbers and unsurveyed deposits. Operator (all) shows the same rows filled in.
6. Badges on the map are only summaries: remaining feed, depleted, clock surveyed or not, Home flag, glassed layers, standing alerts. The window is where the numbers live.
7. Pan, zoom, viewpoint, tag colors, and waypoints belong to the shell. They must work before the window tree is deep.

### Primary windows (v1 set)
- **System map / galaxy graph** — the shell: pan, zoom, viewpoint, tag colors, waypoints.
- **Body** — layers, deposits, accessibility, colonies and outposts on that rock.
- **Colony** — pops, structure list, deficit bill, stockpiles, build queue.
- **Industry** — mines, refineries, chains, civilian shipping.
- **Catalog** — named lines and segments.
- **Research** — labs, teams, queue.
- **Design** — module list, mass/power/crew/BOM/fuel tier.
- **Yard / fleet** — tooling, instances, orders, fuel and maintenance state.
- **Contact** — standing, treaties, known atrocities.
- **Chronicle / History** — notable EventLog moments (tick, system, kind, one-line); filterable; click-through to records. Not a tick-perfect replay viewer.
- **Record editor** — same tree, write mode on.

### What the UI will not do
It will not hide the economy inside sliders with no tonnes. It will not replace the map with a full-screen menu. It will not require a unique screen per mechanic if that mechanic is already a row on an existing record.

---

## 12. Operator

**Chronicle mode.** Time runs. Map, ledger, event log.

**Intervention.** Pause or not. Change any field. Spawn or delete systems. Force dry, force pause, force nova. Glass a world or clear fallout. Arm a pistol. Forge a rumor. Possess one empire.

Every mutation is a chronicle event so the history stays honest.

---

## 13. Empire API

The sim is a ledger with a tick. The shell is one client. An external program can be another.

**What it is.** A local interface (first: HTTP or sockets on the same machine; later anything that speaks the same schema) bound to **one empire**. The client may:

- read that empire’s **knowledge** (map, stocks, fleets, designs, treaties, events it has seen)
- submit **orders** the possessed UI could submit (move, survey, queue research, tool a yard, offer/refuse a treaty, set standing orders)
- read accepted / rejected reasons using the same validators as the shell

**What it is not.** It is not Operator (all) unless the operator issues a separate god key. A client cannot see unsurveyed fuses, spawn systems, or edit module damage unless that key is on. Fog stays fog.

**Same rules.** Fuel, yards, knowledge, and combat do not care whether the order came from a window or a script. Invalid orders fail the same way.

**Tick.** Orders queue and apply on a tick boundary. The client does not step the clock unless the operator allows it. Headless runs are the point: leave the galaxy going, drive one empire from a notebook, a bot, or another app.

**Identity.** Each bound client names an empire and a role: `possess` (orders + knowledge) or `observe` (knowledge only). The shell can still open that empire’s viewpoint while the API is connected. If both issue orders, last-applied-on-tick wins and both writes land in the chronicle.

**v1 shape.** Stable record ids, JSON (or equivalent) read of known records, POST of typed orders, event stream for that empire. No requirement for internet, accounts, or other human players.

This is not multiplayer. It is remote possession.

The neural mind is one kind of possess client. The algorithmic mind is what runs when no client is bound.

---

## 14. Scope

### Product
A headless-capable 2D ledger sim of empires, matter, research, module ships, writable worlds, and a map-and-window operator shell. Default play is autonomous algorithmic minds. The operator may watch, edit, possess, or bind an external / neural client to one empire and score a war trial.

### In v1
- One species, one envelope, tech-and-structure uplift only
- 2D systems, orbits good enough for time and fuel, jump graph, survey fog
- Binding feed, depletion, fuse timer then explode/end (no sick stages), **capital-only** home pause, remnants, spawn from the **full** binding table, LOD early
- Short raw list, product chains, fuel tiers, rares on recipes, civilian freight, salvage
- Named tech lines with segments; design after research; no cap
- Ships as module lists; instances need the declared fuel; operator-writable combat fields
- Environment layers; bombardment writes them; remediation exists and is slower
- Standing and typed cruelty events along knowledge paths
- Shell: map stays up; pan/zoom; viewpoint Operator-or-empire; color wheel; waypoints; window tree; comfort set (find, follow, overlays, pause-on, watchlist, layout, edit undo)
- Algorithmic mind on every unbound empire
- Empire API: observe or possess one empire; same validators as the shell
- War trials: small canned scenarios, written score, fog-honest neural bind
- God editor on the same tree; edits logged

### Not in v1
- Multiplayer, accounts, internet-required play
- 3D or cinematic combat
- Other species, gene mods, per-race envelopes
- Real n-body gravity or astrophysical stellar evolution
- A campaign victory screen (trials score; the chronicle does not “end”)
- Aurora names, formulas, or pixel-for-pixel chrome
- Neural net as the default galaxy brain
- Training on Operator (all) presented as a fair trial
- Full ground-combat template designer
- Asking the neural mind to run a whole economy before a one-system fleet trial works
- Full tick-perfect replay files or a replay viewer (chronicle/history of EventLog moments instead)

### Later, not promised
- Galaxy inset, design-vs-design split, viewpoint diff, map export
- Waypoints as order targets for a possessed empire
- Two neural clients in one trial
- Wider trials (economy + diplomacy + multi-system war)
- Remote API beyond local host
- Deeper map-scale chrome (LOD itself is already a v1 kernel requirement)

### Explicit non-goals
The sim does not exist to teach physics, to moralize bombardment, or to replace Aurora. It exists to keep a precise toy universe running, and to let you put a second brain on one side of a war without lying to that brain.

---

## 15. What “working” looks like

- A system full of pebbles and unused dirt does not live forever.
- A system that still holds catalog-binding ore does not start a fuse.
- Once it is dry, the fuse timer runs; anyone who wants the exact ticks must survey. The star does not look sick first.
- Wilderness that nobody has surveyed or claimed can outlive empires; after either, it drains.
- A living empire’s **capital** can sit dry and live on imports; a destroyed empire’s capital can start to die. Outposts cannot pause.
- New systems always roll from the full cosmology binding table.
- Opening a ship shows modules, recipes, fuel tier, and parent segments.
- A late engine with no fuel chain does not move.
- A god-edited planet-killer works and is logged.
- Standing moves when knowledge moves, not when a hidden karma counter twitches.
- Left alone, the galaxy still produces expansion, bottlenecks, trade, salt, evacuation, collapse, and successors.
- From the map you can walk a breadcrumb to any module, stock, layer, or treaty without losing the system view.
- You can flip viewpoint from Operator (all) to any empire and back and immediately see who knows which fleets and fuses.
- You can pan and zoom the same map from a hull to the jump graph, paint tags with a color wheel, and drop named waypoints on places you want to watch.
- An external client can possess one empire: read only what that empire knows, submit only legal orders, and leave the shell free to watch or go omniscient.
- The galaxy runs on the algorithmic mind by default. A neural client can be bound to one empire and scored in a war trial against that mind without seeing through the fog.

---

## 16. Build order

Each step ends with a save and a tick that can run with nobody at the keyboard.

**A. Kernel** — clock, seed, log, records, operator read/write, headless run, **LOD skeleton** (coarse quiet / fine hot). Cut what breaks long headless runs.

**U. Shell** — map that stays up; pan and zoom; viewpoint Operator-or-empire; color wheel on tags; named waypoints; openable windows; breadcrumbs; time controls; event strip. Can start as soon as A and B0 exist. New record types add a window, they do not add a new program.

**B. Sky** — bodies and orbits; transfers; jumps; fuse arms on depleted (timer only, no sick look); remnants; spawn from the **full** binding table; wilderness immortal until surveyed or claimed; map shows feed, dry, fuse timer, unknown ticks, capital Home pause.

**C. Matter** — deposits, mines, stocks, refine, fabricate, fuel chains, rares, binding remainder, civilians, salvage.

**D. Worlds** — envelope, layer bills, upkeep, growth and evac, automation on dead rock.

**E. Research** — lines, segments, labs, design-after-segment, salvage skips.

**F. Hulls** — module schema, editor, yards, instances, correct-tier fuel, maintenance, writable combat fields.

**G. Contact** — fog, first contact, treaties, trade contracts.

**H. Violence** — sensors, short tick, module damage, magazines, layer writes, wrecks and refugees.

**P. Politics** — standing, typed atrocities, victim path, witness path, treaty break, optional internal fracture.

**I. Minds** — standing orders; doctrines (including salt and punishment); feed first, fuse second; collapse drops Home flags.

**J. Operator product** — full editor, injectors, possession, chronicle/history of EventLog moments (LOD already started in A). **No full tick replay.**

**K. Empire API** — bind a client to one empire; knowledge read; typed orders; event stream; optional operator god key. Can follow I once orders exist; observe-only can land earlier.

**L. Minds and trials** — algorithmic default on every unbound empire; bind a neural (or any) policy through K; war-trial scenarios with a written score. After H exists at least as “fleets can kill fleets.” Start trials small: one system, two fleets, no full economy. Widen only when that trial is honest.

Ship it in slices, not as a finished galaxy:

1. A rock goes around a star and a mine counts down.
2. People cost habitat; fuel is a product.
3. A researched segment becomes a module on a ship that will not move without its fuel.
4. Binding ore is eaten; the surveyed fuse starts; new lights appear.
5. Wars can salt worlds; witnesses can care; the operator can rewrite the gun.

---

## 17. Known tensions (accepted)

- **Wilderness immortality.** Unsurveyed and unclaimed rich systems never die. Once surveyed or claimed, they drain and fuse like any other. Feature. No silent refill.
- **Dry immortal capitals.** Only the capital flag pauses a fuse. Winning seats become imported-fed fortresses. Feature. Count them in the live-system band. Outposts cannot pause.
- **Shared crust.** Anyone can mine a system toward depletion and arm a neighbor’s fuse. Feature.
- **Catalog-wide binding.** A material stays binding if *anyone* still has a recipe for it. Prevents price loops. Means a backward enclave cannot keep a sun alive on ore the era has moved past, and cannot unbind ore the era still uses.
- **Performance.** Module-level fleets plus endless spawn will drown a naive tick. Quiet systems must cheapen.
- **API vs shell.** Two commanders on one empire (window and script) can fight. Last-applied-on-tick wins; both writes log. Do not invent a secret priority.
- **Neural action space.** A whole galaxy is a bad first exam. Trial scenarios must shrink orders (move, shoot, retreat) before the net is asked to run an economy.
- **Cheating baseline.** Training on Operator (all) will look like genius. Label those runs. Fair trials use possess-fog only.

---

## 18. Purpose

Industry is the physics. Feed is finite per place. Lights fail after they have been eaten. New ground keeps the machines fed. Homes stand with their empires. Worlds can be ruined on purpose. Witnesses may care. One species improves itself with tools.

You may change any sentence — including the caliber of a pistol — and the chronicle keeps going.
