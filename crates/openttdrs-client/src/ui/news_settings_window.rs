//! Ventana de configuración Off / Summary / Full por tipo de noticia (N5).

use bevy::prelude::*;
use openttdrs_core::{NewsDisplayMode, NewsType, news_type_label};

use crate::news_prefs::NewsDisplayPrefs;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);

#[derive(Resource, Default)]
pub(crate) struct NewsSettingsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct NewsSettingsModeButton {
    pub(crate) news_type: NewsType,
    pub(crate) mode: NewsDisplayMode,
}

const NEWS_TYPES: [NewsType; 4] = [
    NewsType::CargoDelivered,
    NewsType::FirstCargoDelivered,
    NewsType::FirstVehicleRunning,
    NewsType::VehicleAdvice,
];

pub(crate) fn setup_news_settings_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::NewsSettings,
        "Noticias",
        TITLE_BROWN,
        Vec2::new(260.0, 140.0),
        360.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Off = silencio · Summary = ticker · Full = cartel"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
        ));
        for news_type in NEWS_TYPES {
            body.spawn((Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: Val::Px(148.0),
                            ..default()
                        },
                        children![(
                            Text::new(news_type_label(news_type)),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                    for mode in [
                        NewsDisplayMode::Off,
                        NewsDisplayMode::Summary,
                        NewsDisplayMode::Full,
                    ] {
                        row.spawn((
                            Button,
                            NewsSettingsModeButton { news_type, mode },
                            Node {
                                min_width: Val::Px(58.0),
                                height: Val::Px(24.0),
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
                                Text::new(mode_button_label(mode)),
                                window_text_font(asset_server, UiFontRole::Caption),
                                TextColor(WINDOW_TEXT),
                            )],
                        ));
                    }
                });
        }
    });
}

fn mode_button_label(mode: NewsDisplayMode) -> &'static str {
    match mode {
        NewsDisplayMode::Off => "Off",
        NewsDisplayMode::Summary => "Ticker",
        NewsDisplayMode::Full => "Cartel",
    }
}

pub(crate) fn sync_news_settings_window(
    state: Res<NewsSettingsWindowState>,
    prefs: Res<NewsDisplayPrefs>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut buttons: Query<(&NewsSettingsModeButton, &mut BorderColor), Without<FloatingWindow>>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::NewsSettings)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    for (button, mut border) in &mut buttons {
        let active = prefs.0.display_for(button.news_type) == button.mode;
        *border = if active {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }
}

pub(crate) fn handle_news_settings_buttons(
    mut prefs: ResMut<NewsDisplayPrefs>,
    buttons: Query<(&Interaction, &NewsSettingsModeButton), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        prefs.0.set_display(button.news_type, button.mode);
    }
}

pub(crate) fn news_settings_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<NewsSettingsWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::NewsSettings {
            state.open = false;
        }
    }
}
