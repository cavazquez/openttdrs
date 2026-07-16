//! Motores base `OpenGFX` (velocidad máxima en unidades internas de `OpenTTD`).
//!
//! Catálogo con los vehículos originales del clima templado; los valores
//! provienen de `_orig_rail_vehicle_info` / `_orig_road_vehicle_info` y
//! `_orig_engine_info` (`src/table/engines.h` del upstream), con precios y
//! costes de operación derivados de `src/table/pricebase.h` (`cost = base ×
//! cost_factor >> 8`).

mod catalog_data;
mod model;
mod physics;
mod query;

pub use catalog_data::*;
pub use model::*;
pub use physics::*;
pub use query::*;
