//! Entity ledger — single shared store for tick + operator.
//!
//! Systems (Phase B sky) + empires/orders (Phase I minds) share one id space.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::globals::Globals;
use crate::lod::LodHint;

/// Stable entity identifier (systems, empires, orders, KOs share this newtype).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u64);

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Stable empire / polity identifier (G/P). Often mirrored as EntityId for ledger empires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EmpireId(pub u64);

impl std::fmt::Display for EmpireId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "empire:{}", self.0)
    }
}

impl From<EntityId> for EmpireId {
    fn from(id: EntityId) -> Self {
        EmpireId(id.0)
    }
}

impl From<EmpireId> for EntityId {
    fn from(id: EmpireId) -> Self {
        EntityId(id.0)
    }
}

fn clamp01(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// System entity schema (Phase B sky fields + Kernel A fuse ledger).
///
/// Home pause (Lock 4): while `fuse_paused`, tick slides `fuse_end_tick`
/// forward by `dt` so the absolute countdown freezes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemEntity {
    pub id: EntityId,
    pub binding_remainder: f64,
    pub depleted: bool,
    pub fuse_end_tick: Option<u64>,
    pub fuse_remaining: Option<u64>,
    pub is_home_capital: bool,
    pub home_flag: bool,
    pub lod_hint: LodHint,
    #[serde(default)]
    pub wilderness: bool,
    #[serde(default)]
    pub surveyed: bool,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub fuse_paused: bool,
    #[serde(default)]
    pub ended: bool,
    #[serde(default)]
    pub remnant_harvest: Option<f64>,
    #[serde(default)]
    pub home_empire: Option<EmpireId>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub jump_links: Vec<EntityId>,
    #[serde(default)]
    pub binding_stocks: BTreeMap<String, f64>,
    /// Phase C deposits (C owns; remainder aggregated from binding deposits).
    #[serde(default)]
    pub deposits: Vec<crate::matter::Deposit>,
    /// Salvage / recycling feed-pipe stock (no vein refill).
    #[serde(default)]
    pub salvage_stock: f64,
}

impl Default for SystemEntity {
    fn default() -> Self {
        Self::new(EntityId(0))
    }
}

impl SystemEntity {
    /// Seeded / playable system: claimed+surveyed, not immortal wilderness.
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            binding_remainder: 1000.0,
            depleted: false,
            fuse_end_tick: None,
            fuse_remaining: None,
            is_home_capital: false,
            home_flag: false,
            lod_hint: LodHint::Quiet,
            wilderness: false,
            surveyed: true,
            claimed: true,
            fuse_paused: false,
            ended: false,
            remnant_harvest: None,
            home_empire: None,
            x: 0.0,
            y: 0.0,
            jump_links: Vec::new(),
            binding_stocks: BTreeMap::new(),
            deposits: Vec::new(),
            salvage_stock: 0.0,
        }
    }

    pub fn new_wilderness(id: EntityId) -> Self {
        let mut s = Self::new(id);
        s.wilderness = true;
        s.surveyed = false;
        s.claimed = false;
        s
    }

    pub fn is_wilderness_immortal(&self) -> bool {
        self.wilderness && !(self.surveyed || self.claimed)
    }

    pub fn sync_fuse_remaining(&mut self, master_tick: u64) {
        self.fuse_remaining = self
            .fuse_end_tick
            .map(|end| end.saturating_sub(master_tick));
    }

    pub fn sync_fuse_paused(&mut self, home_pause_enabled: bool) {
        self.fuse_paused =
            self.is_home_capital && self.home_flag && home_pause_enabled && !self.ended;
    }

    pub fn fuse_crossed_end(&self, master_tick_before: u64, dt: u64) -> bool {
        match self.fuse_end_tick {
            Some(end) => {
                let after = master_tick_before.saturating_add(dt);
                master_tick_before < end && after >= end
            }
            None => false,
        }
    }
}

/// Empire entity (Phase I) — operator-writable doctrine; one mind per empire v1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmpireEntity {
    pub id: EntityId,
    pub name: Option<String>,
    pub salt_willingness: f64,
    pub punishment_willingness: f64,
    pub evacuate_vs_die_in_place: f64,
    /// Phase E: researched segment ids (empire progress; never mutates cosmology).
    #[serde(default)]
    pub unlocked_segments: BTreeSet<String>,
    /// Phase E: unlocked catalog module/facility/recipe ids.
    #[serde(default)]
    pub unlocked_catalog_ids: BTreeSet<String>,
    /// Salvage jumps with incomplete stats.
    #[serde(default)]
    pub incomplete_stat_segments: BTreeSet<String>,
    #[serde(default)]
    pub tooled_design_ids: BTreeSet<u64>,
}

impl EmpireEntity {
    pub fn from_defaults(id: EntityId, globals: &Globals, name: Option<String>) -> Self {
        Self {
            id,
            name,
            salt_willingness: clamp01(globals.salt_willingness),
            punishment_willingness: clamp01(globals.punishment_willingness),
            evacuate_vs_die_in_place: clamp01(globals.evacuate_vs_die_in_place),
            unlocked_segments: BTreeSet::new(),
            unlocked_catalog_ids: BTreeSet::new(),
            incomplete_stat_segments: BTreeSet::new(),
            tooled_design_ids: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderIntent {
    ExpandSurvey,
    ClaimFeed,
    PlantCity,
    PlantYard,
    StripMine,
    Fortify,
    Evacuate,
    Abandon,
    ProsecuteAtrocity,
    SaltWorld,
    PunishSalter,
}

impl OrderIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExpandSurvey => "expand_survey",
            Self::ClaimFeed => "claim_feed",
            Self::PlantCity => "plant_city",
            Self::PlantYard => "plant_yard",
            Self::StripMine => "strip_mine",
            Self::Fortify => "fortify",
            Self::Evacuate => "evacuate",
            Self::Abandon => "abandon",
            Self::ProsecuteAtrocity => "prosecute_atrocity",
            Self::SaltWorld => "salt_world",
            Self::PunishSalter => "punish_salter",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "expand_survey" | "expandsurvey" => Some(Self::ExpandSurvey),
            "claim_feed" | "claimfeed" => Some(Self::ClaimFeed),
            "plant_city" | "plantcity" => Some(Self::PlantCity),
            "plant_yard" | "plantyard" => Some(Self::PlantYard),
            "strip_mine" | "stripmine" => Some(Self::StripMine),
            "fortify" => Some(Self::Fortify),
            "evacuate" => Some(Self::Evacuate),
            "abandon" => Some(Self::Abandon),
            "prosecute_atrocity" | "prosecuteatrocity" => Some(Self::ProsecuteAtrocity),
            "salt_world" | "saltworld" => Some(Self::SaltWorld),
            "punish_salter" | "punishsalter" => Some(Self::PunishSalter),
            _ => None,
        }
    }
}

impl std::fmt::Display for OrderIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Queued,
    Active,
    Done,
    Cancelled,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Active => "active",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSource {
    Ai,
    Operator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderEntity {
    pub id: EntityId,
    pub empire_id: EntityId,
    pub intent: OrderIntent,
    pub target_ref: Option<EntityId>,
    pub status: OrderStatus,
    pub created_tick: u64,
    pub updated_tick: u64,
    pub source: OrderSource,
}


/// Five writable environment layers (Phase D). Weapons (H) and optional fuse-end
/// bursts (B) write the same columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvLayers {
    pub atmosphere_pressure: f64,
    pub temperature: f64,
    pub radiation: f64,
    pub toxins_fallout: f64,
    pub biosphere: f64,
}

impl Default for EnvLayers {
    fn default() -> Self {
        Self {
            atmosphere_pressure: 1.0,
            temperature: 288.0,
            radiation: 0.0,
            toxins_fallout: 0.0,
            biosphere: 1.0,
        }
    }
}

/// Body / colony stub on a system (Phase D).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyEntity {
    pub id: EntityId,
    pub system: EntityId,
    pub layers: EnvLayers,
    pub pops: f64,
    /// When true and pops==0, C still drains binding (Lock 8); no life-support bill.
    pub automation_active: bool,
    pub structure_soak: f64,
    pub power_soak: f64,
    pub upkeep_soak: f64,
}

impl BodyEntity {
    pub fn new(id: EntityId, system: EntityId) -> Self {
        Self {
            id,
            system,
            layers: EnvLayers::default(),
            pops: 0.0,
            automation_active: false,
            structure_soak: 0.0,
            power_soak: 0.0,
            upkeep_soak: 0.0,
        }
    }
}

/// Single shared entity store — systems + empires + orders + bodies, one id space.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityLedger {
    next_id: u64,
    systems: BTreeMap<EntityId, SystemEntity>,
    #[serde(default)]
    empires: BTreeMap<EntityId, EmpireEntity>,
    #[serde(default)]
    orders: BTreeMap<EntityId, OrderEntity>,
    #[serde(default)]
    bodies: BTreeMap<EntityId, BodyEntity>,
}

impl EntityLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_id(&mut self) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id += 1;
        id
    }

    fn bump_next(&mut self, id: EntityId) {
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
    }

    pub fn spawn_system(&mut self) -> EntityId {
        let id = self.alloc_id();
        self.systems.insert(id, SystemEntity::new(id));
        id
    }

    pub fn spawn_wilderness_system(&mut self) -> EntityId {
        let id = self.alloc_id();
        self.systems.insert(id, SystemEntity::new_wilderness(id));
        id
    }

    pub fn insert(&mut self, entity: SystemEntity) {
        self.bump_next(entity.id);
        self.systems.insert(entity.id, entity);
    }

    pub fn get(&self, id: EntityId) -> Option<&SystemEntity> {
        self.systems.get(&id)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut SystemEntity> {
        self.systems.get_mut(&id)
    }

    pub fn systems(&self) -> impl Iterator<Item = (&EntityId, &SystemEntity)> {
        self.systems.iter()
    }

    pub fn systems_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut SystemEntity)> {
        self.systems.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.systems.len()
    }

    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
            && self.empires.is_empty()
            && self.orders.is_empty()
            && self.bodies.is_empty()
    }

    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    pub fn capital_of(&self, empire: EmpireId) -> Option<EntityId> {
        self.systems.iter().find_map(|(id, sys)| {
            if sys.is_home_capital && sys.home_empire == Some(empire) {
                Some(*id)
            } else {
                None
            }
        })
    }

    // --- Empires ---

    pub fn spawn_empire(&mut self, globals: &Globals, name: Option<String>) -> EntityId {
        let id = self.alloc_id();
        self.empires
            .insert(id, EmpireEntity::from_defaults(id, globals, name));
        id
    }

    pub fn insert_empire(&mut self, entity: EmpireEntity) {
        self.bump_next(entity.id);
        self.empires.insert(entity.id, entity);
    }

    pub fn get_empire(&self, id: EntityId) -> Option<&EmpireEntity> {
        self.empires.get(&id)
    }

    pub fn get_empire_mut(&mut self, id: EntityId) -> Option<&mut EmpireEntity> {
        self.empires.get_mut(&id)
    }

    pub fn empires(&self) -> impl Iterator<Item = (&EntityId, &EmpireEntity)> {
        self.empires.iter()
    }

    pub fn empires_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut EmpireEntity)> {
        self.empires.iter_mut()
    }

    pub fn empires_len(&self) -> usize {
        self.empires.len()
    }

    // --- Orders ---

    pub fn spawn_order(
        &mut self,
        empire_id: EntityId,
        intent: OrderIntent,
        target_ref: Option<EntityId>,
        tick: u64,
        source: OrderSource,
    ) -> EntityId {
        let id = self.alloc_id();
        self.orders.insert(
            id,
            OrderEntity {
                id,
                empire_id,
                intent,
                target_ref,
                status: OrderStatus::Queued,
                created_tick: tick,
                updated_tick: tick,
                source,
            },
        );
        id
    }

    pub fn insert_order(&mut self, entity: OrderEntity) {
        self.bump_next(entity.id);
        self.orders.insert(entity.id, entity);
    }

    pub fn get_order(&self, id: EntityId) -> Option<&OrderEntity> {
        self.orders.get(&id)
    }

    pub fn get_order_mut(&mut self, id: EntityId) -> Option<&mut OrderEntity> {
        self.orders.get_mut(&id)
    }

    pub fn orders(&self) -> impl Iterator<Item = (&EntityId, &OrderEntity)> {
        self.orders.iter()
    }

    pub fn orders_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut OrderEntity)> {
        self.orders.iter_mut()
    }

    pub fn orders_len(&self) -> usize {
        self.orders.len()
    }

    // --- Bodies (Phase D) ---

    pub fn spawn_body(&mut self, system: EntityId) -> EntityId {
        let id = self.alloc_id();
        self.bodies.insert(id, BodyEntity::new(id, system));
        id
    }

    pub fn insert_body(&mut self, entity: BodyEntity) {
        self.bump_next(entity.id);
        self.bodies.insert(entity.id, entity);
    }

    pub fn get_body(&self, id: EntityId) -> Option<&BodyEntity> {
        self.bodies.get(&id)
    }

    pub fn get_body_mut(&mut self, id: EntityId) -> Option<&mut BodyEntity> {
        self.bodies.get_mut(&id)
    }

    pub fn bodies(&self) -> impl Iterator<Item = (&EntityId, &BodyEntity)> {
        self.bodies.iter()
    }

    pub fn bodies_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut BodyEntity)> {
        self.bodies.iter_mut()
    }

    pub fn bodies_len(&self) -> usize {
        self.bodies.len()
    }

    pub fn bodies_for_system(&self, system: EntityId) -> impl Iterator<Item = (&EntityId, &BodyEntity)> {
        self.bodies.iter().filter(move |(_, b)| b.system == system)
    }
}
