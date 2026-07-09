//! Ventana flotante de tren/vehículo estilo `OpenTTD`.
//!
//! Se abre al hacer clic en un vehículo del mapa: vista previa en vivo
//! (cámara a render-target sobre el vehículo), modelo, velocidad actual y
//! máxima, carga, estado («Detenido» en rojo / «En marcha» en verde) y
//! acciones Iniciar/Detener, Órdenes, Enviar al depósito y Centrar vista.
//! La venta es una acción del depósito, no de esta ventana.

use bevy::camera::RenderTarget;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::text::EditableText;
use bevy::ui::widget::ImageNode;
use openttdrs_core::{
    Command, VehicleKind, apply_command, cargo_display_name, next_refit_cargo, refit_allowed,
    station::resolve_order_destination, vehicle::MAX_VEHICLE_NAME_CHARS,
};

use crate::camera::tile_camera_world_pos;

use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, vehicle_world_position,
};
use crate::state::{OrderPickState, SimWorld};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::{BuildMenuUi, OrderEditState, open_order_edit_for_vehicle};

const PREVIEW_TEX_W: u32 = 280;
const PREVIEW_TEX_H: u32 = 120;
const PREVIEW_SCALE: f32 = 0.5;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const STATUS_STOPPED: Color = Color::srgb(0.92, 0.35, 0.3);
const STATUS_RUNNING: Color = Color::srgb(0.45, 0.85, 0.4);
const STATUS_NO_ROUTE: Color = Color::srgb(0.95, 0.75, 0.25);

#[derive(Resource, Default)]
pub(crate) struct VehicleWindowState {
    pub(crate) vehicle_id: Option<u32>,
    /// Campo de renombrado visible y activo.
    pub(crate) rename_editing: bool,
}

#[derive(Component)]
pub(crate) struct VehicleWindowRenameRow;

#[derive(Component)]
pub(crate) struct VehicleWindowRenameInput;

#[derive(Component, Clone, Copy)]
pub(crate) enum VehicleWindowRenameButton {
    Apply,
    Cancel,
}

/// Cámara dedicada de la vista previa de esta ventana (independiente de la
/// del panel de órdenes).
#[derive(Component)]
pub(crate) struct VehicleWindowPreviewCamera;

#[derive(Component)]
pub(crate) struct VehicleWindowBodyText;

#[derive(Component)]
pub(crate) struct VehicleWindowStatusText;

#[derive(Component, Clone, Copy)]
pub(crate) enum VehicleWindowButton {
    ToggleRunning,
    Orders,
    GotoDepot,
    CenterOrder,
    CenterCamera,
    Rename,
    /// Solo trenes: invierte el sentido de marcha.
    TurnAround,
    /// Solo trenes: forzar paso en señal roja.
    ForceProceed,
    /// Cambia el tipo de carga (solo en depósito, vacío).
    Refit,
}

/// Botones visibles solo para trenes.
#[derive(Component)]
pub(crate) struct VehicleWindowTrainOnly;

/// Botón refit (solo en depósito, sin carga, tipos alternativos).
#[derive(Component)]
pub(crate) struct VehicleWindowRefitOnly;

/// Texto del botón Iniciar/Detener (cambia según el estado del vehículo).
#[derive(Component)]
pub(crate) struct VehicleWindowToggleText;

pub(crate) fn setup_vehicle_window(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let asset_server = &*asset_server;
    let image = Image::new_target_texture(
        PREVIEW_TEX_W,
        PREVIEW_TEX_H,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    let rt_handle = images.add(image);

    commands.spawn((
        Camera2d,
        MapPreviewCamera,
        VehicleWindowPreviewCamera,
        Camera {
            order: -3,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        RenderTarget::from(rt_handle.clone()),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: PREVIEW_SCALE,
            ..OrthographicProjection::default_2d()
        }),
    ));

    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Vehicle,
        "Vehículo",
        TITLE_CRIMSON,
        Vec2::new(720.0, 148.0),
        300.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            ImageNode::new(rt_handle),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(PREVIEW_TEX_H as f32),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.13, 0.10, 0.07)),
            BuildMenuUi,
        ));
        panel.spawn((
            VehicleWindowBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel.spawn((
            VehicleWindowStatusText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(STATUS_STOPPED),
        ));
        panel
            .spawn((
                VehicleWindowRenameRow,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    display: Display::None,
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                row.spawn((
                    VehicleWindowRenameInput,
                    EditableText::new(""),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(22.0),
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(BTN_BORDER),
                ));
                spawn_rename_action(
                    row,
                    asset_server,
                    VehicleWindowRenameButton::Apply,
                    "Guardar",
                );
                spawn_rename_action(
                    row,
                    asset_server,
                    VehicleWindowRenameButton::Cancel,
                    "Cancelar",
                );
            });
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::ToggleRunning,
                    "Iniciar",
                    true,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::Orders,
                    "Órdenes",
                    false,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::GotoDepot,
                    "Depósito",
                    false,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::CenterOrder,
                    "Ir a orden",
                    false,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::CenterCamera,
                    "Centrar",
                    false,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::Rename,
                    "Renombrar",
                    false,
                );
            });
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                VehicleWindowTrainOnly,
                BuildMenuUi,
            ))
            .with_children(|row| {
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::TurnAround,
                    "Dar la vuelta",
                    false,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::ForceProceed,
                    "Forzar paso",
                    false,
                );
            });
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    display: Display::None,
                    ..default()
                },
                VehicleWindowRefitOnly,
                BuildMenuUi,
            ))
            .with_children(|row| {
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::Refit,
                    "Refit carga",
                    false,
                );
            });
    });
}

fn spawn_rename_action(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: VehicleWindowRenameButton,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: Val::Px(66.0),
                height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            ));
        });
}

fn spawn_vehicle_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: VehicleWindowButton,
    label: &'static str,
    is_toggle: bool,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                width: Val::Px(66.0),
                height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            let mut text = btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            ));
            if is_toggle {
                text.insert(VehicleWindowToggleText);
            }
        });
}

fn speed_to_kmh(kind: VehicleKind, units: u16) -> u16 {
    match kind {
        VehicleKind::Train | VehicleKind::Aircraft => units,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Ship => units / 2,
    }
}

fn cargo_type_label(vehicle: &openttdrs_core::Vehicle) -> String {
    vehicle.cargo_type.map_or_else(
        || "Cualquiera".to_string(),
        |c| cargo_display_name(c).to_string(),
    )
}

fn vehicle_details_body(vehicle: &openttdrs_core::Vehicle, sim: &SimWorld) -> String {
    let engine = vehicle.effective_engine();
    let depot_note = if openttdrs_core::vehicle_in_depot(&sim.state.map, vehicle.pos) {
        " · En depósito"
    } else {
        ""
    };
    let active_order = if vehicle.orders.is_empty() {
        "—".to_string()
    } else {
        format!(
            "{}",
            vehicle
                .current_order
                .min(vehicle.orders.len().saturating_sub(1))
                + 1
        )
    };
    format!(
        "Modelo: {}\nTipo carga: {}\nPosición: ({}, {}){depot_note}\n\
         Velocidad: {} km/h (máx. {}) · Carga: {}/{} ({} pkt, {}d)\n\
         Coste: ${}/año · Fiabilidad: {}%\n\
         Órdenes: {} · Orden activa: {active_order}",
        engine.name,
        cargo_type_label(vehicle),
        vehicle.pos.x,
        vehicle.pos.y,
        speed_to_kmh(vehicle.kind, vehicle.cur_speed),
        engine.speed_kmh(),
        vehicle.cargo,
        vehicle.capacity,
        vehicle.cargo_packets.packets.len(),
        vehicle.cargo_packets.max_periods_in_transit(),
        engine.running_cost_year,
        engine.reliability_pct,
        vehicle.orders.len(),
    )
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_vehicle_window(
    window_state: Res<VehicleWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut body_q: Query<
        &mut Text,
        (
            With<VehicleWindowBodyText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut status_q: Query<
        (&mut Text, &mut TextColor),
        (
            With<VehicleWindowStatusText>,
            Without<VehicleWindowBodyText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut toggle_q: Query<
        &mut Text,
        (
            With<VehicleWindowToggleText>,
            Without<VehicleWindowStatusText>,
            Without<VehicleWindowBodyText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut rename_row_q: Query<&mut Node, With<VehicleWindowRenameRow>>,
    mut train_row_q: Query<
        &mut Node,
        (
            With<VehicleWindowTrainOnly>,
            Without<VehicleWindowRenameRow>,
        ),
    >,
    mut refit_row_q: Query<
        &mut Node,
        (
            With<VehicleWindowRefitOnly>,
            Without<VehicleWindowRenameRow>,
            Without<VehicleWindowTrainOnly>,
        ),
    >,
    _rename_input_q: Query<&mut EditableText, With<VehicleWindowRenameInput>>,
    mut preview: Query<
        (&mut Transform, &mut Camera),
        (With<VehicleWindowPreviewCamera>, Without<PrimaryGameCamera>),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Vehicle)
    else {
        return;
    };
    let vehicle = window_state
        .vehicle_id
        .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
    let Some(vehicle) = vehicle else {
        *vis = Visibility::Hidden;
        if let Ok((_, mut cam)) = preview.single_mut() {
            cam.is_active = false;
        }
        return;
    };
    *vis = Visibility::Visible;

    if window_state.rename_editing
        && let Ok(mut row) = rename_row_q.single_mut()
    {
        row.display = Display::Flex;
    } else if let Ok(mut row) = rename_row_q.single_mut() {
        row.display = Display::None;
    }

    let train_display = if vehicle.kind == VehicleKind::Train {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut row) = train_row_q.single_mut() {
        row.display = train_display;
    }

    let refit_display = if refit_allowed(vehicle, &sim.state.map) {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut row) = refit_row_q.single_mut() {
        row.display = refit_display;
    }

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Vehicle)
    {
        **title = vehicle.display_name();
    }
    if let Ok(mut body) = body_q.single_mut() {
        **body = vehicle_details_body(vehicle, &sim);
    }
    if let Ok((mut status, mut color)) = status_q.single_mut() {
        if vehicle.running {
            if vehicle.no_network_route_to_order {
                **status = "Sin ruta".to_string();
                *color = TextColor(STATUS_NO_ROUTE);
            } else {
                **status = "En marcha".to_string();
                *color = TextColor(STATUS_RUNNING);
            }
        } else {
            **status = "Detenido".to_string();
            *color = TextColor(STATUS_STOPPED);
        }
    }
    if let Ok(mut toggle) = toggle_q.single_mut() {
        **toggle = if vehicle.running {
            "Detener".to_string()
        } else {
            "Iniciar".to_string()
        };
    }
    if let Ok((mut tf, mut cam)) = preview.single_mut() {
        cam.is_active = true;
        let world_pos = vehicle_world_position(vehicle, &sim.state.map);
        tf.translation = Vec3::new(world_pos.x, world_pos.y, 999.0);
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_vehicle_window_buttons(
    mut buttons: Query<(&Interaction, &VehicleWindowButton), (Changed<Interaction>, With<Button>)>,
    mut window_state: ResMut<VehicleWindowState>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut rename_input_q: Query<&mut EditableText, With<VehicleWindowRenameInput>>,
    time: Res<Time>,
) {
    for (interaction, button) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(vehicle_id) = window_state.vehicle_id else {
            continue;
        };
        match button {
            VehicleWindowButton::ToggleRunning => {
                if apply_command(&mut sim.state, &Command::ToggleVehicleRunning(vehicle_id)).is_ok()
                {
                    pending.pending = true;
                }
            }
            VehicleWindowButton::Orders => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    open_order_edit_for_vehicle(&mut order_state, vehicle, &mut next_pick);
                }
            }
            VehicleWindowButton::GotoDepot => {
                match apply_command(&mut sim.state, &Command::AppendGotoNearestDepot(vehicle_id)) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::CenterOrder => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
                    && !vehicle.orders.is_empty()
                {
                    let order = vehicle.orders[vehicle.current_order.min(vehicle.orders.len() - 1)];
                    let dest = resolve_order_destination(&sim.state.map, vehicle.kind, order);
                    let world = tile_camera_world_pos(&sim.state.map, dest);
                    if let Ok(mut transform) = cam_q.single_mut() {
                        transform.translation.x = world.x;
                        transform.translation.y = world.y;
                    }
                }
            }
            VehicleWindowButton::Rename => {
                window_state.rename_editing = true;
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id)
                    && let Ok(mut editable) = rename_input_q.single_mut()
                {
                    let seed = vehicle
                        .name
                        .as_deref()
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or(vehicle.effective_engine().name);
                    editable.editor_mut().set_text(seed);
                }
            }
            VehicleWindowButton::CenterCamera => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    let world_pos = vehicle_world_position(vehicle, &sim.state.map);
                    if let Ok(mut transform) = cam_q.single_mut() {
                        transform.translation.x = world_pos.x;
                        transform.translation.y = world_pos.y;
                    }
                }
            }
            VehicleWindowButton::TurnAround => {
                match apply_command(&mut sim.state, &Command::TurnAroundVehicle(vehicle_id)) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::ForceProceed => {
                match apply_command(&mut sim.state, &Command::ForceVehicleProceed(vehicle_id)) {
                    Ok(()) => {}
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleWindowButton::Refit => {
                let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
                    continue;
                };
                let Some(cargo) = next_refit_cargo(vehicle) else {
                    continue;
                };
                match apply_command(&mut sim.state, &Command::RefitVehicle { vehicle_id, cargo }) {
                    Ok(()) => {}
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
        }
    }
}

fn apply_vehicle_rename(
    window_state: &mut VehicleWindowState,
    sim: &mut SimWorld,
    hud_feedback: &mut HudBuildFeedback,
    rename_input_q: &Query<&EditableText, With<VehicleWindowRenameInput>>,
    elapsed_secs: f32,
) {
    let Some(vehicle_id) = window_state.vehicle_id else {
        return;
    };
    let name = rename_input_q.single().ok().map(|e| e.value().to_string());
    match apply_command(&mut sim.state, &Command::RenameVehicle { vehicle_id, name }) {
        Ok(()) => window_state.rename_editing = false,
        Err(e) => push_build_command_error(hud_feedback, e, elapsed_secs),
    }
}

pub(crate) fn handle_vehicle_rename_buttons(
    mut buttons: Query<
        (&Interaction, &VehicleWindowRenameButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut window_state: ResMut<VehicleWindowState>,
    rename_input_q: Query<&EditableText, With<VehicleWindowRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, action) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            VehicleWindowRenameButton::Cancel => {
                window_state.rename_editing = false;
            }
            VehicleWindowRenameButton::Apply => {
                apply_vehicle_rename(
                    &mut window_state,
                    &mut sim,
                    &mut hud_feedback,
                    &rename_input_q,
                    time.elapsed_secs(),
                );
            }
        }
    }
}

/// Enter aplica el nombre; Escape cancela edición.
pub(crate) fn vehicle_window_rename_keyboard(
    mut window_state: ResMut<VehicleWindowState>,
    keys: Res<ButtonInput<KeyCode>>,
    rename_input_q: Query<&EditableText, With<VehicleWindowRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !window_state.rename_editing {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        window_state.rename_editing = false;
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        apply_vehicle_rename(
            &mut window_state,
            &mut sim,
            &mut hud_feedback,
            &rename_input_q,
            time.elapsed_secs(),
        );
    }
}

/// Teclas alfanuméricas en el campo de renombrado.
pub(crate) fn vehicle_window_rename_editable_keyboard(
    window_state: Res<VehicleWindowState>,
    mut key_events: MessageReader<KeyboardInput>,
    mut rename_input_q: Query<&mut EditableText, With<VehicleWindowRenameInput>>,
) {
    if !window_state.rename_editing {
        key_events.clear();
        return;
    }
    let Ok(mut editable) = rename_input_q.single_mut() else {
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
        let Some(text) = &ev.text else {
            continue;
        };
        for c in text.chars() {
            if !c.is_control() && editable.value().chars().count() < MAX_VEHICLE_NAME_CHARS {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(c.to_string()),
                ));
            }
        }
    }
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn vehicle_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut window_state: ResMut<VehicleWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Vehicle {
            window_state.vehicle_id = None;
            window_state.rename_editing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn road_speed_units_halve_for_display() {
        assert_eq!(speed_to_kmh(VehicleKind::Bus, 112), 56);
        assert_eq!(speed_to_kmh(VehicleKind::Truck, 96), 48);
        assert_eq!(speed_to_kmh(VehicleKind::Train, 128), 128);
    }
}
