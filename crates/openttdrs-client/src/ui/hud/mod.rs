use bevy::prelude::*;

mod display;
mod input;

pub(crate) use display::{setup_tile_info_ui, update_tile_info_text};
pub(crate) use input::{cycle_json_save_path_hotkey, handle_pause_toggle, handle_tool_hotkeys};

/// Pausa simulacion y ruta del JSON de **F5/F9** (alternativa a variable de entorno al arranque).
#[derive(Resource)]
pub(crate) struct SimHudControls {
    pub(crate) paused: bool,
    pub(crate) json_save_path: String,
}

impl Default for SimHudControls {
    fn default() -> Self {
        Self {
            paused: false,
            json_save_path: crate::config::json_save_path(),
        }
    }
}

/// Informacion del tile actualmente seleccionado (click izquierdo).
#[derive(Resource, Default)]
pub(crate) struct SelectedTileInfo {
    pub(crate) pos: Option<openttdrs_core::TileCoord>,
}

/// Marcador para el texto de informacion del tile.
#[derive(Component)]
pub(crate) struct TileInfoText;
