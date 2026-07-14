//! Radar animado sobre torres de aeropuerto (`SPR_AIRPORT_RADAR_*`).
//!
//! La simulación avanza `m7` en `step_airport_tiles`; el cliente lee el frame
//! vivo del mapa cada tick visual.

use bevy::prelude::*;
use openttdrs_core::{TileCoord, airport_radar_frame, is_airport_tower_tile};

use crate::bevy_app::UpdateSet;
use crate::render::WorldAssets;
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct AirportRadarAnimPlugin;

impl Plugin for AirportRadarAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_airport_radar
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Overlay del radar; `pos` apunta a la tesela Tower en el mapa.
#[derive(Component, Clone, Copy)]
pub(crate) struct AirportRadarAnim {
    pub(crate) pos: TileCoord,
}

fn animate_airport_radar(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut q: Query<(&AirportRadarAnim, &mut Sprite, &mut Visibility)>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (anim, mut sprite, mut visibility) in &mut q {
        let Some(tile) = sim.state.map.get(anim.pos) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if !is_airport_tower_tile(tile.kind, tile.m5) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let frame = usize::from(airport_radar_frame(tile.m7));
        let Some(frame_sprite) = assets.airport_radar.get(frame) else {
            continue;
        };
        if !frame_sprite.matches(&sprite) {
            frame_sprite.apply_to(&mut sprite);
        }
    }
}
