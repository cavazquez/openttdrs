//! Fuente UI con cobertura UTF-8 (tildes y eñes en `Text2d` / HUD).

use bevy::prelude::*;

pub(crate) const UI_FONT_PATH: &str = "static/fonts/DejaVuSansMono.ttf";

/// Fuente cargada para etiquetas `Text2d` del mapa y HUD.
#[derive(Resource, Clone)]
pub(crate) struct HudUiFont(pub Handle<Font>);

/// `TextFont` con handle y tamaño en píxeles (Bevy 0.19: `FontSource` + `FontSize`).
#[must_use]
pub(crate) fn text_font(font: Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.into(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

#[must_use]
pub(crate) fn text_font_loaded(asset_server: &AssetServer, size: f32) -> TextFont {
    text_font(asset_server.load::<Font>(UI_FONT_PATH), size)
}

pub(crate) fn load_hud_ui_font(
    asset_server: &AssetServer,
    commands: &mut Commands,
) -> Handle<Font> {
    let font = asset_server.load::<Font>(UI_FONT_PATH);
    commands.insert_resource(HudUiFont(font.clone()));
    font
}
