//! Fuente UI con cobertura UTF-8 (tildes y eñes en `Text2d` / HUD).

use bevy::prelude::*;
use bevy::text::RemSize;
use bevy::window::PrimaryWindow;

pub(crate) const UI_FONT_PATH: &str = "static/fonts/DejaVuSansMono.ttf";

/// Fuente cargada para etiquetas `Text2d` del mapa y HUD.
#[derive(Resource, Clone)]
pub(crate) struct HudUiFont(pub Handle<Font>);

/// Roles tipográficos de la UI (tamaños en `rem` respecto a [`RemSize`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiFontRole {
    Caption,
    Body,
    Hud,
    Title,
}

impl UiFontRole {
    pub(crate) fn rem_size(self) -> f32 {
        match self {
            Self::Caption => 0.7,
            Self::Body => 0.85,
            Self::Hud => 1.0,
            Self::Title => 1.4,
        }
    }
}

/// `TextFont` con handle y tamaño en píxeles (Bevy 0.19: `FontSource` + `FontSize`).
#[must_use]
#[allow(dead_code)]
pub(crate) fn text_font(font: Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.into(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// Fuente UI responsiva según rol (`FontSize::Rem`).
#[must_use]
pub(crate) fn ui_text_font(font: Handle<Font>, role: UiFontRole) -> TextFont {
    TextFont {
        font: font.into(),
        font_size: FontSize::Rem(role.rem_size()),
        ..default()
    }
}

#[must_use]
#[allow(dead_code)]
pub(crate) fn text_font_loaded(asset_server: &AssetServer, size: f32) -> TextFont {
    text_font(asset_server.load::<Font>(UI_FONT_PATH), size)
}

#[must_use]
pub(crate) fn ui_text_font_loaded(asset_server: &AssetServer, role: UiFontRole) -> TextFont {
    ui_text_font(asset_server.load::<Font>(UI_FONT_PATH), role)
}

pub(crate) fn load_hud_ui_font(
    asset_server: &AssetServer,
    commands: &mut Commands,
) -> Handle<Font> {
    let font = asset_server.load::<Font>(UI_FONT_PATH);
    commands.insert_resource(HudUiFont(font.clone()));
    font
}

/// Escala `RemSize` desde la altura lógica de la ventana (base 720 px → 14 px).
pub(crate) fn sync_rem_size_from_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut rem_size: ResMut<RemSize>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let height = window.resolution.height();
    let px = (height / 720.0 * 14.0).clamp(10.0, 20.0);
    if (rem_size.0 - px).abs() > 0.01 {
        rem_size.0 = px;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn ui_font_role_rem_sizes_ordered() {
        assert!(UiFontRole::Caption.rem_size() < UiFontRole::Body.rem_size());
        assert!(UiFontRole::Body.rem_size() < UiFontRole::Hud.rem_size());
        assert!(UiFontRole::Hud.rem_size() < UiFontRole::Title.rem_size());
    }

    #[test]
    fn sync_rem_clamps_to_range() {
        let mut world = World::new();
        world.init_resource::<RemSize>();
        world.spawn((
            Window {
                resolution: (1280_u32, 360_u32).into(),
                ..default()
            },
            PrimaryWindow,
        ));
        world.run_system_once(sync_rem_size_from_window).unwrap();
        assert!((world.resource::<RemSize>().0 - 10.0).abs() < 0.1);
    }
}
