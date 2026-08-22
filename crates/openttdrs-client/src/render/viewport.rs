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

    /// Recorre las teselas en el mismo barrido diagonal que
    /// `ViewportAddLandscape` de OpenTTD.
    ///
    /// OpenTTD incrementa primero `row = x + y` y, dentro de cada fila,
    /// `column = y - x`. Como `column` ascendente equivale a `x` descendente,
    /// este orden conserva la inserción de `DrawGroundSprite` aun cuando dos
    /// claves de profundidad `f32` muy próximas empatan.
    pub fn iter_coords(self) -> impl Iterator<Item = (u32, u32)> {
        DiagonalTileCoords::new(self)
    }
}

/// Iterador de coordenadas para el barrido de tiles de OpenTTD.
///
/// Mantenerlo separado del culling evita que el renderer dependa de un orden
/// incidental fila-a-fila (`ty`, luego `tx`) al crear entidades de Bevy.
#[derive(Clone, Copy, Debug)]
struct DiagonalTileCoords {
    bounds: TileViewportBounds,
    row: u32,
    last_row: u32,
    min_tx: u32,
    next_tx: Option<u32>,
}

impl DiagonalTileCoords {
    fn new(bounds: TileViewportBounds) -> Self {
        if bounds.tx0 >= bounds.tx1 || bounds.ty0 >= bounds.ty1 {
            return Self {
                bounds,
                row: 0,
                last_row: 0,
                min_tx: 0,
                next_tx: None,
            };
        }

        let row = bounds.tx0.saturating_add(bounds.ty0);
        let mut out = Self {
            bounds,
            row,
            last_row: bounds
                .tx1
                .saturating_sub(1)
                .saturating_add(bounds.ty1.saturating_sub(1)),
            min_tx: 0,
            next_tx: None,
        };
        out.set_row(row);
        out
    }

    fn set_row(&mut self, row: u32) {
        self.row = row;
        let last_tx = self.bounds.tx1.saturating_sub(1);
        let last_ty = self.bounds.ty1.saturating_sub(1);
        self.min_tx = self.bounds.tx0.max(row.saturating_sub(last_ty));
        let max_tx = last_tx.min(row.saturating_sub(self.bounds.ty0));
        self.next_tx = (self.min_tx <= max_tx).then_some(max_tx);
    }
}

impl Iterator for DiagonalTileCoords {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<Self::Item> {
        let tx = self.next_tx?;
        let ty = self.row.saturating_sub(tx);

        if tx > self.min_tx {
            self.next_tx = Some(tx - 1);
        } else if self.row < self.last_row {
            self.set_row(self.row + 1);
        } else {
            self.next_tx = None;
        }

        Some((tx, ty))
    }
}

/// Umbral por defecto: mapas con al menos este número de teselas usan culling por viewport.
pub const LARGE_MAP_TILE_THRESHOLD: u32 = 1_024;

/// Ancho visible inicial (teselas) cuando el culling está activo.
const DEFAULT_VIEWPORT_SPAN_TILES: f32 = 64.0;

/// Máximo lado del rectángulo de spawn en mapas grandes (~192² ≈ 37k teselas).
/// Sin esto, zoom 0.05× (ortho scale 20) instancia cientos de miles de sprites.
pub const MAX_SPAWN_SPAN_TILES: u32 = 192;

/// Zoom más cercano (ortho scale mínimo).
pub const MIN_ORTHO_SCALE: f32 = 0.25;

/// Techo absoluto de alejamiento (mapas pequeños / sin culling).
pub const ABSOLUTE_MAX_ORTHO_SCALE: f32 = 20.0;

/// A partir de este nivel los mapas que superan el presupuesto de detalle usan
/// la representación agregada de overview. OpenTTD conserva `Out4x` y `Out8x`
/// también en mapas grandes, pero el cliente evita materializar allí cientos
/// de miles de sprites de una sola vez.
pub const OVERVIEW_MIN_ORTHO_SCALE: f32 = 4.0;

/// Máximo de teselas materializadas con detalle en `Out4x`/`Out8x`.
///
/// El presupuesto corresponde al viewport, no al tamaño completo del mapa:
/// al panear un 1024×1024 sólo se instancian las teselas que realmente caben
/// en pantalla. Conservar hasta 512² teselas mantiene infraestructura,
/// edificios y vehículos en esos niveles sin volver a construir un mapa
/// 4096² entero.
pub const OVERVIEW_DETAIL_MAX_VIEWPORT_TILES: u64 = 512 * 512;

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

/// Márgenes que `resolve_spawn_viewport` suma al AABB visible (ambos lados).
const SPAWN_BOUND_PAD_TILES: u32 = VIEWPORT_MARGIN_TILES + 2 + VIEWPORT_REBUILD_LEAD_TILES;

/// Escala ortho máxima para que el AABB isométrico de spawn quepa en [`MAX_SPAWN_SPAN_TILES`].
///
/// En iso, el span en teselas de un rectángulo de pantalla es
/// `scale * (w/(2·ISO_HW) + h/(2·ISO_QH))` — no solo el ancho. Un tope solo por ancho
/// dejaba huecos diagonales al recortar el spawn.
#[must_use]
pub fn max_ortho_scale_for_window(window_width: f32, window_height: f32) -> f32 {
    use crate::iso::{ISO_HW, ISO_QH};
    let w = window_width.max(320.0);
    let h = window_height.max(240.0);
    let pad = 2 * SPAWN_BOUND_PAD_TILES;
    let budget = MAX_SPAWN_SPAN_TILES.saturating_sub(pad).max(32) as f32;
    // span_tx = span_ty = half_w/ISO_HW + half_h/ISO_QH
    let coeff = w / (2.0 * ISO_HW) + h / (2.0 * ISO_QH);
    let scale = budget / coeff.max(1.0);
    scale.clamp(MIN_ORTHO_SCALE, ABSOLUTE_MAX_ORTHO_SCALE)
}

/// Acota el zoom: en mapas con culling, el alejamiento máximo evita spawn masivo.
#[must_use]
pub fn clamp_ortho_scale(
    scale: f32,
    window_width: f32,
    window_height: f32,
    large_map_cull: bool,
) -> f32 {
    let scale = scale.clamp(MIN_ORTHO_SCALE, ABSOLUTE_MAX_ORTHO_SCALE);
    if !large_map_cull || scale >= OVERVIEW_MIN_ORTHO_SCALE {
        return scale;
    }

    // Antes de `Out4x` el renderer materializa sprites individuales y debe
    // respetar el presupuesto de chunks. En `Out4x`/`Out8x`, los mapas grandes
    // cambian a `spawn_overview_tiles_in_bounds`; los medianos mantienen
    // detalle. Esta bifurcación permite el alejamiento máximo de OpenTTD sin
    // un spawn masivo en mapas de gran tamaño.
    scale.min(max_ortho_scale_for_window(window_width, window_height))
}

#[must_use]
pub const fn overview_stride_for_scale(scale: f32) -> Option<u32> {
    if scale >= 8.0 {
        Some(8)
    } else if scale >= OVERVIEW_MIN_ORTHO_SCALE {
        Some(4)
    } else {
        None
    }
}

/// Selecciona overview sólo cuando el viewport supera el presupuesto de detalle.
///
/// La escala ni el tamaño total del mapa bastan: un mapa 1024×1024 puede tener
/// un viewport de sólo 512² teselas. Mantener ese recorte en el camino
/// detallado evita reemplazar infraestructura por rombos de color al alejar la
/// cámara. La reducción 4×4/8×8 queda reservada para viewports realmente
/// mayores, como protección contra un spawn masivo.
#[must_use]
pub fn overview_stride_for_viewport(scale: f32, bounds: TileViewportBounds) -> Option<u32> {
    if bounds.tile_count() <= OVERVIEW_DETAIL_MAX_VIEWPORT_TILES {
        None
    } else {
        overview_stride_for_scale(scale)
    }
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
    fn iter_coords_matches_openttd_diagonal_row_and_column_scan() {
        let bounds = TileViewportBounds {
            tx0: 2,
            ty0: 3,
            tx1: 5,
            ty1: 6,
        };

        // `ViewportAddLandscape`: row = x + y ascendente; para cada row,
        // column = y - x ascendente, es decir x descendente.
        assert_eq!(
            bounds.iter_coords().collect::<Vec<_>>(),
            vec![
                (2, 3),
                (3, 3),
                (2, 4),
                (4, 3),
                (3, 4),
                (2, 5),
                (4, 4),
                (3, 5),
                (4, 5),
            ]
        );
    }

    #[test]
    fn iter_coords_is_empty_for_an_empty_viewport() {
        let bounds = TileViewportBounds {
            tx0: 4,
            ty0: 3,
            tx1: 4,
            ty1: 6,
        };
        assert!(bounds.iter_coords().next().is_none());
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

    #[test]
    fn max_ortho_scale_keeps_spawn_aabb_within_cap() {
        let max = max_ortho_scale_for_window(1280.0, 720.0);
        assert!(max < ABSOLUTE_MAX_ORTHO_SCALE);
        assert!(max > MIN_ORTHO_SCALE);
        // Tope iso correcto (~3.7 @ 1280×720), no el antiguo ~9.6 solo-por-ancho.
        assert!(max < 5.0, "scale={max}");
        assert!(max > 2.5, "scale={max}");

        let b = ortho_visible_tile_bounds(
            Vec2::ZERO,
            max,
            1280.0,
            720.0,
            4096,
            4096,
            VIEWPORT_MARGIN_TILES,
        )
        .expand(VIEWPORT_REBUILD_LEAD_TILES, 4096, 4096);
        assert!(
            b.tx1 - b.tx0 <= MAX_SPAWN_SPAN_TILES,
            "tx span {}",
            b.tx1 - b.tx0
        );
        assert!(
            b.ty1 - b.ty0 <= MAX_SPAWN_SPAN_TILES,
            "ty span {}",
            b.ty1 - b.ty0
        );
    }

    #[test]
    fn clamp_ortho_scale_caps_detail_but_keeps_overview_levels() {
        let detail_cap = max_ortho_scale_for_window(1280.0, 720.0);
        assert_eq!(clamp_ortho_scale(3.95, 1280.0, 720.0, true), detail_cap);
        assert_eq!(
            clamp_ortho_scale(20.0, 1280.0, 720.0, true),
            ABSOLUTE_MAX_ORTHO_SCALE
        );
        assert_eq!(clamp_ortho_scale(20.0, 1280.0, 720.0, false), 20.0);
    }

    #[test]
    fn overview_stride_matches_openttd_out_levels() {
        assert_eq!(overview_stride_for_scale(2.0), None);
        assert_eq!(overview_stride_for_scale(4.0), Some(4));
        assert_eq!(overview_stride_for_scale(8.0), Some(8));
    }

    #[test]
    fn overview_keeps_detail_while_viewport_fits_budget() {
        assert_eq!(
            overview_stride_for_viewport(8.0, TileViewportBounds::full(512, 512)),
            None
        );
        assert_eq!(
            overview_stride_for_viewport(
                8.0,
                TileViewportBounds {
                    tx0: 200,
                    ty0: 300,
                    tx1: 712,
                    ty1: 812,
                },
            ),
            None
        );
        assert_eq!(
            overview_stride_for_viewport(
                8.0,
                TileViewportBounds {
                    tx0: 0,
                    ty0: 0,
                    tx1: 513,
                    ty1: 512,
                },
            ),
            Some(8)
        );
        assert_eq!(
            overview_stride_for_viewport(4.0, TileViewportBounds::full(1024, 1024)),
            Some(4)
        );
        assert_eq!(
            overview_stride_for_viewport(8.0, TileViewportBounds::full(1024, 1024)),
            Some(8)
        );
    }
}
