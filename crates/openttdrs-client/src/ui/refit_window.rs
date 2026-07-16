//! Ventana de refit: lista de cargas disponibles en depósito.

use bevy::prelude::*;
use openttdrs_core::{
    CargoType, Command, cargo_display_name, consist_unit_ids, refit_allowed, refittable_cargo_types,
};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

const REFIT_ROWS: usize = 8;
const CONSIST_SLOTS: usize = 8;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct RefitWindowState {
    pub(crate) open: bool,
    pub(crate) vehicle_id: Option<u32>,
    /// Unidades del consist seleccionadas para refit; vacío = solo `vehicle_id`.
    pub(crate) selected_unit_ids: Vec<u32>,
}

impl RefitWindowState {
    pub(crate) fn open_for(&mut self, vehicle_id: u32) {
        self.open = true;
        self.vehicle_id = Some(vehicle_id);
        self.selected_unit_ids.clear();
    }
}

#[derive(Component)]
pub(crate) struct RefitWindowHintText;

#[derive(Component, Clone, Copy)]
pub(crate) struct RefitCargoRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct RefitCargoRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct RefitUnitRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct RefitUnitRowText {
    slot: usize,
}

pub(crate) fn setup_refit_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Refit,
        "Refit",
        TITLE_CRIMSON,
        Vec2::new(520.0, 180.0),
        340.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            RefitWindowHintText,
            Text::new("Elige el tipo de carga."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(6.0)),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                for slot in 0..CONSIST_SLOTS {
                    row.spawn((
                        Button,
                        RefitUnitRow { slot },
                        Node {
                            width: Val::Px(54.0),
                            height: Val::Px(22.0),
                            padding: UiRect::horizontal(Val::Px(4.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            RefitUnitRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::top(Val::Px(6.0)),
                max_height: Val::Px(200.0),
                overflow: Overflow::scroll_y(),
                ..default()
            })
            .with_children(|list| {
                for slot in 0..REFIT_ROWS {
                    list.spawn((
                        Button,
                        RefitCargoRow { slot },
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(24.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            RefitCargoRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
    });
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_refit_window(
    state: Res<RefitWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (
            Without<RefitWindowHintText>,
            Without<RefitCargoRowText>,
            Without<RefitUnitRowText>,
        ),
    >,
    mut hint_q: Query<
        &mut Text,
        (
            With<RefitWindowHintText>,
            Without<FloatingWindowTitleText>,
            Without<RefitCargoRowText>,
            Without<RefitUnitRowText>,
        ),
    >,
    mut row_q: Query<
        (
            &RefitCargoRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        Without<RefitUnitRow>,
    >,
    mut row_text_q: Query<
        (&RefitCargoRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<RefitWindowHintText>,
            Without<RefitUnitRowText>,
        ),
    >,
    mut unit_q: Query<
        (&RefitUnitRow, &Interaction, &mut Node, &mut BackgroundColor),
        Without<RefitCargoRow>,
    >,
    mut unit_text_q: Query<
        (&RefitUnitRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<RefitWindowHintText>,
            Without<RefitCargoRowText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::Refit)
    else {
        return;
    };
    let vehicle = state
        .vehicle_id
        .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
    let Some(vehicle) = vehicle.filter(|_| state.open) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(text, _)| text.0 == FloatingWindowId::Refit)
    {
        **title = format!("Refit · {}", vehicle.display_name());
    }

    let allowed = refit_allowed(vehicle, &sim.state.map);
    let options = refittable_cargo_types(vehicle);
    let unit_ids = consist_unit_ids(&sim.state.vehicles, vehicle.id);
    let show_units = unit_ids.len() > 1;
    let selected_count = if state.selected_unit_ids.is_empty() {
        1
    } else {
        state.selected_unit_ids.len()
    };
    if let Ok(mut hint) = hint_q.single_mut() {
        **hint = if !allowed {
            "Refit solo en depósito, sin carga y con tipos alternativos.".to_string()
        } else if show_units {
            format!(
                "Capacidad actual: {} · Unidades: {selected_count}/{}\n\
                 Clic en unidad para seleccionar; clic en carga para aplicar.",
                vehicle.capacity,
                unit_ids.len()
            )
        } else {
            format!(
                "Capacidad actual: {} · Coste: gratis\nClic para aplicar el tipo de carga.",
                vehicle.capacity
            )
        };
    }

    for (row, interaction, mut node, mut bg) in &mut unit_q {
        let Some(&unit_id) = unit_ids.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        if !show_units {
            node.display = Display::None;
            continue;
        }
        node.display = Display::Flex;
        let selected = state.selected_unit_ids.is_empty() && unit_id == vehicle.id
            || state.selected_unit_ids.contains(&unit_id);
        *bg = if selected {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut unit_text_q {
        if let Some(&unit_id) = unit_ids.get(row_text.slot)
            && show_units
        {
            **text = format!("U{unit_id}");
        } else {
            **text = String::new();
        }
    }

    let current = vehicle
        .cargo_type
        .unwrap_or(options.first().copied().unwrap_or(CargoType::Goods));
    for (row, interaction, mut node, mut bg) in &mut row_q {
        let Some(cargo) = options.get(row.slot).copied() else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = cargo == current;
        *bg = if !allowed {
            BackgroundColor(Color::srgb(0.28, 0.24, 0.17))
        } else if selected {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        if let Some(cargo) = options.get(row_text.slot).copied() {
            let mark = if cargo == current { " ●" } else { "" };
            **text = format!("{}{mark}", cargo_display_name(cargo));
        } else {
            **text = String::new();
        }
    }
}

pub(crate) fn handle_refit_window_buttons(
    mut rows: Query<(&Interaction, &RefitCargoRow), (Changed<Interaction>, With<Button>)>,
    mut units: Query<(&Interaction, &RefitUnitRow), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<RefitWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    let Some(vehicle_id) = state.vehicle_id else {
        return;
    };
    if !state.open {
        return;
    }
    let unit_ids = consist_unit_ids(&sim.state.vehicles, vehicle_id);
    for (interaction, row) in &mut units {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(&unit_id) = unit_ids.get(row.slot) else {
            continue;
        };
        if let Some(pos) = state.selected_unit_ids.iter().position(|&id| id == unit_id) {
            state.selected_unit_ids.remove(pos);
        } else {
            state.selected_unit_ids.push(unit_id);
        }
    }

    let Some(options) = sim
        .state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .map(refittable_cargo_types)
    else {
        return;
    };
    let options: Vec<CargoType> = options.to_vec();
    let selected = state.selected_unit_ids.clone();
    for (interaction, row) in &mut rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(cargo) = options.get(row.slot).copied() else {
            continue;
        };
        match crate::network::apply_player_command(
            &mut sim.state,
            &Command::RefitVehicle {
                vehicle_id,
                cargo,
                unit_ids: selected.clone(),
            },
        ) {
            Ok(()) => pending.pending = true,
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
}

pub(crate) fn refit_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<RefitWindowState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::Refit {
            state.open = false;
            state.vehicle_id = None;
            state.selected_unit_ids.clear();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{GameState, TileCoord, TileKind, Vehicle, VehicleKind};

    fn sim_with(state: GameState) -> SimWorld {
        SimWorld {
            state,
            ..SimWorld::default()
        }
    }

    #[test]
    fn refit_row_applies_cargo_type() {
        let mut world = World::new();
        let mut state = GameState::new(16, 16);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        let mut vehicle = Vehicle::new(9, VehicleKind::Truck, depot, depot);
        vehicle.cargo = 0;
        vehicle.cargo_type = Some(CargoType::Mail);
        state.vehicles.push(vehicle);
        world.insert_resource(sim_with(state));
        world.insert_resource(RefitWindowState {
            open: true,
            vehicle_id: Some(9),
            selected_unit_ids: vec![],
        });
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        let coal_slot =
            refittable_cargo_types(world.resource::<SimWorld>().state.vehicles.first().unwrap())
                .iter()
                .position(|&c| c == CargoType::Coal)
                .unwrap();
        world.spawn((
            Button,
            RefitCargoRow { slot: coal_slot },
            Interaction::Pressed,
        ));
        world.run_system_once(handle_refit_window_buttons).unwrap();
        let sim = world.resource::<SimWorld>();
        assert_eq!(sim.state.vehicles[0].cargo_type, Some(CargoType::Coal));
    }

    #[test]
    fn sync_refit_queries_are_disjoint() {
        let mut world = World::new();
        world.init_resource::<RefitWindowState>();
        world.insert_resource(sim_with(GameState::new(8, 8)));
        assert!(
            world.run_system_once(sync_refit_window).is_ok(),
            "Queries de sync_refit_window deben ser disjuntas (B0001)"
        );
    }
}
