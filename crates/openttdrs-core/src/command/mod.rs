//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

mod apply;
mod build_object;
mod buy_land;
mod company;
mod economy;
mod error;
mod industry;
mod metadata;
mod newgrf;
mod preview;
mod sign;
mod terraform;
mod town;
mod transport;
mod types;
mod util;
mod vehicle_fleet;
pub(crate) mod vehicles;

pub use apply::apply_command;
pub use error::{CommandError, OrderMoveDirection};
pub use industry::{
    check_place_industry_spec, check_place_industry_spec_def, check_place_industry_spec_def_layout,
    check_place_industry_spec_layout, industry_template, industry_template_layout_count,
    industry_template_with_layout, place_industry_spec_def_layout_sandbox,
    place_industry_spec_def_sandbox,
};
pub use metadata::command_effects;
pub use preview::command_would_fail;
pub(crate) use terraform::simulate_generated_terraform_north_corner;
pub use transport::{
    MAX_STATION_NAME_CHARS, rail_bits_placement_target, rail_station_footprint,
    rail_station_layout, rail_trackbits_from_neighbors,
};
pub use transport::{
    ROAD_PLACE_FORCE_AXIS, finalize_road_drag_line, infer_road_drag_axis, preview_road_bits_at,
    road_bits_for_autoroute, road_drag_line_tiles, road_locked_tool_axis,
};
pub(crate) use transport::{
    normalize_rail_trackbits_from_neighbors, normalize_synthetic_rail_crossings,
};
pub use types::{Command, LevelMode};

pub(super) use util::{
    in_bounds, require_tile_owned_by_active, require_vehicle_owned_by_active, tile_owner,
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
