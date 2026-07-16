//! Aplicación de Action0 `IndustryTiles` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::industry_tile::{
    INVALID_INDUSTRY_TILE, IndustryTileGfxId, IndustryTileSpecDef,
    empty_industry_tile_overrides, next_free_industry_tile_gfx_id,
};

use super::super::action0::collect_industry_tile_metas_from_grf;

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
            catalog.push(IndustryTileSpecDef {
                gfx: IndustryTileGfxId(global_gfx),
                subst_id: u16::from(meta.subst_id),
                from_newgrf: true,
                newgrf_local_id: meta.local_id,
                newgrf_grfid: entry.grfid,
                newgrf_preview: preview,
                newgrf_views: views,
                newgrf_runtime,
            });
        }
    }
    // Marcar slots sin override como inválidos (por claridad).
    let _ = INVALID_INDUSTRY_TILE;
    state.industry_tile_spec_catalog = catalog;
    state.industry_tile_overrides = overrides;
}

/// `IndustryTiles` con directorios de búsqueda por defecto.
pub fn apply_newgrf_industry_tiles_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_industry_tiles(state, &refs);
}
