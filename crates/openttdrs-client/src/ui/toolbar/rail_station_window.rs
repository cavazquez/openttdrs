//! Ventana «Selección de estación» de tren, estilo `OpenTTD`.
//!
//! Se abre al activar la herramienta de estación de tren y permite elegir
//! orientación (eje X/Y), número de andenes (1..=7), longitud de andén
//! (1..=7) y mostrar/ocultar el área de cobertura. Abajo informa qué carga
//! aceptaría/suministraría la estación en la tesela bajo el cursor.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use openttdrs_core::{STATION_COVERAGE_RADIUS, TileCoord, station_coverage_at};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::hud::HoveredTileCoord;

use super::{BuildMenuAction, BuildMenuUi, StationBuildState, UiToolState};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BG_SELECTED: Color = Color::srgb(0.55, 0.47, 0.3);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.92, 0.8, 0.5);

/// Botones de la ventana de selección de estación.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RailStationPickerButton {
    AxisX,
    AxisY,
    Platforms(u8),
    Length(u8),
    CoverageOff,
    CoverageOn,
}

#[derive(Component)]
pub(crate) struct RailStationAcceptsText;

#[derive(Component)]
pub(crate) struct RailStationSuppliesText;

pub(crate) fn setup_rail_station_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::RailStationPicker,
        "Selección de estación",
        TITLE_BROWN,
        Vec2::new(240.0, 64.0),
        230.0,
    );
    commands.entity(content).with_children(|panel| {
        spawn_section_label(panel, asset_server, "Orientación");
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                spawn_axis_button(
                    row,
                    asset_server,
                    RailStationPickerButton::AxisX,
                    "assets/opengfx/tiles/rail_platform_x_front.png",
                );
                spawn_axis_button(
                    row,
                    asset_server,
                    RailStationPickerButton::AxisY,
                    "assets/opengfx/tiles/rail_platform_y_front.png",
                );
            });
        spawn_section_label(panel, asset_server, "Número de andenes");
        spawn_number_row(panel, asset_server, RailStationPickerButton::Platforms);
        spawn_section_label(panel, asset_server, "Longitud de andén");
        spawn_number_row(panel, asset_server, RailStationPickerButton::Length);
        spawn_section_label(panel, asset_server, "Mostrar área de cobertura");
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_text_button(
                    row,
                    asset_server,
                    RailStationPickerButton::CoverageOff,
                    "Desactivado",
                    92.0,
                );
                spawn_text_button(
                    row,
                    asset_server,
                    RailStationPickerButton::CoverageOn,
                    "Activado",
                    92.0,
                );
            });
        panel.spawn((
            RailStationAcceptsText,
            Text::new("Acepta: Nada"),
            window_text_font(asset_server, 11.0),
            TextColor(Color::srgb(0.95, 0.9, 0.3)),
        ));
        panel.spawn((
            RailStationSuppliesText,
            Text::new("Suministra: Nada"),
            window_text_font(asset_server, 11.0),
            TextColor(Color::srgb(0.95, 0.9, 0.3)),
        ));
    });
}

fn spawn_section_label(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &'static str,
) {
    parent.spawn((
        Node {
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Text::new(label),
            window_text_font(asset_server, 11.0),
            TextColor(WINDOW_TEXT),
        )],
    ));
}

/// Botón de orientación con la imagen del andén (eje X o Y).
fn spawn_axis_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: RailStationPickerButton,
    image_path: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(92.0),
            height: Val::Px(54.0),
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
            ImageNode::new(asset_server.load::<Image>(image_path)),
            Node {
                width: Val::Px(64.0),
                height: Val::Px(40.0),
                ..default()
            },
        )],
    ));
}

fn spawn_number_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    make: fn(u8) -> RailStationPickerButton,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|row| {
            for n in 1..=7u8 {
                spawn_text_button(row, asset_server, make(n), number_label(n), 24.0);
            }
        });
}

const fn number_label(n: u8) -> &'static str {
    match n {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        _ => "7",
    }
}

fn spawn_text_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: RailStationPickerButton,
    label: &'static str,
    width: f32,
) {
    parent.spawn((
        Button,
        action,
        Node {
            width: Val::Px(width),
            height: Val::Px(20.0),
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
            window_text_font(asset_server, 11.0),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn button_is_selected(button: RailStationPickerButton, state: &StationBuildState) -> bool {
    match button {
        RailStationPickerButton::AxisX => !state.rail_axis_y,
        RailStationPickerButton::AxisY => state.rail_axis_y,
        RailStationPickerButton::Platforms(n) => state.rail_platforms == n,
        RailStationPickerButton::Length(n) => state.rail_length == n,
        RailStationPickerButton::CoverageOff => !state.rail_show_coverage,
        RailStationPickerButton::CoverageOn => state.rail_show_coverage,
    }
}

/// Texto «Acepta/Suministra» según la cobertura de la huella bajo el cursor.
fn coverage_texts(sim: &SimWorld, state: &StationBuildState, hover: TileCoord) -> (String, String) {
    let (w, h) = openttdrs_core::rail_station_footprint(
        state.rail_axis_y,
        state.rail_platforms,
        state.rail_length,
    );
    let anchor = TileCoord::new(hover.x + (w - 1) / 2, hover.y + (h - 1) / 2);
    let coverage = station_coverage_at(
        &sim.state.map,
        &sim.state.industries,
        anchor,
        STATION_COVERAGE_RADIUS,
    );
    let mut accepts: Vec<&str> = Vec::new();
    if coverage.accepts_mail > 0 {
        accepts.push("correo");
    }
    if coverage.accepts_goods > 0 {
        accepts.push("mercancías");
    }
    let mut supplies: Vec<&str> = Vec::new();
    if coverage.supplies_coal > 0 {
        supplies.push("carbón");
    }
    if coverage.supplies_wood > 0 {
        supplies.push("madera");
    }
    if coverage.supplies_oil > 0 {
        supplies.push("petróleo");
    }
    let join = |items: &[&str]| {
        if items.is_empty() {
            "Nada".to_string()
        } else {
            items.join(", ")
        }
    };
    (
        format!("Acepta: {}", join(&accepts)),
        format!("Suministra: {}", join(&supplies)),
    )
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_rail_station_picker(
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    sim: Res<SimWorld>,
    hovered: Res<HoveredTileCoord>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut buttons_q: Query<
        (
            &RailStationPickerButton,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut accepts_q: Query<&mut Text, With<RailStationAcceptsText>>,
    mut supplies_q: Query<
        &mut Text,
        (
            With<RailStationSuppliesText>,
            Without<RailStationAcceptsText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::RailStationPicker)
    else {
        return;
    };
    if tool_state.active_tool != Some(BuildMenuAction::RailStation) {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    for (button, interaction, mut bg, mut border) in &mut buttons_q {
        let selected = button_is_selected(*button, &station_state);
        *bg = if selected {
            BackgroundColor(BTN_BG_SELECTED)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.44, 0.38, 0.26))
        } else {
            BackgroundColor(BTN_BG)
        };
        *border = if selected {
            BorderColor::all(BTN_BORDER_SELECTED)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }

    if let Some(hover) = hovered.pos {
        let (accepts, supplies) = coverage_texts(&sim, &station_state, hover);
        if let Ok(mut text) = accepts_q.single_mut() {
            **text = accepts;
        }
        if let Ok(mut text) = supplies_q.single_mut() {
            **text = supplies;
        }
    }
}

pub(crate) fn handle_rail_station_picker_buttons(
    buttons_q: Query<
        (&Interaction, &RailStationPickerButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut station_state: ResMut<StationBuildState>,
) {
    for (interaction, button) in &buttons_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            RailStationPickerButton::AxisX => station_state.rail_axis_y = false,
            RailStationPickerButton::AxisY => station_state.rail_axis_y = true,
            RailStationPickerButton::Platforms(n) => station_state.rail_platforms = n,
            RailStationPickerButton::Length(n) => station_state.rail_length = n,
            RailStationPickerButton::CoverageOff => station_state.rail_show_coverage = false,
            RailStationPickerButton::CoverageOn => station_state.rail_show_coverage = true,
        }
    }
}

/// Cerrar la ventana con ✕ desactiva la herramienta (como en `OpenTTD`).
pub(crate) fn rail_station_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::RailStationPicker
            && tool_state.active_tool == Some(BuildMenuAction::RailStation)
        {
            tool_state.active_tool = None;
        }
    }
}
