use bevy::prelude::*;
use openttdrs_core::{
    NewsDisplayMode, NewsReference, NewsType, PendingNewsEvent, format_calendar_date, format_money,
};

use crate::camera::{CameraFocusRequest, tile_camera_world_pos};
use crate::news_prefs::NewsDisplayPrefs;
use crate::state::{EditorSession, SimRunState, SimWorld, sim_is_paused};
use crate::ui::hud::{HudBuildFeedback, SelectedTileInfo};

use super::{
    COMPANY_DISPLAY_NAME, NewsUiState, StatusBarDateText, StatusBarDefaultText, StatusBarMoneyText,
    StatusBarReminderDot, StatusBarTickerText, TICKER_SCROLL_MAX, TICKER_SCROLL_SPEED, TickerState,
    news_has_audible_alert,
};

fn active_company_display_name(sim: &SimWorld) -> String {
    sim.state
        .companies
        .iter()
        .find(|c| c.id == sim.state.active_company)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| COMPANY_DISPLAY_NAME.to_string())
}

#[derive(Default)]
pub(crate) struct StatusBarCache {
    date: String,
    money: String,
    company: String,
    editor: bool,
    ticker: Option<(u64, f32)>,
    default_visible: bool,
    ticker_visible: bool,
    reminder_visible: bool,
    paused_label: Option<String>,
}

pub(crate) fn sync_status_bar(
    sim: Res<SimWorld>,
    news_ui: Res<NewsUiState>,
    run_state: Res<State<SimRunState>>,
    editor: Res<EditorSession>,
    mut queries: ParamSet<(
        Query<&mut Text, With<StatusBarDateText>>,
        Query<&mut Text, With<StatusBarMoneyText>>,
        Query<(&mut Text, &mut Visibility), With<StatusBarDefaultText>>,
        Query<(&mut Text, &mut Node, &mut Visibility), With<StatusBarTickerText>>,
        Query<&mut Visibility, With<StatusBarReminderDot>>,
    )>,
    time: Res<Time>,
    mut cache: Local<StatusBarCache>,
) {
    let date = format_calendar_date(sim.state.tick);
    let money = format_money(sim.state.economy.money);
    let company_name = active_company_display_name(&sim);
    let company = if editor.active {
        format!("EDITOR · {company_name}")
    } else {
        company_name
    };
    let paused_label = sim_is_paused(&run_state).then(|| "Pausado".to_string());
    let ticker_key = news_ui.ticker.as_ref().map(|t| (t.item_id, t.scroll));
    let default_visible = news_ui.ticker.is_none() && paused_label.is_none();
    let ticker_visible = news_ui.ticker.is_some() && paused_label.is_none();
    let reminder_visible = time.elapsed_secs() < news_ui.reminder_until_secs
        && !ticker_visible
        && paused_label.is_none();

    if cache.date == date
        && cache.money == money
        && cache.company == company
        && cache.editor == editor.active
        && cache.ticker == ticker_key
        && cache.default_visible == default_visible
        && cache.ticker_visible == ticker_visible
        && cache.reminder_visible == reminder_visible
        && cache.paused_label == paused_label
    {
        return;
    }
    cache.date = date.clone();
    cache.money = money.clone();
    cache.company = company.clone();
    cache.editor = editor.active;
    cache.ticker = ticker_key;
    cache.default_visible = default_visible;
    cache.ticker_visible = ticker_visible;
    cache.reminder_visible = reminder_visible;
    cache.paused_label = paused_label.clone();

    if let Ok(mut text) = queries.p0().single_mut() {
        *text = Text::new(date);
    }
    if let Ok(mut text) = queries.p1().single_mut() {
        *text = Text::new(money);
    }

    if let Ok((mut text, mut vis)) = queries.p2().single_mut() {
        if let Some(ref paused) = paused_label {
            *text = Text::new(paused.clone());
            *vis = Visibility::Visible;
        } else if default_visible {
            *text = Text::new(company);
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }

    if let Ok((mut text, mut node, mut vis)) = queries.p3().single_mut() {
        if ticker_visible {
            if let Some(ticker) = &news_ui.ticker
                && let Some(item) = sim.state.news.get(ticker.item_id)
            {
                *text = Text::new(item.headline.clone());
            }
            node.left = Val::Px(-ticker_key.map_or(0.0, |(_, s)| s));
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }

    if let Ok(mut vis) = queries.p4().single_mut() {
        *vis = if reminder_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(crate) fn drain_news_events(
    mut sim: ResMut<SimWorld>,
    mut news_ui: ResMut<NewsUiState>,
    mut feedback: ResMut<HudBuildFeedback>,
    news_prefs: Res<NewsDisplayPrefs>,
    time: Res<Time>,
) {
    let events: Vec<_> = sim.state.runtime.pending_news_events.drain(..).collect();
    for event in events {
        let PendingNewsEvent::ItemAdded { id } = event;
        let (display, news_type) = sim
            .state
            .news
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| (news_prefs.0.display_for(item.news_type), item.news_type))
            .unwrap_or((NewsDisplayMode::Full, NewsType::CompanyInfo));
        if let Some(item) = sim.state.news.items.iter_mut().find(|item| item.id == id) {
            item.display = display;
        }
        match display {
            NewsDisplayMode::Full => {
                if !news_ui.shown_full.contains(&id) {
                    news_ui.waiting_full.push_back(id);
                }
            }
            NewsDisplayMode::Summary => {
                news_ui.waiting_ticker.push_back(id);
                if news_has_audible_alert(news_type) {
                    feedback.pending_news_ticker = true;
                    info!("noticias: id={id} tipo={news_type:?}; ticker con sonido");
                } else {
                    info!("noticias: id={id} entrega recurrente; ticker sin sonido");
                }
            }
            NewsDisplayMode::Off => {
                news_ui.reminder_until_secs = time.elapsed_secs() + 1.35;
            }
        }
    }
}

pub(crate) fn update_news_playback(
    mut commands: Commands,
    time: Res<Time>,
    run_state: Res<State<SimRunState>>,
    sim: Res<SimWorld>,
    mut news_ui: ResMut<NewsUiState>,
    mut feedback: ResMut<HudBuildFeedback>,
    mut popup_nodes: Query<&mut Node, With<super::NewsPopupRoot>>,
) {
    let dt = time.delta_secs();
    if sim_is_paused(&run_state) {
        return;
    }

    if news_ui.popup.is_none()
        && let Some(id) = news_ui.waiting_full.pop_front()
        && let Some(item) = sim.state.news.get(id).cloned()
    {
        spawn_news_popup(&mut commands, &item, &mut news_ui, &mut feedback);
        news_ui.shown_full.insert(id);
    }

    let mut popup_despawn: Option<Entity> = None;
    let mut popup_layout: Option<(Entity, f32)> = None;
    if let Some(popup) = news_ui.popup.as_mut() {
        if popup.sliding_in {
            popup.bottom += super::POPUP_SLIDE_SPEED * dt;
            if popup.bottom >= popup.target_bottom {
                popup.bottom = popup.target_bottom;
                popup.sliding_in = false;
                popup.hold_remaining_ms = super::POPUP_HOLD_MS;
            }
        } else {
            popup.hold_remaining_ms -= dt * 1000.0;
            if popup.hold_remaining_ms <= 0.0 {
                popup_despawn = Some(popup.entity);
            }
        }
        if popup_despawn.is_none() {
            popup_layout = Some((popup.entity, popup.bottom));
        }
    }

    if let Some(entity) = popup_despawn {
        commands.entity(entity).despawn();
        news_ui.popup = None;
    } else if let Some((entity, bottom)) = popup_layout
        && let Ok(mut node) = popup_nodes.get_mut(entity)
    {
        node.bottom = Val::Px(bottom);
    }

    if news_ui.popup.is_none()
        && news_ui.ticker.is_none()
        && let Some(id) = news_ui.waiting_ticker.pop_front()
    {
        news_ui.ticker = Some(TickerState {
            item_id: id,
            scroll: 0.0,
        });
    }
    if let Some(ticker) = news_ui.ticker.as_mut() {
        ticker.scroll += TICKER_SCROLL_SPEED * dt;
        if ticker.scroll >= TICKER_SCROLL_MAX {
            news_ui.ticker = None;
        }
    }
}

fn spawn_news_popup(
    commands: &mut Commands,
    item: &openttdrs_core::NewsItem,
    news_ui: &mut NewsUiState,
    feedback: &mut HudBuildFeedback,
) {
    use crate::ui::font::UiFontRole;

    let target_bottom = super::STATUS_BAR_HEIGHT + 4.0;
    let body = item.body.clone().unwrap_or_else(|| item.headline.clone());
    let date_label = item.date_label();

    let entity = commands
        .spawn((
            super::NewsPopupRoot,
            crate::ui::toolbar::BuildMenuUi,
            GlobalZIndex(super::STATUS_BAR_Z + 5),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                bottom: Val::Px(-220.0),
                width: Val::Px(super::POPUP_WIDTH),
                margin: UiRect::left(Val::Px(-super::POPUP_WIDTH * 0.5)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.98, 0.98, 0.94)),
            BorderColor::all(Color::srgb(0.08, 0.08, 0.08)),
        ))
        .with_children(|popup| {
            popup.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    ..default()
                },
                children![
                    (
                        Text::new("Noticias"),
                        TextFont {
                            font_size: FontSize::Rem(UiFontRole::Title.rem_size()),
                            ..default()
                        },
                        TextColor(Color::srgb(0.12, 0.12, 0.12)),
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        },
                        children![
                            (
                                super::NewsPopupDateText,
                                Text::new(date_label.clone()),
                                TextFont {
                                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.35, 0.35, 0.35)),
                            ),
                            (
                                super::NewsPopupCloseButton,
                                Button,
                                Node {
                                    min_width: Val::Px(22.0),
                                    min_height: Val::Px(22.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                                Interaction::default(),
                                children![(
                                    Text::new("×"),
                                    TextFont {
                                        font_size: FontSize::Rem(UiFontRole::Title.rem_size()),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.25, 0.25, 0.25)),
                                )],
                            ),
                        ],
                    ),
                ],
            ));
            popup
                .spawn((
                    super::NewsPopupFocusButton,
                    Button,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Interaction::default(),
                ))
                .with_children(|focus| {
                    focus.spawn((
                        super::NewsPopupHeadlineText,
                        Text::new(item.headline.clone()),
                        TextFont {
                            font_size: FontSize::Rem(UiFontRole::Hud.rem_size()),
                            ..default()
                        },
                        TextColor(Color::srgb(0.05, 0.05, 0.05)),
                    ));
                    focus.spawn((
                        super::NewsPopupBodyText,
                        Text::new(body),
                        TextFont {
                            font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                            ..default()
                        },
                        TextColor(Color::srgb(0.15, 0.15, 0.15)),
                    ));
                });
        })
        .id();

    news_ui.popup = Some(super::PopupState {
        item_id: item.id,
        bottom: -220.0,
        target_bottom,
        hold_remaining_ms: 0.0,
        sliding_in: true,
        entity,
    });

    match item.news_type {
        NewsType::FirstCargoDelivered | NewsType::FirstVehicleRunning => {
            feedback.pending_news_applause = true;
            info!(
                "noticias: id={} tipo={:?}; popup con sonido",
                item.id, item.news_type
            );
        }
        // Las descargas posteriores a la primera no deben encadenar repiques.
        NewsType::CargoDelivered => {
            info!(
                "noticias: id={} entrega recurrente; popup sin sonido",
                item.id
            );
        }
        NewsType::VehicleAdvice
        | NewsType::CompanyInfo
        | NewsType::IndustryClose
        | NewsType::Economy => {
            feedback.pending_news_ticker = true;
            info!(
                "noticias: id={} tipo={:?}; aviso sonoro",
                item.id, item.news_type
            );
        }
        NewsType::Accident => {
            feedback.pending_news_chime = true;
            info!("noticias: id={} accidente; campanilla", item.id);
        }
    }
}

pub(crate) fn focus_news_reference(
    reference: NewsReference,
    sim: &SimWorld,
    focus: &mut CameraFocusRequest,
    selected: &mut SelectedTileInfo,
) {
    let NewsReference::Tile(coord) = reference else {
        return;
    };
    focus.target = Some(tile_camera_world_pos(&sim.state.map, coord));
    selected.pos = Some(coord);
}

pub(crate) fn handle_news_popup_focus(
    news_ui: Res<NewsUiState>,
    sim: Res<SimWorld>,
    mut focus: ResMut<CameraFocusRequest>,
    mut selected: ResMut<SelectedTileInfo>,
    interaction_q: Query<&Interaction, (Changed<Interaction>, With<super::NewsPopupFocusButton>)>,
) {
    for interaction in &interaction_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(popup) = news_ui.popup.as_ref() else {
            continue;
        };
        let Some(item) = sim.state.news.get(popup.item_id) else {
            continue;
        };
        focus_news_reference(item.reference, &sim, &mut focus, &mut selected);
    }
}

pub(crate) fn handle_news_popup_close(
    mut news_ui: ResMut<NewsUiState>,
    mut interaction_q: Query<
        &Interaction,
        (Changed<Interaction>, With<super::NewsPopupCloseButton>),
    >,
    mut commands: Commands,
) {
    for interaction in &mut interaction_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(popup) = news_ui.popup.take() else {
            continue;
        };
        commands.entity(popup.entity).despawn();
    }
}

pub(crate) fn handle_status_bar_center_click(
    mut news_ui: ResMut<NewsUiState>,
    mut interaction_q: Query<
        &Interaction,
        (Changed<Interaction>, With<super::StatusBarCenterButton>),
    >,
    sim: Res<SimWorld>,
    mut feedback: ResMut<HudBuildFeedback>,
    mut focus: ResMut<CameraFocusRequest>,
    mut selected: ResMut<SelectedTileInfo>,
) {
    for interaction in &mut interaction_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(ticker) = &news_ui.ticker
            && let Some(item) = sim.state.news.get(ticker.item_id)
        {
            focus_news_reference(item.reference, &sim, &mut focus, &mut selected);
            continue;
        }
        let Some(item) = sim.state.news.items.front().cloned() else {
            continue;
        };
        if item.display != NewsDisplayMode::Full {
            continue;
        }
        focus_news_reference(item.reference, &sim, &mut focus, &mut selected);
        news_ui.shown_full.remove(&item.id);
        news_ui.waiting_full.push_front(item.id);
        if news_has_audible_alert(item.news_type) {
            feedback.pending_news_chime = true;
            info!("noticias: id={} reabierta; campanilla", item.id);
        }
    }
}
