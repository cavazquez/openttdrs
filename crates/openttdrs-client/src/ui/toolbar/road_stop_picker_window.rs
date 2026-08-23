//! Ventana «Selección de parada» NewGRF (bus / camión).
//!
//! Más simple que el picker de estación rail: clase + tipo, sin cobertura ni ejes.

use bevy::prelude::*;
use openttdrs_core::{
    Command, StopKind, list_road_stop_classes, list_road_stop_specs, road_stop_class_def,
    road_stop_spec_def,
};

use crate::render::NewGrfAction5SpriteCache;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::scrollbar::spawn_classic_scroll_area_with;

use super::{BuildMenuAction, BuildMenuUi, UiToolState};

/// Type id sintético para miniaturas Action1/3 en el caché Action5.
const ROAD_STOP_PREVIEW_CACHE_TYPE: u8 = 0xFE;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoadStopPickerButton {
    Class(u16),
    Spec(u16),
}

#[derive(Component)]
pub(crate) struct RoadStopClassList;

#[derive(Component)]
pub(crate) struct RoadStopSpecList;

#[derive(Component)]
pub(crate) struct RoadStopPickerPreviewImage;

#[derive(Component)]
pub(crate) struct RoadStopPickerEmptyHint;

pub(crate) fn setup_road_stop_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::RoadStopPicker,
        "Selección de parada",
        TITLE_BROWN,
        Vec2::new(200.0, 48.0),
        300.0,
    );
    commands.entity(content).with_children(|panel| {
        spawn_section_label(panel, asset_server, "Clase");
        panel.spawn((
            RoadStopClassList,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
        ));
        spawn_section_label(panel, asset_server, "Vista previa");
        panel.spawn((
            RoadStopPickerPreviewImage,
            ImageNode::default(),
            Node {
                width: Val::Px(64.0),
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
                    RoadStopSpecList,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ));
            },
            180.0,
        );
        panel.spawn((
            RoadStopPickerEmptyHint,
            Text::new("Sin paradas NewGRF para este tipo"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.7, 0.65, 0.55)),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
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
    marker: RoadStopPickerButton,
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

fn road_stop_tool_kind(tool: &UiToolState) -> Option<StopKind> {
    match tool.active_tool {
        Some(BuildMenuAction::BusStop) => Some(StopKind::BusStop),
        Some(BuildMenuAction::Station) => Some(StopKind::TruckStop),
        _ => None,
    }
}

/// Añade botones de clase/spec que aún no existen (tras apply NewGRF RoadStops).
pub(crate) fn sync_road_stop_catalog_entries(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tool_state: Res<UiToolState>,
    sim: Res<SimWorld>,
    class_lists: Query<Entity, With<RoadStopClassList>>,
    spec_lists: Query<Entity, With<RoadStopSpecList>>,
    existing: Query<&RoadStopPickerButton>,
) {
    let Some(kind) = road_stop_tool_kind(&tool_state) else {
        return;
    };
    let existing_classes: std::collections::HashSet<u16> = existing
        .iter()
        .filter_map(|b| match *b {
            RoadStopPickerButton::Class(id) => Some(id),
            RoadStopPickerButton::Spec(_) => None,
        })
        .collect();
    let existing_specs: std::collections::HashSet<u16> = existing
        .iter()
        .filter_map(|b| match *b {
            RoadStopPickerButton::Spec(id) => Some(id),
            RoadStopPickerButton::Class(_) => None,
        })
        .collect();

    if let Ok(list) = class_lists.single() {
        for def in list_road_stop_classes(
            &sim.state.road_stop_class_catalog,
            &sim.state.road_stop_spec_catalog,
            kind,
        ) {
            if existing_classes.contains(&def.id) {
                continue;
            }
            let id = def.id;
            let label = def.label.clone();
            commands.entity(list).with_children(|row| {
                spawn_text_button(
                    row,
                    &asset_server,
                    RoadStopPickerButton::Class(id),
                    &label,
                    88.0,
                );
            });
        }
    }
    if let Ok(list) = spec_lists.single() {
        for def in list_road_stop_specs(&sim.state.road_stop_spec_catalog, None, kind) {
            if existing_specs.contains(&def.id) {
                continue;
            }
            let id = def.id;
            let label = def.label.clone();
            commands.entity(list).with_children(|col| {
                spawn_text_button(
                    col,
                    &asset_server,
                    RoadStopPickerButton::Spec(id),
                    &label,
                    260.0,
                );
            });
        }
    }
}

pub(crate) fn sync_road_stop_preview_image(
    tool_state: Res<UiToolState>,
    sim: Res<SimWorld>,
    mut cache: ResMut<NewGrfAction5SpriteCache>,
    mut images: ResMut<Assets<Image>>,
    mut preview: Query<(&mut ImageNode, &mut Node), With<RoadStopPickerPreviewImage>>,
) {
    let Ok((mut image, mut node)) = preview.single_mut() else {
        return;
    };
    if road_stop_tool_kind(&tool_state).is_none() {
        node.display = Display::None;
        return;
    }
    let Some(spec_id) = sim.state.current_road_stop_spec else {
        node.display = Display::None;
        return;
    };
    let Some(def) = road_stop_spec_def(&sim.state.road_stop_spec_catalog, spec_id) else {
        node.display = Display::None;
        return;
    };
    let Some(view) = def.newgrf_view(0) else {
        node.display = Display::None;
        return;
    };
    image.image = cache.handle_for(ROAD_STOP_PREVIEW_CACHE_TYPE, spec_id, view, &mut images);
    node.display = Display::Flex;
}

pub(crate) fn sync_road_stop_picker(
    tool_state: Res<UiToolState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility), Without<RoadStopPickerButton>>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<RoadStopPickerEmptyHint>>,
    mut hint_q: Query<&mut Node, With<RoadStopPickerEmptyHint>>,
    mut buttons: Query<
        (&RoadStopPickerButton, &mut BackgroundColor, &mut Visibility),
        (With<Button>, Without<FloatingWindow>),
    >,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::RoadStopPicker)
    else {
        return;
    };
    let Some(kind) = road_stop_tool_kind(&tool_state) else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Visible;

    let class = sim.state.current_road_stop_class;
    let spec = sim.state.current_road_stop_spec;
    let kind_label = match kind {
        StopKind::BusStop => "Bus",
        StopKind::TruckStop => "Camión",
        _ => "Parada",
    };
    let spec_label = spec
        .and_then(|id| road_stop_spec_def(&sim.state.road_stop_spec_catalog, id))
        .map_or("—", |d| d.label.as_str());

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::RoadStopPicker)
    {
        **title = format!("Parada {kind_label} · {spec_label}");
    }

    let matching = list_road_stop_specs(&sim.state.road_stop_spec_catalog, None, kind);
    if let Ok(mut hint) = hint_q.single_mut() {
        hint.display = if matching.is_empty() {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (button, mut bg, mut vis) in &mut buttons {
        let (visible, on) = match *button {
            RoadStopPickerButton::Class(c) => {
                let visible = list_road_stop_classes(
                    &sim.state.road_stop_class_catalog,
                    &sim.state.road_stop_spec_catalog,
                    kind,
                )
                .iter()
                .any(|d| d.id == c);
                (visible, class == Some(c))
            }
            RoadStopPickerButton::Spec(s) => {
                let visible = matching.iter().any(|d| d.id == s);
                let in_class = class.is_none_or(|c| {
                    road_stop_spec_def(&sim.state.road_stop_spec_catalog, s)
                        .is_some_and(|d| d.class == c)
                });
                (visible, spec == Some(s) && in_class)
            }
        };
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        *bg = BackgroundColor(if on { BTN_ACTIVE } else { BTN_BG });
        let _ = road_stop_class_def(&sim.state.road_stop_class_catalog, class.unwrap_or(0));
    }
}

pub(crate) fn handle_road_stop_picker_buttons(
    buttons: Query<(&Interaction, &RoadStopPickerButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            RoadStopPickerButton::Class(class) => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetCurrentRoadStopClass(class),
                );
            }
            RoadStopPickerButton::Spec(spec) => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetCurrentRoadStopSpec(spec),
                );
            }
        }
    }
}

pub(crate) fn road_stop_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::RoadStopPicker
            && road_stop_tool_kind(&tool_state).is_some()
        {
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
    use openttdrs_core::{RoadStopClassDef, RoadStopSpecDef};

    #[test]
    fn picking_spec_updates_state() {
        let mut world = World::new();
        let mut sim = SimWorld::default();
        sim.state.road_stop_class_catalog.push(RoadStopClassDef {
            id: 0,
            label: "Cls".into(),
            short_label: "CLS".into(),
            from_newgrf: true,
        });
        sim.state.road_stop_spec_catalog.push(RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "Bus".into(),
            short_label: "B".into(),
            stop_type: 0,
            from_newgrf: true,
            grfid: 0,
            newgrf_local_id: 0,
            draw_mode: openttdrs_core::ROADSTOP_DRAW_MODE_DEFAULT,
            flags: 0,
            callback_mask: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        });
        world.insert_resource(sim);
        world.spawn((Button, RoadStopPickerButton::Spec(7), Interaction::Pressed));
        world
            .run_system_once(handle_road_stop_picker_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<SimWorld>().state.current_road_stop_spec,
            Some(7)
        );
    }

    #[test]
    fn road_stop_picker_on_closed_clears_stop_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::BusStop),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::RoadStopPicker),
        ));
        world.run_system_once(road_stop_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }
}
