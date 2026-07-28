//! Aplicación de Action0 `RoadStops` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::road_stop_spec::{
    RoadStopClassDef, RoadStopSpecDef, empty_road_stop_class_catalog, empty_road_stop_spec_catalog,
    next_free_road_stop_class_id, next_free_road_stop_spec_id,
};

use super::super::action0::{ParsedRoadStopMeta, collect_roadstop_metas_from_grf};

fn resolve_or_create_road_stop_class(
    classes: &mut Vec<RoadStopClassDef>,
    meta: &ParsedRoadStopMeta,
) -> Option<u16> {
    if let Some(existing) = classes
        .iter()
        .find(|c| c.short_label.eq_ignore_ascii_case(&meta.class_short_label))
    {
        return Some(existing.id);
    }
    let id = next_free_road_stop_class_id(classes)?;
    classes.push(RoadStopClassDef {
        id,
        label: meta.class_label.clone(),
        short_label: meta.class_short_label.clone(),
        from_newgrf: true,
    });
    Some(id)
}

/// Reconstruye catálogos de road stop desde el stack `enabled`.
pub fn apply_newgrf_roadstops(state: &mut GameState, search_dirs: &[&Path]) {
    let mut classes = empty_road_stop_class_catalog();
    let mut specs = empty_road_stop_spec_catalog();
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
            crate::newgrf_sprites::collect_roadstop_sprite_graphics(&data).unwrap_or_default();
        let metas = collect_roadstop_metas_from_grf(&data);
        for (local_idx, meta) in metas.into_iter().enumerate() {
            let Some(class_id) = resolve_or_create_road_stop_class(&mut classes, &meta) else {
                break;
            };
            let Some(spec_id) = next_free_road_stop_spec_id(&specs) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            specs.push(RoadStopSpecDef {
                id: spec_id,
                class: class_id,
                label: meta.label,
                short_label: meta.short_label,
                stop_type: meta.stop_type,
                from_newgrf: true,
                grfid: entry.grfid,
                newgrf_views: views,
            });
        }
    }
    state.road_stop_class_catalog = classes;
    state.road_stop_spec_catalog = specs;
}

/// Aplica `RoadStops` con directorios de búsqueda por defecto.
pub fn apply_newgrf_roadstops_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_roadstops(state, &refs);
}
