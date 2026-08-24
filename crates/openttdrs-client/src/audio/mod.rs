//! Audio espacial y música de fondo.

use bevy::prelude::Resource;

mod asset_probe;
mod music;
mod sim_events;
mod world_sfx;

/// Indica si esta ejecución inicializó un backend de audio.
///
/// Las capturas raster desactivan el backend para que no dependan de ALSA,
/// PipeWire ni de una salida de sonido. La UI conserva sus controles, pero no
/// debe pedir handles de [`bevy::audio::AudioSource`] cuando este valor es
/// falso.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClientAudioEnabled(pub(crate) bool);

pub(crate) use asset_probe::{
    ClientAssetRoot, ClientAssetStatus, insert_asset_root, warn_missing_optional_assets,
};

pub(crate) use music::MusicPlugin;
pub(crate) use music::{
    MusicPlayer, MusicPlaylist, MusicState, music_apply_playlist, music_skip, music_toggle_playback,
};
pub(crate) use sim_events::{
    PendingSimEvents, SimEventsPlugin, play_vehicle_event_sound_with_default,
};
pub(crate) use world_sfx::{PlayWorldSfx, WorldSfxPlugin};
