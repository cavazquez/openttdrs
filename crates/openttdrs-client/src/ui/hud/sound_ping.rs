//! SFX del HUD: error de construcción, colocación OK e ingreso de carga.

use bevy::prelude::*;

/// Tipo de efecto corto del HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudSfxKind {
    /// Clic en botones del toolbar (`SND_15_BEEP`, `sound.click_beep`).
    ClickBeep,
    Error,
    Income,
    NewsTicker,
    NewsApplause,
    NewsChime,
}

/// Reproducir un efecto del HUD.
#[derive(Message)]
pub(crate) struct PlayHudSfx(pub HudSfxKind);

#[derive(Resource, Default)]
pub(crate) struct HudSfxHandles {
    pub error: Option<Handle<AudioSource>>,
    pub build_ok: Option<Handle<AudioSource>>,
    pub income: Option<Handle<AudioSource>>,
    pub news_ticker: Option<Handle<AudioSource>>,
    pub news_applause: Option<Handle<AudioSource>>,
    pub news_chime: Option<Handle<AudioSource>>,
}

pub(crate) fn flush_hud_sfx(
    mut hud: ResMut<super::HudBuildFeedback>,
    mut writer: MessageWriter<PlayHudSfx>,
) {
    if hud.pending_soft_ping {
        hud.pending_soft_ping = false;
        writer.write(PlayHudSfx(HudSfxKind::Error));
    }
    if hud.pending_income_ping {
        hud.pending_income_ping = false;
        writer.write(PlayHudSfx(HudSfxKind::Income));
    }
    if hud.pending_news_ticker {
        hud.pending_news_ticker = false;
        writer.write(PlayHudSfx(HudSfxKind::NewsTicker));
    }
    if hud.pending_news_applause {
        hud.pending_news_applause = false;
        writer.write(PlayHudSfx(HudSfxKind::NewsApplause));
    }
    if hud.pending_news_chime {
        hud.pending_news_chime = false;
        writer.write(PlayHudSfx(HudSfxKind::NewsChime));
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
    handles.news_ticker = Some(asset_server.load("assets/sounds/news_ticker.wav"));
    handles.news_applause = Some(asset_server.load("assets/sounds/news_applause.wav"));
    handles.news_chime = Some(asset_server.load("assets/sounds/news_chime.wav"));
    *done = true;
}

fn play_handle(commands: &mut Commands, handle: Option<&Handle<AudioSource>>, volume: f32) {
    if let Some(h) = handle {
        commands.spawn((
            AudioPlayer::new(h.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(bevy::audio::Volume::Linear(volume.clamp(0.0, 1.0))),
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
            HudSfxKind::ClickBeep | HudSfxKind::Error => sound.error.as_ref(),
            HudSfxKind::Income => sound.income.as_ref().or(sound.error.as_ref()),
            HudSfxKind::NewsTicker => sound.news_ticker.as_ref().or(sound.error.as_ref()),
            HudSfxKind::NewsApplause => sound
                .news_applause
                .as_ref()
                .or(sound.income.as_ref())
                .or(sound.error.as_ref()),
            HudSfxKind::NewsChime => sound
                .news_chime
                .as_ref()
                .or(sound.build_ok.as_ref())
                .or(sound.error.as_ref()),
        };
        play_handle(&mut commands, handle, volume);
    }
}
