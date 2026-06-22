//! Pitido suave al rechazar construcción (p. ej. parada sin vecino de transporte).

use bevy::prelude::*;

/// Mensaje: reproducir sonido de aviso en HUD (un solo ping corto).
#[derive(Message)]
pub(crate) struct PlayHudSoftPing;

#[derive(Resource, Default)]
pub(crate) struct HudSoftPingHandle(pub Option<Handle<AudioSource>>);

pub(crate) fn flush_hud_soft_ping(
    mut hud: ResMut<super::HudBuildFeedback>,
    mut writer: MessageWriter<PlayHudSoftPing>,
) {
    if hud.pending_soft_ping {
        hud.pending_soft_ping = false;
        writer.write(PlayHudSoftPing);
    }
}

pub(crate) fn load_hud_soft_ping(
    mut handles: ResMut<HudSoftPingHandle>,
    asset_server: Res<AssetServer>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    handles.0 = Some(asset_server.load("assets/sounds/hud_soft.wav"));
    *done = true;
}

pub(crate) fn play_hud_soft_ping(
    mut commands: Commands,
    mut reader: MessageReader<PlayHudSoftPing>,
    sound: Res<HudSoftPingHandle>,
    hud: Res<super::SimHudControls>,
) {
    for _ in reader.read() {
        if let Some(h) = sound.0.as_ref() {
            commands.spawn((
                AudioPlayer::new(h.clone()),
                PlaybackSettings::ONCE
                    .with_volume(bevy::audio::Volume::Linear(hud.sfx_volume.clamp(0.0, 1.0))),
            ));
        }
    }
}
