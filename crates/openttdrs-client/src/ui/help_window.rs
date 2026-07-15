//! Ventana Ayuda / About + mapa de hotkeys (UI-7).

use bevy::prelude::*;

use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

const HELP_BODY: &str = "\
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
  F4         Alternar ruta de guardado JSON\n\
  F5 / F9    Guardar / cargar partida rápida\n\
  Esc        Cerrar ventana superior / cancelar herramienta\n\
\n\
Ajustes → Consola: help|list, fps, overlay, gizmos, tile, newgrf, cheat, cheats, endgame, clear.\n\
Ajustes → Cheats: ventana formal (también `cheats` en consola).\n\
Ajustes → Guardar escenario: JSON en save/scenarios/ (menú Editor).\n\
Menú → Editor de escenarios: sandbox (∞$, bulldozer) + Paisaje/Fundar pueblo.\n\
Ajustes → Finalizar partida: retiro voluntario → endscreen / highscore.\n\
Ajustes → NewGRF: stack + Inspeccionar (scan/validate; sin Action0–14).\n\
Ajustes → Display: presets Clásico / Rendimiento / Dev.\n\
Con gizmos ON, el tile seleccionado muestra bounds (aligner lite).\n\
";

#[derive(Resource, Default)]
pub(crate) struct HelpWindowState {
    pub(crate) open: bool,
}

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
            Text::new(HELP_BODY),
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
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::Help {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub(crate) fn help_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<HelpWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Help {
            state.open = false;
        }
    }
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
