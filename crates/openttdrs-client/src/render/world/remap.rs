//! Orquestación de remap del mapa visual.

use std::collections::HashSet;

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
use super::viewport::{resolve_spawn_viewport, sync_camera_for_sim};

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
    label_entities: MapLabelEntities,
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
    pending.pending = false;
    pending.sync_camera = false;
    pending.full = true;

    let (mw, mh) = sim.state.map.dimensions();
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
    commands.insert_resource(MapTileSpawnViewport {
        bounds: spawn_bounds,
        last_ortho_scale: ortho_scale,
    });

    let use_incremental = !full_rebuild
        && large_map_viewport_cull_enabled(mw, mh)
        && !loaded_chunks.chunks.is_empty();

    let show_pbs = prefs.show_pbs_reservations;
    let show_full_detail = prefs.full_detail;
    let show_town_labels = prefs.show_town_labels;
    let show_station_labels = prefs.show_station_labels;

    if use_incremental {
        let needed = chunks_in_bounds(spawn_bounds);
        // Solo refrescar chunks dirty que siguen en el viewport (no todo el área visible).
        refresh_chunks.retain(|c| needed.contains(c));
        let to_remove: HashSet<_> = loaded_chunks.chunks.difference(&needed).copied().collect();
        let to_add: HashSet<_> = needed.difference(&loaded_chunks.chunks).copied().collect();

        for (entity, chunk) in &q_chunks {
            if to_remove.contains(&(chunk.cx, chunk.cy)) {
                commands.entity(entity).despawn();
            }
        }
        for &(cx, cy) in &to_add {
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
        let mut refresh_despawn = Vec::new();
        for (entity, chunk) in &q_chunks {
            if refresh_chunks.contains(&(chunk.cx, chunk.cy)) {
                refresh_despawn.push(entity);
            }
        }
        for entity in refresh_despawn {
            commands.entity(entity).despawn();
        }
        for &(cx, cy) in &refresh_chunks {
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
        // Etiquetas no van en chunks: re-sincronizar al panear el viewport.
        let label_font = asset_server.load::<Font>(crate::ui::font::UI_FONT_PATH);
        let town_entities: Vec<Entity> = label_entities.towns.iter().collect();
        crate::render::town_labels::resync_town_labels(
            &mut commands,
            town_entities,
            &sim,
            &label_font,
            spawn_bounds,
            show_town_labels,
        );
        let station_label_entities: Vec<Entity> = label_entities.stations.iter().collect();
        crate::render::station_labels::resync_station_labels(
            &mut commands,
            station_label_entities,
            &sim,
            &label_font,
            spawn_bounds,
            show_station_labels,
        );
        let sign_entities: Vec<Entity> = label_entities.signs.iter().collect();
        crate::render::sign_labels::resync_sign_labels(
            &mut commands,
            sign_entities,
            &sim,
            &label_font,
            spawn_bounds,
        );
        if !to_add.is_empty() || !to_remove.is_empty() || !refresh_chunks.is_empty() {
            info!(
                "Mapa visual incremental: +{} −{} ↻{} chunks ({} teselas visibles)",
                to_add.len(),
                to_remove.len(),
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
            spawn_bounds,
            true,
            show_pbs,
            show_full_detail,
            show_town_labels,
            show_station_labels,
            newgrf_sprites.road.as_mut(),
            newgrf_sprites.station.as_mut(),
            newgrf_sprites.shore.as_mut(),
            newgrf_sprites.catenary.as_mut(),
            newgrf_sprites.signal.as_mut(),
            newgrf_sprites.industry.as_mut(),
            newgrf_sprites.object.as_mut(),
            newgrf_sprites.action5.as_mut(),
        );
        loaded_chunks.chunks = chunks_in_bounds(spawn_bounds);
    }

    if do_sync_camera {
        sync_camera_for_sim(&mut q_cam, &sim);
    }
}
