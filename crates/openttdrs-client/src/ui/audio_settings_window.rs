//! Ventana de volumen y flags de sonido/música (fase A3).

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

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
const SLIDER_TRACK: Color = Color::srgb(0.18, 0.14, 0.10);
const SLIDER_FILL: Color = Color::srgb(0.52, 0.68, 0.38);

#[derive(Resource, Default)]
pub(crate) struct AudioSettingsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum AudioSettingsButton {
    Vehicle,
    Ambient,
    Disaster,
    Confirm,
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

pub(crate) fn setup_audio_settings_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::AudioSettings,
        "Audio",
        TITLE_BROWN,
        Vec2::new(280.0, 200.0),
        300.0,
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
    });
}

fn volume_from_cursor(rel: &RelativeCursorPosition) -> Option<f32> {
    let pos = rel.normalized?;
    Some((pos.x + 0.5).clamp(0.0, 1.0))
}

pub(crate) fn sync_audio_settings_window(
    state: Res<AudioSettingsWindowState>,
    hud: Res<SimHudControls>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut labels: Query<(&VolumeSliderLabel, &mut Text)>,
    mut fills: Query<(&VolumeSliderFill, &mut Node)>,
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

    for (label, mut text) in &mut labels {
        **text = match label.0 {
            VolumeSliderKind::Sfx => {
                format!("Efectos de sonido: {:.0} %", hud.sfx_volume * 100.0)
            }
            VolumeSliderKind::Music => {
                format!("Música: {:.0} %", hud.music_volume * 100.0)
            }
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
        };
        *border = if on {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
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
    buttons: Query<(&Interaction, &AudioSettingsButton), (Changed<Interaction>, With<Button>)>,
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
