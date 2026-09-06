//! Ventana «Selección de objeto» (vanilla faro/transmisor + NewGRF W×H).

use bevy::prelude::*;
use openttdrs_core::{
    Command, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_TRANSMITTER, ObjectFundMoreText,
    list_buildable_object_specs, object_spec_def, resolve_object_fund_more_text_callback,
};

use crate::i18n::Locale;
use crate::render::NewGrfObjectSpriteCache;
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::scrollbar::spawn_classic_scroll_area_with;

use super::{BuildMenuAction, BuildMenuUi, UiToolState};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectPickerButton(pub u16);

#[derive(Component)]
pub(crate) struct ObjectSpecList;

#[derive(Component)]
pub(crate) struct ObjectPickerPreviewImage;

#[derive(Component)]
pub(crate) struct ObjectPickerLabel;

#[derive(Component)]
pub(crate) struct ObjectPickerFundMoreText;

pub(crate) fn setup_object_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::ObjectPicker,
        "Selección de objeto",
        TITLE_BROWN,
        Vec2::new(220.0, 80.0),
        300.0,
    );
    commands.entity(content).with_children(|panel| {
        spawn_section_label(panel, asset_server, "Vista previa");
        panel.spawn((
            ObjectPickerPreviewImage,
            ImageNode::default(),
            Node {
                width: Val::Px(64.0),
                height: Val::Px(64.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
        ));
        spawn_section_label(panel, asset_server, "Objeto");
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
                    ObjectSpecList,
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ))
                .with_children(|list| {
                    spawn_text_button(
                        list,
                        asset_server,
                        ObjectPickerButton(u16::from(OBJECT_TYPE_TRANSMITTER)),
                        "Transmisor",
                        260.0,
                    );
                    spawn_text_button(
                        list,
                        asset_server,
                        ObjectPickerButton(u16::from(OBJECT_TYPE_LIGHTHOUSE)),
                        "Faro",
                        260.0,
                    );
                });
            },
            200.0,
        );
        panel.spawn((
            ObjectPickerLabel,
            Text::new("Seleccionado: —"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ));
        panel.spawn((
            ObjectPickerFundMoreText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.85, 0.65, 0.30)),
            Node {
                margin: UiRect::top(Val::Px(3.0)),
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
    marker: ObjectPickerButton,
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

fn object_tool_active(tool: &UiToolState) -> bool {
    tool.active_tool == Some(BuildMenuAction::PlaceNewGrfObject)
}

fn object_label(sim: &SimWorld, id: u16) -> String {
    match id {
        0 => "Transmisor".into(),
        1 => "Faro".into(),
        other => object_spec_def(&sim.state.object_spec_catalog, other)
            .map(|d| format!("{} ({}×{})", d.name, d.size_width(), d.size_height()))
            .unwrap_or_else(|| format!("Objeto {other}")),
    }
}

fn object_fund_more_text_label(sim: &SimWorld, id: u16, locale: Locale) -> String {
    let Some(def) = object_spec_def(&sim.state.object_spec_catalog, id) else {
        return String::new();
    };
    match resolve_object_fund_more_text_callback(def, 0) {
        ObjectFundMoreText::None => String::new(),
        ObjectFundMoreText::Local(offset) => {
            let string_id = openttdrs_core::GRF_STRING_GENERIC_BASE + u32::from(offset);
            sim.state
                .runtime
                .newgrf_string_catalog
                .lookup(def.grfid, string_id, newgrf_language(locale))
                .map_or_else(
                    || format!("Texto NewGRF local #{offset} (Action4 ausente)"),
                    |text| format!("Texto NewGRF: {text}"),
                )
        }
        ObjectFundMoreText::GrfString(string_id) => sim
            .state
            .runtime
            .newgrf_string_catalog
            .lookup(def.grfid, string_id, newgrf_language(locale))
            .map_or_else(
                || format!("Texto NewGRF StringID {string_id:#06X} (Action4 ausente)"),
                |text| format!("Texto NewGRF: {text}"),
            ),
        ObjectFundMoreText::Invalid(result) => {
            format!("Callback CB15C inválido: {result:#06X}")
        }
    }
}

fn newgrf_language(locale: Locale) -> u8 {
    match locale {
        Locale::Es => openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
        Locale::En => openttdrs_core::NEWGRF_LANGUAGE_ENGLISH,
    }
}

/// Añade specs NewGRF (cualquier W×H) que aún no tienen botón.
pub(crate) fn sync_object_catalog_entries(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    sim: Res<SimWorld>,
    lists: Query<Entity, With<ObjectSpecList>>,
    existing: Query<&ObjectPickerButton>,
) {
    let existing_ids: std::collections::HashSet<u16> = existing.iter().map(|b| b.0).collect();
    let Ok(list) = lists.single() else {
        return;
    };
    for def in list_buildable_object_specs(&sim.state.object_spec_catalog) {
        if existing_ids.contains(&def.id) {
            continue;
        }
        let id = def.id;
        let label = format!("{} ({}×{})", def.name, def.size_width(), def.size_height());
        commands.entity(list).with_children(|col| {
            spawn_text_button(col, &asset_server, ObjectPickerButton(id), &label, 260.0);
        });
    }
}

pub(crate) fn sync_object_preview_image(
    tool_state: Res<UiToolState>,
    sim: Res<SimWorld>,
    mut cache: ResMut<NewGrfObjectSpriteCache>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut preview: Query<(&mut ImageNode, &mut Node), With<ObjectPickerPreviewImage>>,
) {
    let Ok((mut image, mut node)) = preview.single_mut() else {
        return;
    };
    if !object_tool_active(&tool_state) {
        node.display = Display::None;
        return;
    }
    let id = sim.state.current_object_spec;
    match id {
        0 => {
            image.image = asset_server.load("assets/opengfx/tiles/object_transmitter.png");
            node.display = Display::Flex;
        }
        1 => {
            image.image = asset_server.load("assets/opengfx/tiles/object_lighthouse.png");
            node.display = Display::Flex;
        }
        other => {
            let Some(def) = object_spec_def(&sim.state.object_spec_catalog, other) else {
                node.display = Display::None;
                return;
            };
            let Some(view) = def.view(0) else {
                node.display = Display::None;
                return;
            };
            image.image = cache.handle_for(def, 0, view, &mut images);
            node.display = Display::Flex;
        }
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_object_picker(
    tool_state: Res<UiToolState>,
    sim: Res<SimWorld>,
    prefs: Option<Res<ClientPreferences>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), (Without<ObjectPickerLabel>,)>,
    mut label_q: Query<
        &mut Text,
        (
            With<ObjectPickerLabel>,
            Without<FloatingWindowTitleText>,
            Without<ObjectPickerFundMoreText>,
        ),
    >,
    mut more_text_q: Query<
        &mut Text,
        (
            With<ObjectPickerFundMoreText>,
            Without<ObjectPickerLabel>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut buttons: Query<(&ObjectPickerButton, &mut BackgroundColor), With<Button>>,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::ObjectPicker)
    else {
        return;
    };
    let open = object_tool_active(&tool_state);
    *visibility = if open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !open {
        if let Ok(mut text) = more_text_q.single_mut() {
            **text = String::new();
        }
        return;
    }

    let current = sim.state.current_object_spec;
    let label = object_label(&sim, current);
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::ObjectPicker)
    {
        **title = format!("Objeto · {label}");
    }
    if let Ok(mut text) = label_q.single_mut() {
        **text = format!("Seleccionado: {label}");
    }
    if let Ok(mut text) = more_text_q.single_mut() {
        let locale = prefs
            .as_deref()
            .map_or(Locale::Es, ClientPreferences::locale);
        **text = object_fund_more_text_label(&sim, current, locale);
    }
    for (button, mut bg) in &mut buttons {
        *bg = BackgroundColor(if button.0 == current {
            BTN_ACTIVE
        } else {
            BTN_BG
        });
    }
}

pub(crate) fn handle_object_picker_buttons(
    buttons: Query<(&Interaction, &ObjectPickerButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let _ = crate::network::apply_player_command(
            &mut sim.state,
            &Command::SetCurrentObjectSpec(button.0),
        );
    }
}

pub(crate) fn object_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::ObjectPicker && object_tool_active(&tool_state) {
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
    fn picking_vanilla_updates_current_object_spec() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.spawn((Button, ObjectPickerButton(1), Interaction::Pressed));
        world.run_system_once(handle_object_picker_buttons).unwrap();
        assert_eq!(world.resource::<SimWorld>().state.current_object_spec, 1);
    }

    #[test]
    fn object_picker_on_closed_clears_object_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::PlaceNewGrfObject),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::ObjectPicker),
        ));
        world.run_system_once(object_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }
}
