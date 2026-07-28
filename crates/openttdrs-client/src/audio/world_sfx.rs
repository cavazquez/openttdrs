//! SFX espaciales del mundo (OpenSFX completo) con mixer de 8 canales.
//!
//! NewGRF (#254): la cola `GameState.runtime.pending_newgrf_sounds` se valida en
//! core (`play_newgrf_sound`). Un drenado futuro a `AudioSource` PCM puede
//! engancharse aquí sin cambiar el catálogo.

use bevy::prelude::*;

use openttdrs_core::SoundId;
use openttdrs_core::prelude::*;

use crate::audio::ClientAssetRoot;
use crate::bevy_app::UpdateSet;
use crate::iso::tile_pos;
use crate::render::{MapTileSpawnViewport, PrimaryGameCamera, TileViewportBounds};
use crate::state::ClientScreen;
use crate::ui::SimHudControls;

/// Canales concurrentes de efectos (paridad `mixer.cpp` `_channels[8]`).
pub(crate) const SFX_MIXER_CHANNELS: usize = 8;

/// Reproducir un efecto en coordenadas de tesela.
#[derive(Message)]
pub(crate) struct PlayWorldSfx {
    pub sound: SoundId,
    pub at: TileCoord,
    pub volume: f32,
    /// Prioridad 0..=255; mayor gana al robar canal (OpenTTD usa 0 por defecto).
    pub priority: u8,
}

impl PlayWorldSfx {
    #[must_use]
    pub(crate) fn new(sound: SoundId, at: TileCoord, volume: f32) -> Self {
        Self {
            sound,
            at,
            volume,
            priority: 64,
        }
    }

    #[must_use]
    pub(crate) fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Component)]
struct WorldSfxChannel(#[allow(dead_code)] u8);

#[derive(Clone, Copy)]
struct MixerSlot {
    entity: Entity,
    priority: u8,
    age: u32,
}

#[derive(Resource, Default)]
pub(crate) struct WorldSfxHandles {
    handles: std::collections::HashMap<SoundId, Handle<AudioSource>>,
}

/// Mixer de 8 canales: como máximo 8 `AudioPlayer` de mundo a la vez.
#[derive(Resource)]
pub(crate) struct SfxMixer {
    slots: [Option<MixerSlot>; SFX_MIXER_CHANNELS],
}

impl Default for SfxMixer {
    fn default() -> Self {
        Self {
            slots: [None; SFX_MIXER_CHANNELS],
        }
    }
}

pub(crate) struct WorldSfxPlugin;

impl Plugin for WorldSfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSfxHandles>()
            .init_resource::<SfxMixer>()
            .add_message::<PlayWorldSfx>()
            .add_systems(Startup, load_world_sfx)
            .add_systems(
                Update,
                (reap_finished_sfx_channels, play_world_sfx)
                    .chain()
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
    for sound in SoundId::ALL {
        if sound.is_unused() {
            continue;
        }
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

fn reap_finished_sfx_channels(
    mut mixer: ResMut<SfxMixer>,
    q: Query<Entity, With<WorldSfxChannel>>,
) {
    let alive: std::collections::HashSet<Entity> = q.iter().collect();
    for slot in &mut mixer.slots {
        if let Some(s) = *slot
            && !alive.contains(&s.entity)
        {
            *slot = None;
        }
    }
    for slot in mixer.slots.iter_mut().flatten() {
        slot.age = slot.age.saturating_add(1);
    }
}

fn allocate_channel(mixer: &mut SfxMixer, priority: u8) -> Option<(usize, Option<Entity>)> {
    for (i, slot) in mixer.slots.iter().enumerate() {
        if slot.is_none() {
            return Some((i, None));
        }
    }
    // Robar el de menor prioridad; empate → el más viejo.
    let mut best: Option<(usize, u8, u32)> = None;
    for (i, slot) in mixer.slots.iter().enumerate() {
        let Some(s) = slot else {
            continue;
        };
        if s.priority > priority {
            continue;
        }
        let replace = match best {
            None => true,
            Some((_, bp, ba)) => s.priority < bp || (s.priority == bp && s.age >= ba),
        };
        if replace {
            best = Some((i, s.priority, s.age));
        }
    }
    best.map(|(i, _, _)| (i, mixer.slots[i].map(|s| s.entity)))
}

fn play_world_sfx(
    mut commands: Commands,
    mut reader: MessageReader<PlayWorldSfx>,
    handles: Res<WorldSfxHandles>,
    mut mixer: ResMut<SfxMixer>,
    hud: Res<SimHudControls>,
    viewport: Option<Res<MapTileSpawnViewport>>,
    camera: Query<&Transform, With<PrimaryGameCamera>>,
) {
    let base = hud.sfx_volume.clamp(0.0, 1.0);
    let cam = camera.iter().next();
    let bounds = viewport.as_deref().map(|v| &v.bounds);
    for msg in reader.read() {
        if msg.sound.is_unused() {
            continue;
        }
        let Some(handle) = handles.handles.get(&msg.sound) else {
            continue;
        };
        let spatial = spatial_volume_factor(msg.at, bounds, cam);
        let vol = (base * msg.volume.clamp(0.0, 1.0) * msg.sound.base_volume_factor() * spatial)
            .clamp(0.0, 1.0);
        if vol < 0.02 {
            continue;
        }
        let Some((channel, steal)) = allocate_channel(&mut mixer, msg.priority) else {
            continue;
        };
        if let Some(old) = steal {
            commands.entity(old).despawn();
        }
        let entity = commands
            .spawn((
                AudioPlayer::new(handle.clone()),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(vol)),
                WorldSfxChannel(channel as u8),
            ))
            .id();
        mixer.slots[channel] = Some(MixerSlot {
            entity,
            priority: msg.priority,
            age: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_prefers_free_slot_then_steals_lower_priority() {
        let mut mixer = SfxMixer::default();
        let Some((c0, steal0)) = allocate_channel(&mut mixer, 10) else {
            panic!("expected free channel");
        };
        assert_eq!(c0, 0);
        assert!(steal0.is_none());
        mixer.slots[0] = Some(MixerSlot {
            entity: Entity::from_bits(1),
            priority: 5,
            age: 3,
        });
        for i in 1..SFX_MIXER_CHANNELS {
            mixer.slots[i] = Some(MixerSlot {
                entity: Entity::from_bits(u64::try_from(i).unwrap_or(1) + 1),
                priority: 100,
                age: 0,
            });
        }
        let Some((c, steal)) = allocate_channel(&mut mixer, 50) else {
            panic!("expected stealable channel");
        };
        assert_eq!(c, 0);
        assert_eq!(steal, Some(Entity::from_bits(1)));
    }
}
