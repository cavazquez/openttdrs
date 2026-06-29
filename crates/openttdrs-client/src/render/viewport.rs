//! Ventana de teselas visibles para culling al generar sprites del mapa.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::config::{env_flag, env_u32_in_range};
use crate::iso::world_to_tile;
use crate::render::components::MAP_TILE_CHUNK_SIZE;

/// Rectángulo de teselas `[tx0, tx1) × [ty0, ty1)` en coordenadas de mapa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileViewportBounds {
    pub tx0: u32,
    pub ty0: u32,
    pub tx1: u32,
    pub ty1: u32,
}

impl TileViewportBounds {
    #[must_use]
    pub const fn full(mw: u32, mh: u32) -> Self {
        Self {
            tx0: 0,
            ty0: 0,
            tx1: mw,
            ty1: mh,
        }
    }

    #[must_use]
    pub fn expand(self, margin: u32, mw: u32, mh: u32) -> Self {
        let tx0 = self.tx0.saturating_sub(margin);
        let ty0 = self.ty0.saturating_sub(margin);
        let tx1 = (self.tx1 + margin).min(mw);
        let ty1 = (self.ty1 + margin).min(mh);
        Self { tx0, ty0, tx1, ty1 }
    }

    /// `true` si `other` está totalmente dentro de `self`.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.tx0 <= other.tx0
            && self.ty0 <= other.ty0
            && self.tx1 >= other.tx1
            && self.ty1 >= other.ty1
    }

    #[must_use]
    pub fn tile_count(self) -> u64 {
        u64::from(self.tx1.saturating_sub(self.tx0)) * u64::from(self.ty1.saturating_sub(self.ty0))
    }

    pub fn iter_coords(self) -> impl Iterator<Item = (u32, u32)> {
        (self.ty0..self.ty1).flat_map(move |ty| (self.tx0..self.tx1).map(move |tx| (tx, ty)))
    }
}

/// Umbral por defecto: mapas con al menos este número de teselas usan culling por viewport.
pub const LARGE_MAP_TILE_THRESHOLD: u32 = 1_024;

/// Ancho visible inicial (teselas) cuando el culling está activo.
const DEFAULT_VIEWPORT_SPAN_TILES: f32 = 64.0;

/// Margen extra (teselas) alrededor del rectángulo visible (rombos isométricos).
pub const VIEWPORT_MARGIN_TILES: u32 = 10;

/// Si la vista sale más de N teselas del bloque ya generado, se vuelve a generar sprites.
pub const VIEWPORT_REBUILD_LEAD_TILES: u32 = 6;

fn map_viewport_tile_threshold() -> u32 {
    env_u32_in_range(
        "OPENTTDRS_MAP_VIEWPORT_THRESHOLD",
        LARGE_MAP_TILE_THRESHOLD,
        256..=65_536,
    )
}

#[must_use]
pub fn large_map_viewport_cull_enabled(mw: u32, mh: u32) -> bool {
    mw.saturating_mul(mh) >= map_viewport_tile_threshold()
        && !env_flag("OPENTTDRS_MAP_VIEWPORT_OFF")
}

/// Zoom ortográfico inicial: mapas pequeños encuadran todo el mapa; mapas grandes
/// mantienen ~64 teselas visibles (culling por viewport), cargados o partida nueva.
#[must_use]
pub fn initial_camera_span_tiles(mw: u32, mh: u32, _loaded_from_file: bool) -> f32 {
    let span = mw.max(mh).max(1) as f32;
    if large_map_viewport_cull_enabled(mw, mh) {
        DEFAULT_VIEWPORT_SPAN_TILES
    } else {
        span
    }
}

/// Estima el rectángulo de teselas bajo el viewport ortográfico 2D.
#[must_use]
pub fn ortho_visible_tile_bounds(
    cam_world: Vec2,
    ortho_scale: f32,
    window_width: f32,
    window_height: f32,
    mw: u32,
    mh: u32,
    margin_tiles: u32,
) -> TileViewportBounds {
    let half_w = window_width * 0.5 * ortho_scale;
    let half_h = window_height * 0.5 * ortho_scale;
    let corners = [
        Vec2::new(cam_world.x - half_w, cam_world.y - half_h),
        Vec2::new(cam_world.x + half_w, cam_world.y - half_h),
        Vec2::new(cam_world.x - half_w, cam_world.y + half_h),
        Vec2::new(cam_world.x + half_w, cam_world.y + half_h),
    ];

    let mut tx_min = i32::MAX;
    let mut ty_min = i32::MAX;
    let mut tx_max = i32::MIN;
    let mut ty_max = i32::MIN;
    for c in corners {
        let (tx, ty) = world_to_tile(c);
        tx_min = tx_min.min(tx);
        ty_min = ty_min.min(ty);
        tx_max = tx_max.max(tx);
        ty_max = ty_max.max(ty);
    }

    if tx_min == i32::MAX {
        return TileViewportBounds::full(mw, mh);
    }

    let extra = margin_tiles as i32 + 2;
    tx_min -= extra;
    ty_min -= extra;
    tx_max += extra;
    ty_max += extra;

    let mw_i = mw as i32;
    let mh_i = mh as i32;
    let tx0 = tx_min.clamp(0, mw_i) as u32;
    let ty0 = ty_min.clamp(0, mh_i) as u32;
    let tx1 = (tx_max + 1).clamp(0, mw_i) as u32;
    let ty1 = (ty_max + 1).clamp(0, mh_i) as u32;

    TileViewportBounds {
        tx0,
        ty0,
        tx1: tx1.max(tx0 + 1).min(mw),
        ty1: ty1.max(ty0 + 1).min(mh),
    }
}

/// Rectángulo de teselas de un bloque `MAP_TILE_CHUNK_SIZE`×`MAP_TILE_CHUNK_SIZE`.
#[must_use]
pub fn chunk_tile_bounds(cx: u32, cy: u32, mw: u32, mh: u32) -> TileViewportBounds {
    TileViewportBounds {
        tx0: cx * MAP_TILE_CHUNK_SIZE,
        ty0: cy * MAP_TILE_CHUNK_SIZE,
        tx1: ((cx + 1) * MAP_TILE_CHUNK_SIZE).min(mw),
        ty1: ((cy + 1) * MAP_TILE_CHUNK_SIZE).min(mh),
    }
}

/// Conjunto de bloques que intersectan `bounds`.
#[must_use]
pub fn chunks_in_bounds(bounds: TileViewportBounds) -> HashSet<(u32, u32)> {
    if bounds.tx1 <= bounds.tx0 || bounds.ty1 <= bounds.ty0 {
        return HashSet::new();
    }
    let cx0 = bounds.tx0 / MAP_TILE_CHUNK_SIZE;
    let cy0 = bounds.ty0 / MAP_TILE_CHUNK_SIZE;
    let cx1 = bounds.tx1.saturating_sub(1) / MAP_TILE_CHUNK_SIZE;
    let cy1 = bounds.ty1.saturating_sub(1) / MAP_TILE_CHUNK_SIZE;
    let mut out = HashSet::new();
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            out.insert((cx, cy));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_bounds_contains_smaller_rect() {
        let full = TileViewportBounds::full(256, 256);
        let inner = TileViewportBounds {
            tx0: 10,
            ty0: 20,
            tx1: 40,
            ty1: 50,
        };
        assert!(full.contains(inner));
        assert!(!inner.contains(full));
    }

    #[test]
    fn visible_bounds_clamp_to_map() {
        let b = ortho_visible_tile_bounds(Vec2::new(0.0, 0.0), 1.0, 1280.0, 720.0, 64, 64, 4);
        assert!(b.tx1 <= 64);
        assert!(b.ty1 <= 64);
        assert!(b.tile_count() <= 64 * 64);
    }

    #[test]
    fn initial_camera_span_small_map_fits_whole_map() {
        assert_eq!(initial_camera_span_tiles(12, 8, true), 12.0);
        assert_eq!(initial_camera_span_tiles(12, 8, false), 12.0);
    }

    #[test]
    fn initial_camera_span_large_map_uses_viewport_window() {
        assert_eq!(initial_camera_span_tiles(256, 256, true), 64.0);
        assert_eq!(initial_camera_span_tiles(256, 256, false), 64.0);
    }

    #[test]
    fn large_map_threshold_at_32_squared() {
        assert!(large_map_viewport_cull_enabled(32, 32));
        assert!(!large_map_viewport_cull_enabled(31, 31));
        assert!(large_map_viewport_cull_enabled(256, 256));
    }

    #[test]
    fn chunks_in_bounds_covers_viewport() {
        let bounds = TileViewportBounds {
            tx0: 10,
            ty0: 20,
            tx1: 50,
            ty1: 40,
        };
        let chunks = chunks_in_bounds(bounds);
        assert!(chunks.contains(&(0, 1)));
        assert!(chunks.contains(&(3, 2)));
        assert_eq!(chunks.len(), 4 * 2);
    }

    #[test]
    fn chunk_tile_bounds_clamps_to_map() {
        let b = chunk_tile_bounds(15, 15, 256, 256);
        assert_eq!(b.tx0, 240);
        assert_eq!(b.ty0, 240);
        assert_eq!(b.tx1, 256);
        assert_eq!(b.ty1, 256);
    }
}
