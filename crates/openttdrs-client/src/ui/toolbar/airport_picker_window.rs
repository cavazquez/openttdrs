//! Ventana «Selección de aeropuerto» (estilo `BuildAirportWindow` de OpenTTD).
//!
//! Se abre al activar la herramienta Aeropuerto: clase, tipo, orientación y cobertura.

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::{
    AirportClassId, AirportSpecId, STATION_COVERAGE_RADIUS, airport_spec_def,
    airport_spec_footprint, list_airport_classes, list_airport_specs,
    newgrf_airport_footprint_with_layout, newgrf_airport_layout_selection_with_index,
    newgrf_airport_spec_def, station_coverage_at,
};

use crate::i18n::{Locale, localized_text};
use crate::render::NewGrfAction5SpriteCache;
use crate::settings::ClientPreferences;
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
/// Espacio de cache separado para previews Action3 de Airport NewGRF.
const AIRPORT_NEWGRF_PREVIEW_CACHE_TYPE: u8 = 0xFD;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AirportPickerButton {
    Class(AirportClassId),
    Spec(AirportSpecId),
    NewgrfSpec(u16),
    AxisX,
    AxisY,
    LayoutPrevious,
    LayoutNext,
    CoverageOff,
    CoverageOn,
}

#[derive(Component)]
pub(crate) struct AirportPickerSizeLabel;

#[derive(Component)]
pub(crate) struct AirportPickerCoverageText;

/// Miniatura Action5 `0x16` del spec seleccionado.
#[derive(Component)]
pub(crate) struct AirportPickerPreviewImage;

/// Lista dinámica: los Airports Action0 pueden aparecer después del setup UI.
#[derive(Component)]
pub(crate) struct AirportPickerSpecList;

#[derive(Component)]
pub(crate) struct AirportPickerLayoutText;

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
        spawn_section_label(panel, asset_server, "Vista previa");
        panel.spawn((
            AirportPickerPreviewImage,
            ImageNode::default(),
            Node {
                width: Val::Px(96.0),
                height: Val::Px(48.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
        ));
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
                col.spawn((
                    AirportPickerSpecList,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ))
                .with_children(|list| {
                    for class in list_airport_classes("") {
                        for def in list_airport_specs(class.id, "") {
                            spawn_text_button(
                                list,
                                asset_server,
                                AirportPickerButton::Spec(def.id),
                                def.label,
                                280.0,
                            );
                        }
                    }
                });
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
                spawn_text_button(
                    row,
                    asset_server,
                    AirportPickerButton::LayoutPrevious,
                    "◀",
                    24.0,
                );
                row.spawn((
                    AirportPickerLayoutText,
                    Text::new("Layout —"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                    Node {
                        min_width: Val::Px(86.0),
                        align_self: AlignSelf::Center,
                        ..default()
                    },
                ));
                spawn_text_button(
                    row,
                    asset_server,
                    AirportPickerButton::LayoutNext,
                    "▶",
                    24.0,
                );
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
                    "Desactivado",
                    72.0,
                );
                spawn_text_button(
                    row,
                    asset_server,
                    AirportPickerButton::CoverageOn,
                    "Activado",
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

fn active_newgrf_airport(sim: &SimWorld) -> Option<&openttdrs_core::NewgrfAirportSpecDef> {
    sim.state
        .current_airport_newgrf_id
        .and_then(|id| newgrf_airport_spec_def(&sim.state.airport_spec_catalog, id))
}

fn selected_newgrf_layout(sim: &SimWorld, station_state: &StationBuildState) -> Option<(u8, u8)> {
    let def = active_newgrf_airport(sim)?;
    let requested = station_state.airport_layout.unwrap_or(0);
    newgrf_airport_layout_selection_with_index(def, Some(requested), station_state.airport_axis_y)
}

fn airport_axis_for_rotation(rotation: u8) -> bool {
    matches!(rotation & 6, 2 | 6)
}

fn airport_layout_label(selection: Option<(u8, u8)>, layout_count: usize) -> String {
    let Some((index, rotation)) = selection else {
        return "Layout —".into();
    };
    let direction = match rotation & 6 {
        2 => "E",
        4 => "S",
        6 => "O",
        _ => "N",
    };
    format!(
        "Layout {}/{} · {direction}",
        usize::from(index) + 1,
        layout_count
    )
}

/// Añade los Airports Action0 que aparecen al aplicar/cambiar NewGRF.
pub(crate) fn sync_airport_catalog_entries(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    lists: Query<Entity, With<AirportPickerSpecList>>,
    existing: Query<&AirportPickerButton>,
) {
    let existing_ids: std::collections::HashSet<u16> = existing
        .iter()
        .filter_map(|button| match *button {
            AirportPickerButton::NewgrfSpec(id) => Some(id),
            _ => None,
        })
        .collect();
    let Ok(list) = lists.single() else {
        return;
    };
    for def in sim
        .state
        .airport_spec_catalog
        .iter()
        .filter(|def| def.enabled)
    {
        if existing_ids.contains(&def.id) {
            continue;
        }
        let id = def.id;
        let label = format!("{} · NewGRF", def.label);
        commands.entity(list).with_children(|col| {
            spawn_text_button(
                col,
                &asset_server,
                AirportPickerButton::NewgrfSpec(id),
                &label,
                280.0,
            );
        });
    }
}

fn localized_airport_title(locale: Locale, label: &str) -> String {
    format!("{} · {label}", localized_text(locale, "Aeropuerto"))
}

fn localized_airport_size(locale: Locale, width: i32, height: i32) -> String {
    format!("{} {width}×{height}", localized_text(locale, "Tamaño:"))
}

fn localized_airport_coverage_hidden(locale: Locale) -> String {
    format!(
        "{} {}",
        localized_text(locale, "Cobertura:"),
        localized_text(locale, "oculta")
    )
}

fn localized_airport_coverage_at(
    locale: Locale,
    radius: i32,
    house_tiles: u32,
    supplied_stock: u32,
) -> String {
    format!(
        "{} r={radius}: {} {house_tiles} · {} {supplied_stock}",
        localized_text(locale, "Cobertura"),
        localized_text(locale, "casas"),
        localized_text(locale, "stock ind."),
    )
}

fn localized_airport_coverage_hint(locale: Locale, radius: i32) -> String {
    format!(
        "{} r={radius}: {}",
        localized_text(locale, "Cobertura"),
        localized_text(locale, "apunta al mapa")
    )
}

/// Actualiza la miniatura Action5 `0x16` según el aeropuerto seleccionado.
pub(crate) fn sync_airport_preview_image(
    station_state: Res<StationBuildState>,
    sim: Res<SimWorld>,
    mut cache: ResMut<NewGrfAction5SpriteCache>,
    mut images: ResMut<Assets<Image>>,
    mut preview: Query<(&mut ImageNode, &mut Node), With<AirportPickerPreviewImage>>,
) {
    let Ok((mut image, mut node)) = preview.single_mut() else {
        return;
    };
    if let Some(def) = active_newgrf_airport(&sim) {
        let Some(decoded) = def.newgrf_preview_sprite() else {
            node.display = Display::None;
            return;
        };
        image.image = cache.handle_for(
            AIRPORT_NEWGRF_PREVIEW_CACHE_TYPE,
            def.id,
            decoded,
            &mut images,
        );
        node.display = Display::Flex;
        return;
    }
    let Some(slot) = openttdrs_core::airport_preview_action5_slot(station_state.airport_spec)
    else {
        node.display = Display::None;
        return;
    };
    let Some(decoded) = sim
        .state
        .runtime
        .airport_preview_newgrf_sprites
        .get(slot)
        .and_then(|s| s.as_ref())
    else {
        node.display = Display::None;
        return;
    };
    let slot_u16 = u16::try_from(slot).unwrap_or(0);
    image.image = cache.handle_for(
        openttdrs_core::ACTION5_TYPE_AIRPORT_PREVIEW,
        slot_u16,
        decoded,
        &mut images,
    );
    node.display = Display::Flex;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_airport_picker(
    tool_state: Res<UiToolState>,
    mut station_state: ResMut<StationBuildState>,
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
    hovered: Option<Res<HoveredTileCoord>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (
            Without<AirportPickerSizeLabel>,
            Without<AirportPickerCoverageText>,
            Without<AirportPickerLayoutText>,
        ),
    >,
    mut size_q: Query<
        &mut Text,
        (
            With<AirportPickerSizeLabel>,
            Without<AirportPickerCoverageText>,
            Without<FloatingWindowTitleText>,
            Without<AirportPickerLayoutText>,
        ),
    >,
    mut coverage_q: Query<
        &mut Text,
        (
            With<AirportPickerCoverageText>,
            Without<AirportPickerSizeLabel>,
            Without<FloatingWindowTitleText>,
            Without<AirportPickerLayoutText>,
        ),
    >,
    mut layout_q: Query<
        &mut Text,
        (
            With<AirportPickerLayoutText>,
            Without<AirportPickerSizeLabel>,
            Without<AirportPickerCoverageText>,
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

    let newgrf = active_newgrf_airport(&sim);
    if let Some(def) = newgrf {
        station_state.airport_newgrf_spec_id = Some(def.id);
        if let Some(last) = def.layouts.len().checked_sub(1) {
            let last = u8::try_from(last).unwrap_or(u8::MAX);
            let index = station_state.airport_layout.unwrap_or(0).min(last);
            station_state.airport_layout = Some(index);
            station_state.airport_axis_y =
                airport_axis_for_rotation(def.layouts[usize::from(index)].rotation);
        } else {
            station_state.airport_layout = None;
        }
    } else {
        station_state.airport_layout = None;
        station_state.airport_newgrf_spec_id = None;
    }

    let class = sim.state.current_airport_class;
    let spec = station_state.airport_spec;
    let axis_y = station_state.airport_axis_y;
    let layout = selected_newgrf_layout(&sim, &station_state);
    let layout_count = newgrf.map_or(0, |def| def.layouts.len());
    let (w, h) = newgrf
        .and_then(|def| {
            newgrf_airport_footprint_with_layout(def, layout.map(|(index, _)| index), axis_y)
        })
        .unwrap_or_else(|| airport_spec_footprint(spec, axis_y));
    let label = newgrf
        .map(|def| def.label.as_str())
        .unwrap_or_else(|| airport_spec_def(spec).map_or("—", |def| def.label));
    let radius = newgrf.map_or_else(
        || {
            airport_spec_def(spec)
                .map(|def| def.catchment)
                .unwrap_or(STATION_COVERAGE_RADIUS)
        },
        |def| def.catchment,
    );
    let locale = prefs.locale();

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::AirportPicker)
    {
        **title = localized_airport_title(locale, label);
    }
    if let Ok(mut size) = size_q.single_mut() {
        **size = localized_airport_size(locale, w, h);
    }
    if let Ok(mut layout_text) = layout_q.single_mut() {
        **layout_text = airport_layout_label(layout, layout_count);
    }
    if let Ok(mut cov) = coverage_q.single_mut() {
        let text = if !station_state.airport_show_coverage {
            localized_airport_coverage_hidden(locale)
        } else if let Some(pos) = hovered.as_ref().and_then(|h| h.pos) {
            let coverage = station_coverage_at(&sim.state.map, &sim.state.industries, pos, radius);
            localized_airport_coverage_at(
                locale,
                radius,
                coverage.house_tiles,
                coverage.supplied_stock,
            )
        } else {
            localized_airport_coverage_hint(locale, radius)
        };
        **cov = text;
    }

    for (button, mut bg) in &mut buttons {
        let on = match *button {
            AirportPickerButton::Class(c) => c == class,
            AirportPickerButton::Spec(s) => {
                sim.state.current_airport_newgrf_id.is_none() && s == spec
            }
            AirportPickerButton::NewgrfSpec(id) => sim.state.current_airport_newgrf_id == Some(id),
            AirportPickerButton::AxisX => !axis_y,
            AirportPickerButton::AxisY => axis_y,
            AirportPickerButton::LayoutPrevious | AirportPickerButton::LayoutNext => false,
            AirportPickerButton::CoverageOff => !station_state.airport_show_coverage,
            AirportPickerButton::CoverageOn => station_state.airport_show_coverage,
        };
        let visible_spec = match *button {
            AirportPickerButton::Spec(s) => airport_spec_def(s).is_some_and(|d| d.class == class),
            AirportPickerButton::NewgrfSpec(id) => {
                newgrf_airport_spec_def(&sim.state.airport_spec_catalog, id)
                    .is_some_and(|def| def.class == class)
            }
            _ => true,
        };
        let layout_disabled = match *button {
            AirportPickerButton::LayoutPrevious => layout.is_none_or(|(index, _)| index == 0),
            AirportPickerButton::LayoutNext => {
                layout.is_none_or(|(index, _)| usize::from(index) + 1 >= layout_count)
            }
            _ => false,
        };
        *bg = BackgroundColor(if !visible_spec || layout_disabled {
            Color::srgb(0.22, 0.20, 0.16)
        } else if on {
            BTN_ACTIVE
        } else {
            BTN_BG
        });
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
                station_state.airport_layout = None;
                station_state.airport_newgrf_spec_id = None;
            }
            AirportPickerButton::Spec(spec) => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetCurrentAirportSpec(spec),
                );
                station_state.airport_spec = sim.state.current_airport_spec;
                station_state.airport_layout = None;
                station_state.airport_newgrf_spec_id = None;
            }
            AirportPickerButton::NewgrfSpec(id) => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetCurrentAirportNewgrfSpec(id),
                );
                station_state.airport_spec = sim.state.current_airport_spec;
                if let Some(def) = active_newgrf_airport(&sim)
                    && let Some(first) = def.layouts.first()
                {
                    station_state.airport_layout = Some(0);
                    station_state.airport_newgrf_spec_id = Some(def.id);
                    station_state.airport_axis_y = airport_axis_for_rotation(first.rotation);
                } else {
                    station_state.airport_layout = None;
                    station_state.airport_newgrf_spec_id = None;
                }
            }
            AirportPickerButton::AxisX | AirportPickerButton::AxisY => {
                let axis_y = matches!(*button, AirportPickerButton::AxisY);
                if let Some(def) = active_newgrf_airport(&sim)
                    && let Some((layout, rotation)) =
                        newgrf_airport_layout_selection_with_index(def, None, axis_y)
                {
                    station_state.airport_layout = Some(layout);
                    station_state.airport_newgrf_spec_id = Some(def.id);
                    station_state.airport_axis_y = airport_axis_for_rotation(rotation);
                } else {
                    station_state.airport_layout = None;
                    station_state.airport_newgrf_spec_id = None;
                    station_state.airport_axis_y = axis_y;
                }
            }
            AirportPickerButton::LayoutPrevious | AirportPickerButton::LayoutNext => {
                let Some(def) = active_newgrf_airport(&sim) else {
                    continue;
                };
                let Some((current, _)) = selected_newgrf_layout(&sim, &station_state) else {
                    continue;
                };
                let next = match *button {
                    AirportPickerButton::LayoutPrevious => current.checked_sub(1),
                    AirportPickerButton::LayoutNext => {
                        let next = current.saturating_add(1);
                        (usize::from(next) < def.layouts.len()).then_some(next)
                    }
                    _ => None,
                };
                if let Some(next) = next {
                    station_state.airport_layout = Some(next);
                    station_state.airport_newgrf_spec_id = Some(def.id);
                    station_state.airport_axis_y =
                        airport_axis_for_rotation(def.layouts[usize::from(next)].rotation);
                }
            }
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
        if msg.0.class == FloatingWindowId::AirportPicker && airport_tool_active(&tool_state) {
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
    use openttdrs_core::{AirportLayoutTile, AirportTileLayout, NewgrfAirportSpecDef};

    fn picker_newgrf_spec(layouts: Vec<AirportTileLayout>) -> NewgrfAirportSpecDef {
        NewgrfAirportSpecDef {
            id: 10,
            class: AirportClassId::Small,
            label: "Picker custom".into(),
            short_label: "Picker".into(),
            size_x: 4,
            size_y: 2,
            catchment: 5,
            noise_level: 1,
            subst_id: AirportSpecId::Small,
            ttd_airport_type: 0,
            layouts,
            enabled: true,
            min_year: 0,
            max_year: u16::MAX,
            maintenance_cost: 0,
            associated_badges: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 0,
            newgrf_views: Vec::new(),
            newgrf_purchase_views: Vec::new(),
        }
    }

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

    #[test]
    fn airport_picker_adds_enabled_newgrf_specs_to_the_catalog() {
        use bevy::asset::AssetPlugin;

        let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
            file_path: asset_root.into(),
            ..default()
        });
        app.init_asset::<Image>();
        app.init_asset::<Font>();

        let mut sim = SimWorld::default();
        sim.state
            .airport_spec_catalog
            .push(picker_newgrf_spec(vec![AirportTileLayout {
                rotation: 0,
                tiles: vec![AirportLayoutTile {
                    x: 0,
                    y: 0,
                    gfx: 24,
                }],
            }]));
        app.world_mut().insert_resource(sim);
        app.world_mut()
            .run_system_once(setup_airport_picker)
            .unwrap();
        app.world_mut()
            .run_system_once(sync_airport_catalog_entries)
            .unwrap();

        let world = app.world_mut();
        let mut buttons = world.query::<&AirportPickerButton>();
        assert!(
            buttons
                .iter(world)
                .any(|button| *button == AirportPickerButton::NewgrfSpec(10))
        );
    }

    #[test]
    fn newgrf_picker_selects_and_cycles_the_exact_action0_layout() {
        let layouts = vec![
            AirportTileLayout {
                rotation: 0,
                tiles: vec![AirportLayoutTile {
                    x: 0,
                    y: 0,
                    gfx: 24,
                }],
            },
            AirportTileLayout {
                rotation: 6,
                tiles: vec![AirportLayoutTile {
                    x: 0,
                    y: 0,
                    gfx: 24,
                }],
            },
        ];
        let mut select_world = World::new();
        select_world.insert_resource(StationBuildState::default());
        let mut sim = SimWorld::default();
        sim.state
            .airport_spec_catalog
            .push(picker_newgrf_spec(layouts.clone()));
        select_world.insert_resource(sim);
        select_world.spawn((
            Button,
            AirportPickerButton::NewgrfSpec(10),
            Interaction::Pressed,
        ));
        select_world
            .run_system_once(handle_airport_picker_buttons)
            .unwrap();
        assert_eq!(
            select_world
                .resource::<SimWorld>()
                .state
                .current_airport_newgrf_id,
            Some(10)
        );
        assert_eq!(
            select_world.resource::<StationBuildState>().airport_layout,
            Some(0)
        );
        assert_eq!(
            select_world
                .resource::<StationBuildState>()
                .airport_newgrf_spec_id,
            Some(10)
        );

        let mut cycle_world = World::new();
        cycle_world.insert_resource(StationBuildState {
            airport_layout: Some(0),
            airport_newgrf_spec_id: Some(10),
            ..Default::default()
        });
        let mut sim = SimWorld::default();
        sim.state
            .airport_spec_catalog
            .push(picker_newgrf_spec(layouts));
        sim.state.current_airport_newgrf_id = Some(10);
        cycle_world.insert_resource(sim);
        cycle_world.spawn((
            Button,
            AirportPickerButton::LayoutNext,
            Interaction::Pressed,
        ));
        cycle_world
            .run_system_once(handle_airport_picker_buttons)
            .unwrap();
        assert_eq!(
            cycle_world.resource::<StationBuildState>().airport_layout,
            Some(1)
        );
        assert!(cycle_world.resource::<StationBuildState>().airport_axis_y);
    }

    #[test]
    fn airport_picker_on_closed_clears_airport_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::Airport),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::AirportPicker),
        ));
        world.run_system_once(airport_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }

    #[test]
    fn airport_picker_chrome_localizes_without_touching_spec_labels_or_values() {
        assert_eq!(
            localized_airport_title(Locale::En, "Custom Aeródromo"),
            "Airport · Custom Aeródromo"
        );
        assert_eq!(localized_airport_size(Locale::En, 4, 3), "Size: 4×3");
        assert_eq!(
            localized_airport_coverage_hidden(Locale::En),
            "Coverage: hidden"
        );
        assert_eq!(
            localized_airport_coverage_at(Locale::En, 5, 12, 7),
            "Coverage r=5: houses 12 · industry stock 7"
        );
        assert_eq!(
            localized_airport_coverage_hint(Locale::En, 5),
            "Coverage r=5: point to the map"
        );
        assert_eq!(
            localized_airport_title(Locale::Es, "Custom Aeródromo"),
            "Aeropuerto · Custom Aeródromo"
        );
    }
}
