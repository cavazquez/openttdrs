//! SFX espaciales del mundo (subset OpenSFX, paneo simplificado por distancia al viewport).

use bevy::prelude::*;

use openttdrs_core::{SoundId, TileCoord};

use crate::audio::ClientAssetRoot;
use crate::bevy_app::UpdateSet;
use crate::iso::tile_pos;
use crate::render::{MapTileSpawnViewport, PrimaryGameCamera, TileViewportBounds};
use crate::state::ClientScreen;
use crate::ui::SimHudControls;

/// Reproducir un efecto en coordenadas de tesela.
#[derive(Message)]
pub(crate) struct PlayWorldSfx {
    pub sound: SoundId,
    pub at: TileCoord,
    pub volume: f32,
}

#[derive(Resource, Default)]
pub(crate) struct WorldSfxHandles {
    handles: std::collections::HashMap<SoundId, Handle<AudioSource>>,
}

pub(crate) struct WorldSfxPlugin;

impl Plugin for WorldSfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSfxHandles>()
            .add_message::<PlayWorldSfx>()
            .add_systems(Startup, load_world_sfx)
            .add_systems(
                Update,
                play_world_sfx
                    .in_set(UpdateSet::Status)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

fn load_world_sfx(
    root: Res<ClientAssetRoot>,
    mut handles: ResMut<WorldSfxHandles>,
    asset_server: Res<AssetServer>,
) {
    if !handles.handles.is_empty() {
        return;
    }
    for sound in [
        SoundId::CashTill,
        SoundId::ConstructionRail,
        SoundId::ConstructionBridge,
        SoundId::ConstructionWater,
        SoundId::ConstructionOther,
        SoundId::Beep,
        SoundId::NewsTicker,
        SoundId::Applause,
        SoundId::NewEngine,
        SoundId::DepartureTrain,
        SoundId::DepartureRoad,
        SoundId::LevelCrossing,
        SoundId::Explosion,
        SoundId::RoadWorks,
        SoundId::TrainCollision,
    ] {
        let path = sound.asset_path();
        if !root.asset_file_exists(path) {
            continue;
        }
        handles.handles.insert(sound, asset_server.load(path));
    }
}

/// Factor de volumen por distancia al centro del viewport (paridad simplificada con `sound.cpp`).
fn spatial_volume_factor(
    at: TileCoord,
    bounds: Option<&TileViewportBounds>,
    camera: Option<&Transform>,
) -> f32 {
    let Some(bounds) = bounds else {
        return 1.0;
    };
    let cx = (bounds.tx0 + bounds.tx1) as f32 * 0.5;
    let cy = (bounds.ty0 + bounds.ty1) as f32 * 0.5;
    let dx = at.x as f32 - cx;
    let dy = at.y as f32 - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    let max_dist = ((bounds.tx1 - bounds.tx0).max(bounds.ty1 - bounds.ty0) as f32) * 0.65;
    let mut factor = (1.0 - dist / max_dist.max(1.0)).clamp(0.15, 1.0);
    if let Some(cam) = camera {
        let world = tile_pos(at.x, at.y, 0, 0.0);
        let cam_dist = cam.translation.truncate().distance(world.truncate());
        factor *= 1.0 - (cam_dist / 12_000.0).clamp(0.0, 0.5);
    }
    factor
}

fn play_world_sfx(
    mut commands: Commands,
    mut reader: MessageReader<PlayWorldSfx>,
    handles: Res<WorldSfxHandles>,
    hud: Res<SimHudControls>,
    viewport: Option<Res<MapTileSpawnViewport>>,
    camera: Query<&Transform, With<PrimaryGameCamera>>,
) {
    let base = hud.sfx_volume.clamp(0.0, 1.0);
    let cam = camera.iter().next();
    let bounds = viewport.as_deref().map(|v| &v.bounds);
    for msg in reader.read() {
        let spatial = spatial_volume_factor(msg.at, bounds, cam);
        let vol = base * msg.volume.clamp(0.0, 1.0) * spatial;
        if let Some(handle) = handles.handles.get(&msg.sound) {
            commands.spawn((
                AudioPlayer::new(handle.clone()),
                PlaybackSettings::ONCE.with_volume(bevy::audio::Volume::Linear(vol)),
            ));
        }
    }
}
