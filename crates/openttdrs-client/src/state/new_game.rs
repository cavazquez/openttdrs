//! Recurso Bevy con opciones de nueva partida (menú principal).

use bevy::prelude::*;

use crate::state::bootstrap::{MapSizePreset, NewGameSettings, START_YEARS};

/// Alias del recurso en el menú (evita colisión con `NewGameSettings` del bootstrap).
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct NewGameSettingsResource(pub NewGameSettings);

impl Default for NewGameSettingsResource {
    fn default() -> Self {
        Self(NewGameSettings {
            map_size: MapSizePreset::Small,
            start_year: START_YEARS[0],
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed: 0,
            ..NewGameSettings::default()
        })
    }
}

impl NewGameSettingsResource {
    #[must_use]
    pub const fn settings(self) -> NewGameSettings {
        self.0
    }
}
