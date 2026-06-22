//! Spawn del modal de guardar/cargar partidas (oculto hasta abrirse).

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, GlobalZIndex};

use crate::ui::toolbar::BuildMenuUi;

use super::{
    SAVE_WINDOW_ROWS, SaveWindowButton, SaveWindowConfirmText, SaveWindowNameRow,
    SaveWindowNameText, SaveWindowPageText, SaveWindowRoot, SaveWindowRow, SaveWindowRowText,
    SaveWindowStatusText, SaveWindowTitle,
};

const UI_FONT: &str = "static/fonts/DejaVuSansMono.ttf";

pub(crate) fn setup_save_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let ui_font = asset_server.load::<Font>(UI_FONT);

    commands
        .spawn((
            SaveWindowRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.55)),
            GlobalZIndex(2900),
            Visibility::Hidden,
            FocusPolicy::Block,
            BuildMenuUi,
            Interaction::default(),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(560.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(12.0)),
                        border: UiRect::all(Val::Px(3.0)),
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.16, 0.13, 0.09, 0.98)),
                    BorderColor::all(Color::srgb(0.74, 0.66, 0.45)),
                    FocusPolicy::Block,
                    BuildMenuUi,
                ))
                .with_children(|panel| {
                    panel.spawn((
                        SaveWindowTitle,
                        Text::new("Cargar partida"),
                        TextFont {
                            font: ui_font.clone().into(),
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.91, 0.72)),
                        BuildMenuUi,
                    ));

                    panel
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(2.0),
                                ..default()
                            },
                            BuildMenuUi,
                        ))
                        .with_children(|list| {
                            for slot in 0..SAVE_WINDOW_ROWS {
                                spawn_save_row(list, &ui_font, slot);
                            }
                        });

                    panel
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                ..default()
                            },
                            BuildMenuUi,
                        ))
                        .with_children(|row| {
                            spawn_small_button(row, &ui_font, SaveWindowButton::PrevPage, "<");
                            row.spawn((
                                SaveWindowPageText,
                                Text::new("1/1"),
                                TextFont {
                                    font: ui_font.clone().into(),
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.85, 0.81, 0.66)),
                                BuildMenuUi,
                            ));
                            spawn_small_button(row, &ui_font, SaveWindowButton::NextPage, ">");
                        });

                    panel
                        .spawn((
                            SaveWindowNameRow,
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                display: Display::None,
                                ..default()
                            },
                            BuildMenuUi,
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new("Nombre:"),
                                TextFont {
                                    font: ui_font.clone().into(),
                                    font_size: FontSize::Px(13.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.86, 0.7)),
                                BuildMenuUi,
                            ));
                            row.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    height: Val::Px(26.0),
                                    padding: UiRect::horizontal(Val::Px(6.0)),
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.1, 0.08, 0.06)),
                                BorderColor::all(Color::srgb(0.6, 0.53, 0.36)),
                                BuildMenuUi,
                                children![(
                                    SaveWindowNameText,
                                    Text::new(""),
                                    TextFont {
                                        font: ui_font.clone().into(),
                                        font_size: FontSize::Px(13.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.95, 0.93, 0.8)),
                                )],
                            ));
                        });

                    panel.spawn((
                        SaveWindowStatusText,
                        Text::new(""),
                        TextFont {
                            font: ui_font.clone().into(),
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.93, 0.72, 0.5)),
                        BuildMenuUi,
                    ));

                    panel
                        .spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::FlexEnd,
                                column_gap: Val::Px(8.0),
                                ..default()
                            },
                            BuildMenuUi,
                        ))
                        .with_children(|row| {
                            spawn_action_button(
                                row,
                                &ui_font,
                                SaveWindowButton::Delete,
                                "Borrar",
                                Color::srgb(0.45, 0.26, 0.2),
                                None,
                            );
                            spawn_action_button(
                                row,
                                &ui_font,
                                SaveWindowButton::Cancel,
                                "Cancelar",
                                Color::srgb(0.3, 0.27, 0.2),
                                None,
                            );
                            spawn_action_button(
                                row,
                                &ui_font,
                                SaveWindowButton::Confirm,
                                "Cargar",
                                Color::srgb(0.32, 0.42, 0.24),
                                Some(SaveWindowConfirmText),
                            );
                        });
                });
        });
}

fn spawn_save_row(parent: &mut ChildSpawnerCommands, ui_font: &Handle<Font>, slot: usize) {
    parent.spawn((
        SaveWindowRow { slot },
        Button,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
        BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            SaveWindowRowText { slot },
            Text::new(""),
            TextFont {
                font: ui_font.clone().into(),
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_small_button(
    parent: &mut ChildSpawnerCommands,
    ui_font: &Handle<Font>,
    action: SaveWindowButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(28.0),
            height: Val::Px(22.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.34, 0.29, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.55, 0.38)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            TextFont {
                font: ui_font.clone().into(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    ui_font: &Handle<Font>,
    action: SaveWindowButton,
    label: &'static str,
    bg: Color,
    confirm_marker: Option<SaveWindowConfirmText>,
) {
    let mut entity = parent.spawn((
        Button,
        action,
        Node {
            min_width: Val::Px(96.0),
            height: Val::Px(30.0),
            padding: UiRect::horizontal(Val::Px(10.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        BuildMenuUi,
    ));
    entity.with_children(|b| {
        let mut text = b.spawn((
            Text::new(label),
            TextFont {
                font: ui_font.clone().into(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.93, 0.8)),
        ));
        if let Some(marker) = confirm_marker {
            text.insert(marker);
        }
    });
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use super::setup_save_window;

    #[test]
    fn setup_save_window_spawns_hidden_modal() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Font>();
        app.update();
        app.world_mut()
            .run_system_once(setup_save_window)
            .expect("setup");

        let mut q = app
            .world_mut()
            .query::<(&super::SaveWindowRoot, &Visibility)>();
        let (_, vis) = q.single(app.world()).expect("root");
        assert_eq!(*vis, Visibility::Hidden);
    }
}
