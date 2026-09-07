//! Ventana flotante de información de pueblo (estilo `OpenTTD`).
//!
//! Se abre al hacer clic sin herramienta sobre una casa o sobre el cartel del
//! pueblo. Muestra habitantes, casas y la demanda de pasajeros/correo del
//! último período, con botón para centrar la cámara en el pueblo.

use bevy::prelude::*;
use openttdrs_core::{
    Command, GameState, TileCoord, TileKind, format_money,
    town::{
        FUND_BUILDINGS_COST, MAIL_PER_HOUSE, PASSENGERS_PER_HOUSE, TOWN_ADVERTISE_COST,
        TOWN_GROWTH_DESERT, TOWN_GROWTH_WINTER, Town, TownGrowthEffect,
    },
};

use crate::i18n::{Locale, localized_text};
use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CREAM,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::sparkline::sparkline_u32;
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct TownWindowState {
    pub(crate) town_id: Option<u32>,
}

#[derive(Component)]
pub(crate) struct TownWindowBodyText;

#[derive(Component, Clone, Copy)]
pub(crate) enum TownWindowButton {
    CenterCamera,
    Advertise,
    FundBuildings,
    /// Abre las acciones de autoridad local.
    OpenAuthority,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TownWindowActionText(pub(crate) TownWindowButton);

pub(crate) fn setup_town_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Town,
        "Pueblo",
        TITLE_CREAM,
        Vec2::new(60.0, 90.0),
        340.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            TownWindowBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        // Chrome compacto (#179): fila de acciones cortas, no muro de botones.
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        })
        .with_children(|row| {
            spawn_town_action_button(row, asset_server, TownWindowButton::CenterCamera);
            spawn_town_action_button(row, asset_server, TownWindowButton::Advertise);
            spawn_town_action_button(row, asset_server, TownWindowButton::FundBuildings);
            spawn_town_action_button(row, asset_server, TownWindowButton::OpenAuthority);
        });
    });
}

fn spawn_town_action_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    button: TownWindowButton,
) {
    parent.spawn((
        Button,
        button,
        Node {
            flex_grow: 1.0,
            height: Val::Px(22.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            padding: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            TownWindowActionText(button),
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
}

/// Casas del pueblo: por `town_id` en `m2` (saves de `OpenTTD`) o, si el mapa
/// no atribuye casas por `m2` (mapas procedurales), por cercanía al centro.
pub(crate) fn count_town_houses(state: &GameState, town_id: u32) -> u32 {
    let (w, h) = state.map.dimensions();
    let mut by_m2 = 0_u32;
    let mut by_near = 0_u32;
    let mut attributed_ids: Vec<u32> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let pos = TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(tile) = state.map.get(pos) else {
                continue;
            };
            if tile.kind != TileKind::House {
                continue;
            }
            let tid = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
            if state.towns.iter().any(|t| t.id == tid) {
                if !attributed_ids.contains(&tid) {
                    attributed_ids.push(tid);
                }
                if tid == town_id {
                    by_m2 += 1;
                }
            }
            if nearest_town_id(state, pos) == Some(town_id) {
                by_near += 1;
            }
        }
    }
    // En mapas procedurales m2 vale 0 en todas las casas: si todas caen en un
    // único id habiendo varios pueblos, el dato no es fiable y usamos cercanía.
    let m2_reliable =
        !attributed_ids.is_empty() && (attributed_ids.len() > 1 || state.towns.len() <= 1);
    if m2_reliable { by_m2 } else { by_near }
}

/// Pueblo dueño de una casa: `m2` si apunta a un pueblo existente, si no el más cercano.
pub(crate) fn town_for_house_tile(state: &GameState, pos: TileCoord) -> Option<u32> {
    let tile = state.map.get(pos)?;
    let tid = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
    if state.towns.iter().any(|t| t.id == tid) {
        return Some(tid);
    }
    nearest_town_id(state, pos)
}

fn nearest_town_id(state: &GameState, pos: TileCoord) -> Option<u32> {
    state
        .towns
        .iter()
        .min_by_key(|t| t.pos.x.abs_diff(pos.x) + t.pos.y.abs_diff(pos.y))
        .map(|t| t.id)
}

fn format_goal_value(locale: Locale, goal: u32) -> String {
    if goal == 0 {
        "—".into()
    } else if goal == TOWN_GROWTH_WINTER {
        localized_text(locale, "invierno")
    } else if goal == TOWN_GROWTH_DESERT {
        localized_text(locale, "desierto")
    } else {
        goal.to_string()
    }
}

fn format_town_goals(locale: Locale, town: &Town) -> String {
    let labels = [
        (TownGrowthEffect::Passengers, "Pasajeros"),
        (TownGrowthEffect::Mail, "Correo"),
        (TownGrowthEffect::Goods, "Bienes"),
        (TownGrowthEffect::Food, "Comida"),
        (TownGrowthEffect::Water, "Agua"),
    ];
    let mut lines = Vec::new();
    for (effect, label) in labels {
        let i = effect as usize;
        let goal = town.goals[i];
        if goal == 0 && town.received_old[i] == 0 {
            continue;
        }
        lines.push(format!(
            "  {}: {} / {} {}",
            localized_text(locale, label),
            town.received_old[i],
            localized_text(locale, "meta"),
            format_goal_value(locale, goal)
        ));
    }
    if lines.is_empty() {
        format!(
            "  ({})",
            localized_text(locale, "sin metas activas en este clima")
        )
    } else {
        lines.join("\n")
    }
}

fn town_action_button_label(locale: Locale, button: TownWindowButton) -> String {
    match button {
        TownWindowButton::CenterCamera => localized_text(locale, "Loc"),
        TownWindowButton::Advertise => format!(
            "{} {}",
            localized_text(locale, "Pub"),
            format_money(TOWN_ADVERTISE_COST)
        ),
        TownWindowButton::FundBuildings => format!(
            "{} {}",
            localized_text(locale, "Fondos"),
            format_money(FUND_BUILDINGS_COST)
        ),
        TownWindowButton::OpenAuthority => localized_text(locale, "Aut."),
    }
}

pub(crate) fn sync_town_window(
    town_state: Res<TownWindowState>,
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<TownWindowBodyText>>,
    mut body_q: Query<
        &mut Text,
        (
            With<TownWindowBodyText>,
            Without<FloatingWindowTitleText>,
            Without<TownWindowActionText>,
        ),
    >,
    mut action_text_q: Query<
        (&TownWindowActionText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<TownWindowBodyText>,
        ),
    >,
) {
    let locale = prefs.locale();
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Town)
    else {
        return;
    };
    let town = town_state
        .town_id
        .and_then(|id| sim.state.towns.iter().find(|t| t.id == id));
    let Some(town) = town else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    for (button, mut text) in &mut action_text_q {
        **text = town_action_button_label(locale, button.0);
    }
    if !town_state.is_changed() && !sim.is_changed() && !prefs.is_changed() {
        return;
    }
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Town)
    {
        **title = format!("{} ({})", town.name, town.population);
    }
    let houses = count_town_houses(&sim.state, town.id);
    if let Ok(mut body) = body_q.single_mut() {
        let rating = town.authority_rating(sim.state.active_company);
        let rating_hint = if rating >= 500 {
            localized_text(locale, "buena")
        } else if rating >= 0 {
            localized_text(locale, "neutral")
        } else {
            localized_text(locale, "mala")
        };
        let pop_hist: Vec<u32> = town.history.samples.iter().map(|s| s.population).collect();
        let pax_hist: Vec<u32> = town
            .history
            .samples
            .iter()
            .map(|s| s.passengers_served)
            .collect();
        let mail_hist: Vec<u32> = town.history.samples.iter().map(|s| s.mail_served).collect();
        let hist_block = if town.history.samples.is_empty() {
            format!(
                "{}: ({})",
                localized_text(locale, "Historial mensual"),
                localized_text(locale, "avanza el tiempo para ver series")
            )
        } else {
            format!(
                "{} ({} m):\n  {}  {}\n  {}  {}\n  {}     {}",
                localized_text(locale, "Historial mensual"),
                town.history.samples.len(),
                localized_text(locale, "Población"),
                sparkline_u32(&pop_hist, 24),
                localized_text(locale, "Pasajeros"),
                sparkline_u32(&pax_hist, 24),
                localized_text(locale, "Correo"),
                sparkline_u32(&mail_hist, 24),
            )
        };
        let growing = if town.is_growing {
            localized_text(locale, "sí")
        } else {
            localized_text(locale, "no")
        };
        let fund_left = town.fund_buildings_months;
        let goals_line = format_town_goals(locale, town);
        **body = format!(
            "{}: {}\n{}: {}\n{}: {rating} (−1000…+1000, {rating_hint})\n{}: {growing}\n{}: {fund_left} {}\n\n{} ({})\n  {}: {}\n  {}: {}\n  {}: {}\n\n{}:\n{goals_line}\n\n{}:\n  {} {}\n  {} {}\n\n{hist_block}",
            localized_text(locale, "Habitantes"),
            town.population,
            localized_text(locale, "Casas"),
            houses,
            localized_text(locale, "Autoridad local"),
            localized_text(locale, "Creciendo"),
            localized_text(locale, "Financiación edificios"),
            localized_text(locale, "mes(es)"),
            localized_text(locale, "Servicio acumulado"),
            localized_text(locale, "partida"),
            localized_text(locale, "Pasajeros"),
            town.passengers_served,
            localized_text(locale, "Correo"),
            town.mail_served,
            localized_text(locale, "Crecimiento financiado"),
            town.growth_funded,
            localized_text(locale, "Metas de carga (mes anterior)"),
            localized_text(locale, "Demanda teórica por ciclo (casas × tasa)"),
            localized_text(locale, "Pasajeros máx."),
            houses * PASSENGERS_PER_HOUSE,
            localized_text(locale, "Correo máx."),
            houses * MAIL_PER_HOUSE,
        );
    }
}

pub(crate) fn handle_town_window_buttons(
    buttons: Query<(&Interaction, &TownWindowButton), (Changed<Interaction>, With<Button>)>,
    town_state: Res<TownWindowState>,
    mut authority: ResMut<crate::ui::town_authority_window::TownAuthorityWindowState>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            TownWindowButton::CenterCamera => {
                let Some(town) = town_state
                    .town_id
                    .and_then(|id| sim.state.towns.iter().find(|t| t.id == id))
                else {
                    continue;
                };
                let height = sim.state.map.get(town.pos).map_or(0, |t| t.height);
                let center = tile_pos(town.pos.x, town.pos.y, height, 0.0);
                if let Ok(mut transform) = cam_q.single_mut() {
                    transform.translation.x = center.x;
                    transform.translation.y = center.y;
                }
            }
            TownWindowButton::Advertise => {
                let Some(town_id) = town_state.town_id else {
                    continue;
                };
                if let Err(e) = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::TownAdvertise(town_id),
                ) {
                    push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                }
            }
            TownWindowButton::FundBuildings => {
                let Some(town_id) = town_state.town_id else {
                    continue;
                };
                if let Err(e) = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::TownFundBuildings(town_id),
                ) {
                    push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                }
            }
            TownWindowButton::OpenAuthority => {
                if let Some(town_id) = town_state.town_id {
                    crate::ui::town_authority_window::open_town_authority_for(
                        town_id,
                        &mut authority,
                    );
                }
            }
        }
    }
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn town_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut town_state: ResMut<TownWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::Town {
            town_state.town_id = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn state_with_two_towns() -> GameState {
        let mut state = GameState::new(32, 32);
        state.towns.push(openttdrs_core::Town {
            id: 1,
            pos: TileCoord::new(5, 5),
            name: "Oeste".into(),
            population: 100,
            ..Default::default()
        });
        state.towns.push(openttdrs_core::Town {
            id: 2,
            pos: TileCoord::new(25, 25),
            name: "Este".into(),
            population: 200,
            ..Default::default()
        });
        state
    }

    #[test]
    fn counts_houses_by_m2_when_attributed() {
        let mut state = state_with_two_towns();
        for (pos, tid) in [
            (TileCoord::new(4, 5), 1_u8),
            (TileCoord::new(6, 5), 1),
            (TileCoord::new(24, 25), 2),
        ] {
            state.map.set_kind(pos, TileKind::House).unwrap();
            let mut tile = state.map.get(pos).unwrap();
            tile.m2 = tid;
            state.map.set_tile(pos, tile).unwrap();
        }
        assert_eq!(count_town_houses(&state, 1), 2);
        assert_eq!(count_town_houses(&state, 2), 1);
    }

    #[test]
    fn falls_back_to_distance_on_procedural_maps() {
        let mut state = state_with_two_towns();
        // m2 = 0 en todas (mapa procedural): ningún id de pueblo coincide.
        for pos in [
            TileCoord::new(4, 5),
            TileCoord::new(6, 6),
            TileCoord::new(26, 25),
        ] {
            state.map.set_kind(pos, TileKind::House).unwrap();
        }
        assert_eq!(count_town_houses(&state, 1), 2);
        assert_eq!(count_town_houses(&state, 2), 1);
    }

    #[test]
    fn localizes_town_window_chrome_without_mutating_values() {
        assert_eq!(
            town_action_button_label(Locale::En, TownWindowButton::CenterCamera),
            "Center"
        );
        assert_eq!(
            town_action_button_label(Locale::En, TownWindowButton::Advertise),
            format!("Advertise {}", format_money(TOWN_ADVERTISE_COST))
        );
        assert_eq!(
            town_action_button_label(Locale::En, TownWindowButton::FundBuildings),
            format!("Fund {}", format_money(FUND_BUILDINGS_COST))
        );
        assert_eq!(format_goal_value(Locale::En, TOWN_GROWTH_WINTER), "winter");
        assert_eq!(format_goal_value(Locale::En, TOWN_GROWTH_DESERT), "desert");

        let mut town = Town::default();
        town.goals[0] = 12;
        town.received_old[0] = 8;
        let goals = format_town_goals(Locale::En, &town);
        assert!(goals.contains("Passengers"));
        assert!(goals.contains("goal"));
        assert!(goals.contains("8"));
        // Dynamic names remain byte-for-byte intact when no catalog entry exists.
        assert_eq!(localized_text(Locale::En, "Pueblo Ñandú"), "Pueblo Ñandú");
    }
}
