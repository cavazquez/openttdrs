//! Música de fondo (OpenMSX pre-decodificado a OGG) y jukebox.

use bevy::prelude::*;

use crate::audio::ClientAssetRoot;
use crate::bevy_app::UpdateSet;
use crate::state::ClientScreen;
use crate::ui::SimHudControls;

/// Marca la entidad de audio de música (theme o playlist) para distinguirla
/// de los SFX, que comparten el componente `AudioPlayer`.
#[derive(Component)]
pub(crate) struct MusicPlayer;

/// Playlist estilo OpenTTD (`music_gui.cpp`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum MusicPlaylist {
    #[default]
    All,
    Old,
    New,
    Ezy,
    Theme,
}

impl MusicPlaylist {
    pub(crate) const CHOICES: [Self; 5] = [Self::All, Self::Old, Self::New, Self::Ezy, Self::Theme];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "Todas",
            Self::Old => "Old",
            Self::New => "New",
            Self::Ezy => "Ezy",
            Self::Theme => "Theme",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrackCategory {
    Theme,
    Old,
    New,
    Ezy,
}

struct MusicTrackDef {
    path: &'static str,
    title: &'static str,
    category: TrackCategory,
}

/// Catálogo OpenMSX 0.4.2 (theme + old/new/ezy × 10).
const MUSIC_CATALOG: &[MusicTrackDef] = &[
    MusicTrackDef {
        path: "assets/music/theme.ogg",
        title: "TT Theme",
        category: TrackCategory::Theme,
    },
    MusicTrackDef {
        path: "assets/music/old_00.ogg",
        title: "Keep on Rolling",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_01.ogg",
        title: "Song III",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_02.ogg",
        title: "Modern Motion",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_03.ogg",
        title: "Busy Schedule",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_04.ogg",
        title: "The Fast Route",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_05.ogg",
        title: "Song IV",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_06.ogg",
        title: "Train Filled with Cash",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_07.ogg",
        title: "Flying Scotsman",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_08.ogg",
        title: "Chugga Chugga",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/old_09.ogg",
        title: "The Hobo",
        category: TrackCategory::Old,
    },
    MusicTrackDef {
        path: "assets/music/new_00.ogg",
        title: "Ultimate Run",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_01.ogg",
        title: "Midnight Snow Run",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_02.ogg",
        title: "Run for Your Life",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_03.ogg",
        title: "Coconut Run",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_04.ogg",
        title: "Harp Harmony",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_05.ogg",
        title: "Mighty Giant Run",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_06.ogg",
        title: "Wood Whistles",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_07.ogg",
        title: "Linn's Basket",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_08.ogg",
        title: "Relax Song",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/new_09.ogg",
        title: "Chemistry Lab",
        category: TrackCategory::New,
    },
    MusicTrackDef {
        path: "assets/music/ezy_00.ogg",
        title: "Boogi Marabi",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_01.ogg",
        title: "5432 Gone",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_02.ogg",
        title: "Moo",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_03.ogg",
        title: "Say What",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_04.ogg",
        title: "Be Sharp",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_05.ogg",
        title: "Careless Perc",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_06.ogg",
        title: "Mosey Along",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_07.ogg",
        title: "City Blues",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_08.ogg",
        title: "No Work Song",
        category: TrackCategory::Ezy,
    },
    MusicTrackDef {
        path: "assets/music/ezy_09.ogg",
        title: "Slow n Easy",
        category: TrackCategory::Ezy,
    },
];

#[derive(Resource)]
pub(crate) struct MusicState {
    pub playing: bool,
    pub playlist: MusicPlaylist,
    pub track_index: usize,
    catalog_handles: Vec<Option<Handle<AudioSource>>>,
    active_catalog_indices: Vec<usize>,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            playing: true,
            playlist: MusicPlaylist::All,
            track_index: 0,
            catalog_handles: Vec::new(),
            active_catalog_indices: Vec::new(),
        }
    }
}

impl MusicState {
    pub(crate) fn current_track_title(&self) -> &'static str {
        let Some(&cat_idx) = self.active_catalog_indices.get(self.track_index) else {
            return "(sin pistas)";
        };
        MUSIC_CATALOG
            .get(cat_idx)
            .map(|t| t.title)
            .unwrap_or("(desconocida)")
    }

    pub(crate) fn track_position_label(&self) -> String {
        let total = self.active_catalog_indices.len();
        if total == 0 {
            return "0 / 0".to_string();
        }
        format!("{} / {}", self.track_index + 1, total)
    }

    fn rebuild_active_indices(&mut self) {
        self.active_catalog_indices = MUSIC_CATALOG
            .iter()
            .enumerate()
            .filter(|(i, track)| {
                self.catalog_handles
                    .get(*i)
                    .and_then(|h| h.as_ref())
                    .is_some()
                    && playlist_matches(self.playlist, track.category)
            })
            .map(|(i, _)| i)
            .collect();
        if self.track_index >= self.active_catalog_indices.len() {
            self.track_index = 0;
        }
    }

    pub(crate) fn set_playlist(&mut self, playlist: MusicPlaylist) {
        self.playlist = playlist;
        self.rebuild_active_indices();
    }
}

fn playlist_matches(playlist: MusicPlaylist, category: TrackCategory) -> bool {
    match playlist {
        MusicPlaylist::All => true,
        MusicPlaylist::Old => category == TrackCategory::Old,
        MusicPlaylist::New => category == TrackCategory::New,
        MusicPlaylist::Ezy => category == TrackCategory::Ezy,
        MusicPlaylist::Theme => category == TrackCategory::Theme,
    }
}

pub(crate) struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MusicState>()
            .add_systems(Startup, load_music_tracks)
            .add_systems(OnExit(ClientScreen::MainMenu), stop_music_on_leave_menu)
            .add_systems(OnEnter(ClientScreen::InGame), start_ingame_music)
            .add_systems(
                Update,
                (play_menu_theme, sync_music_volume, advance_playlist)
                    .chain()
                    .in_set(UpdateSet::Status),
            );
    }
}

fn load_music_tracks(
    root: Res<ClientAssetRoot>,
    mut music: ResMut<MusicState>,
    asset_server: Res<AssetServer>,
) {
    if !music.catalog_handles.is_empty() {
        return;
    }
    music.catalog_handles = MUSIC_CATALOG
        .iter()
        .map(|track| {
            if root.asset_file_exists(track.path) {
                Some(asset_server.load(track.path))
            } else {
                None
            }
        })
        .collect();
    music.rebuild_active_indices();
}

fn play_menu_theme(
    mut commands: Commands,
    screen: Res<State<ClientScreen>>,
    hud: Res<SimHudControls>,
    music: Res<MusicState>,
    music_players: Query<(), With<MusicPlayer>>,
) {
    if *screen.get() != ClientScreen::MainMenu || !music.playing || !music_players.is_empty() {
        return;
    }
    let Some(theme_idx) = MUSIC_CATALOG
        .iter()
        .position(|t| t.category == TrackCategory::Theme)
    else {
        return;
    };
    let Some(handle) = music
        .catalog_handles
        .get(theme_idx)
        .and_then(|h| h.as_ref())
        .cloned()
    else {
        return;
    };
    spawn_music_player(&mut commands, handle, hud.music_volume, true);
}

pub(crate) fn stop_all_music_players(
    commands: &mut Commands,
    players: &Query<Entity, With<MusicPlayer>>,
) {
    for entity in players.iter() {
        commands.entity(entity).despawn();
    }
}

fn stop_music_on_leave_menu(mut commands: Commands, players: Query<Entity, With<MusicPlayer>>) {
    stop_all_music_players(&mut commands, &players);
}

fn spawn_music_player(
    commands: &mut Commands,
    handle: Handle<AudioSource>,
    music_volume: f32,
    r#loop: bool,
) {
    let vol = music_volume.clamp(0.0, 1.0);
    let settings = if r#loop {
        PlaybackSettings::LOOP
    } else {
        PlaybackSettings::DESPAWN
    };
    commands.spawn((
        MusicPlayer,
        AudioPlayer::new(handle),
        settings.with_volume(bevy::audio::Volume::Linear(vol)),
    ));
}

pub(crate) fn music_spawn_current(
    commands: &mut Commands,
    music: &MusicState,
    hud: &SimHudControls,
    screen: ClientScreen,
) {
    if music.active_catalog_indices.is_empty() {
        return;
    }
    let idx = music.track_index % music.active_catalog_indices.len();
    let Some(&cat_idx) = music.active_catalog_indices.get(idx) else {
        return;
    };
    let Some(handle) = music.catalog_handles.get(cat_idx).cloned().flatten() else {
        return;
    };
    let r#loop = screen == ClientScreen::MainMenu
        || (music.playlist == MusicPlaylist::Theme && music.active_catalog_indices.len() == 1);
    spawn_music_player(commands, handle, hud.music_volume, r#loop);
}

pub(crate) fn music_toggle_playback(
    commands: &mut Commands,
    players: Query<Entity, With<MusicPlayer>>,
    music: &mut MusicState,
    hud: &SimHudControls,
    screen: ClientScreen,
) {
    if music.playing {
        music.playing = false;
        stop_all_music_players(commands, &players);
    } else {
        music.playing = true;
        music_spawn_current(commands, music, hud, screen);
    }
}

pub(crate) fn music_skip(
    commands: &mut Commands,
    players: Query<Entity, With<MusicPlayer>>,
    music: &mut MusicState,
    hud: &SimHudControls,
    screen: ClientScreen,
    delta: isize,
) {
    let len = music.active_catalog_indices.len();
    if len == 0 {
        return;
    }
    stop_all_music_players(commands, &players);
    let next = (music.track_index as isize + delta).rem_euclid(len as isize) as usize;
    music.track_index = next;
    if music.playing {
        music_spawn_current(commands, music, hud, screen);
    }
}

pub(crate) fn music_apply_playlist(
    commands: &mut Commands,
    players: Query<Entity, With<MusicPlayer>>,
    music: &mut MusicState,
    hud: &SimHudControls,
    screen: ClientScreen,
    playlist: MusicPlaylist,
) {
    if music.playlist == playlist {
        return;
    }
    music.set_playlist(playlist);
    music.track_index = 0;
    stop_all_music_players(commands, &players);
    if music.playing {
        music_spawn_current(commands, music, hud, screen);
    }
}

/// Al entrar en partida, reemplaza el theme del menú por la playlist activa.
pub(crate) fn start_ingame_music(
    mut commands: Commands,
    music: Res<MusicState>,
    hud: Res<SimHudControls>,
    players: Query<Entity, With<MusicPlayer>>,
) {
    stop_all_music_players(&mut commands, &players);
    if music.playing {
        music_spawn_current(&mut commands, &music, &hud, ClientScreen::InGame);
    }
}

/// Aplica el volumen de música en caliente a la pista activa.
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

/// Avanza playlist en partida cuando termina la pista actual.
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
    let len = music.active_catalog_indices.len();
    if len == 0 || music.playlist == MusicPlaylist::Theme {
        return;
    }
    music.track_index = (music.track_index + 1) % len;
    music_spawn_current(&mut commands, &music, &hud, ClientScreen::InGame);
}
