//! Utilidades de proyección isométrica.

use bevy::prelude::*;

/// Desplazamiento horizontal por tesela en pantalla (la tesela mide 64 px de ancho).
pub const ISO_HW: f32 = 32.0;
/// Desplazamiento vertical por tesela en pantalla (ratio 2:1 isométrico).
pub const ISO_QH: f32 = 16.0;
/// La mitad de la altura de los sprites de tesela (64×31 → 15.5 px).
pub const TILE_HALF_H: f32 = 15.5;
/// Píxeles de elevación en Y por cada unidad de altura de `OpenTTD`.
pub const HEIGHT_PX: f32 = 8.0;

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

/// Hash de Wang para generar variación determinista (sin RNG en el core).
pub fn wang_hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = seed
        .wrapping_add(x.wrapping_mul(0x9E37_79B9))
        .wrapping_add(y.wrapping_mul(0x6C62_272E));
    h ^= h >> 16;
    h = h.wrapping_mul(0x045D_9F3B);
    h ^= h >> 16;
    h
}
