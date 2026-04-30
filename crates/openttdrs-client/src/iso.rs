//! Utilidades de proyección isométrica.
#![allow(clippy::unwrap_used)] // tests de `compute_tileh` usan mapas mínimos fijos

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
#[inline]
fn slope_bits_from_corner_vals(hnorth: u8, hwest: u8, heast: u8, hsouth: u8) -> (u8, u8) {
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
    (tileh.min(14), min_h)
}

/// Pendiente y `min_h` **solo** desde las cuatro alturas de esquina (sin truco de UI
/// para MP_WATER). Es lo que usa OpenTTD en [`GetTileSlopeZ`].
#[must_use]
pub fn tile_slope_bits_from_heights(map: &Map, tx: u32, ty: u32) -> (u8, u8) {
    let (mw, mh) = map.dimensions();
    let get_h = |dtx: i32, dty: i32| height_for_slope_corner_sample(map, dtx, dty, mw, mh);
    let hnorth = get_h(tx as i32, ty as i32);
    let hwest = get_h(tx as i32 + 1, ty as i32);
    let heast = get_h(tx as i32, ty as i32 + 1);
    let hsouth = get_h(tx as i32 + 1, ty as i32 + 1);
    slope_bits_from_corner_vals(hnorth, hwest, heast, hsouth)
}

/// `tileh` para [`DrawShoreTile`] (`water_cmd.cpp`): la costa usa la pendiente **real**
/// del MAPH cuando no es plana (SW, NE, …); si el 2×2 es uniforme se usa
/// [`infer_coast_tileh_when_flat`] mirando vecinos de tierra.
///
/// No reutilizar `tile_slope_and_min_z` sobre MP_WATER: ahí forzamos `tileh=0` para la
/// UI; aquí hace falta el bitmask crudo o la silueta costera sería solo N/E/S/W.
#[must_use]
pub fn shore_tileh_for_draw_shore(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> u8 {
    let (raw, _) = tile_slope_bits_from_heights(map, tx, ty);
    if raw == 0 {
        return infer_coast_tileh_when_flat(map, tx, ty, mw, mh);
    }
    // Con sprites legacy (4062..4069) solo existen estas pendientes de costa.
    // El resto (incl. WE/NS y 3 esquinas elevadas) en OpenTTD se resuelve con
    // reemplazos adicionales; aquí caemos a inferencia para evitar artefactos.
    if !matches!(raw, 1 | 2 | 3 | 4 | 6 | 8 | 9 | 12) {
        return infer_coast_tileh_when_flat(map, tx, ty, mw, mh);
    }
    raw
}

/// El resultado está limitado a 0–14 (pendientes simples; las empinadas (15)
/// requieren sprites especiales y se omiten por ahora).
#[must_use]
pub fn tile_slope_and_min_z(map: &Map, tx: u32, ty: u32) -> (u8, u8) {
    let (mw, mh) = map.dimensions();
    let get_h = |dtx: i32, dty: i32| height_for_slope_corner_sample(map, dtx, dty, mw, mh);
    let hnorth = get_h(tx as i32, ty as i32);
    let hwest = get_h(tx as i32 + 1, ty as i32);
    let heast = get_h(tx as i32, ty as i32 + 1);
    let hsouth = get_h(tx as i32 + 1, ty as i32 + 1);
    let (tileh_computed, min_h) = slope_bits_from_corner_vals(hnorth, hwest, heast, hsouth);
    let center = map.get(TileCoord::new(tx as i32, ty as i32));
    let is_water = center.is_some_and(|t| t.kind == TileKind::Water);
    // MP_WATER se dibuja como superficie plana (Clear / costa); `tileh`≠0 aquí solo
    // confunde UI y el grid de costa — las pendientes vienen de `DrawShoreTile` en el
    // agua o del terreno en MP_CLEAR, no de “pendiente de rombo” sobre el mar.
    //
    // `min_z` debe ser siempre `min_h` (= [`GetTileZ`]) también en agua: si usamos otra
    // métrica (p. ej. mediana), la costa y la hierba lindera quedan desfasadas en Y y
    // aparece la “sierra” / escalones entre rombos.
    let tileh_out = if is_water { 0 } else { tileh_computed };
    (tileh_out, min_h)
}

/// Altura usada en una esquina del 2×2 de [`tile_slope_and_min_z`], análoga a
/// [`GetTileSlopeZ`] / `TileHeight` en OpenTTD.
///
/// Los exports `.ottdmap` a veces guardan **`height = 0`** en `MP_WATER` aunque el mar
/// comparta nivel con la costa; si usamos ese valor literal, `min_h` cae y el suelo
/// “cuelga” sobre el agua. Para **`Water`** y **`Void`** inferimos un nivel con el
/// **mínimo** de `Tile.height` en teselas de **tierra** (no agua/void) en el
/// vecindario de 8 celdas (incluye diagonales). Así en una **bahía** o esquina
/// entrante las celdas de agua comparten el mismo “nivel de mar”; usar `max` daba
/// alturas distintas por celda y pendientes/costas asimétricas con la hierba lindera.
#[inline]
fn height_for_slope_corner_sample(map: &Map, cx: i32, cy: i32, mw: u32, mh: u32) -> u8 {
    if cx < 0 || cy < 0 {
        return 0;
    }
    let Ok(ux) = u32::try_from(cx) else {
        return 0;
    };
    let Ok(uy) = u32::try_from(cy) else {
        return 0;
    };
    if ux >= mw || uy >= mh {
        return 0;
    }
    let Some(t) = map.get(TileCoord::new(cx, cy)) else {
        return 0;
    };
    if matches!(t.kind, TileKind::Water | TileKind::Void) {
        water_void_effective_height_for_slope(map, ux, uy, mw, mh, t.height)
    } else {
        t.height
    }
}

fn water_void_effective_height_for_slope(
    map: &Map,
    ux: u32,
    uy: u32,
    mw: u32,
    mh: u32,
    stored: u8,
) -> u8 {
    // En exports normales, la altura de MP_WATER/MP_VOID suele ser válida.
    // Solo inferimos cuando viene 0 (caso típico de `.ottdmap` que hunde costa).
    if stored != 0 {
        return stored;
    }

    let x = ux as i32;
    let y = uy as i32;
    const NEIGH8: [(i32, i32); 8] = [
        (0, -1),
        (0, 1),
        (-1, 0),
        (1, 0),
        (-1, -1),
        (-1, 1),
        (1, -1),
        (1, 1),
    ];
    let mut best: Option<u8> = None;
    for (dx, dy) in NEIGH8 {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 {
            continue;
        }
        let Ok(nux) = u32::try_from(nx) else {
            continue;
        };
        let Ok(nuy) = u32::try_from(ny) else {
            continue;
        };
        if nux >= mw || nuy >= mh {
            continue;
        }
        let Some(nt) = map.get(TileCoord::new(nx, ny)) else {
            continue;
        };
        if matches!(nt.kind, TileKind::Water | TileKind::Void) {
            continue;
        }
        best = Some(best.map_or(nt.height, |b| b.min(nt.height)));
    }
    best.unwrap_or(stored)
}

#[must_use]
pub fn compute_tileh(map: &Map, tx: u32, ty: u32) -> u8 {
    tile_slope_and_min_z(map, tx, ty).0
}

/// Altura base de la tesela para dibujar el suelo: **mínimo** de las cuatro esquinas
/// (misma muestra que `compute_tileh`), equivalente a `GetTileZ` en OpenTTD (`tile_map.cpp`).
///
/// OpenTTD ancla los sprites de terreno a la esquina **más baja** del rombo; usar solo
/// `Tile.height` (esquina N de esa celda) desplaza verticalmente las pendientes y abre
/// huecos entre teselas vecinas.
#[must_use]
pub fn tile_min_corner_height(map: &Map, tx: u32, ty: u32) -> u8 {
    tile_slope_and_min_z(map, tx, ty).1
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
    // Vecinos del bloque 2x2 de GetTileSlopeZ.
    let land_west_corner = is_land(x + 1, y); // hwest
    let land_east_corner = is_land(x, y + 1); // heast
    let land_south_corner = is_land(x + 1, y + 1); // hsouth
    // Fuera del bloque (referencias para costa recta / orientación).
    let land_north_side = is_land(x, y - 1);
    let land_west_side = is_land(x - 1, y);
    // Contactos puramente diagonales alrededor del rombo. En pantalla son,
    // respectivamente, arriba, izquierda y derecha del tile de agua.
    let land_north_diag = is_land(x - 1, y - 1);
    let land_west_diag = is_land(x + 1, y - 1);
    let land_east_diag = is_land(x - 1, y + 1);

    // Patrón típico de costa larga en diagonal (agua al SE, tierra al NO):
    // priorizar familia NW (`t9/s7`) para evitar dientes alternando con W/E.
    if land_north_side && (land_west_corner || land_east_corner || land_south_corner) {
        return 9;
    }

    // Preferir pendientes diagonales cuando dos esquinas lindan con tierra.
    if land_west_corner && land_south_corner {
        return 3;
    } // SW
    if land_east_corner && land_south_corner {
        return 6;
    } // SE
    if land_west_corner && land_north_side {
        return 9;
    } // NW
    if land_east_corner && land_north_side {
        return 12;
    } // NE

    if land_west_corner {
        // En algunas diagonales largas de costa plana, el export deja un patrón donde
        // solo `hwest` aparece como tierra en una celda sí y otra no. Si devolvemos W
        // puro (1) en esos huecos, se generan "picos" serrados. Miramos un vecindario
        // corto hacia el sur para mantener continuidad diagonal.
        let south_diag_hint = is_land(x - 1, y + 1) || is_land(x, y + 2) || is_land(x + 1, y + 2);
        if south_diag_hint {
            return 3;
        } // SW
        return 1;
    } // W
    if land_south_corner {
        return 2;
    } // S
    if land_east_corner {
        let south_diag_hint = is_land(x + 1, y - 1) || is_land(x + 2, y) || is_land(x + 2, y + 1);
        if south_diag_hint {
            return 6;
        } // SE
        return 4;
    } // E
    if land_north_side || land_west_side {
        return 8;
    } // N
    if land_west_diag {
        return 1;
    } // W
    if land_east_diag {
        return 4;
    } // E
    if land_north_diag {
        return 8;
    } // N
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
    // `ActivateOldShore` (OpenTTD/newgrf.cpp): mapea pendiente -> sprite original 4062..4069.
    match tileh.min(14) {
        1 => 1,  // W
        2 => 2,  // S
        3 => 6,  // SW
        4 => 0,  // E
        6 => 4,  // SE
        8 => 3,  // N
        9 => 7,  // NW
        12 => 5, // NE
        _ => 0,
    }
}

/// `half_h` visual para el sprite elegido por [`DrawShoreTile`].
///
/// Los `shore_*.png` heredados no miden todos 64x31: S/SW/SE son 64x23 y
/// N/NW/NE son 64x39 con `yrel=-8`. En ambos casos el ancla NFO equivale al
/// mismo centro que las pendientes de terreno (`SLOPE_HALF_H[tileh]`).
#[must_use]
pub fn shore_sprite_half_h(tileh: u8) -> f32 {
    SLOPE_HALF_H[tileh.min(14) as usize]
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
mod water_coast_height_tests {
    //! Agua con `height` 0 en el export no debe hundir las esquinas de la costa.

    use super::{
        TILE_HALF_H, shore_sprite_half_h, shore_tileh_for_draw_shore, tile_slope_and_min_z,
        water_void_effective_height_for_slope,
    };
    use openttdrs_core::{Map, TileCoord, TileKind};

    #[test]
    fn peninsula_grass_flat_when_water_corners_stored_zero() {
        let mut m = Map::new_flat(2, 2, 0);
        m.set_kind(TileCoord::new(0, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 0), 5).unwrap();
        for (x, y) in [(1, 0), (0, 1), (1, 1)] {
            m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            m.set_height(TileCoord::new(x, y), 0).unwrap();
        }
        let (tileh, min_z) = tile_slope_and_min_z(&m, 0, 0);
        assert_eq!(min_z, 5, "min_h no debe ser 0 por las celdas de agua");
        assert_eq!(tileh, 0);
    }

    #[test]
    fn water_pool_inherits_ring_grass_height() {
        // Anillo de hierba h=5, charco 2×2 de agua con height 0 en el centro de un 4×4.
        let mut m = Map::new_flat(4, 4, 0);
        for y in 0..4 {
            for x in 0..4 {
                let ring = x == 0 || y == 0 || x == 3 || y == 3;
                if ring {
                    m.set_kind(TileCoord::new(x, y), TileKind::Grass).unwrap();
                    m.set_height(TileCoord::new(x, y), 5).unwrap();
                } else {
                    m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
                    m.set_height(TileCoord::new(x, y), 0).unwrap();
                }
            }
        }
        let (tileh, min_z) = tile_slope_and_min_z(&m, 1, 1);
        assert_eq!(min_z, 5);
        assert_eq!(tileh, 0);
    }

    #[test]
    fn mp_water_never_exposes_terrain_slope_bits() {
        use super::compute_tileh;
        let mut m = Map::new_flat(4, 4, 0);
        for y in 0..4 {
            for x in 0..4 {
                let ring = x == 0 || y == 0 || x == 3 || y == 3;
                if ring {
                    m.set_kind(TileCoord::new(x, y), TileKind::Grass).unwrap();
                    m.set_height(TileCoord::new(x, y), 5).unwrap();
                } else {
                    m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
                    m.set_height(TileCoord::new(x, y), 0).unwrap();
                }
            }
        }
        assert_eq!(compute_tileh(&m, 1, 1), 0);
    }

    #[test]
    fn shore_tileh_uses_diagonal_slope_not_infer_priority_w() {
        // 2×2: agua (0,0); hierba con alturas que dan SLOPE_SW (3) en el cuarteto.
        // `infer_coast` miraría primero tierra en (1,0) y devolvería solo W (1).
        let mut m = Map::new_flat(2, 2, 1);
        m.set_kind(TileCoord::new(0, 0), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(0, 0), 1).unwrap();
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 0), 3).unwrap();
        m.set_kind(TileCoord::new(0, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 1), 1).unwrap();
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 1), 3).unwrap();
        assert_eq!(shore_tileh_for_draw_shore(&m, 0, 0, 2, 2), 3);
    }

    #[test]
    fn water_height_nonzero_is_preserved_for_slope_sampling() {
        // Si MP_WATER ya trae altura válida, no debemos sustituirla por min(vecinos).
        let mut m = Map::new_flat(3, 3, 0);
        m.set_kind(TileCoord::new(1, 1), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(1, 1), 7).unwrap();
        // Vecinos de tierra más bajos (si hubiese inferencia, bajaría).
        for (x, y, h) in [(0, 1, 3), (2, 1, 4), (1, 0, 2), (1, 2, 5)] {
            m.set_kind(TileCoord::new(x, y), TileKind::Grass).unwrap();
            m.set_height(TileCoord::new(x, y), h).unwrap();
        }
        let got = water_void_effective_height_for_slope(&m, 1, 1, 3, 3, 7);
        assert_eq!(got, 7);
    }

    #[test]
    fn unsupported_raw_shore_slopes_fallback_to_infer() {
        // raw=7 (WSE) no tiene sprite legacy directo; debe caer a inferencia.
        let mut m = Map::new_flat(2, 2, 0);
        m.set_kind(TileCoord::new(0, 0), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(0, 0), 0).unwrap(); // hnorth
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 0), 1).unwrap(); // hwest
        m.set_kind(TileCoord::new(0, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 1), 1).unwrap(); // heast
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 1), 1).unwrap(); // hsouth => raw 7
        assert_eq!(shore_tileh_for_draw_shore(&m, 0, 0, 2, 2), 3);
    }

    #[test]
    fn shore_half_h_matches_effective_slope_anchor() {
        assert_eq!(shore_sprite_half_h(1), TILE_HALF_H);
        assert_eq!(shore_sprite_half_h(4), TILE_HALF_H);

        for tileh in [2, 3, 6, 8, 9, 12] {
            assert_eq!(shore_sprite_half_h(tileh), 11.5);
        }
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
        for y in 0..2 {
            for x in 0..2 {
                m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            }
        }
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

    #[test]
    fn land_west_and_south_corners_prefers_sw_diagonal() {
        let mut m = Map::new_flat(2, 2, 3);
        for y in 0..2 {
            for x in 0..2 {
                m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            }
        }
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap(); // hwest
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap(); // hsouth
        assert_eq!(infer_coast_tileh_when_flat(&m, 0, 0, 2, 2), 3);
    }

    #[test]
    fn diagonal_land_outside_quartet_keeps_screen_side_orientation() {
        let mut m = Map::new_flat(3, 3, 3);
        for y in 0..3 {
            for x in 0..3 {
                m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            }
        }

        m.set_kind(TileCoord::new(2, 0), TileKind::Grass).unwrap(); // screen-left
        assert_eq!(infer_coast_tileh_when_flat(&m, 1, 1, 3, 3), 1);

        m.set_kind(TileCoord::new(2, 0), TileKind::Water).unwrap();
        m.set_kind(TileCoord::new(0, 2), TileKind::Grass).unwrap(); // screen-right
        assert_eq!(infer_coast_tileh_when_flat(&m, 1, 1, 3, 3), 4);
    }
}

#[cfg(test)]
mod world_pos_to_tile_tests {
    use bevy::prelude::Vec2;

    use super::{
        HEIGHT_PX, Map, TILE_HALF_H, TileCoord, TileKind, iso, world_pos_to_tile_coord,
        world_to_tile,
    };

    /// Mapa al mismo nivel: el centro del sprite (como en `tile_pos`) debe mapear a su tesela;
    /// [`world_to_tile`] daño con el desfase de elevación.
    #[test]
    fn corrects_height_offset_for_flat_tileh() {
        let m = Map::new_flat(256, 256, 5);
        let tx: i32 = 137;
        let ty: i32 = 118;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let p = iso(tx, ty);
        let center = Vec2::new(p.x, p.y - TILE_HALF_H + elev);
        assert_eq!(world_pos_to_tile_coord(center, &m), Some((tx, ty)));
        assert_ne!(world_to_tile(center), (tx, ty));
    }

    #[test]
    fn keeps_same_tile_on_left_and_right_half_of_diamond() {
        let m = Map::new_flat(256, 256, 5);
        let tx: i32 = 149;
        let ty: i32 = 122;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let top = iso(tx, ty) + Vec2::new(0.0, elev);
        // Dos puntos bien dentro del mismo rombo (mitad izquierda y derecha).
        let left_inside = top + Vec2::new(-8.0, -8.0);
        let right_inside = top + Vec2::new(8.0, -8.0);

        assert_eq!(world_pos_to_tile_coord(left_inside, &m), Some((tx, ty)));
        assert_eq!(world_pos_to_tile_coord(right_inside, &m), Some((tx, ty)));
    }

    #[test]
    fn keeps_same_tile_near_top_inside_of_diamond() {
        let m = Map::new_flat(256, 256, 5);
        let tx: i32 = 149;
        let ty: i32 = 122;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - TILE_HALF_H + elev);
        let near_top_inside = center + Vec2::new(0.0, TILE_HALF_H - 1.0);
        assert_eq!(world_pos_to_tile_coord(near_top_inside, &m), Some((tx, ty)));
    }

    #[test]
    fn road_flat_keeps_bottom_visible_area_in_same_tile() {
        let mut m = Map::new_flat(256, 256, 5);
        let tx: i32 = 149;
        let ty: i32 = 122;
        m.set_kind(TileCoord::new(tx, ty), TileKind::Road).unwrap();
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        // Carretera plana puede tener half_h visual mayor (~19.5).
        let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - 19.5 + elev);
        let near_bottom_inside = center + Vec2::new(0.0, 19.5 - 1.0);
        assert_eq!(
            world_pos_to_tile_coord(near_bottom_inside, &m),
            Some((tx, ty))
        );
    }
}
