//! Ventana de reglas de autoreemplazo de motores.

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    AutoReplaceRule, DepotPurchaseKind, EngineCatalogSort, RoadEngineFilter, calendar_year_at_tick,
    engine_by_id, engines_for_depot_kind,
};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

const RULE_ROWS: usize = 10;
const ENGINE_ROWS: usize = 14;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct AutoreplaceWindowState {
    pub(crate) open: bool,
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) from_engine: Option<u16>,
    pub(crate) to_engine: Option<u16>,
    pub(crate) selected_rule_from: Option<u16>,
}

impl AutoreplaceWindowState {
    pub(crate) fn open_for_depot(&mut self, depot_pos: TileCoord) {
        self.open = true;
        self.depot_pos = Some(depot_pos);
    }
}

#[derive(Component)]
pub(crate) struct AutoreplaceHintText;

#[derive(Component, Clone, Copy)]
pub(crate) struct AutoreplaceRuleRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct AutoreplaceRuleRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct AutoreplaceFromRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct AutoreplaceFromRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct AutoreplaceToRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct AutoreplaceToRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum AutoreplaceButton {
    AddRule,
    ToggleRule,
    ToggleOnlyWhenOld,
    ClearRule,
    MassReplace,
}

pub(crate) fn setup_autoreplace_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Autoreplace,
        "Autoreemplazo",
        TITLE_BROWN,
        Vec2::new(480.0, 80.0),
        420.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            AutoreplaceHintText,
            Text::new("Reglas de autoreemplazo."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        spawn_labeled_list(panel, asset_server, "Reglas", true);
        spawn_engine_pickers(panel, asset_server);
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_action(row, asset_server, AutoreplaceButton::AddRule, "Añadir");
                spawn_action(row, asset_server, AutoreplaceButton::ToggleRule, "On/Off");
                spawn_action(
                    row,
                    asset_server,
                    AutoreplaceButton::ToggleOnlyWhenOld,
                    "Solo viejos",
                );
                spawn_action(row, asset_server, AutoreplaceButton::ClearRule, "Borrar");
                spawn_action(
                    row,
                    asset_server,
                    AutoreplaceButton::MassReplace,
                    "Aplicar depósito",
                );
            });
    });
}

fn spawn_labeled_list(
    panel: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    rules: bool,
) {
    panel.spawn((
        Text::new(label),
        window_text_font(asset_server, UiFontRole::Caption),
        TextColor(WINDOW_TEXT),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        },
    ));
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            max_height: Val::Px(120.0),
            overflow: Overflow::scroll_y(),
            ..default()
        })
        .with_children(|list| {
            if rules {
                for slot in 0..RULE_ROWS {
                    list.spawn((
                        Button,
                        AutoreplaceRuleRow { slot },
                        row_style(),
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            AutoreplaceRuleRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            }
        });
}

fn spawn_engine_pickers(panel: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        })
        .with_children(|cols| {
            spawn_engine_column(cols, asset_server, true);
            spawn_engine_column(cols, asset_server, false);
        });
}

fn spawn_engine_column(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    is_from: bool,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(if is_from { "Desde" } else { "Hacia" }),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            col.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                max_height: Val::Px(140.0),
                overflow: Overflow::scroll_y(),
                ..default()
            })
            .with_children(|list| {
                for slot in 0..ENGINE_ROWS {
                    if is_from {
                        list.spawn((
                            Button,
                            AutoreplaceFromRow { slot },
                            row_style(),
                            BackgroundColor(BTN_BG),
                            BorderColor::all(BTN_BORDER),
                            Interaction::default(),
                            BuildMenuUi,
                            children![(
                                AutoreplaceFromRowText { slot },
                                Text::new(""),
                                window_text_font(asset_server, UiFontRole::Caption),
                                TextColor(WINDOW_TEXT),
                            )],
                        ));
                    } else {
                        list.spawn((
                            Button,
                            AutoreplaceToRow { slot },
                            row_style(),
                            BackgroundColor(BTN_BG),
                            BorderColor::all(BTN_BORDER),
                            Interaction::default(),
                            BuildMenuUi,
                            children![(
                                AutoreplaceToRowText { slot },
                                Text::new(""),
                                window_text_font(asset_server, UiFontRole::Caption),
                                TextColor(WINDOW_TEXT),
                            )],
                        ));
                    }
                }
            });
        });
}

fn row_style() -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Px(22.0),
        padding: UiRect::horizontal(Val::Px(4.0)),
        justify_content: JustifyContent::FlexStart,
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        display: Display::None,
        ..default()
    }
}

fn spawn_action(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: AutoreplaceButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            min_width: Val::Px(88.0),
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

fn depot_engines(sim: &SimWorld, depot_pos: TileCoord) -> Vec<u16> {
    let depot_kind = match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => DepotPurchaseKind::Rail,
        Some(TileKind::ShipDepot) => DepotPurchaseKind::Ship,
        Some(TileKind::Airport) => DepotPurchaseKind::Aircraft,
        _ => DepotPurchaseKind::Road,
    };
    let year = calendar_year_at_tick(sim.state.tick);
    engines_for_depot_kind(
        depot_kind,
        year,
        EngineCatalogSort::Name,
        RoadEngineFilter::All,
    )
    .into_iter()
    .filter(|e| !e.is_wagon())
    .map(|e| e.id)
    .collect()
}

fn rule_label(rule: &AutoReplaceRule) -> String {
    let from = engine_by_id(rule.from_engine_id)
        .map(|e| e.name.as_str())
        .unwrap_or("?");
    let to = engine_by_id(rule.to_engine_id)
        .map(|e| e.name.as_str())
        .unwrap_or("?");
    let flags = match (rule.enabled, rule.only_when_old) {
        (true, true) => "on · viejos",
        (true, false) => "on",
        (false, true) => "off · viejos",
        (false, false) => "off",
    };
    format!("{from} → {to} ({flags})")
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn sync_autoreplace_window(
    state: Res<AutoreplaceWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (
            Without<AutoreplaceHintText>,
            Without<AutoreplaceRuleRowText>,
            Without<AutoreplaceFromRowText>,
            Without<AutoreplaceToRowText>,
        ),
    >,
    mut hint_q: Query<
        &mut Text,
        (
            With<AutoreplaceHintText>,
            Without<FloatingWindowTitleText>,
            Without<AutoreplaceRuleRowText>,
            Without<AutoreplaceFromRowText>,
            Without<AutoreplaceToRowText>,
        ),
    >,
    mut rule_rows: Query<(
        &AutoreplaceRuleRow,
        &Interaction,
        &mut Node,
        &mut BackgroundColor,
    )>,
    mut rule_texts: Query<
        (&AutoreplaceRuleRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<AutoreplaceHintText>,
            Without<AutoreplaceFromRowText>,
            Without<AutoreplaceToRowText>,
        ),
    >,
    mut from_rows: Query<
        (
            &AutoreplaceFromRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        Without<AutoreplaceRuleRow>,
    >,
    mut from_texts: Query<
        (&AutoreplaceFromRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<AutoreplaceHintText>,
            Without<AutoreplaceRuleRowText>,
            Without<AutoreplaceToRowText>,
        ),
    >,
    mut to_rows: Query<
        (
            &AutoreplaceToRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        (Without<AutoreplaceRuleRow>, Without<AutoreplaceFromRow>),
    >,
    mut to_texts: Query<
        (&AutoreplaceToRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<AutoreplaceHintText>,
            Without<AutoreplaceRuleRowText>,
            Without<AutoreplaceFromRowText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::Autoreplace)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(text, _)| text.0 == FloatingWindowId::Autoreplace)
    {
        **title = "Autoreemplazo".to_string();
    }
    if let Ok(mut hint) = hint_q.single_mut() {
        let from = state
            .from_engine
            .and_then(engine_by_id)
            .map(|e| e.name.as_str())
            .unwrap_or("—");
        let to = state
            .to_engine
            .and_then(engine_by_id)
            .map(|e| e.name.as_str())
            .unwrap_or("—");
        **hint = format!("Desde: {from} · Hacia: {to}");
    }

    let rules = &sim.state.autoreplace_rules;
    for (row, interaction, mut node, mut bg) in &mut rule_rows {
        let Some(rule) = rules.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = state.selected_rule_from == Some(rule.from_engine_id);
        *bg = if selected {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut rule_texts {
        if let Some(rule) = rules.get(row_text.slot) {
            **text = rule_label(rule);
        } else {
            **text = String::new();
        }
    }

    let engines = state
        .depot_pos
        .map(|pos| depot_engines(&sim, pos))
        .unwrap_or_default();
    for (row, interaction, mut node, mut bg) in &mut from_rows {
        let Some(&engine_id) = engines.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = state.from_engine == Some(engine_id);
        *bg = if selected {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut from_texts {
        if let Some(&engine_id) = engines.get(row_text.slot) {
            **text = engine_by_id(engine_id)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| format!("#{engine_id}"));
        } else {
            **text = String::new();
        }
    }
    for (row, interaction, mut node, mut bg) in &mut to_rows {
        let Some(&engine_id) = engines.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = state.to_engine == Some(engine_id);
        *bg = if selected {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut to_texts {
        if let Some(&engine_id) = engines.get(row_text.slot) {
            **text = engine_by_id(engine_id)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| format!("#{engine_id}"));
        } else {
            **text = String::new();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_autoreplace_buttons(
    mut rule_rows: Query<(&Interaction, &AutoreplaceRuleRow), (Changed<Interaction>, With<Button>)>,
    mut from_rows: Query<
        (&Interaction, &AutoreplaceFromRow),
        (
            Changed<Interaction>,
            With<Button>,
            Without<AutoreplaceRuleRow>,
        ),
    >,
    mut to_rows: Query<
        (&Interaction, &AutoreplaceToRow),
        (
            Changed<Interaction>,
            With<Button>,
            Without<AutoreplaceRuleRow>,
            Without<AutoreplaceFromRow>,
        ),
    >,
    mut buttons: Query<
        (&Interaction, &AutoreplaceButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<AutoreplaceRuleRow>,
            Without<AutoreplaceFromRow>,
            Without<AutoreplaceToRow>,
        ),
    >,
    mut state: ResMut<AutoreplaceWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !state.open {
        return;
    }
    let rules: Vec<u16> = sim
        .state
        .autoreplace_rules
        .iter()
        .map(|r| r.from_engine_id)
        .collect();
    let engines = state
        .depot_pos
        .map(|pos| depot_engines(&sim, pos))
        .unwrap_or_default();

    for (interaction, row) in &mut rule_rows {
        if *interaction == Interaction::Pressed
            && let Some(&from) = rules.get(row.slot)
        {
            state.selected_rule_from = Some(from);
            if let Some(rule) = sim
                .state
                .autoreplace_rules
                .iter()
                .find(|r| r.from_engine_id == from)
            {
                state.from_engine = Some(rule.from_engine_id);
                state.to_engine = Some(rule.to_engine_id);
            }
        }
    }
    for (interaction, row) in &mut from_rows {
        if *interaction == Interaction::Pressed
            && let Some(&id) = engines.get(row.slot)
        {
            state.from_engine = Some(id);
        }
    }
    for (interaction, row) in &mut to_rows {
        if *interaction == Interaction::Pressed
            && let Some(&id) = engines.get(row.slot)
        {
            state.to_engine = Some(id);
        }
    }
    for (interaction, button) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            AutoreplaceButton::AddRule => {
                let (Some(from), Some(to)) = (state.from_engine, state.to_engine) else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::SetAutoReplaceRule {
                        from_engine_id: from,
                        to_engine_id: to,
                    },
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        state.selected_rule_from = Some(from);
                    }
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            AutoreplaceButton::ToggleRule => {
                let Some(from) = state.selected_rule_from.or(state.from_engine) else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleAutoReplaceRule {
                        from_engine_id: from,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            AutoreplaceButton::ToggleOnlyWhenOld => {
                let Some(from) = state.selected_rule_from.or(state.from_engine) else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleAutoReplaceOnlyWhenOld {
                        from_engine_id: from,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            AutoreplaceButton::ClearRule => {
                let Some(from) = state.selected_rule_from.or(state.from_engine) else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ClearAutoReplaceRule {
                        from_engine_id: from,
                    },
                ) {
                    Ok(()) => {
                        pending.pending = true;
                        state.selected_rule_from = None;
                    }
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
            AutoreplaceButton::MassReplace => {
                let Some(depot_pos) = state.depot_pos else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::DepotMassAutoreplace { depot_pos },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
        }
    }
}

pub(crate) fn autoreplace_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<AutoreplaceWindowState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::Autoreplace {
            *state = AutoreplaceWindowState::default();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::ENGINE_TRUCK_MPS;

    #[test]
    fn add_rule_stores_autoreplace() {
        let mut world = World::new();
        world.insert_resource(SimWorld {
            state: GameState::new(8, 8),
            ..SimWorld::default()
        });
        // ENGINE_TRUCK_BALOGH_GOODS = 11 (mismo kind que MPS).
        const TO: u16 = 11;
        world.insert_resource(AutoreplaceWindowState {
            open: true,
            depot_pos: Some(TileCoord::new(1, 1)),
            from_engine: Some(ENGINE_TRUCK_MPS),
            to_engine: Some(TO),
            selected_rule_from: None,
        });
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.spawn((Button, AutoreplaceButton::AddRule, Interaction::Pressed));
        world.run_system_once(handle_autoreplace_buttons).unwrap();
        assert!(
            world
                .resource::<SimWorld>()
                .state
                .autoreplace_rules
                .iter()
                .any(|r| r.from_engine_id == ENGINE_TRUCK_MPS && r.to_engine_id == TO)
        );
    }
}
