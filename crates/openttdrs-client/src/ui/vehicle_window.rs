//! Ventana flotante de tren/vehículo estilo `OpenTTD`.
//!
//! Se abre al hacer clic en un vehículo del mapa: vista previa en vivo
//! (cámara a render-target sobre el vehículo), modelo, velocidad actual y
//! máxima, carga, estado («Detenido» en rojo / «En marcha» en verde) y
//! acciones Iniciar/Detener, Órdenes, Centrar vista y Vender.

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::widget::ImageNode;
use openttdrs_core::{Command, VehicleKind, apply_command};

use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, vehicle_world_position,
};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::{BuildMenuUi, OrderEditState};

const PREVIEW_TEX_W: u32 = 280;
const PREVIEW_TEX_H: u32 = 120;
const PREVIEW_SCALE: f32 = 0.5;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const STATUS_STOPPED: Color = Color::srgb(0.92, 0.35, 0.3);
const STATUS_RUNNING: Color = Color::srgb(0.45, 0.85, 0.4);

#[derive(Resource, Default)]
pub(crate) struct VehicleWindowState {
    pub(crate) vehicle_id: Option<u32>,
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
    CenterCamera,
    Sell,
}

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
        Vec2::new(720.0, 120.0),
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
            window_text_font(asset_server, 11.0),
            TextColor(WINDOW_TEXT),
        ));
        panel.spawn((
            VehicleWindowStatusText,
            Text::new(""),
            window_text_font(asset_server, 12.0),
            TextColor(STATUS_STOPPED),
        ));
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
                    VehicleWindowButton::CenterCamera,
                    "Centrar",
                    false,
                );
                spawn_vehicle_button(
                    row,
                    asset_server,
                    VehicleWindowButton::Sell,
                    "Vender",
                    false,
                );
            });
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
                window_text_font(asset_server, 10.0),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            ));
            if is_toggle {
                text.insert(VehicleWindowToggleText);
            }
        });
}

fn vehicle_kind_label(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::Bus => "Bus",
        VehicleKind::Truck => "Camión",
        VehicleKind::Train => "Tren",
    }
}

/// km/h mostrados a partir de las unidades internas (vía ≈ 1, carretera ≈ 0,5).
const fn speed_to_kmh(kind: VehicleKind, units: u16) -> u16 {
    match kind {
        VehicleKind::Train => units,
        VehicleKind::Bus | VehicleKind::Truck => units / 2,
    }
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

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Vehicle)
    {
        **title = format!("{} #{}", vehicle_kind_label(vehicle.kind), vehicle.id);
    }
    let engine = vehicle.effective_engine();
    if let Ok(mut body) = body_q.single_mut() {
        **body = format!(
            "Modelo: {}\nVelocidad: {} km/h (máx. {} km/h)\nCarga: {}/{}  ·  Órdenes: {}",
            engine.name,
            speed_to_kmh(vehicle.kind, vehicle.cur_speed),
            engine.speed_kmh(),
            vehicle.cargo,
            vehicle.capacity,
            vehicle.orders.len(),
        );
    }
    if let Ok((mut status, mut color)) = status_q.single_mut() {
        if vehicle.running {
            **status = "En marcha".to_string();
            *color = TextColor(STATUS_RUNNING);
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
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
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
                    order_state.vehicle_id = Some(vehicle_id);
                    order_state.orders = vehicle.orders.clone();
                    order_state.picking_destination = false;
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
            VehicleWindowButton::Sell => {
                match apply_command(&mut sim.state, &Command::SellVehicle(vehicle_id)) {
                    Ok(()) => {
                        pending.pending = true;
                        window_state.vehicle_id = None;
                        if order_state.vehicle_id == Some(vehicle_id) {
                            order_state.vehicle_id = None;
                            order_state.orders.clear();
                            order_state.picking_destination = false;
                        }
                    }
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
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
