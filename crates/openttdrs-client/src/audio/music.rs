//! Música de fondo (OpenMSX pre-decodificado a OGG).

use bevy::prelude::*;

use crate::audio::ClientAssetRoot;
use crate::bevy_app::UpdateSet;
use crate::state::ClientScreen;
use crate::ui::SimHudControls;

/// Marca la entidad de audio de música (theme o playlist) para distinguirla
/// de los SFX, que comparten el componente `AudioPlayer`.
#[derive(Component)]
pub(crate) struct MusicPlayer;

#[derive(Resource)]
pub(crate) struct MusicState {
    pub playing: bool,
    pub track_index: usize,
    handles: Vec<Handle<AudioSource>>,
    theme: Option<Handle<AudioSource>>,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            playing: true,
            track_index: 0,
            handles: Vec::new(),
            theme: None,
        }
    }
}

pub(crate) struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MusicState>()
            .add_systems(Startup, load_music_tracks)
            .add_systems(OnExit(ClientScreen::MainMenu), stop_menu_theme)
            .add_systems(
                Update,
                (play_menu_theme, sync_music_volume, advance_playlist)
                    .chain()
                    .in_set(UpdateSet::Status),
            );
    }
}

const MENU_THEME: &str = "assets/music/theme.ogg";

/// Playlist en partida: slots `openmsx.obm` (v0.4.2), orden old → new → ezy.
const PLAYLIST: &[&str] = &[
    "assets/music/old_00.ogg",
    "assets/music/old_01.ogg",
    "assets/music/old_02.ogg",
    "assets/music/old_03.ogg",
    "assets/music/old_04.ogg",
    "assets/music/old_05.ogg",
    "assets/music/old_06.ogg",
    "assets/music/old_07.ogg",
    "assets/music/old_08.ogg",
    "assets/music/old_09.ogg",
    "assets/music/new_00.ogg",
    "assets/music/new_01.ogg",
    "assets/music/new_02.ogg",
    "assets/music/new_03.ogg",
    "assets/music/new_04.ogg",
    "assets/music/new_05.ogg",
    "assets/music/new_06.ogg",
    "assets/music/new_07.ogg",
    "assets/music/new_08.ogg",
    "assets/music/new_09.ogg",
    "assets/music/ezy_00.ogg",
    "assets/music/ezy_01.ogg",
    "assets/music/ezy_02.ogg",
    "assets/music/ezy_03.ogg",
    "assets/music/ezy_04.ogg",
    "assets/music/ezy_05.ogg",
    "assets/music/ezy_06.ogg",
];

fn load_music_tracks(
    root: Res<ClientAssetRoot>,
    mut music: ResMut<MusicState>,
    asset_server: Res<AssetServer>,
) {
    if music.theme.is_some() || !music.handles.is_empty() {
        return;
    }
    if root.asset_file_exists(MENU_THEME) {
        music.theme = Some(asset_server.load(MENU_THEME));
    }
    music.handles = PLAYLIST
        .iter()
        .filter(|path| root.asset_file_exists(path))
        .map(|path| asset_server.load(*path))
        .collect();
}

fn play_menu_theme(
    mut commands: Commands,
    screen: Res<State<ClientScreen>>,
    hud: Res<SimHudControls>,
    music: Res<MusicState>,
    music_players: Query<(), With<MusicPlayer>>,
) {
    if *screen.get() != ClientScreen::MainMenu || !music_players.is_empty() {
        return;
    }
    let Some(theme) = music.theme.as_ref() else {
        return;
    };
    let vol = hud.music_volume.clamp(0.0, 1.0);
    commands.spawn((
        MusicPlayer,
        AudioPlayer::new(theme.clone()),
        PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(vol)),
    ));
}

/// Detiene el theme del menú al entrar en partida para que no bloquee la playlist.
fn stop_menu_theme(mut commands: Commands, players: Query<Entity, With<MusicPlayer>>) {
    for entity in &players {
        commands.entity(entity).despawn();
    }
}

/// Aplica el volumen de música en caliente a la pista activa.
///
/// Bevy no propaga cambios en [`PlaybackSettings`] a un sink ya creado; hay que
/// usar [`AudioSink::set_volume`].
fn sync_music_volume(
    hud: Res<SimHudControls>,
    global_volume: Res<GlobalVolume>,
    mut players: Query<&mut AudioSink, With<MusicPlayer>>,
) {
    let target =
        bevy::audio::Volume::Linear(hud.music_volume.clamp(0.0, 1.0)) * global_volume.volume;
    for mut sink in &mut players {
        if sink.volume() != target {
            sink.set_volume(target);
        }
    }
}

/// Avanza playlist en partida cuando no hay pista de música activa.
fn advance_playlist(
    mut commands: Commands,
    screen: Res<State<ClientScreen>>,
    mut music: ResMut<MusicState>,
    hud: Res<SimHudControls>,
    players: Query<(), With<MusicPlayer>>,
) {
    if *screen.get() != ClientScreen::InGame || !music.playing {
        return;
    }
    if !players.is_empty() {
        return;
    }
    if music.handles.is_empty() {
        return;
    }
    let idx = music.track_index % music.handles.len();
    music.track_index = music.track_index.wrapping_add(1);
    let vol = hud.music_volume.clamp(0.0, 1.0);
    commands.spawn((
        MusicPlayer,
        AudioPlayer::new(music.handles[idx].clone()),
        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(vol)),
    ));
}
