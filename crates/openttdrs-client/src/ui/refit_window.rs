//! Ventana de refit: lista de cargas con coste/capacidad (#178).

use bevy::prelude::*;
use openttdrs_core::Command;
#[cfg(test)]
use openttdrs_core::prelude::*;
use openttdrs_core::{
    CargoType, cargo_display_name, consist_unit_ids, refit_allowed, refittable_cargo_types,
};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, WindowKey, spawn_floating_window_keyed, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::vehicle_chain::{
    MAX_VEHICLE_CHAIN_SLOTS, VehicleChainRegistry, VehicleChainSlot, vehicle_window_key,
};

/// Cubrir `TRUCK_FREIGHT` (29 cargos vanilla) y margen; la lista hace scroll.
const REFIT_ROWS: usize = 32;
const CONSIST_SLOTS: usize = 8;
const BASE_POS: Vec2 = Vec2::new(520.0, 220.0);
const SLOT_OFFSET: Vec2 = Vec2::new(36.0, 36.0);
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Clone, Debug, Default)]
pub(crate) struct RefitSlotState {
    pub(crate) open: bool,
    pub(crate) vehicle_id: Option<u32>,
    /// Unidades del consist seleccionadas para refit; vacío = solo `vehicle_id`.
    pub(crate) selected_unit_ids: Vec<u32>,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct RefitWindowState {
    pub(crate) slots: [RefitSlotState; MAX_VEHICLE_CHAIN_SLOTS],
    pub(crate) focused: Option<u32>,
}

impl RefitWindowState {
    #[must_use]
    #[allow(dead_code)] // API multi-slot (#244); handlers usan slots[idx] directo.
    pub(crate) fn vehicle_id(&self) -> Option<u32> {
        self.focused.filter(|&id| {
            self.slots
                .iter()
                .any(|s| s.open && s.vehicle_id == Some(id))
        })
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn open(&self) -> bool {
        self.slots.iter().any(|s| s.open)
    }

    pub(crate) fn open_for(&mut self, chain: &VehicleChainRegistry, vehicle_id: u32) {
        let Some(slot) = chain.slot_of(vehicle_id) else {
            return;
        };
        self.slots[slot as usize] = RefitSlotState {
            open: true,
            vehicle_id: Some(vehicle_id),
            selected_unit_ids: Vec::new(),
        };
        self.focused = Some(vehicle_id);
    }

    pub(crate) fn close_vehicle(&mut self, vehicle_id: u32) {
        for slot in &mut self.slots {
            if slot.vehicle_id == Some(vehicle_id) {
                *slot = RefitSlotState::default();
            }
        }
        if self.focused == Some(vehicle_id) {
            self.focused = self
                .slots
                .iter()
                .find(|s| s.open)
                .and_then(|s| s.vehicle_id);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn focused_slot_mut(&mut self) -> Option<&mut RefitSlotState> {
        let id = self.focused?;
        self.slots
            .iter_mut()
            .find(|s| s.open && s.vehicle_id == Some(id))
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
    for slot in 0..MAX_VEHICLE_CHAIN_SLOTS {
        let slot_u8 = slot as u8;
        let pos = BASE_POS + SLOT_OFFSET * slot as f32;
        let (root, content) = spawn_floating_window_keyed(
            &mut commands,
            asset_server,
            WindowKey {
                class: FloatingWindowId::Refit,
                instance: 0,
            },
            "Refit",
            TITLE_CRIMSON,
            pos,
            360.0,
        );
        commands.entity(root).insert(VehicleChainSlot(slot_u8));
        spawn_refit_content(&mut commands, content, asset_server, slot_u8);
    }
}

fn spawn_refit_content(
    commands: &mut Commands,
    content: Entity,
    asset_server: &AssetServer,
    chain_slot: u8,
) {
    let chain = VehicleChainSlot(chain_slot);
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            RefitWindowHintText,
            chain,
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
                for unit_slot in 0..CONSIST_SLOTS {
                    row.spawn((
                        Button,
                        RefitUnitRow { slot: unit_slot },
                        chain,
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
                            RefitUnitRowText { slot: unit_slot },
                            chain,
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
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
                row_gap: Val::Px(2.0),
                ..default()
            },
            BTN_BG,
            BTN_BORDER,
            (),
            (),
            |list| {
                for cargo_slot in 0..REFIT_ROWS {
                    list.spawn((
                        Button,
                        RefitCargoRow { slot: cargo_slot },
                        chain,
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
                            RefitCargoRowText { slot: cargo_slot },
                            chain,
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            },
            200.0,
        );
    });
}

/// Unidades a refitear: selección explícita o solo la cabeza.
fn refit_target_unit_ids(vehicle_id: u32, selected: &[u32]) -> Vec<u32> {
    if selected.is_empty() {
        vec![vehicle_id]
    } else {
        selected.to_vec()
    }
}

fn refit_result_capacity(sim: &SimWorld, unit_ids: &[u32]) -> u32 {
    unit_ids
        .iter()
        .filter_map(|&id| {
            sim.state
                .vehicles
                .iter()
                .find(|v| v.id == id)
                .map(|v| v.capacity)
        })
        .sum()
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_refit_window(
    state: Res<RefitWindowState>,
    chain: Res<VehicleChainRegistry>,
    sim: Res<SimWorld>,
    mut root_q: Query<(
        Entity,
        &mut FloatingWindow,
        &VehicleChainSlot,
        &mut Visibility,
    )>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text, &ChildOf),
        (
            Without<RefitWindowHintText>,
            Without<RefitCargoRowText>,
            Without<RefitUnitRowText>,
        ),
    >,
    parents: Query<&ChildOf>,
    mut hint_q: Query<
        (&VehicleChainSlot, &mut Text),
        (
            With<RefitWindowHintText>,
            Without<FloatingWindowTitleText>,
            Without<RefitCargoRowText>,
            Without<RefitUnitRowText>,
        ),
    >,
    mut row_q: Query<
        (
            &VehicleChainSlot,
            &RefitCargoRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        Without<RefitUnitRow>,
    >,
    mut row_text_q: Query<
        (&VehicleChainSlot, &RefitCargoRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<RefitWindowHintText>,
            Without<RefitUnitRowText>,
        ),
    >,
    mut unit_q: Query<
        (
            &VehicleChainSlot,
            &RefitUnitRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        Without<RefitCargoRow>,
    >,
    mut unit_text_q: Query<
        (&VehicleChainSlot, &RefitUnitRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<RefitWindowHintText>,
            Without<RefitCargoRowText>,
        ),
    >,
) {
    fn title_root_entity(child_of: &ChildOf, parents: &Query<&ChildOf>) -> Option<Entity> {
        let center = child_of.parent();
        let bar = parents.get(center).ok()?.parent();
        parents.get(bar).ok().map(ChildOf::parent)
    }

    for (root_entity, mut win, slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::Refit {
            continue;
        }
        let idx = slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        let slot_state = &state.slots[idx];
        let vehicle_id = slot_state
            .vehicle_id
            .filter(|&id| slot_state.open && chain.slot_of(id) == Some(slot.0));
        win.key = vehicle_window_key(FloatingWindowId::Refit, vehicle_id.unwrap_or(0));
        let vehicle = vehicle_id.and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
        let Some(vehicle) = vehicle else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Visible;

        let title_name = format!("Refit · {}", vehicle.display_name());
        for (title, mut text, child_of) in &mut title_q {
            if title.0 != FloatingWindowId::Refit {
                continue;
            }
            if title_root_entity(child_of, &parents) == Some(root_entity) {
                **text = title_name.clone();
            }
        }

        let allowed = refit_allowed(vehicle, &sim.state.map);
        let options = refittable_cargo_types(vehicle);
        let unit_ids = consist_unit_ids(&sim.state.vehicles, vehicle.id);
        let show_units = unit_ids.len() > 1;
        let target_ids = refit_target_unit_ids(vehicle.id, &slot_state.selected_unit_ids);
        let selected_count = target_ids.len();
        let result_capacity = refit_result_capacity(&sim, &target_ids);
        for (hint_slot, mut hint) in &mut hint_q {
            if hint_slot.0 != slot.0 {
                continue;
            }
            **hint = if !allowed {
                "Refit solo en depósito, sin carga y con tipos alternativos.".to_string()
            } else if show_units {
                format!(
                    "Unidades: {selected_count}/{} · Cap. resultante: {result_capacity}\n                 Clic en unidad para seleccionar; clic en carga para aplicar.",
                    unit_ids.len()
                )
            } else {
                format!(
                    "Cap. resultante: {result_capacity} · Coste: gratis\n                 Clic en una carga de la lista para aplicar."
                )
            };
        }

        for (unit_slot, row, interaction, mut node, mut bg) in &mut unit_q {
            if unit_slot.0 != slot.0 {
                continue;
            }
            let Some(&unit_id) = unit_ids.get(row.slot) else {
                node.display = Display::None;
                continue;
            };
            if !show_units {
                node.display = Display::None;
                continue;
            }
            node.display = Display::Flex;
            let selected = slot_state.selected_unit_ids.is_empty() && unit_id == vehicle.id
                || slot_state.selected_unit_ids.contains(&unit_id);
            *bg = if selected {
                BackgroundColor(BTN_ACTIVE)
            } else if *interaction == Interaction::Hovered {
                BackgroundColor(BTN_HOVER)
            } else {
                BackgroundColor(BTN_BG)
            };
        }
        for (text_slot, row_text, mut text) in &mut unit_text_q {
            if text_slot.0 != slot.0 {
                continue;
            }
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
        for (row_slot, row, interaction, mut node, mut bg) in &mut row_q {
            if row_slot.0 != slot.0 {
                continue;
            }
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
        for (text_slot, row_text, mut text) in &mut row_text_q {
            if text_slot.0 != slot.0 {
                continue;
            }
            if let Some(cargo) = options.get(row_text.slot).copied() {
                let mark = if cargo == current { " ●" } else { "" };
                **text = format!(
                    "{} · cap. {result_capacity} · gratis{mark}",
                    cargo_display_name(cargo)
                );
            } else {
                **text = String::new();
            }
        }
    }
}

pub(crate) fn handle_refit_window_buttons(
    mut rows: Query<
        (&Interaction, &RefitCargoRow, &VehicleChainSlot),
        (Changed<Interaction>, With<Button>),
    >,
    mut units: Query<
        (&Interaction, &RefitUnitRow, &VehicleChainSlot),
        (Changed<Interaction>, With<Button>),
    >,
    mut state: ResMut<RefitWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, row, chain_slot) in &mut units {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        let vehicle_id = state
            .slots
            .get(idx)
            .and_then(|s| if s.open { s.vehicle_id } else { None });
        let Some(vehicle_id) = vehicle_id else {
            continue;
        };
        state.focused = Some(vehicle_id);
        let unit_ids = consist_unit_ids(&sim.state.vehicles, vehicle_id);
        let Some(&unit_id) = unit_ids.get(row.slot) else {
            continue;
        };
        let Some(slot) = state.slots.get_mut(idx) else {
            continue;
        };
        if let Some(pos) = slot.selected_unit_ids.iter().position(|&id| id == unit_id) {
            slot.selected_unit_ids.remove(pos);
        } else {
            slot.selected_unit_ids.push(unit_id);
        }
    }

    for (interaction, row, chain_slot) in &mut rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        let (vehicle_id, selected) = {
            let Some(slot) = state.slots.get(idx) else {
                continue;
            };
            if !slot.open {
                continue;
            }
            let Some(vehicle_id) = slot.vehicle_id else {
                continue;
            };
            (vehicle_id, slot.selected_unit_ids.clone())
        };
        state.focused = Some(vehicle_id);
        let Some(options) = sim
            .state
            .vehicles
            .iter()
            .find(|v| v.id == vehicle_id)
            .map(refittable_cargo_types)
        else {
            continue;
        };
        let options: Vec<CargoType> = options.to_vec();
        let Some(cargo) = options.get(row.slot).copied() else {
            continue;
        };
        match crate::network::apply_player_command(
            &mut sim.state,
            &Command::RefitVehicle {
                vehicle_id,
                cargo,
                unit_ids: selected,
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
        if message.0.class != FloatingWindowId::Refit {
            continue;
        }
        let vehicle_id = message.0.instance;
        if vehicle_id == 0 {
            continue;
        }
        state.close_vehicle(vehicle_id);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

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
        let mut refit = RefitWindowState::default();
        refit.slots[0] = RefitSlotState {
            open: true,
            vehicle_id: Some(9),
            selected_unit_ids: vec![],
        };
        refit.focused = Some(9);
        world.insert_resource(refit);
        world.init_resource::<VehicleChainRegistry>();
        world
            .resource_mut::<VehicleChainRegistry>()
            .open_or_focus(9);
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
            VehicleChainSlot(0),
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
        world.init_resource::<VehicleChainRegistry>();
        world.insert_resource(sim_with(GameState::new(8, 8)));
        assert!(
            world.run_system_once(sync_refit_window).is_ok(),
            "Queries de sync_refit_window deben ser disjuntas (B0001)"
        );
    }

    #[test]
    fn refit_rows_cover_truck_freight_options() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        v.cargo_type = Some(CargoType::Mail);
        assert!(
            refittable_cargo_types(&v).len() <= REFIT_ROWS,
            "REFIT_ROWS debe cubrir todas las cargas de camión"
        );
    }

    #[test]
    fn cargo_row_label_includes_capacity_and_cost() {
        let label = format!(
            "{} · cap. {} · gratis",
            cargo_display_name(CargoType::Coal),
            40
        );
        assert!(label.contains("cap."));
        assert!(label.contains("gratis"));
        assert!(label.contains(cargo_display_name(CargoType::Coal)));
    }
}
