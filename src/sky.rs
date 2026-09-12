//! Phase B — Sky: depletion/fuse/home/wilderness/spawn against Kernel events.
//!
//! Pause policy: while `fuse_paused`, each tick adds `dt` to `fuse_end_tick`
//! so the absolute end slides forward (countdown freezes). LOD coarse dt still
//! fires fuse ends monotonically via `fuse_crossed_end`.

use crate::cosmology::embedded_catalog;
use crate::matter::{self, ExtractorKind};
use crate::entity::{EmpireId, EntityId, SystemEntity};
use crate::event::EventKind;
use crate::world::World;

/// Map presentation state (prefer dry+fuse / ended; "dying" is PHASES synonym only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapState {
    Feed,
    DryFuse,
    HomePaused,
    Ended,
    WildernessUnknown,
}

impl MapState {
    pub fn as_str(self) -> &'static str {
        match self {
            MapState::Feed => "feed",
            MapState::DryFuse => "dry+fuse",
            MapState::HomePaused => "home-paused",
            MapState::Ended => "ended",
            MapState::WildernessUnknown => "wilderness-unknown",
        }
    }
}

/// Resolve map ledger state for a system.
pub fn map_state(sys: &SystemEntity) -> MapState {
    if sys.ended {
        MapState::Ended
    } else if sys.is_wilderness_immortal() {
        MapState::WildernessUnknown
    } else if sys.depleted && sys.fuse_end_tick.is_some() && sys.fuse_paused {
        MapState::HomePaused
    } else if sys.depleted && sys.fuse_end_tick.is_some() {
        MapState::DryFuse
    } else {
        MapState::Feed
    }
}

/// Count live systems for the band: non-ended (paused capitals count).
pub fn count_live_systems(world: &World) -> u32 {
    world
        .ledger()
        .systems()
        .filter(|(_, s)| !s.ended)
        .count() as u32
}

/// Sync fuse_paused on all systems from capital + home_flag + globals.
pub fn sync_all_fuse_paused(world: &mut World) {
    let enabled = world.globals.home_pause_enabled;
    for (_, sys) in world.ledger.systems_mut() {
        sys.sync_fuse_paused(enabled);
    }
}

/// While paused, slide absolute fuse_end_tick forward by `dt` (freeze countdown).
pub fn apply_fuse_pause_slide(world: &mut World, dt: u64) {
    sync_all_fuse_paused(world);
    for (_, sys) in world.ledger.systems_mut() {
        if sys.fuse_paused {
            if let Some(end) = sys.fuse_end_tick.as_mut() {
                *end = end.saturating_add(dt);
            }
        }
    }
}

/// Claim ends wilderness immortality (Lock 3 + Lead: claim alone is enough).
pub fn claim_system(world: &mut World, id: EntityId) -> bool {
    let Some(sys) = world.ledger.get_mut(id) else {
        return false;
    };
    sys.claimed = true;
    true
}

/// Survey also ends wilderness immortality.
pub fn survey_system(world: &mut World, id: EntityId) -> bool {
    let Some(sys) = world.ledger.get_mut(id) else {
        return false;
    };
    sys.surveyed = true;
    true
}

/// If remainder crossed dry threshold and system is mortal → deplete + arm fuse.
pub fn check_depletion(world: &mut World, id: EntityId) -> bool {
    let tick = world.master_tick;
    let threshold = world.globals.dry_threshold;
    let fuse_len = world.globals.fuse_length_ticks();

    let Some(sys) = world.ledger.get(id).cloned() else {
        return false;
    };
    if sys.ended || sys.depleted {
        return false;
    }
    if sys.is_wilderness_immortal() {
        return false;
    }
    if sys.binding_remainder >= threshold {
        return false;
    }

    let end_tick = tick.saturating_add(fuse_len);
    {
        let sys = world.ledger.get_mut(id).expect("just checked");
        sys.depleted = true;
        sys.fuse_end_tick = Some(end_tick);
        sys.sync_fuse_remaining(tick);
        sys.sync_fuse_paused(world.globals.home_pause_enabled);
        sys.lod_hint = crate::lod::LodHint::Hot;
    }
    world.log.append(tick, EventKind::Deplete { system: id });
    world
        .log
        .append(tick, EventKind::FuseArmed { system: id, end_tick });
    true
}

/// Apply FuseEnd side effects: ended, clear fuse, minimal remnant harvest stub.
pub fn apply_fuse_end(world: &mut World, id: EntityId) {
    let tick = world.master_tick;
    if let Some(sys) = world.ledger.get_mut(id) {
        let remnant = if sys.binding_remainder > 0.0 {
            sys.binding_remainder.max(1.0)
        } else {
            1.0
        };
        sys.ended = true;
        sys.fuse_end_tick = None;
        sys.fuse_remaining = Some(0);
        sys.fuse_paused = false;
        sys.remnant_harvest = Some(remnant);
        sys.lod_hint = crate::lod::LodHint::Quiet;
    }
    // FuseEnd event is emitted by world tick after this helper (or caller).
    let _ = tick;
}

/// Roll full day-one binding table onto a system (Lock 6).
pub fn roll_binding_stocks(world: &mut World, id: EntityId) {
    let weights = world.globals.effective_spawn_weights();
    let mut stocks = std::collections::BTreeMap::new();
    let mut total_remainder = 0.0;
    for (stock_id, weight) in weights {
        // Deterministic quantity stub from RNG + weight.
        let roll = world.rng_u64() % 1000;
        let qty = weight.max(0.0) * (0.5 + (roll as f64) / 1000.0);
        total_remainder += qty;
        stocks.insert(stock_id, qty);
    }
    // Ensure every spawn-table id is present even if weight was 0.
    for sid in embedded_catalog().spawn_stock_ids() {
        stocks.entry(sid.to_string()).or_insert(0.0);
    }
    if let Some(sys) = world.ledger.get_mut(id) {
        sys.binding_stocks = stocks;
        if total_remainder > 0.0 {
            sys.binding_remainder = total_remainder;
        }
    }
}

/// Spawn a seeded (claimed) system with full catalog binding table.
pub fn spawn_system_with_catalog(world: &mut World, wilderness: bool) -> EntityId {
    let id = if wilderness {
        world.ledger.spawn_wilderness_system()
    } else {
        world.ledger.spawn_system()
    };
    // Place roughly on a ring from RNG.
    let angle = (world.rng_u64() % 360) as f64;
    let radius = 10.0 + (world.rng_u64() % 90) as f64;
    roll_binding_stocks(world, id);
    let _ = matter::seed_deposits_from_binding_stocks(world, id, 1.0, ExtractorKind::State);
    if let Some(sys) = world.ledger.get_mut(id) {
        sys.x = radius * angle.to_radians().cos();
        sys.y = radius * angle.to_radians().sin();
    }
    let tick = world.master_tick;
    world.log.append(tick, EventKind::Spawn { system: id });
    id
}

/// Maintain live-system band: spawn wilderness frontiers until band met.
pub fn maintain_live_band(world: &mut World) {
    let target = world.globals.live_system_band;
    while count_live_systems(world) < target {
        spawn_system_with_catalog(world, true);
    }
}

/// Set empire capital/home: clear previous capital for that empire, then set new.
pub fn set_home_capital(world: &mut World, empire: EmpireId, system: EntityId) -> Result<(), String> {
    if world.ledger.get(system).is_none() {
        return Err(format!("system {system} not found"));
    }
    let tick = world.master_tick;
    // Clear previous capital(s) for this empire.
    let mut to_clear: Vec<EntityId> = world
        .ledger
        .systems()
        .filter_map(|(id, sys)| {
            if *id != system
                && (sys.home_empire == Some(empire)
                    || (sys.is_home_capital && sys.home_empire == Some(empire)))
            {
                Some(*id)
            } else if *id != system && sys.is_home_capital && sys.home_flag {
                // Also clear any capital tagged with this empire id via home_empire only.
                None
            } else {
                None
            }
        })
        .collect();
    // Also clear any system that claims to be capital of this empire.
    for (id, sys) in world.ledger.systems() {
        if *id != system && sys.home_empire == Some(empire) && !to_clear.contains(id) {
            to_clear.push(*id);
        }
    }
    for old in to_clear {
        if let Some(sys) = world.ledger.get_mut(old) {
            sys.is_home_capital = false;
            sys.home_flag = false;
            sys.home_empire = None;
            sys.sync_fuse_paused(world.globals.home_pause_enabled);
        }
        world
            .log
            .append(tick, EventKind::HomeFlagClear { system: old });
        world.handle_home_flag_clear(old);
    }
    {
        let enabled = world.globals.home_pause_enabled;
        let sys = world.ledger.get_mut(system).expect("checked");
        sys.is_home_capital = true;
        sys.home_flag = true;
        sys.home_empire = Some(empire);
        sys.sync_fuse_paused(enabled);
    }
    world
        .log
        .append(tick, EventKind::HomeFlagSet { system });
    Ok(())
}

/// Count map states (for CLI).

/// Bidirectional jump link (hidden-until-surveyed is Contact/G; B stores topology).
pub fn link_jump(world: &mut World, a: EntityId, b: EntityId) -> Result<(), String> {
    if a == b {
        return Err("cannot link a system to itself".into());
    }
    if world.ledger.get(a).is_none() {
        return Err(format!("system {a} not found"));
    }
    if world.ledger.get(b).is_none() {
        return Err(format!("system {b} not found"));
    }
    {
        let sys = world.ledger.get_mut(a).unwrap();
        if !sys.jump_links.contains(&b) {
            sys.jump_links.push(b);
        }
    }
    {
        let sys = world.ledger.get_mut(b).unwrap();
        if !sys.jump_links.contains(&a) {
            sys.jump_links.push(a);
        }
    }
    Ok(())
}

/// Euclidean map distance between system positions (sky stub coordinates).
pub fn system_distance(world: &World, a: EntityId, b: EntityId) -> Option<f64> {
    let sa = world.ledger.get(a)?;
    let sb = world.ledger.get(b)?;
    let dx = sa.x - sb.x;
    let dy = sa.y - sb.y;
    Some((dx * dx + dy * dy).sqrt())
}

/// Transfer ETA in master ticks: jump link ⇒ 1 tick; else ceil(distance) (min 1).
pub fn transfer_eta_ticks(world: &World, from: EntityId, to: EntityId) -> Option<u64> {
    if from == to {
        return Some(0);
    }
    let from_sys = world.ledger.get(from)?;
    if from_sys.jump_links.contains(&to) {
        return Some(1);
    }
    let dist = system_distance(world, from, to)?;
    Some(dist.ceil().max(1.0) as u64)
}

pub fn map_state_counts(world: &World) -> [(MapState, usize); 5] {
    let mut feed = 0;
    let mut dry = 0;
    let mut paused = 0;
    let mut ended = 0;
    let mut wild = 0;
    for (_, sys) in world.ledger().systems() {
        match map_state(sys) {
            MapState::Feed => feed += 1,
            MapState::DryFuse => dry += 1,
            MapState::HomePaused => paused += 1,
            MapState::Ended => ended += 1,
            MapState::WildernessUnknown => wild += 1,
        }
    }
    [
        (MapState::Feed, feed),
        (MapState::DryFuse, dry),
        (MapState::HomePaused, paused),
        (MapState::Ended, ended),
        (MapState::WildernessUnknown, wild),
    ]
}


/// Ticks per unit orbital radius (in-system clock). Bodies have no extra
/// schema fields — radius is 1 + stable index among bodies on the system.
pub const ORBIT_TICKS_PER_RADIUS: f64 = 10.0;

fn bodies_sorted(world: &World, system: EntityId) -> Vec<EntityId> {
    let mut ids: Vec<EntityId> = world
        .ledger
        .bodies_for_system(system)
        .map(|(id, _)| *id)
        .collect();
    ids.sort();
    ids
}

/// Stable 0-based orbit index for a body among siblings on its system.
pub fn orbit_index(world: &World, body: EntityId) -> Option<usize> {
    let b = world.ledger.get_body(body)?;
    bodies_sorted(world, b.system)
        .iter()
        .position(|&id| id == body)
}

/// Orbital radius stub: `1 + orbit_index`.
pub fn orbit_radius(world: &World, body: EntityId) -> Option<f64> {
    Some(1.0 + orbit_index(world, body)? as f64)
}

/// Period in master ticks: `ceil(radius * ORBIT_TICKS_PER_RADIUS)`, min 1.
pub fn orbit_period_ticks(world: &World, body: EntityId) -> Option<u64> {
    let r = orbit_radius(world, body)?;
    Some((r * ORBIT_TICKS_PER_RADIUS).ceil().max(1.0) as u64)
}

/// Absolute map XY of a body at `tick` (system XY + advancing orbit).
pub fn body_orbit_xy(world: &World, body: EntityId, tick: u64) -> Option<(f64, f64)> {
    let b = world.ledger.get_body(body)?;
    let sys = world.ledger.get(b.system)?;
    let r = orbit_radius(world, body)?;
    let period = orbit_period_ticks(world, body)? as f64;
    let phase = (body.0 % 360) as f64 * std::f64::consts::PI / 180.0;
    let ang = phase + (tick as f64) * 2.0 * std::f64::consts::PI / period;
    Some((sys.x + r * ang.cos(), sys.y + r * ang.sin()))
}

/// In-system transfer clock: `|r_a - r_b|` ceil ticks (0 if same body).
/// Different systems → None (use `transfer_eta_ticks` / `convoy_eta_ticks`).
pub fn in_system_transfer_eta(
    world: &World,
    from_body: EntityId,
    to_body: EntityId,
) -> Option<u64> {
    if from_body == to_body {
        return Some(0);
    }
    let a = world.ledger.get_body(from_body)?;
    let b = world.ledger.get_body(to_body)?;
    if a.system != b.system {
        return None;
    }
    let ra = orbit_radius(world, from_body)?;
    let rb = orbit_radius(world, to_body)?;
    Some((ra - rb).abs().ceil().max(1.0) as u64)
}

/// Patrol radius: max body orbit on the system, or 1.0 if no bodies.
pub fn patrol_radius(world: &World, system: EntityId) -> Option<f64> {
    world.ledger.get(system)?;
    let ids = bodies_sorted(world, system);
    let mut max_r = 0.0_f64;
    for id in ids {
        if let Some(r) = orbit_radius(world, id) {
            if r > max_r {
                max_r = r;
            }
        }
    }
    Some(if max_r > 0.0 { max_r } else { 1.0 })
}

/// Convoy clock: climb patrol at origin + inter-system transfer + drop at dest.
pub fn convoy_eta_ticks(world: &World, from: EntityId, to: EntityId) -> Option<u64> {
    let hop = transfer_eta_ticks(world, from, to)?;
    let climb = patrol_radius(world, from)?.ceil().max(0.0) as u64;
    let drop = patrol_radius(world, to)?.ceil().max(0.0) as u64;
    Some(hop.saturating_add(climb).saturating_add(drop))
}

/// Shortest jump-link path (BFS). Same system → `[from]`. No path → None.
pub fn jump_path(world: &World, from: EntityId, to: EntityId) -> Option<Vec<EntityId>> {
    world.ledger.get(from)?;
    world.ledger.get(to)?;
    if from == to {
        return Some(vec![from]);
    }
    let mut prev: std::collections::BTreeMap<EntityId, EntityId> =
        std::collections::BTreeMap::new();
    let mut q = std::collections::VecDeque::new();
    q.push_back(from);
    prev.insert(from, from);
    while let Some(cur) = q.pop_front() {
        let links = match world.ledger.get(cur) {
            Some(sys) => sys.jump_links.clone(),
            None => continue,
        };
        for nxt in links {
            if prev.contains_key(&nxt) {
                continue;
            }
            if world.ledger.get(nxt).is_none() {
                continue;
            }
            prev.insert(nxt, cur);
            if nxt == to {
                let mut path = vec![to];
                let mut walk = to;
                while walk != from {
                    walk = *prev.get(&walk)?;
                    path.push(walk);
                }
                path.reverse();
                return Some(path);
            }
            q.push_back(nxt);
        }
    }
    None
}

/// Jump-network ETA: one tick per hop (0 if same system). None if unlinkable.
pub fn jump_eta_ticks(world: &World, from: EntityId, to: EntityId) -> Option<u64> {
    let path = jump_path(world, from, to)?;
    Some(path.len().saturating_sub(1) as u64)
}

#[cfg(test)]
mod sky_tests {
    use super::*;
    use crate::entity::EmpireId;
    use crate::event::EventKind;
    use crate::globals::Globals;
    use crate::lod::LodMode;
    use crate::operator::Operator;
    use crate::world::World;

    #[test]
    fn wilderness_immortal_until_claim_or_survey() {
        let mut w = World::new(10);
        let id = w.ledger.spawn_wilderness_system();
        {
            let s = w.ledger.get_mut(id).unwrap();
            s.binding_remainder = 0.0;
        }
        assert!(!check_depletion(&mut w, id), "immortal wilderness must not deplete");
        assert!(!w.ledger.get(id).unwrap().depleted);

        survey_system(&mut w, id);
        // surveyed ends immortality even if wilderness flag remains
        assert!(check_depletion(&mut w, id));
        assert!(w.ledger.get(id).unwrap().depleted);
    }

    #[test]
    fn claim_alone_allows_depletion() {
        let mut w = World::new(11);
        let id = w.ledger.spawn_wilderness_system();
        {
            let s = w.ledger.get_mut(id).unwrap();
            s.binding_remainder = 0.0;
        }
        assert!(!check_depletion(&mut w, id));
        claim_system(&mut w, id);
        assert!(check_depletion(&mut w, id), "claim alone ends immortality");
        assert!(w.ledger.get(id).unwrap().depleted);
        assert!(w.ledger.get(id).unwrap().fuse_end_tick.is_some());
    }

    #[test]
    fn deplete_arms_fuse_from_globals_ratio() {
        let mut g = Globals::default();
        g.master_era_length = 1000;
        g.fuse_length_ratio = 0.25; // 250
        g.dry_threshold = 1.0;
        let mut w = World::with_globals(12, g);
        let id = w.ledger.spawn_system();
        {
            let s = w.ledger.get_mut(id).unwrap();
            s.binding_remainder = 0.5;
        }
        assert!(check_depletion(&mut w, id));
        let sys = w.ledger.get(id).unwrap();
        assert_eq!(sys.fuse_end_tick, Some(250));
        assert!(w.log.events().iter().any(|e| matches!(e.kind, EventKind::Deplete { system } if system == id)));
        assert!(w.log.events().iter().any(|e| matches!(
            &e.kind,
            EventKind::FuseArmed { system, end_tick: 250 } if *system == id
        )));
    }

    #[test]
    fn remnant_set_on_fuse_end() {
        let mut w = World::new(13);
        let id = w.ledger.spawn_system();
        {
            let mut op = Operator::new(&mut w);
            op.arm_fuse(id, 5).unwrap();
        }
        w.tick(5);
        let sys = w.ledger.get(id).unwrap();
        assert!(sys.ended);
        assert!(sys.remnant_harvest.is_some());
        assert!(sys.fuse_end_tick.is_none());
        assert!(w.log.events().iter().any(|e| matches!(e.kind, EventKind::FuseEnd { system } if system == id)));
    }

    #[test]
    fn home_pause_prevents_fuse_end_while_flag_held() {
        let mut g = Globals::default();
        g.home_pause_enabled = true;
        let mut w = World::with_globals(14, g);
        let id = w.ledger.spawn_system();
        set_home_capital(&mut w, EmpireId(0), id).unwrap();
        {
            let mut op = Operator::new(&mut w);
            op.arm_fuse(id, 3).unwrap();
        }
        // Ensure pause sync
        apply_fuse_pause_slide(&mut w, 0);
        assert!(w.ledger.get(id).unwrap().fuse_paused);
        let end_before = w.ledger.get(id).unwrap().fuse_end_tick.unwrap();
        w.tick(5);
        let sys = w.ledger.get(id).unwrap();
        assert!(!sys.ended, "paused capital must not end");
        assert!(sys.fuse_end_tick.is_some());
        assert!(sys.fuse_end_tick.unwrap() > end_before, "end tick must slide while paused");
        // Clear home flag → countdown resumes
        {
            let s = w.ledger.get_mut(id).unwrap();
            s.home_flag = false;
            s.sync_fuse_paused(true);
        }
        let end_now = w.ledger.get(id).unwrap().fuse_end_tick.unwrap();
        let need = end_now.saturating_sub(w.master_tick()) + 1;
        w.tick(need);
        assert!(w.ledger.get(id).unwrap().ended);
    }

    #[test]
    fn spawn_uses_full_catalog_ids() {
        let mut w = World::new(15);
        let id = spawn_system_with_catalog(&mut w, false);
        let stocks = &w.ledger.get(id).unwrap().binding_stocks;
        let catalog = embedded_catalog();
        for sid in catalog.spawn_stock_ids() {
            assert!(
                stocks.contains_key(sid),
                "spawn missing catalog stock {sid}"
            );
        }
        assert_eq!(stocks.len(), catalog.spawn_stock_ids().len());
    }

    #[test]
    fn one_capital_enforced() {
        let mut w = World::new(16);
        let ids: Vec<_> = w.ledger.systems().map(|(i, _)| *i).take(2).map(|x| x).collect();
        assert!(ids.len() >= 2);
        let a = ids[0];
        let b = ids[1];
        let empire = EmpireId(1);
        set_home_capital(&mut w, empire, a).unwrap();
        assert!(w.ledger.get(a).unwrap().is_home_capital);
        set_home_capital(&mut w, empire, b).unwrap();
        assert!(!w.ledger.get(a).unwrap().is_home_capital);
        assert!(!w.ledger.get(a).unwrap().home_flag);
        assert!(w.ledger.get(b).unwrap().is_home_capital);
        assert_eq!(w.ledger.get(b).unwrap().home_empire, Some(empire));
    }

    #[test]
    fn map_state_home_paused() {
        let mut w = World::new(17);
        let id = w.ledger.spawn_system();
        set_home_capital(&mut w, EmpireId(9), id).unwrap();
        {
            let mut op = Operator::new(&mut w);
            op.arm_fuse(id, 100).unwrap();
        }
        apply_fuse_pause_slide(&mut w, 0);
        assert_eq!(map_state(w.ledger.get(id).unwrap()), MapState::HomePaused);
    }

    #[test]
    fn coarse_lod_fuse_end_still_monotonic_via_sky() {
        let mut globals = Globals::default();
        globals.coarse_dt = 10;
        let mut w = World::with_globals(18, globals);
        w.set_lod(LodMode::Coarse);
        let id = w.ledger.spawn_system();
        {
            let mut op = Operator::new(&mut w);
            op.arm_fuse(id, 25).unwrap();
        }
        w.tick(2);
        assert_eq!(w.master_tick(), 20);
        w.tick(1);
        assert_eq!(w.master_tick(), 30);
        let ends = w
            .log
            .events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::FuseEnd { system } if system == id))
            .count();
        assert_eq!(ends, 1);
        assert!(w.ledger.get(id).unwrap().ended);
        assert!(w.ledger.get(id).unwrap().remnant_harvest.is_some());
    }

    #[test]
    fn jump_link_and_transfer_eta() {
        let mut w = World::new(501);
        let a = spawn_system_with_catalog(&mut w, false);
        let b = spawn_system_with_catalog(&mut w, false);
        // place far apart
        w.ledger.get_mut(a).unwrap().x = 0.0;
        w.ledger.get_mut(a).unwrap().y = 0.0;
        w.ledger.get_mut(b).unwrap().x = 30.0;
        w.ledger.get_mut(b).unwrap().y = 40.0; // dist 50
        let eta_coast = transfer_eta_ticks(&w, a, b).unwrap();
        assert_eq!(eta_coast, 50);
        link_jump(&mut w, a, b).unwrap();
        assert!(w.ledger.get(a).unwrap().jump_links.contains(&b));
        assert!(w.ledger.get(b).unwrap().jump_links.contains(&a));
        assert_eq!(transfer_eta_ticks(&w, a, b).unwrap(), 1);
    }

    #[test]
    fn orbits_advance_and_convoy_eta() {
        let mut w = World::new(700);
        let sys = spawn_system_with_catalog(&mut w, false);
        w.ledger.get_mut(sys).unwrap().x = 0.0;
        w.ledger.get_mut(sys).unwrap().y = 0.0;
        let inner = w.ledger.spawn_body(sys);
        let outer = w.ledger.spawn_body(sys);
        assert_eq!(orbit_index(&w, inner), Some(0));
        assert_eq!(orbit_index(&w, outer), Some(1));
        assert_eq!(orbit_radius(&w, inner).unwrap(), 1.0);
        assert_eq!(orbit_radius(&w, outer).unwrap(), 2.0);
        assert_eq!(orbit_period_ticks(&w, inner).unwrap(), 10);
        let p0 = body_orbit_xy(&w, inner, 0).unwrap();
        let p1 = body_orbit_xy(&w, inner, 5).unwrap();
        assert!((p0.0 - p1.0).abs() + (p0.1 - p1.1).abs() > 1e-6, "orbit must advance");
        assert_eq!(in_system_transfer_eta(&w, inner, inner).unwrap(), 0);
        assert_eq!(in_system_transfer_eta(&w, inner, outer).unwrap(), 1);
        assert!((patrol_radius(&w, sys).unwrap() - 2.0).abs() < 1e-12);

        let dest = spawn_system_with_catalog(&mut w, false);
        w.ledger.get_mut(dest).unwrap().x = 30.0;
        w.ledger.get_mut(dest).unwrap().y = 40.0;
        let coast = transfer_eta_ticks(&w, sys, dest).unwrap();
        let convoy = convoy_eta_ticks(&w, sys, dest).unwrap();
        assert!(convoy >= coast + 2, "convoy includes climb+drop patrol");
        assert!(jump_path(&w, sys, dest).is_none());
        assert!(jump_eta_ticks(&w, sys, dest).is_none());
        let mid = spawn_system_with_catalog(&mut w, false);
        link_jump(&mut w, sys, mid).unwrap();
        link_jump(&mut w, mid, dest).unwrap();
        let path = jump_path(&w, sys, dest).unwrap();
        assert_eq!(path, vec![sys, mid, dest]);
        assert_eq!(jump_eta_ticks(&w, sys, dest).unwrap(), 2);
        assert_eq!(jump_eta_ticks(&w, sys, sys).unwrap(), 0);
    }

}
