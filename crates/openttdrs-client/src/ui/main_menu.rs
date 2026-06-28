use bevy::app::AppExit;
use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::NewGameSettings;
use crate::state::{ClientScreen, SimWorld, new_game::NewGameSettingsResource};
use crate::ui::font::UiFontRole;

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct MainMenuStartButton;

#[derive(Component)]
pub(crate) struct MainMenuDemoButton;

#[derive(Component)]
pub(crate) struct MainMenuQuitButton;

#[derive(Component)]
pub(crate) struct MainMenuCamera;

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuClimateButton(pub Climate);

#[derive(Component, Clone, Copy)]
pub(crate) enum MainMenuToggle {
    WorldGen,
    Island,
    PreserveDemo,
}

#[derive(Component)]
pub(crate) struct MainMenuSummaryText;

fn climate_label(climate: Climate) -> &'static str {
    match climate {
        Climate::Temperate => "Templado",
        Climate::SubArctic => "Artico",
        Climate::SubTropical => "Tropical",
        Climate::Toyland => "Toyland",
    }
}

fn summary_text(settings: NewGameSettings) -> String {
    let mode = if settings.world_gen {
        if settings.island {
            "isla procedural"
        } else {
            "colinas procedural"
        }
    } else if settings.preserve_demo {
        "demo clasica (plana)"
    } else {
        "mapa plano"
    };
    format!(
        "Clima: {} · {} · semilla={}",
        climate_label(settings.climate),
        mode,
        if settings.seed == 0 {
            "auto".to_string()
        } else {
            settings.seed.to_string()
        }
    )
}

pub(crate) fn setup_main_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.07, 0.1, 0.96)),
            GlobalZIndex(3000),
            MainMenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(480.0),
                    height: Val::Px(420.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::new(
                        Val::Px(18.0),
                        Val::Px(18.0),
                        Val::Px(18.0),
                        Val::Px(14.0),
                    ),
                    border: UiRect::all(Val::Px(3.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.18, 0.17, 0.12, 0.96)),
                BorderColor::all(Color::srgb(0.74, 0.68, 0.5)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("OpenTTDRS"),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Title.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.91, 0.72)),
                ));

                panel.spawn((
                    Text::new("Nueva partida"),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.83, 0.79, 0.64)),
                ));

                panel
                    .spawn((Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },))
                    .with_children(|row| {
                        for climate in [
                            Climate::Temperate,
                            Climate::SubArctic,
                            Climate::SubTropical,
                            Climate::Toyland,
                        ] {
                            row.spawn(climate_button(climate));
                        }
                    });

                panel
                    .spawn((Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },))
                    .with_children(|toggles| {
                        toggles.spawn(toggle_button(
                            MainMenuToggle::WorldGen,
                            "Terreno procedural",
                        ));
                        toggles.spawn(toggle_button(MainMenuToggle::Island, "Modo isla (costas)"));
                        toggles.spawn(toggle_button(
                            MainMenuToggle::PreserveDemo,
                            "Incluir zona demo/tutorial",
                        ));
                    });

                panel.spawn((
                    MainMenuSummaryText,
                    Text::new(summary_text(NewGameSettingsResource::default().settings())),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.78, 0.74, 0.58)),
                ));

                panel
                    .spawn((Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },))
                    .with_children(|menu| {
                        menu.spawn(primary_button(MainMenuStartButton, "Iniciar partida", 50.0));
                        menu.spawn(secondary_button(
                            MainMenuDemoButton,
                            "Demo clasica (mapa plano)",
                            42.0,
                        ));
                        menu.spawn(secondary_button(MainMenuQuitButton, "Salir", 42.0));
                    });

                panel.spawn((
                    Text::new("Enter iniciar · Esc salir · 1-4 clima"),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.76, 0.72, 0.58)),
                ));
            });
        });
}

fn climate_button(climate: Climate) -> impl Bundle {
    (
        Button,
        MainMenuClimateButton(climate),
        Node {
            width: Val::Px(100.0),
            height: Val::Px(32.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
        Interaction::default(),
        children![(
            Text::new(climate_label(climate)),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.74)),
        )],
    )
}

fn toggle_button(toggle: MainMenuToggle, label: &'static str) -> impl Bundle {
    (
        Button,
        toggle,
        Node {
            width: Val::Px(360.0),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
        BorderColor::all(Color::srgb(0.55, 0.5, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

fn primary_button(marker: impl Component, label: &str, height: f32) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        BorderColor::all(Color::srgb(0.7, 0.66, 0.5)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Hud.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.8)),
        )],
    )
}

fn secondary_button(marker: impl Component, label: &str, height: f32) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.4)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.91, 0.88, 0.76)),
        )],
    )
}

pub(crate) fn setup_main_menu_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.16, 0.17, 0.2)),
            ..default()
        },
        MainMenuCamera,
    ));
}

pub(crate) fn sync_main_menu_summary(
    settings: Res<NewGameSettingsResource>,
    mut q: Query<&mut Text, With<MainMenuSummaryText>>,
) {
    if !settings.is_changed() {
        return;
    }
    for mut text in &mut q {
        text.0 = summary_text(settings.settings());
    }
}

pub(crate) fn main_menu_options_interaction(
    mut settings: ResMut<NewGameSettingsResource>,
    mut q_climate: Query<
        (&Interaction, &MainMenuClimateButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut q_toggle: Query<
        (&Interaction, &MainMenuToggle, &mut BackgroundColor),
        (Changed<Interaction>, Without<MainMenuClimateButton>),
    >,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        settings.0.climate = Climate::Temperate;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        settings.0.climate = Climate::SubArctic;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        settings.0.climate = Climate::SubTropical;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        settings.0.climate = Climate::Toyland;
    }

    for (interaction, btn, mut bg) in &mut q_climate {
        if *interaction == Interaction::Pressed {
            settings.0.climate = btn.0;
        }
        *bg = if settings.0.climate == btn.0 {
            BackgroundColor(Color::srgb(0.48, 0.42, 0.28))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.36, 0.32, 0.22))
        } else {
            BackgroundColor(Color::srgb(0.28, 0.26, 0.2))
        };
    }

    for (interaction, toggle, mut bg) in &mut q_toggle {
        if *interaction == Interaction::Pressed {
            match toggle {
                MainMenuToggle::WorldGen => settings.0.world_gen = !settings.0.world_gen,
                MainMenuToggle::Island => settings.0.island = !settings.0.island,
                MainMenuToggle::PreserveDemo => {
                    settings.0.preserve_demo = !settings.0.preserve_demo;
                }
            }
        }
        let on = match toggle {
            MainMenuToggle::WorldGen => settings.0.world_gen,
            MainMenuToggle::Island => settings.0.island,
            MainMenuToggle::PreserveDemo => settings.0.preserve_demo,
        };
        *bg = if on {
            BackgroundColor(Color::srgb(0.38, 0.44, 0.32))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.3, 0.28, 0.2))
        } else {
            BackgroundColor(Color::srgb(0.24, 0.22, 0.17))
        };
    }
}

fn enter_game(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    settings: NewGameSettings,
    next_screen: &mut NextState<ClientScreen>,
) {
    commands.insert_resource(SimWorld::from_new_game(&settings));
    for e in q_menu {
        commands.entity(e).despawn();
    }
    for cam in q_menu_cam {
        commands.entity(cam).despawn();
    }
    next_screen.set(ClientScreen::InGame);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_interaction(
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut settings: ResMut<NewGameSettingsResource>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    mut button_sets: ParamSet<(
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuStartButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuDemoButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitButton>),
        >,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    let start_via_key = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
    let quit_via_key = keys.just_pressed(KeyCode::Escape);

    let mut start_requested = start_via_key;
    for (interaction, mut bg) in &mut button_sets.p0() {
        match *interaction {
            Interaction::Pressed => start_requested = true,
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.46, 0.42, 0.3)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        }
    }

    if start_requested {
        enter_game(
            &mut commands,
            &q_menu,
            &q_menu_cam,
            settings.settings(),
            &mut next_screen,
        );
        return;
    }

    for (interaction, mut bg) in &mut button_sets.p1() {
        match *interaction {
            Interaction::Pressed => {
                settings.0 = NewGameSettings {
                    climate: Climate::Temperate,
                    world_gen: false,
                    island: false,
                    preserve_demo: true,
                    seed: 0,
                };
                enter_game(
                    &mut commands,
                    &q_menu,
                    &q_menu_cam,
                    settings.settings(),
                    &mut next_screen,
                );
                return;
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.34, 0.3, 0.22)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        }
    }

    let mut quit_requested = quit_via_key;
    for (interaction, mut bg) in &mut button_sets.p2() {
        match *interaction {
            Interaction::Pressed => quit_requested = true,
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.34, 0.3, 0.22)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        }
    }

    if quit_requested {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;

    #[test]
    fn setup_main_menu_and_camera_run() {
        let mut world = World::new();
        world.init_resource::<NewGameSettingsResource>();
        world.run_system_once(setup_main_menu).unwrap();
        world.run_system_once(setup_main_menu_camera).unwrap();
    }
}
