//! Opt-in bin order on the same render frame as the sprite input capture.
//! Requests cross worlds outside ECS so enabling the diagnostic does not add
//! main-world resources or components that can change source query order.

use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Mutex,
};

use bevy::core_pipeline::core_2d::{AlphaMask2d, AlphaMask2dBinKey};
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::{
    Extract,
    camera::ExtractedCamera,
    render_phase::{BinnedRenderPhase, ViewBinnedRenderPhases},
    sync_world::MainEntity,
    view::ExtractedView,
};
use bevy::sprite_render::RenderMesh2dInstances;
use serde_json::{Value, json};

static REQUESTS: Mutex<Vec<(PathBuf, u32)>> = Mutex::new(Vec::new());

pub(super) fn request(path: PathBuf, frame: u32) {
    let Ok(mut requests) = REQUESTS.lock() else {
        error!("mask trace request lock is poisoned");
        return;
    };
    requests.push((path, frame));
}

#[derive(Default, Resource)]
pub(super) struct PendingDrawOrderRequests {
    extracted_frame: u32,
    requests: Vec<(PathBuf, u32)>,
}

/// Extraction runs synchronously after Main. Draining in Render instead could
/// consume Main's next-frame request while the prior frame is still rendering.
pub(super) fn extract_requests(
    frame: Extract<Res<FrameCount>>,
    mut pending: ResMut<PendingDrawOrderRequests>,
) {
    let Ok(mut requests) = REQUESTS.lock() else {
        error!("mask trace request lock is poisoned");
        return;
    };
    pending.extracted_frame = frame.0;
    pending.requests = std::mem::take(&mut *requests);
}

fn instance(entity: MainEntity, instances: &RenderMesh2dInstances) -> Value {
    json!({
        "main_entity": entity.id().to_bits(),
        "world_from_local_bits": instances.get(&entity).map(|mesh| {
            Mat4::from(mesh.transforms.world_from_local).to_cols_array().map(f32::to_bits)
        }),
    })
}

fn bin_key(key: &AlphaMask2dBinKey) -> Value {
    json!({
        "pipeline": format!("{:?}", key.pipeline),
        "draw_function": format!("{:?}", key.draw_function),
        "mesh_asset": format!("{:?}", key.asset_id),
        "material_bind_group": format!("{:?}", key.material_bind_group_id),
    })
}

fn capture_bins(
    phase: &BinnedRenderPhase<AlphaMask2d>,
    instances: &RenderMesh2dInstances,
) -> Value {
    let batchable: Vec<_> = phase
        .batchable_meshes
        .iter()
        .map(|((batch_key, key), bin)| {
            let entities: Vec<_> = bin
                .entities()
                .keys()
                .map(|&entity| instance(entity, instances))
                .collect();
            json!({"indexed": batch_key.indexed, "bin_key": bin_key(key), "entities": entities})
        })
        .collect();
    let unbatchable: Vec<_> = phase
        .unbatchable_meshes
        .iter()
        .map(|((batch_key, key), bin)| {
            let entities: Vec<_> = bin
                .entities
                .keys()
                .map(|&entity| instance(entity, instances))
                .collect();
            json!({"indexed": batch_key.indexed, "bin_key": bin_key(key), "entities": entities})
        })
        .collect();
    json!({
        "batchable_bins": batchable,
        "unbatchable_bins": unbatchable,
        "multidrawable_bins": phase.multidrawable_meshes.len(),
        "non_mesh_bins": phase.non_mesh_items.len(),
    })
}

pub(super) fn export_requested_draw_order(
    phases: Res<ViewBinnedRenderPhases<AlphaMask2d>>,
    instances: Res<RenderMesh2dInstances>,
    views: Query<(Entity, &ExtractedView, &ExtractedCamera)>,
    mut pending: ResMut<PendingDrawOrderRequests>,
) {
    let requests = std::mem::take(&mut pending.requests);
    if requests.is_empty() {
        return;
    }
    let mut rows = Vec::new();
    for (entity, view, camera) in &views {
        if camera.order != -101 {
            continue;
        }
        let Some(phase) = phases.get(&view.retained_view_entity) else {
            continue;
        };
        rows.push(json!({
            "render_view_entity": entity.to_bits(),
            "main_view_entity": view.retained_view_entity.main_entity.id().to_bits(),
            "order": camera.order,
            "clip_from_view_bits": view.clip_from_view.to_cols_array().map(f32::to_bits),
            "world_from_view_bits": view.world_from_view.to_matrix().to_cols_array().map(f32::to_bits),
            "phase": capture_bins(phase, &instances),
        }));
    }
    let mut document = json!({
        "schema": 1,
        "extracted_frame_count": pending.extracted_frame,
        "scope": "AlphaMask2d bin sequence after PrepareBindGroups; instance enumeration is not a GPU command/readback trace",
        "views": rows,
    });
    for (path, request_frame) in requests {
        // FrameCount increments in Last, after the PostUpdate input capture.
        if pending.extracted_frame != request_frame.wrapping_add(1) {
            error!(
                "mask draw order {}: capture/extraction frames do not match",
                path.display()
            );
            continue;
        }
        document["input_frame_count"] = json!(request_frame);
        let result = (|| -> io::Result<()> {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let mut writer = io::BufWriter::new(file);
            serde_json::to_writer(&mut writer, &document).map_err(io::Error::other)?;
            writer.flush()
        })();
        if let Err(error) = result {
            error!("mask draw order {}: {error}", path.display());
        } else {
            info!("mask draw order saved to {}", path.display());
        }
    }
}
