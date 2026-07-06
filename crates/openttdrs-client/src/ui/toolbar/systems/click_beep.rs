//! Beep al pulsar botones del toolbar (paridad `SndClickBeep` / `sound.click_beep`).

use bevy::prelude::*;

use crate::ui::hud::{HudSfxKind, PlayHudSfx, SimHudControls};
use crate::ui::toolbar::BuildMenuUi;

/// Reproduce `SND_15_BEEP` cuando el jugador pulsa un botón del toolbar.
pub(crate) fn toolbar_click_beep(
    hud: Res<SimHudControls>,
    mut writer: MessageWriter<PlayHudSfx>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>, With<BuildMenuUi>)>,
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
