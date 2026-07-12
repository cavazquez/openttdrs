//! Selectores filtrables de roadtype / tramtype (`GetRoadTypeDropDownList`).
//!
//! Catálogo dinámico: vanilla + Action0 RoadTypes (metadatos; sin sprites).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::RelativeCursorPosition;
use openttdrs_core::{RoadTramType, RoadType, list_road_types, road_type_def};

use crate::state::SimWorld;
use crate::ui::font::{UiFontRole, ui_text_font_loaded};
use crate::ui::toolbar::{BuildMenuUi, ToolbarTooltipTarget};

const BTN_BG: Color = Color::srgb(0.36, 0.47, 0.26);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);
const BTN_BORDER: Color = Color::srgb(0.55, 0.68, 0.4);
const BTN_TEXT: Color = Color::srgb(0.95, 0.96, 0.82);
const MENU_BG: Color = Color::srgb(0.22, 0.28, 0.18);
const ENTRY_BG: Color = Color::srgb(0.30, 0.38, 0.22);

/// Estado del popover filtrable (road o tram).
#[derive(Resource, Default)]
pub(crate) struct RoadTypePickerState {
    pub(crate) open: Option<RoadTramType>,
    pub(crate) filter: String,
}

/// Esc ya consumido por el popover roadtype (evita cascada al menú).
#[derive(Resource, Default)]
pub(crate) struct RoadTypeEscapeConsumed(pub bool);

/// Botón que abre el dropdown de una clase.
#[derive(Component, Clone, Copy)]
pub(crate) struct RoadTypeClassButton(pub RoadTramType);

/// Etiqueta del tipo actual en el botón de clase.
#[derive(Component, Clone, Copy)]
pub(crate) struct RoadTypeClassLabel(pub RoadTramType);

/// Raíz del popover de una clase.
#[derive(Component, Clone, Copy)]
pub(crate) struct RoadTypePopover(pub RoadTramType);

/// Campo de filtro de texto.
#[derive(Component, Clone, Copy)]
pub(crate) struct RoadTypeFilterInput(pub RoadTramType);

/// Entrada del listado filtrable.
#[derive(Component, Clone, Copy)]
pub(crate) struct RoadTypeSelectButton {
    pub class: RoadTramType,
    pub id: RoadType,
}

pub(crate) fn spawn_road_type_selectors(
    buttons: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    for class in [RoadTramType::Road, RoadTramType::Tram] {
        spawn_class_dropdown(buttons, asset_server, class);
    }
}

fn spawn_class_dropdown(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    class: RoadTramType,
) {
    let tip = match class {
        RoadTramType::Road => "Tipo de carretera (vanilla + NewGRF Action0 metadatos)",
        RoadTramType::Tram => "Tipo de tranvía (vanilla + NewGRF Action0 metadatos)",
    };
    let catalog = openttdrs_core::vanilla_road_type_catalog();
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
                RoadTypeClassButton(class),
                ToolbarTooltipTarget { text: tip },
                BuildMenuUi,
                Node {
                    min_width: Val::Px(56.0),
                    height: Val::Px(48.0),
                    padding: UiRect::horizontal(Val::Px(4.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(BTN_BG),
                BorderColor::all(BTN_BORDER),
                Interaction::default(),
                children![(
                    RoadTypeClassLabel(class),
                    Text::new(default_short(class)),
                    ui_text_font_loaded(asset_server, UiFontRole::Caption),
                    TextColor(BTN_TEXT),
                )],
            ));
            col.spawn((
                RoadTypePopover(class),
                RelativeCursorPosition::default(),
                BuildMenuUi,
                Node {
                    width: Val::Px(180.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    border: UiRect::all(Val::Px(1.0)),
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    top: Val::Px(50.0),
                    left: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(MENU_BG),
                BorderColor::all(BTN_BORDER),
                GlobalZIndex(2200),
            ))
            .with_children(|menu| {
                menu.spawn((
                    RoadTypeFilterInput(class),
                    EditableText::new(""),
                    Text::new("filtrar…"),
                    ui_text_font_loaded(asset_server, UiFontRole::Caption),
                    TextColor(Color::srgb(0.75, 0.78, 0.65)),
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(22.0),
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(ENTRY_BG),
                    BorderColor::all(Color::srgb(0.45, 0.55, 0.35)),
                ));
                for def in list_road_types(&catalog, class, "", 10_000) {
                    menu.spawn((
                        Button,
                        RoadTypeSelectButton { class, id: def.id },
                        BuildMenuUi,
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(26.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(ENTRY_BG),
                        BorderColor::all(Color::srgb(0.45, 0.55, 0.35)),
                        Interaction::default(),
                        children![(
                            Text::new(def.label.clone()),
                            ui_text_font_loaded(asset_server, UiFontRole::Caption),
                            TextColor(BTN_TEXT),
                        )],
                    ));
                }
            });
        });
}

const fn default_short(class: RoadTramType) -> &'static str {
    match class {
        RoadTramType::Road => "C:Norm",
        RoadTramType::Tram => "T:Tram",
    }
}

pub(crate) fn close_road_type_picker_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut picker: ResMut<RoadTypePickerState>,
    mut consumed: ResMut<RoadTypeEscapeConsumed>,
) {
    consumed.0 = false;
    if keyboard.just_pressed(KeyCode::Escape) && picker.open.take().is_some() {
        picker.filter.clear();
        consumed.0 = true;
    }
}

pub(crate) fn handle_road_type_class_buttons(
    buttons: Query<(&Interaction, &RoadTypeClassButton), (Changed<Interaction>, With<Button>)>,
    mut picker: ResMut<RoadTypePickerState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        picker.open = if picker.open == Some(button.0) {
            None
        } else {
            Some(button.0)
        };
        if picker.open.is_none() {
            picker.filter.clear();
        }
    }
}

pub(crate) fn handle_road_type_select_buttons(
    buttons: Query<(&Interaction, &RoadTypeSelectButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
    mut picker: ResMut<RoadTypePickerState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button.class {
            RoadTramType::Road => sim.state.current_road_type = button.id,
            RoadTramType::Tram => sim.state.current_tram_type = button.id,
        }
        picker.open = None;
        picker.filter.clear();
    }
}

pub(crate) fn road_type_filter_keyboard(
    mut key_events: MessageReader<KeyboardInput>,
    mut picker: ResMut<RoadTypePickerState>,
    mut inputs: Query<(&RoadTypeFilterInput, &mut EditableText, &mut Text)>,
) {
    let Some(open) = picker.open else {
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
    picker.filter = editable.value().to_string();
    if picker.filter.is_empty() {
        **text = "filtrar…".into();
    } else {
        **text = picker.filter.clone();
    }
}

pub(crate) fn sync_road_type_popovers(
    picker: Res<RoadTypePickerState>,
    mut popovers: Query<(&RoadTypePopover, &mut Node)>,
) {
    for (popover, mut node) in &mut popovers {
        node.display = if picker.open == Some(popover.0) {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn sync_road_type_entry_visibility(
    sim: Res<SimWorld>,
    picker: Res<RoadTypePickerState>,
    mut entries: Query<(&RoadTypeSelectButton, &mut Visibility)>,
) {
    let Some(open) = picker.open else {
        for (_, mut vis) in &mut entries {
            *vis = Visibility::Inherited;
        }
        return;
    };
    let matched: Vec<RoadType> =
        list_road_types(&sim.state.road_type_catalog, open, &picker.filter, 10_000)
            .into_iter()
            .map(|d| d.id)
            .collect();
    for (entry, mut vis) in &mut entries {
        if entry.class != open {
            continue;
        }
        *vis = if matched.contains(&entry.id) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Añade entradas del catálogo que aún no tienen botón (tras apply NewGRF).
pub(crate) fn sync_road_type_catalog_entries(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    popovers: Query<(Entity, &RoadTypePopover)>,
    entries: Query<&RoadTypeSelectButton>,
) {
    let existing: std::collections::HashSet<(RoadTramType, u8)> =
        entries.iter().map(|e| (e.class, e.id.as_u8())).collect();
    for (popover_entity, popover) in &popovers {
        for def in list_road_types(&sim.state.road_type_catalog, popover.0, "", 10_000) {
            if existing.contains(&(def.class, def.id.as_u8())) {
                continue;
            }
            let class = def.class;
            let id = def.id;
            let label = def.label.clone();
            commands.entity(popover_entity).with_children(|menu| {
                menu.spawn((
                    Button,
                    RoadTypeSelectButton { class, id },
                    BuildMenuUi,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(26.0),
                        padding: UiRect::horizontal(Val::Px(6.0)),
                        justify_content: JustifyContent::FlexStart,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(ENTRY_BG),
                    BorderColor::all(Color::srgb(0.45, 0.55, 0.35)),
                    Interaction::default(),
                    children![(
                        Text::new(label),
                        ui_text_font_loaded(&asset_server, UiFontRole::Caption),
                        TextColor(BTN_TEXT),
                    )],
                ));
            });
        }
    }
}

pub(crate) fn sync_road_type_class_labels(
    sim: Res<SimWorld>,
    mut labels: Query<(&RoadTypeClassLabel, &mut Text)>,
    mut buttons: Query<(&RoadTypeClassButton, &mut BackgroundColor), With<Button>>,
    picker: Res<RoadTypePickerState>,
) {
    for (label, mut text) in &mut labels {
        let id = match label.0 {
            RoadTramType::Road => sim.state.current_road_type,
            RoadTramType::Tram => sim.state.current_tram_type,
        };
        let short = road_type_def(&sim.state.road_type_catalog, id)
            .map(|d| d.short_label.as_str())
            .unwrap_or(id.short_label());
        let prefix = match label.0 {
            RoadTramType::Road => "C",
            RoadTramType::Tram => "T",
        };
        **text = format!("{prefix}:{short}");
    }
    for (button, mut bg) in &mut buttons {
        *bg = BackgroundColor(if picker.open == Some(button.0) {
            BTN_ACTIVE
        } else {
            BTN_BG
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn selecting_road_type_updates_game_state() {
        let mut world = World::new();
        let mut sim = SimWorld::default();
        sim.state.current_road_type = RoadType::Road;
        world.insert_resource(sim);
        world.insert_resource(RoadTypePickerState {
            open: Some(RoadTramType::Road),
            filter: String::new(),
        });
        world.spawn((
            Button,
            RoadTypeSelectButton {
                class: RoadTramType::Road,
                id: RoadType::Road,
            },
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_road_type_select_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<SimWorld>().state.current_road_type,
            RoadType::Road
        );
        assert!(world.resource::<RoadTypePickerState>().open.is_none());
    }
}
