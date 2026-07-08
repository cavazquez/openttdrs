use bevy::prelude::*;

use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::sprites::company_colour_name;
use crate::state::SimWorld;
use crate::state::{
    ClientScreen, SimRunState, SuspendedGameSession, sim_is_paused, toggle_sim_run_state,
};
use crate::ui::hud::SimHudControls;
use crate::ui::main_menu::return_to_main_menu;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};

use super::{CompanyColourSwatch, SaveMenuAction};

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_settings_menu_buttons(
    mut q: Query<(&Interaction, &SaveMenuAction), (Changed<Interaction>, With<Button>)>,
    mut hud: ResMut<SimHudControls>,
    mut save_window: ResMut<SaveWindowState>,
    mut news_settings: ResMut<crate::ui::news_settings_window::NewsSettingsWindowState>,
    run_state: Res<State<SimRunState>>,
    mut next_run: ResMut<NextState<SimRunState>>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SaveMenuAction::SaveAs => {
                save_window.open_in_mode(SaveWindowMode::Save, &save_dir_from(&hud.json_save_path));
            }
            SaveMenuAction::LoadFrom => {
                save_window.open_in_mode(SaveWindowMode::Load, &save_dir_from(&hud.json_save_path));
            }
            SaveMenuAction::PauseResume => {
                let will_pause = !sim_is_paused(&run_state);
                toggle_sim_run_state(&run_state, &mut next_run);
                info!("Pausa: {}", if will_pause { "ON" } else { "OFF" });
            }
            SaveMenuAction::SpeedUp => {
                hud.sim_speed = if hud.sim_speed < 1.5 {
                    2.0
                } else if hud.sim_speed < 3.5 {
                    4.0
                } else {
                    1.0
                };
                info!("Velocidad simulacion: {:.0}x", hud.sim_speed);
            }
            SaveMenuAction::Normalize => {
                hud.sim_speed = 1.0;
                if let Ok((mut cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    let keep_pos = cam_tf.translation;
                    o.scale = 1.0;
                    cam_tf.translation = keep_pos;
                }
                info!("Normalizado: velocidad 1x y zoom 1.0x");
            }
            SaveMenuAction::ZoomIn => {
                if let Ok((_cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 0.85).max(0.25);
                }
            }
            SaveMenuAction::ZoomOut => {
                if let Ok((_cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 1.15).min(20.0);
                }
            }
            SaveMenuAction::NewsSettings => {
                news_settings.open = true;
            }
            SaveMenuAction::ReturnToMainMenu => {
                return_to_main_menu(&mut next_screen, &mut suspended);
            }
        }
    }
}

pub(crate) fn handle_company_colour_swatches(
    mut q: Query<(&Interaction, &CompanyColourSwatch), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
) {
    for (interaction, swatch) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let colour = swatch.0 % 16;
        if sim.state.company_colour == colour {
            continue;
        }
        sim.state.company_colour = colour;
        info!("Color compañía: {} ({colour})", company_colour_name(colour));
    }
}

pub(crate) fn sync_company_colour_swatch_visuals(
    sim: Res<SimWorld>,
    mut q: Query<
        (&CompanyColourSwatch, &mut BorderColor, &Interaction),
        (With<Button>, Without<SaveMenuAction>),
    >,
) {
    let active = sim.state.company_colour % 16;
    for (swatch, mut border, interaction) in &mut q {
        let selected = swatch.0 == active;
        *border = if selected {
            BorderColor::all(Color::srgb(0.98, 0.92, 0.35))
        } else if *interaction == Interaction::Hovered {
            BorderColor::all(Color::srgb(0.86, 0.86, 0.72))
        } else {
            BorderColor::all(Color::srgb(0.18, 0.25, 0.12))
        };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::state::{ClientScreen, SimWorld, SuspendedGameSession};
    use crate::ui::hud::SimHudControls;
    use crate::ui::news_settings_window::NewsSettingsWindowState;
    use crate::ui::save_window::{SaveWindowMode, SaveWindowState};
    use crate::ui::toolbar::{CompanyColourSwatch, SaveMenuAction};

    use super::{handle_company_colour_swatches, handle_settings_menu_buttons};

    #[test]
    fn save_and_load_buttons_open_save_window() {
        let mut world = World::new();
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(NewsSettingsWindowState::default());
        crate::state::insert_test_sim_run_state(&mut world);
        world.insert_resource(NextState::<ClientScreen>::default());
        world.insert_resource(SuspendedGameSession::default());

        world.spawn((Button, SaveMenuAction::SaveAs, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        {
            let w = world.resource::<SaveWindowState>();
            assert!(w.open);
            assert_eq!(w.mode, SaveWindowMode::Save);
        }

        world.spawn((Button, SaveMenuAction::LoadFrom, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        let w = world.resource::<SaveWindowState>();
        assert!(w.open);
        assert_eq!(w.mode, SaveWindowMode::Load);
    }

    #[test]
    fn return_to_main_menu_button_sets_next_screen() {
        use crate::state::ClientScreen;

        let mut world = World::new();
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(NewsSettingsWindowState::default());
        world.insert_resource(NextState::<ClientScreen>::default());
        world.insert_resource(SuspendedGameSession::default());
        crate::state::insert_test_sim_run_state(&mut world);

        world.spawn((
            Button,
            SaveMenuAction::ReturnToMainMenu,
            Interaction::Pressed,
        ));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert!(matches!(
            world.resource::<NextState<ClientScreen>>(),
            NextState::Pending(ClientScreen::MainMenu)
                | NextState::PendingIfNeq(ClientScreen::MainMenu)
        ));
    }

    #[test]
    fn company_colour_swatch_updates_sim_state() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());

        world.spawn((Button, CompanyColourSwatch(6), Interaction::Pressed));
        world
            .run_system_once(handle_company_colour_swatches)
            .unwrap();
        assert_eq!(world.resource::<SimWorld>().state.company_colour, 6);
    }
}
