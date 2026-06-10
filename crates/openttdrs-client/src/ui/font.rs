//! Fuente UI con cobertura UTF-8 (tildes y eñes en `Text2d` / HUD).

use bevy::prelude::*;

pub(crate) const UI_FONT_PATH: &str = "static/fonts/DejaVuSansMono.ttf";

/// Fuente cargada para etiquetas `Text2d` del mapa y HUD.
#[derive(Resource, Clone)]
pub(crate) struct HudUiFont(pub Handle<Font>);

pub(crate) fn load_hud_ui_font(
    asset_server: &AssetServer,
    commands: &mut Commands,
) -> Handle<Font> {
    let font = asset_server.load::<Font>(UI_FONT_PATH);
    commands.insert_resource(HudUiFont(font.clone()));
    font
}
