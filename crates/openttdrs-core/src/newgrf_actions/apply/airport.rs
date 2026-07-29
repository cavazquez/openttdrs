//! Aplicación de Action0 `AirportTiles` (`0x11`) y `Airports` (`0x0D`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::GameState;
use crate::airport_class::{
    AIRPORT_ACTION3_PURCHASE, AirportClassId, AirportLayoutTile, AirportSpecId, AirportTileLayout,
    NEW_AIRPORT_OFFSET, NewgrfAirportSpecDef, airport_spec_def, next_free_airport_id,
};
use crate::airport_tile_spec::{
    AirportTileGfxId, AirportTileSpecDef, empty_airport_tile_overrides,
    next_free_airport_tile_gfx_id,
};
use crate::newgrf_sprites::Action2EvalCtx;

use super::super::action0::{collect_airport_metas_from_grf, collect_airport_tile_metas_from_grf};

/// Reconstruye catálogo `AirportTiles` desde el stack enabled.
pub fn apply_newgrf_airport_tiles(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = Vec::new();
    let mut overrides = empty_airport_tile_overrides();
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
            crate::newgrf_sprites::collect_airport_tile_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_airport_tile_metas_from_grf(&data) {
            let Some(global_gfx) = next_free_airport_tile_gfx_id(&catalog) else {
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
            catalog.push(AirportTileSpecDef {
                gfx: AirportTileGfxId(global_gfx),
                subst_id: u16::from(meta.subst_id),
                from_newgrf: true,
                callback_mask: meta.callback_mask,
                newgrf_local_id: meta.local_id,
                newgrf_grfid: entry.grfid,
                newgrf_preview: preview,
                newgrf_views: views,
                newgrf_runtime,
            });
        }
    }
    state.airport_tile_spec_catalog = catalog;
    state.airport_tile_overrides = overrides;
}

fn local_tile_gfx_map(catalog: &[AirportTileSpecDef]) -> HashMap<(u32, u16), u16> {
    let mut map = HashMap::new();
    for def in catalog {
        map.insert(
            (def.newgrf_grfid, u16::from(def.newgrf_local_id)),
            def.gfx.as_u16(),
        );
    }
    map
}

fn class_of_subst(subst: AirportSpecId) -> AirportClassId {
    airport_spec_def(subst).map_or(AirportClassId::Small, |d| d.class)
}

/// Reconstruye catálogo `Airports` (requiere tiles ya aplicados).
#[allow(clippy::too_many_lines)]
pub fn apply_newgrf_airports(state: &mut GameState, search_dirs: &[&Path]) {
    let local_tile_map = local_tile_gfx_map(&state.airport_tile_spec_catalog);
    let mut catalog = Vec::new();
    let mut disabled_vanilla = vec![false; NEW_AIRPORT_OFFSET as usize];
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
        let gfx = crate::newgrf_sprites::collect_airport_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_airport_metas_from_grf(&data) {
            if meta.disabled {
                if (meta.subst_id as usize) < disabled_vanilla.len() {
                    // `subst_id` en disable es el id vanilla OpenTTD (`AT_*`).
                    disabled_vanilla[meta.subst_id as usize] = true;
                }
                continue;
            }
            let Some(global_id) = next_free_airport_id(&catalog) else {
                break;
            };
            // `subst_id` es `AirportTypes` OpenTTD (`AT_*`), no nuestro `repr`.
            let subst = AirportSpecId::from_ottd_airport_type(meta.subst_id);
            let layouts: Vec<AirportTileLayout> = meta
                .layouts
                .iter()
                .map(|lay| AirportTileLayout {
                    rotation: lay.rotation,
                    tiles: lay
                        .tiles
                        .iter()
                        .map(|t| {
                            let gfx = if t.use_local_tile {
                                local_tile_map
                                    .get(&(entry.grfid, t.gfx_or_local))
                                    .copied()
                                    .unwrap_or(t.gfx_or_local)
                            } else {
                                t.gfx_or_local
                            };
                            AirportLayoutTile {
                                x: t.x,
                                y: t.y,
                                gfx,
                            }
                        })
                        .collect(),
                })
                .collect();
            let (sx, sy) = if meta.size_x > 0 && meta.size_y > 0 {
                (i32::from(meta.size_x), i32::from(meta.size_y))
            } else {
                airport_spec_def(subst).map_or((2, 2), |d| (d.size_x, d.size_y))
            };
            let label = if meta.name.is_empty() {
                format!("NewGRF Airport {}", meta.local_id)
            } else {
                meta.name.clone()
            };
            let short = label.chars().take(6).collect::<String>();
            let views = gfx
                .views_for_local_id(meta.local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let purchase_views = gfx
                .views_for_specific_ctx(
                    meta.local_id,
                    AIRPORT_ACTION3_PURCHASE,
                    &mut Action2EvalCtx::default(),
                )
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            catalog.push(NewgrfAirportSpecDef {
                id: global_id,
                class: class_of_subst(subst),
                label,
                short_label: short,
                size_x: sx,
                size_y: sy,
                catchment: i32::from(meta.catchment),
                noise_level: meta.noise_level,
                subst_id: subst,
                layouts,
                enabled: true,
                min_year: meta.min_year,
                max_year: meta.max_year,
                maintenance_cost: meta.maintenance_cost,
                newgrf_local_id: meta.local_id,
                newgrf_grfid: entry.grfid,
                newgrf_views: views,
                newgrf_purchase_views: purchase_views,
            });
        }
    }
    state.airport_spec_catalog = catalog;
    state.airport_vanilla_disabled = disabled_vanilla;
}

pub fn apply_newgrf_airport_tiles_default_dirs(state: &mut GameState) {
    let dirs = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    apply_newgrf_airport_tiles(state, &refs);
}

pub fn apply_newgrf_airports_default_dirs(state: &mut GameState) {
    let dirs = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    apply_newgrf_airports(state, &refs);
}
