//! Ventana «Selección de puente» (estilo `BuildBridgeWindow` de OpenTTD).
//!
//! Se abre al soltar el arrastre de un tramo válido y lista los 13 tipos
//! vanilla con coste estimado y velocidad máxima.

use bevy::prelude::*;
use openttdrs_core::{
    BridgeType, Command, apply_command, bridge_available_at_tick, bridge_build_cost,
    bridge_middle_length, bridge_spec, command_would_fail,
};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error, push_build_command_success};
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BG_HOVER: Color = Color::srgb(0.44, 0.38, 0.26);
const BTN_BG_DISABLED: Color = Color::srgb(0.28, 0.24, 0.18);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

/// Tramo pendiente de confirmación (tras arrastrar con herramienta puente).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingBridge {
    pub start: openttdrs_core::TileCoord,
    pub end: openttdrs_core::TileCoord,
    pub road: bool,
}

#[derive(Resource, Default)]
pub(crate) struct BridgeBuildState {
    pub pending: Option<PendingBridge>,
    pub last_road_type: BridgeType,
    pub last_rail_type: BridgeType,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BridgePickerButton(pub BridgeType);

#[derive(Component)]
pub(crate) struct BridgePickerHintText;

pub(crate) fn setup_bridge_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::BridgePicker,
        "Selección de puente",
        TITLE_BROWN,
        Vec2::new(280.0, 64.0),
        320.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            BridgePickerHintText,
            Text::new("Arrastra un tramo válido sobre agua o desnivel."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|list| {
                for spec in openttdrs_core::BRIDGE_SPECS {
                    spawn_bridge_row(list, asset_server, spec.bridge_type);
                }
            });
    });
}

fn spawn_bridge_row(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer, bt: BridgeType) {
    let spec = bridge_spec(bt);
    parent
        .spawn((
            BridgePickerButton(bt),
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(spec.name),
                window_text_font(asset_server, UiFontRole::Body),
                TextColor(WINDOW_TEXT),
            ));
            row.spawn((
                Text::new(format!("{} km/h", spec.max_speed)),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.85, 0.82, 0.65)),
            ));
        });
}

pub(crate) fn sync_bridge_picker(
    bridge_state: Res<BridgeBuildState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut buttons_q: Query<(&BridgePickerButton, &Interaction, &mut BackgroundColor), With<Button>>,
    sim: Res<SimWorld>,
    mut hint_q: Query<&mut Text, With<BridgePickerHintText>>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::BridgePicker)
    else {
        return;
    };
    let Some(pending) = bridge_state.pending else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;

    if let Ok(mut hint) = hint_q.single_mut() {
        let middle = bridge_middle_length(pending.start, pending.end);
        let transport = if pending.road { "carretera" } else { "vía" };
        **hint = format!("Puente de {transport}: vano {middle} teselas (sin rampas)");
    }

    for (button, interaction, mut bg) in &mut buttons_q {
        let available =
            bridge_available_at_tick(button.0, sim.state.tick, pending.start, pending.end);
        let affordable =
            sim.state.economy.money >= bridge_build_cost(button.0, pending.start, pending.end);
        let enabled = available && affordable;
        *bg = if !enabled {
            BackgroundColor(BTN_BG_DISABLED)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_BG_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
}

pub(crate) fn handle_bridge_picker_buttons(
    mut bridge_state: ResMut<BridgeBuildState>,
    buttons_q: Query<(&Interaction, &BridgePickerButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
    mut pending_remap: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    let Some(pending) = bridge_state.pending else {
        return;
    };
    for (interaction, button) in &buttons_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if !bridge_available_at_tick(button.0, sim.state.tick, pending.start, pending.end) {
            continue;
        }
        let cmd = if pending.road {
            Command::PlaceRoadBridge(pending.start, pending.end, button.0)
        } else {
            Command::PlaceRailBridge(pending.start, pending.end, button.0)
        };
        if let Some(err) = command_would_fail(&sim.state, &cmd) {
            push_build_command_error(&mut hud_feedback, err, time.elapsed_secs());
            continue;
        }
        match apply_command(&mut sim.state, &cmd) {
            Ok(()) => {
                if pending.road {
                    bridge_state.last_road_type = button.0;
                } else {
                    bridge_state.last_rail_type = button.0;
                }
                let (mw, mh) = sim.state.map.dimensions();
                let tiles: Vec<(i32, i32)> =
                    openttdrs_core::bridge_line_tiles(pending.start, pending.end)
                        .into_iter()
                        .map(|c| (c.x, c.y))
                        .collect();
                crate::render::request_map_visual_remap(&mut pending_remap, mw, mh, &tiles);
                push_build_command_success(&mut hud_feedback);
                bridge_state.pending = None;
            }
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
}

pub(crate) fn bridge_picker_on_closed(
    mut reader: MessageReader<FloatingWindowClosed>,
    mut bridge_state: ResMut<BridgeBuildState>,
) {
    for msg in reader.read() {
        if msg.0 == FloatingWindowId::BridgePicker {
            bridge_state.pending = None;
        }
    }
}
