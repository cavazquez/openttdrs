//! Aplicación de Action0 `Canals` (`0x05`) + vistas Action3 desde el stack.

use std::path::Path;

use crate::GameState;
use crate::canal_spec::{CanalFeatureDef, vanilla_canal_feature_catalog};

use super::super::action0::collect_canal_metas_from_grf;

/// Reconstruye el catálogo de features de canal (último stack gana por `local_id`).
pub fn apply_newgrf_canals(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_canal_feature_catalog();
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
        let gfx = crate::newgrf_sprites::collect_canal_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_canal_metas_from_grf(&data) {
            let idx = usize::from(meta.local_id);
            if idx >= catalog.len() {
                continue;
            }
            let views = gfx
                .views_for_local_id(meta.local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            catalog[idx] = CanalFeatureDef {
                id: meta.local_id,
                callback_mask: meta.callback_mask,
                flags: meta.flags,
                from_newgrf: true,
                grfid: entry.grfid,
                newgrf_views: views,
            };
        }
    }
    state.canal_feature_catalog = catalog;
}

/// Aplica Canals con directorios de búsqueda por defecto.
pub fn apply_newgrf_canals_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_canals(state, &refs);
}
