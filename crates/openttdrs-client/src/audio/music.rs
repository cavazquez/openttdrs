//! Música de fondo (OpenMSX pre-decodificado a OGG).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::state::ClientScreen;
use crate::ui::SimHudControls;

#[derive(Resource)]
pub(crate) struct MusicState {
    pub playing: bool,
    pub track_index: usize,
    handles: Vec<Handle<AudioSource>>,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            playing: true,
            track_index: 0,
            handles: Vec::new(),
        }
    }
}

pub(crate) struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MusicState>()
            .add_systems(Startup, load_music_tracks)
            .add_systems(
                Update,
                (play_menu_theme, advance_playlist.in_set(UpdateSet::Status)),
            );
    }
}

const MENU_THEME: &str = "assets/music/theme.ogg";
const PLAYLIST: &[&str] = &[
    "assets/music/old_01.ogg",
    "assets/music/old_02.ogg",
    "assets/music/new_01.ogg",
    "assets/music/ezy_01.ogg",
];

fn load_music_tracks(mut music: ResMut<MusicState>, asset_server: Res<AssetServer>) {
    if !music.handles.is_empty() {
        return;
    }
    music.handles = PLAYLIST.iter().map(|p| asset_server.load(*p)).collect();
}

fn play_menu_theme(
    mut commands: Commands,
    screen: Res<State<ClientScreen>>,
    mut played: Local<bool>,
    asset_server: Res<AssetServer>,
    hud: Res<SimHudControls>,
) {
    if *played || *screen.get() != ClientScreen::MainMenu {
        return;
    }
    *played = true;
    let vol = hud.music_volume.clamp(0.0, 1.0);
    commands.spawn((
        AudioPlayer::new(asset_server.load(MENU_THEME)),
        PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(vol)),
    ));
}

/// Avanza playlist en partida cuando no hay entidad de música activa.
fn advance_playlist(
    mut commands: Commands,
    screen: Res<State<ClientScreen>>,
    music: Res<MusicState>,
    hud: Res<SimHudControls>,
    players: Query<&AudioPlayer>,
) {
    if *screen.get() != ClientScreen::InGame || !music.playing || music.handles.is_empty() {
        return;
    }
    if !players.is_empty() {
        return;
    }
    let idx = music.track_index % music.handles.len();
    let vol = hud.music_volume.clamp(0.0, 1.0);
    commands.spawn((
        AudioPlayer::new(music.handles[idx].clone()),
        PlaybackSettings::ONCE.with_volume(bevy::audio::Volume::Linear(vol)),
    ));
}
