//! Aplicación de Action0 `Bridges` (`0x06`) desde el stack `NewGRF`.

use std::path::Path;

use crate::GameState;
use crate::bridge_spec::vanilla_bridge_spec_catalog;

use super::super::action0::collect_bridge_metas_from_grf;

/// Reconstruye el catálogo de puentes: vanilla + overrides in-place (último stack gana).
pub fn apply_newgrf_bridges(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_bridge_spec_catalog();
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
        for meta in collect_bridge_metas_from_grf(&data) {
            let idx = usize::from(meta.local_id);
            if idx >= catalog.len() {
                continue;
            }
            let slot = &mut catalog[idx];
            if meta.year_set {
                slot.available_from_year = meta.available_from_year;
            }
            if meta.min_len_set {
                slot.min_middle_len = meta.min_middle_len;
            }
            if meta.max_len_set {
                slot.max_middle_len = meta.max_middle_len;
            }
            if meta.price_set {
                slot.price_mult = meta.price_mult;
            }
            if meta.speed_set {
                slot.max_speed = meta.max_speed;
            }
            if let Some(name) = meta.name {
                slot.name = name;
            }
            if meta.has_custom_sprites {
                slot.has_custom_sprites = true;
            }
            if meta.pillar_flags_set {
                slot.pillar_flags = meta.pillar_flags;
                slot.has_custom_pillar_flags = true;
            }
            slot.from_newgrf = true;
            slot.grfid = entry.grfid;
        }
    }
    state.bridge_spec_catalog = catalog;
}

/// Aplica Bridges con directorios de búsqueda por defecto.
pub fn apply_newgrf_bridges_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_bridges(state, &refs);
}
