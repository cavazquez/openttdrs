//! Aplicación de Action0 `Houses` (`0x07`) desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::house_spec::{
    HouseSpecDef, empty_house_overrides, empty_house_spec_catalog, next_free_house_id,
};

use super::super::action0::collect_house_metas_from_grf;

/// Reconstruye catálogo `Houses` desde el stack enabled.
pub fn apply_newgrf_houses(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = empty_house_spec_catalog();
    let mut overrides = empty_house_overrides();
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
        let gfx = crate::newgrf_sprites::collect_house_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_house_metas_from_grf(&data) {
            let Some(global_id) = next_free_house_id(&catalog) else {
                break;
            };
            let views = gfx
                .views_for_local_id(meta.local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let newgrf_runtime = if gfx.needs_runtime_resolve() {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            if let Some(ovr) = meta.override_id {
                overrides[usize::from(ovr)] = global_id;
            }
            catalog.push(HouseSpecDef {
                id: global_id,
                local_id: meta.local_id,
                subst_id: meta.subst_id,
                building_flags: meta.building_flags,
                min_year: meta.min_year,
                max_year: meta.max_year,
                population: meta.population,
                mail_generation: meta.mail_generation,
                availability: meta.availability,
                probability: meta.probability,
                override_id: meta.override_id,
                callback_mask: meta.callback_mask,
                name: meta.name,
                from_newgrf: true,
                grfid: entry.grfid,
                newgrf_views: views,
                newgrf_local_id: meta.local_id,
                newgrf_runtime,
            });
        }
    }
    state.house_spec_catalog = catalog;
    state.house_overrides = overrides;
}

/// `Houses` con directorios de búsqueda por defecto.
pub fn apply_newgrf_houses_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_houses(state, &refs);
}
