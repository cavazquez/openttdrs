//! Ventana unificada de sonido: volúmenes, flags SFX y jukebox OpenMSX.

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::audio::{
    MusicPlayer, MusicPlaylist, MusicState, music_apply_playlist, music_skip, music_toggle_playback,
};
use crate::i18n::Locale;
use crate::settings::ClientPreferences;
use crate::state::ClientScreen;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, MENU_OVERLAY_WINDOW_Z, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::SimHudControls;
use crate::ui::toolbar::{BuildMenuUi, SoundMusicToolbarButton};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);
const SLIDER_TRACK: Color = Color::srgb(0.18, 0.14, 0.10);
const SLIDER_FILL: Color = Color::srgb(0.52, 0.68, 0.38);

#[derive(Resource, Default)]
pub(crate) struct SoundMusicWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum AudioSettingsButton {
    Vehicle,
    Ambient,
    Disaster,
    Confirm,
    ClickBeep,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MusicWindowButton {
    PlayStop,
    Prev,
    Next,
    Playlist(MusicPlaylist),
}

#[derive(Component, Clone, Copy)]
pub(crate) enum VolumeSliderKind {
    Sfx,
    Music,
}

#[derive(Component)]
pub(crate) struct VolumeSliderLabel(pub(crate) VolumeSliderKind);

#[derive(Component)]
pub(crate) struct VolumeSliderFill(pub(crate) VolumeSliderKind);

#[derive(Component)]
pub(crate) struct MusicTrackStatusText;

#[derive(Component)]
pub(crate) struct MusicTrackTitleText;

fn volume_label(locale: Locale, kind: VolumeSliderKind, volume: f32) -> String {
    let percent = volume * 100.0;
    match (locale, kind) {
        (Locale::Es, VolumeSliderKind::Sfx) => format!("Efectos de sonido: {percent:.0} %"),
        (Locale::En, VolumeSliderKind::Sfx) => format!("Sound effects: {percent:.0} %"),
        (Locale::Es, VolumeSliderKind::Music) => format!("Música: {percent:.0} %"),
        (Locale::En, VolumeSliderKind::Music) => format!("Music: {percent:.0} %"),
    }
}

fn music_status_label(locale: Locale, playing: bool, position: &str) -> String {
    let status = match (locale, playing) {
        (Locale::Es, true) => "Reproduciendo",
        (Locale::Es, false) => "Detenido",
        (Locale::En, true) => "Playing",
        (Locale::En, false) => "Stopped",
    };
    format!("{status} · {position}")
}

fn music_play_button_label(locale: Locale, playing: bool) -> &'static str {
    match (locale, playing) {
        (Locale::Es, true) => "Detener",
        (Locale::Es, false) => "Reproducir",
        (Locale::En, true) => "Stop",
        (Locale::En, false) => "Play",
    }
}

fn spawn_volume_slider(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    kind: VolumeSliderKind,
) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        },))
        .with_children(|col| {
            col.spawn((
                VolumeSliderLabel(kind),
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            col.spawn((
                Button,
                kind,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(18.0),
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::ZERO,
                    ..default()
                },
                BackgroundColor(SLIDER_TRACK),
                BorderColor::all(BTN_BORDER),
                Interaction::default(),
                RelativeCursorPosition::default(),
                BuildMenuUi,
            ))
            .with_children(|track| {
                track.spawn((
                    VolumeSliderFill(kind),
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(SLIDER_FILL),
                ));
            });
        });
}

fn spawn_jukebox_section(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(10.0)),
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|section| {
            section.spawn((
                Text::new("— Música —"),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            section.spawn((
                MusicTrackStatusText,
                // Estos dos textos se materializan con MusicState; no deben
                // conservar una clave estática que pueda pisar el título real.
                Text::new("—"),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            section.spawn((
                MusicTrackTitleText,
                Text::new("—"),
                window_text_font(asset_server, UiFontRole::Body),
                TextColor(WINDOW_TEXT),
            ));
            section
                .spawn((Node {
                    width: Val::Percent(100.0),
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(3.0),
                    row_gap: Val::Px(3.0),
                    ..default()
                },))
                .with_children(|row| {
                    for playlist in MusicPlaylist::CHOICES {
                        row.spawn((
                            Button,
                            MusicWindowButton::Playlist(playlist),
                            Node {
                                min_width: Val::Px(52.0),
                                height: Val::Px(22.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                padding: UiRect::horizontal(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(BTN_BG),
                            BorderColor::all(BTN_BORDER),
                            Interaction::default(),
                            BuildMenuUi,
                            children![(
                                Text::new(playlist.label()),
                                window_text_font(asset_server, UiFontRole::Caption),
                                TextColor(WINDOW_TEXT),
                            )],
                        ));
                    }
                });
            section
                .spawn((Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                },))
                .with_children(|row| {
                    for (label, button) in [
                        ("◀ Ant.", MusicWindowButton::Prev),
                        ("Reproducir", MusicWindowButton::PlayStop),
                        ("Sig. ▶", MusicWindowButton::Next),
                    ] {
                        let label = if button == MusicWindowButton::PlayStop {
                            "—"
                        } else {
                            label
                        };
                        row.spawn((
                            Button,
                            button,
                            Node {
                                flex_grow: 1.0,
                                height: Val::Px(26.0),
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
                });
        });
}

pub(crate) fn setup_sound_music_window(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing: Query<&FloatingWindow>,
) {
    if existing
        .iter()
        .any(|w| w.id == FloatingWindowId::SoundMusic)
    {
        return;
    }
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::SoundMusic,
        "Sonido y música",
        TITLE_BROWN,
        Vec2::new(300.0, 72.0),
        320.0,
    );
    commands.entity(content).with_children(|body| {
        spawn_volume_slider(
            body,
            asset_server,
            "Efectos de sonido: 0 %",
            VolumeSliderKind::Sfx,
        );
        spawn_volume_slider(body, asset_server, "Música: 0 %", VolumeSliderKind::Music);
        for (label, button) in [
            ("Vehículos", AudioSettingsButton::Vehicle),
            ("Ambiente", AudioSettingsButton::Ambient),
            ("Desastres", AudioSettingsButton::Disaster),
            ("Confirmación", AudioSettingsButton::Confirm),
            ("Clic toolbar", AudioSettingsButton::ClickBeep),
        ] {
            body.spawn((
                Button,
                button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(22.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    margin: UiRect::top(Val::Px(3.0)),
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
        spawn_jukebox_section(body, asset_server);
    });
}

fn volume_from_cursor(rel: &RelativeCursorPosition) -> Option<f32> {
    let pos = rel.normalized?;
    Some((pos.x + 0.5).clamp(0.0, 1.0))
}

pub(crate) fn handle_sound_music_toolbar_button(
    q: Query<&Interaction, (Changed<Interaction>, With<SoundMusicToolbarButton>)>,
    mut state: ResMut<SoundMusicWindowState>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            state.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_sound_music_window(
    state: Res<SoundMusicWindowState>,
    screen: Res<State<ClientScreen>>,
    hud: Res<SimHudControls>,
    music: Res<MusicState>,
    prefs: Res<ClientPreferences>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility, &mut GlobalZIndex)>,
    mut labels: Query<(&VolumeSliderLabel, &mut Text)>,
    mut fills: Query<(&VolumeSliderFill, &mut Node)>,
    mut toggles: Query<
        (&AudioSettingsButton, &mut BorderColor),
        (Without<FloatingWindow>, Without<MusicWindowButton>),
    >,
    mut status_q: Query<
        &mut Text,
        (
            With<MusicTrackStatusText>,
            Without<MusicTrackTitleText>,
            Without<VolumeSliderLabel>,
        ),
    >,
    mut title_q: Query<
        &mut Text,
        (
            With<MusicTrackTitleText>,
            Without<MusicTrackStatusText>,
            Without<VolumeSliderLabel>,
            Without<Button>,
        ),
    >,
    mut playlist_btns: Query<
        (&MusicWindowButton, &mut BorderColor),
        (
            With<Button>,
            Without<FloatingWindow>,
            Without<AudioSettingsButton>,
        ),
    >,
    mut play_btn: Query<
        (&MusicWindowButton, &mut Text),
        (
            With<Button>,
            Without<MusicTrackStatusText>,
            Without<MusicTrackTitleText>,
            Without<VolumeSliderLabel>,
        ),
    >,
) {
    let Some((_, mut vis, mut z)) = root_q
        .iter_mut()
        .find(|(w, _, _)| w.id == FloatingWindowId::SoundMusic)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;
    // El menú principal usa GlobalZIndex(3000); sin esto la ventana queda detrás y no recibe clics.
    if *screen.get() == ClientScreen::MainMenu {
        z.0 = z.0.max(MENU_OVERLAY_WINDOW_Z);
    }
    let locale = prefs.locale();

    for (label, mut text) in &mut labels {
        **text = match label.0 {
            VolumeSliderKind::Sfx => volume_label(locale, label.0, hud.sfx_volume),
            VolumeSliderKind::Music => volume_label(locale, label.0, hud.music_volume),
        };
    }
    for (fill, mut node) in &mut fills {
        let pct = match fill.0 {
            VolumeSliderKind::Sfx => hud.sfx_volume.clamp(0.0, 1.0) * 100.0,
            VolumeSliderKind::Music => hud.music_volume.clamp(0.0, 1.0) * 100.0,
        };
        node.width = Val::Percent(pct);
    }

    for (button, mut border) in &mut toggles {
        let on = match button {
            AudioSettingsButton::Vehicle => hud.sound_vehicle,
            AudioSettingsButton::Ambient => hud.sound_ambient,
            AudioSettingsButton::Disaster => hud.sound_disaster,
            AudioSettingsButton::Confirm => hud.sound_confirm,
            AudioSettingsButton::ClickBeep => hud.sound_click_beep,
        };
        *border = if on {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }

    for mut text in &mut status_q {
        **text = music_status_label(locale, music.playing, &music.track_position_label());
    }
    for mut text in &mut title_q {
        **text = music.current_track_title().to_string();
    }
    for (button, mut border) in &mut playlist_btns {
        if let MusicWindowButton::Playlist(pl) = button {
            *border = if *pl == music.playlist {
                BorderColor::all(BTN_ACTIVE)
            } else {
                BorderColor::all(BTN_BORDER)
            };
        }
    }
    for (button, mut text) in &mut play_btn {
        if *button == MusicWindowButton::PlayStop {
            **text = music_play_button_label(locale, music.playing).to_owned();
        }
    }
}

pub(crate) fn handle_volume_sliders(
    mut hud: ResMut<SimHudControls>,
    mouse: Res<ButtonInput<MouseButton>>,
    sliders: Query<(&VolumeSliderKind, &Interaction, &RelativeCursorPosition)>,
) {
    let dragging = mouse.pressed(MouseButton::Left);
    for (kind, interaction, rel) in &sliders {
        let active = matches!(*interaction, Interaction::Pressed | Interaction::Hovered)
            && dragging
            || *interaction == Interaction::Pressed;
        if !active {
            continue;
        }
        let Some(v) = volume_from_cursor(rel) else {
            continue;
        };
        match kind {
            VolumeSliderKind::Sfx => hud.sfx_volume = v,
            VolumeSliderKind::Music => hud.music_volume = v,
        }
    }
}

pub(crate) fn handle_audio_settings_buttons(
    mut hud: ResMut<SimHudControls>,
    buttons: Query<
        (&Interaction, &AudioSettingsButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<MusicWindowButton>,
        ),
    >,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            AudioSettingsButton::Vehicle => hud.sound_vehicle = !hud.sound_vehicle,
            AudioSettingsButton::Ambient => hud.sound_ambient = !hud.sound_ambient,
            AudioSettingsButton::Disaster => hud.sound_disaster = !hud.sound_disaster,
            AudioSettingsButton::Confirm => hud.sound_confirm = !hud.sound_confirm,
            AudioSettingsButton::ClickBeep => hud.sound_click_beep = !hud.sound_click_beep,
        }
    }
}

pub(crate) fn handle_music_window_buttons(
    mut commands: Commands,
    screen: Res<State<ClientScreen>>,
    hud: Res<SimHudControls>,
    mut music: ResMut<MusicState>,
    players: Query<Entity, With<MusicPlayer>>,
    buttons: Query<
        (&Interaction, &MusicWindowButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<AudioSettingsButton>,
        ),
    >,
) {
    let screen = *screen.get();
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            MusicWindowButton::PlayStop => {
                music_toggle_playback(&mut commands, players, &mut music, &hud, screen);
            }
            MusicWindowButton::Prev => {
                music_skip(&mut commands, players, &mut music, &hud, screen, -1);
            }
            MusicWindowButton::Next => {
                music_skip(&mut commands, players, &mut music, &hud, screen, 1);
            }
            MusicWindowButton::Playlist(playlist) => {
                music_apply_playlist(&mut commands, players, &mut music, &hud, screen, *playlist);
            }
        }
    }
}

pub(crate) fn sound_music_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<SoundMusicWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::SoundMusic {
            state.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{VolumeSliderKind, music_play_button_label, music_status_label, volume_label};
    use crate::i18n::Locale;

    #[test]
    fn music_chrome_follows_locale_without_touching_track_data() {
        assert_eq!(
            volume_label(Locale::En, VolumeSliderKind::Sfx, 0.42),
            "Sound effects: 42 %"
        );
        assert_eq!(
            volume_label(Locale::Es, VolumeSliderKind::Music, 0.5),
            "Música: 50 %"
        );
        assert_eq!(
            music_status_label(Locale::En, true, "2 / 17"),
            "Playing · 2 / 17"
        );
        assert_eq!(
            music_status_label(Locale::Es, false, "2 / 17"),
            "Detenido · 2 / 17"
        );
        assert_eq!(music_play_button_label(Locale::En, false), "Play");
        assert_eq!(music_play_button_label(Locale::Es, true), "Detener");
    }
}
