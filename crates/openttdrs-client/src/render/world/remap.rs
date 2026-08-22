//! Orquestación de remap del mapa visual.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::render::vehicles::VehicleIndex;
use crate::render::{
    CompanyColoredSprites, MapTileChunk, MapVisualLayer, WorldAssets, chunks_in_bounds,
    large_map_viewport_cull_enabled,
};
use crate::sprites::CompanyColour;
use crate::state::SimWorld;

use super::plugin::{
    LoadedMapTileChunks, MapLabelEntities, MapTileSpawnViewport, NewGrfMapSpriteCaches,
    RemapMapVisualsPending,
};
use super::tile_spawn::{spawn_map_chunk, spawn_world_layer};
use super::viewport::{overview_stride_for_viewport, resolve_spawn_viewport, sync_camera_for_sim};

/// Materializa chunks en un orden canónico, nunca en el orden aleatorio de un
/// `HashSet`.
///
/// El culling puede reconstruir varios chunks después de mover la cámara. En
/// ese camino, el orden de `Commands::spawn` desempata sprites que comparten
/// profundidad en el renderer 2D, así que iterar directamente un `HashSet`
/// hacía que el mismo save dependiera del seed del proceso. El orden es
/// deliberadamente geométrico (fila, columna), no del hash ni de los IDs ECS.
fn canonical_chunk_order(chunks: &std::collections::HashSet<(u32, u32)>) -> Vec<(u32, u32)> {
    let mut ordered: Vec<_> = chunks.iter().copied().collect();
    ordered.sort_unstable_by_key(|&(cx, cy)| (cy, cx));
    ordered
}

pub(crate) fn sync_company_colored_sprites(
    sim: Res<SimWorld>,
    mut company: ResMut<CompanyColoredSprites>,
    mut images: ResMut<Assets<Image>>,
    mut pending: ResMut<RemapMapVisualsPending>,
) {
    let colour = CompanyColour::from_u8(sim.state.company_colour);
    if company.colour == colour {
        return;
    }
    company.colour = colour;
    company.build_all(&mut images);
    pending.pending = true;
    pending.full = true;
    pending.sync_camera = false;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_remap_map_visuals(
    mut commands: Commands,
    mut pending: ResMut<RemapMapVisualsPending>,
    q_vis: Query<Entity, With<MapVisualLayer>>,
    q_chunks: Query<(Entity, &MapTileChunk), With<MapVisualLayer>>,
    mut label_entities: MapLabelEntities,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q_cam: Query<
        (&mut Transform, &mut Projection),
        (
            With<crate::render::PrimaryGameCamera>,
            Without<crate::render::MapPreviewCamera>,
        ),
    >,
    asset_server: Res<AssetServer>,
    assets: Option<Res<WorldAssets>>,
    mut company: Option<ResMut<CompanyColoredSprites>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut newgrf_sprites: NewGrfMapSpriteCaches,
    sim: Res<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut loaded_chunks: ResMut<LoadedMapTileChunks>,
    prefs: Res<crate::settings::ClientPreferences>,
) {
    if !pending.pending {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    let Some(company) = company.as_mut() else {
        return;
    };
    let Some(images) = images.as_mut() else {
        return;
    };
    let do_sync_camera = pending.sync_camera;
    let full_rebuild = pending.full;
    let mut refresh_chunks = std::mem::take(&mut pending.refresh_chunks);
    let labels_dirty = std::mem::take(&mut pending.labels_dirty);
    pending.pending = false;
    pending.sync_camera = false;
    pending.full = true;

    let (mw, mh) = sim.state.map.dimensions();
    if full_rebuild || labels_dirty {
        label_entities.spatial_index.rebuild(&sim.state);
    }
    let spawn_bounds = resolve_spawn_viewport(&sim, &windows, &q_cam);
    let ortho_scale = q_cam
        .single()
        .ok()
        .and_then(|(_, proj)| {
            if let Projection::Orthographic(o) = proj {
                Some(o.scale)
            } else {
                None
            }
        })
        .unwrap_or(1.0);
    let overview_stride = overview_stride_for_viewport(ortho_scale, spawn_bounds);
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
        last_ortho_scale: ortho_scale,
    });

    let use_incremental = !full_rebuild
        && overview_stride.is_none()
        && large_map_viewport_cull_enabled(mw, mh)
        && !loaded_chunks.is_empty();

    let show_pbs = prefs.show_pbs_reservations;
    let show_full_detail = prefs.full_detail;
    let show_town_labels = prefs.show_town_labels;
    let show_station_labels = prefs.show_station_labels;
    let show_waypoint_labels = prefs.show_waypoint_labels;
    let show_competitor_labels = prefs.show_competitor_labels;

    if use_incremental {
        let needed = chunks_in_bounds(spawn_bounds);
        // Solo refrescar chunks dirty que siguen en el viewport (no todo el área visible).
        refresh_chunks.retain(|c| needed.contains(c));
        let plan = loaded_chunks.plan_incremental_remap(&needed, &refresh_chunks);

        for (entity, chunk) in &q_chunks {
            if plan.to_despawn.contains(&(chunk.cx, chunk.cy)) {
                commands.entity(entity).despawn();
            }
        }
        for (cx, cy) in canonical_chunk_order(&plan.to_add) {
            if refresh_chunks.contains(&(cx, cy)) {
                continue;
            }
            spawn_map_chunk(
                &mut commands,
                assets.as_ref(),
                company.as_mut(),
                images.as_mut(),
                &sim,
                cx,
                cy,
                show_pbs,
                show_full_detail,
                newgrf_sprites.road.as_mut(),
                newgrf_sprites.station.as_mut(),
                newgrf_sprites.shore.as_mut(),
                newgrf_sprites.catenary.as_mut(),
                newgrf_sprites.signal.as_mut(),
                newgrf_sprites.industry.as_mut(),
                newgrf_sprites.object.as_mut(),
                newgrf_sprites.action5.as_mut(),
            );
        }
        for (cx, cy) in canonical_chunk_order(&refresh_chunks) {
            if !needed.contains(&(cx, cy)) {
                continue;
            }
            spawn_map_chunk(
                &mut commands,
                assets.as_ref(),
                company.as_mut(),
                images.as_mut(),
                &sim,
                cx,
                cy,
                show_pbs,
                show_full_detail,
                newgrf_sprites.road.as_mut(),
                newgrf_sprites.station.as_mut(),
                newgrf_sprites.shore.as_mut(),
                newgrf_sprites.catenary.as_mut(),
                newgrf_sprites.signal.as_mut(),
                newgrf_sprites.industry.as_mut(),
                newgrf_sprites.object.as_mut(),
                newgrf_sprites.action5.as_mut(),
            );
        }
        loaded_chunks.chunks = needed;
        loaded_chunks.partial_chunks.clear();
        // Etiquetas no van en chunks. Sólo se re-sincronizan tras un cambio de
        // viewport o de una entidad que pueda tener etiqueta. Un refresco de
        // catenaria, señal o reserva PBS no debe despawn/spawn de todas las
        // etiquetas cada tick.
        let viewport_chunks_changed = !plan.to_add.is_empty() || !plan.to_remove.is_empty();
        if labels_dirty || viewport_chunks_changed {
            let label_font = asset_server.load::<Font>(crate::ui::font::UI_FONT_PATH);
            let label_candidates = label_entities.spatial_index.candidates(spawn_bounds);
            let town_entities: Vec<Entity> = label_entities.towns.iter().collect();
            crate::render::town_labels::resync_town_labels(
                &mut commands,
                town_entities,
                &sim,
                &label_font,
                &label_candidates,
                show_town_labels,
            );
            let station_label_entities: Vec<Entity> = label_entities.stations.iter().collect();
            crate::render::station_labels::resync_station_labels(
                &mut commands,
                station_label_entities,
                &sim,
                &label_font,
                &label_candidates,
                show_station_labels,
                show_waypoint_labels,
                show_competitor_labels,
            );
            let sign_entities: Vec<Entity> = label_entities.signs.iter().collect();
            crate::render::sign_labels::resync_sign_labels(
                &mut commands,
                sign_entities,
                &sim,
                &label_font,
                &label_candidates,
                show_competitor_labels,
            );
        }
        if !plan.to_add.is_empty() || !plan.to_remove.is_empty() || !refresh_chunks.is_empty() {
            debug!(
                "Mapa visual incremental: +{} −{} ↻{} chunks ({} teselas visibles)",
                plan.to_add.len(),
                plan.to_remove.len(),
                refresh_chunks.len(),
                spawn_bounds.tile_count()
            );
        }
    } else {
        let to_remove: Vec<Entity> = q_vis.iter().collect();
        for e in to_remove {
            commands.entity(e).despawn();
        }
        vehicle_index.rebuild(&sim.state.vehicles);
        if large_map_viewport_cull_enabled(mw, mh) {
            info!(
                "Mapa visual: {} teselas en viewport (de {})",
                spawn_bounds.tile_count(),
                u64::from(mw) * u64::from(mh)
            );
        }
        spawn_world_layer(
            &mut commands,
            &asset_server,
            assets.as_ref(),
            company.as_mut(),
            images.as_mut(),
            &sim,
            &label_entities.spatial_index,
            spawn_bounds,
            true,
            show_pbs,
            show_full_detail,
            show_town_labels,
            show_station_labels,
            show_waypoint_labels,
            show_competitor_labels,
            overview_stride,
            newgrf_sprites.road.as_mut(),
            newgrf_sprites.station.as_mut(),
            newgrf_sprites.shore.as_mut(),
            newgrf_sprites.catenary.as_mut(),
            newgrf_sprites.signal.as_mut(),
            newgrf_sprites.industry.as_mut(),
            newgrf_sprites.object.as_mut(),
            newgrf_sprites.action5.as_mut(),
        );
        loaded_chunks.set_spawn_bounds(spawn_bounds, mw, mh);
    }

    if do_sync_camera {
        sync_camera_for_sim(&mut q_cam, &sim);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::canonical_chunk_order;

    #[test]
    fn canonical_chunk_order_does_not_depend_on_hashset_insertion_order() {
        let mut first = HashSet::new();
        let mut second = HashSet::new();
        for chunk in [(2, 1), (0, 0), (1, 0), (0, 1)] {
            first.insert(chunk);
        }
        for chunk in [(0, 1), (1, 0), (0, 0), (2, 1)] {
            second.insert(chunk);
        }

        let expected = vec![(0, 0), (1, 0), (0, 1), (2, 1)];
        assert_eq!(canonical_chunk_order(&first), expected);
        assert_eq!(canonical_chunk_order(&second), expected);
    }
}
