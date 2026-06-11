//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

mod apply;
mod industry;
mod preview;
mod transport;
mod types;
mod util;
mod vehicles;

pub use apply::apply_command;
pub use industry::industry_template;
pub use preview::command_would_fail;
pub(crate) use transport::normalize_synthetic_rail_crossings;
pub use transport::{rail_station_footprint, rail_trackbits_from_neighbors};
pub use types::{Command, CommandError, command_error_message};

pub(super) use util::in_bounds;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
