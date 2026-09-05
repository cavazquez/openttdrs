//! Ventana Ayuda / About + mapa de hotkeys (UI-7).

use bevy::prelude::*;

use crate::i18n::Locale;
use crate::settings::ClientPreferences;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::window_lifecycle::{
    close_floating_window_on_message, sync_floating_window_visibility,
};

const HELP_BODY_ES: &str = "\
openttdrs — cliente Rust de OpenTTD (paridad single-player)\n\
Gráficos base: OpenGFX · Sonido: OpenSFX · Música: OpenMSX\n\
\n\
Atajos de teclado\n\
  P          Pausa / reanudar simulación\n\
  M          Mostrar / ocultar minimapa\n\
  R          Remap visual del mapa\n\
  1 / 2 / 3 / 4   Herramientas rápidas de toolbar\n\
  C          Demoler (Clear)\n\
  F1 / ?     Esta ayuda\n\
  F2         Inspector de tile\n\
  F3 / `     Consola / Dev (FPS, comandos)\n\
  Ctrl+Alt+C Cheats (dinero, año, bulldozer, compañía)\n\
  + / −      Acercar / alejar cámara\n\
  Ctrl+Alt+Z Alternar zoom fijo (inicial) / libre\n\
  Ctrl+H     Mostrar / ocultar HUD informativo\n\
  F4         Alternar ruta de guardado JSON\n\
  F5 / F9    Guardar / cargar partida rápida\n\
  Esc        Cerrar ventana superior / cancelar herramienta\n\
\n\
Ajustes → Consola: help|list, fps, overlay, gizmos, tile, newgrf, cheat, cheats, endgame, clear.\n\
Ajustes → Cheats: ventana formal (también `cheats` en consola).\n\
Ajustes → Guardar escenario: JSON en save/scenarios/ (menú Editor).\n\
Menú → Editor de escenarios: sandbox (∞$, bulldozer) + Paisaje/Fundar pueblo.\n\
Economía → Objetivos / Liga · Mundo → Historia: GameScript-lite (#43).\n\
Ajustes → Finalizar partida: retiro voluntario → endscreen / highscore.\n\
Ajustes → NewGRF: stack + Inspeccionar (scan/validate; sin Action0–14).\n\
Ajustes → Display: presets Clásico / Rendimiento / Dev.\n\
Con gizmos ON, el tile seleccionado muestra bounds (aligner lite).\n\
";

const HELP_BODY_EN: &str = "\
openttdrs — Rust OpenTTD client (single-player parity)\n\
Base graphics: OpenGFX · Sound: OpenSFX · Music: OpenMSX\n\
\n\
Keyboard shortcuts\n\
  P          Pause / resume simulation\n\
  M          Show / hide minimap\n\
  R          Visual map remap\n\
  1 / 2 / 3 / 4   Quick toolbar tools\n\
  C          Demolish (Clear)\n\
  F1 / ?     This help\n\
  F2         Tile inspector\n\
  F3 / `     Console / Dev (FPS, commands)\n\
  Ctrl+Alt+C Cheats (money, year, bulldozer, company)\n\
  + / −      Zoom camera in / out\n\
  Ctrl+Alt+Z Toggle fixed (initial) / free zoom\n\
  Ctrl+H     Show / hide information HUD\n\
  F4         Toggle JSON save path\n\
  F5 / F9    Quick save / load game\n\
  Esc        Close top window / cancel tool\n\
\n\
Settings → Console: help|list, fps, overlay, gizmos, tile, newgrf, cheat, cheats, endgame, clear.\n\
Settings → Cheats: formal window (also `cheats` in console).\n\
Settings → Save scenario: JSON in save/scenarios/ (Editor menu).\n\
Menu → Scenario editor: sandbox (∞$, bulldozer) + Landscape/Found town.\n\
Economy → Goals / League · World → Story: GameScript-lite (#43).\n\
Settings → End game: voluntary retirement → endscreen / high score.\n\
Settings → NewGRF: stack + Inspect (scan/validate; no Action0–14).\n\
Settings → Display: Classic / Performance / Dev presets.\n\
With gizmos ON, the selected tile shows bounds (aligner lite).\n\
";

fn help_body(locale: Locale) -> &'static str {
    match locale {
        Locale::Es => HELP_BODY_ES,
        Locale::En => HELP_BODY_EN,
    }
}

#[derive(Resource, Default)]
pub(crate) struct HelpWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct HelpBodyText;

pub(crate) fn setup_help_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Help,
        "Ayuda",
        TITLE_BROWN,
        Vec2::new(360.0, 80.0),
        420.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            HelpBodyText,
            Text::new(HELP_BODY_ES),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

pub(crate) fn sync_help_window(
    state: Res<HelpWindowState>,
    prefs: Res<ClientPreferences>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut bodies: Query<&mut Text, With<HelpBodyText>>,
) {
    sync_floating_window_visibility(&mut windows, FloatingWindowId::Help, state.open);
    let body = help_body(prefs.locale());
    for mut text in &mut bodies {
        if text.as_str() != body {
            **text = body.to_owned();
        }
    }
}

pub(crate) fn help_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<HelpWindowState>,
) {
    close_floating_window_on_message(&mut closed, FloatingWindowId::Help, || {
        state.open = false;
    });
}

/// **F1** o **?** abre/cierra la ayuda.
pub(crate) fn handle_help_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<HelpWindowState>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
) {
    if console.is_some_and(|c| crate::ui::dev_console::dev_console_captures_keyboard(&c)) {
        return;
    }
    if keyboard.just_pressed(KeyCode::F1) || keyboard.just_pressed(KeyCode::Slash) {
        state.open = !state.open;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::prelude::*;

    use super::{HELP_BODY_EN, HELP_BODY_ES, HelpBodyText, HelpWindowState, sync_help_window};
    use crate::settings::ClientPreferences;

    #[test]
    fn help_body_follows_the_live_locale_without_translating_commands() {
        let mut world = World::new();
        world.insert_resource(HelpWindowState { open: true });
        world.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        let body = world.spawn((HelpBodyText, Text::new(HELP_BODY_ES))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(sync_help_window);
        schedule.run(&mut world);
        let english = world.entity(body).get::<Text>().unwrap().as_str();
        assert_eq!(english, HELP_BODY_EN);
        assert!(english.contains("Ctrl+Alt+C Cheats"));
        assert!(english.contains("help|list"));

        world.resource_mut::<ClientPreferences>().language = "es-AR".into();
        schedule.run(&mut world);
        assert_eq!(
            world.entity(body).get::<Text>().unwrap().as_str(),
            HELP_BODY_ES
        );
    }
}
