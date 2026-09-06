use bevy::prelude::*;

mod display;
mod feedback;
mod income_popup;
mod input;
mod place_flash;
mod sound_ping;

pub(crate) use feedback::{push_build_command_error, push_vehicle_start_stop_error};

pub(crate) use display::{setup_tile_info_ui, update_tile_info_text};
pub(crate) use income_popup::{animate_income_popups, spawn_income_popups};
pub(crate) use input::{
    cycle_json_save_path_hotkey, handle_hud_toggle, handle_pause_toggle, handle_tool_hotkeys,
};
pub(crate) use place_flash::{
    animate_build_place_flash, enqueue_build_place_flash, spawn_build_place_flash,
};
pub(crate) use sound_ping::{
    HudSfxHandles, HudSfxKind, PlayHudSfx, UiClickBeep, flush_hud_sfx, load_hud_sfx, play_hud_sfx,
};

/// Pausa simulacion y ruta del JSON de **F5/F9** (alternativa a variable de entorno al arranque).
#[derive(Resource)]
pub(crate) struct SimHudControls {
    pub(crate) sim_speed: f32,
    pub(crate) json_save_path: String,
    pub(crate) minimap_visible: bool,
    pub(crate) sfx_volume: f32,
    pub(crate) music_volume: f32,
    pub(crate) sound_vehicle: bool,
    pub(crate) sound_ambient: bool,
    pub(crate) sound_disaster: bool,
    pub(crate) sound_confirm: bool,
    /// Beep al pulsar botones del toolbar (`sound.click_beep` en OpenTTD).
    pub(crate) sound_click_beep: bool,
}

/// Visibilidad del HUD informativo de la esquina superior izquierda.
///
/// No incluye toolbar, minimapa ni barra de estado: el objetivo es poder
/// comparar el mapa sin el texto de diagnóstico, manteniendo los controles de
/// juego disponibles. Arranca oculto; `OPENTTDRS_SHOW_HUD=1` lo muestra desde
/// el arranque y `Ctrl+H` lo alterna durante la sesión.
#[derive(Resource)]
pub(crate) struct HudVisibility {
    pub(crate) visible: bool,
}

impl Default for HudVisibility {
    fn default() -> Self {
        Self {
            visible: crate::config::env_flag("OPENTTDRS_SHOW_HUD"),
        }
    }
}

impl Default for SimHudControls {
    fn default() -> Self {
        Self {
            sim_speed: 1.0,
            json_save_path: crate::config::json_save_path(),
            minimap_visible: true,
            sfx_volume: 0.22,
            music_volume: 0.35,
            sound_vehicle: true,
            sound_ambient: true,
            sound_disaster: true,
            sound_confirm: true,
            sound_click_beep: true,
        }
    }
}

/// Tesela bajo el cursor del ratón (hover); distinto de [`SelectedTileInfo`].
#[derive(Resource, Default)]
pub(crate) struct HoveredTileCoord {
    pub(crate) pos: Option<openttdrs_core::TileCoord>,
    pub(crate) fract_x: u8,
    pub(crate) fract_y: u8,
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
    /// Encola pitido de error (lo consume [`flush_hud_sfx`]).
    pub(crate) pending_soft_ping: bool,
    /// Ticker de noticias en la barra inferior.
    pub(crate) pending_news_ticker: bool,
    /// Aplausos (primera entrega / hito).
    pub(crate) pending_news_applause: bool,
    /// Sonido genérico de noticia completa.
    pub(crate) pending_news_chime: bool,
    /// Destello visual breve tras colocar construcción (coordenadas mundo).
    pub(crate) pending_place_flash: Option<Vec3>,
}
