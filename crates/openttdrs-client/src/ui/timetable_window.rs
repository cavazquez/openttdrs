//! Ventana flotante de horario por orden (Sprint F4).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{cycle_travel_ticks, cycle_wait_ticks};

use crate::i18n::{Locale, text as localized};
use crate::render::RemapMapVisualsPending;
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, WindowKey, spawn_floating_window_keyed, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::vehicle_chain::{
    MAX_VEHICLE_CHAIN_SLOTS, VehicleChainRegistry, VehicleChainSlot, vehicle_window_key,
};

const TIMETABLE_ROWS: usize = 8;
const BASE_POS: Vec2 = Vec2::new(420.0, 220.0);
const SLOT_OFFSET: Vec2 = Vec2::new(36.0, 36.0);
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Debug)]
pub(crate) struct TimetableWindowState {
    pub(crate) slots: [Option<u32>; MAX_VEHICLE_CHAIN_SLOTS],
    pub(crate) focused: Option<u32>,
}

impl Default for TimetableWindowState {
    fn default() -> Self {
        Self {
            slots: [None; MAX_VEHICLE_CHAIN_SLOTS],
            focused: None,
        }
    }
}

impl TimetableWindowState {
    #[must_use]
    #[allow(dead_code)] // API multi-slot (#244); handlers usan slots[idx] directo.
    pub(crate) fn vehicle_id(&self) -> Option<u32> {
        self.focused.filter(|&id| self.slots.contains(&Some(id)))
    }

    pub(crate) fn close_vehicle(&mut self, vehicle_id: u32) {
        for slot in &mut self.slots {
            if *slot == Some(vehicle_id) {
                *slot = None;
            }
        }
        if self.focused == Some(vehicle_id) {
            self.focused = self.slots.iter().flatten().next().copied();
        }
    }
}

/// Overrides de presentación de horario que pertenecen sólo a este cliente.
///
/// Los SAV/JSON previos pueden traer `Vehicle::timetable_display_seconds`.
/// Ese valor se usa como default de compatibilidad hasta que el jugador cambie
/// la vista; el override nunca se escribe de vuelta a `GameState` ni viaja por
/// el protocolo lockstep.
#[derive(Resource, Debug, Default)]
pub(crate) struct TimetableDisplayPrefs {
    seconds_by_vehicle: HashMap<u32, bool>,
}

impl TimetableDisplayPrefs {
    #[must_use]
    pub(crate) fn seconds_for(&self, vehicle: &Vehicle) -> bool {
        self.seconds_by_vehicle
            .get(&vehicle.id)
            .copied()
            .unwrap_or(vehicle.timetable_display_seconds)
    }

    pub(crate) fn toggle_for(&mut self, vehicle: &Vehicle) {
        let next = !self.seconds_for(vehicle);
        self.seconds_by_vehicle.insert(vehicle.id, next);
    }
}

#[derive(Component)]
pub(crate) struct TimetableSummaryText;

/// Contenedor de una fila de orden (no confundir con botones de la fila).
#[derive(Component, Clone, Copy)]
pub(crate) struct TimetableOrderRowStrip {
    index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TimetableOrderRowLabel {
    index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TimetableRowAction {
    index: usize,
    kind: TimetableRowButton,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum TimetableWindowButton {
    ToggleTimetable,
    ToggleAutofill,
    ClearLateness,
    ToggleSeconds,
    Close,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum TimetableRowButton {
    Wait,
    Travel,
}

pub(crate) fn setup_timetable_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    for slot in 0..MAX_VEHICLE_CHAIN_SLOTS {
        let slot_u8 = slot as u8;
        let pos = BASE_POS + SLOT_OFFSET * slot as f32;
        let (root, content) = spawn_floating_window_keyed(
            &mut commands,
            asset_server,
            WindowKey {
                class: FloatingWindowId::Timetable,
                instance: 0,
            },
            // El título incluye un nombre de vehículo y se materializa en el
            // sync. No registrarlo como clave estática del catálogo.
            "",
            TITLE_CRIMSON,
            pos,
            420.0,
        );
        commands.entity(root).insert(VehicleChainSlot(slot_u8));
        spawn_timetable_content(&mut commands, content, asset_server, slot_u8);
    }
}

fn spawn_timetable_content(
    commands: &mut Commands,
    content: Entity,
    asset_server: &AssetServer,
    chain_slot: u8,
) {
    let chain = VehicleChainSlot(chain_slot);
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            TimetableSummaryText,
            chain,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|list| {
                for index in 0..TIMETABLE_ROWS {
                    spawn_timetable_row(list, asset_server, chain, index);
                }
            });
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ToggleTimetable,
                    "Horario ON/OFF",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ToggleAutofill,
                    "Autorrelleno",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ClearLateness,
                    "Poner en hora",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ToggleSeconds,
                    "Ticks/Seg",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::Close,
                    "Cerrar",
                );
            });
    });
}

fn spawn_timetable_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    index: usize,
) {
    parent
        .spawn((
            TimetableOrderRowStrip { index },
            chain_slot,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                TimetableOrderRowLabel { index },
                chain_slot,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            spawn_row_btn(
                row,
                asset_server,
                chain_slot,
                index,
                TimetableRowButton::Wait,
                "Espera",
                72.0,
            );
            spawn_row_btn(
                row,
                asset_server,
                chain_slot,
                index,
                TimetableRowButton::Travel,
                "Viaje",
                72.0,
            );
        });
}

fn spawn_row_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    index: usize,
    kind: TimetableRowButton,
    label: &'static str,
    width: f32,
) {
    parent.spawn((
        Button,
        TimetableRowAction { index, kind },
        chain_slot,
        Node {
            width: Val::Px(width),
            height: Val::Px(22.0),
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

fn spawn_tt_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    action: TimetableWindowButton,
    label: &str,
) {
    parent.spawn((
        Button,
        action,
        chain_slot,
        Node {
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(8.0)),
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
            Text::new(label.to_string()),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn format_ticks(ticks: u32, seconds_mode: bool) -> String {
    if seconds_mode {
        format!(
            "{:.0}s",
            ticks as f32 / openttdrs_core::SIM_TICKS_PER_SECOND as f32
        )
    } else {
        format!("{ticks}t")
    }
}

fn timetable_title(
    locale: Locale,
    vehicle: &Vehicle,
    catalog: &[openttdrs_core::EngineDef],
) -> String {
    format!(
        "{} — {}",
        localized(locale, "Horario"),
        vehicle.display_name_with_catalog(catalog)
    )
}

fn timetable_summary(locale: Locale, vehicle: &Vehicle) -> String {
    let late = vehicle.timetable_lateness;
    let late_label = if late > 0 {
        format!("+{late}t {}", localized(locale, "tarde"))
    } else if late < 0 {
        format!("{late}t {}", localized(locale, "adelantado"))
    } else {
        localized(locale, "en hora").to_owned()
    };
    format!(
        "{}: {} · {}: {} · {late_label}",
        localized(locale, "Horario"),
        if vehicle.timetable_active {
            "ON"
        } else {
            "OFF"
        },
        localized(locale, "Autorrelleno"),
        if vehicle.timetable_autofill {
            "ON"
        } else {
            "OFF"
        },
    )
}

fn order_timing_label(locale: Locale, order: VehicleOrder, seconds_mode: bool) -> String {
    match order {
        VehicleOrder::Station {
            wait_ticks,
            travel_ticks,
            ..
        }
        | VehicleOrder::Depot {
            wait_ticks,
            travel_ticks,
            ..
        } => {
            format!(
                "{}{} {} {}",
                localized(locale, "esp."),
                format_ticks(wait_ticks, seconds_mode),
                localized(locale, "viaje"),
                format_ticks(travel_ticks, seconds_mode)
            )
        }
        VehicleOrder::Waypoint { travel_ticks, .. } => {
            format!(
                "{} {}",
                localized(locale, "viaje"),
                format_ticks(travel_ticks, seconds_mode)
            )
        }
        VehicleOrder::Conditional { .. } => localized(locale, "condicional").to_owned(),
        VehicleOrder::Tile(_) => "—".into(),
    }
}

pub(crate) fn open_timetable_for_vehicle(
    state: &mut TimetableWindowState,
    chain: &VehicleChainRegistry,
    vehicle_id: u32,
) {
    let Some(slot) = chain.slot_of(vehicle_id) else {
        return;
    };
    state.slots[slot as usize] = Some(vehicle_id);
    state.focused = Some(vehicle_id);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_timetable_window(
    tt_state: Res<TimetableWindowState>,
    display_prefs: Res<TimetableDisplayPrefs>,
    chain: Res<VehicleChainRegistry>,
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
    mut root_q: Query<(
        Entity,
        &mut FloatingWindow,
        &VehicleChainSlot,
        &mut Visibility,
    )>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut summary_q: Query<
        (&VehicleChainSlot, &mut Text),
        (With<TimetableSummaryText>, Without<FloatingWindowTitleText>),
    >,
    mut row_strip_q: Query<
        (&VehicleChainSlot, &TimetableOrderRowStrip, &mut Node),
        (Without<Button>, Without<TimetableOrderRowLabel>),
    >,
    mut row_label_q: Query<
        (&VehicleChainSlot, &TimetableOrderRowLabel, &mut Text),
        (
            Without<TimetableSummaryText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
) {
    // No hay caché de textos: cada slot vuelve a leer el locale activo junto
    // con los tiempos actuales, también al reabrir la ventana.
    let locale = prefs.locale();
    fn title_root_entity(child_of: &ChildOf, parents: &Query<&ChildOf>) -> Option<Entity> {
        let center = child_of.parent();
        let bar = parents.get(center).ok()?.parent();
        parents.get(bar).ok().map(ChildOf::parent)
    }

    for (root_entity, mut win, slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::Timetable {
            continue;
        }
        let idx = slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        let vehicle_id = tt_state.slots[idx].filter(|&id| chain.slot_of(id) == Some(slot.0));
        win.key = vehicle_window_key(FloatingWindowId::Timetable, vehicle_id.unwrap_or(0));
        let Some(vehicle_id) = vehicle_id else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Visible;
        let title_name = timetable_title(locale, vehicle, &sim.state.engine_catalog);
        for (title, mut text, child_of) in &mut title_q {
            if title.0 != FloatingWindowId::Timetable {
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
            **summary = timetable_summary(locale, vehicle);
        }
        let seconds_mode = display_prefs.seconds_for(vehicle);
        for (strip_slot, strip, mut node) in &mut row_strip_q {
            if strip_slot.0 != slot.0 {
                continue;
            }
            node.display = if strip.index < vehicle.orders.len() {
                Display::Flex
            } else {
                Display::None
            };
        }
        for (label_slot, label, mut text) in &mut row_label_q {
            if label_slot.0 != slot.0 {
                continue;
            }
            if label.index < vehicle.orders.len() {
                **text = format!(
                    "{}. {}",
                    label.index + 1,
                    order_timing_label(locale, vehicle.orders[label.index], seconds_mode)
                );
            } else {
                **text = String::new();
            }
        }
    }
}

pub(crate) fn timetable_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tt_state: ResMut<TimetableWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::Timetable {
            continue;
        }
        let vehicle_id = msg.0.instance;
        if vehicle_id == 0 {
            continue;
        }
        tt_state.close_vehicle(vehicle_id);
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS: dos queries y recursos de UI/sim.
pub(crate) fn handle_timetable_window_buttons(
    mut btn_q: Query<
        (&Interaction, &TimetableWindowButton, &VehicleChainSlot),
        (Changed<Interaction>, With<Button>),
    >,
    mut row_btn_q: Query<
        (&Interaction, &TimetableRowAction, &VehicleChainSlot),
        (
            Changed<Interaction>,
            With<Button>,
            Without<TimetableWindowButton>,
        ),
    >,
    mut tt_state: ResMut<TimetableWindowState>,
    mut display_prefs: ResMut<TimetableDisplayPrefs>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, action, chain_slot) in &mut row_btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        let Some(vehicle_id) = tt_state.slots.get(idx).copied().flatten() else {
            continue;
        };
        tt_state.focused = Some(vehicle_id);
        let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
            continue;
        };
        if action.index >= vehicle.orders.len() {
            continue;
        }
        let order = vehicle.orders[action.index];
        let cmd = match action.kind {
            TimetableRowButton::Wait => {
                let next = cycle_wait_ticks(order.wait_ticks());
                Command::SetVehicleOrderWaitTicks {
                    vehicle_id,
                    index: action.index,
                    wait_ticks: next,
                }
            }
            TimetableRowButton::Travel => {
                let next = cycle_travel_ticks(order.travel_ticks());
                Command::SetVehicleOrderTravelTicks {
                    vehicle_id,
                    index: action.index,
                    travel_ticks: next,
                }
            }
        };
        match crate::network::apply_player_command(&mut sim.state, &cmd) {
            Ok(()) => pending.request_full(),
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }

    for (interaction, button, chain_slot) in &mut btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        let Some(vehicle_id) = tt_state.slots.get(idx).copied().flatten() else {
            continue;
        };
        tt_state.focused = Some(vehicle_id);
        match button {
            TimetableWindowButton::ToggleTimetable => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleVehicleTimetable(vehicle_id),
                );
                pending.request_full();
            }
            TimetableWindowButton::ToggleAutofill => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleVehicleTimetableAutofill(vehicle_id),
                );
                pending.request_full();
            }
            TimetableWindowButton::ClearLateness => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ClearVehicleTimetableLateness(vehicle_id),
                );
                pending.request_full();
            }
            TimetableWindowButton::ToggleSeconds => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    display_prefs.toggle_for(vehicle);
                }
            }
            TimetableWindowButton::Close => {
                tt_state.close_vehicle(vehicle_id);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::i18n::LocalizationPlugin;
    use crate::render::RemapMapVisualsPending;
    use crate::ui::hud::HudBuildFeedback;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::OrderConditionKind;

    #[test]
    fn timetable_labels_cover_order_types_and_signed_lateness() {
        let pos = TileCoord::new(2, 3);
        for order in [VehicleOrder::station(pos), VehicleOrder::depot(pos)] {
            let order = order.with_wait_ticks(30).unwrap().with_travel_ticks(60);
            assert_eq!(
                order_timing_label(Locale::Es, order, false),
                "esp.30t viaje 60t"
            );
            assert_eq!(
                order_timing_label(Locale::En, order, false),
                "wait 30t travel 60t"
            );
            assert_eq!(
                order_timing_label(Locale::En, order, true),
                "wait 1s travel 2s"
            );
        }
        let waypoint = VehicleOrder::waypoint(pos).with_travel_ticks(60);
        assert_eq!(
            order_timing_label(Locale::En, waypoint, false),
            "travel 60t"
        );
        assert_eq!(order_timing_label(Locale::Es, waypoint, true), "viaje 2s");
        let conditional = VehicleOrder::conditional(OrderConditionKind::CargoLoadAbove, 50, 1);
        assert_eq!(
            order_timing_label(Locale::En, conditional, false),
            "conditional"
        );
        assert_eq!(
            order_timing_label(Locale::Es, conditional, true),
            "condicional"
        );
        assert_eq!(
            order_timing_label(Locale::En, VehicleOrder::tile(pos), false),
            "—"
        );

        let mut vehicle = Vehicle::new(7, VehicleKind::Train, pos, pos);
        for (lateness, spanish, english) in [
            (0, "en hora", "on time"),
            (42, "+42t tarde", "+42t late"),
            (-42, "-42t adelantado", "-42t early"),
            (i32::MIN, "-2147483648t adelantado", "-2147483648t early"),
        ] {
            vehicle.timetable_lateness = lateness;
            assert_eq!(
                timetable_summary(Locale::Es, &vehicle),
                format!("Horario: OFF · Autorrelleno: OFF · {spanish}")
            );
            assert_eq!(
                timetable_summary(Locale::En, &vehicle),
                format!("Timetable: OFF · Autofill: OFF · {english}")
            );
        }
    }

    #[test]
    // Mantener juntos fixture, aplicación ECS y aserciones de ambos slots
    // permite auditar que el cambio de locale no modifica los vehículos.
    #[allow(clippy::too_many_lines)]
    fn timetable_windows_follow_locale_in_both_slots_without_mutating_vehicles() {
        let pos = TileCoord::new(2, 3);
        let mut game = GameState::new(8, 8);
        let mut first = Vehicle::new(42, VehicleKind::Train, pos, pos);
        first.name = Some("Horario".into());
        first.orders = vec![
            VehicleOrder::station(pos)
                .with_wait_ticks(30)
                .unwrap()
                .with_travel_ticks(60),
        ];
        first.timetable_active = true;
        first.timetable_lateness = 7;
        let mut second = Vehicle::new(99, VehicleKind::Bus, pos, pos);
        second.name = Some("Ñandú | Custom 99".into());
        second.orders = vec![VehicleOrder::waypoint(pos).with_travel_ticks(60)];
        second.timetable_autofill = true;
        second.timetable_display_seconds = true;
        second.timetable_lateness = -3;
        game.vehicles = vec![first, second];
        let vehicles_before = serde_json::to_value(&game.vehicles).unwrap();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin {
                file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../..").into(),
                ..default()
            })
            .init_asset::<Image>()
            .init_asset::<Font>()
            .insert_resource(ClientPreferences::default())
            .insert_resource(SimWorld {
                state: game,
                ..SimWorld::default()
            })
            .insert_resource(VehicleChainRegistry {
                slots: [Some(42), Some(99)],
                focused: Some(99),
            })
            .insert_resource(TimetableWindowState {
                slots: [Some(42), Some(99)],
                focused: Some(99),
            })
            .init_resource::<TimetableDisplayPrefs>()
            .add_plugins(LocalizationPlugin)
            .add_systems(Startup, setup_timetable_window)
            .add_systems(Update, sync_timetable_window);

        for language in ["es", "en", "es"] {
            app.world_mut().resource_mut::<ClientPreferences>().language = language.into();
            // Más de un frame detecta también que el catálogo estático no
            // recupere el caption sin nombre sobre el título ya sincronizado.
            app.update();
            app.update();
            let world = app.world_mut();
            let mut texts = world.query::<&Text>();
            let texts = texts
                .iter(world)
                .map(|text| text.as_str())
                .collect::<Vec<_>>();
            let expected: &[&str] = if language == "en" {
                &[
                    "Timetable — Horario",
                    "Timetable — Ñandú | Custom 99",
                    "Timetable: ON · Autofill: OFF · +7t late",
                    "Timetable: OFF · Autofill: ON · -3t early",
                    "Timetable ON/OFF",
                    "Autofill",
                    "Reset lateness",
                    "Ticks/Sec",
                    "Travel",
                    "Wait",
                    "Close",
                ]
            } else {
                &[
                    "Horario — Horario",
                    "Horario — Ñandú | Custom 99",
                    "Horario: ON · Autorrelleno: OFF · +7t tarde",
                    "Horario: OFF · Autorrelleno: ON · -3t adelantado",
                    "Horario ON/OFF",
                    "Autorrelleno",
                    "Poner en hora",
                    "Ticks/Seg",
                    "Viaje",
                    "Espera",
                    "Cerrar",
                ]
            };
            for expected in expected {
                assert!(texts.contains(expected), "{language}: falta {expected:?}");
            }
            let mut rows = world.query::<(&VehicleChainSlot, &TimetableOrderRowLabel, &Text)>();
            for (slot, row, text) in rows.iter(world) {
                let expected = match (row.index, slot.0, language) {
                    (0, 0, "en") => "1. wait 30t travel 60t",
                    (0, 0, _) => "1. esp.30t viaje 60t",
                    (0, 1, "en") => "1. travel 2s",
                    (0, 1, _) => "1. viaje 2s",
                    _ => "",
                };
                assert_eq!(text.as_str(), expected);
            }
            let mut windows = world.query::<(&FloatingWindow, &VehicleChainSlot, &Visibility)>();
            for (window, slot, visibility) in windows.iter(world) {
                assert_eq!(window.key.instance, [42, 99][slot.0 as usize]);
                assert_eq!(*visibility, Visibility::Visible);
            }
            assert_eq!(world.resource::<TimetableWindowState>().focused, Some(99));
            assert_eq!(world.resource::<VehicleChainRegistry>().focused, Some(99));
            assert_eq!(
                serde_json::to_value(&world.resource::<SimWorld>().state.vehicles).unwrap(),
                vehicles_before
            );
        }
    }

    #[test]
    fn focused_vehicle_is_validated_against_open_slots() {
        let mut state = TimetableWindowState::default();
        state.slots[0] = Some(42);
        state.focused = Some(42);
        assert_eq!(state.vehicle_id(), Some(42));

        state.focused = Some(99);
        assert_eq!(state.vehicle_id(), None);
    }

    #[test]
    fn closing_vehicle_selects_next_open_slot() {
        let mut state = TimetableWindowState::default();
        state.slots[0] = Some(42);
        state.slots[1] = Some(99);
        state.focused = Some(42);

        state.close_vehicle(42);

        assert_eq!(state.slots[0], None);
        assert_eq!(state.focused, Some(99));
    }

    #[test]
    fn timetable_seconds_toggle_is_local_and_legacy_default_survives_save_load() {
        let pos = TileCoord::new(2, 3);

        // Una partida anterior conserva su dato serializado y el cliente lo
        // toma como default hasta que el usuario elige un override local.
        let mut legacy = GameState::new(8, 8);
        let mut legacy_vehicle = Vehicle::new(42, VehicleKind::Train, pos, pos);
        legacy_vehicle.timetable_display_seconds = true;
        legacy.vehicles.push(legacy_vehicle);
        let loaded = GameState::load_json(&legacy.save_json().unwrap()).unwrap();
        let loaded_vehicle = &loaded.vehicles[0];
        assert!(loaded_vehicle.timetable_display_seconds);
        let mut legacy_prefs = TimetableDisplayPrefs::default();
        assert!(legacy_prefs.seconds_for(loaded_vehicle));
        legacy_prefs.toggle_for(loaded_vehicle);
        assert!(!legacy_prefs.seconds_for(loaded_vehicle));
        assert!(loaded_vehicle.timetable_display_seconds);

        let mut state = GameState::new(8, 8);
        state
            .vehicles
            .push(Vehicle::new(7, VehicleKind::Train, pos, pos));
        let hash_before = state.canonical_hash();
        let mut slots = [None; MAX_VEHICLE_CHAIN_SLOTS];
        slots[0] = Some(7);

        let mut world = World::new();
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.insert_resource(TimetableWindowState {
            slots,
            focused: Some(7),
        });
        world.init_resource::<TimetableDisplayPrefs>();
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.spawn((
            Button,
            TimetableWindowButton::ToggleSeconds,
            VehicleChainSlot(0),
            Interaction::Pressed,
        ));

        world
            .run_system_once(handle_timetable_window_buttons)
            .unwrap();

        let sim = world.resource::<SimWorld>();
        let vehicle = sim
            .state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == 7)
            .unwrap();
        assert!(!vehicle.timetable_display_seconds);
        assert_eq!(sim.state.canonical_hash(), hash_before);
        assert!(
            world
                .resource::<TimetableDisplayPrefs>()
                .seconds_for(vehicle)
        );
        assert!(!world.resource::<RemapMapVisualsPending>().is_pending());
    }
}
