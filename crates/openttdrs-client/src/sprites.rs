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
// El gfx es el valor de m5 para tiles de industria.
// Cada entrada mapea a un sprite del set OpenGFX.

/// Mapeo de gfx de industria a (sprite_id, w, h, xrel, yrel).
/// Sprite 0 significa "solo suelo, sin edificio".
/// Los valores son para industria completada (stage 3).
///
/// Coal Mine: gfx 0-6
/// Power Station: gfx 7-14
/// etc.
pub const INDUSTRY_GFX_SPRITES: [(u32, f32, f32, f32, f32); 8] = [
    // Coal Mine (gfx 0-3 tienen edificios, 4-6 solo suelo)
    (2013, 58.0, 50.0, -16.0, -33.0), // gfx 0: headframe principal
    (2015, 46.0, 53.0, -14.0, -38.0), // gfx 1: torre animada
    (2018, 64.0, 39.0, -31.0, -8.0),  // gfx 2: edificio auxiliar
    (2021, 44.0, 38.0, -13.0, -21.0), // gfx 3: edificio pequeño
    (0, 0.0, 0.0, 0.0, 0.0),          // gfx 4: solo suelo
    (0, 0.0, 0.0, 0.0, 0.0),          // gfx 5: solo suelo
    (0, 0.0, 0.0, 0.0, 0.0),          // gfx 6: solo suelo
    (0, 0.0, 0.0, 0.0, 0.0),          // placeholder
];

/// Devuelve el sprite y metadatos para un tile de industria dado su gfx (m5).
/// Retorna None si es gfx desconocido o solo suelo.
pub fn industry_sprite_for_gfx(gfx: u8) -> Option<(u32, f32, f32, f32, f32)> {
    // Solo soportamos Coal Mine por ahora (gfx 0-6)
    if gfx < 7 {
        let entry = INDUSTRY_GFX_SPRITES[gfx as usize];
        if entry.0 != 0 {
            return Some(entry);
        }
    }
    // Para otros gfx, usar sprite genérico basado en patrón
    // Muchas industrias comparten sprites similares
    match gfx {
        // Power Station (gfx 7-14)
        7..=14 => Some((2013, 58.0, 50.0, -16.0, -33.0)), // usar headframe como placeholder
        // Oil Rig (gfx 24-28)
        24..=28 => Some((2013, 58.0, 50.0, -16.0, -33.0)),
        // Otros: usar el sprite que tengamos más a mano
        _ => {
            // Para gfx > 6, algunos tienen edificios y otros no
            // Usamos headframe como fallback genérico
            if gfx % 4 < 3 {
                Some((2013, 58.0, 50.0, -16.0, -33.0))
            } else {
                None // Solo suelo
            }
        }
    }
}

/// IDs de sprites de vía férrea usados.
pub const RAIL_SPRITE_IDS: [u32; 20] = [
    1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020,
    1021, 1022, 1035, 1036,
];

// ── Lógica de road bits ─────────────────────────────────────────────────────

/// Intercambia bits NW (0) ↔ SE (2) para compensar eje Y invertido.
#[inline]
fn swap_y_road_bits(bits: u8) -> u8 {
    (bits & 0b1010) | ((bits & 0b0001) << 2) | ((bits & 0b0100) >> 2)
}

/// Decodifica los road bits efectivos desde m5 según el tipo de tesela.
pub fn effective_road_bits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    let tt = (mapt >> 4) & 0xF;
    let raw = match tt {
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
    };
    raw.map(swap_y_road_bits)
}

#[inline]
pub fn road_flat_index(road_bits: u8) -> usize {
    usize::from(ROAD_FLAT_OFFSET_TBL[usize::from(road_bits & 0x0F)])
}

/// Road bits para dibujar: `m5` / vecinos (mapa procedural).
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
        bits |= 8;
    }
    if is_road_or_station(TileCoord::new(pos.x, pos.y + 1)) {
        bits |= 1;
    }
    if is_road_or_station(TileCoord::new(pos.x + 1, pos.y)) {
        bits |= 2;
    }
    if is_road_or_station(TileCoord::new(pos.x, pos.y - 1)) {
        bits |= 4;
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
    let has_tx = rail_neighbor(TileCoord::new(pos.x - 1, pos.y))
        || rail_neighbor(TileCoord::new(pos.x + 1, pos.y));
    let has_ty = rail_neighbor(TileCoord::new(pos.x, pos.y - 1))
        || rail_neighbor(TileCoord::new(pos.x, pos.y + 1));
    match (has_tx, has_ty) {
        (true, false) => RAIL_TB_Y,
        (false, true) => RAIL_TB_X,
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
