//! Aplicación de Action0 `RoadStops` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::badge::resolve_badge_labels_detailed;
use crate::road_stop_spec::{
    RoadStopClassDef, RoadStopSpecDef, empty_road_stop_class_catalog, empty_road_stop_spec_catalog,
    next_free_road_stop_class_id, next_free_road_stop_spec_id, road_stop_spec_by_grf_local,
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
#[allow(clippy::too_many_lines)]
pub fn apply_newgrf_roadstops(state: &mut GameState, search_dirs: &[&Path]) {
    // Snapshot identidad estable → rebind `Station.road_stop_spec` tras rebuild.
    let station_bindings: Vec<(usize, u32, u8)> = state
        .stations
        .iter()
        .enumerate()
        .filter_map(|(i, st)| {
            let id = st.road_stop_spec?;
            let def = state.road_stop_spec_catalog.iter().find(|d| d.id == id)?;
            Some((i, def.grfid, def.newgrf_local_id))
        })
        .collect();
    let current_binding = state.current_road_stop_spec.and_then(|id| {
        state
            .road_stop_spec_catalog
            .iter()
            .find(|d| d.id == id)
            .map(|d| (d.grfid, d.newgrf_local_id))
    });

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
        let type_tables = crate::newgrf_type_tables::collect_type_tables_from_grf(&data);
        let type_tables = (!type_tables.is_empty()).then_some(type_tables);
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
            let newgrf_runtime = if gfx.needs_runtime_resolve() {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            if let Some(err) = &meta.badge_list_error {
                state.runtime.newgrf_diagnostics.push(format!(
                    "{}: roadstop '{}': {err}",
                    entry.filename, meta.label
                ));
            }
            let (associated_badges, unresolved) = resolve_badge_labels_detailed(
                &meta.badge_labels,
                &state.badge_catalog,
                entry.grfid,
            );
            for label in unresolved {
                state.runtime.newgrf_diagnostics.push(format!(
                    "{}: roadstop '{}': badge desconocido '{label}'",
                    entry.filename, meta.label
                ));
            }
            specs.push(RoadStopSpecDef {
                id: spec_id,
                class: class_id,
                label: meta.label,
                short_label: meta.short_label,
                stop_type: meta.stop_type,
                from_newgrf: true,
                grfid: entry.grfid,
                newgrf_local_id: local_id,
                draw_mode: meta.draw_mode,
                flags: meta.flags,
                callback_mask: meta.callback_mask,
                animation_status: meta.animation_status,
                animation_frames: meta.animation_frames,
                animation_speed: meta.animation_speed,
                animation_triggers: meta.animation_triggers,
                newgrf_views: views,
                newgrf_runtime,
                newgrf_type_tables: type_tables.clone(),
                associated_badges,
            });
        }
    }

    for (station_idx, grfid, local_id) in station_bindings {
        let new_id = road_stop_spec_by_grf_local(&specs, grfid, local_id).map(|d| d.id);
        if let Some(st) = state.stations.get_mut(station_idx) {
            st.road_stop_spec = new_id;
        }
    }
    state.current_road_stop_spec = current_binding.and_then(|(grfid, local_id)| {
        road_stop_spec_by_grf_local(&specs, grfid, local_id).map(|d| d.id)
    });
    state.current_road_stop_class = state
        .current_road_stop_spec
        .and_then(|id| specs.iter().find(|d| d.id == id).map(|d| d.class));
    if state
        .current_road_stop_class
        .is_some_and(|id| !classes.iter().any(|c| c.id == id))
    {
        state.current_road_stop_class = None;
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
