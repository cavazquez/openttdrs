use openttdrs_core::{Map, TileCoord, TileKind};

use super::{SLOPE_HALF_H, tile_slope_bits_from_heights};

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
