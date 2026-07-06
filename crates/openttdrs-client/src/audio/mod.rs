//! Audio espacial y música de fondo.

mod music;
mod sim_events;
mod world_sfx;

pub(crate) use music::MusicPlugin;
pub(crate) use sim_events::SimEventsPlugin;
pub(crate) use world_sfx::{PlayWorldSfx, WorldSfxPlugin};
