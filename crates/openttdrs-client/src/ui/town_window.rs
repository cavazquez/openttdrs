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

use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
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
}

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
        spawn_town_action_button(
            body,
            asset_server,
            "Centrar vista en el pueblo",
            TownWindowButton::CenterCamera,
        );
        spawn_town_action_button(
            body,
            asset_server,
            &format!("Publicidad ({})", format_money(TOWN_ADVERTISE_COST)),
            TownWindowButton::Advertise,
        );
        spawn_town_action_button(
            body,
            asset_server,
            &format!(
                "Financiar edificios ({})",
                format_money(FUND_BUILDINGS_COST)
            ),
            TownWindowButton::FundBuildings,
        );
    });
}

fn spawn_town_action_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    button: TownWindowButton,
) {
    parent.spawn((
        Button,
        button,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(22.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
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

fn format_goal_value(goal: u32) -> String {
    if goal == 0 {
        "—".into()
    } else if goal == TOWN_GROWTH_WINTER {
        "invierno".into()
    } else if goal == TOWN_GROWTH_DESERT {
        "desierto".into()
    } else {
        goal.to_string()
    }
}

fn format_town_goals(town: &Town) -> String {
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
            "  {label}: {} / meta {}",
            town.received_old[i],
            format_goal_value(goal)
        ));
    }
    if lines.is_empty() {
        "  (sin metas activas en este clima)".into()
    } else {
        lines.join("\n")
    }
}

pub(crate) fn sync_town_window(
    town_state: Res<TownWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut body_q: Query<&mut Text, (With<TownWindowBodyText>, Without<FloatingWindowTitleText>)>,
) {
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
    if !town_state.is_changed() && !sim.is_changed() {
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
        let rating = town.local_authority_rating;
        let rating_hint = if rating >= 500 {
            "buena"
        } else if rating >= 0 {
            "neutral"
        } else {
            "mala"
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
            "Historial mensual: (avanza el tiempo para ver series)".to_string()
        } else {
            format!(
                "Historial mensual ({} m):\n  Población  {}\n  Pasajeros  {}\n  Correo     {}",
                town.history.samples.len(),
                sparkline_u32(&pop_hist, 24),
                sparkline_u32(&pax_hist, 24),
                sparkline_u32(&mail_hist, 24),
            )
        };
        let growing = if town.is_growing { "sí" } else { "no" };
        let fund_left = town.fund_buildings_months;
        let goals_line = format_town_goals(town);
        **body = format!(
            "Habitantes: {}\nCasas: {}\nAutoridad local: {rating} (−1000…+1000, {rating_hint})\nCreciendo: {growing}\nFinanciación edificios: {fund_left} mes(es)\n\nServicio acumulado (partida):\n  Pasajeros: {}\n  Correo: {}\n  Crecimiento financiado: {}\n\nMetas de carga (mes anterior):\n{goals_line}\n\nDemanda teórica por ciclo (casas × tasa):\n  Pasajeros máx. {}\n  Correo máx. {}\n\n{hist_block}",
            town.population,
            houses,
            town.passengers_served,
            town.mail_served,
            town.growth_funded,
            houses * PASSENGERS_PER_HOUSE,
            houses * MAIL_PER_HOUSE,
        );
    }
}

pub(crate) fn handle_town_window_buttons(
    buttons: Query<(&Interaction, &TownWindowButton), (Changed<Interaction>, With<Button>)>,
    town_state: Res<TownWindowState>,
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
        }
    }
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn town_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut town_state: ResMut<TownWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Town {
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
}
