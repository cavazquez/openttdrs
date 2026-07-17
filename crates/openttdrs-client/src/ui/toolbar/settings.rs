use bevy::prelude::*;

use crate::render::RemapMapVisualsPending;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::settings::ClientPreferences;
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
    mut pathfinding_settings: ResMut<
        crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState,
    >,
    mut newgrf_window: ResMut<crate::ui::newgrf_window::NewGrfWindowState>,
    mut display_options: ResMut<crate::ui::display_options_window::DisplayOptionsWindowState>,
    mut extra_viewport: ResMut<crate::ui::extra_viewport_window::ExtraViewportWindowState>,
    mut prefs: Option<ResMut<ClientPreferences>>,
    mut pending_remap: Option<ResMut<RemapMapVisualsPending>>,
    run_state: Res<State<SimRunState>>,
    mut next_run: ResMut<NextState<SimRunState>>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    mut help_tools: ParamSet<(
        ResMut<crate::ui::help_window::HelpWindowState>,
        ResMut<crate::ui::dev_console::DevConsoleState>,
        ResMut<crate::ui::tile_inspector_window::TileInspectorWindowState>,
        ResMut<crate::ui::endscreen::RetireGameRequested>,
        ResMut<crate::ui::ai_settings_window::AiSettingsWindowState>,
        ResMut<crate::ui::cargo_dist_settings_window::CargoDistSettingsWindowState>,
    )>,
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
            SaveMenuAction::PathfindingSettings => {
                pathfinding_settings.open = true;
            }
            SaveMenuAction::CargoDistSettings => {
                help_tools.p5().open = true;
            }
            SaveMenuAction::AiSettings => {
                help_tools.p4().open = true;
            }
            SaveMenuAction::NewGrf => {
                newgrf_window.open = true;
            }
            SaveMenuAction::DisplayOptions => {
                display_options.open = true;
            }
            SaveMenuAction::ExtraViewport => {
                extra_viewport.open = true;
            }
            SaveMenuAction::Help => {
                help_tools.p0().open = true;
            }
            SaveMenuAction::DevConsole => {
                help_tools.p1().open = true;
            }
            SaveMenuAction::TileInspector => {
                help_tools.p2().open = true;
            }
            SaveMenuAction::Cheats => {
                // Abierto en `handle_cheats_menu_button` (límite SystemParam).
            }
            SaveMenuAction::SaveScenario => {
                let dir = crate::state::scenarios_save_dir();
                let _ = std::fs::create_dir_all(&dir);
                save_window.open_in_mode(SaveWindowMode::Save, &dir);
            }
            SaveMenuAction::EndGame => {
                help_tools.p3().0 = true;
            }
            SaveMenuAction::CycleCatenaryDisplay => {
                let (Some(prefs), Some(pending_remap)) =
                    (prefs.as_deref_mut(), pending_remap.as_deref_mut())
                else {
                    continue;
                };
                use crate::sprites::{TransparencyMode, TransparencyOption};
                let next = match prefs.transparency_mode(TransparencyOption::Catenary) {
                    TransparencyMode::Visible => TransparencyMode::Transparent,
                    TransparencyMode::Transparent => TransparencyMode::Hidden,
                    TransparencyMode::Hidden => TransparencyMode::Visible,
                };
                prefs.set_transparency_mode(TransparencyOption::Catenary, next);
                crate::sprites::set_transparency_preferences(
                    prefs.transparency_opt,
                    prefs.invisibility_opt,
                );
                pending_remap.pending = true;
                pending_remap.full = true;
                info!("Catenaria: {}", next.label_es().to_ascii_lowercase());
            }
            SaveMenuAction::ReturnToMainMenu => {
                return_to_main_menu(&mut next_screen, &mut suspended);
            }
        }
    }
}

/// Abre la ventana de cheats desde Ajustes (separado por límite de SystemParam).
pub(crate) fn handle_cheats_menu_button(
    mut q: Query<(&Interaction, &SaveMenuAction), (Changed<Interaction>, With<Button>)>,
    mut cheat_window: ResMut<crate::ui::cheat_window::CheatWindowState>,
) {
    for (interaction, action) in &mut q {
        if *interaction == Interaction::Pressed && matches!(action, SaveMenuAction::Cheats) {
            cheat_window.open = true;
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
        let colour = swatch.0 % openttdrs_core::COMPANY_COLOUR_SLOTS;
        if sim.state.company_colour == colour {
            continue;
        }
        if openttdrs_core::company_colour_taken_by_other(
            &sim.state.companies,
            sim.state.active_company,
            colour,
        ) {
            continue;
        }
        if crate::network::apply_player_command(
            &mut sim.state,
            &openttdrs_core::Command::SetCompanyColour(colour),
        )
        .is_ok()
        {
            info!("Color compañía: {} ({colour})", company_colour_name(colour));
        }
    }
}

pub(crate) fn sync_company_colour_swatch_visuals(
    sim: Res<SimWorld>,
    mut q: Query<
        (
            &CompanyColourSwatch,
            &mut BorderColor,
            &mut BackgroundColor,
            &Interaction,
        ),
        (With<Button>, Without<SaveMenuAction>),
    >,
) {
    use crate::sprites::company_colour_swatch_color;

    let active = sim.state.company_colour % openttdrs_core::COMPANY_COLOUR_SLOTS;
    let active_id = sim.state.active_company;
    for (swatch, mut border, mut bg, interaction) in &mut q {
        let colour = swatch.0 % openttdrs_core::COMPANY_COLOUR_SLOTS;
        let taken =
            openttdrs_core::company_colour_taken_by_other(&sim.state.companies, active_id, colour);
        let base = company_colour_swatch_color(colour).to_srgba();
        *bg = if taken {
            BackgroundColor(Color::srgba(
                base.red * 0.4,
                base.green * 0.4,
                base.blue * 0.4,
                0.55,
            ))
        } else {
            BackgroundColor(Color::srgba(base.red, base.green, base.blue, 1.0))
        };
        let selected = colour == active;
        *border = if selected {
            BorderColor::all(Color::srgb(0.98, 0.92, 0.35))
        } else if taken {
            BorderColor::all(Color::srgb(0.35, 0.35, 0.32))
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
        world.insert_resource(
            crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState::default(),
        );
        world.insert_resource(
            crate::ui::cargo_dist_settings_window::CargoDistSettingsWindowState::default(),
        );
        world.insert_resource(crate::ui::ai_settings_window::AiSettingsWindowState::default());
        world.insert_resource(crate::ui::newgrf_window::NewGrfWindowState::default());
        world.insert_resource(
            crate::ui::display_options_window::DisplayOptionsWindowState::default(),
        );
        world
            .insert_resource(crate::ui::extra_viewport_window::ExtraViewportWindowState::default());
        world.insert_resource(crate::ui::help_window::HelpWindowState::default());
        world.insert_resource(crate::ui::dev_console::DevConsoleState::default());
        world
            .insert_resource(crate::ui::tile_inspector_window::TileInspectorWindowState::default());
        world.insert_resource(crate::ui::cheat_window::CheatWindowState::default());
        world.insert_resource(crate::ui::endscreen::RetireGameRequested::default());
        world.insert_resource(crate::ui::endscreen::EndScreenState::default());
        world.insert_resource(crate::settings::ClientPreferences::default());
        world.insert_resource(crate::render::RemapMapVisualsPending::default());
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
        world.insert_resource(
            crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState::default(),
        );
        world.insert_resource(
            crate::ui::cargo_dist_settings_window::CargoDistSettingsWindowState::default(),
        );
        world.insert_resource(crate::ui::ai_settings_window::AiSettingsWindowState::default());
        world.insert_resource(crate::ui::newgrf_window::NewGrfWindowState::default());
        world.insert_resource(
            crate::ui::display_options_window::DisplayOptionsWindowState::default(),
        );
        world
            .insert_resource(crate::ui::extra_viewport_window::ExtraViewportWindowState::default());
        world.insert_resource(crate::ui::help_window::HelpWindowState::default());
        world.insert_resource(crate::ui::dev_console::DevConsoleState::default());
        world
            .insert_resource(crate::ui::tile_inspector_window::TileInspectorWindowState::default());
        world.insert_resource(crate::ui::cheat_window::CheatWindowState::default());
        world.insert_resource(crate::ui::endscreen::RetireGameRequested::default());
        world.insert_resource(crate::ui::endscreen::EndScreenState::default());
        world.insert_resource(crate::settings::ClientPreferences::default());
        world.insert_resource(crate::render::RemapMapVisualsPending::default());
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
        let mut sim = SimWorld::default();
        sim.state.ensure_companies();
        world.insert_resource(sim);

        world.spawn((Button, CompanyColourSwatch(6), Interaction::Pressed));
        world
            .run_system_once(handle_company_colour_swatches)
            .unwrap();
        assert_eq!(world.resource::<SimWorld>().state.company_colour, 6);
        assert_eq!(world.resource::<SimWorld>().state.companies[0].colour, 6);
    }

    #[test]
    fn company_colour_swatch_ignores_taken_colour() {
        let mut world = World::new();
        let mut sim = SimWorld::default();
        sim.state.ensure_companies();
        sim.state.ensure_rival_transcargo();
        let rival_colour = sim
            .state
            .companies
            .iter()
            .find(|c| c.is_ai)
            .expect("rival")
            .colour;
        world.insert_resource(sim);

        world.spawn((
            Button,
            CompanyColourSwatch(rival_colour),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_company_colour_swatches)
            .unwrap();
        assert_eq!(world.resource::<SimWorld>().state.company_colour, 0);
    }

    #[test]
    fn catenary_button_cycles_visible_transparent_hidden() {
        let mut world = World::new();
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(NewsSettingsWindowState::default());
        world.insert_resource(
            crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState::default(),
        );
        world.insert_resource(
            crate::ui::cargo_dist_settings_window::CargoDistSettingsWindowState::default(),
        );
        world.insert_resource(crate::ui::ai_settings_window::AiSettingsWindowState::default());
        world.insert_resource(crate::ui::newgrf_window::NewGrfWindowState::default());
        world.insert_resource(
            crate::ui::display_options_window::DisplayOptionsWindowState::default(),
        );
        world
            .insert_resource(crate::ui::extra_viewport_window::ExtraViewportWindowState::default());
        world.insert_resource(crate::ui::help_window::HelpWindowState::default());
        world.insert_resource(crate::ui::dev_console::DevConsoleState::default());
        world
            .insert_resource(crate::ui::tile_inspector_window::TileInspectorWindowState::default());
        world.insert_resource(crate::ui::cheat_window::CheatWindowState::default());
        world.insert_resource(crate::ui::endscreen::RetireGameRequested::default());
        world.insert_resource(crate::ui::endscreen::EndScreenState::default());
        world.insert_resource(crate::settings::ClientPreferences::default());
        world.insert_resource(crate::render::RemapMapVisualsPending::default());
        world.insert_resource(NextState::<ClientScreen>::default());
        world.insert_resource(SuspendedGameSession::default());
        crate::state::insert_test_sim_run_state(&mut world);

        for expected in [
            crate::sprites::TransparencyMode::Transparent,
            crate::sprites::TransparencyMode::Hidden,
            crate::sprites::TransparencyMode::Visible,
        ] {
            let entity = world
                .spawn((
                    Button,
                    SaveMenuAction::CycleCatenaryDisplay,
                    Interaction::Pressed,
                ))
                .id();
            world.run_system_once(handle_settings_menu_buttons).unwrap();
            let prefs = world.resource::<crate::settings::ClientPreferences>();
            assert_eq!(
                prefs.transparency_mode(crate::sprites::TransparencyOption::Catenary),
                expected
            );
            world.despawn(entity);
        }
        assert!(
            world
                .resource::<crate::render::RemapMapVisualsPending>()
                .full
        );
    }
}
