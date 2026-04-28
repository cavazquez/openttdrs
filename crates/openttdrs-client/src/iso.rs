//! Utilidades de proyección isométrica.

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, TileKind};

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

// ── Pendientes (slopes) ───────────────────────────────────────────────────────

/// `half_h` para `tile_pos_half` según el índice `tileh` (0–14).
///
/// Derivado de los campos `height` y `yrel` del NFO de OpenGFX:
/// - Plano (tileh=0): 31 px, yrel=0 → half_h = 15.5
/// - Pendiente con esquina N elevada (bit 3): yrel=-8, h varía → half_h menor
///
/// Bitmask de `tileh` (idéntico al `Slope` de OpenTTD):
/// `bit0=W, bit1=S, bit2=E, bit3=N`
pub const SLOPE_HALF_H: [f32; 15] = [
    15.5, // 0:  flat
    15.5, // 1:  W
    11.5, // 2:  S
    11.5, // 3:  WS
    15.5, // 4:  E
    15.5, // 5:  WE
    11.5, // 6:  SE
    11.5, // 7:  WSE
    11.5, // 8:  N
    11.5, // 9:  NW
    7.5,  // 10: NS
    7.5,  // 11: NWS
    11.5, // 12: NE
    11.5, // 13: NWE
    7.5,  // 14: NSE
];

/// Calcula el bitmask de pendiente (`tileh`) de la tesela `(tx, ty)` igual que
/// OpenTTD [`GetTileSlopeZ`] / `GetTileSlopeGivenHeight` (`tile_map.cpp`):
///
/// ```text
/// hnorth = height(tx,   ty  )
/// hwest  = height(tx+1, ty  )
/// heast  = height(tx,   ty+1)
/// hsouth = height(tx+1, ty+1)
/// min_h = min(los cuatro)
/// si hnorth > min_h → SLOPE_N (8); hwest → SLOPE_W (1);
///    heast → SLOPE_E (4); hsouth → SLOPE_S (2)
/// ```
///
/// El resultado está limitado a 0–14 (pendientes simples; las empinadas (15)
/// requieren sprites especiales y se omiten por ahora).
#[must_use]
pub fn compute_tileh(map: &Map, tx: u32, ty: u32) -> u8 {
    let get_h = |dtx: i32, dty: i32| map.get(TileCoord::new(dtx, dty)).map_or(0, |t| t.height);
    let hnorth = get_h(tx as i32, ty as i32);
    let hwest = get_h(tx as i32 + 1, ty as i32);
    let heast = get_h(tx as i32, ty as i32 + 1);
    let hsouth = get_h(tx as i32 + 1, ty as i32 + 1);
    let min_h = hnorth.min(hwest).min(heast).min(hsouth);
    let mut tileh: u8 = 0;
    if hwest > min_h {
        tileh |= 1;
    } // SLOPE_W
    if hsouth > min_h {
        tileh |= 2;
    } // SLOPE_S
    if heast > min_h {
        tileh |= 4;
    } // SLOPE_E
    if hnorth > min_h {
        tileh |= 8;
    } // SLOPE_N
    tileh.min(14)
}

/// Altura base de la tesela para dibujar el suelo: **mínimo** de las cuatro esquinas
/// (misma muestra que `compute_tileh`), equivalente a `GetTileZ` en OpenTTD (`tile_map.cpp`).
///
/// OpenTTD ancla los sprites de terreno a la esquina **más baja** del rombo; usar solo
/// `Tile.height` (esquina N de esa celda) desplaza verticalmente las pendientes y abre
/// huecos entre teselas vecinas.
#[must_use]
pub fn tile_min_corner_height(map: &Map, tx: u32, ty: u32) -> u8 {
    let get_h = |dtx: i32, dty: i32| map.get(TileCoord::new(dtx, dty)).map_or(0, |t| t.height);
    let hnorth = get_h(tx as i32, ty as i32);
    let hwest = get_h(tx as i32 + 1, ty as i32);
    let heast = get_h(tx as i32, ty as i32 + 1);
    let hsouth = get_h(tx as i32 + 1, ty as i32 + 1);
    hnorth.min(hwest).min(heast).min(hsouth)
}

/// [`tile_min_corner_height`] para una coordenada de tesela (vehículos, etc.).
#[must_use]
pub fn tile_min_z(map: &Map, c: TileCoord) -> u8 {
    if c.x < 0 || c.y < 0 {
        return 0;
    }
    let Ok(ux) = u32::try_from(c.x) else {
        return 0;
    };
    let Ok(uy) = u32::try_from(c.y) else {
        return 0;
    };
    let (mw, mh) = map.dimensions();
    if ux >= mw || uy >= mh {
        return 0;
    }
    tile_min_corner_height(map, ux, uy)
}

/// Cuando las cuatro alturas del bloque 2×2 son iguales, [`compute_tileh`] da **0**.
/// En la costa eso es habitual: la tierra puede estar **fuera** de ese bloque (p. ej.
/// solo al norte), así que OpenTTD guarda agua homogénea pero **`DrawShoreTile`**
/// sigue siendo no plano (`water_cmd.cpp` aserte `tileh != SLOPE_FLAT`).
///
/// Inferimos una pendiente de **una esquina** mirando vecinos ortogonales de tierra,
/// priorizando teselas que **sí** entran en el muestreo (`tx+1`, `ty+1`, …).
#[must_use]
pub fn infer_coast_tileh_when_flat(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> u8 {
    let is_land = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= mw as i32 || y >= mh as i32 {
            return false;
        }
        map.get(TileCoord::new(x, y))
            .is_some_and(|t| t.kind != TileKind::Water && t.kind != TileKind::Void)
    };
    let x = tx as i32;
    let y = ty as i32;
    // En el cuarteto de GetTileSlopeZ: hwest@(tx+1,ty), heast@(tx,ty+1), hsouth@(tx+1,ty+1).
    if is_land(x + 1, y) {
        return 1;
    } // SLOPE_W
    if is_land(x, y + 1) {
        return 4;
    } // SLOPE_E
    if is_land(x + 1, y + 1) {
        return 2;
    } // SLOPE_S
    // Tierra fuera del bloque 2×2 (costa recta típica).
    if is_land(x, y - 1) {
        return 8;
    } // SLOPE_N
    if is_land(x - 1, y) {
        return 8;
    } // mismo síntoma plano; sprite costa “hacia tierra”
    8
}

/// Índice `0..8` para `shore_{i}.png` (sprites OpenGFX 4062–4069).
///
/// OpenTTD dibuja costas con [`DrawShoreTile`] (`water_cmd.cpp`): un único sprite
/// según la pendiente de la tesela, **no** máscara N/E/S/W sobre agua plana.
/// Tabla `tileh_to_shoresprite` + conversión `SPR_SHORE_BASE+d` → PNG original vía
/// `DupSprite` en `newgrf.cpp` (`ActivateOldShore`).
#[must_use]
pub fn shore_png_index(tileh: u8) -> usize {
    const TILEH_TO_SHORE_DST: [u8; 16] = [
        0, 1, 2, 3, 4, 16, 6, 7, 8, 9, 17, 11, 12, 13, 14, 0,
    ];
    let th = tileh.min(15) as usize;
    let dst = TILEH_TO_SHORE_DST[th];
    shore_dst_to_png(dst)
}

/// Convierte offset relativo a `SPR_SHORE_BASE` en índice `shore_{n}.png` (4062+n).
fn shore_dst_to_png(dst: u8) -> usize {
    match dst {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 6,
        4 => 0,
        5 => 5,
        6 => 4,
        7 => 7,
        8 => 3,
        9 => 7,
        10 => 4,
        11 => 6,
        12 => 5,
        13 => 4,
        14 => 6,
        15 => 0,
        16 => 4,
        17 => 3,
        _ => (dst as usize).min(7),
    }
}

/// Nombre corto del bitmask de pendiente OpenTTD (`Slope` / `tileh` 0–14).
/// Bits: W=1, S=2, E=4, N=8 (esquinas elevadas respecto al mínimo local).
#[must_use]
pub fn slope_label(tileh: u8) -> &'static str {
    match tileh.min(14) {
        0 => "FLAT",
        1 => "W",
        2 => "S",
        3 => "SW",
        4 => "E",
        5 => "WE",
        6 => "SE",
        7 => "WSE",
        8 => "N",
        9 => "NW",
        10 => "NS",
        11 => "NWS",
        12 => "NE",
        13 => "NWE",
        14 => "NSE",
        _ => "?",
    }
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

#[cfg(test)]
mod compute_tileh_tests {
    //! Regresión: `compute_tileh` debe coincidir con `GetTileSlopeZ` / `GetTileSlopeGivenHeight`
    //! (`tile_map.cpp` de OpenTTD): hnorth@(tx,ty), hwest@(tx+1,ty), heast@(tx,ty+1), hsouth@(tx+1,ty+1).

    use super::compute_tileh;
    use openttdrs_core::{Map, TileCoord};

    fn set_h(map: &mut Map, x: i32, y: i32, h: u8) {
        map.set_height(TileCoord::new(x, y), h).unwrap();
    }

    #[test]
    fn flat_2x2_all_zero() {
        let m = Map::new_flat(2, 2, 0);
        assert_eq!(compute_tileh(&m, 0, 0), 0);
        assert_eq!(compute_tileh(&m, 1, 0), 0);
        assert_eq!(compute_tileh(&m, 0, 1), 0);
        assert_eq!(compute_tileh(&m, 1, 1), 0);
    }

    #[test]
    fn only_hnorth_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 0, 0, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 8); // SLOPE_N
    }

    #[test]
    fn only_hwest_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 1, 0, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 1); // SLOPE_W
    }

    #[test]
    fn only_heast_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 0, 1, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 4); // SLOPE_E
    }

    #[test]
    fn only_hsouth_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 1, 1, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 2); // SLOPE_S
    }

    #[test]
    fn hwest_and_hsouth_sw_slope() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 1, 0, 1);
        set_h(&mut m, 1, 1, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 3); // SLOPE_SW
    }

    #[test]
    fn map_edge_1x1_void_corners_read_as_zero() {
        let mut m = Map::new_flat(1, 1, 0);
        set_h(&mut m, 0, 0, 2);
        // Fuera del mapa → altura 0; solo hnorth=2 > min(0,0,0,0)
        assert_eq!(compute_tileh(&m, 0, 0), 8);
    }

    #[test]
    fn thin_map_2x1_row() {
        let mut m = Map::new_flat(2, 1, 0);
        set_h(&mut m, 1, 0, 1);
        // (0,0): hnorth=0, hwest=1, heast/hsouth fuera → 0; min=0 → solo W
        assert_eq!(compute_tileh(&m, 0, 0), 1);
        // (1,0): hnorth=1, hwest fuera 0, heast/hsouth 0 → min=0 → N
        assert_eq!(compute_tileh(&m, 1, 0), 8);
    }

    #[test]
    fn inner_tile_flat_when_plateau_uniform() {
        let m = Map::new_flat(3, 3, 5);
        assert_eq!(compute_tileh(&m, 1, 1), 0);
        assert_eq!(compute_tileh(&m, 0, 1), 0);
    }
}

#[cfg(test)]
mod tile_min_corner_height_tests {
    use super::{tile_min_corner_height, tile_min_z};
    use openttdrs_core::{Map, TileCoord};

    fn set_h(map: &mut Map, x: i32, y: i32, h: u8) {
        map.set_height(TileCoord::new(x, y), h).unwrap();
    }

    #[test]
    fn plateau_all_same() {
        let m = Map::new_flat(2, 2, 9);
        assert_eq!(tile_min_corner_height(&m, 0, 0), 9);
    }

    #[test]
    fn min_follows_lowest_corner_sample() {
        let mut m = Map::new_flat(2, 2, 5);
        set_h(&mut m, 1, 1, 2);
        // Esquinas de (0,0): N=5, W=5, E=5, S=2 → min 2
        assert_eq!(tile_min_corner_height(&m, 0, 0), 2);
    }

    #[test]
    fn tile_min_z_out_of_bounds() {
        let m = Map::new_flat(1, 1, 1);
        assert_eq!(tile_min_z(&m, TileCoord::new(-1, 0)), 0);
        assert_eq!(tile_min_z(&m, TileCoord::new(0, 1)), 0);
    }
}

#[cfg(test)]
mod infer_coast_tileh_tests {
    use super::infer_coast_tileh_when_flat;
    use openttdrs_core::{Map, TileCoord, TileKind};

    #[test]
    fn land_in_quartet_east_sample_prefers_w_slope() {
        let mut m = Map::new_flat(2, 2, 3);
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        assert_eq!(infer_coast_tileh_when_flat(&m, 0, 0, 2, 2), 1);
    }

    #[test]
    fn land_north_outside_quartet_prefers_n() {
        // Fila y=0 hierba, y=1 agua: la costa mira hacia N; el 2×2 de (0,1) es todo agua.
        let mut m = Map::new_flat(2, 3, 2);
        for x in 0..2 {
            m.set_kind(TileCoord::new(x, 0), TileKind::Grass).unwrap();
            m.set_kind(TileCoord::new(x, 1), TileKind::Water).unwrap();
            m.set_kind(TileCoord::new(x, 2), TileKind::Water).unwrap();
        }
        assert_eq!(infer_coast_tileh_when_flat(&m, 0, 1, 2, 3), 8);
    }
}
