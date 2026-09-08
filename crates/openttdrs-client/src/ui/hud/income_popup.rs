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
    if sim.state.runtime.pending_income_popups.is_empty() {
        return;
    }
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::IncomePopup;
    use openttdrs_core::prelude::*;

    #[test]
    fn empty_pending_popups_do_not_mark_simworld_changed() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(HudUiFont(Handle::default()));
        world.clear_trackers();
        let before = world.get_resource_ref::<SimWorld>().unwrap().last_changed();

        world.run_system_once(spawn_income_popups).unwrap();

        assert_eq!(
            world.get_resource_ref::<SimWorld>().unwrap().last_changed(),
            before,
            "un drain vacío no invalida consumidores de SimWorld"
        );
    }

    #[test]
    fn pending_popups_spawn_exactly_once() {
        let mut world = World::new();
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            ..Default::default()
        };
        sim.state.runtime.pending_income_popups.push(IncomePopup {
            amount: 42,
            at: TileCoord::new(1, 1),
        });
        world.insert_resource(sim);
        world.insert_resource(HudUiFont(Handle::default()));

        world.run_system_once(spawn_income_popups).unwrap();
        assert!(
            world
                .resource::<SimWorld>()
                .state
                .runtime
                .pending_income_popups
                .is_empty()
        );
        assert_eq!(
            world.query::<&IncomePopupText>().iter(&world).count(),
            1,
            "el popup pendiente se materializa"
        );

        world.run_system_once(spawn_income_popups).unwrap();
        assert_eq!(
            world.query::<&IncomePopupText>().iter(&world).count(),
            1,
            "un segundo drain vacío no duplica el popup"
        );
    }
}
