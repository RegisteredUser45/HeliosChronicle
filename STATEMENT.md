# Project Statement

## Helios Chronicle

### Intent

A 2D, Aurora-class space-empire simulation that runs without a player. One species. Ships as module lists. Named research in incremental segments. Industry as chains, not sliders. Systems that remain stable while they still hold catalog-binding matter, then start a surveyed extinction countdown once that matter is gone. New systems and remnants keep throughput alive. Home suns are politically anchored until the host is gone. Worlds have writable environment layers; there is no Geneva in the physics. Polities remember cruelty when they know about it. The operator may rewrite any field, including a sidearm that cracks a planet.

The simulation is the product. The operator is optional. The tick and the editor share one ledger.

### Pillars

Chronicle by default.

Ledger over lore physics. Orbits and burns exist for time and cost.

One flesh, many machines. One envelope; uplift is structures and segments.

Named tech, segmented progress.

Tiered matter. Late modules imply late chains (fuel included).

Drain, then die; feed the map; anchor the home.

Writable worlds.

Acts have witnesses.

Same rules for AI and hand.

### Species and survival

One sophont type. Editable globals: pressure band, temperature band, gravity, radiation, breathable mix, calories, water, baseline lifespan and fertility.

Pops improve only through facilities and research segments (medicine, training throughput, habitat density, sealed living). No gene track. Nukes and fallout change headcount and growth, not the species definition.

A colony is pops + infrastructure on a body. Hostile layers bill structure, power, and upkeep. Untreated remainder hits mortality, fertility, and labor. Automation segments decide whether a tomb still yields ore with nobody living there.

### Space, feed, and death

Motion. 2D systems good enough for orbits, transfer time, patrol radius, and convoy ETA. Jump links exist hidden until surveyed. Time increment runs from tactical seconds to strategic months.

Two numbers on every system.

binding_remainder — extractable catalog-binding value still in the ground.

extinction_clock — exists only after depletion; exact remaining ticks are a survey product.

Catalog-binding (locked definition).
 A stock is binding if:

it appears on any recipe, fuel chain, or rare gate in the galactic tech book (starting book ∪ anything any empire has unlocked), and

extractable quantity × accessibility ≥ a global floor (specks and dust do not count).

Spot price and AI disinterest do not unbind a stock. They only change who bothers to dig.

Depleted. When binding_remainder drops below a global editable threshold, the system sets depleted and arms the extinction clock. All extraction counts: state mines, civilian lines, foreign strip-mines, abandoned automation still running.

Clock behavior. Stable until depleted. Then stages write environment layers and eventually remove or sterilize the system. Remnants may leave a last harvest. Unmined wilderness does not die; say that plainly.

Survey split.

Always: deposits, accessibility, remaining feed.

After depletion: exact fuse length, earned by survey.

Appearance may show “this light is sick” once stages have begun. Sick ≠ known number.

Homes. Home is a flag held by a living empire. Depletion still happens. Default: clock armed and paused while the flag is held. Pause ends on destruction or abandonment. Re-flag by conqueror or successor is allowed so capitals stay prizes. Glassing layers does not, by itself, kill the star.

Feed pipes (no refill of worked veins). Remaining binding stocks; new unsurveyed systems to hold a live-system band that includes paused homes; remnants; salvage/recycling. Spawn tables must be able to roll whatever the current book considers binding, or late gates starve by construction.

Balance triad (operator settings). Typical time-to-dry a worked system; countdown length after dry; time-to-complete a research segment. Change one, watch the other two.

### Industry

Short raw list. Products appear when a segment needs them. A component declares BOM and operating consumable. If you field antimatter drives, you run antimatter plants and tankage or the fleet is sculpture.

Rares are small natural veins or gated refineries on recipes. Freight is tonnes and time. Wealth, labor, energy, maintenance supplies are constraints.

### Research

Named lines; segments inside each line (stats, RP, material gate). Labs are capacity. Segment researched → component designed → yard tooled → instance built. No final segment. Salvage can skip with incomplete stats.

Population tech is more lines, same machine.

### Ships

A design is a module list. An instance has damage, magazines, the correct fuel tier, crew, maintenance. Wrong fuel: no move.

Operator-writable fields include damage and planetary flags. Combat reads those fields. The log records the edit.

### Environment and war on worlds

Few layers: atmosphere/pressure, temperature, radiation, toxins/fallout, biosphere. Star stages and weapons write the same columns at different rates.

No legal subsystem. Planetary strike is ordinary damage plus layer writes. Uninhabitable ≠ unclaimable and ≠ unmineable. Remediation is slower and more expensive than break.

### Polities

Standing, treaties, doctrines, officers, grudges.

Typed chronicle events for cruelty. Victims react hard. Witnesses react only along a knowledge path (sensors, wrecks, refugees, confessed/leaked events). Response scales with severity, target type, doctrine, relation to the victim, and repetition. Some empires salt; some punish salt; some only care about precedent.

Internal factions may punish a successful salting. Physics does not.

### AI (stub, for the later pass)

Score what the empire knows:

Remaining feed (survey).

Distance, lift, threat, gates.

Exact fuse — only if depletion is known and surveyed.

Plant cities and yards on long feed. Allow short-feed and dry sites as mines, salvage, rares, forts. Treat Home-anchored lights as stable until the flag drops, then re-score. Unknown feed and unknown fuses are uncertain, not infinite.

Doctrines later: salt-willingness, punishment-willingness, evacuate vs die in place.

### Operator

Inspect and mutate any record. Force dry, force pause, force nova, force a rumor, possess an empire. Every mutation is an event.

### Non-goals (v1)

Multiplayer. 3D spectacle. Other species. Gene mods. N-body / real stellar physics as content. Victory screen. Aurora’s UI or copyrighted names.

### Success

Binding ore keeps a sun stable; pebbles and non-catalog dirt do not.

Dry then dying is visible on the ledger; the number on the fuse is earned.

Wilderness can outlive empires that never came.

A living home can sit dry and imported-fed; a dead empire’s home can start the fuse.

Spawn still yields what the current book eats.

Hulls expose modules, recipes, fuel tier, parent segments.

Planet-killer sidearm works and is logged.

Standing moves when knowledge moves.

Headless runs produce recognizable arcs.

### Phases (unchanged in order, tighter contracts)

A Kernel — tick, seed, log, save, entity ledger, operator R/W, headless.
 B Sky — orbits, transfers, jumps; clock starts on depleted; end + remnants; spawn including current binding table; map of feed / dry / dying / unknown fuse / Home pause.
 C Matter — deposits through chains; C7 binding remainder; civilians; salvage.
 D Worlds — envelope, deficits, drains, evac, automation on ash.
 E Research — lines, segments, labs, prototypes, salvage jumps.
 F Hulls — schema through god fields and fuel-tier motion.
 G Contact — fog, treaties, contracts.
 H Violence — sensors through layer writes and refugees (knowledge objects).
 P Politics — standing driven by typed events + knowledge paths.
 I Minds — orders and doctrines; feed first, fuse second; Home flag on collapse.
 J Operator — injectors, possession, replay, LOD.

Slices: (1) mine a pile (2) people + fuel (3) segment → ship that needs that fuel (4) eat binding ore → surveyed fuse + new lights (5) war, salt, witnesses, god tools.

### Open settings (not ambiguities)

Binding floor and dry threshold.

Home pause on/off.

Countdown length and stage schedule.

Live-system band and spawn recipe table.

Whether appearance reveals “sick” before survey.

Doctrine defaults for salt and punishment.

### Purpose

Industry is the physics. Feed is finite per place. Lights fail after they have been eaten. New ground keeps the machines fed. Homes stand with their empires. Worlds can be ruined on purpose. Witnesses may care. You may change any sentence — including the caliber of a pistol — and the chronicle keeps going.
