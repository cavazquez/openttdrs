//! Overlays `draw_proc` (1–5) — chispas, burbujas, toy factory, etc.

use bevy::prelude::*;
use openttdrs_core::TileCoord;

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, remap_tile_offset};
use crate::render::tiles::leveled_foundation_overlay_pos;
use crate::render::{IndustryOverlayContext, MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    DrawProcLayer, industry_draw_proc_anim_frame, industry_draw_proc_dynamic_layers,
    industry_draw_proc_for_tile,
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

/// Capa animada por `draw_proc`; una entidad por sprite hijo.
#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryDrawProcAnim {
    proc: u8,
    part: u8,
    ctx: IndustryOverlayContext,
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
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_industry_draw_proc_overlays(
    commands: &mut Commands,
    assets: &WorldAssets,
    _ctx: &TileRenderContext,
    gfx: u16,
    m1: u8,
    m3hi: u8,
    overlay_ctx: IndustryOverlayContext,
    chunk: crate::render::MapTileChunk,
) {
    let proc = industry_draw_proc_for_tile(gfx, m1);
    if proc == 0 {
        return;
    }
    let frame = industry_draw_proc_anim_frame(m3hi);
    let layers = industry_draw_proc_dynamic_layers(proc, m1, frame);
    for (part, layer) in layers.iter().enumerate() {
        let Some(img) = assets.industries.get(&layer.sprite_id) else {
            continue;
        };
        let anim = IndustryDrawProcAnim {
            proc,
            part: part as u8,
            ctx: overlay_ctx,
        };
        let pos3 = anim.pos3(layer);
        commands.spawn((
            MapVisualLayer,
            chunk,
            anim,
            img.sprite(),
            Transform::from_translation(pos3),
            Visibility::Visible,
        ));
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
    )>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (anim, mut sprite, mut transform, mut visibility) in &mut q {
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
        let layers = industry_draw_proc_dynamic_layers(proc, tile.m1, frame);
        let Some(layer) = layers.get(usize::from(anim.part)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(img) = assets.industries.get(&layer.sprite_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Visible;
        if !img.matches(&sprite) {
            img.apply_to(&mut sprite);
        }
        let pos3 = anim.pos3(layer);
        transform.translation = pos3;
    }
}
