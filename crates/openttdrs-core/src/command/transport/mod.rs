mod air;
mod bridge;
mod internal;
mod rail;
mod road;
mod shared;
mod station;
mod water;

pub(crate) use rail::{
    bridge_collinear_rail_gaps, normalize_rail_trackbits_from_neighbors,
    normalize_synthetic_rail_crossings,
};
pub use rail::{rail_bits_placement_target, rail_trackbits_from_neighbors};
pub use road::{
    ROAD_PLACE_FORCE_AXIS, finalize_road_drag_line, infer_road_drag_axis, preview_road_bits_at,
    road_bits_for_autoroute, road_drag_line_tiles, road_locked_tool_axis,
};
pub use station::{MAX_STATION_NAME_CHARS, rail_station_footprint, rail_station_layout};

pub(in crate::command) use air::*;
pub(in crate::command) use bridge::*;
pub(in crate::command) use rail::*;
pub(in crate::command) use road::*;
pub(in crate::command) use shared::*;
pub(in crate::command) use station::*;
pub(in crate::command) use water::*;
