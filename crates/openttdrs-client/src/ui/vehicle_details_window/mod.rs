//! Ventana de detalles del vehículo (`VehicleDetailsWindow` OpenTTD / #173).
//!
//! Se abre desde el botón «Detalles» de la vista de vehículo. Contiene las
//! pestañas Info / Carga / Capacidad / Totales y el cuerpo de texto.

mod details;

use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

pub(crate) use details::{speed_to_kmh, vehicle_details_body};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);

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

#[derive(Component)]
pub(crate) struct VehicleDetailsBodyText;

pub(crate) fn setup_vehicle_details_window(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::VehicleDetails,
        "Detalles",
        TITLE_CRIMSON,
        Vec2::new(420.0, 220.0),
        360.0,
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
            });
        panel.spawn((
            VehicleDetailsBodyText,
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

pub(crate) fn handle_vehicle_details_buttons(
    mut tab_buttons: Query<
        (&Interaction, &VehicleDetailsTabButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut details_state: ResMut<VehicleDetailsWindowState>,
) {
    for (interaction, tab) in &mut tab_buttons {
        if *interaction == Interaction::Pressed {
            details_state.details_tab = tab.0;
        }
    }
}

pub(crate) fn sync_vehicle_details_window(
    details_state: Res<VehicleDetailsWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut body_q: Query<
        &mut Text,
        (With<VehicleDetailsBodyText>, Without<FloatingWindowTitleText>),
    >,
    mut tab_buttons: Query<
        (&VehicleDetailsTabButton, &Interaction, &mut BackgroundColor),
        With<Button>,
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
        .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
    let Some(vehicle) = vehicle else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::VehicleDetails)
    {
        **title = format!("Detalles — {}", vehicle.display_name());
    }
    if let Ok(mut body) = body_q.single_mut() {
        **body = vehicle_details_body(vehicle, &sim, details_state.details_tab);
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
