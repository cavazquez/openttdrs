use openttdrs_core::{Map, TileCoord, TileKind};

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

pub(crate) fn water_void_effective_height_for_slope(
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
