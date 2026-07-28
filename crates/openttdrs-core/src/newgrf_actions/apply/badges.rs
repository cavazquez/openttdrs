//! Aplicación de Action0 `Badges` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::badge::{BadgeDef, empty_badge_catalog, next_free_badge_id};

use super::super::action0::collect_badge_metas_from_grf;

/// Reconstruye el catálogo de badges desde el stack `enabled`.
pub fn apply_newgrf_badges(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = empty_badge_catalog();
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
        for meta in collect_badge_metas_from_grf(&data) {
            let Some(id) = next_free_badge_id(&catalog) else {
                break;
            };
            catalog.push(BadgeDef {
                id,
                label: meta.label,
                flags: meta.flags,
                from_newgrf: true,
                grfid: entry.grfid,
            });
        }
    }
    state.badge_catalog = catalog;
}

/// Aplica Badges con directorios de búsqueda por defecto.
pub fn apply_newgrf_badges_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_badges(state, &refs);
}
