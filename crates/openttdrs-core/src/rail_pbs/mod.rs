//! Reserva de ruta ferroviaria (`PBS` fase 2).
//!
//! Cada tren reserva **pistas** (`TrackBits`) a lo largo de su `path` hasta la siguiente
//! **posición segura de espera** (delante de señal, depósito o fin de vía), o hasta el
//! primer conflicto (otra reserva, ocupación o señal block cerrada). Las vías paralelas
//! en la misma tesela (`Horz`/`Vert`) pueden reservarse de forma independiente.

#![allow(clippy::implicit_hasher)]

mod choose_track;
mod conflicts;
mod map_sync;
mod model;
mod search;
mod train_reservation;
mod try_reserve;
mod wait_policy;

#[cfg(test)]
mod tests;

// Reexports públicos desde model
pub use model::{
    MAX_TRAIN_RESERVATION_LEN, RAIL_RESERVATION_M2_HI_MASK, ReservedRailStep,
    YAPF_RESERVATION_CROSS_PENALTY, YAPF_TILE_CORNER_LENGTH, YAPF_TILE_LENGTH,
    decode_rail_reservation_m2_hi, encode_rail_reservation_to_m2_hi, rail_tile_has_pbs_reservation,
    track_for_rail_step, track_on_departure_tile,
};

// Reexports públicos desde search
pub use search::{
    find_path_to_safe_wait, find_path_to_safe_wait_with_wormholes, is_safe_waiting_position,
    reservation_ends_at_safe_wait, tile_has_any_pbs_signal,
};

// Reexports públicos desde conflicts
pub use conflicts::{
    pbs_exit_has_complete_reservation, platform_reserved_or_occupied,
    platform_track_reserved_or_occupied, tile_track_reserved_by_map,
};

// Reexports públicos desde train_reservation
pub use train_reservation::{
    compute_train_reservation, compute_train_reservation_with_settings,
    compute_train_reservation_with_wormholes, follow_train_reservation,
    train_blocked_by_reservation, update_train_reservations,
    update_train_reservations_with_settings, update_train_reservations_with_wormholes,
};

// Reexports públicos desde try_reserve
pub use try_reserve::try_path_reserve;

// Reexports públicos desde wait_policy
pub use wait_policy::{
    tick_pbs_wait_and_maybe_reverse, tick_signal_wait_and_maybe_reverse, train_waiting_for_pbs_path,
};

// Reexports públicos desde map_sync
pub use map_sync::{
    CROSSING_RESERVATION_M5_BIT, free_train_track_reservation, sync_reservations_to_map,
};

// Reexports públicos desde choose_track
pub use choose_track::{ChosenTrainTrack, choose_train_track_on_enter, tile_is_track_choice};
