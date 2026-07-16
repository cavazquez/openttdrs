//! Animación de capas estáticas de industria con `anim_state` (torres, pozos).
//!
//! OpenTTD indexa `_industry_draw_tile_data` con `m4 & 3`. P7 avanza `m3hi` en
//! la simulación; el cliente lee el frame vivo del mapa cada frame.

use bevy::prelude::*;
use openttdrs_core::industry_gfx as core_industry_gfx;
use openttdrs_core::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, wang_hash};
use crate::render::tiles::leveled_foundation_overlay_pos;
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{industry_effective_m4_for_draw, industry_gfx_entry_for_tile};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct IndustryBuildingAnimPlugin;

impl Plugin for IndustryBuildingAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_industry_building_layers
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Contexto para recalcular posición al cambiar sprite (offsets NFO por frame).
#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryOverlayContext {
    pub(crate) iso_pos: Vec2,
    pub(crate) base_z: u8,
    pub(crate) overlay_z: u8,
    pub(crate) leveled: bool,
    pub(crate) tx: i32,
    pub(crate) ty: i32,
}

/// Capa de suelo o edificio que cicla con `anim_state` + frame `m4`.
#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryBuildingAnim {
    gfx: u16,
    m1: u8,
    /// Fase inicial: `m3hi` del save o hash de tesela.
    phase: u8,
    ground: bool,
    ctx: IndustryOverlayContext,
}

impl IndustryBuildingAnim {
    pub(crate) fn new(
        gfx: u16,
        m1: u8,
        phase: u8,
        ground: bool,
        ctx: IndustryOverlayContext,
    ) -> Self {
        Self {
            gfx,
            m1,
            phase,
            ground,
            ctx,
        }
    }
}

impl IndustryOverlayContext {
    pub(crate) fn from_tile_ctx(
        ctx: &TileRenderContext,
        base_z: u8,
        overlay_z: u8,
        leveled: bool,
    ) -> Self {
        Self {
            iso_pos: ctx.iso_pos,
            base_z,
            overlay_z,
            leveled,
            tx: ctx.tx_i32(),
            ty: ctx.ty_i32(),
        }
    }

    fn overlay_at(&self, xrel: f32, yrel: f32, w: f32, h: f32, layer: f32) -> Vec3 {
        if self.leveled {
            leveled_foundation_overlay_pos(
                self.iso_pos,
                xrel,
                yrel,
                w,
                h,
                self.base_z,
                layer,
                self.tx,
                self.ty,
            )
        } else {
            overlay_pos(
                self.iso_pos,
                xrel,
                yrel,
                w,
                h,
                self.overlay_z,
                layer,
                self.tx,
                self.ty,
            )
        }
    }
}

/// Fase estable por tesela (desincroniza torres/pozos adyacentes).
#[must_use]
pub(crate) fn industry_anim_phase(tx: i32, ty: i32, m4: u8) -> u8 {
    let tx = tx.max(0) as u32;
    let ty = ty.max(0) as u32;
    (wang_hash(tx, ty, 0x1A07) as u8).wrapping_add(m4 & 3)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_industry_anim_layer(
    commands: &mut Commands,
    assets: &WorldAssets,
    chunk: crate::render::MapTileChunk,
    anim: IndustryBuildingAnim,
    sprite_id: u32,
    xrel: f32,
    yrel: f32,
    w: f32,
    h: f32,
    layer: f32,
) {
    let Some(img) = assets.industries.get(&sprite_id) else {
        return;
    };
    let pos3 = anim.ctx.overlay_at(xrel, yrel, w, h, layer);
    commands.spawn((
        MapVisualLayer,
        chunk,
        anim,
        img.sprite(),
        Transform::from_translation(pos3),
        Visibility::Visible,
    ));
}

pub(crate) fn animate_industry_building_layers(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut q: Query<(
        &IndustryBuildingAnim,
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
        let (gfx, m1, m3hi) = sim
            .state
            .map
            .get(coord)
            .map(|t| (core_industry_gfx(&t), t.m1, t.m3hi))
            .unwrap_or((anim.gfx, anim.m1, anim.phase));
        let m4 = industry_effective_m4_for_draw(gfx, m1, m3hi, 0.0, 0);
        let Some(entry) = industry_gfx_entry_for_tile(gfx, m1, m4) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let (sprite_id, w, h, xrel, yrel) = if anim.ground {
            (
                entry.ground_sprite_id,
                entry.ground_w,
                entry.ground_h,
                entry.ground_xrel,
                entry.ground_yrel,
            )
        } else {
            (entry.sprite_id, entry.w, entry.h, entry.xrel, entry.yrel)
        };
        if sprite_id == 0 || w <= 0.0 || h <= 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let Some(img) = assets.industries.get(&sprite_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Visible;
        if !img.matches(&sprite) {
            img.apply_to(&mut sprite);
        }
        transform.translation =
            anim.ctx
                .overlay_at(xrel, yrel, w, h, if anim.ground { 0.45 } else { 0.5 });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn anim_phase_varies_by_tile() {
        assert_ne!(industry_anim_phase(0, 0, 0), industry_anim_phase(1, 0, 0));
    }
}
