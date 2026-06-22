use bevy::prelude::*;

mod display;
mod feedback;
mod income_popup;
mod input;
mod sound_ping;

pub(crate) use feedback::push_build_command_error;

pub(crate) use display::{setup_tile_info_ui, update_tile_info_text};
pub(crate) use income_popup::{animate_income_popups, spawn_income_popups};
pub(crate) use input::{cycle_json_save_path_hotkey, handle_pause_toggle, handle_tool_hotkeys};
pub(crate) use sound_ping::{
    HudSoftPingHandle, PlayHudSoftPing, flush_hud_soft_ping, load_hud_soft_ping, play_hud_soft_ping,
};

/// Pausa simulacion y ruta del JSON de **F5/F9** (alternativa a variable de entorno al arranque).
#[derive(Resource)]
pub(crate) struct SimHudControls {
    pub(crate) paused: bool,
    pub(crate) sim_speed: f32,
    pub(crate) json_save_path: String,
    pub(crate) minimap_visible: bool,
    pub(crate) sfx_volume: f32,
}

impl Default for SimHudControls {
    fn default() -> Self {
        Self {
            paused: false,
            sim_speed: 1.0,
            json_save_path: crate::config::json_save_path(),
            minimap_visible: true,
            sfx_volume: 0.22,
        }
    }
}

/// Tesela bajo el cursor del ratón (hover); distinto de [`SelectedTileInfo`].
#[derive(Resource, Default)]
pub(crate) struct HoveredTileCoord {
    pub(crate) pos: Option<openttdrs_core::TileCoord>,
}

/// Informacion del tile actualmente seleccionado (click izquierdo).
#[derive(Resource, Default)]
pub(crate) struct SelectedTileInfo {
    pub(crate) pos: Option<openttdrs_core::TileCoord>,
}

/// Marcador para el texto de informacion del tile.
#[derive(Component)]
pub(crate) struct TileInfoText;

/// Mensaje temporal tras errores de construcción (HUD superior).
#[derive(Resource, Default)]
pub(crate) struct HudBuildFeedback {
    pub(crate) message: Option<String>,
    pub(crate) expires_at_secs: f32,
    /// Encola pitido suave (reduce parámetros en `handle_tile_click`; lo consume `flush_hud_soft_ping`).
    pub(crate) pending_soft_ping: bool,
}
