use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, TileKind, rail_signals::resolve_signal_track};

use super::{HEIGHT_PX, ISO_HW, ISO_QH, SLOPE_HALF_H, TILE_HALF_H, tile_slope_and_min_z};

/// Convierte coordenadas de tesela a posición del vértice superior del rombo (Bevy Y-up).
///
/// Fórmula de OpenTTD: `screen_x = (ty - tx) * half_tile_w` (igual que `RemapCoords` con z=0).
/// Con esto: +tx mueve al SW (abajo-izquierda), +ty mueve al SE (abajo-derecha),
/// lo que produce la orientación Norte-arriba estándar de OpenTTD.
#[inline]
pub fn iso(tx: i32, ty: i32) -> Vec2 {
    Vec2::new((ty - tx) as f32 * ISO_HW, (tx + ty) as f32 * -ISO_QH)
}

/// Convierte posición del mundo a coordenadas de tesela (inversa de `iso`).
#[inline]
pub fn world_to_tile(world_pos: Vec2) -> (i32, i32) {
    let a = world_pos.x / ISO_HW; // = ty - tx
    let b = world_pos.y / -ISO_QH; // = tx + ty
    let ty = f32::midpoint(a, b);
    let tx = (b - a) / 2.0;
    (tx.floor() as i32, ty.floor() as i32)
}

/// Límite estricto del rombo (`dx + dy <= 1`).
const PICK_METRIC_STRICT: f32 = 1.000_1;
/// Margen para vértices/bordes del rombo y solapes entre teselas vecinas.
const PICK_METRIC_RELAXED: f32 = 1.10;

fn pick_metric_raw(map: &Map, tx: i32, ty: i32, world_pos: Vec2) -> f32 {
    let (tileh, base_z) = tile_slope_and_min_z(map, tx as u32, ty as u32);
    let tile_kind = map
        .get(TileCoord::new(tx, ty))
        .map_or(TileKind::Grass, |t| t.kind);
    let half_h_base = SLOPE_HALF_H[tileh.min(14) as usize];
    let half_h = if tileh == 0 && tile_kind == TileKind::Road {
        half_h_base.max(19.5)
    } else {
        half_h_base
    };
    let elev = f32::from(base_z) * HEIGHT_PX;
    let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - half_h + elev);
    let dx = (world_pos.x - center.x).abs() / ISO_HW;
    let dy = (world_pos.y - center.y).abs() / half_h.max(1.0);
    dx + dy
}

/// Busca la tesela más cercana en un vecindario cuadrado; `max_metric` acota el rombo.
#[allow(clippy::too_many_arguments)]
fn pick_tile_in_neighborhood(
    map: &Map,
    seed_tx: i32,
    seed_ty: i32,
    world_pos: Vec2,
    mw_i: i32,
    mh_i: i32,
    radius: i32,
    max_metric: f32,
) -> Option<(i32, i32)> {
    let in_bounds = |tx: i32, ty: i32| tx >= 0 && ty >= 0 && tx < mw_i && ty < mh_i;
    let mut best: Option<((i32, i32), f32)> = None;
    for dty in -radius..=radius {
        for dtx in -radius..=radius {
            let tx = seed_tx + dtx;
            let ty = seed_ty + dty;
            if !in_bounds(tx, ty) {
                continue;
            }
            let metric = pick_metric_raw(map, tx, ty, world_pos);
            if metric > max_metric {
                continue;
            }
            match best {
                None => best = Some(((tx, ty), metric)),
                Some((_, cur)) if metric < cur => best = Some(((tx, ty), metric)),
                _ => {}
            }
        }
    }
    best.map(|(c, _)| c)
}

/// Convierte posición en mundo (p. ej. [`Camera::viewport_to_world_2d`]) a tesela del mapa.
///
/// El cálculo hace dos pasos:
/// 1) estimación por inversión lineal de [`iso`] compensando elevación (`z*HEIGHT_PX`);
/// 2) desambiguación geométrica entre candidatos vecinos usando la ecuación del rombo
///    `abs(dx)/ISO_HW + abs(dy)/ISO_QH <= 1`.
///
/// Esto evita que el `floor` crudo de [`world_to_tile`] “parta” visualmente un rombo
/// en dos teselas cuando hay elevación o redondeo cerca de diagonales.
#[must_use]
pub fn world_pos_to_tile_coord(world_pos: Vec2, map: &Map) -> Option<(i32, i32)> {
    let (mw, mh) = map.dimensions();
    let mw_i = mw as i32;
    let mh_i = mh as i32;

    let in_bounds = |tx: i32, ty: i32| tx >= 0 && ty >= 0 && tx < mw_i && ty < mh_i;
    let raw = world_to_tile(world_pos);

    let mut seed = if in_bounds(raw.0, raw.1) {
        raw
    } else {
        let near_map = raw.0 >= -1 && raw.0 <= mw_i && raw.1 >= -1 && raw.1 <= mh_i;
        if !near_map {
            return None;
        }
        (raw.0.clamp(0, mw_i - 1), raw.1.clamp(0, mh_i - 1))
    };

    for _ in 0..8 {
        if !in_bounds(seed.0, seed.1) {
            break;
        }
        let (_, base_z) = tile_slope_and_min_z(map, seed.0 as u32, seed.1 as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let corrected = Vec2::new(world_pos.x, world_pos.y - elev);
        let next = world_to_tile(corrected);
        if next == seed {
            break;
        }
        if in_bounds(next.0, next.1) {
            seed = next;
        } else if next.0 >= -1 && next.0 <= mw_i && next.1 >= -1 && next.1 <= mh_i {
            seed = (next.0.clamp(0, mw_i - 1), next.1.clamp(0, mh_i - 1));
            break;
        } else {
            break;
        }
    }

    if let Some(hit) = pick_tile_in_neighborhood(
        map,
        seed.0,
        seed.1,
        world_pos,
        mw_i,
        mh_i,
        1,
        PICK_METRIC_STRICT,
    ) {
        return Some(hit);
    }
    pick_tile_in_neighborhood(
        map,
        seed.0,
        seed.1,
        world_pos,
        mw_i,
        mh_i,
        2,
        PICK_METRIC_RELAXED,
    )
}

/// Fracción dentro de la tesela (0–255), como `_tile_fract_coords` de `OpenTTD`.
///
/// Convierte la posición del cursor respecto al centro del rombo a coordenadas
/// `TILE_SEQ` (0–16) y las escala; alinea con `viewport.cpp` (`fract_x > fract_y`
/// → carril izquierdo, etc.).
#[must_use]
pub fn world_pos_to_tile_fract(world_pos: Vec2, map: &Map, tx: i32, ty: i32) -> (u8, u8) {
    let (tileh, base_z) = tile_slope_and_min_z(map, tx as u32, ty as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh.min(14) as usize]
    };
    let elev = f32::from(base_z) * HEIGHT_PX;
    let top = iso(tx, ty);
    let center = Vec2::new(top.x, top.y - half_h + elev);
    let rel = world_pos - center;

    // Inversa de `remap_tile_offset(dx, dy, 0)` con origen en el centro del rombo.
    // dx,dy en 0..16 con (8,8) = centro de tesela.
    let dy_minus_dx = rel.x / 4.0;
    let dx_plus_dy = -rel.y / 2.0;
    let dy = (dy_minus_dx + dx_plus_dy) * 0.5;
    let dx = dx_plus_dy - dy;

    let fx = ((dx + 8.0).clamp(0.0, 16.0) / 16.0 * 255.0).round() as u8;
    let fy = ((dy + 8.0).clamp(0.0, 16.0) / 16.0 * 255.0).round() as u8;
    (fx, fy)
}

/// Tesela ferroviaria bajo el cursor para colocar señales (`GenericPlaceSignals`).
///
/// Busca vía en un vecindario 5×5 alrededor del pick geométrico y elige la tesela
/// con riel válido más cercana al cursor (métrica del rombo). El `fract` se calcula
/// respecto al centro de esa tesela, no de la hierba adyacente.
#[must_use]
pub fn world_pos_to_rail_signal_pick(world_pos: Vec2, map: &Map) -> Option<(i32, i32, u8, u8)> {
    let seed = world_pos_to_tile_coord(world_pos, map)?;
    let (mw, mh) = map.dimensions();
    let mw_i = mw as i32;
    let mh_i = mh as i32;
    let in_bounds = |tx: i32, ty: i32| tx >= 0 && ty >= 0 && tx < mw_i && ty < mh_i;

    // Tesela bajo el cursor con vía válida (paridad GetTileBelowCursor + GenericPlaceSignals).
    if in_bounds(seed.0, seed.1) {
        let coord = TileCoord::new(seed.0, seed.1);
        if let Some(tile) = map.get(coord).filter(|t| t.kind == TileKind::Rail) {
            let tb = tile.m5 & 0x3F;
            if tb != 0 {
                let fract = world_pos_to_tile_fract(world_pos, map, seed.0, seed.1);
                if resolve_signal_track(tb, fract.0, fract.1).is_some() {
                    return Some((seed.0, seed.1, fract.0, fract.1));
                }
            }
        }
    }

    let mut best: Option<((i32, i32), f32, (u8, u8))> = None;

    for dty in -2..=2 {
        for dtx in -2..=2 {
            let tx = seed.0 + dtx;
            let ty = seed.1 + dty;
            if !in_bounds(tx, ty) {
                continue;
            }
            let coord = TileCoord::new(tx, ty);
            let Some(tile) = map.get(coord) else {
                continue;
            };
            if tile.kind != TileKind::Rail {
                continue;
            }
            let tb = tile.m5 & 0x3F;
            if tb == 0 {
                continue;
            }
            let fract = world_pos_to_tile_fract(world_pos, map, tx, ty);
            if resolve_signal_track(tb, fract.0, fract.1).is_none() {
                continue;
            }
            let metric = pick_metric_raw(map, tx, ty, world_pos);
            if metric > PICK_METRIC_RELAXED {
                continue;
            }
            match &best {
                None => best = Some(((tx, ty), metric, fract)),
                Some(((bx, by), bm, _))
                    if rail_signal_pick_better(seed, (tx, ty), metric, (*bx, *by), *bm) =>
                {
                    best = Some(((tx, ty), metric, fract));
                }
                _ => {}
            }
        }
    }

    best.map(|((tx, ty), _, (fx, fy))| (tx, ty, fx, fy))
}

/// Desempate entre teselas ferroviarias con métrica similar (p. ej. vía diagonal en cadena).
fn rail_signal_pick_better(
    seed: (i32, i32),
    cand: (i32, i32),
    cand_metric: f32,
    best: (i32, i32),
    best_metric: f32,
) -> bool {
    const EPS: f32 = 0.001;
    if cand_metric + EPS < best_metric {
        return true;
    }
    if cand_metric > best_metric + EPS {
        return false;
    }
    let cand_dist = (cand.0 - seed.0).abs() + (cand.1 - seed.1).abs();
    let best_dist = (best.0 - seed.0).abs() + (best.1 - seed.1).abs();
    if cand_dist != best_dist {
        return cand_dist < best_dist;
    }
    // Estable: preferir la tesela del pick geométrico si empata.
    cand == seed || (best != seed && (cand.0, cand.1) < best)
}

#[cfg(test)]
mod rail_signal_pick_tests {
    use super::rail_signal_pick_better;

    #[test]
    fn tie_break_prefers_rail_tile_closer_to_geometric_seed() {
        let seed = (10, 10);
        assert!(rail_signal_pick_better(
            seed,
            (11, 10),
            0.55,
            (12, 10),
            0.55
        ));
        assert!(!rail_signal_pick_better(
            seed,
            (12, 10),
            0.55,
            (11, 10),
            0.55
        ));
    }

    #[test]
    fn lower_metric_always_wins() {
        let seed = (3, 3);
        assert!(rail_signal_pick_better(seed, (4, 4), 0.4, (3, 4), 0.9));
    }
}

/// Vec3 para teselas de suelo con soporte de altura isométrica.
#[inline]
pub fn tile_pos_half(tx: i32, ty: i32, height: u8, layer: f32, half_h: f32) -> Vec3 {
    let p = iso(tx, ty);
    let elev = f32::from(height) * HEIGHT_PX;
    Vec3::new(
        p.x,
        p.y - half_h + elev,
        (tx + ty) as f32 * 0.01 + f32::from(height) * 0.001 + layer,
    )
}

/// [`tile_pos_half`] con la altura estándar de tesela 64×31.
#[inline]
pub fn tile_pos(tx: i32, ty: i32, height: u8, layer: f32) -> Vec3 {
    tile_pos_half(tx, ty, height, layer, TILE_HALF_H)
}

/// Posición en pantalla de `(x_pos, y_pos)` de un vehículo OpenTTD.
///
/// En este cliente `iso(tx, ty) == RemapCoords(16·tx, 16·ty) / 2`; los offsets
/// sub-tesela deben usar la misma escala (no el `remap` entero de piezas BUILD).
#[must_use]
pub fn road_vehicle_tile_anchor(tx: i32, ty: i32, sub_x: f32, sub_y: f32, sub_z: f32) -> Vec2 {
    remap_tile_offset(tx as f32 * 16.0 + sub_x, ty as f32 * 16.0 + sub_y, sub_z) * 0.5
}

/// Delta de pantalla (Bevy) para un offset local `TILE_SEQ` dentro de la tesela.
///
/// Equivalente a `RemapCoords(dx,dy,dz) - RemapCoords(0,0,0)` en zoom Normal de OpenTTD,
/// escalado al rombo ~64×31 px del cliente (`ISO_HW`/`ISO_QH`).
///
#[must_use]
pub fn remap_tile_offset(dx: f32, dy: f32, dz: f32) -> Vec2 {
    const PX_PER_X_UNIT: f32 = 4.0; // (dy - dx) * 2 * ZOOM_BASE * (ISO_HW / 64)
    const PY_PER_Y_UNIT: f32 = 2.0; // (dx + dy - dz) * ZOOM_BASE * escala
    Vec2::new((dy - dx) * PX_PER_X_UNIT, -(dx + dy - dz) * PY_PER_Y_UNIT)
}

/// Origen `TILE_SEQ` + offsets NFO de una pieza BUILD de parada.
#[derive(Debug, Clone, Copy)]
pub struct RoadStopSeqGfx {
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub x_offs: f32,
    pub y_offs: f32,
    /// Ajuste extra de Δx en unidades TILE_SEQ (×4 px), por capa en datos generados.
    pub remap_x_adj: f32,
}

/// Posición mundo (ancla **superior izquierda**, como OpenTTD) para una pieza de parada.
#[must_use]
#[allow(dead_code)]
pub fn road_stop_sprite_pos(
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
    seq: RoadStopSeqGfx,
) -> Vec3 {
    let anchor = iso(tx, ty);
    let elev = f32::from(base_z) * HEIGHT_PX;
    const PX_PER_X_UNIT: f32 = 4.0;
    let off = remap_tile_offset(seq.dx, seq.dy, seq.dz);
    Vec3::new(
        anchor.x + off.x + seq.x_offs + seq.remap_x_adj * PX_PER_X_UNIT,
        anchor.y + off.y - seq.y_offs + elev,
        (tx + ty) as f32 * 0.01 + f32::from(base_z) * 0.001 + layer_z,
    )
}

/// `xrel`/`yrel` para [`overlay_pos`] (ancla esquina norte + `RemapCoords` + offsets NFO).
#[must_use]
pub fn road_stop_overlay_rel(seq: RoadStopSeqGfx) -> (f32, f32) {
    const PX_PER_X_UNIT: f32 = 4.0;
    let off = remap_tile_offset(seq.dx, seq.dy, seq.dz);
    (
        off.x + seq.x_offs + seq.remap_x_adj * PX_PER_X_UNIT,
        -off.y + seq.y_offs,
    )
}

/// Centro Bevy de una capa BUILD (misma cadena que estaciones de tren).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn road_stop_build_sprite_center(
    ref_pos: Vec2,
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
    seq: RoadStopSeqGfx,
    w: f32,
    h: f32,
) -> Vec3 {
    let (xrel, yrel) = road_stop_overlay_rel(seq);
    overlay_pos(ref_pos, xrel, yrel, w, h, base_z, layer_z, tx, ty)
}

/// `xrel`/`yrel` para depósito de carretera (como [`rail_station_overlay_rel`]).
///
/// `iso(tx, ty)` ya incluye `RemapCoords(16·tx, 16·ty) / 4`; el delta TILE_SEQ local
/// debe usar la misma escala (`remap_tile_offset` × 0.5), no la de paradas bus/camión.
#[must_use]
pub fn road_depot_overlay_rel(seq: RoadStopSeqGfx) -> (f32, f32) {
    let off = remap_tile_offset(seq.dx, seq.dy, seq.dz) * 0.5;
    (
        off.x + seq.x_offs + seq.remap_x_adj * 2.0,
        seq.y_offs - off.y,
    )
}

/// Centro Bevy de una capa BUILD del depósito de carretera.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn road_depot_build_sprite_center(
    ref_pos: Vec2,
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
    seq: RoadStopSeqGfx,
    w: f32,
    h: f32,
) -> Vec3 {
    let (xrel, yrel) = road_depot_overlay_rel(seq);
    overlay_pos(ref_pos, xrel, yrel, w, h, base_z, layer_z, tx, ty)
}

/// Posición mundo (ancla **superior izquierda**) para una pieza de depósito de carretera.
#[must_use]
#[allow(dead_code)] // usado en tests de alineación (`iso/mod.rs`)
pub fn road_depot_sprite_pos(
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
    seq: RoadStopSeqGfx,
) -> Vec3 {
    let anchor = iso(tx, ty);
    let elev = f32::from(base_z) * HEIGHT_PX;
    let (xrel, yrel) = road_depot_overlay_rel(seq);
    Vec3::new(
        anchor.x + xrel,
        anchor.y - yrel + elev,
        (tx + ty) as f32 * 0.01 + f32::from(base_z) * 0.001 + layer_z,
    )
}

/// Calcula la posición del centro de un sprite overlay a partir del xrel/yrel del NFO.
#[allow(clippy::too_many_arguments)]
pub fn overlay_pos(
    ref_pos: Vec2,
    xrel: f32,
    yrel: f32,
    w: f32,
    h: f32,
    height: u8,
    layer: f32,
    tx: i32,
    ty: i32,
) -> Vec3 {
    let elev = f32::from(height) * HEIGHT_PX;
    Vec3::new(
        ref_pos.x + xrel + w / 2.0,
        ref_pos.y - yrel - h / 2.0 + elev,
        (tx + ty) as f32 * 0.01 + f32::from(height) * 0.001 + layer,
    )
}

/// Dibuja el contorno de un rombo isométrico.
pub fn gizmo_diamond(gizmos: &mut Gizmos, center: Vec2, hw: f32, hh: f32, color: Color) {
    let t = center + Vec2::new(0.0, hh);
    let r = center + Vec2::new(hw, 0.0);
    let b = center + Vec2::new(0.0, -hh);
    let l = center + Vec2::new(-hw, 0.0);
    gizmos.line_2d(t, r, color);
    gizmos.line_2d(r, b, color);
    gizmos.line_2d(b, l, color);
    gizmos.line_2d(l, t, color);
}
