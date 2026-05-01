use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, TileKind};

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
    // Estimación inicial rápida sin compensación.
    let mut guess = world_to_tile(world_pos);
    if !in_bounds(guess.0, guess.1) {
        return None;
    }

    // Ajuste iterativo por elevación (tile_min_z): world_y = iso_y + elev.
    for _ in 0..8 {
        let (_, base_z) = tile_slope_and_min_z(map, guess.0 as u32, guess.1 as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let corrected = Vec2::new(world_pos.x, world_pos.y - elev);
        let next = world_to_tile(corrected);
        if next == guess || !in_bounds(next.0, next.1) {
            break;
        }
        guess = next;
    }

    // Desambiguar cerca de bordes: buscar el rombo que realmente contiene el punto.
    let mut best: Option<((i32, i32), f32)> = None;
    for dty in -1..=1 {
        for dtx in -1..=1 {
            let tx = guess.0 + dtx;
            let ty = guess.1 + dty;
            if !in_bounds(tx, ty) {
                continue;
            }
            let (tileh, base_z) = tile_slope_and_min_z(map, tx as u32, ty as u32);
            let tile_kind = map
                .get(TileCoord::new(tx, ty))
                .map_or(TileKind::Grass, |t| t.kind);
            let half_h_base = SLOPE_HALF_H[tileh.min(14) as usize];
            // Carretera plana: algunos sprites (`road_flat_XX`) ocupan hasta 39 px de alto
            // (half_h ~= 19.5). Si usamos 15.5, la zona baja visible “cae” en el tile inferior.
            let half_h = if tileh == 0 && tile_kind == TileKind::Road {
                half_h_base.max(19.5)
            } else {
                half_h_base
            };
            let elev = f32::from(base_z) * HEIGHT_PX;
            let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - half_h + elev);
            let dx = (world_pos.x - center.x).abs() / ISO_HW;
            let dy = (world_pos.y - center.y).abs() / half_h.max(1.0);
            let metric = dx + dy;

            if metric <= 1.000_1 {
                match best {
                    None => best = Some(((tx, ty), metric)),
                    Some((_, cur_metric)) if metric < cur_metric => {
                        best = Some(((tx, ty), metric));
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some((coord, _)) = best {
        Some(coord)
    } else if in_bounds(guess.0, guess.1) {
        Some(guess)
    } else {
        None
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
