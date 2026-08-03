//! Ventana Town Authority (`WC_TOWN_AUTHORITY`) — hija de Town.
//!
//! Presenta las ocho acciones de autoridad de OpenTTD 15.3 y ejecuta el
//! `Command::DoTownAction` determinista del dominio.

use bevy::prelude::*;
use openttdrs_core::{
    Command, TownAction, TownAuthoritySettings, format_money, mask_of_town_actions,
};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CREAM,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::town_window::TownWindowState;

#[derive(Resource, Default)]
pub(crate) struct TownAuthorityWindowState {
    pub(crate) open: bool,
    pub(crate) town_id: Option<u32>,
}

#[derive(Component)]
pub(crate) struct TownAuthorityBodyText;

#[derive(Component, Clone, Copy)]
pub(crate) struct TownAuthorityActionButton(pub(crate) TownAction);

#[derive(Component, Clone, Copy)]
pub(crate) struct TownAuthorityActionText(TownAction);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TownActionAvailability {
    Enabled,
    InsufficientFunds,
    NotAvailable,
}

const ACTION_ENABLED_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const ACTION_DISABLED_BG: Color = Color::srgb(0.19, 0.18, 0.16);
const ACTION_ENABLED_TEXT: Color = WINDOW_TEXT;
const ACTION_DISABLED_TEXT: Color = Color::srgb(0.52, 0.50, 0.45);

pub(crate) fn setup_town_authority_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::TownAuthority,
        "Autoridad local",
        TITLE_CREAM,
        Vec2::new(80.0, 140.0),
        300.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            TownAuthorityBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                margin: UiRect::top(Val::Px(7.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|actions| {
            for action in TownAction::all() {
                spawn_town_authority_action_button(actions, asset_server, action);
            }
        });
    });
}

fn spawn_town_authority_action_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: TownAction,
) {
    parent.spawn((
        Button,
        TownAuthorityActionButton(action),
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(22.0),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::horizontal(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(ACTION_ENABLED_BG),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            TownAuthorityActionText(action),
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(ACTION_ENABLED_TEXT),
        )],
    ));
}

pub(crate) fn open_town_authority_for(town_id: u32, state: &mut TownAuthorityWindowState) {
    state.open = true;
    state.town_id = Some(town_id);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_town_authority_window(
    state: Res<TownAuthorityWindowState>,
    town: Res<TownWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (
            Without<TownAuthorityBodyText>,
            Without<TownAuthorityActionText>,
        ),
    >,
    mut body_q: Query<
        &mut Text,
        (
            With<TownAuthorityBodyText>,
            (
                Without<FloatingWindowTitleText>,
                Without<TownAuthorityActionText>,
            ),
        ),
    >,
    mut action_button_q: Query<(&TownAuthorityActionButton, &mut BackgroundColor)>,
    mut action_text_q: Query<(&TownAuthorityActionText, &mut Text, &mut TextColor)>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::TownAuthority)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let town_id = state.town_id.or(town.town_id);
    let Some(t) = town_id.and_then(|id| sim.state.towns.iter().find(|x| x.id == id)) else {
        if let Ok(mut body) = body_q.single_mut() {
            **body = "Sin pueblo seleccionado.".into();
        }
        return;
    };

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(tt, _)| tt.0 == FloatingWindowId::TownAuthority)
    {
        **title = format!("Autoridad — {}", t.name);
    }

    let rating = t.authority_rating(sim.state.active_company);
    let ratings = authority_ratings_text(t, sim.state.companies.len());
    let settings = TownAuthoritySettings::default();
    let available_mask = mask_of_town_actions(
        t,
        sim.state.active_company,
        sim.state.economy.money,
        settings,
    );
    let unrestricted_mask = mask_of_town_actions(t, sim.state.active_company, i64::MAX, settings);
    for (button, mut bg) in &mut action_button_q {
        let status = town_action_availability(button.0, available_mask, unrestricted_mask);
        *bg = BackgroundColor(if status == TownActionAvailability::Enabled {
            ACTION_ENABLED_BG
        } else {
            ACTION_DISABLED_BG
        });
    }
    for (label, mut text, mut color) in &mut action_text_q {
        let status = town_action_availability(label.0, available_mask, unrestricted_mask);
        **text = format!(
            "{}  {} — {}",
            town_action_name(label.0),
            format_money(label.0.cost()),
            town_action_status_text(status),
        );
        *color = TextColor(if status == TownActionAvailability::Enabled {
            ACTION_ENABLED_TEXT
        } else {
            ACTION_DISABLED_TEXT
        });
    }
    if let Ok(mut body) = body_q.single_mut() {
        **body = format!(
            "Pueblo: {}\nDinero: {}\nRating compañía activa: {rating}\n\nRatings por compañía:\n{ratings}\n\nAcciones de autoridad:",
            t.name,
            format_money(sim.state.economy.money),
        );
    }
}

fn town_action_name(action: TownAction) -> &'static str {
    match action {
        TownAction::AdvertiseSmall => "Publicidad pequeña",
        TownAction::AdvertiseMedium => "Publicidad mediana",
        TownAction::AdvertiseLarge => "Publicidad grande",
        TownAction::RoadRebuild => "Reconstruir carreteras",
        TownAction::BuildStatue => "Construir estatua",
        TownAction::FundBuildings => "Financiar edificios",
        TownAction::BuyRights => "Comprar derechos exclusivos",
        TownAction::Bribe => "Sobornar autoridad",
    }
}

fn town_action_availability(
    action: TownAction,
    available_mask: u8,
    unrestricted_mask: u8,
) -> TownActionAvailability {
    let bit = 1 << action as u8;
    if available_mask & bit != 0 {
        TownActionAvailability::Enabled
    } else if unrestricted_mask & bit != 0 {
        TownActionAvailability::InsufficientFunds
    } else {
        TownActionAvailability::NotAvailable
    }
}

fn town_action_status_text(status: TownActionAvailability) -> &'static str {
    match status {
        TownActionAvailability::Enabled => "Disponible",
        TownActionAvailability::InsufficientFunds => "Sin fondos",
        TownActionAvailability::NotAvailable => "No disponible",
    }
}

fn authority_ratings_text(town: &openttdrs_core::Town, company_count: usize) -> String {
    let count = company_count.max(1).min(town.authority_ratings.len());
    (0..count)
        .map(|index| {
            format!(
                "  Compañía {}: {}",
                index + 1,
                town.authority_ratings[index]
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_town_authority_buttons(
    buttons: Query<
        (&Interaction, &TownAuthorityActionButton),
        (Changed<Interaction>, With<Button>),
    >,
    window: Res<TownAuthorityWindowState>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    let Some(town_id) = window.town_id else {
        return;
    };
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(town) = sim.state.towns.iter().find(|town| town.id == town_id) else {
            continue;
        };
        let enabled_mask = mask_of_town_actions(
            town,
            sim.state.active_company,
            sim.state.economy.money,
            TownAuthoritySettings::default(),
        );
        if enabled_mask & (1 << button.0 as u8) == 0 {
            continue;
        }
        if let Err(error) = crate::network::apply_player_command(
            &mut sim.state,
            &Command::DoTownAction {
                town_id,
                action: button.0,
            },
        ) {
            push_build_command_error(&mut hud_feedback, error, time.elapsed_secs());
        }
    }
}

pub(crate) fn town_authority_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<TownAuthorityWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::TownAuthority {
            state.open = false;
            state.town_id = None;
        }
        // Cascada: cerrar Town cierra Authority (#269 / matriz padre→hija).
        if msg.0.class == FloatingWindowId::Town {
            state.open = false;
            state.town_id = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{CompanyId, GameState, Town};

    fn state_with_town(money: i64) -> GameState {
        let mut state = GameState::new(16, 16);
        state.economy.money = money;
        state.towns.push(Town {
            id: 7,
            name: "Puerto Test".into(),
            pos: openttdrs_core::TileCoord::new(8, 8),
            ..Town::default()
        });
        state
    }

    #[test]
    fn on_closed_clears_authority_state() {
        let mut world = World::new();
        world.init_resource::<TownAuthorityWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        {
            let mut st = world.resource_mut::<TownAuthorityWindowState>();
            st.open = true;
            st.town_id = Some(3);
        }
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::TownAuthority),
        ));
        world
            .run_system_once(town_authority_window_on_closed)
            .unwrap();
        let st = world.resource::<TownAuthorityWindowState>();
        assert!(!st.open);
        assert!(st.town_id.is_none());
    }

    #[test]
    fn parent_town_closed_closes_authority() {
        let mut world = World::new();
        world.init_resource::<TownAuthorityWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.resource_mut::<TownAuthorityWindowState>().open = true;
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::Town),
        ));
        world
            .run_system_once(town_authority_window_on_closed)
            .unwrap();
        assert!(!world.resource::<TownAuthorityWindowState>().open);
    }

    #[test]
    fn action_statuses_match_domain_mask_and_explain_money_or_availability() {
        let mut town = Town::default();
        let settings = TownAuthoritySettings::default();
        let all = mask_of_town_actions(&town, CompanyId::PLAYER, i64::MAX, settings);
        assert_eq!(all, 0xFF);
        let no_money = mask_of_town_actions(&town, CompanyId::PLAYER, 0, settings);
        for action in TownAction::all() {
            assert_eq!(
                town_action_availability(action, no_money, all),
                TownActionAvailability::InsufficientFunds
            );
        }

        town.road_build_months = 1;
        let blocked = mask_of_town_actions(&town, CompanyId::PLAYER, i64::MAX, settings);
        assert_eq!(
            town_action_availability(TownAction::RoadRebuild, blocked, blocked),
            TownActionAvailability::NotAvailable
        );
    }

    #[test]
    fn authority_button_executes_command_and_keeps_window_open() {
        let mut world = World::new();
        world.insert_resource(SimWorld {
            state: state_with_town(10_000),
            ..SimWorld::default()
        });
        world.insert_resource(TownAuthorityWindowState {
            open: true,
            town_id: Some(7),
        });
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.spawn((
            Button,
            TownAuthorityActionButton(TownAction::RoadRebuild),
            Interaction::Pressed,
        ));

        world
            .run_system_once(handle_town_authority_buttons)
            .unwrap();

        let sim = world.resource::<SimWorld>();
        assert_eq!(
            sim.state.economy.money,
            10_000 - TownAction::RoadRebuild.cost()
        );
        assert_eq!(sim.state.towns[0].road_build_months, 6);
        assert!(world.resource::<TownAuthorityWindowState>().open);
    }

    #[test]
    fn authority_ratings_list_each_known_company() {
        let mut town = Town::default();
        town.authority_ratings = vec![120, -50];
        assert_eq!(
            authority_ratings_text(&town, 2),
            "  Compañía 1: 120\n  Compañía 2: -50"
        );
    }
}
