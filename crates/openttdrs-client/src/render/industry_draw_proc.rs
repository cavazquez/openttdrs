//! Overlays `draw_proc` (1–5) — chispas, burbujas, toy factory, etc.

use bevy::prelude::*;
use openttdrs_core::TileCoord;

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, remap_tile_offset};
use crate::render::tiles::leveled_foundation_overlay_pos;
use crate::render::{
    IndustryOverlayContext, MapVisualLayer, ViewportSortableChild, WorldAssets,
    viewport_source_depth,
};
use crate::sprites::{
    DrawProcLayer, industry_draw_proc_anim_frame, industry_draw_proc_for_tile,
    industry_draw_proc_layer_for_slot, industry_draw_proc_layer_sample_for_slot,
    industry_draw_proc_layer_slot_count, industry_sprite_uses_fizzy_drink_anim,
};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct IndustryDrawProcPlugin;

impl Plugin for IndustryDrawProcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_industry_draw_proc_layers
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Capa animada por `draw_proc`; una entidad por slot hijo, incluso si el
/// frame inicial lo mantiene oculto.
#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryDrawProcAnim {
    proc: u8,
    slot: u8,
    ctx: IndustryOverlayContext,
    /// Dimensión del mapa que produjo esta capa; conserva el slot fuente aun
    /// después de que el compositor haya movido al parent.
    map_width: u32,
}

const DRAW_PROC_WH: (f32, f32, f32, f32) = (48.0, 48.0, -24.0, -24.0);

impl IndustryDrawProcAnim {
    fn pos3(&self, layer: &DrawProcLayer) -> Vec3 {
        let (w, h, xrel, yrel) = DRAW_PROC_WH;
        let off = remap_tile_offset(layer.dx as f32, layer.dy as f32, 0.0) * 0.5;
        let anchor = self.ctx.iso_pos + off;
        if self.ctx.leveled {
            leveled_foundation_overlay_pos(
                anchor,
                xrel,
                yrel,
                w,
                h,
                self.ctx.base_z,
                0.56,
                self.ctx.tx,
                self.ctx.ty,
            )
        } else {
            overlay_pos(
                anchor,
                xrel,
                yrel,
                w,
                h,
                self.ctx.overlay_z,
                0.56,
                self.ctx.tx,
                self.ctx.ty,
            )
        }
    }

    fn source_depth(&self, position: Vec3) -> f32 {
        viewport_source_depth(position.z, self.ctx.tx.max(0) as u32, self.map_width)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_industry_draw_proc_overlays(
    commands: &mut Commands,
    assets: &WorldAssets,
    map_width: u32,
    gfx: u16,
    m1: u8,
    m3hi: u8,
    overlay_ctx: IndustryOverlayContext,
    chunk: crate::render::MapTileChunk,
    parent: Option<Entity>,
) {
    let proc = industry_draw_proc_for_tile(gfx, m1);
    if proc == 0 {
        return;
    }
    let frame = industry_draw_proc_anim_frame(m3hi);
    for slot in 0..industry_draw_proc_layer_slot_count(proc) {
        let current_layer = industry_draw_proc_layer_for_slot(proc, m1, frame, slot);
        // Un procedimiento puede no dibujar un slot en el primer frame (por
        // ejemplo, chispas en frame 0). Se instancia oculto con una muestra
        // válida para poder activarlo en el siguiente tick sin recrear chunk.
        let Some(layer) =
            current_layer.or_else(|| industry_draw_proc_layer_sample_for_slot(proc, m1, slot))
        else {
            continue;
        };
        let fizzy = industry_sprite_uses_fizzy_drink_anim(layer.sprite_id)
            && assets.fizzy_drink_frames.contains_key(&layer.sprite_id);
        let Some(img) = (if fizzy {
            assets
                .fizzy_drink_frames
                .get(&layer.sprite_id)
                .and_then(|f| f.first())
        } else {
            assets.industries.get(&layer.sprite_id)
        }) else {
            continue;
        };
        let anim = IndustryDrawProcAnim {
            proc,
            slot,
            ctx: overlay_ctx,
            map_width,
        };
        let mut pos3 = anim.pos3(&layer);
        // `IndustryDraw*` se ejecuta inmediatamente después de
        // `AddSortableSpriteToDraw` y añade estas capas con
        // `AddChildSpriteScreen`. En la ruta plana el edificio ya tiene su
        // parent exacto; conservar la profundidad fuente deja que el sorter
        // emita edificio y efectos como un bloque atómico.
        let sortable_child = parent.map(|parent| {
            let source_depth = anim.source_depth(pos3);
            pos3.z = source_depth;
            ViewportSortableChild {
                parent,
                source_depth,
            }
        });
        let mut entity = commands.spawn((
            MapVisualLayer,
            chunk,
            anim,
            img.sprite(),
            Transform::from_translation(pos3),
            if current_layer.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ));
        if let Some(child) = sortable_child {
            entity.insert(child);
        }
        if fizzy {
            entity.insert(crate::render::FizzyDrinkAnim {
                sprite_id: layer.sprite_id,
            });
        }
    }
}

pub(crate) fn animate_industry_draw_proc_layers(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut q: Query<(
        &IndustryDrawProcAnim,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
        Option<&crate::render::FizzyDrinkAnim>,
        Option<&mut ViewportSortableChild>,
    )>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (anim, mut sprite, mut transform, mut visibility, fizzy, sortable_child) in &mut q {
        let coord = TileCoord::new(anim.ctx.tx, anim.ctx.ty);
        let Some(tile) = sim.state.map.get(coord) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let gfx = openttdrs_core::industry_gfx(&tile);
        let proc = industry_draw_proc_for_tile(gfx, tile.m1);
        if proc != anim.proc {
            *visibility = Visibility::Hidden;
            continue;
        }
        let frame = industry_draw_proc_anim_frame(tile.m3hi);
        let Some(layer) = industry_draw_proc_layer_for_slot(proc, tile.m1, frame, anim.slot) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(img) = assets.industries.get(&layer.sprite_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Visible;
        if fizzy.is_none() && !img.matches(&sprite) {
            img.apply_to(&mut sprite);
        }
        let pos3 = anim.pos3(&layer);
        // La Z efectiva de un child pertenece al compositor. La animación
        // actualiza su ancla X/Y y su profundidad fuente, sin restaurar el
        // slot que el sorter ya resolvió entre el parent y su siguiente vecino.
        let translation = if sortable_child.is_some() {
            Vec3::new(pos3.x, pos3.y, transform.translation.z)
        } else {
            pos3
        };
        if transform.translation != translation {
            transform.translation = translation;
        }
        if let Some(mut child) = sortable_child {
            let source_depth = anim.source_depth(pos3);
            if child.source_depth != source_depth {
                child.source_depth = source_depth;
            }
        }
    }
}
