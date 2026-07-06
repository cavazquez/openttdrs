//! Ventana de volumen y flags de sonido/música (fase A3).

use bevy::prelude::*;

use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::SimHudControls;
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);
const VOLUME_STEP: f32 = 0.05;

#[derive(Resource, Default)]
pub(crate) struct AudioSettingsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum AudioSettingsButton {
    SfxDown,
    SfxUp,
    MusicDown,
    MusicUp,
    ToggleVehicle,
    ToggleAmbient,
    ToggleDisaster,
    ToggleConfirm,
}

#[derive(Component)]
pub(crate) struct AudioSettingsBodyText;

pub(crate) fn setup_audio_settings_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::AudioSettings,
        "Audio",
        TITLE_BROWN,
        Vec2::new(280.0, 120.0),
        320.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            AudioSettingsBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        for (label, button) in [
            ("Vehículos", AudioSettingsButton::ToggleVehicle),
            ("Ambiente", AudioSettingsButton::ToggleAmbient),
            ("Desastres", AudioSettingsButton::ToggleDisaster),
            ("Confirmación", AudioSettingsButton::ToggleConfirm),
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
        body.spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        },))
            .with_children(|row| {
                for (label, action) in [
                    ("SFX −", AudioSettingsButton::SfxDown),
                    ("SFX +", AudioSettingsButton::SfxUp),
                    ("Música −", AudioSettingsButton::MusicDown),
                    ("Música +", AudioSettingsButton::MusicUp),
                ] {
                    row.spawn((
                        Button,
                        action,
                        Node {
                            flex_grow: 1.0,
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
                            Text::new(label),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
    });
}

pub(crate) fn sync_audio_settings_window(
    state: Res<AudioSettingsWindowState>,
    hud: Res<SimHudControls>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut body_q: Query<&mut Text, With<AudioSettingsBodyText>>,
    mut toggles: Query<(&AudioSettingsButton, &mut BorderColor), Without<FloatingWindow>>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::AudioSettings)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    if let Ok(mut body) = body_q.single_mut() {
        **body = format!(
            "Volumen efectos: {:.0} %\nVolumen música: {:.0} %",
            hud.sfx_volume * 100.0,
            hud.music_volume * 100.0,
        );
    }

    for (button, mut border) in &mut toggles {
        let on = match button {
            AudioSettingsButton::ToggleVehicle => hud.sound_vehicle,
            AudioSettingsButton::ToggleAmbient => hud.sound_ambient,
            AudioSettingsButton::ToggleDisaster => hud.sound_disaster,
            AudioSettingsButton::ToggleConfirm => hud.sound_confirm,
            _ => continue,
        };
        *border = if on {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }
}

pub(crate) fn handle_audio_settings_buttons(
    mut hud: ResMut<SimHudControls>,
    buttons: Query<(&Interaction, &AudioSettingsButton), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            AudioSettingsButton::SfxDown => {
                hud.sfx_volume = (hud.sfx_volume - VOLUME_STEP).clamp(0.0, 1.0);
            }
            AudioSettingsButton::SfxUp => {
                hud.sfx_volume = (hud.sfx_volume + VOLUME_STEP).clamp(0.0, 1.0);
            }
            AudioSettingsButton::MusicDown => {
                hud.music_volume = (hud.music_volume - VOLUME_STEP).clamp(0.0, 1.0);
            }
            AudioSettingsButton::MusicUp => {
                hud.music_volume = (hud.music_volume + VOLUME_STEP).clamp(0.0, 1.0);
            }
            AudioSettingsButton::ToggleVehicle => hud.sound_vehicle = !hud.sound_vehicle,
            AudioSettingsButton::ToggleAmbient => hud.sound_ambient = !hud.sound_ambient,
            AudioSettingsButton::ToggleDisaster => hud.sound_disaster = !hud.sound_disaster,
            AudioSettingsButton::ToggleConfirm => hud.sound_confirm = !hud.sound_confirm,
        }
    }
}

pub(crate) fn audio_settings_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<AudioSettingsWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::AudioSettings {
            state.open = false;
        }
    }
}
