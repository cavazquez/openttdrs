use std::sync::OnceLock;

use openttdrs_core::{Map, TileCoord, TileKind};

use crate::config;

/// Subtipo de tesela ferroviaria en bits 6–7 de `m5` (`rail_map.h`).
pub const RAIL_TILE_NORMAL: u8 = 0;
pub const RAIL_TILE_SIGNALS: u8 = 1;
pub const RAIL_TILE_DEPOT: u8 = 3;

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

/// `SPR_RAIL_TRACK_Y` (`rail_cmd.cpp`).
pub const RAIL_SPRITE_TRACK_Y: u32 = 1011;

/// Delta de sprite en teselas con nieve/deserto (`SPR_RAIL_SNOW_OFFSET`).
pub const RAIL_SPRITE_SNOW_OFFSET: u32 = 26;

/// Offsets desde `SPR_RAIL_TRACK_Y` por `tileh` 1..14 (`_track_sloped_sprites`).
pub const RAIL_TRACK_SLOPED_OFFSETS: [u8; 14] =
    [14, 15, 22, 13, 0, 21, 17, 12, 23, 0, 18, 20, 19, 16];

/// Sprite combinado suelo+riel en tesela inclinada (vía clásica sin overlay NewGRF).
#[must_use]
pub fn rail_sloped_track_sprite_id(tileh: u8, snow_ground: bool) -> Option<u32> {
    let th = tileh.min(14);
    if th == 0 {
        return None;
    }
    let offset = u32::from(RAIL_TRACK_SLOPED_OFFSETS[(th - 1) as usize]);
    let mut sid = RAIL_SPRITE_TRACK_Y + offset;
    if snow_ground {
        sid += RAIL_SPRITE_SNOW_OFFSET;
    }
    Some(sid)
}

#[inline]
fn push_rail_junction_overlays(t: u8, out: &mut Vec<u32>) {
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

#[inline]
fn is_rail_junction_trackbits(t: u8) -> bool {
    !matches!(
        t,
        RAIL_TB_Y
            | RAIL_TB_X
            | RAIL_TB_UPPER
            | RAIL_TB_LOWER
            | RAIL_TB_RIGHT
            | RAIL_TB_LEFT
            | RAIL_TB_CROSS
            | RAIL_TB_HORZ
            | RAIL_TB_VERT
    )
}

/// IDs de sprites de vía férrea usados (cruce a nivel 1370–1373; nieve 1037/1038; pendiente 1023–1034).
pub const RAIL_SPRITE_IDS: [u32; 38] = [
    1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020,
    1021, 1022, 1023, 1024, 1025, 1026, 1027, 1028, 1029, 1030, 1031, 1032, 1033, 1034, 1035, 1036,
    1037, 1038, 1370, 1371, 1372, 1373,
];

/// `SPR_RAIL_TRACK_Y_SNOW` / `SPR_RAIL_TRACK_X_SNOW` (OpenGFX).
pub const RAIL_SPRITE_Y_SNOW: u32 = 1037;
pub const RAIL_SPRITE_X_SNOW: u32 = 1038;

/// Sprites de señal que la fórmula puede calcular pero el NFO recortado de OpenGFX no exporta (SP3.0 audit).
pub const SIGNAL_SPRITE_OPENGFX_GAPS: &[u32] = &[1438, 1439, 1530, 1532, 1540, 1542, 1546, 1548];

/// `RoadTileType::Crossing` en bits 6–7 de `m5` (`road_map.h`).
#[must_use]
pub fn is_road_level_crossing(mapt: u8, m5: u8, kind: TileKind, mp_road: u8) -> bool {
    kind == TileKind::Road && (mapt >> 4) & 0xF == mp_road && ((m5 >> 6) & 0x3) == 1
}

/// Sprite de raíl del cruce: `GetRailTypeInfo(...)->base_sprites.crossing + GetCrossingRailAxis(tile)` → 1370 + eje de **vía**.
/// Si el cruce está barrado (`IsCrossingBarred`, bit 5 de `m5`), OpenTTD suma **+2** al sprite (`road_cmd.cpp`).
#[must_use]
pub fn level_crossing_rail_sprite_id(m5: u8) -> u32 {
    const SPR_CROSSING_OFF_X_RAIL: u32 = 1370;
    let road_axis = m5 & 1;
    let rail_axis = 1 - road_axis;
    let mut sid = SPR_CROSSING_OFF_X_RAIL + u32::from(rail_axis);
    if (m5 >> 5) & 1 != 0 {
        sid += 2;
    }
    sid
}

/// Reserva PBS en el cruce (bit 4 de `m5`, `HasCrossingReservation`).
#[must_use]
pub fn level_crossing_has_rail_reservation(m5: u8) -> bool {
    (m5 >> 4) & 1 != 0
}

/// Vía con señales (`RailTileType::Signals`, bits 6–7 de `m5`).
#[must_use]
pub fn rail_tile_is_signals(m5: u8) -> bool {
    (m5 >> 6) & 0x3 == RAIL_TILE_SIGNALS
}

// OpenTTD `Track` / `TrackBits` (`track_type.h`, `rail_cmd.cpp::DrawSignals`).
const OTTD_TRACK_X: u8 = 0;
const OTTD_TRACK_Y: u8 = 1;
const OTTD_TRACK_UPPER: u8 = 2;
const OTTD_TRACK_LOWER: u8 = 3;
const OTTD_TRACK_LEFT: u8 = 4;
const OTTD_TRACK_RIGHT: u8 = 5;
const TB_X: u8 = 1 << OTTD_TRACK_X;
const TB_Y: u8 = 1 << OTTD_TRACK_Y;
const TB_UPPER: u8 = 1 << OTTD_TRACK_UPPER;
const TB_LOWER: u8 = 1 << OTTD_TRACK_LOWER;
const TB_LEFT: u8 = 1 << OTTD_TRACK_LEFT;
const TB_RIGHT: u8 = 1 << OTTD_TRACK_RIGHT;

/// Sprite base OpenTTD para señales eléctricas clásicas (`SPR_ORIGINAL_SIGNALS_BASE`).
const SPR_ORIGINAL_SIGNALS_BASE: u32 = 1275;
/// Base por defecto OpenGFX 8bpp para señales no “clásicas eléctricas” (semáforo/PBS); +77 respecto a 1275 en el GRF base.
const SPR_SIGNAL_ALT_BASE: u32 = 1352;

/// Bases de sprite para señales (OpenGFX 8bpp por defecto). Sobrescribibles con `OPENTTDRS_SIGNAL_BASE` / `OPENTTDRS_SIGNAL_ALT_BASE` (valores 512–4096).
#[must_use]
pub fn signal_sprite_bases() -> (u32, u32) {
    static MAIN: OnceLock<u32> = OnceLock::new();
    static ALT: OnceLock<u32> = OnceLock::new();
    let main = *MAIN.get_or_init(|| {
        config::env_u32_in_range(
            "OPENTTDRS_SIGNAL_BASE",
            SPR_ORIGINAL_SIGNALS_BASE,
            512..=4096,
        )
    });
    let alt = *ALT.get_or_init(|| {
        config::env_u32_in_range("OPENTTDRS_SIGNAL_ALT_BASE", SPR_SIGNAL_ALT_BASE, 512..=4096)
    });
    (main, alt)
}
const SIGTYPE_LAST_NOPBS: u8 = 3;

#[inline]
fn signal_type_from_m2(m2: u8, track: u8) -> u8 {
    let base = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        4
    } else {
        0
    };
    (m2 >> base) & 7
}

#[inline]
fn signal_variant_from_m2(m2: u8, track: u8) -> u8 {
    let bit = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        7
    } else {
        3
    };
    (m2 >> bit) & 1
}

/// Offset de imagen en la hoja de señales (`SignalOffsets` en `rail_cmd.cpp`).
#[inline]
fn signal_sprite_id(sig_type: u8, variant: u8, image: u8, green: bool) -> u32 {
    let (spr_main, spr_alt) = signal_sprite_bases();
    let cond = u32::from(green);
    let pbs_extra = if sig_type > SIGTYPE_LAST_NOPBS { 64 } else { 0 };
    let base = if sig_type == 0 && variant == 0 {
        spr_main
    } else {
        spr_alt
    };
    base + u32::from(sig_type) * 16
        + u32::from(variant) * 64
        + u32::from(image) * 2
        + cond
        + pbs_extra
}

/// Bits de señal presentes en el nibble alto de M3LO (`GetPresentSignals`, `rail_map.h`).
#[must_use]
pub fn rail_signal_present_mask(m3: u8) -> u8 {
    (m3 >> 4) & 0x0F
}

/// Estados rojo/verde por bit de señal: nibble alto de **`m4()`** (`GetSignalStates`); el chunk save `M3HI` carga en `m4` (`map_sl.cpp`), exportado como `m3hi` en `.ottdmap`.
#[must_use]
pub fn rail_signal_state_mask(m3hi: u8) -> u8 {
    (m3hi >> 4) & 0x0F
}

/// IDs de sprites (OpenGFX) para cada señal visible en la tesela, en orden de pintado.
/// Replica la selección de `DrawSignals` + fórmula de `DrawSingleSignal` para el bloque clásico.
#[must_use]
pub fn collect_signal_sprite_ids(m2: u8, m3: u8, m3hi: u8, m5: u8) -> Vec<u32> {
    if !rail_tile_is_signals(m5) {
        return Vec::new();
    }
    let rails = m5 & 0x3F;
    let present = rail_signal_present_mask(m3);
    let states = rail_signal_state_mask(m3hi);
    if present == 0 {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(4);
    let mut push_if = |sig_bit: u8, image: u8, track: u8| {
        if present & (1 << sig_bit) == 0 {
            return;
        }
        let green = (states >> sig_bit) & 1 != 0;
        let ty = signal_type_from_m2(m2, track);
        let var = signal_variant_from_m2(m2, track);
        out.push(signal_sprite_id(ty, var, image, green));
    };

    if rails & TB_Y == 0 {
        if rails & TB_X == 0 {
            if rails & TB_LEFT != 0 {
                push_if(2, 7, OTTD_TRACK_LEFT); // NORTH
                push_if(3, 6, OTTD_TRACK_LEFT); // SOUTH
            }
            if rails & TB_RIGHT != 0 {
                push_if(0, 7, OTTD_TRACK_RIGHT);
                push_if(1, 6, OTTD_TRACK_RIGHT);
            }
            if rails & TB_UPPER != 0 {
                push_if(3, 5, OTTD_TRACK_UPPER); // WEST
                push_if(2, 4, OTTD_TRACK_UPPER); // EAST
            }
            if rails & TB_LOWER != 0 {
                push_if(1, 5, OTTD_TRACK_LOWER);
                push_if(0, 4, OTTD_TRACK_LOWER);
            }
        } else {
            push_if(3, 0, OTTD_TRACK_X); // SW
            push_if(2, 1, OTTD_TRACK_X); // NE
        }
    } else {
        push_if(3, 2, OTTD_TRACK_Y); // SE
        push_if(2, 3, OTTD_TRACK_Y); // NW
    }
    out
}

/// Construye un byte `m2` que produce `sig_type` / `variant` para el `track` dado (`DrawSignals`).
#[inline]
fn m2_for_signal_encoding(sig_type: u8, variant: u8, track: u8) -> u8 {
    let base = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        4
    } else {
        0
    };
    let var_bit = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        7
    } else {
        3
    };
    ((sig_type & 7) << base) | ((variant & 1) << var_bit)
}

/// IDs de señal que [`collect_signal_sprite_ids`] puede emitir (no el producto cartesiano completo).
#[must_use]
pub fn signal_sprite_ids_for_preload() -> Vec<u32> {
    use std::collections::BTreeSet;
    let mut set = BTreeSet::new();
    for rails in 0u8..64 {
        let m5 = (RAIL_TILE_SIGNALS << 6) | rails;
        for present in 1u8..16 {
            let m3 = present << 4;
            for states in 0u8..16 {
                let m3hi = states << 4;
                for ty in 0u8..8 {
                    for var in 0u8..2 {
                        for track in 0u8..6 {
                            let m2 = m2_for_signal_encoding(ty, var, track);
                            for id in collect_signal_sprite_ids(m2, m3, m3hi, m5) {
                                set.insert(id);
                            }
                        }
                    }
                }
            }
        }
    }
    set.into_iter().collect()
}

/// IDs para precargar `assets/opengfx/tiles/rail_<id>.png`: piezas de vía + señales alcanzables.
#[must_use]
pub fn rail_sprite_ids_for_preload() -> Vec<u32> {
    use std::collections::BTreeSet;
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<u32>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut set: BTreeSet<u32> = RAIL_SPRITE_IDS.iter().copied().collect();
            for th in 1..=14 {
                if let Some(id) = rail_sloped_track_sprite_id(th, true) {
                    set.insert(id);
                }
            }
            for id in signal_sprite_ids_for_preload() {
                if !SIGNAL_SPRITE_OPENGFX_GAPS.contains(&id) {
                    set.insert(id);
                }
            }
            set.into_iter().collect()
        })
        .clone()
}

pub fn effective_rail_trackbits(mapt: u8, m5: u8, kind: TileKind, mp_rail: u8) -> Option<u8> {
    if kind != TileKind::Rail {
        return None;
    }
    let tt = (mapt >> 4) & 0xF;
    if tt != mp_rail {
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
        matches!(
            map.get_kind(c),
            Some(
                TileKind::Rail
                    | TileKind::Station
                    | TileKind::RailDepot
                    | TileKind::RailTunnel
                    | TileKind::RailBridge
            )
        )
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

pub fn rail_trackbits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32, mp_rail: u8) -> u8 {
    if let Some(t) = map.get(pos) {
        if let Some(tb) = effective_rail_trackbits(t.mapt, t.m5, t.kind, mp_rail) {
            return tb & 0x3F;
        }
        if t.kind == TileKind::Rail {
            return synthetic_rail_trackbits(map, pos, mw, mh);
        }
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
/// Con `snow_ground`, tramos planos Y/X usan `1037`/`1038`; en pendiente se suma
/// [`RAIL_SPRITE_SNOW_OFFSET`] al sprite inclinado (`rail_cmd.cpp`).
pub fn collect_rail_sprites(tb: u8, tileh: u8, snow_ground: bool, out: &mut Vec<u32>) {
    out.clear();
    let t = tb & 0x3F;
    if t == 0 {
        return;
    }
    if tileh != 0 {
        if let Some(sid) = rail_sloped_track_sprite_id(tileh, snow_ground) {
            out.push(sid);
        }
        if is_rail_junction_trackbits(t) {
            push_rail_junction_overlays(t, out);
        }
        return;
    }
    let y_track = if snow_ground {
        RAIL_SPRITE_Y_SNOW
    } else {
        RAIL_SPRITE_TRACK_Y
    };
    let x_track = if snow_ground {
        RAIL_SPRITE_X_SNOW
    } else {
        1012
    };
    match t {
        RAIL_TB_Y => out.push(y_track),
        RAIL_TB_X => out.push(x_track),
        RAIL_TB_UPPER => out.push(1013),
        RAIL_TB_LOWER => out.push(1014),
        RAIL_TB_RIGHT => out.push(1015),
        RAIL_TB_LEFT => out.push(1016),
        RAIL_TB_CROSS => out.push(1017),
        RAIL_TB_HORZ => out.push(1035),
        RAIL_TB_VERT => out.push(1036),
        _ => {
            out.push(1018_u32 + u32::from(junction_ground_off(t)));
            push_rail_junction_overlays(t, out);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{Map, TileCoord, TileKind};

    #[test]
    fn collect_rail_sprites_uses_snow_track_ids() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_Y, 0, true, &mut out);
        assert_eq!(out, vec![RAIL_SPRITE_Y_SNOW]);
        collect_rail_sprites(RAIL_TB_X, 0, true, &mut out);
        assert_eq!(out, vec![RAIL_SPRITE_X_SNOW]);
    }

    #[test]
    fn collect_rail_sprites_uses_sloped_track_on_diagonal_slopes() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_Y, 12, false, &mut out);
        assert_eq!(out, vec![1031]);
        collect_rail_sprites(RAIL_TB_X, 6, false, &mut out);
        assert_eq!(out, vec![1032]);
        collect_rail_sprites(RAIL_TB_Y, 3, false, &mut out);
        assert_eq!(out, vec![1033]);
        collect_rail_sprites(RAIL_TB_X, 9, false, &mut out);
        assert_eq!(out, vec![1034]);
    }

    #[test]
    fn collect_rail_sprites_sloped_snow_adds_offset() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_Y, 12, true, &mut out);
        assert_eq!(out, vec![1031 + RAIL_SPRITE_SNOW_OFFSET]);
    }

    #[test]
    fn collect_rail_sprites_sloped_t_adds_junction_overlays() {
        let mut out = Vec::new();
        collect_rail_sprites(0x07, 12, false, &mut out);
        assert_eq!(out.first(), Some(&1031));
        assert!(out.contains(&1005));
        assert!(out.contains(&1006));
        assert!(out.contains(&1007));
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn rail_tile_with_zero_m5_does_not_use_synthetic_neighbors() {
        let mut map = Map::new_flat(3, 3, 0);
        let c = TileCoord::new(1, 1);
        map.set_kind(c, TileKind::Rail).unwrap();
        map.set_mapt_m5(c, 0x10, 0).unwrap();
        map.set_kind(TileCoord::new(0, 1), TileKind::Rail).unwrap();
        map.set_mapt_m5(TileCoord::new(0, 1), 0x10, 0x02).unwrap();
        assert_eq!(rail_trackbits_for_render(&map, c, 3, 3, 1), 0);
    }

    #[test]
    fn signal_preload_excludes_known_opengfx_gaps() {
        let ids: std::collections::BTreeSet<_> =
            rail_sprite_ids_for_preload().into_iter().collect();
        for gap in SIGNAL_SPRITE_OPENGFX_GAPS {
            assert!(
                !ids.contains(gap),
                "hueco OpenGFX {gap} no debe precargarse"
            );
        }
        assert!(ids.contains(&1279));
    }
}
