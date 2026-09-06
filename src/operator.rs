//! Operator R/W hooks — inspect/mutate any ledger record; every mutation is an event.

use thiserror::Error;

use crate::entity::{EntityId, SystemEntity};
use crate::event::EventKind;
use crate::lod::LodHint;
use crate::world::World;

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("entity {0} not found")]
    NotFound(EntityId),
    #[error("unknown field '{0}'")]
    UnknownField(String),
    #[error("invalid value for field '{field}': {reason}")]
    InvalidValue { field: String, reason: String },
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

    /// Mutate a named field on a system entity. Every successful mutation is logged.
    pub fn set_field(
        &mut self,
        id: EntityId,
        field: &str,
        value: &str,
    ) -> Result<(), OperatorError> {
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
            other => return Err(OperatorError::UnknownField(other.into())),
        };

        // Special-case home flag events for Phase B stubs.
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

        Ok(())
    }

    /// Force-arm a fuse ending at an absolute master tick (stub for B).
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
