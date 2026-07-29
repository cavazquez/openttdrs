//! Aplicación de Action0 `RoadTypes` / `TramTypes` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::road_type::{RoadType, RoadTypeDef, next_free_road_type_id, vanilla_road_type_catalog};

use super::super::action0::collect_roadtype_metas_from_grf;

fn resolve_powered_mask(catalog: &[RoadTypeDef], labels: &[[u8; 4]], self_id: RoadType) -> u64 {
    let mut mask = 1u64 << self_id.as_u8();
    for label in labels {
        let key = std::str::from_utf8(label)
            .unwrap_or("")
            .trim_end_matches('\0')
            .trim();
        if key.is_empty() {
            continue;
        }
        if let Some(def) = catalog
            .iter()
            .find(|d| d.short_label.eq_ignore_ascii_case(key) || d.label.eq_ignore_ascii_case(key))
        {
            mask |= 1u64 << def.id.as_u8();
        }
    }
    mask
}

/// Reconstruye el catálogo road/tram desde el stack `enabled` + vanilla.
pub fn apply_newgrf_road_types(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_road_type_catalog();
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
        let type_tables = crate::newgrf_type_tables::collect_type_tables_from_grf(&data);
        let tables_opt = (!type_tables.is_empty()).then_some(type_tables);
        let gfx =
            crate::newgrf_sprites::collect_roadtype_sprite_graphics(&data).unwrap_or_default();
        let metas = collect_roadtype_metas_from_grf(&data);
        for (local_idx, meta) in metas.into_iter().enumerate() {
            let Some(id) = next_free_road_type_id(&catalog) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let preview = views.first().cloned();
            let newgrf_runtime = if gfx.needs_runtime_resolve() {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            let powered_mask = resolve_powered_mask(&catalog, &meta.powered_labels, id);
            catalog.push(RoadTypeDef {
                id,
                class: meta.class,
                label: meta.label,
                short_label: meta.short_label,
                intro_year: meta.intro_year,
                max_speed: meta.max_speed,
                cost_multiplier: meta.cost_multiplier,
                maintenance_multiplier: meta.maintenance_multiplier,
                flags: meta.flags,
                powered_mask,
                from_tramtypes_feature: meta.from_tramtypes_feature,
                from_newgrf: true,
                newgrf_preview: preview,
                newgrf_views: views,
                newgrf_local_id: local_id,
                newgrf_runtime,
                newgrf_grfid: entry.grfid,
                newgrf_type_tables: tables_opt.clone(),
            });
        }
    }
    state.road_type_catalog = catalog;
    if !state
        .road_type_catalog
        .iter()
        .any(|d| d.id == state.current_road_type)
    {
        state.current_road_type = RoadType::ROAD;
    }
    if !state
        .road_type_catalog
        .iter()
        .any(|d| d.id == state.current_tram_type)
    {
        state.current_tram_type = RoadType::TRAM;
    }
}

/// Aplica `RoadTypes` con directorios de búsqueda por defecto.
pub fn apply_newgrf_road_types_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_road_types(state, &refs);
}
