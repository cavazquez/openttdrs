use bevy::prelude::*;

use crate::state::ClientScreen;

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct MainMenuStartButton;

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
            BackgroundColor(Color::srgba(0.08, 0.12, 0.16, 0.92)),
            GlobalZIndex(3000),
            MainMenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Button,
                MainMenuStartButton,
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(58.0),
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
                    Text::new("Iniciar"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.8)),
                ));
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
    mut q_button: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuStartButton>),
    >,
    mut commands: Commands,
) {
    for (interaction, mut bg) in &mut q_button {
        match *interaction {
            Interaction::Pressed => {
                for e in &q_menu {
                    commands.entity(e).despawn();
                }
                for cam in &q_menu_cam {
                    commands.entity(cam).despawn();
                }
                next_screen.set(ClientScreen::InGame);
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(Color::srgb(0.46, 0.42, 0.3));
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgb(0.35, 0.33, 0.24));
            }
        }
    }
}
