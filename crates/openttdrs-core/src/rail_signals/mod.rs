//! Señales ferroviarias de bloque: codificación, topología, routing e invalidación incremental.

mod encoding;
mod presignal;
mod routing;
mod topology;
mod update;

pub(crate) use crate::map::rail_traversal_bits;
pub use crate::map::{RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, rail_tile_is_signals};

pub use encoding::{
    RAIL_REMOVE_REFUND, SEMAPHORE_BUILD_BEFORE_YEAR, SIGNAL_BUILD_COST, SIGNAL_REMOVE_REFUND,
    SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_LAST_NOPBS, SIGTYPE_PATH,
    SIGTYPE_PATH_ONEWAY, SignalPlacement, SignalTrack, calendar_year_at_tick,
    clear_signal_type_bits_m2, cycle_signal_facing, cycle_signal_side_m3, cycle_signal_type_m2,
    cycle_signal_variant_m2, default_signal_variant, encode_block_signal_on_track,
    encode_block_signal_on_track_with_variant, is_pbs_signal_type, m2_for_signal,
    next_placeable_signal_type, rail_signal_present_mask, rail_signal_state_mask,
    resolve_signal_track, set_signal_variant_m2, signal_bit_for_facing,
    signal_facing_for_orientation, signal_is_green, signal_on_track_mask,
    signal_placement_for_facing, signal_placement_for_track, signal_type_for_track,
    signal_type_label, signal_variant_for_track, tracks_overlap, valid_signal_facings_track,
};
pub(crate) use encoding::{signal_exit_dir, signal_track_for_bit};
pub(crate) use routing::signal_bits_for_exit;

pub(crate) use topology::{dir_from_to, rail_neighbors};
pub use topology::{rail_block_ahead, rail_block_ahead_with_wormholes};

pub(crate) use routing::rail_step_signal_allows;
pub(crate) use routing::train_blocked_by_traffic_indexed;
pub use routing::{
    YAPF_PBS_BEHIND_PENALTY, YAPF_RED_SIGNAL_PENALTY, YapfSignalRouting, train_blocked_by_pbs_path,
    train_blocked_by_signal, train_blocked_by_traffic, train_facing_head_on_traffic,
    yapf_routing_signal,
};

pub use update::{
    SIG_GLOB_UPDATE, SignalGlobEntry, SignalGlobSet, SignalSpatialIndex,
    collect_signals_affected_by_tiles, collect_signals_affected_by_tiles_indexed,
    collect_signals_affected_by_tiles_with_wormholes, drain_signal_globset,
    drain_signal_globset_indexed_with_wormholes, drain_signal_globset_with_wormholes,
    enqueue_pbs_reservations_for_signal_update, enqueue_signal_glob, enqueue_signal_glob_side,
    enqueue_trains_for_signal_update, signal_globset_needs_flush, update_rail_signal_states,
    update_rail_signal_states_scoped, update_rail_signal_states_with_wormholes,
};

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;
