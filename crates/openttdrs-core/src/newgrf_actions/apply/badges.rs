//! Aplicación de Action0 `Badges` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::badge::{BadgeDef, empty_badge_catalog, next_free_badge_id};

use super::super::action0::collect_badge_metas_from_grf;

/// Reconstruye el catálogo de badges desde el stack `enabled`.
///
/// Misma etiqueta (case-insensitive) entre GRFs → un solo [`BadgeDef`] (sin colisión).
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
            if let Some(existing) = catalog
                .iter_mut()
                .find(|b| b.label.eq_ignore_ascii_case(&meta.label))
            {
                // Merge: conservar id/label/grfid del primero; actualizar flags.
                existing.flags = meta.flags;
                existing.from_newgrf = true;
                continue;
            }
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
    // Sanity: no debe haber duplicados case-insensitive tras el merge.
    debug_assert!(catalog.iter().enumerate().all(|(i, a)| {
        !catalog
            .iter()
            .skip(i + 1)
            .any(|b| a.label.eq_ignore_ascii_case(&b.label))
    }));
    state.badge_catalog = catalog;
}

/// Aplica Badges con directorios de búsqueda por defecto.
pub fn apply_newgrf_badges_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_badges(state, &refs);
}
