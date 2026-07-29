//! Aplicación de Action11 samples + Action0 `Sounds` (`0x0C`) desde el stack `NewGRF`.

use std::path::Path;

use crate::GameState;
use crate::sound_effect::{
    SoundEffectDef, clamp_sound_volume, collect_sound_samples_from_grf, empty_sound_effect_catalog,
};
use crate::sound_id::SOUND_COUNT;

use super::super::action0::collect_sound_metas_from_grf;

/// Reconstruye el catálogo de sonidos `NewGRF` desde el stack `enabled`.
///
/// Clave `(grfid, local_id)`: dos GRFs con el mismo `local_id` no se pisan.
/// Action0 sin sample → diagnóstico + entrada `has_sample=false`.
pub fn apply_newgrf_sounds(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = empty_sound_effect_catalog();
    let mut overrides = [None; SOUND_COUNT];
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let collected = collect_sound_samples_from_grf(&data);
        if collected.truncated {
            state
                .runtime
                .newgrf_diagnostics
                .push(format!("sound Action11 truncated grfid={}", entry.grfid));
        }
        for (local_id, pcm) in collected.samples {
            let has_sample = !pcm.is_empty();
            if let Some(existing) = catalog
                .iter_mut()
                .find(|d| d.grfid == entry.grfid && d.local_id == local_id)
            {
                existing.has_sample = has_sample;
                existing.sample_pcm = pcm;
                existing.from_newgrf = true;
            } else {
                catalog.push(SoundEffectDef {
                    local_id,
                    grfid: entry.grfid,
                    volume: 128,
                    priority: 0,
                    override_old: None,
                    has_sample,
                    sample_pcm: pcm,
                    from_newgrf: true,
                });
            }
        }
        for meta in collect_sound_metas_from_grf(&data) {
            let volume = clamp_sound_volume(meta.volume);
            let override_old = meta
                .override_old
                .filter(|&id| usize::from(id) < SOUND_COUNT);
            if let Some(existing) = catalog
                .iter_mut()
                .find(|d| d.grfid == entry.grfid && d.local_id == meta.local_id)
            {
                existing.volume = volume;
                existing.priority = meta.priority;
                existing.override_old = override_old;
                if !existing.has_sample || existing.sample_pcm.is_empty() {
                    state.runtime.newgrf_diagnostics.push(format!(
                        "sound local_id={} grfid={}: missing sample",
                        meta.local_id, entry.grfid
                    ));
                    existing.has_sample = false;
                }
            } else {
                state.runtime.newgrf_diagnostics.push(format!(
                    "sound local_id={} grfid={}: missing sample",
                    meta.local_id, entry.grfid
                ));
                catalog.push(SoundEffectDef {
                    local_id: meta.local_id,
                    grfid: entry.grfid,
                    volume,
                    priority: meta.priority,
                    override_old,
                    has_sample: false,
                    sample_pcm: Vec::new(),
                    from_newgrf: true,
                });
            }
            if let Some(old) = override_old {
                overrides[usize::from(old)] = Some((entry.grfid, meta.local_id));
            }
        }
    }
    state.sound_effect_catalog = catalog;
    state.runtime.sound_overrides = overrides;
}

/// Aplica Sounds con directorios de búsqueda por defecto.
pub fn apply_newgrf_sounds_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_sounds(state, &refs);
}
