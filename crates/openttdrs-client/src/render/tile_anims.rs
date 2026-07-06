//! Animaciones de teselas secundarias: contador global para aeropuertos/casas/estaciones.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::state::ClientScreen;
use crate::ui::ToolbarState;

pub(crate) struct TileAnimPlugin;

impl Plugin for TileAnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileAnimClock>().add_systems(
            Update,
            (
                advance_tile_anim_clock,
                sync_toolbar_anim_cursor.after(advance_tile_anim_clock),
            )
                .chain()
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Contador global de frames para teselas con animación vanilla (`m7` / `AnimationBase`).
#[derive(Resource)]
pub(crate) struct TileAnimClock {
    pub frame: u8,
    elapsed: Timer,
}

impl Default for TileAnimClock {
    fn default() -> Self {
        Self {
            frame: 0,
            elapsed: Timer::from_seconds(0.25, TimerMode::Repeating),
        }
    }
}

fn advance_tile_anim_clock(time: Res<Time>, mut clock: ResMut<TileAnimClock>) {
    clock.elapsed.tick(time.delta());
    if clock.elapsed.just_finished() {
        clock.frame = clock.frame.wrapping_add(1) & 0x0F;
    }
}

fn sync_toolbar_anim_cursor(clock: Res<TileAnimClock>, mut toolbar: ResMut<ToolbarState>) {
    toolbar.anim_cursor_frame = clock.frame;
}
