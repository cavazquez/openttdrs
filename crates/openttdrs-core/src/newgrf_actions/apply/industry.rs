//! Aplicación de Action0 `IndustryTiles` (`0x09`) e `Industries` (`0x0A`).

use std::collections::HashMap;
use std::path::Path;

use crate::GameState;
use crate::industry_spec::{
    IndustryLayoutTile, IndustrySpecDef, empty_industry_overrides, empty_industry_spec_catalog,
    get_cargo_translation, next_free_industry_id,
};
use crate::industry_tile::{
    IndustryTileGfxId, IndustryTileSpecDef, empty_industry_tile_overrides,
    next_free_industry_tile_gfx_id,
};

use super::super::action0::{
    collect_industry_metas_from_grf, collect_industry_tile_metas_from_grf,
};

/// Reconstruye catálogo `IndustryTiles` desde el stack enabled.
pub fn apply_newgrf_industry_tiles(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = Vec::new();
    let mut overrides = empty_industry_tile_overrides();
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
        let gfx =
            crate::newgrf_sprites::collect_industry_tile_sprite_graphics(&data).unwrap_or_default();
        let metas = collect_industry_tile_metas_from_grf(&data);
        for meta in metas {
            let Some(global_gfx) = next_free_industry_tile_gfx_id(&catalog) else {
                break;
            };
            let views = gfx
                .views_for_local_id(meta.local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let preview = views.first().cloned();
            let newgrf_runtime = if gfx.needs_runtime_resolve() {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            if let Some(ovr) = meta.override_of {
                overrides[usize::from(ovr)] = global_gfx;
            }
            let accepts_cargo_labels: Vec<String> = meta
                .accepts_cargo_indices
                .iter()
                .filter_map(|&idx| get_cargo_translation(idx, &state.cargo_spec_catalog))
                .collect();
            catalog.push(IndustryTileSpecDef {
                gfx: IndustryTileGfxId(global_gfx),
                subst_id: u16::from(meta.subst_id),
                from_newgrf: true,
                accepts_cargo_indices: meta.accepts_cargo_indices,
                accepts_cargo_labels,
                acceptance: meta.acceptance,
                callback_mask: meta.callback_mask,
                animation_frames: meta.animation_frames,
                animation_status: meta.animation_status,
                animation_speed: meta.animation_speed,
                animation_triggers: meta.animation_triggers,
                animation_special_flags: meta.animation_special_flags,
                newgrf_local_id: meta.local_id,
                newgrf_grfid: entry.grfid,
                newgrf_preview: preview,
                newgrf_views: views,
                newgrf_runtime,
            });
        }
    }
    state.industry_tile_spec_catalog = catalog;
    state.industry_tile_overrides = overrides;
}

fn local_tile_gfx_map(catalog: &[IndustryTileSpecDef]) -> HashMap<(u32, u16), u16> {
    let mut map = HashMap::new();
    for def in catalog {
        map.insert(
            (def.newgrf_grfid, u16::from(def.newgrf_local_id)),
            def.gfx.as_u16(),
        );
    }
    map
}

/// Reconstruye catálogo `Industries` (requiere tiles ya aplicados).
pub fn apply_newgrf_industries(state: &mut GameState, search_dirs: &[&Path]) {
    let local_tile_map = local_tile_gfx_map(&state.industry_tile_spec_catalog);
    let mut catalog = empty_industry_spec_catalog();
    let mut overrides = empty_industry_overrides();
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
        let gfx =
            crate::newgrf_sprites::collect_industry_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_industry_metas_from_grf(&data) {
            let Some(global_id) = next_free_industry_id(&catalog) else {
                break;
            };
            let layouts: Vec<Vec<IndustryLayoutTile>> = meta
                .layouts
                .iter()
                .map(|layout| {
                    layout
                        .iter()
                        .map(|t| {
                            let gfx = if t.use_local_tile {
                                local_tile_map
                                    .get(&(entry.grfid, t.gfx_or_local))
                                    .copied()
                                    .unwrap_or(crate::industry_tile::INVALID_INDUSTRY_TILE)
                            } else {
                                t.gfx_or_local
                            };
                            IndustryLayoutTile {
                                x: t.x,
                                y: t.y,
                                gfx,
                            }
                        })
                        .collect()
                })
                .collect();
            let produced_cargo_labels: Vec<String> = meta
                .produced_cargo_indices
                .iter()
                .filter_map(|&idx| get_cargo_translation(idx, &state.cargo_spec_catalog))
                .collect();
            let accepted_cargo_labels: Vec<String> = meta
                .accepted_cargo_indices
                .iter()
                .filter_map(|&idx| get_cargo_translation(idx, &state.cargo_spec_catalog))
                .collect();
            if let Some(ovr) = meta.override_id {
                overrides[usize::from(ovr)] = global_id;
            }
            catalog.push(IndustrySpecDef {
                id: global_id,
                local_id: meta.local_id,
                subst_id: meta.subst_id,
                override_id: meta.override_id,
                layouts,
                produced_cargo_indices: meta.produced_cargo_indices,
                produced_cargo_labels,
                accepted_cargo_indices: meta.accepted_cargo_indices,
                accepted_cargo_labels,
                production_rates: meta.production_rates,
                input_multipliers: meta.input_multipliers,
                callback_mask: meta.callback_mask,
                cost_multiplier: meta.cost_multiplier,
                name: meta.name,
                from_newgrf: true,
                grfid: entry.grfid,
                newgrf_local_id: meta.local_id,
                newgrf_runtime: gfx.needs_runtime_resolve().then(|| Box::new(gfx.clone())),
            });
        }
    }
    state.industry_spec_catalog = catalog;
    state.industry_overrides = overrides;
}

/// `IndustryTiles` con directorios de búsqueda por defecto.
pub fn apply_newgrf_industry_tiles_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_industry_tiles(state, &refs);
}

/// `Industries` con directorios de búsqueda por defecto.
pub fn apply_newgrf_industries_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_industries(state, &refs);
}
