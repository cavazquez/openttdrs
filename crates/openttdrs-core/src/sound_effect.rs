//! Efectos de sonido `NewGRF` (Action11 samples + Action0 feature `0x0C`).
//!
//! `OpenTTD` 15.3: Action0 `Sounds` solo ajusta volume/priority/override sobre samples
//! registrados vía Action11. Formato local de fixtures: tras cabecera Action11
//! (`0x11`, count), cada sample es `WORD` tamaño LE + `size` bytes PCM mono u8.

use serde::{Deserialize, Serialize};

use crate::sound_id::{SOUND_COUNT, SoundId};
use crate::{GameState, TileCoord};

/// Spec de efecto de sonido definido por Action11 + Action0 `0x0C`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundEffectDef {
    pub local_id: u8,
    pub grfid: u32,
    /// Prop `0x08`; default 128; clamp `0..=128`.
    pub volume: u8,
    /// Prop `0x09`.
    pub priority: u8,
    /// Prop `0x0A`: índice baseset [`SoundId`] si `< SOUND_COUNT`.
    pub override_old: Option<u8>,
    pub has_sample: bool,
    /// PCM mono u8 para tests / reproducción; vacío = inválido.
    #[serde(default)]
    pub sample_pcm: Vec<u8>,
    pub from_newgrf: bool,
}

/// Cola observable de reproducción `NewGRF` (sin Bevy).
#[derive(Debug, Clone, PartialEq)]
pub struct PendingNewgrfSound {
    pub grfid: u32,
    pub local_id: u8,
    pub volume: f32,
    pub priority: u8,
    /// Origen espacial de un callback de tesela. `None` conserva el camino
    /// global de vehículos y de overrides de baseset.
    pub at: Option<TileCoord>,
}

/// Solicitud de sonido producida por un callback de animación de tesela.
///
/// El `SoundID` de `NewGRF` ocupa los bits 8..14 del resultado; los siete
/// bits evitan confundir `CALLBACK_FAILED` con el sample local 127.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewgrfTileSound {
    pub grfid: u32,
    pub local_id: u8,
    pub at: TileCoord,
}

/// Extrae el sonido opcional codificado por `AnimationBase` en un callback.
#[must_use]
pub const fn newgrf_tile_animation_sound_from_callback(
    grfid: u32,
    result: u16,
    at: TileCoord,
) -> Option<NewgrfTileSound> {
    if result == u16::MAX {
        return None;
    }
    let local_id = ((result >> 8) & 0x7F) as u8;
    if local_id == 0 {
        return None;
    }
    Some(NewgrfTileSound {
        grfid,
        local_id,
        at,
    })
}

/// Error al encolar reproducción de un sonido `NewGRF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundPlayError {
    /// No hay entrada `(grfid, local_id)` en el catálogo.
    NotFound,
    /// Entrada sin sample PCM válido.
    InvalidSample,
}

/// Resultado de recolectar samples Action11 de un GRF.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollectedSoundSamples {
    pub samples: Vec<(u8, Vec<u8>)>,
    /// Action11 declaró más sonidos de los que cabían en el payload.
    pub truncated: bool,
}

/// Catálogo vacío (no hay SFX `NewGRF` vanilla).
#[must_use]
pub fn empty_sound_effect_catalog() -> Vec<SoundEffectDef> {
    Vec::new()
}

/// Busca un sonido por `(grfid, local_id)`.
#[must_use]
pub fn sound_effect_def(
    catalog: &[SoundEffectDef],
    grfid: u32,
    local_id: u8,
) -> Option<&SoundEffectDef> {
    catalog
        .iter()
        .find(|d| d.grfid == grfid && d.local_id == local_id)
}

/// Volumen efectivo `0.0..=1.0` (`volume / 128`).
#[must_use]
pub fn effective_volume(def: &SoundEffectDef) -> f32 {
    f32::from(def.volume) / 128.0
}

/// Clamp de volumen Action0 prop `0x08`.
#[must_use]
pub fn clamp_sound_volume(volume: u8) -> u8 {
    volume.min(128)
}

/// Recolecta samples Action11 (formato fixture: count + N×(WORD size + PCM)).
#[must_use]
pub fn collect_sound_samples_from_grf(data: &[u8]) -> CollectedSoundSamples {
    let mut out = CollectedSoundSamples::default();
    let _ = crate::newgrf_actions::for_each_pseudo_payload(data, |payload| {
        if payload.first() != Some(&0x11) || payload.len() < 2 {
            return;
        }
        let count = usize::from(payload[1]);
        let mut i = 2usize;
        for local_id in 0..count {
            if i + 2 > payload.len() {
                out.truncated = true;
                break;
            }
            let size = usize::from(u16::from_le_bytes([payload[i], payload[i + 1]]));
            i += 2;
            if i + size > payload.len() {
                out.truncated = true;
                break;
            }
            let pcm = payload[i..i + size].to_vec();
            i += size;
            let local_id = u8::try_from(local_id).unwrap_or(u8::MAX);
            out.samples.push((local_id, pcm));
        }
    });
    out
}

/// Encola reproducción de un sonido `NewGRF` (observable en tests sin audio).
///
/// # Errors
///
/// `NotFound` si no hay def; `InvalidSample` si no hay PCM.
fn enqueue_newgrf_sound(
    state: &mut GameState,
    grfid: u32,
    local_id: u8,
    at: Option<TileCoord>,
) -> Result<(), SoundPlayError> {
    let Some(def) = sound_effect_def(&state.sound_effect_catalog, grfid, local_id) else {
        return Err(SoundPlayError::NotFound);
    };
    if !def.has_sample || def.sample_pcm.is_empty() {
        return Err(SoundPlayError::InvalidSample);
    }
    let pending = PendingNewgrfSound {
        grfid: def.grfid,
        local_id: def.local_id,
        volume: effective_volume(def),
        priority: def.priority,
        at,
    };
    state.runtime.pending_newgrf_sounds.push(pending);
    Ok(())
}

/// Encola reproducción global de un sonido `NewGRF` (p. ej. vehículo).
///
/// # Errors
///
/// `NotFound` si no hay def; `InvalidSample` si no hay PCM.
pub fn play_newgrf_sound(
    state: &mut GameState,
    grfid: u32,
    local_id: u8,
) -> Result<(), SoundPlayError> {
    enqueue_newgrf_sound(state, grfid, local_id, None)
}

/// Encola un sonido ambiental `NewGRF` en una tesela concreta.
///
/// El cliente lo filtra con `sound.ambient` y conserva la coordenada para
/// aplicar la misma atenuación espacial que el resto de SFX de mundo.
///
/// # Errors
///
/// `NotFound` si no hay def; `InvalidSample` si no hay PCM.
pub fn play_newgrf_tile_sound(
    state: &mut GameState,
    grfid: u32,
    local_id: u8,
    at: TileCoord,
) -> Result<(), SoundPlayError> {
    enqueue_newgrf_sound(state, grfid, local_id, Some(at))
}

/// Encola el sonido ambiental que `AnimationBase` codifica en un callback.
///
/// Un resultado fallido o un identificador local cero es silencioso, tal como
/// `PlayTileSound` en `OpenTTD`; un sample inexistente se devuelve al caller
/// para que las rutas de simulación puedan conservar un fallback silencioso.
///
/// # Errors
///
/// `NotFound` si el callback pidió un sample ausente; `InvalidSample` si el
/// sample existe pero no tiene PCM utilizable.
pub fn play_newgrf_tile_animation_sound(
    state: &mut GameState,
    grfid: u32,
    result: u16,
    at: TileCoord,
) -> Result<(), SoundPlayError> {
    let Some(sound) = newgrf_tile_animation_sound_from_callback(grfid, result, at) else {
        return Ok(());
    };
    play_newgrf_tile_sound(state, sound.grfid, sound.local_id, sound.at)
}

/// Reproduce un SFX baseset, o el `NewGRF` que lo overridea si existe mapping.
///
/// # Errors
///
/// Si hay override pero el sample `NewGRF` es inválido / ausente.
pub fn play_sound_or_override(state: &mut GameState, sound: SoundId) -> Result<(), SoundPlayError> {
    let idx = usize::from(sound.as_u8());
    if idx < SOUND_COUNT
        && let Some((grfid, local_id)) = state.runtime.sound_overrides[idx]
    {
        return play_newgrf_sound(state, grfid, local_id);
    }
    // Baseset: no hay cola NewGRF; éxito silencioso (cliente usa SoundId).
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_callback_sound_masks_bit_15_and_rejects_failed_or_zero() {
        let at = TileCoord::new(4, 9);
        assert_eq!(
            newgrf_tile_animation_sound_from_callback(0x1234_5678, 0xF301, at),
            Some(NewgrfTileSound {
                grfid: 0x1234_5678,
                local_id: 0x73,
                at,
            })
        );
        assert_eq!(
            newgrf_tile_animation_sound_from_callback(1, u16::MAX, at),
            None
        );
        assert_eq!(
            newgrf_tile_animation_sound_from_callback(1, 0x00FE, at),
            None
        );
    }
}
