//! Beep al pulsar botones de UI (toolbar, menú principal, ventana guardar/cargar).
//! Paridad `SndClickBeep` / `sound.click_beep`.

use bevy::ecs::query::Or;
use bevy::prelude::*;

use crate::ui::hud::{HudSfxKind, PlayHudSfx, SimHudControls, UiClickBeep};
use crate::ui::toolbar::BuildMenuUi;

/// Reproduce `SND_15_BEEP` cuando el jugador pulsa un botón marcado con [`BuildMenuUi`] o [`UiClickBeep`].
pub(crate) fn toolbar_click_beep(
    hud: Res<SimHudControls>,
    mut writer: MessageWriter<PlayHudSfx>,
    buttons: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            Or<(With<BuildMenuUi>, With<UiClickBeep>)>,
        ),
    >,
) {
    if !hud.sound_click_beep {
        return;
    }
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            writer.write(PlayHudSfx(HudSfxKind::ClickBeep));
        }
    }
}
