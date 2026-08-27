//! Aplicación de Action0 `Objects` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::badge::resolve_badge_labels_detailed;
use crate::object_spec::{ObjectSpecDef, empty_object_spec_catalog, next_free_object_spec_id};
use crate::sav::SavObjectMapping;

use super::super::action0::collect_object_metas_from_grf;

/// Reconstruye el catálogo de objetos desde el stack `enabled`.
pub fn apply_newgrf_objects(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = empty_object_spec_catalog();
    let stack = state.newgrf_stack.clone();
    let mappings = state.object_mappings.clone();
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
        let newgrf_runtime =
            (gfx.needs_runtime_resolve() || gfx.has_tile_layouts()).then(|| Box::new(gfx.clone()));
        for meta in collect_object_metas_from_grf(&data) {
            let mapped_id = mapped_object_id(&mappings, entry.grfid, meta.local_id, &catalog);
            let Some(id) = mapped_id.or_else(|| next_free_object_spec_id(&catalog)) else {
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
                callback_mask: meta.callback_mask,
                views,
                newgrf_runtime: newgrf_runtime.clone(),
                associated_badges,
            });
        }
    }
    state.object_spec_catalog = catalog;
}

/// Recupera el `ObjectType` que `OpenTTD` asignó a `(GRFID, local ID)` en
/// `OBID`. Si el mapping no existe o apunta a un ID ya ocupado, se deja que el
/// catálogo asigne el siguiente hueco libre, igual que en una partida nueva.
fn mapped_object_id(
    mappings: &[SavObjectMapping],
    grfid: u32,
    local_id: u8,
    catalog: &[ObjectSpecDef],
) -> Option<u16> {
    let id = mappings
        .iter()
        .find(|mapping| mapping.grfid == grfid && mapping.entity_id == u16::from(local_id))
        .map(|mapping| mapping.object_type)
        .filter(|&id| id >= crate::object_spec::NEW_OBJECT_OFFSET)
        .filter(|id| !catalog.iter().any(|spec| spec.id == *id))?;
    Some(id)
}

/// Aplica Objects con directorios de búsqueda por defecto.
pub fn apply_newgrf_objects_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_objects(state, &refs);
}
