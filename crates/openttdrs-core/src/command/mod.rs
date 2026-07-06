//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

mod apply;
mod buy_land;
mod economy;
mod industry;
mod preview;
mod terraform;
mod town;
mod transport;
mod types;
mod util;
mod vehicle_fleet;
mod vehicles;

pub use apply::apply_command;
pub use industry::{check_place_industry_spec, industry_template};
pub use preview::command_would_fail;
pub use transport::{
    ROAD_PLACE_FORCE_AXIS, finalize_road_drag_line, infer_road_drag_axis, preview_road_bits_at,
    road_bits_for_autoroute, road_drag_line_tiles, road_locked_tool_axis,
};
pub(crate) use transport::{
    bridge_collinear_rail_gaps, normalize_rail_trackbits_from_neighbors,
    normalize_synthetic_rail_crossings,
};
pub use transport::{
    rail_bits_placement_target, rail_station_footprint, rail_trackbits_from_neighbors,
};
pub use types::{Command, CommandError, LevelMode, OrderMoveDirection, command_error_message};

pub(super) use util::in_bounds;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
