//! Pantalla de fin de partida + highscore local (UI-8).

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{GameScore, format_money, retire_game};

use crate::audio::PendingSimEvents;
use crate::settings::ClientPreferences;
use crate::state::{
    ClientScreen, SimRunState, SimWorld, SuspendedGameSession, toggle_sim_run_state,
};
use crate::ui::font::UiFontRole;

const PANEL_BG: Color = Color::srgba(0.14, 0.13, 0.1, 0.97);
const PANEL_BORDER: Color = Color::srgb(0.74, 0.68, 0.5);
const TITLE: Color = Color::srgb(0.96, 0.91, 0.72);
const BODY: Color = Color::srgb(0.9, 0.86, 0.74);
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct EndScreenState {
    pub(crate) open: bool,
    pub(crate) score: Option<GameScore>,
    pub(crate) rank: usize,
}

/// Pedido diferido desde Ajustes / consola para retirar la compañía.
#[derive(Resource, Default)]
pub(crate) struct RetireGameRequested(pub bool);

#[derive(Component)]
pub(crate) struct EndScreenRoot;

#[derive(Component)]
pub(crate) struct EndScreenBodyText;

#[derive(Component)]
pub(crate) struct EndScreenMenuButton;

pub(crate) fn setup_endscreen(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    commands
        .spawn((
            EndScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.72)),
            GlobalZIndex(4500),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(420.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                BorderColor::all(PANEL_BORDER),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Fin de partida"),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Title.rem_size()),
                        ..default()
                    },
                    TextColor(TITLE),
                ));
                panel.spawn((
                    EndScreenBodyText,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                        ..default()
                    },
                    TextColor(BODY),
                    Node {
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ));
                panel
                    .spawn((
                        Button,
                        EndScreenMenuButton,
                        Node {
                            min_width: Val::Px(180.0),
                            height: Val::Px(36.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        Interaction::default(),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Menú principal"),
                            TextFont {
                                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                                ..default()
                            },
                            TextColor(TITLE),
                        ));
                    });
            });
        });
    let _ = asset_server;
}

pub(crate) fn sync_endscreen(
    state: Res<EndScreenState>,
    mut roots: Query<&mut Visibility, With<EndScreenRoot>>,
    mut body_q: Query<&mut Text, With<EndScreenBodyText>>,
) {
    for mut vis in &mut roots {
        *vis = if state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !state.open {
        return;
    }
    let body = match &state.score {
        Some(score) => format!(
            "{}\n\nCompañía: {}\nPatrimonio: {}\nAño: {}\nRanking local: #{}",
            score.reason.label_es(),
            score.company_name,
            format_money(score.company_value),
            score.calendar_year,
            state.rank.max(1)
        ),
        None => "Partida terminada.".into(),
    };
    for mut text in &mut body_q {
        **text = body.clone();
    }
}

pub(crate) fn watch_game_over_events(
    mut pending: ResMut<PendingSimEvents>,
    mut endscreen: ResMut<EndScreenState>,
    mut prefs: ResMut<ClientPreferences>,
    run_state: Res<State<SimRunState>>,
    mut next_run: ResMut<NextState<SimRunState>>,
) {
    if endscreen.open {
        return;
    }
    let mut score: Option<GameScore> = None;
    pending.0.retain(|ev| {
        if let SimEvent::GameOver {
            company_name,
            company_value,
            calendar_year,
            reason,
        } = ev
        {
            score = Some(GameScore {
                company_name: company_name.clone(),
                company_value: *company_value,
                calendar_year: *calendar_year,
                reason: *reason,
            });
            false
        } else {
            true
        }
    });
    let Some(score) = score else {
        return;
    };
    let rank = prefs.insert_highscore(&score);
    endscreen.open = true;
    endscreen.score = Some(score);
    endscreen.rank = rank;
    if *run_state.get() != SimRunState::Paused {
        toggle_sim_run_state(&run_state, &mut next_run);
    }
    info!("Fin de partida — ranking local #{rank}");
}

/// Marca retiro voluntario (Ajustes / consola); `process_retire_game_request` lo aplica.
pub(crate) fn request_retire_game(req: &mut RetireGameRequested) {
    req.0 = true;
}

pub(crate) fn process_retire_game_request(
    mut req: ResMut<RetireGameRequested>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<PendingSimEvents>,
) {
    if !req.0 {
        return;
    }
    req.0 = false;
    if retire_game(&mut sim.state).is_some() {
        pending
            .0
            .extend(sim.state.runtime.pending_sim_events.drain());
    }
}

pub(crate) fn handle_endscreen_menu_button(
    buttons: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<EndScreenMenuButton>,
        ),
    >,
    mut endscreen: ResMut<EndScreenState>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    mut sim: ResMut<SimWorld>,
) {
    for interaction in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        endscreen.open = false;
        endscreen.score = None;
        endscreen.rank = 0;
        suspended.active = false;
        *sim = SimWorld::default();
        info!("Fin de partida → menú principal (sin Continuar)");
        next_screen.set(ClientScreen::MainMenu);
    }
}

/// True si el endscreen bloquea Esc → menú (solo salida por botón).
#[must_use]
pub(crate) fn endscreen_blocks_escape(state: &EndScreenState) -> bool {
    state.open
}

#[cfg(test)]
mod tests {
    use super::*;

    // smoke: setup symbols compile with GameScore helpers
    #[test]
    fn endscreen_blocks_when_open() {
        let mut state = EndScreenState::default();
        assert!(!endscreen_blocks_escape(&state));
        state.open = true;
        assert!(endscreen_blocks_escape(&state));
    }
}
