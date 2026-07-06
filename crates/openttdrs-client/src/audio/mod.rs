//! Audio espacial y música de fondo.

mod asset_probe;
mod music;
mod sim_events;
mod world_sfx;

pub(crate) use asset_probe::{ClientAssetRoot, insert_asset_root, warn_missing_optional_assets};

pub(crate) use music::MusicPlugin;
pub(crate) use sim_events::SimEventsPlugin;
pub(crate) use world_sfx::{PlayWorldSfx, WorldSfxPlugin};
