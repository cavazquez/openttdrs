//! Ventana de detalles del vehículo (`VehicleDetailsWindow` OpenTTD / #173–#175).
//!
//! Se abre desde el botón «Detalles» de la vista. Tabs Info / Carga / Capacidad /
//! Totales con **una fila por unidad** del consist (sprite + datos).
//!
//! Multi-instancia (#244): hasta [`MAX_VEHICLE_CHAIN_SLOTS`] Details concurrentes.

mod details;

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::render::TruckHandles;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, WindowKey, spawn_floating_window_keyed, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::refit_window::RefitWindowState;
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::vehicle_chain::{
    MAX_VEHICLE_CHAIN_SLOTS, VehicleChainRegistry, VehicleChainSlot, vehicle_window_key,
};
use crate::ui::vehicle_window::{
    CONSIST_UNIT_SPRITE_H, CONSIST_UNIT_SPRITE_W, vehicle_side_sprite,
};

pub(crate) use details::speed_to_kmh;

use details::{details_unit_ids, vehicle_details_summary, vehicle_details_unit_line};

const DETAILS_UNIT_ROWS: usize = 24;
const ROW_HEIGHT: f32 = 28.0;
const LIST_VISIBLE_ROWS: usize = 7;
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";
const BASE_POS: Vec2 = Vec2::new(440.0, 280.0);
const SLOT_OFFSET: Vec2 = Vec2::new(36.0, 36.0);

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const LIST_BG: Color = Color::srgb(0.16, 0.13, 0.09);
const ROW_BG: Color = Color::srgb(0.22, 0.18, 0.12);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum VehicleDetailsTab {
    #[default]
    Info,
    Cargo,
    Capacity,
    Totals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct VehicleDetailsSlotState {
    pub(crate) vehicle_id: Option<u32>,
    pub(crate) details_tab: VehicleDetailsTab,
}

#[derive(Resource, Debug)]
pub(crate) struct VehicleDetailsWindowState {
    pub(crate) slots: [VehicleDetailsSlotState; MAX_VEHICLE_CHAIN_SLOTS],
    /// Vehículo enfocado (acciones que no llevan `VehicleChainSlot`).
    pub(crate) focused: Option<u32>,
}

impl Default for VehicleDetailsWindowState {
    fn default() -> Self {
        Self {
            slots: [VehicleDetailsSlotState::default(); MAX_VEHICLE_CHAIN_SLOTS],
            focused: None,
        }
    }
}

impl VehicleDetailsWindowState {
    /// Compat: vehicle_id del Details enfocado.
    #[must_use]
    pub(crate) fn vehicle_id(&self) -> Option<u32> {
        self.focused
            .filter(|&id| self.slots.iter().any(|s| s.vehicle_id == Some(id)))
    }

    /// Abre Details en el slot del registry para `vehicle_id`.
    pub(crate) fn open_for(&mut self, chain: &VehicleChainRegistry, vehicle_id: u32) {
        let Some(slot) = chain.slot_of(vehicle_id) else {
            return;
        };
        self.slots[slot as usize].vehicle_id = Some(vehicle_id);
        self.focused = Some(vehicle_id);
    }

    pub(crate) fn close_vehicle(&mut self, vehicle_id: u32) {
        for slot in &mut self.slots {
            if slot.vehicle_id == Some(vehicle_id) {
                *slot = VehicleDetailsSlotState::default();
            }
        }
        if self.focused == Some(vehicle_id) {
            self.focused = self.slots.iter().find_map(|s| s.vehicle_id);
        }
    }

    #[must_use]
    pub(crate) fn is_open_for(&self, vehicle_id: u32) -> bool {
        self.slots.iter().any(|s| s.vehicle_id == Some(vehicle_id))
    }
}

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleDetailsTabButton(pub(crate) VehicleDetailsTab);

/// Acciones de chrome en Details (p. ej. abrir Refit).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VehicleDetailsAction {
    Refit,
}

/// Resumen del tab Totales (oculto en el resto).
#[derive(Component)]
pub(crate) struct VehicleDetailsSummaryText;

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleDetailsUnitRow {
    unit_idx: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleDetailsUnitSprite {
    unit_idx: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleDetailsUnitText {
    unit_idx: usize,
}

/// TitleText → contenedor → title bar → FloatingWindow root.
fn title_root_entity(child_of: &ChildOf, parents: &Query<&ChildOf>) -> Option<Entity> {
    let center = child_of.parent();
    let bar = parents.get(center).ok()?.parent();
    parents.get(bar).ok().map(|c| c.parent())
}

pub(crate) fn setup_vehicle_details_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    for slot in 0..MAX_VEHICLE_CHAIN_SLOTS {
        let slot_u8 = slot as u8;
        let pos = BASE_POS + SLOT_OFFSET * slot as f32;
        let (root, content) = spawn_floating_window_keyed(
            &mut commands,
            asset_server,
            WindowKey {
                class: FloatingWindowId::VehicleDetails,
                instance: 0,
            },
            "Detalles",
            TITLE_CRIMSON,
            pos,
            380.0,
        );
        commands.entity(root).insert(VehicleChainSlot(slot_u8));
        spawn_details_content(&mut commands, content, asset_server, slot_u8);
    }
}

fn spawn_details_content(
    commands: &mut Commands,
    content: Entity,
    asset_server: &AssetServer,
    slot: u8,
) {
    let chain_slot = VehicleChainSlot(slot);
    commands.entity(content).with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_details_tab(
                    row,
                    asset_server,
                    chain_slot,
                    VehicleDetailsTab::Info,
                    "Info",
                );
                spawn_details_tab(
                    row,
                    asset_server,
                    chain_slot,
                    VehicleDetailsTab::Cargo,
                    "Carga",
                );
                spawn_details_tab(
                    row,
                    asset_server,
                    chain_slot,
                    VehicleDetailsTab::Capacity,
                    "Capacidad",
                );
                spawn_details_tab(
                    row,
                    asset_server,
                    chain_slot,
                    VehicleDetailsTab::Totals,
                    "Totales",
                );
                spawn_details_action(
                    row,
                    asset_server,
                    chain_slot,
                    VehicleDetailsAction::Refit,
                    "Refit",
                );
            });
        panel.spawn((
            VehicleDetailsSummaryText,
            chain_slot,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ));
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
                row_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            LIST_BG,
            BTN_BORDER,
            (),
            (),
            |list| {
                for unit_idx in 0..DETAILS_UNIT_ROWS {
                    spawn_details_unit_row(list, asset_server, chain_slot, unit_idx);
                }
            },
            ROW_HEIGHT * LIST_VISIBLE_ROWS as f32 + 4.0,
        );
    });
}

fn spawn_details_unit_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    unit_idx: usize,
) {
    parent
        .spawn((
            VehicleDetailsUnitRow { unit_idx },
            chain_slot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                display: Display::None,
                padding: UiRect::horizontal(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(ROW_BG),
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                VehicleDetailsUnitSprite { unit_idx },
                chain_slot,
                ImageNode::new(asset_server.load::<Image>(PLACEHOLDER_SPRITE)),
                Node {
                    width: Val::Px(CONSIST_UNIT_SPRITE_W),
                    height: Val::Px(CONSIST_UNIT_SPRITE_H),
                    flex_shrink: 0.0,
                    ..default()
                },
                BuildMenuUi,
            ));
            row.spawn((
                VehicleDetailsUnitText { unit_idx },
                chain_slot,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
        });
}

fn spawn_details_tab(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    tab: VehicleDetailsTab,
    label: &'static str,
) {
    parent.spawn((
        Button,
        VehicleDetailsTabButton(tab),
        chain_slot,
        Node {
            min_width: Val::Px(64.0),
            height: Val::Px(22.0),
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
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_details_action(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    action: VehicleDetailsAction,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        chain_slot,
        Node {
            min_width: Val::Px(56.0),
            height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            margin: UiRect::left(Val::Px(8.0)),
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

pub(crate) fn handle_vehicle_details_buttons(
    mut tab_buttons: Query<
        (&Interaction, &VehicleDetailsTabButton, &VehicleChainSlot),
        (
            Changed<Interaction>,
            With<Button>,
            Without<VehicleDetailsAction>,
        ),
    >,
    mut action_buttons: Query<
        (&Interaction, &VehicleDetailsAction, &VehicleChainSlot),
        (
            Changed<Interaction>,
            With<Button>,
            Without<VehicleDetailsTabButton>,
        ),
    >,
    mut details_state: ResMut<VehicleDetailsWindowState>,
    chain: Res<VehicleChainRegistry>,
    mut refit_window: ResMut<RefitWindowState>,
) {
    for (interaction, tab, chain_slot) in &mut tab_buttons {
        if *interaction == Interaction::Pressed {
            let idx = chain_slot.0 as usize;
            if idx < MAX_VEHICLE_CHAIN_SLOTS {
                details_state.slots[idx].details_tab = tab.0;
                if let Some(vid) = details_state.slots[idx].vehicle_id {
                    details_state.focused = Some(vid);
                }
            }
        }
    }
    for (interaction, action, chain_slot) in &mut action_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            VehicleDetailsAction::Refit => {
                let idx = chain_slot.0 as usize;
                if let Some(vehicle_id) = details_state.slots.get(idx).and_then(|s| s.vehicle_id) {
                    details_state.focused = Some(vehicle_id);
                    refit_window.open_for(&chain, vehicle_id);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // sistema ECS Bevy
pub(crate) fn sync_vehicle_details_window(
    details_state: Res<VehicleDetailsWindowState>,
    chain: Res<VehicleChainRegistry>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(Entity, &mut FloatingWindow, &VehicleChainSlot, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut summary_q: Query<
        (&VehicleChainSlot, &mut Text),
        (
            With<VehicleDetailsSummaryText>,
            Without<FloatingWindowTitleText>,
            Without<VehicleDetailsUnitText>,
        ),
    >,
    mut row_q: Query<
        (&VehicleChainSlot, &VehicleDetailsUnitRow, &mut Node),
        Without<VehicleDetailsUnitSprite>,
    >,
    mut sprite_q: Query<
        (
            &VehicleChainSlot,
            &VehicleDetailsUnitSprite,
            &mut ImageNode,
            &mut Node,
        ),
        Without<VehicleDetailsUnitRow>,
    >,
    mut unit_text_q: Query<
        (&VehicleChainSlot, &VehicleDetailsUnitText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<VehicleDetailsSummaryText>,
        ),
    >,
    mut tab_buttons: Query<
        (
            &VehicleChainSlot,
            &VehicleDetailsTabButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        (With<Button>, Without<VehicleDetailsAction>),
    >,
) {
    for (root_entity, mut win, slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::VehicleDetails {
            continue;
        }
        let slot_idx = slot.0 as usize;
        if slot_idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        let slot_state = details_state.slots[slot_idx];
        let vehicle_id = slot_state.vehicle_id.filter(|&id| chain.slot_of(id) == Some(slot.0));
        win.key = vehicle_window_key(
            FloatingWindowId::VehicleDetails,
            vehicle_id.unwrap_or(0),
        );
        let vehicle = vehicle_id.and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
        let Some(vehicle) = vehicle else {
            *vis = Visibility::Hidden;
            for (row_slot, _, mut node) in &mut row_q {
                if row_slot.0 == slot.0 {
                    node.display = Display::None;
                }
            }
            continue;
        };
        *vis = Visibility::Visible;

        let title_name = format!("Detalles — {}", vehicle.display_name());
        for (title, mut text, child_of) in &mut title_q {
            if title.0 != FloatingWindowId::VehicleDetails {
                continue;
            }
            if title_root_entity(child_of, &parents) == Some(root_entity) {
                **text = title_name.clone();
            }
        }
        for (sum_slot, mut summary) in &mut summary_q {
            if sum_slot.0 != slot.0 {
                continue;
            }
            **summary = vehicle_details_summary(&vehicle, &sim, slot_state.details_tab);
        }

        let unit_ids = details_unit_ids(&vehicle, &sim);
        for (row_slot, row, mut node) in &mut row_q {
            if row_slot.0 != slot.0 {
                continue;
            }
            node.display = if unit_ids.get(row.unit_idx).is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
        for (text_slot, unit_text, mut text) in &mut unit_text_q {
            if text_slot.0 != slot.0 {
                continue;
            }
            if let Some(&unit_id) = unit_ids.get(unit_text.unit_idx)
                && let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id)
            {
                **text = vehicle_details_unit_line(unit, &vehicle, &sim, slot_state.details_tab);
            } else {
                **text = String::new();
            }
        }
        if let Some(trucks) = trucks.as_ref() {
            for (sprite_slot, sprite, mut image, mut node) in &mut sprite_q {
                if sprite_slot.0 != slot.0 {
                    continue;
                }
                if let Some(&unit_id) = unit_ids.get(sprite.unit_idx)
                    && let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id)
                {
                    node.display = Display::Flex;
                    image.image = vehicle_side_sprite(trucks, unit);
                } else {
                    node.display = Display::None;
                }
            }
        } else {
            for (sprite_slot, _, _, mut node) in &mut sprite_q {
                if sprite_slot.0 == slot.0 {
                    node.display = Display::None;
                }
            }
        }

        for (tab_slot, tab, interaction, mut bg) in &mut tab_buttons {
            if tab_slot.0 != slot.0 {
                continue;
            }
            *bg = if tab.0 == slot_state.details_tab {
                BackgroundColor(BTN_ACTIVE)
            } else if *interaction == Interaction::Hovered {
                BackgroundColor(BTN_HOVER)
            } else {
                BackgroundColor(BTN_BG)
            };
        }
    }
}

pub(crate) fn vehicle_details_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut details_state: ResMut<VehicleDetailsWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::VehicleDetails {
            continue;
        }
        let vehicle_id = msg.0.instance;
        if vehicle_id == 0 {
            // Slot sin bind: no tocar otros.
            continue;
        }
        details_state.close_vehicle(vehicle_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::vehicle_chain::VehicleChainRegistry;

    #[test]
    fn open_for_sets_vehicle_id_on_slot() {
        let mut chain = VehicleChainRegistry::default();
        chain.open_or_focus(7);
        let mut state = VehicleDetailsWindowState::default();
        state.open_for(&chain, 7);
        assert_eq!(state.slots[0].vehicle_id, Some(7));
        assert_eq!(state.vehicle_id(), Some(7));
    }

    #[test]
    fn two_details_open_with_distinct_vehicle_ids() {
        let mut chain = VehicleChainRegistry::default();
        chain.open_or_focus(10);
        chain.open_or_focus(20);
        let mut state = VehicleDetailsWindowState::default();
        state.open_for(&chain, 10);
        state.open_for(&chain, 20);
        assert_eq!(state.slots[0].vehicle_id, Some(10));
        assert_eq!(state.slots[1].vehicle_id, Some(20));
        assert!(state.is_open_for(10));
        assert!(state.is_open_for(20));
    }

    #[test]
    fn closing_one_details_keeps_the_other() {
        let mut chain = VehicleChainRegistry::default();
        chain.open_or_focus(1);
        chain.open_or_focus(2);
        let mut state = VehicleDetailsWindowState::default();
        state.open_for(&chain, 1);
        state.open_for(&chain, 2);
        state.close_vehicle(1);
        assert!(!state.is_open_for(1));
        assert!(state.is_open_for(2));
        assert_eq!(state.slots[1].vehicle_id, Some(2));
        assert_eq!(state.focused, Some(2));
    }
}
