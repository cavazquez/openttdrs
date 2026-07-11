//! Panel de selección de señales (tipo + densidad) al activar la herramienta.

use bevy::prelude::*;
use openttdrs_core::{
    SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH, SIGTYPE_PATH_ONEWAY,
    signal_type_label,
};

use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;

use super::{BuildMenuAction, BuildMenuUi, StationBuildState, UiToolState};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);

const SIGNAL_TYPES: [(u8, &str); 6] = [
    (SIGTYPE_BLOCK, "Block"),
    (SIGTYPE_ENTRY, "Entry"),
    (SIGTYPE_EXIT, "Exit"),
    (SIGTYPE_COMBO, "Combo"),
    (SIGTYPE_PATH, "Path"),
    (SIGTYPE_PATH_ONEWAY, "Path 1vía"),
];

const DENSITIES: [u8; 7] = [1, 2, 4, 8, 12, 16, 20];

#[derive(Component, Clone, Copy)]
pub(crate) enum SignalPickerButton {
    Type(u8),
    Density(u8),
}

pub(crate) fn setup_signal_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::SignalPicker,
        "Señales",
        TITLE_BROWN,
        Vec2::new(200.0, 160.0),
        320.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            Text::new("Tipo (Ctrl+clic en mapa cicla)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                for (sig, label) in SIGNAL_TYPES {
                    spawn_chip(row, asset_server, SignalPickerButton::Type(sig), label);
                }
            });
        panel.spawn((
            Text::new("Densidad al arrastrar (Shift+RMB cicla)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                for d in DENSITIES {
                    spawn_chip(
                        row,
                        asset_server,
                        SignalPickerButton::Density(d),
                        &d.to_string(),
                    );
                }
            });
    });
}

fn spawn_chip(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    button: SignalPickerButton,
    label: &str,
) {
    parent.spawn((
        Button,
        button,
        Node {
            min_width: Val::Px(52.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
}

pub(crate) fn sync_signal_picker(
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(
        &crate::ui::floating_window::FloatingWindowTitleText,
        &mut Text,
    )>,
    mut buttons: Query<(&SignalPickerButton, &mut BackgroundColor), With<Button>>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::SignalPicker)
    else {
        return;
    };
    if tool_state.active_tool != Some(BuildMenuAction::RailSignals) {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::SignalPicker)
    {
        **title = format!(
            "Señales · {} · dens {}",
            signal_type_label(station_state.signal_type),
            station_state.signal_density
        );
    }
    for (button, mut bg) in &mut buttons {
        let on = match *button {
            SignalPickerButton::Type(t) => station_state.signal_type == t,
            SignalPickerButton::Density(d) => station_state.signal_density == d,
        };
        *bg = BackgroundColor(if on { BTN_ACTIVE } else { BTN_BG });
    }
}

pub(crate) fn handle_signal_picker_buttons(
    buttons: Query<(&Interaction, &SignalPickerButton), (Changed<Interaction>, With<Button>)>,
    mut station_state: ResMut<StationBuildState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            SignalPickerButton::Type(t) => station_state.signal_type = t,
            SignalPickerButton::Density(d) => station_state.signal_density = d,
        }
    }
}

pub(crate) fn signal_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::SignalPicker
            && tool_state.active_tool == Some(BuildMenuAction::RailSignals)
        {
            tool_state.active_tool = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn picking_signal_type_updates_station_state() {
        let mut world = World::new();
        world.insert_resource(StationBuildState::default());
        world.spawn((
            Button,
            SignalPickerButton::Type(SIGTYPE_BLOCK),
            Interaction::Pressed,
        ));
        world.run_system_once(handle_signal_picker_buttons).unwrap();
        assert_eq!(
            world.resource::<StationBuildState>().signal_type,
            SIGTYPE_BLOCK
        );
    }
}
