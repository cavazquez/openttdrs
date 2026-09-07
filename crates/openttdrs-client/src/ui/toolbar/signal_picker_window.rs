//! Panel de selección de señales (tipo + densidad) al activar la herramienta.

use bevy::prelude::*;
use openttdrs_core::{
    SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH, SIGTYPE_PATH_ONEWAY,
};

use crate::i18n::{Locale, localized_text};
use crate::settings::ClientPreferences;
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
    (SIGTYPE_BLOCK, "Bloque"),
    (SIGTYPE_ENTRY, "Entrada"),
    (SIGTYPE_EXIT, "Salida"),
    (SIGTYPE_COMBO, "Combinada"),
    (SIGTYPE_PATH, "Ruta PBS"),
    (SIGTYPE_PATH_ONEWAY, "Ruta 1vía"),
];

const DENSITIES: [u8; 7] = [1, 2, 4, 8, 12, 16, 20];
const SIGNAL_VARIANTS: [(u8, &str); 2] = [(0, "Eléctrica"), (1, "Semáforo")];

fn signal_type_source(signal_type: u8) -> &'static str {
    SIGNAL_TYPES
        .iter()
        .find_map(|(value, label)| (*value == signal_type).then_some(*label))
        .unwrap_or("Bloque")
}

fn signal_variant_source(signal_variant: u8) -> &'static str {
    SIGNAL_VARIANTS
        .iter()
        .find_map(|(value, label)| (*value == signal_variant).then_some(*label))
        .unwrap_or("Eléctrica")
}

fn signal_picker_title(locale: Locale, state: &StationBuildState) -> String {
    format!(
        "{} · {} · {} · {} {}",
        localized_text(locale, "Señales"),
        localized_text(locale, signal_type_source(state.signal_type)),
        localized_text(locale, signal_variant_source(state.signal_variant)),
        localized_text(locale, "densidad"),
        state.signal_density,
    )
}

#[derive(Component, Clone, Copy)]
pub(crate) enum SignalPickerButton {
    Type(u8),
    Variant(u8),
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
            Text::new("Tipo (Ctrl+clic cicla; Ctrl+Shift cambia estilo)"),
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
            Text::new("Estilo"),
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
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                for (variant, label) in SIGNAL_VARIANTS {
                    spawn_chip(
                        row,
                        asset_server,
                        SignalPickerButton::Variant(variant),
                        label,
                    );
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
    prefs: Res<ClientPreferences>,
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
        **title = signal_picker_title(prefs.locale(), &station_state);
    }
    for (button, mut bg) in &mut buttons {
        let on = match *button {
            SignalPickerButton::Type(t) => station_state.signal_type == t,
            SignalPickerButton::Variant(v) => station_state.signal_variant == v,
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
            SignalPickerButton::Variant(v) => station_state.signal_variant = v,
            SignalPickerButton::Density(d) => station_state.signal_density = d,
        }
    }
}

pub(crate) fn signal_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::SignalPicker
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

    #[test]
    fn picking_semaphore_updates_station_state() {
        let mut world = World::new();
        world.insert_resource(StationBuildState::default());
        world.spawn((Button, SignalPickerButton::Variant(1), Interaction::Pressed));
        world.run_system_once(handle_signal_picker_buttons).unwrap();
        assert_eq!(world.resource::<StationBuildState>().signal_variant, 1);
    }

    #[test]
    fn signal_picker_on_closed_clears_signals_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::RailSignals),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::SignalPicker),
        ));
        world.run_system_once(signal_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }

    #[test]
    fn signal_picker_title_uses_the_active_locale_without_changing_values() {
        let state = StationBuildState {
            signal_type: SIGTYPE_PATH_ONEWAY,
            signal_variant: 1,
            signal_density: 12,
            ..Default::default()
        };

        assert_eq!(
            signal_picker_title(Locale::Es, &state),
            "Señales · Ruta 1vía · Semáforo · densidad 12"
        );
        assert_eq!(
            signal_picker_title(Locale::En, &state),
            "Signals · One-way path · Semaphore · density 12"
        );
        assert_eq!(state.signal_type, SIGTYPE_PATH_ONEWAY);
        assert_eq!(state.signal_variant, 1);
        assert_eq!(state.signal_density, 12);
    }
}
