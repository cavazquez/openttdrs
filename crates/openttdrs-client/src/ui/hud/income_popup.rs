//! Texto flotante «+$N» al cobrar entregas de carga.

use bevy::prelude::*;

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::MapVisualLayer;
use crate::state::SimWorld;
use crate::ui::font::HudUiFont;

#[derive(Component)]
pub(crate) struct IncomePopupText {
    lifetime: Timer,
}

/// Drena `GameState::pending_income_popups` y crea etiquetas en el mapa.
pub(crate) fn spawn_income_popups(
    mut sim: ResMut<SimWorld>,
    hud_font: Res<HudUiFont>,
    mut commands: Commands,
) {
    let popups = std::mem::take(&mut sim.state.runtime.pending_income_popups);
    // SFX de ingreso vía SimEvent::Income (SimEventsPlugin), no pending_income_ping.
    let map = &sim.state.map;
    for popup in popups {
        let (tileh, base_z) = tile_slope_and_min_z(map, popup.at.x as u32, popup.at.y as u32);
        let pos = tile_pos(popup.at.x, popup.at.y, base_z, 0.0);
        let label = format!("+${}", popup.amount);
        commands.spawn((
            MapVisualLayer,
            IncomePopupText {
                lifetime: Timer::from_seconds(2.0, TimerMode::Once),
            },
            Text2d::new(label),
            TextFont {
                font: hud_font.0.clone().into(),
                font_size: FontSize::Rem(0.7),
                ..default()
            },
            TextColor(Color::srgb(0.35, 0.92, 0.42)),
            Transform::from_translation(Vec3::new(
                pos.x,
                pos.y + 22.0 + tileh as f32 * 2.0,
                pos.z + 0.5,
            )),
            Visibility::Visible,
        ));
    }
}

pub(crate) fn animate_income_popups(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut IncomePopupText)>,
    mut commands: Commands,
) {
    for (entity, mut transform, mut popup) in &mut q {
        popup.lifetime.tick(time.delta());
        transform.translation.y += 28.0 * time.delta_secs();
        if popup.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::IncomePopup;
    use openttdrs_core::prelude::*;

    #[test]
    fn pending_popups_drained_after_spawn_system() {
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            ..Default::default()
        };
        sim.state.runtime.pending_income_popups.push(IncomePopup {
            amount: 42,
            at: TileCoord::new(1, 1),
        });
        assert_eq!(sim.state.runtime.pending_income_popups.len(), 1);
        sim.state.runtime.pending_income_popups.drain(..).count();
        assert!(sim.state.runtime.pending_income_popups.is_empty());
    }
}
