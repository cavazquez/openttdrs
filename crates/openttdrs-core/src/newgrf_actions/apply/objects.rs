//! Aplicación de Action0 `Objects` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::badge::resolve_badge_labels_detailed;
use crate::object_spec::{ObjectSpecDef, empty_object_spec_catalog, next_free_object_spec_id};

use super::super::action0::collect_object_metas_from_grf;

/// Reconstruye el catálogo de objetos desde el stack `enabled`.
pub fn apply_newgrf_objects(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = empty_object_spec_catalog();
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
        let gfx = crate::newgrf_sprites::collect_object_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_object_metas_from_grf(&data) {
            let Some(id) = next_free_object_spec_id(&catalog) else {
                break;
            };
            let views = gfx
                .views_for_local_id(meta.local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            if let Some(err) = &meta.badge_list_error {
                state
                    .runtime
                    .newgrf_diagnostics
                    .push(format!("{}: object '{}': {err}", entry.filename, meta.name));
            }
            let (associated_badges, unresolved) = resolve_badge_labels_detailed(
                &meta.badge_labels,
                &state.badge_catalog,
                entry.grfid,
            );
            for label in unresolved {
                state.runtime.newgrf_diagnostics.push(format!(
                    "{}: object '{}': badge desconocido '{label}'",
                    entry.filename, meta.name
                ));
            }
            catalog.push(ObjectSpecDef {
                id,
                class_label: meta.class_label,
                name: meta.name,
                size: meta.size,
                from_newgrf: true,
                local_id: meta.local_id,
                grfid: entry.grfid,
                climate_mask: meta.climate_mask,
                build_cost_factor: meta.build_cost_factor,
                views,
                associated_badges,
            });
        }
    }
    state.object_spec_catalog = catalog;
}

/// Aplica Objects con directorios de búsqueda por defecto.
pub fn apply_newgrf_objects_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_objects(state, &refs);
}
