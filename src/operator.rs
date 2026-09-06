//! Operator R/W hooks — inspect/mutate any ledger record; every mutation is an event.

use thiserror::Error;

use crate::entity::{EntityId, OrderIntent, OrderSource, SystemEntity};
use crate::event::EventKind;
use crate::lod::LodHint;
use crate::matter::{self, CivilianLine, Deposit, ExtractorKind};
use crate::minds::{self, set_empire_doctrine_field};
use crate::world::World;

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("entity {0} not found")]
    NotFound(EntityId),
    #[error("unknown field '{0}'")]
    UnknownField(String),
    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },
    #[error("{0}")]
    Other(String),
}

/// Read/write surface over the shared ledger. Mutations always append events.
pub struct Operator<'a> {
    world: &'a mut World,
}

impl<'a> Operator<'a> {
    pub fn new(world: &'a mut World) -> Self {
        Self { world }
    }

    pub fn inspect(&self, id: EntityId) -> Result<&SystemEntity, OperatorError> {
        self.world
            .ledger()
            .get(id)
            .ok_or(OperatorError::NotFound(id))
    }

    pub fn list_ids(&self) -> Vec<EntityId> {
        self.world.ledger().systems().map(|(id, _)| *id).collect()
    }

    /// Mutate a named field on a system entity, or empire doctrine by id.
    /// Every successful mutation is logged.
    pub fn set_field(
        &mut self,
        id: EntityId,
        field: &str,
        value: &str,
    ) -> Result<(), OperatorError> {
        // Doctrine fields may target an empire id when no system matches.
        if matches!(
            field,
            "salt_willingness" | "punishment_willingness" | "evacuate_vs_die_in_place"
        ) && self.world.ledger().get(id).is_none()
        {
            return self.set_empire_doctrine(id, field, value);
        }

        let tick = self.world.master_tick();
        let entity = self
            .world
            .ledger_mut()
            .get_mut(id)
            .ok_or(OperatorError::NotFound(id))?;

        let (old, new) = match field {
            "binding_remainder" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = entity.binding_remainder.to_string();
                entity.binding_remainder = v;
                (old, v.to_string())
            }
            "depleted" => {
                let v: bool = parse_bool(value).ok_or_else(|| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected bool".into(),
                })?;
                let old = entity.depleted.to_string();
                entity.depleted = v;
                (old, v.to_string())
            }
            "fuse_end_tick" => {
                let old = format!("{:?}", entity.fuse_end_tick);
                if value.eq_ignore_ascii_case("none") || value.is_empty() {
                    entity.fuse_end_tick = None;
                    entity.fuse_remaining = None;
                    (old, "None".into())
                } else {
                    let v: u64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                        field: field.into(),
                        reason: "expected u64 or none".into(),
                    })?;
                    entity.fuse_end_tick = Some(v);
                    entity.sync_fuse_remaining(tick);
                    (old, v.to_string())
                }
            }
            "is_home_capital" => {
                let v: bool = parse_bool(value).ok_or_else(|| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected bool".into(),
                })?;
                let old = entity.is_home_capital.to_string();
                entity.is_home_capital = v;
                (old, v.to_string())
            }
            "home_flag" => {
                let v: bool = parse_bool(value).ok_or_else(|| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected bool".into(),
                })?;
                let old = entity.home_flag.to_string();
                entity.home_flag = v;
                (old, v.to_string())
            }
            "lod_hint" => {
                let v = match value.to_ascii_lowercase().as_str() {
                    "quiet" => LodHint::Quiet,
                    "hot" => LodHint::Hot,
                    _ => {
                        return Err(OperatorError::InvalidValue {
                            field: field.into(),
                            reason: "expected quiet|hot".into(),
                        })
                    }
                };
                let old = format!("{:?}", entity.lod_hint);
                entity.lod_hint = v;
                (old, format!("{:?}", v))
            }
            "salt_willingness" | "punishment_willingness" | "evacuate_vs_die_in_place" => {
                return Err(OperatorError::UnknownField(format!(
                    "{field} (use empire id / set_empire_doctrine)"
                )));
            }
            other => return Err(OperatorError::UnknownField(other.into())),
        };

        // Special-case home flag events for Phase B stubs.
        let cleared_home = field == "home_flag" && new != "true";
        let home_event = if field == "home_flag" {
            if new == "true" {
                Some(EventKind::HomeFlagSet { system: id })
            } else {
                Some(EventKind::HomeFlagClear { system: id })
            }
        } else {
            None
        };

        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: id,
                field: field.into(),
                old,
                new,
            },
        );

        if let Some(ev) = home_event {
            self.world.log_mut().append(tick, ev);
        }

        // Phase I: same-tick capital re-score after HomeFlagClear.
        if cleared_home {
            self.world.handle_home_flag_clear(id);
        }

        Ok(())
    }

    /// Set an empire doctrine field (clamped 0..1). Logs OperatorMutation.
    pub fn set_empire_doctrine(
        &mut self,
        empire_id: EntityId,
        field: &str,
        value: &str,
    ) -> Result<(), OperatorError> {
        let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
            field: field.into(),
            reason: "expected f64".into(),
        })?;
        let tick = self.world.master_tick();
        let (old, new) = set_empire_doctrine_field(&mut self.world.ledger, empire_id, field, v)
            .map_err(|e| {
                if e.contains("not found") {
                    OperatorError::NotFound(empire_id)
                } else {
                    OperatorError::UnknownField(e)
                }
            })?;
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: empire_id,
                field: field.into(),
                old,
                new,
            },
        );
        self.world.recompute_outcome_hash();
        Ok(())
    }

    /// Issue an order through try_emit_order (respects salt emit flags / KO gate).
    pub fn issue_order(
        &mut self,
        empire_id: EntityId,
        intent: &str,
        target: Option<EntityId>,
    ) -> Result<Option<EntityId>, OperatorError> {
        let intent = OrderIntent::parse(intent).ok_or_else(|| OperatorError::InvalidValue {
            field: "intent".into(),
            reason: format!("unknown intent '{intent}'"),
        })?;
        minds::try_emit_order(
            self.world,
            empire_id,
            intent,
            target,
            OrderSource::Operator,
        )
        .map_err(OperatorError::Other)
    }

    /// Force-arm a fuse ending at an absolute master tick (stub for B).


    /// Spawn a research lab for an empire (Phase E).
    pub fn spawn_lab(&mut self, empire_id: EntityId, capacity: f64) -> Result<EntityId, OperatorError> {
        if self.world.ledger().get_empire(empire_id).is_none() {
            return Err(OperatorError::NotFound(empire_id));
        }
        let id = self.world.ledger_mut().alloc_id();
        let lab = crate::research::make_lab(id, empire_id, capacity);
        self.world.labs.insert(id, lab);
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: id,
                field: "lab_spawned".into(),
                old: String::new(),
                new: format!("empire={empire_id} capacity={capacity}"),
            },
        );
        Ok(id)
    }


    /// Bind a lab to a system for material_gates checks.
    pub fn set_lab_research_site(
        &mut self,
        lab_id: EntityId,
        system: Option<EntityId>,
    ) -> Result<(), OperatorError> {
        let lab = self.world.labs.get_mut(&lab_id).ok_or(OperatorError::NotFound(lab_id))?;
        crate::research::set_lab_site(lab, system);
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: lab_id,
                field: "site_system".into(),
                old: String::new(),
                new: format!("{system:?}"),
            },
        );
        Ok(())
    }


    /// Repair a salvage-incomplete segment via lab RP (clears incomplete_stat_segments).
    pub fn repair_salvage_research(
        &mut self,
        lab_id: EntityId,
        segment_id: &str,
        dt: u64,
    ) -> Result<bool, OperatorError> {
        let globals = self.world.globals.clone();
        let empire_id = self
            .world
            .labs
            .get(&lab_id)
            .map(|l| l.empire_id)
            .ok_or(OperatorError::NotFound(lab_id))?;
        let capacity = self
            .world
            .labs
            .get(&lab_id)
            .map(|l| l.capacity)
            .ok_or(OperatorError::NotFound(lab_id))?;
        let mut progress = self.world.labs.get(&lab_id).unwrap().progress_rp;
        let done = {
            let empire = self
                .world
                .ledger_mut()
                .get_empire_mut(empire_id)
                .ok_or(OperatorError::NotFound(empire_id))?;
            crate::research::repair_salvage_segment(
                empire, segment_id, &globals, capacity, &mut progress, dt,
            )
            .map_err(OperatorError::Other)?
        };
        if let Some(lab) = self.world.labs.get_mut(&lab_id) {
            lab.progress_rp = progress;
            if done {
                lab.assigned_segment = None;
            }
        }
        if done {
            let tick = self.world.master_tick();
            self.world.log_mut().append(
                tick,
                EventKind::OperatorMutation {
                    entity: lab_id,
                    field: "salvage_repaired".into(),
                    old: String::new(),
                    new: segment_id.into(),
                },
            );
        }
        Ok(done)
    }

    /// Salvage-jump unlock a segment (incomplete stats flag; no RNG).
    pub fn salvage_research_segment(
        &mut self,
        empire_id: EntityId,
        segment_id: &str,
    ) -> Result<(), OperatorError> {
        let seg = crate::research::find_segment(segment_id)
            .ok_or_else(|| OperatorError::Other(format!("unknown segment {segment_id}")))?;
        let empire = self
            .world
            .ledger_mut()
            .get_empire_mut(empire_id)
            .ok_or(OperatorError::NotFound(empire_id))?;
        crate::research::salvage_unlock_segment(empire, &seg).map_err(OperatorError::Other)?;
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::SegmentResearched {
                empire: empire_id,
                segment: segment_id.into(),
            },
        );
        Ok(())
    }

    /// Assign a lab to a research segment id.
    pub fn assign_research(&mut self, lab_id: EntityId, segment_id: &str) -> Result<(), OperatorError> {
        let lab = self.world.labs.get_mut(&lab_id).ok_or(OperatorError::NotFound(lab_id))?;
        crate::research::assign_lab(lab, segment_id).map_err(OperatorError::Other)?;
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: lab_id,
                field: "assigned_segment".into(),
                old: String::new(),
                new: segment_id.into(),
            },
        );
        Ok(())
    }

    /// Register a ship design (modules must be unlocked).
    pub fn register_ship_design(
        &mut self,
        empire_id: EntityId,
        name: &str,
        modules: Vec<String>,
    ) -> Result<EntityId, OperatorError> {
        crate::hulls::register_design(self.world, empire_id, name, modules)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }


    /// Tool yard consuming recipe.yard_mk1 at system.
    pub fn tool_ship_yard_at_system(
        &mut self,
        empire_id: EntityId,
        design_id: EntityId,
        system: EntityId,
    ) -> Result<(), OperatorError> {
        crate::hulls::tool_yard_at_system(self.world, empire_id, design_id, system)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }



    /// Apply empire facility soak boosts to a body.
    pub fn apply_body_facility_soaks(
        &mut self,
        body_id: EntityId,
        empire_id: EntityId,
    ) -> Result<(), OperatorError> {
        let empire = self
            .world
            .ledger()
            .get_empire(empire_id)
            .ok_or(OperatorError::NotFound(empire_id))?
            .clone();
        let body = self
            .world
            .ledger_mut()
            .get_body_mut(body_id)
            .ok_or(OperatorError::NotFound(body_id))?;
        crate::worlds::apply_facility_soaks(body, &empire);
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: body_id,
                field: "facility_soaks".into(),
                old: String::new(),
                new: format!("empire={empire_id}"),
            },
        );
        Ok(())
    }

    /// Evacuate a body (clear pops; optional ash automation).
    pub fn evacuate_colony(
        &mut self,
        body_id: EntityId,
        leave_automation: bool,
    ) -> Result<(), OperatorError> {
        let body = self
            .world
            .ledger_mut()
            .get_body_mut(body_id)
            .ok_or(OperatorError::NotFound(body_id))?;
        crate::worlds::evacuate_body(body, leave_automation);
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: body_id,
                field: "evacuate".into(),
                old: String::new(),
                new: format!("leave_automation={leave_automation}"),
            },
        );
        Ok(())
    }




    /// Cargo capacity from design cargo_hold modules.
    pub fn ship_cargo_capacity(&self, ship_id: EntityId) -> Result<f64, OperatorError> {
        let ship = self
            .world
            .ships
            .get(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        let design = self
            .world
            .ship_designs
            .get(&ship.design_id)
            .ok_or(OperatorError::NotFound(ship.design_id))?;
        Ok(crate::hulls::cargo_capacity(design))
    }

    /// Whether a ship can sense (sensor module + not wrecked).
    pub fn ship_can_sense(&self, ship_id: EntityId) -> Result<bool, OperatorError> {
        let ship = self
            .world
            .ships
            .get(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        let design = self
            .world
            .ship_designs
            .get(&ship.design_id)
            .ok_or(OperatorError::NotFound(ship.design_id))?;
        Ok(crate::hulls::can_sense(design, ship))
    }

    /// Fire ship kinetic weapon (spends magazine).
    pub fn fire_ship_kinetic(
        &mut self,
        ship_id: EntityId,
        ammo_id: &str,
    ) -> Result<f64, OperatorError> {
        let design_id = self
            .world
            .ships
            .get(&ship_id)
            .map(|s| s.design_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        let design = self
            .world
            .ship_designs
            .get(&design_id)
            .ok_or(OperatorError::NotFound(design_id))?
            .clone();
        let ship = self
            .world
            .ships
            .get_mut(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        crate::hulls::fire_kinetic(&design, ship, ammo_id).map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Spend ship magazine ammo.
    pub fn spend_ship_magazine(
        &mut self,
        ship_id: EntityId,
        ammo_id: &str,
        qty: f64,
    ) -> Result<f64, OperatorError> {
        let ship = self
            .world
            .ships
            .get_mut(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        crate::hulls::spend_magazine(ship, ammo_id, qty).map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Load ship magazine.
    pub fn load_ship_magazine(
        &mut self,
        ship_id: EntityId,
        ammo_id: &str,
        qty: f64,
    ) -> Result<f64, OperatorError> {
        let ship = self
            .world
            .ships
            .get_mut(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        crate::hulls::load_magazine(ship, ammo_id, qty).map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Refine ship fuel by its design tier recipe.
    pub fn refine_ship_tier_fuel(
        &mut self,
        ship_id: EntityId,
        system: EntityId,
    ) -> Result<f64, OperatorError> {
        crate::hulls::refine_ship_tier_fuel(self.world, ship_id, system)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Tool yard for a design.
    pub fn tool_ship_yard(&mut self, empire_id: EntityId, design_id: EntityId) -> Result<(), OperatorError> {
        crate::hulls::tool_yard(self.world, empire_id, design_id)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }


    /// Build ship at a system yard (consumes recipe.hull_plate).
    pub fn build_ship_at_yard(
        &mut self,
        empire_id: EntityId,
        design_id: EntityId,
        system: EntityId,
        fuel_qty: f64,
    ) -> Result<EntityId, OperatorError> {
        crate::hulls::build_ship_at_system(self.world, empire_id, design_id, system, fuel_qty)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Spend ship fuel (motion burn).
    pub fn spend_ship_fuel(&mut self, ship_id: EntityId, amount: f64) -> Result<f64, OperatorError> {
        let ship = self
            .world
            .ships
            .get_mut(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        crate::hulls::spend_fuel(ship, amount).map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Refine fuel recipe at system into ship tank.
    pub fn refine_ship_fuel(
        &mut self,
        ship_id: EntityId,
        system: EntityId,
        recipe_id: &str,
    ) -> Result<f64, OperatorError> {
        crate::hulls::refine_fuel_at_system(self.world, ship_id, system, recipe_id)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Build a ship from a tooled design.
    pub fn build_ship_instance(
        &mut self,
        empire_id: EntityId,
        design_id: EntityId,
        fuel_qty: f64,
    ) -> Result<EntityId, OperatorError> {
        crate::hulls::build_ship(self.world, empire_id, design_id, fuel_qty)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Mutate Phase F ship god/fuel fields.
    pub fn set_ship_field(
        &mut self,
        ship_id: EntityId,
        field: &str,
        value: &str,
    ) -> Result<(), OperatorError> {
        let tick = self.world.master_tick();
        let ship = self
            .world
            .ships
            .get_mut(&ship_id)
            .ok_or(OperatorError::NotFound(ship_id))?;
        let (old, new) = match field {
            "fuel_qty" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = ship.fuel_qty.to_string();
                ship.fuel_qty = v;
                (old, v.to_string())
            }
            "damage" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = ship.damage.to_string();
                ship.damage = v.clamp(0.0, 1.0);
                (old, ship.damage.to_string())
            }
            "planetary_strike" => {
                let v = parse_bool(value).ok_or_else(|| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected bool".into(),
                })?;
                let old = ship.planetary_strike.to_string();
                ship.planetary_strike = v;
                (old, v.to_string())
            }
            "sidearm_caliber" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = ship.sidearm_caliber.to_string();
                ship.sidearm_caliber = v;
                (old, v.to_string())
            }
            other => return Err(OperatorError::UnknownField(other.into())),
        };
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: ship_id,
                field: field.into(),
                old,
                new,
            },
        );
        Ok(())
    }


    /// Mutate a Phase D body field. Every successful mutation is logged.
    pub fn set_body_field(
        &mut self,
        id: EntityId,
        field: &str,
        value: &str,
    ) -> Result<(), OperatorError> {
        let tick = self.world.master_tick();
        let body = self
            .world
            .ledger_mut()
            .get_body_mut(id)
            .ok_or(OperatorError::NotFound(id))?;

        let (old, new) = match field {
            "pops" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = body.pops.to_string();
                body.pops = v;
                (old, v.to_string())
            }
            "automation_active" => {
                let v = parse_bool(value).ok_or_else(|| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected bool".into(),
                })?;
                let old = body.automation_active.to_string();
                body.automation_active = v;
                (old, v.to_string())
            }
            "structure_soak" | "power_soak" | "upkeep_soak" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = match field {
                    "structure_soak" => {
                        let o = body.structure_soak.to_string();
                        body.structure_soak = v;
                        o
                    }
                    "power_soak" => {
                        let o = body.power_soak.to_string();
                        body.power_soak = v;
                        o
                    }
                    _ => {
                        let o = body.upkeep_soak.to_string();
                        body.upkeep_soak = v;
                        o
                    }
                };
                (old, v.to_string())
            }
            "atmosphere_pressure" | "temperature" | "radiation" | "toxins_fallout" | "biosphere" => {
                let v: f64 = value.parse().map_err(|_| OperatorError::InvalidValue {
                    field: field.into(),
                    reason: "expected f64".into(),
                })?;
                let old = match field {
                    "atmosphere_pressure" => {
                        let o = body.layers.atmosphere_pressure.to_string();
                        body.layers.atmosphere_pressure = v;
                        o
                    }
                    "temperature" => {
                        let o = body.layers.temperature.to_string();
                        body.layers.temperature = v;
                        o
                    }
                    "radiation" => {
                        let o = body.layers.radiation.to_string();
                        body.layers.radiation = v;
                        o
                    }
                    "toxins_fallout" => {
                        let o = body.layers.toxins_fallout.to_string();
                        body.layers.toxins_fallout = v;
                        o
                    }
                    _ => {
                        let o = body.layers.biosphere.to_string();
                        body.layers.biosphere = v;
                        o
                    }
                };
                (old, v.to_string())
            }
            other => return Err(OperatorError::UnknownField(other.into())),
        };

        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: id,
                field: field.into(),
                old,
                new,
            },
        );
        Ok(())
    }

    /// Phase C: add a deposit (stock id, qty, accessibility, extractor) and reaggregate.
    pub fn add_deposit(
        &mut self,
        system: EntityId,
        stock_id: &str,
        quantity: f64,
        accessibility: f64,
        extractor: &str,
    ) -> Result<(), OperatorError> {
        let kind = ExtractorKind::parse(extractor).ok_or_else(|| OperatorError::InvalidValue {
            field: "extractor".into(),
            reason: format!("expected state|civilian|foreign|abandoned_auto, got '{extractor}'"),
        })?;
        matter::add_deposit(
            self.world,
            system,
            Deposit::new(stock_id, quantity, accessibility, kind),
        )
        .map_err(|e| OperatorError::Other(e.to_string()))
    }

    /// Phase C: Lock-8 extract drain by extractor kind; triggers sky depletion check.
    pub fn extract(
        &mut self,
        system: EntityId,
        extractor: &str,
        amount: f64,
    ) -> Result<f64, OperatorError> {
        let kind = ExtractorKind::parse(extractor).ok_or_else(|| OperatorError::InvalidValue {
            field: "extractor".into(),
            reason: format!("expected state|civilian|foreign|abandoned_auto, got '{extractor}'"),
        })?;
        let drained = matter::extract(self.world, system, kind, amount)
            .map_err(|e| OperatorError::Other(e.to_string()))?;
        let tick = self.world.master_tick();
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: system,
                field: format!("extract:{}", kind.as_str()),
                old: String::new(),
                new: drained.to_string(),
            },
        );
        Ok(drained)
    }

    /// Phase C: salvage feed-pipe (no vein refill).
    pub fn salvage_feed(&mut self, system: EntityId, amount: f64) -> Result<f64, OperatorError> {
        matter::salvage_into_feed(self.world, system, amount)
            .map_err(|e| OperatorError::Other(e.to_string()))
    }


    /// Add a civilian extraction line (rate = quantity per master-tick).
    pub fn add_civilian_line(
        &mut self,
        system: EntityId,
        rate: f64,
    ) -> Result<(), OperatorError> {
        if !rate.is_finite() || rate < 0.0 {
            return Err(OperatorError::InvalidValue {
                field: "civilian_line.rate".into(),
                reason: "expected finite >= 0".into(),
            });
        }
        let tick = self.world.master_tick();
        {
            let entity = self
                .world
                .ledger_mut()
                .get_mut(system)
                .ok_or(OperatorError::NotFound(system))?;
            entity.civilian_lines.push(CivilianLine::new(rate));
        }
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: system,
                field: "civilian_lines.push".into(),
                old: "".into(),
                new: rate.to_string(),
            },
        );
        Ok(())
    }

    /// Run a cosmology recipe BOM at a system (C chain stub).
    pub fn run_recipe(
        &mut self,
        system: EntityId,
        recipe_id: &str,
    ) -> Result<bool, OperatorError> {
        match matter::try_run_recipe(self.world, system, recipe_id) {
            Ok(v) => Ok(v),
            Err(matter::MatterError::SystemNotFound(id)) => Err(OperatorError::NotFound(id)),
            Err(matter::MatterError::UnknownStock(id)) => Err(OperatorError::InvalidValue {
                field: "recipe_id".into(),
                reason: format!("unknown recipe/stock {id}"),
            }),
            Err(matter::MatterError::InvalidAmount) => Err(OperatorError::InvalidValue {
                field: "recipe".into(),
                reason: "invalid amount".into(),
            }),
        }
    }

    pub fn arm_fuse(&mut self, id: EntityId, end_tick: u64) -> Result<(), OperatorError> {
        let tick = self.world.master_tick();
        {
            let entity = self
                .world
                .ledger_mut()
                .get_mut(id)
                .ok_or(OperatorError::NotFound(id))?;
            entity.depleted = true;
            entity.fuse_end_tick = Some(end_tick);
            entity.sync_fuse_remaining(tick);
        }
        self.world
            .log_mut()
            .append(tick, EventKind::Deplete { system: id });
        self.world.log_mut().append(
            tick,
            EventKind::FuseArmed {
                system: id,
                end_tick,
            },
        );
        self.world.log_mut().append(
            tick,
            EventKind::OperatorMutation {
                entity: id,
                field: "fuse_end_tick".into(),
                old: "None".into(),
                new: end_tick.to_string(),
            },
        );
        Ok(())
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::{find_segment, unlock_segment};
    use crate::world::World;

    #[test]
    fn operator_ef_lab_to_ship_pipeline() {
        let mut w = World::new(42);
        let empire = *w.ledger.empires().next().unwrap().0;
        {
            let e = w.ledger.get_empire_mut(empire).unwrap();
            unlock_segment(e, &find_segment("seg.basic_lab").unwrap()).unwrap();
            unlock_segment(e, &find_segment("seg.chem_drive").unwrap()).unwrap();
            unlock_segment(e, &find_segment("seg.yard").unwrap()).unwrap();
            unlock_segment(e, &find_segment("seg.tankage").unwrap()).unwrap();
        }
        let mut op = Operator::new(&mut w);
        let lab = op.spawn_lab(empire, 2.0).unwrap();
        op.assign_research(lab, "seg.mine_auto").unwrap();
        let did = op
            .register_ship_design(
                empire,
                "scout",
                vec!["module.engine_chem".into(), "module.tankage".into()],
            )
            .unwrap();
        op.tool_ship_yard(empire, did).unwrap();
        let sid = op.build_ship_instance(empire, did, 10.0).unwrap();
        op.set_ship_field(sid, "fuel_qty", "5.0").unwrap();
        assert_eq!(w.ships.get(&sid).unwrap().fuel_qty, 5.0);
        assert_eq!(
            w.labs.get(&lab).unwrap().assigned_segment.as_deref(),
            Some("seg.mine_auto")
        );
    }
}
