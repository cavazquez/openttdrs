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

/// IDs de sprites de vía férrea usados (cruce a nivel 1370–1373 con barreras, `road_cmd.cpp`).
pub const RAIL_SPRITE_IDS: [u32; 24] = [
    1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020,
    1021, 1022, 1035, 1036, 1370, 1371, 1372, 1373,
];

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

/// IDs para precargar `opengfx/tiles/rail_<id>.png`: piezas de vía y **todas** las señales que
/// puede devolver [`signal_sprite_id`] con las bases por defecto / env (`OPENTTDRS_SIGNAL_*`).
///
/// Evita `asset_server.load` sobre el rango entero `1275..1520` cuando OpenGFX no incluye cada
/// fila del NFO (huecos sin PNG).
#[must_use]
pub fn rail_sprite_ids_for_preload() -> Vec<u32> {
    use std::collections::BTreeSet;
    let mut set: BTreeSet<u32> = RAIL_SPRITE_IDS.iter().copied().collect();
    for sig_type in 0u8..=7u8 {
        for variant in 0u8..=1u8 {
            for image in 0u8..=7u8 {
                for green in [false, true] {
                    set.insert(signal_sprite_id(sig_type, variant, image, green));
                }
            }
        }
    }
    set.into_iter().collect()
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

pub fn rail_trackbits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32, mp_rail: u8) -> u8 {
    if let Some(t) = map.get(pos)
        && let Some(tb) = effective_rail_trackbits(t.mapt, t.m5, t.kind, mp_rail)
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
