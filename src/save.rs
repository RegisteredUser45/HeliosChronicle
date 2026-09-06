//! Save / load world + ledger to disk (JSON).

use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::event::EventKind;
use crate::world::World;

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Serialize world to JSON path. Appends a WorldSaved event before writing.
pub fn save_world(world: &mut World, path: impl AsRef<Path>) -> Result<(), SaveError> {
    let path = path.as_ref();
    let path_str = path.display().to_string();
    let tick = world.master_tick();
    world.log_mut().append(
        tick,
        EventKind::WorldSaved {
            path: path_str.clone(),
        },
    );
    world.recompute_outcome_hash();

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(world)?;
    fs::write(path, json)?;
    Ok(())
}

/// Load world from JSON path. Appends a WorldLoaded event after restore.
pub fn load_world(path: impl AsRef<Path>) -> Result<World, SaveError> {
    let path = path.as_ref();
    let data = fs::read_to_string(path)?;
    let mut world: World = serde_json::from_str(&data)?;
    let path_str = path.display().to_string();
    let tick = world.master_tick();
    world.log_mut().append(tick, EventKind::WorldLoaded { path: path_str });
    world.recompute_outcome_hash();
    Ok(world)
}
