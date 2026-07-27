//! Ventana «Selección de aeropuerto» (estilo `BuildAirportWindow` de OpenTTD).
//!
//! Se abre al activar la herramienta Aeropuerto: clase, tipo, orientación y cobertura.

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::{
    AirportClassId, AirportSpecId, STATION_COVERAGE_RADIUS, airport_class_def, airport_spec_def,
    airport_spec_footprint, list_airport_classes, list_airport_specs, station_coverage_at,
};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::HoveredTileCoord;
use crate::ui::scrollbar::spawn_classic_scroll_area_with;

use super::{BuildMenuAction, BuildMenuUi, StationBuildState, UiToolState};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AirportPickerButton {
    Class(AirportClassId),
    Spec(AirportSpecId),
    AxisX,
    AxisY,
    CoverageOff,
    CoverageOn,
}

#[derive(Component)]
pub(crate) struct AirportPickerSizeLabel;

#[derive(Component)]
pub(crate) struct AirportPickerCoverageText;

pub(crate) fn setup_airport_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::AirportPicker,
        "Selección de aeropuerto",
        TITLE_BROWN,
        Vec2::new(200.0, 48.0),
        320.0,
    );
    commands.entity(content).with_children(|panel| {
        spawn_section_label(panel, asset_server, "Clase");
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                for def in list_airport_classes("") {
                    spawn_text_button(
                        row,
                        asset_server,
                        AirportPickerButton::Class(def.id),
                        def.label,
                        88.0,
                    );
                }
            });
        spawn_section_label(panel, asset_server, "Tipo");
        spawn_classic_scroll_area_with(
            panel,
            asset_server,
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            BTN_BG,
            BTN_BORDER,
            (),
            (),
            |col| {
                for class in list_airport_classes("") {
                    for def in list_airport_specs(class.id, "") {
                        spawn_text_button(
                            col,
                            asset_server,
                            AirportPickerButton::Spec(def.id),
                            def.label,
                            280.0,
                        );
                    }
                }
            },
            200.0,
        );
        spawn_section_label(panel, asset_server, "Orientación");
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_text_button(row, asset_server, AirportPickerButton::AxisX, "Eje X", 72.0);
                spawn_text_button(row, asset_server, AirportPickerButton::AxisY, "Eje Y", 72.0);
            });
        spawn_section_label(panel, asset_server, "Cobertura");
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_text_button(
                    row,
                    asset_server,
                    AirportPickerButton::CoverageOff,
                    "Off",
                    72.0,
                );
                spawn_text_button(
                    row,
                    asset_server,
                    AirportPickerButton::CoverageOn,
                    "On",
                    72.0,
                );
            });
        panel.spawn((
            AirportPickerSizeLabel,
            Text::new("Tamaño: —"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ));
        panel.spawn((
            AirportPickerCoverageText,
            Text::new("Cobertura: —"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
    });
}

fn spawn_section_label(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer, label: &str) {
    parent.spawn((
        Text::new(label),
        window_text_font(asset_server, UiFontRole::Caption),
        TextColor(Color::srgb(0.85, 0.80, 0.65)),
        Node {
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        },
        BuildMenuUi,
    ));
}

fn spawn_text_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    marker: AirportPickerButton,
    label: &str,
    min_width: f32,
) {
    parent.spawn((
        Button,
        marker,
        Node {
            min_width: Val::Px(min_width),
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

fn airport_tool_active(tool: &UiToolState) -> bool {
    tool.active_tool == Some(BuildMenuAction::Airport)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_airport_picker(
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    sim: Res<SimWorld>,
    hovered: Option<Res<HoveredTileCoord>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (
            Without<AirportPickerSizeLabel>,
            Without<AirportPickerCoverageText>,
        ),
    >,
    mut size_q: Query<
        &mut Text,
        (
            With<AirportPickerSizeLabel>,
            Without<AirportPickerCoverageText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut coverage_q: Query<
        &mut Text,
        (
            With<AirportPickerCoverageText>,
            Without<AirportPickerSizeLabel>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut buttons: Query<(&AirportPickerButton, &mut BackgroundColor), With<Button>>,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::AirportPicker)
    else {
        return;
    };
    let open = airport_tool_active(&tool_state);
    *visibility = if open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !open {
        return;
    }

    let class = sim.state.current_airport_class;
    let spec = station_state.airport_spec;
    let axis_y = station_state.airport_axis_y;
    let (w, h) = airport_spec_footprint(spec, axis_y);
    let label = airport_spec_def(spec).map_or("—", |d| d.label);

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::AirportPicker)
    {
        **title = format!("Aeropuerto · {label}");
    }
    if let Ok(mut size) = size_q.single_mut() {
        **size = format!("Tamaño: {w}×{h}");
    }
    if let Ok(mut cov) = coverage_q.single_mut() {
        let radius = airport_spec_def(spec)
            .map(|d| d.catchment)
            .unwrap_or(STATION_COVERAGE_RADIUS);
        let text = if !station_state.airport_show_coverage {
            "Cobertura: oculta".to_string()
        } else if let Some(pos) = hovered.as_ref().and_then(|h| h.pos) {
            let coverage = station_coverage_at(&sim.state.map, &sim.state.industries, pos, radius);
            format!(
                "Cobertura r={radius}: casas {} · stock ind. {}",
                coverage.house_tiles, coverage.supplied_stock
            )
        } else {
            format!("Cobertura r={radius}: apunta al mapa")
        };
        **cov = text;
    }

    for (button, mut bg) in &mut buttons {
        let on = match *button {
            AirportPickerButton::Class(c) => c == class,
            AirportPickerButton::Spec(s) => s == spec,
            AirportPickerButton::AxisX => !axis_y,
            AirportPickerButton::AxisY => axis_y,
            AirportPickerButton::CoverageOff => !station_state.airport_show_coverage,
            AirportPickerButton::CoverageOn => station_state.airport_show_coverage,
        };
        // Ocultar specs de otra clase coloreando igual pero podríamos filtrar:
        let visible_spec = match *button {
            AirportPickerButton::Spec(s) => airport_spec_def(s).is_some_and(|d| d.class == class),
            _ => true,
        };
        *bg = BackgroundColor(if !visible_spec {
            Color::srgb(0.22, 0.20, 0.16)
        } else if on {
            BTN_ACTIVE
        } else {
            BTN_BG
        });
        let _ = airport_class_def(class);
    }
}

pub(crate) fn handle_airport_picker_buttons(
    buttons: Query<(&Interaction, &AirportPickerButton), (Changed<Interaction>, With<Button>)>,
    mut station_state: ResMut<StationBuildState>,
    mut sim: ResMut<SimWorld>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            AirportPickerButton::Class(class) => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetCurrentAirportClass(class),
                );
                station_state.airport_spec = sim.state.current_airport_spec;
            }
            AirportPickerButton::Spec(spec) => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetCurrentAirportSpec(spec),
                );
                station_state.airport_spec = sim.state.current_airport_spec;
            }
            AirportPickerButton::AxisX => station_state.airport_axis_y = false,
            AirportPickerButton::AxisY => station_state.airport_axis_y = true,
            AirportPickerButton::CoverageOff => station_state.airport_show_coverage = false,
            AirportPickerButton::CoverageOn => station_state.airport_show_coverage = true,
        }
    }
}

pub(crate) fn airport_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::AirportPicker && airport_tool_active(&tool_state) {
            tool_state.active_tool = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::state::SimWorld;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn picking_spec_updates_state() {
        let mut world = World::new();
        world.insert_resource(StationBuildState::default());
        world.insert_resource(SimWorld::default());
        world.spawn((
            Button,
            AirportPickerButton::Spec(AirportSpecId::Commuter),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_airport_picker_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<StationBuildState>().airport_spec,
            AirportSpecId::Commuter
        );
        assert_eq!(
            world.resource::<SimWorld>().state.current_airport_spec,
            AirportSpecId::Commuter
        );
    }

    #[test]
    fn class_button_selects_first_spec() {
        let mut world = World::new();
        world.insert_resource(StationBuildState {
            airport_spec: AirportSpecId::Small,
            ..Default::default()
        });
        world.insert_resource(SimWorld::default());
        world.spawn((
            Button,
            AirportPickerButton::Class(AirportClassId::Heliport),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_airport_picker_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<StationBuildState>().airport_spec,
            AirportSpecId::Heliport
        );
    }
}
