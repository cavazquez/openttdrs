//! SFX del HUD: error de construcción, colocación OK e ingreso de carga.

use bevy::prelude::*;

/// Tipo de efecto corto del HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudSfxKind {
    Error,
    BuildOk,
    Income,
}

/// Reproducir un efecto del HUD.
#[derive(Message)]
pub(crate) struct PlayHudSfx(pub HudSfxKind);

#[derive(Resource, Default)]
pub(crate) struct HudSfxHandles {
    pub error: Option<Handle<AudioSource>>,
    pub build_ok: Option<Handle<AudioSource>>,
    pub income: Option<Handle<AudioSource>>,
}

pub(crate) fn flush_hud_sfx(
    mut hud: ResMut<super::HudBuildFeedback>,
    mut writer: MessageWriter<PlayHudSfx>,
) {
    if hud.pending_soft_ping {
        hud.pending_soft_ping = false;
        writer.write(PlayHudSfx(HudSfxKind::Error));
    }
    if hud.pending_build_ok_ping {
        hud.pending_build_ok_ping = false;
        writer.write(PlayHudSfx(HudSfxKind::BuildOk));
    }
    if hud.pending_income_ping {
        hud.pending_income_ping = false;
        writer.write(PlayHudSfx(HudSfxKind::Income));
    }
}

pub(crate) fn load_hud_sfx(
    mut handles: ResMut<HudSfxHandles>,
    asset_server: Res<AssetServer>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    handles.error = Some(asset_server.load("assets/sounds/hud_soft.wav"));
    handles.build_ok = Some(asset_server.load("assets/sounds/build_ok.wav"));
    handles.income = Some(asset_server.load("assets/sounds/income.wav"));
    *done = true;
}

fn play_handle(commands: &mut Commands, handle: Option<&Handle<AudioSource>>, volume: f32) {
    if let Some(h) = handle {
        commands.spawn((
            AudioPlayer::new(h.clone()),
            PlaybackSettings::ONCE.with_volume(bevy::audio::Volume::Linear(volume.clamp(0.0, 1.0))),
        ));
    }
}

pub(crate) fn play_hud_sfx(
    mut commands: Commands,
    mut reader: MessageReader<PlayHudSfx>,
    sound: Res<HudSfxHandles>,
    hud: Res<super::SimHudControls>,
) {
    let volume = hud.sfx_volume.clamp(0.0, 1.0);
    for PlayHudSfx(kind) in reader.read() {
        let handle = match kind {
            HudSfxKind::Error => sound.error.as_ref(),
            HudSfxKind::BuildOk => sound.build_ok.as_ref().or(sound.error.as_ref()),
            HudSfxKind::Income => sound.income.as_ref().or(sound.error.as_ref()),
        };
        play_handle(&mut commands, handle, volume);
    }
}
