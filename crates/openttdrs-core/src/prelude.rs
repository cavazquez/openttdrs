//! Facade mínima para el uso cotidiano cross-crate (#157).
//!
//! No incluye [`crate::command::Command`]: en el cliente Bevy choca con
//! `bevy::prelude::Command`. Importar comandos desde la raíz o `command::`.
//!
//! `NewGRF` avanzado, fixtures, paridad y tooling van por módulo
//! (`newgrf_sprites::`, `newgrf_actions::`, `parity::`, `tnbp_decode::`, …).

pub use crate::command::{CommandError, LevelMode, apply_command, command_would_fail};
pub use crate::company::{Company, CompanyId};
pub use crate::game_state::{GameState, SimStats, SimulationRuntime};
pub use crate::map::{Map, MapError, Tile, TileCoord, TileKind};
pub use crate::sim_events::{SimEvent, SimEventQueue};
pub use crate::station::{Station, StopKind};
pub use crate::tick::GameTick;
pub use crate::vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, Vehicle, VehicleDirection,
    VehicleKind, VehicleOrder,
};
