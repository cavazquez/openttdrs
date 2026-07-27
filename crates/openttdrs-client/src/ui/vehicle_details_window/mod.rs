//! Ventana de detalles del vehículo (`VehicleDetailsWindow` OpenTTD / #173–#175).
//!
//! Se abre desde el botón «Detalles» de la vista. Tabs Info / Carga / Capacidad /
//! Totales con **una fila por unidad** del consist (sprite + datos).

mod details;

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::render::TruckHandles;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::refit_window::RefitWindowState;
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::vehicle_window::{
    CONSIST_UNIT_SPRITE_H, CONSIST_UNIT_SPRITE_W, vehicle_side_sprite,
};

pub(crate) use details::speed_to_kmh;

use details::{details_unit_ids, vehicle_details_summary, vehicle_details_unit_line};

const DETAILS_UNIT_ROWS: usize = 24;
const ROW_HEIGHT: f32 = 28.0;
const LIST_VISIBLE_ROWS: usize = 7;
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";

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

#[derive(Resource, Default)]
pub(crate) struct VehicleDetailsWindowState {
    pub(crate) vehicle_id: Option<u32>,
    pub(crate) details_tab: VehicleDetailsTab,
}

impl VehicleDetailsWindowState {
    pub(crate) fn open_for(&mut self, vehicle_id: u32) {
        self.vehicle_id = Some(vehicle_id);
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

pub(crate) fn setup_vehicle_details_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::VehicleDetails,
        "Detalles",
        TITLE_CRIMSON,
        Vec2::new(440.0, 280.0),
        380.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_details_tab(row, asset_server, VehicleDetailsTab::Info, "Info");
                spawn_details_tab(row, asset_server, VehicleDetailsTab::Cargo, "Carga");
                spawn_details_tab(row, asset_server, VehicleDetailsTab::Capacity, "Capacidad");
                spawn_details_tab(row, asset_server, VehicleDetailsTab::Totals, "Totales");
                spawn_details_action(row, asset_server, VehicleDetailsAction::Refit, "Refit");
            });
        panel.spawn((
            VehicleDetailsSummaryText,
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
                    spawn_details_unit_row(list, asset_server, unit_idx);
                }
            },
            ROW_HEIGHT * LIST_VISIBLE_ROWS as f32 + 4.0,
        );
    });
}

fn spawn_details_unit_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    unit_idx: usize,
) {
    parent
        .spawn((
            VehicleDetailsUnitRow { unit_idx },
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
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
        });
}

fn spawn_details_tab(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    tab: VehicleDetailsTab,
    label: &'static str,
) {
    parent.spawn((
        Button,
        VehicleDetailsTabButton(tab),
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
    action: VehicleDetailsAction,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
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
        (&Interaction, &VehicleDetailsTabButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<VehicleDetailsAction>,
        ),
    >,
    mut action_buttons: Query<
        (&Interaction, &VehicleDetailsAction),
        (
            Changed<Interaction>,
            With<Button>,
            Without<VehicleDetailsTabButton>,
        ),
    >,
    mut details_state: ResMut<VehicleDetailsWindowState>,
    mut refit_window: ResMut<RefitWindowState>,
) {
    for (interaction, tab) in &mut tab_buttons {
        if *interaction == Interaction::Pressed {
            details_state.details_tab = tab.0;
        }
    }
    for (interaction, action) in &mut action_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            VehicleDetailsAction::Refit => {
                if let Some(vehicle_id) = details_state.vehicle_id {
                    refit_window.open_for(vehicle_id);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // sistema ECS Bevy
pub(crate) fn sync_vehicle_details_window(
    details_state: Res<VehicleDetailsWindowState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut summary_q: Query<
        &mut Text,
        (
            With<VehicleDetailsSummaryText>,
            Without<FloatingWindowTitleText>,
            Without<VehicleDetailsUnitText>,
        ),
    >,
    mut row_q: Query<(&VehicleDetailsUnitRow, &mut Node), Without<VehicleDetailsUnitSprite>>,
    mut sprite_q: Query<
        (&VehicleDetailsUnitSprite, &mut ImageNode, &mut Node),
        Without<VehicleDetailsUnitRow>,
    >,
    mut unit_text_q: Query<
        (&VehicleDetailsUnitText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<VehicleDetailsSummaryText>,
        ),
    >,
    mut tab_buttons: Query<
        (&VehicleDetailsTabButton, &Interaction, &mut BackgroundColor),
        (With<Button>, Without<VehicleDetailsAction>),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::VehicleDetails)
    else {
        return;
    };

    let vehicle = details_state
        .vehicle_id
        .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id).cloned());
    let Some(vehicle) = vehicle else {
        *vis = Visibility::Hidden;
        for (_, mut node) in &mut row_q {
            node.display = Display::None;
        }
        return;
    };
    *vis = Visibility::Visible;

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::VehicleDetails)
    {
        **title = format!("Detalles — {}", vehicle.display_name());
    }
    if let Ok(mut summary) = summary_q.single_mut() {
        **summary = vehicle_details_summary(&vehicle, &sim, details_state.details_tab);
    }

    let unit_ids = details_unit_ids(&vehicle, &sim);
    for (row, mut node) in &mut row_q {
        node.display = if unit_ids.get(row.unit_idx).is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (unit_text, mut text) in &mut unit_text_q {
        if let Some(&unit_id) = unit_ids.get(unit_text.unit_idx)
            && let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id)
        {
            **text = vehicle_details_unit_line(unit, &vehicle, &sim, details_state.details_tab);
        } else {
            **text = String::new();
        }
    }
    if let Some(trucks) = trucks.as_ref() {
        for (sprite, mut image, mut node) in &mut sprite_q {
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
        for (_, _, mut node) in &mut sprite_q {
            node.display = Display::None;
        }
    }

    for (tab, interaction, mut bg) in &mut tab_buttons {
        *bg = if tab.0 == details_state.details_tab {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
}

pub(crate) fn vehicle_details_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut details_state: ResMut<VehicleDetailsWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::VehicleDetails {
            details_state.vehicle_id = None;
            details_state.details_tab = VehicleDetailsTab::Info;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::VehicleDetailsWindowState;

    #[test]
    fn open_for_sets_vehicle_id() {
        let mut state = VehicleDetailsWindowState::default();
        state.open_for(7);
        assert_eq!(state.vehicle_id, Some(7));
    }
}
