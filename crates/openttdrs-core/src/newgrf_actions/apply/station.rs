//! Aplicación de Action0 `Stations` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::badge::resolve_badge_local_ids;
use crate::station_class::{
    StationClassDef, StationClassId, StationSpecDef, StationSpecId, next_free_station_class_id,
    next_free_station_spec_id, vanilla_station_class_catalog, vanilla_station_spec_catalog,
};

use super::super::action0::{ParsedStationMeta, collect_station_metas_from_grf};

fn resolve_or_create_station_class(
    classes: &mut Vec<StationClassDef>,
    meta: &ParsedStationMeta,
) -> Option<StationClassId> {
    if meta.class_short_label.eq_ignore_ascii_case("DFLT") {
        return Some(StationClassId::DEFAULT);
    }
    if let Some(existing) = classes
        .iter()
        .find(|c| c.short_label.eq_ignore_ascii_case(&meta.class_short_label))
    {
        return Some(existing.id);
    }
    let id = next_free_station_class_id(classes)?;
    classes.push(StationClassDef {
        id,
        label: meta.class_label.clone(),
        short_label: meta.class_short_label.clone(),
        from_newgrf: true,
    });
    Some(id)
}

/// Reconstruye catálogos de estación desde el stack `enabled` + vanilla.
#[allow(clippy::too_many_lines)]
pub fn apply_newgrf_stations(state: &mut GameState, search_dirs: &[&Path]) {
    let mut classes = vanilla_station_class_catalog();
    let mut specs = vanilla_station_spec_catalog();
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
        let gfx = crate::newgrf_sprites::collect_station_sprite_graphics(&data).unwrap_or_default();
        let metas = collect_station_metas_from_grf(&data);
        // Resolver copy_layout (0x0F) dentro del mismo GRF por índice local.
        let mut layouts_by_local: Vec<std::collections::HashMap<(u8, u8), Vec<u8>>> =
            Vec::with_capacity(metas.len());
        for meta in &metas {
            let mut layouts = meta.custom_layouts.clone();
            if layouts.is_empty()
                && let Some(src) = meta.copy_layout_from
            {
                let idx = usize::from(src);
                if let Some(src_layouts) = layouts_by_local.get(idx) {
                    layouts.clone_from(src_layouts);
                }
            }
            layouts_by_local.push(layouts);
        }
        for (local_idx, meta) in metas.into_iter().enumerate() {
            let Some(class_id) = resolve_or_create_station_class(&mut classes, &meta) else {
                break;
            };
            let Some(spec_id) = next_free_station_spec_id(&specs) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let preview = views.first().cloned();
            let newgrf_runtime = if gfx.needs_runtime_resolve() || gfx.has_tile_layouts() {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            let (associated_badges, newgrf_badge_translation, unresolved_badges) =
                resolve_badge_local_ids(
                    &meta.badge_local_ids,
                    tables_opt
                        .as_ref()
                        .map_or(&[][..], |tables| tables.badges.as_slice()),
                    &state.badge_catalog,
                    entry.grfid,
                );
            for local_id in unresolved_badges {
                state.runtime.newgrf_diagnostics.push(format!(
                    "{}: station '{}': badge local no resuelto ({local_id})",
                    entry.filename, meta.label
                ));
            }
            let custom_layouts = layouts_by_local.get(local_idx).cloned().unwrap_or_default();
            specs.push(StationSpecDef {
                id: spec_id,
                class: class_id,
                label: meta.label,
                short_label: meta.short_label,
                disallowed_platforms: meta.disallowed_platforms,
                disallowed_lengths: meta.disallowed_lengths,
                callback_mask: meta.callback_mask,
                flags: meta.flags,
                animation_status: meta.animation_status,
                animation_frames: meta.animation_frames,
                animation_speed: meta.animation_speed,
                animation_triggers: meta.animation_triggers,
                from_newgrf: true,
                newgrf_preview: preview,
                newgrf_views: views,
                newgrf_local_id: local_id,
                newgrf_runtime,
                newgrf_grfid: entry.grfid,
                newgrf_grf_version: entry.grf_version,
                newgrf_type_tables: tables_opt.clone(),
                associated_badges,
                newgrf_badge_translation,
                custom_layouts,
            });
        }
    }
    state.station_class_catalog = classes;
    state.station_spec_catalog = specs;
    if !state
        .station_class_catalog
        .iter()
        .any(|c| c.id == state.current_station_class)
    {
        state.current_station_class = StationClassId::DEFAULT;
    }
    if !state
        .station_spec_catalog
        .iter()
        .any(|s| s.id == state.current_station_spec)
    {
        state.current_station_spec = StationSpecId::DEFAULT_RAIL;
    }
}

/// Aplica Stations con directorios de búsqueda por defecto.
pub fn apply_newgrf_stations_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_stations(state, &refs);
}
