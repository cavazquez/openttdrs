use bevy::prelude::*;
use bevy::app::AppExit;

use crate::state::ClientScreen;

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct MainMenuStartButton;

#[derive(Component)]
pub(crate) struct MainMenuQuitButton;

#[derive(Component)]
pub(crate) struct MainMenuCamera;

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
            BackgroundColor(Color::srgba(0.06, 0.08, 0.11, 0.94)),
            GlobalZIndex(3000),
            MainMenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(96.0),
                    ..default()
                },
                children![(
                    Text::new("OpenTTDRS"),
                    TextFont {
                        font_size: 44.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.9, 0.72)),
                )],
            ));

            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
            ))
            .with_children(|menu| {
                menu.spawn((
                    Button,
                    MainMenuStartButton,
                    Node {
                        width: Val::Px(260.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
                    BorderColor::all(Color::srgb(0.7, 0.66, 0.5)),
                    Interaction::default(),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("Iniciar juego"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.92, 0.8)),
                    ));
                });

                menu.spawn((
                    Button,
                    MainMenuQuitButton,
                    Node {
                        width: Val::Px(260.0),
                        height: Val::Px(42.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
                    BorderColor::all(Color::srgb(0.58, 0.54, 0.4)),
                    Interaction::default(),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("Salir"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.91, 0.88, 0.76)),
                    ));
                });
            });
        });
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

pub(crate) fn main_menu_interaction(
    mut next_screen: ResMut<NextState<ClientScreen>>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    mut button_sets: ParamSet<(
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuStartButton>),
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
            Interaction::Pressed => {
                start_requested = true;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(Color::srgb(0.46, 0.42, 0.3));
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgb(0.35, 0.33, 0.24));
            }
        }
    }

    if start_requested {
        for e in &q_menu {
            commands.entity(e).despawn();
        }
        for cam in &q_menu_cam {
            commands.entity(cam).despawn();
        }
        next_screen.set(ClientScreen::InGame);
        return;
    }

    let mut quit_requested = quit_via_key;
    for (interaction, mut bg) in &mut button_sets.p1() {
        match *interaction {
            Interaction::Pressed => {
                quit_requested = true;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(Color::srgb(0.34, 0.3, 0.22));
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgb(0.24, 0.22, 0.16));
            }
        }
    }

    if quit_requested {
        exit.write(AppExit::Success);
    }
}
