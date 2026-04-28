//! Constantes y lógica de sprites de `OpenGFX`.

use openttdrs_core::{Map, TileCoord, TileKind};

// ── Constantes de renderizado de carreteras y vías ───────────────────────────

/// Tipos de tesela `OpenTTD` (nibble alto del byte MAPT).
pub const OTTD_MP_RAIL: u8 = 1;
pub const OTTD_MP_ROAD: u8 = 2;
pub const OTTD_MP_TUNNELBRIDGE: u8 = 9;

/// Subtipo de tesela ferroviaria en bits 6–7 de `m5` (`rail_map.h`).
pub const RAIL_TILE_NORMAL: u8 = 0;
pub const RAIL_TILE_SIGNALS: u8 = 1;
pub const RAIL_TILE_DEPOT: u8 = 3;

/// Desplazamiento dentro del grupo `SPR_ROAD` para tesela plana.
pub const ROAD_FLAT_OFFSET_TBL: [u8; 16] = [0, 18, 17, 7, 16, 0, 10, 5, 15, 8, 1, 4, 9, 3, 6, 2];

/// Mitad de la altura en px de cada variante `road_flat_XX`.
pub const ROAD_FLAT_HALF_H: [f32; 19] = [
    15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 19.5, 11.5, 11.5, 19.5, 15.5,
    15.5, 15.5, 15.5,
];

/// `TrackBits` en vía clásica (`track_type.h`).
pub const RAIL_TB_X: u8 = 1;
pub const RAIL_TB_Y: u8 = 2;
pub const RAIL_TB_UPPER: u8 = 4;
pub const RAIL_TB_LOWER: u8 = 8;
pub const RAIL_TB_LEFT: u8 = 16;
pub const RAIL_TB_RIGHT: u8 = 32;
pub const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;
pub const RAIL_TB_HORZ: u8 = RAIL_TB_UPPER | RAIL_TB_LOWER;
pub const RAIL_TB_VERT: u8 = RAIL_TB_LEFT | RAIL_TB_RIGHT;

/// Máscaras 3 vías por esquina.
const RAIL_3WAY_NE: u8 = RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_RIGHT;
const RAIL_3WAY_SW: u8 = RAIL_TB_X | RAIL_TB_LOWER | RAIL_TB_LEFT;
const RAIL_3WAY_NW: u8 = RAIL_TB_Y | RAIL_TB_UPPER | RAIL_TB_LEFT;
const RAIL_3WAY_SE: u8 = RAIL_TB_Y | RAIL_TB_LOWER | RAIL_TB_RIGHT;

/// Metadatos de casas: (w, h, xrel, yrel) extraídos del NFO.
pub const HOUSE_META: [(f32, f32, f32, f32); 8] = [
    (64.0, 37.0, -31.0, -6.0),
    (65.0, 71.0, -31.0, -40.0),
    (64.0, 36.0, -31.0, -5.0),
    (66.0, 80.0, -32.0, -49.0),
    (66.0, 87.0, -32.0, -56.0),
    (64.0, 36.0, -31.0, -5.0),
    (64.0, 35.0, -31.0, -4.0),
    (64.0, 34.0, -31.0, -3.0),
];

// ── Industrias: mapeo gfx → sprite ──────────────────────────────────────────
// Basado en _industry_draw_tile_data de OpenTTD (table/industry_land.h).
// El gfx es el valor del byte m5 para tiles de industria (construction_stage=3).
// Cada entrada representa un tile de industria completada.
//
// Fórmula de sprite_id: s2 del M() macro en industry_land.h para stage 3.
// Dimensiones (w, h, xrel, yrel): extraídas del NFO de OpenGFX.
// Para tiles sin edificio (solo suelo), sprite_id = 0.

/// Metadatos de un sprite de tile de industria.
pub struct IndustryGfxSprite {
    /// Sprite ID en OpenGFX (0 = solo suelo, sin overlay de edificio).
    pub sprite_id: u32,
    pub w: f32,
    pub h: f32,
    /// Offset horizontal desde el vértice superior del rombo (pantalla).
    pub xrel: f32,
    /// Offset vertical hacia arriba desde el vértice (positivo = más arriba en NFO = negativo yrel).
    pub yrel: f32,
}

/// Default genérico para edificios cuyas dimensiones exactas no se han calibrado aún.
/// Centra un sprite 64×48 sobre el tile.
const fn gfx_building(sprite_id: u32) -> IndustryGfxSprite {
    IndustryGfxSprite {
        sprite_id,
        w: 64.0,
        h: 48.0,
        xrel: -32.0,
        yrel: -32.0,
    }
}

const fn gfx_ground() -> IndustryGfxSprite {
    IndustryGfxSprite {
        sprite_id: 0,
        w: 0.0,
        h: 0.0,
        xrel: 0.0,
        yrel: 0.0,
    }
}

/// Tabla gfx → sprite para clima templado.
/// Índice = gfx (valor de m5 para tile de industria completada, stage 3).
/// Derivado de `_industry_draw_tile_data` en `table/industry_land.h` de OpenTTD.
///
/// Rangos por industria:
/// |  gfx  | Industria        |
/// |-------|------------------|
/// |  0- 6 | Coal Mine        |
/// |  7-10 | Power Station    |
/// | 11-15 | Sawmill          |
/// | 16-23 | Oil Refinery     |
/// | 24-28 | Forest           |
/// | 29-32 | Printing Works   |
/// | 33-38 | Oil Rig          |
/// | 39-42 | Steel Mill       |
/// | 43-46 | Factory          |
/// | 47-51 | Oil Wells        |
/// | 52-57 | Farm             |
/// | 58-59 | Bank (Templado)  |
pub const INDUSTRY_GFX_DATA: [IndustryGfxSprite; 60] = [
    // ── Coal Mine (gfx 0-6) ──────────────────────────────────────────────────
    // Valores exactos del NFO de OpenGFX.
    IndustryGfxSprite {
        sprite_id: 2013,
        w: 58.0,
        h: 50.0,
        xrel: -16.0,
        yrel: -33.0,
    }, // 0 headframe
    IndustryGfxSprite {
        sprite_id: 2015,
        w: 46.0,
        h: 53.0,
        xrel: -14.0,
        yrel: -38.0,
    }, // 1 torre
    IndustryGfxSprite {
        sprite_id: 2018,
        w: 64.0,
        h: 39.0,
        xrel: -31.0,
        yrel: -8.0,
    }, // 2 aux
    IndustryGfxSprite {
        sprite_id: 2021,
        w: 44.0,
        h: 38.0,
        xrel: -13.0,
        yrel: -21.0,
    }, // 3 pequeño
    gfx_ground(), // 4 suelo
    gfx_ground(), // 5 suelo
    gfx_ground(), // 6 suelo
    // ── Power Station (gfx 7-10) ─────────────────────────────────────────────
    gfx_building(2047), // 7  chimenea (sz=44 → edificio alto)
    gfx_building(2050), // 8  generador
    gfx_building(2053), // 9  transformador
    gfx_building(2054), // 10 edificio principal (proc especial)
    // ── Sawmill (gfx 11-15) ──────────────────────────────────────────────────
    gfx_building(2063), // 11
    gfx_building(2066), // 12
    gfx_building(2069), // 13
    gfx_building(2070), // 14
    gfx_building(2071), // 15
    // ── Oil Refinery (gfx 16-23) ─────────────────────────────────────────────
    gfx_building(2075), // 16
    gfx_building(2076), // 17
    gfx_building(2080), // 18
    gfx_building(2083), // 19
    gfx_building(2086), // 20
    gfx_building(2089), // 21
    gfx_building(2092), // 22
    gfx_building(2095), // 23
    // ── Forest (gfx 24-28) ───────────────────────────────────────────────────
    gfx_ground(),       // 24 suelo animado (sin overlay estático)
    gfx_building(2099), // 25 árbol cluster 1
    gfx_building(2100), // 26 árbol cluster 2
    gfx_building(2101), // 27 árbol cluster 3
    gfx_building(2102), // 28 árbol cluster 4
    // ── Printing Works (gfx 29-32) ───────────────────────────────────────────
    gfx_building(2174), // 29
    gfx_building(2178), // 30
    gfx_building(2177), // 31
    gfx_building(2174), // 32
    // ── Oil Rig (gfx 33-38) ──────────────────────────────────────────────────
    gfx_building(2108), // 33
    gfx_building(2109), // 34
    gfx_building(2111), // 35
    gfx_building(2113), // 36
    gfx_building(2115), // 37
    gfx_building(2117), // 38
    // ── Steel Mill (gfx 39-42) ───────────────────────────────────────────────
    gfx_building(2150), // 39
    gfx_building(2151), // 40
    gfx_building(2152), // 41
    gfx_ground(),       // 42 suelo
    // ── Factory (gfx 43-46) ──────────────────────────────────────────────────
    gfx_building(2169), // 43
    gfx_building(2170), // 44
    gfx_building(2171), // 45
    gfx_building(2172), // 46
    // ── Oil Wells (gfx 47-51) ────────────────────────────────────────────────
    gfx_building(2028), // 47
    gfx_building(2030), // 48
    gfx_building(2033), // 49
    gfx_building(2036), // 50
    gfx_building(2039), // 51
    // ── Farm (gfx 52-57) ─────────────────────────────────────────────────────
    gfx_building(2119), // 52
    gfx_building(2121), // 53
    gfx_building(2123), // 54
    gfx_ground(),       // 55 campo (sin edificio)
    gfx_building(2126), // 56
    gfx_building(2128), // 57
    // ── Bank Templado (gfx 58-59) ────────────────────────────────────────────
    gfx_building(2180), // 58
    gfx_building(2181), // 59
];

/// Devuelve los metadatos del sprite de industria para el gfx dado (byte m5).
/// Retorna `None` si el gfx no tiene overlay de edificio (solo suelo) o está fuera del rango.
pub fn industry_sprite_for_gfx(gfx: u8) -> Option<&'static IndustryGfxSprite> {
    let entry = INDUSTRY_GFX_DATA.get(usize::from(gfx))?;
    if entry.sprite_id != 0 {
        Some(entry)
    } else {
        None
    }
}

/// IDs de sprites de vía férrea usados.
pub const RAIL_SPRITE_IDS: [u32; 20] = [
    1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020,
    1021, 1022, 1035, 1036,
];

// ── Lógica de road bits ─────────────────────────────────────────────────────

/// Decodifica los road bits efectivos desde m5 según el tipo de tesela.
///
/// Los bits del savegame ya están en la orientación correcta de OpenTTD:
/// - NW (bit 0) = conexión hacia (x, y-1)  → visualmente arriba-izquierda
/// - SW (bit 1) = conexión hacia (x+1, y)  → visualmente abajo-izquierda
/// - SE (bit 2) = conexión hacia (x, y+1)  → visualmente abajo-derecha
/// - NE (bit 3) = conexión hacia (x-1, y)  → visualmente arriba-derecha
pub fn effective_road_bits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    let tt = (mapt >> 4) & 0xF;
    match tt {
        OTTD_MP_ROAD => {
            let subtype = (m5 >> 6) & 0x3;
            match subtype {
                0 => {
                    let rb = m5 & 0x0F;
                    if rb == 0 { None } else { Some(rb) }
                }
                1 => {
                    let axis = m5 & 1;
                    Some(if axis == 0 { 0x0A } else { 0x05 })
                }
                2 => {
                    let d = m5 & 0x3;
                    Some((1u8 << (3 ^ d)) & 0x0F)
                }
                _ => None,
            }
        }
        OTTD_MP_TUNNELBRIDGE if kind == TileKind::Road => {
            let d = m5 & 0x3;
            Some((1u8 << (3 ^ d)) & 0x0F)
        }
        _ => None,
    }
}

#[inline]
pub fn road_flat_index(road_bits: u8) -> usize {
    usize::from(ROAD_FLAT_OFFSET_TBL[usize::from(road_bits & 0x0F)])
}

/// Road bits para dibujar: `m5` / vecinos (mapa procedural).
///
/// Asignación de bits conforme a OpenTTD (con iso correcta):
/// - NE (bit 3 = 8): vecino en (x-1, y) → arriba-derecha en pantalla
/// - NW (bit 0 = 1): vecino en (x, y-1) → arriba-izquierda en pantalla
/// - SW (bit 1 = 2): vecino en (x+1, y) → abajo-izquierda en pantalla
/// - SE (bit 2 = 4): vecino en (x, y+1) → abajo-derecha en pantalla
pub fn road_bits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    if let Some(t) = map.get(pos)
        && let Some(rb) = effective_road_bits(t.mapt, t.m5, t.kind)
        && rb != 0
    {
        return rb & 0x0F;
    }
    let is_road_or_station = |c: TileCoord| -> bool {
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
            return false;
        }
        matches!(
            map.get_kind(c),
            Some(TileKind::Road | TileKind::Station | TileKind::Industry | TileKind::House)
        )
    };
    let mut bits = 0u8;
    if is_road_or_station(TileCoord::new(pos.x - 1, pos.y)) {
        bits |= 8; // NE
    }
    if is_road_or_station(TileCoord::new(pos.x, pos.y - 1)) {
        bits |= 1; // NW: y-1 → arriba-izquierda
    }
    if is_road_or_station(TileCoord::new(pos.x + 1, pos.y)) {
        bits |= 2; // SW
    }
    if is_road_or_station(TileCoord::new(pos.x, pos.y + 1)) {
        bits |= 4; // SE: y+1 → abajo-derecha
    }
    if bits == 0 {
        bits = 0x05;
    }
    bits
}

// ── Lógica de rail bits ─────────────────────────────────────────────────────

pub fn effective_rail_trackbits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    if kind != TileKind::Rail {
        return None;
    }
    let tt = (mapt >> 4) & 0xF;
    if tt != OTTD_MP_RAIL {
        return None;
    }
    let subtype = (m5 >> 6) & 0x3;
    match subtype {
        RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS => Some(m5 & 0x3F),
        RAIL_TILE_DEPOT => {
            let d = m5 & 0x3;
            Some(if d == 1 || d == 3 {
                RAIL_TB_X
            } else {
                RAIL_TB_Y
            })
        }
        _ => None,
    }
}

fn synthetic_rail_trackbits(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    let rail_neighbor = |c: TileCoord| -> bool {
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
            return false;
        }
        matches!(map.get_kind(c), Some(TileKind::Rail | TileKind::Station))
    };
    // Vecinos en eje x (dx=±1) forman la diagonal NE-SW → RAIL_TB_X
    let has_tx = rail_neighbor(TileCoord::new(pos.x - 1, pos.y))
        || rail_neighbor(TileCoord::new(pos.x + 1, pos.y));
    // Vecinos en eje y (dy=±1) forman la diagonal NW-SE → RAIL_TB_Y
    let has_ty = rail_neighbor(TileCoord::new(pos.x, pos.y - 1))
        || rail_neighbor(TileCoord::new(pos.x, pos.y + 1));
    match (has_tx, has_ty) {
        (true, false) => RAIL_TB_X,
        (false, true) => RAIL_TB_Y,
        (true, true) => RAIL_TB_CROSS,
        (false, false) => RAIL_TB_Y,
    }
}

pub fn rail_trackbits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    if let Some(t) = map.get(pos)
        && let Some(tb) = effective_rail_trackbits(t.mapt, t.m5, t.kind)
        && tb != 0
    {
        return tb & 0x3F;
    }
    synthetic_rail_trackbits(map, pos, mw, mh)
}

#[inline]
fn junction_ground_off(tb: u8) -> u8 {
    let t = tb & 0x3F;
    if t & RAIL_3WAY_NE == 0 {
        return 0;
    }
    if t & RAIL_3WAY_SW == 0 {
        return 1;
    }
    if t & RAIL_3WAY_NW == 0 {
        return 2;
    }
    if t & RAIL_3WAY_SE == 0 {
        return 3;
    }
    4
}

/// Lista de sprites `OpenGFX` en orden de pintado (suelo de cruce y superposiciones).
pub fn collect_rail_sprites(tb: u8, out: &mut Vec<u32>) {
    out.clear();
    let t = tb & 0x3F;
    match t {
        RAIL_TB_Y => out.push(1011),
        RAIL_TB_X => out.push(1012),
        RAIL_TB_UPPER => out.push(1013),
        RAIL_TB_LOWER => out.push(1014),
        RAIL_TB_RIGHT => out.push(1015),
        RAIL_TB_LEFT => out.push(1016),
        RAIL_TB_CROSS => out.push(1017),
        RAIL_TB_HORZ => out.push(1035),
        RAIL_TB_VERT => out.push(1036),
        _ => {
            out.push(1018_u32 + u32::from(junction_ground_off(t)));
            if t & RAIL_TB_X != 0 {
                out.push(1005);
            }
            if t & RAIL_TB_Y != 0 {
                out.push(1006);
            }
            if t & RAIL_TB_UPPER != 0 {
                out.push(1007);
            }
            if t & RAIL_TB_LOWER != 0 {
                out.push(1008);
            }
            if t & RAIL_TB_RIGHT != 0 {
                out.push(1009);
            }
            if t & RAIL_TB_LEFT != 0 {
                out.push(1010);
            }
        }
    }
}
