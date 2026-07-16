//! Ventana «Selección de estación» de tren, estilo `OpenTTD`.
//!
//! Se abre al activar la herramienta de estación de tren y permite elegir
//! clase/spec (catálogo NewGRF/vanilla), orientación (eje X/Y), número de
//! andenes (1..=7), longitud de andén (1..=7) y mostrar/ocultar el área de
//! cobertura. Abajo informa qué carga aceptaría/suministraría la estación
//! en la tesela bajo el cursor.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::EditableText;
use bevy::ui::widget::ImageNode;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    DecodedSprite, STATION_COVERAGE_RADIUS, StationClassId, StationSpecId, list_station_classes,
    list_station_specs, station_class_def, station_coverage_at, station_spec_def,
};
use std::collections::HashMap;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::HoveredTileCoord;

use super::{BuildMenuAction, BuildMenuUi, StationBuildState, UiToolState};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BG_SELECTED: Color = Color::srgb(0.55, 0.47, 0.3);
const BTN_BG_DISABLED: Color = Color::srgb(0.22, 0.20, 0.16);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_BORDER_SELECTED: Color = Color::srgb(0.92, 0.8, 0.5);
const ENTRY_BG: Color = Color::srgb(0.30, 0.26, 0.18);

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

/// Abre el dropdown de clase o de spec.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StationCatalogKind {
    Class,
    Spec,
}

#[derive(Resource, Default)]
pub(crate) struct StationCatalogPickerState {
    pub(crate) open: Option<StationCatalogKind>,
    pub(crate) filter: String,
}

/// Caché de thumbnails NewGRF (spec id → textura).
#[derive(Resource, Default)]
pub(crate) struct NewGrfStationPreviewCache {
    handles: HashMap<u16, Handle<Image>>,
}

impl NewGrfStationPreviewCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    fn handle_for(
        &mut self,
        id: StationSpecId,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handles
            .entry(id.as_u16())
            .or_insert_with(|| {
                images.add(Image::new(
                    Extent3d {
                        width: u32::from(sprite.width),
                        height: u32::from(sprite.height),
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    sprite.rgba.clone(),
                    TextureFormat::Rgba8UnormSrgb,
                    default(),
                ))
            })
            .clone()
    }
}

#[derive(Component)]
pub(crate) struct StationClassLabel;

#[derive(Component)]
pub(crate) struct StationSpecLabel;

#[derive(Component, Clone, Copy)]
pub(crate) struct StationCatalogOpenButton(pub StationCatalogKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct StationCatalogPopover(pub StationCatalogKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct StationCatalogFilterInput(pub StationCatalogKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct StationClassSelectButton(pub StationClassId);

#[derive(Component, Clone, Copy)]
pub(crate) struct StationSpecSelectButton(pub StationSpecId);

/// Thumbnail NewGRF de una entrada de spec.
#[derive(Component, Clone, Copy)]
pub(crate) struct StationSpecEntryPreview {
    pub id: StationSpecId,
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
        280.0,
    );
    commands.entity(content).with_children(|panel| {
        spawn_section_label(panel, asset_server, "Clase / tipo (NewGRF)");
        spawn_catalog_row(panel, asset_server);
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
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.95, 0.9, 0.3)),
        ));
        panel.spawn((
            RailStationSuppliesText,
            Text::new("Suministra: Nada"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.95, 0.9, 0.3)),
        ));
    });
}

fn spawn_catalog_row(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            spawn_catalog_dropdown(row, asset_server, StationCatalogKind::Class, "Dflt");
            spawn_catalog_dropdown(row, asset_server, StationCatalogKind::Spec, "Rail");
        });
}

fn spawn_catalog_dropdown(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    kind: StationCatalogKind,
    initial: &'static str,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|col| {
            col.spawn((
                Button,
                StationCatalogOpenButton(kind),
                BuildMenuUi,
                Node {
                    min_width: Val::Px(100.0),
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
            ))
            .with_children(|btn| match kind {
                StationCatalogKind::Class => {
                    btn.spawn((
                        StationClassLabel,
                        Text::new(initial),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(Color::srgb(0.92, 0.88, 0.72)),
                    ));
                }
                StationCatalogKind::Spec => {
                    btn.spawn((
                        StationSpecLabel,
                        Text::new(initial),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(Color::srgb(0.92, 0.88, 0.72)),
                    ));
                }
            });
            col.spawn((
                StationCatalogPopover(kind),
                BuildMenuUi,
                Node {
                    width: Val::Px(160.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    border: UiRect::all(Val::Px(1.0)),
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    top: Val::Px(26.0),
                    left: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.20, 0.18, 0.14)),
                BorderColor::all(BTN_BORDER),
                GlobalZIndex(2300),
            ))
            .with_children(|menu| {
                menu.spawn((
                    StationCatalogFilterInput(kind),
                    EditableText::new(""),
                    Text::new("filtrar…"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(Color::srgb(0.75, 0.72, 0.60)),
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(20.0),
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(ENTRY_BG),
                    BorderColor::all(BTN_BORDER),
                ));
                // Entradas iniciales vacías: `sync_station_catalog_entries` las rellena.
            });
        });
}

fn spawn_catalog_entry<B: Component>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: impl Into<String>,
    button: B,
) {
    let label = label.into();
    parent.spawn((
        Button,
        button,
        BuildMenuUi,
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(ENTRY_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_spec_catalog_entry(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    id: StationSpecId,
    label: impl Into<String>,
) {
    let label = label.into();
    parent
        .spawn((
            Button,
            StationSpecSelectButton(id),
            BuildMenuUi,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(28.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                column_gap: Val::Px(6.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Row,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(ENTRY_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
        ))
        .with_children(|row| {
            row.spawn((
                StationSpecEntryPreview { id },
                ImageNode::default(),
                Node {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    flex_shrink: 0.0,
                    display: Display::None,
                    ..default()
                },
            ));
            row.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
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
            window_text_font(asset_server, UiFontRole::Caption),
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
            window_text_font(asset_server, UiFontRole::Caption),
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
    catalog: Res<StationCatalogPickerState>,
    mut root_q: Query<
        (&FloatingWindow, &mut Visibility),
        (
            Without<StationClassSelectButton>,
            Without<StationSpecSelectButton>,
        ),
    >,
    mut buttons_q: Query<
        (
            &RailStationPickerButton,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut accepts_q: Query<
        &mut Text,
        (
            With<RailStationAcceptsText>,
            Without<RailStationSuppliesText>,
            Without<StationClassLabel>,
            Without<StationSpecLabel>,
        ),
    >,
    mut supplies_q: Query<
        &mut Text,
        (
            With<RailStationSuppliesText>,
            Without<RailStationAcceptsText>,
            Without<StationClassLabel>,
            Without<StationSpecLabel>,
        ),
    >,
    mut class_label: Query<
        &mut Text,
        (
            With<StationClassLabel>,
            Without<RailStationAcceptsText>,
            Without<RailStationSuppliesText>,
            Without<StationSpecLabel>,
        ),
    >,
    mut spec_label: Query<
        &mut Text,
        (
            With<StationSpecLabel>,
            Without<StationClassLabel>,
            Without<RailStationAcceptsText>,
            Without<RailStationSuppliesText>,
        ),
    >,
    mut popovers: Query<(&StationCatalogPopover, &mut Node)>,
    mut class_entries: Query<
        (&StationClassSelectButton, &mut Visibility),
        (Without<FloatingWindow>, Without<StationSpecSelectButton>),
    >,
    mut spec_entries: Query<
        (&StationSpecSelectButton, &mut Visibility),
        (Without<FloatingWindow>, Without<StationClassSelectButton>),
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

    let spec = station_spec_def(
        &sim.state.station_spec_catalog,
        sim.state.current_station_spec,
    );
    for (button, interaction, mut bg, mut border) in &mut buttons_q {
        let allowed = match *button {
            RailStationPickerButton::Platforms(n) => spec.is_none_or(|s| s.allows_platforms(n)),
            RailStationPickerButton::Length(n) => spec.is_none_or(|s| s.allows_length(n)),
            _ => true,
        };
        if !allowed {
            *bg = BackgroundColor(BTN_BG_DISABLED);
            *border = BorderColor::all(Color::srgb(0.35, 0.32, 0.28));
            continue;
        }
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

    if let Ok(mut text) = class_label.single_mut() {
        let short = station_class_def(
            &sim.state.station_class_catalog,
            sim.state.current_station_class,
        )
        .map(|c| c.short_label.as_str())
        .unwrap_or("?");
        **text = short.into();
    }
    if let Ok(mut text) = spec_label.single_mut() {
        let short = station_spec_def(
            &sim.state.station_spec_catalog,
            sim.state.current_station_spec,
        )
        .map(|s| s.short_label.as_str())
        .unwrap_or("?");
        **text = short.into();
    }

    for (popover, mut node) in &mut popovers {
        node.display = if catalog.open == Some(popover.0) {
            Display::Flex
        } else {
            Display::None
        };
    }

    if catalog.open == Some(StationCatalogKind::Class) {
        let matched: Vec<_> =
            list_station_classes(&sim.state.station_class_catalog, &catalog.filter)
                .into_iter()
                .map(|c| c.id)
                .collect();
        for (entry, mut vis) in &mut class_entries {
            *vis = if matched.contains(&entry.0) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    if catalog.open == Some(StationCatalogKind::Spec) {
        let matched: Vec<_> = list_station_specs(
            &sim.state.station_spec_catalog,
            sim.state.current_station_class,
            &catalog.filter,
        )
        .into_iter()
        .map(|s| s.id)
        .collect();
        for (entry, mut vis) in &mut spec_entries {
            *vis = if matched.contains(&entry.0) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
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
    sim: Res<SimWorld>,
) {
    let spec = station_spec_def(
        &sim.state.station_spec_catalog,
        sim.state.current_station_spec,
    );
    for (interaction, button) in &buttons_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            RailStationPickerButton::AxisX => station_state.rail_axis_y = false,
            RailStationPickerButton::AxisY => station_state.rail_axis_y = true,
            RailStationPickerButton::Platforms(n) => {
                if spec.is_none_or(|s| s.allows_platforms(n)) {
                    station_state.rail_platforms = n;
                }
            }
            RailStationPickerButton::Length(n) => {
                if spec.is_none_or(|s| s.allows_length(n)) {
                    station_state.rail_length = n;
                }
            }
            RailStationPickerButton::CoverageOff => station_state.rail_show_coverage = false,
            RailStationPickerButton::CoverageOn => station_state.rail_show_coverage = true,
        }
    }
}

pub(crate) fn handle_station_catalog_open_buttons(
    buttons: Query<(&Interaction, &StationCatalogOpenButton), (Changed<Interaction>, With<Button>)>,
    mut catalog: ResMut<StationCatalogPickerState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        catalog.open = if catalog.open == Some(button.0) {
            None
        } else {
            Some(button.0)
        };
        if catalog.open.is_none() {
            catalog.filter.clear();
        }
    }
}

pub(crate) fn handle_station_class_select_buttons(
    buttons: Query<(&Interaction, &StationClassSelectButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
    mut catalog: ResMut<StationCatalogPickerState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let _ = crate::network::apply_player_command(
            &mut sim.state,
            &Command::SetCurrentStationClass(button.0),
        );
        catalog.open = None;
        catalog.filter.clear();
    }
}

pub(crate) fn handle_station_spec_select_buttons(
    buttons: Query<(&Interaction, &StationSpecSelectButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
    mut catalog: ResMut<StationCatalogPickerState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let _ = crate::network::apply_player_command(
            &mut sim.state,
            &Command::SetCurrentStationSpec(button.0),
        );
        catalog.open = None;
        catalog.filter.clear();
    }
}

/// Asigna thumbnails NewGRF a las entradas de spec.
pub(crate) fn sync_station_spec_entry_previews(
    sim: Res<SimWorld>,
    mut cache: ResMut<NewGrfStationPreviewCache>,
    mut images: ResMut<Assets<Image>>,
    mut previews: Query<(&StationSpecEntryPreview, &mut ImageNode, &mut Node)>,
) {
    for (preview, mut image, mut node) in &mut previews {
        let Some(def) = station_spec_def(&sim.state.station_spec_catalog, preview.id) else {
            node.display = Display::None;
            continue;
        };
        let Some(sprite) = def.newgrf_preview_sprite() else {
            node.display = Display::None;
            continue;
        };
        image.image = cache.handle_for(preview.id, sprite, &mut images);
        node.display = Display::Flex;
    }
}

/// Añade entradas del catálogo que aún no tienen botón (tras apply NewGRF Stations).
pub(crate) fn sync_station_catalog_entries(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    popovers: Query<(Entity, &StationCatalogPopover)>,
    class_entries: Query<&StationClassSelectButton>,
    spec_entries: Query<&StationSpecSelectButton>,
) {
    let existing_classes: std::collections::HashSet<u16> =
        class_entries.iter().map(|e| e.0.as_u16()).collect();
    let existing_specs: std::collections::HashSet<u16> =
        spec_entries.iter().map(|e| e.0.as_u16()).collect();
    for (popover_entity, popover) in &popovers {
        match popover.0 {
            StationCatalogKind::Class => {
                for def in list_station_classes(&sim.state.station_class_catalog, "") {
                    if existing_classes.contains(&def.id.as_u16()) {
                        continue;
                    }
                    let id = def.id;
                    let label = def.label.clone();
                    commands.entity(popover_entity).with_children(|menu| {
                        spawn_catalog_entry(
                            menu,
                            &asset_server,
                            label,
                            StationClassSelectButton(id),
                        );
                    });
                }
            }
            StationCatalogKind::Spec => {
                for def in &sim.state.station_spec_catalog {
                    if existing_specs.contains(&def.id.as_u16()) {
                        continue;
                    }
                    let id = def.id;
                    let label = def.label.clone();
                    commands.entity(popover_entity).with_children(|menu| {
                        spawn_spec_catalog_entry(menu, &asset_server, id, label);
                    });
                }
            }
        }
    }
}

pub(crate) fn station_catalog_filter_keyboard(
    mut key_events: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut catalog: ResMut<StationCatalogPickerState>,
    mut inputs: Query<(&StationCatalogFilterInput, &mut EditableText, &mut Text)>,
) {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::Key;

    let Some(open) = catalog.open else {
        key_events.clear();
        return;
    };
    let Some((_, mut editable, mut text)) = inputs.iter_mut().find(|(input, _, _)| input.0 == open)
    else {
        key_events.clear();
        return;
    };
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            editable.queue_edit(bevy::text::TextEdit::Backspace);
            continue;
        }
        if matches!(ev.logical_key, Key::Delete) {
            editable.queue_edit(bevy::text::TextEdit::Delete);
            continue;
        }
        let Some(typed) = &ev.text else {
            continue;
        };
        for c in typed.chars() {
            if !c.is_control() && editable.value().chars().count() < 24 {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(c.to_string()),
                ));
            }
        }
    }
    catalog.filter = editable.value().to_string();
    if catalog.filter.is_empty() {
        **text = "filtrar…".into();
    } else {
        **text = catalog.filter.clone();
    }
}

/// Cerrar la ventana con ✕ desactiva la herramienta (como en `OpenTTD`).
pub(crate) fn rail_station_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
    mut catalog: ResMut<StationCatalogPickerState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::RailStationPicker
            && tool_state.active_tool == Some(BuildMenuAction::RailStation)
        {
            tool_state.active_tool = None;
            catalog.open = None;
            catalog.filter.clear();
        }
    }
}
