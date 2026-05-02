//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

mod apply;
mod industry;
mod transport;
mod types;
mod util;
mod vehicles;

pub use apply::apply_command;
pub use industry::industry_template;
pub use types::{Command, CommandError};

pub(super) use util::in_bounds;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
