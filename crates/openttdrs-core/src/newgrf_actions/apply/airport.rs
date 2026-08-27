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
use crate::map::TileCoord;
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
    rehydrate_newgrf_airport_tiles(state);
}

/// Reatacha los `AirportTile` de aeropuertos que llegaron desde un `.sav`.
///
/// `STNN` sólo persiste la huella y el id global del aeropuerto; el layout
/// `Airport` no guarda explícitamente su origen. Se prueban los anclajes
/// derivados de cada tile real y de cada entrada del layout y se acepta sólo
/// una huella exacta. Si el GRF/catálogo no está disponible se conserva la
/// huella vanilla y el renderer cae a `AirportPiece` sin inventar sprites.
pub fn rehydrate_newgrf_airport_tiles(state: &mut GameState) {
    let tile_catalog = &state.airport_tile_spec_catalog;
    let airport_catalog = &state.airport_spec_catalog;
    for station in &mut state.stations {
        let Some(spec_id) = station.airport_newgrf_spec_id else {
            continue;
        };
        if station.airport_tiles.is_empty() {
            continue;
        }
        let Some(def) = airport_catalog
            .iter()
            .find(|candidate| candidate.id == spec_id && candidate.enabled)
        else {
            continue;
        };
        let rotation = station.airport_rotation & 6;
        let Some(layout) = def
            .layouts
            .get(usize::from(station.airport_layout))
            .filter(|candidate| candidate.rotation == rotation)
            .or_else(|| {
                def.layouts
                    .iter()
                    .find(|candidate| candidate.rotation == rotation)
            })
            .or_else(|| {
                def.layouts
                    .iter()
                    .find(|candidate| candidate.rotation == 0 || candidate.rotation == 4)
            })
            .or_else(|| def.layouts.first())
        else {
            continue;
        };
        let axis_y = rotation == 2 || rotation == 6;
        let actual = station.airport_tiles.clone();
        let mut found = None;
        for axis_y in [axis_y] {
            for actual_coord in &actual {
                for layout_tile in &layout.tiles {
                    let (dx, dy) = if axis_y {
                        (i32::from(layout_tile.y), i32::from(layout_tile.x))
                    } else {
                        (i32::from(layout_tile.x), i32::from(layout_tile.y))
                    };
                    let origin = TileCoord::new(actual_coord.x - dx, actual_coord.y - dy);
                    let mapping = crate::airport::newgrf_airport_tile_gfx_with_layout(
                        origin,
                        def,
                        tile_catalog,
                        axis_y,
                        Some(station.airport_layout),
                        Some(rotation),
                    );
                    if airport_tile_coords_match(&mapping, &actual) {
                        found = Some(mapping);
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some(mapping) = found {
            station.airport_tile_gfx = mapping;
        }
    }
}

fn airport_tile_coords_match(mapping: &[(TileCoord, u16)], actual: &[TileCoord]) -> bool {
    if mapping.len() != actual.len() {
        return false;
    }
    let mut mapped: Vec<_> = mapping.iter().map(|(coord, _)| *coord).collect();
    let mut expected = actual.to_vec();
    mapped.sort_unstable();
    expected.sort_unstable();
    mapped == expected
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

#[cfg(test)]
mod tests {
    use super::rehydrate_newgrf_airport_tiles;
    use crate::GameState;
    use crate::airport_class::{
        AirportClassId, AirportLayoutTile, AirportSpecId, AirportTileLayout, NewgrfAirportSpecDef,
    };
    use crate::airport_tile_spec::{AirportTileGfxId, AirportTileSpecDef};
    use crate::map::TileCoord;
    use crate::station::{Station, StopKind};

    #[test]
    fn rehydrates_sav_airport_tile_mapping_from_exact_footprint() {
        let mut state = GameState::new(16, 16);
        let origin = TileCoord::new(5, 6);
        let mut station = Station::new_with_kind(origin, StopKind::Airport);
        station.airport_newgrf_spec_id = Some(10);
        station.airport_tiles = vec![
            TileCoord::new(5, 6),
            TileCoord::new(6, 6),
            TileCoord::new(5, 7),
            TileCoord::new(6, 7),
        ];
        state.stations.push(station);
        state.airport_tile_spec_catalog = vec![
            AirportTileSpecDef {
                gfx: AirportTileGfxId(74),
                subst_id: 24,
                from_newgrf: true,
                callback_mask: 0,
                newgrf_local_id: 0,
                newgrf_grfid: 1,
                newgrf_preview: None,
                newgrf_views: Vec::new(),
                newgrf_runtime: None,
            },
            AirportTileSpecDef {
                gfx: AirportTileGfxId(75),
                subst_id: 14,
                from_newgrf: true,
                callback_mask: 0,
                newgrf_local_id: 1,
                newgrf_grfid: 1,
                newgrf_preview: None,
                newgrf_views: Vec::new(),
                newgrf_runtime: None,
            },
        ];
        state.airport_spec_catalog = vec![NewgrfAirportSpecDef {
            id: 10,
            class: AirportClassId::Small,
            label: "Test airport".into(),
            short_label: "Test".into(),
            size_x: 2,
            size_y: 2,
            catchment: 4,
            noise_level: 1,
            subst_id: AirportSpecId::Small,
            layouts: vec![AirportTileLayout {
                rotation: 0,
                tiles: vec![
                    AirportLayoutTile {
                        x: 0,
                        y: 0,
                        gfx: 74,
                    },
                    AirportLayoutTile {
                        x: 1,
                        y: 0,
                        gfx: 75,
                    },
                    AirportLayoutTile {
                        x: 0,
                        y: 1,
                        gfx: 74,
                    },
                    AirportLayoutTile {
                        x: 1,
                        y: 1,
                        gfx: 75,
                    },
                ],
            }],
            enabled: true,
            min_year: 0,
            max_year: u16::MAX,
            maintenance_cost: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_purchase_views: Vec::new(),
        }];

        rehydrate_newgrf_airport_tiles(&mut state);

        assert_eq!(
            state.stations[0].airport_tile_gfx,
            vec![
                (TileCoord::new(5, 6), 74),
                (TileCoord::new(6, 6), 75),
                (TileCoord::new(5, 7), 74),
                (TileCoord::new(6, 7), 75),
            ]
        );
    }
}
