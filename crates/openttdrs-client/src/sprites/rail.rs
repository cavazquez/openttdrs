use std::sync::OnceLock;

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, TileKind};

use super::road::RoadDepotLayerGfx;
use crate::config;
use crate::iso::remap_tile_offset;

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

/// `SPR_RAIL_TRACK_Y` / `SPR_RAIL_TRACK_X` (`rail_cmd.cpp`).
pub const RAIL_SPRITE_TRACK_Y: u32 = 1011;
pub const RAIL_SPRITE_TRACK_X: u32 = 1012;

/// Offset OpenGFX monorail respecto a vía normal (`SPR_MONORAIL_*` = rail + 82).
pub const MONO_RAIL_SPRITE_OFFSET: u32 = 82;
/// Offset OpenGFX maglev respecto a vía normal (`SPR_MAGLEV_*` = rail + 164).
pub const MAGLEV_RAIL_SPRITE_OFFSET: u32 = 164;

/// Base de cables de catenaria en OpenGFX 8 (`ogfx1_base` 1039–1062 = `WSO_*`).
pub const WIRE_SPRITE_BASE: u32 = 1039;
/// Último sprite de wire plano/inclinado en el set OpenGFX extraído.
pub const WIRE_SPRITE_LAST: u32 = 1062;

/// `WireSpriteOffset` (plano, postes en ambos extremos) — `elrail_data.h`.
const WSO_X_SHORT: u32 = 0;
const WSO_Y_SHORT: u32 = 1;
const WSO_EW_SHORT: u32 = 2;
const WSO_NS_SHORT: u32 = 3;

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

/// ¿Hay PNG tipado (mono/maglev) para este ID de vía plana?
#[must_use]
pub fn rail_sprite_has_typed_asset(id: u32) -> bool {
    matches!(
        id,
        1005..=1012 | 1018..=1022 | 1035 // overlays, Y/X, junction ground, HORZ
    )
}

/// Remapea un sprite de vía clásica al set mono/maglev si hay asset.
#[must_use]
pub fn remap_rail_sprite_id(id: u32, rail_type: openttdrs_core::RailType) -> u32 {
    use openttdrs_core::RailType;
    let offset = match rail_type {
        RailType::Monorail => MONO_RAIL_SPRITE_OFFSET,
        RailType::Maglev => MAGLEV_RAIL_SPRITE_OFFSET,
        RailType::Rail | RailType::Electric => return id,
    };
    if rail_sprite_has_typed_asset(id) {
        id + offset
    } else {
        id
    }
}

/// Nombre(s) de atlas para un ID de vía (alias `rail_<id>` o `mono_*` / `mglv_*`).
#[must_use]
pub fn rail_sprite_atlas_keys(id: u32) -> Vec<String> {
    let mut keys = vec![format!("rail_{id}.png")];
    if let Some(alt) = rail_sprite_named_alias(id) {
        keys.push(alt);
    }
    keys
}

fn rail_sprite_named_alias(id: u32) -> Option<String> {
    match id {
        1087..=1092 => Some(format!("mono_single_{}.png", id - 1087)),
        1093 => Some("mono_track_y.png".into()),
        1094 => Some("mono_track_x.png".into()),
        1100..=1117 => Some(format!("mono_track_{}.png", id - 1100)),
        1169..=1174 => Some(format!("mglv_single_{}.png", id - 1169)),
        1175 => Some("mglv_track_y.png".into()),
        1176 => Some("mglv_track_x.png".into()),
        1182..=1199 => Some(format!("mglv_track_{}.png", id - 1182)),
        _ => None,
    }
}

/// ¿El ID pertenece al set tipado mono/maglev (no vía clásica)?
#[must_use]
pub fn is_typed_rail_track_sprite(id: u32) -> bool {
    matches!(id, 1087..=1117 | 1169..=1199)
}

/// IDs de sprites de vía férrea usados (cruce a nivel 1370–1373; nieve 1037/1038; pendiente 1023–1034).
pub const RAIL_SPRITE_IDS: [u32; 38] = [
    1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020,
    1021, 1022, 1023, 1024, 1025, 1026, 1027, 1028, 1029, 1030, 1031, 1032, 1033, 1034, 1035, 1036,
    1037, 1038, 1370, 1371, 1372, 1373,
];

/// Sprites de catenaria plana OpenGFX (`WIRE_SPRITE_BASE`..`WIRE_SPRITE_LAST`).
pub fn catenary_wire_sprite_ids() -> impl Iterator<Item = u32> {
    WIRE_SPRITE_BASE..=WIRE_SPRITE_LAST
}

/// Cables de catenaria para vía plana (`tileh == 0`), MVP sin lógica de vecinos/PCP.
///
/// Usa variantes `*_SHORT` (postes en ambos extremos) de `elrail_data.h`.
pub fn collect_catenary_flat_sprites(tb: u8, out: &mut Vec<u32>) {
    out.clear();
    let t = tb & 0x3F;
    if t == 0 {
        return;
    }
    match t {
        RAIL_TB_X => out.push(WIRE_SPRITE_BASE + WSO_X_SHORT),
        RAIL_TB_Y => out.push(WIRE_SPRITE_BASE + WSO_Y_SHORT),
        RAIL_TB_HORZ => out.push(WIRE_SPRITE_BASE + WSO_EW_SHORT),
        RAIL_TB_VERT => out.push(WIRE_SPRITE_BASE + WSO_NS_SHORT),
        RAIL_TB_CROSS => {
            out.push(WIRE_SPRITE_BASE + WSO_X_SHORT);
            out.push(WIRE_SPRITE_BASE + WSO_Y_SHORT);
        }
        _ => {
            if t & RAIL_TB_X != 0 {
                out.push(WIRE_SPRITE_BASE + WSO_X_SHORT);
            }
            if t & RAIL_TB_Y != 0 {
                out.push(WIRE_SPRITE_BASE + WSO_Y_SHORT);
            }
            if t & (RAIL_TB_UPPER | RAIL_TB_LOWER) != 0 {
                out.push(WIRE_SPRITE_BASE + WSO_EW_SHORT);
            }
            if t & (RAIL_TB_LEFT | RAIL_TB_RIGHT) != 0 {
                out.push(WIRE_SPRITE_BASE + WSO_NS_SHORT);
            }
        }
    }
}

/// `SPR_RAIL_TRACK_Y_SNOW` / `SPR_RAIL_TRACK_X_SNOW` (OpenGFX).
pub const RAIL_SPRITE_Y_SNOW: u32 = 1037;
pub const RAIL_SPRITE_X_SNOW: u32 = 1038;

/// Suelo del depósito de vía por dirección (`_depot_gfx_table`, `track_land.h`):
/// NE/NW usan hierba (ya dibujada por el pase de terreno); SE usa `SPR_RAIL_TRACK_Y`
/// (1011) y SW `SPR_RAIL_TRACK_X` (1012) para mostrar la vía de salida.
pub const RAIL_DEPOT_GROUND_TRACK: [Option<u32>; 4] = [None, Some(1011), Some(1012), None];

/// Capas BUILD del depósito de vía (`_depot_gfx_NE..NW` en `track_land.h`,
/// sprites 1063–1068). Indexado por `m5 & 3`: 0=NE, 1=SE, 2=SW, 3=NW.
///
/// Los offsets ya vienen *horneados* respecto al vértice norte de la tesela:
/// `x_offs = 2·(dy−dx) + x_offs_NFO` y `y_offs = (dx+dy) + y_offs_NFO`, con los
/// `dx`/`dy` TILE_SEQ de `track_land.h` y los offsets del NFO de OpenGFX (la
/// cadena `remap_tile_offset` del cliente usa el doble de escala que
/// `RemapCoords`, así que se evita pasando `dx = dy = 0`).
pub const RAIL_DEPOT_BUILD_LAYERS: [&[RoadDepotLayerGfx]; 4] = [
    // NE: edificio único con la entrada hacia el noreste.
    &[RoadDepotLayerGfx {
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        z: 0.05,
        w: 51.0,
        h: 38.0,
        x_offs: -22.0,
        y_offs: -12.0,
        remap_x_adj: 0.0,
        path: "assets/opengfx/tiles/rail_depot_ne.png",
    }],
    // SE: tope del muro trasero + fachada frontal sobre la vía.
    &[
        RoadDepotLayerGfx {
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            z: 0.05,
            w: 10.0,
            h: 9.0,
            x_offs: 14.0,
            y_offs: 8.0,
            remap_x_adj: 0.0,
            path: "assets/opengfx/tiles/rail_depot_se_1.png",
        },
        RoadDepotLayerGfx {
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            z: 0.06,
            w: 51.0,
            h: 38.0,
            x_offs: -23.0,
            y_offs: -11.0,
            remap_x_adj: 0.0,
            path: "assets/opengfx/tiles/rail_depot_se_2.png",
        },
    ],
    // SW: tope del muro trasero + fachada frontal sobre la vía.
    &[
        RoadDepotLayerGfx {
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            z: 0.05,
            w: 10.0,
            h: 9.0,
            x_offs: -20.0,
            y_offs: 8.0,
            remap_x_adj: 0.0,
            path: "assets/opengfx/tiles/rail_depot_sw_1.png",
        },
        RoadDepotLayerGfx {
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            z: 0.06,
            w: 51.0,
            h: 38.0,
            x_offs: -24.0,
            y_offs: -11.0,
            remap_x_adj: 0.0,
            path: "assets/opengfx/tiles/rail_depot_sw_2.png",
        },
    ],
    // NW: edificio único con la entrada hacia el noroeste.
    &[RoadDepotLayerGfx {
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
        z: 0.05,
        w: 51.0,
        h: 38.0,
        x_offs: -25.0,
        y_offs: -12.0,
        remap_x_adj: 0.0,
        path: "assets/opengfx/tiles/rail_depot_nw.png",
    }],
];

#[must_use]
pub fn rail_depot_build_layers(dir: usize) -> &'static [RoadDepotLayerGfx] {
    RAIL_DEPOT_BUILD_LAYERS[dir.min(3)]
}

/// Sprites de señal que la fórmula puede calcular pero el NFO recortado de OpenGFX no exporta (SP3.0 audit).
pub const SIGNAL_SPRITE_OPENGFX_GAPS: &[u32] = &[1438, 1439, 1530, 1532, 1540, 1542, 1546, 1548];

/// `RoadTileType::Crossing` en bits 6–7 de `m5` (`road_map.h`).
#[must_use]
pub fn is_road_level_crossing(mapt: u8, m5: u8, kind: TileKind, mp_road: u8) -> bool {
    kind == TileKind::Road && (mapt >> 4) & 0xF == mp_road && ((m5 >> 6) & 0x3) == 1
}

/// Sprite de raíl del cruce: `GetRailTypeInfo(...)->base_sprites.crossing + GetCrossingRailAxis(tile)`.
/// Si el cruce está barrado (`IsCrossingBarred`, bit 5 de `m5`), OpenTTD suma **+2** (`road_cmd.cpp`).
#[must_use]
pub fn level_crossing_rail_sprite_id(m5: u8) -> u32 {
    level_crossing_rail_sprite_id_for_type(m5, openttdrs_core::RailType::Rail)
}

/// Como [`level_crossing_rail_sprite_id`], eligiendo base mono/maglev.
#[must_use]
pub fn level_crossing_rail_sprite_id_for_type(m5: u8, rail_type: openttdrs_core::RailType) -> u32 {
    use openttdrs_core::RailType;
    let base = match rail_type {
        RailType::Monorail => 1382,
        RailType::Maglev => 1394,
        RailType::Rail | RailType::Electric => 1370,
    };
    let road_axis = m5 & 1;
    let rail_axis = 1 - road_axis;
    let mut sid = base + u32::from(rail_axis);
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

pub use openttdrs_core::rail_tile_has_pbs_reservation;

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

/// Coordenadas sub-tesela OpenTTD (`SignalPositions[side][pos]`, lado izquierdo).
const SIGNAL_SUBTILE_XY: [(i8, i8); 12] = [
    (8, 5),
    (14, 1),
    (1, 14),
    (9, 11),
    (1, 0),
    (3, 10),
    (11, 4),
    (14, 14),
    (11, 3),
    (4, 13),
    (3, 4),
    (11, 13),
];

#[inline]
fn signal_subtile_xy(pos: u8) -> (i8, i8) {
    SIGNAL_SUBTILE_XY[pos.min(11) as usize]
}

/// PNG a cargar para un ID lógico de señal (`DrawSingleSignal` → atlas).
/// OpenGFX2 reutiliza el NFO base en 1416–1419 (topadora u otro gráfico); el bloque
/// eléctrico clásico exportado vive en 1275–1278 (`sid - 141`).
#[must_use]
pub fn signal_sprite_texture_id(sprite_id: u32) -> u32 {
    if (1416..=1419).contains(&sprite_id) {
        return sprite_id - 141;
    }
    sprite_id
}

/// Ajuste del centro del sprite 3×14 respecto al ancla `DrawSingleSignal`
/// (xrel/yrel OpenGFX + mitad del bbox; Bevy ancla al centro del sprite).
#[must_use]
pub fn signal_sprite_center_offset(tex_id: u32) -> Vec2 {
    match tex_id {
        1275 | 1276 => Vec2::new(0.5, 5.0),
        1277 | 1278 => Vec2::new(1.5, 5.0),
        _ => Vec2::ZERO,
    }
}

/// Posición en pantalla de una señal, alineada al mismo ancla que la vía (`tile_pos_half`).
///
/// OpenTTD usa `RemapCoords(16·tx + ox, 16·ty + oy)`; en este cliente el delta
/// sub-tesela respecto al centro del rombo coincide con [`rail_signal_subtile_offset`].
#[must_use]
pub fn signal_screen_position(
    tx: i32,
    ty: i32,
    pos: u8,
    tex_id: u32,
    half_h: f32,
    base_z: u8,
) -> Vec2 {
    let p = crate::iso::iso(tx, ty);
    let elev = f32::from(base_z) * crate::iso::HEIGHT_PX;
    let track_base = Vec2::new(p.x, p.y - half_h + elev);
    track_base + rail_signal_subtile_offset(pos) + signal_sprite_center_offset(tex_id)
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

/// Sprites de señal visibles en la tesela, con carril para el offset de dibujo.
#[must_use]
pub fn collect_signal_sprite_draws(m2: u8, m3: u8, m3hi: u8, m5: u8) -> Vec<SignalSpriteDraw> {
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
    let mut push_if = |sig_bit: u8, image: u8, track: u8, pos: u8| {
        if present & (1 << sig_bit) == 0 {
            return;
        }
        let green = (states >> sig_bit) & 1 != 0;
        let ty = signal_type_from_m2(m2, track);
        let var = signal_variant_from_m2(m2, track);
        out.push(SignalSpriteDraw {
            sprite_id: signal_sprite_texture_id(signal_sprite_id(ty, var, image, green)),
            track,
            pos,
        });
    };

    if rails & TB_Y == 0 {
        if rails & TB_X == 0 {
            if rails & TB_LEFT != 0 {
                push_if(2, 7, OTTD_TRACK_LEFT, 0); // NORTH
                push_if(3, 6, OTTD_TRACK_LEFT, 1); // SOUTH
            }
            if rails & TB_RIGHT != 0 {
                push_if(0, 7, OTTD_TRACK_RIGHT, 2);
                push_if(1, 6, OTTD_TRACK_RIGHT, 3);
            }
            if rails & TB_UPPER != 0 {
                push_if(3, 5, OTTD_TRACK_UPPER, 4); // WEST
                push_if(2, 4, OTTD_TRACK_UPPER, 5); // EAST
            }
            if rails & TB_LOWER != 0 {
                push_if(1, 5, OTTD_TRACK_LOWER, 6);
                push_if(0, 4, OTTD_TRACK_LOWER, 7);
            }
        } else {
            push_if(3, 0, OTTD_TRACK_X, 8); // SW
            push_if(2, 1, OTTD_TRACK_X, 9); // NE
        }
    } else {
        push_if(3, 2, OTTD_TRACK_Y, 10); // SE
        push_if(2, 3, OTTD_TRACK_Y, 11); // NW
    }
    out
}

/// IDs de sprites (OpenGFX) para cada señal visible en la tesela, en orden de pintado.
/// Replica la selección de `DrawSignals` + fórmula de `DrawSingleSignal` para el bloque clásico.
#[must_use]
pub fn collect_signal_sprite_ids(m2: u8, m3: u8, m3hi: u8, m5: u8) -> Vec<u32> {
    collect_signal_sprite_draws(m2, m3, m3hi, m5)
        .into_iter()
        .map(|d| d.sprite_id)
        .collect()
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
            // Mono / maglev planos (overlays + Y/X + junction + HORZ).
            for id in [
                1005u32, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1018, 1019, 1020, 1021, 1022,
                1035,
            ] {
                set.insert(id + MONO_RAIL_SPRITE_OFFSET);
                set.insert(id + MAGLEV_RAIL_SPRITE_OFFSET);
            }
            for id in catenary_wire_sprite_ids() {
                set.insert(id);
            }
            // Cruces tipados (eje + barrado).
            for base in [1382u32, 1394] {
                for d in 0..4 {
                    set.insert(base + d);
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

/// Índice del suelo de cruce (1018 + offset), igual que `GetJunctionGroundSpriteOffset`.
/// Devuelve el sprite donde falta al menos un bit del patrón 3-vías NE/SW/NW/SE.
#[inline]
fn junction_ground_off(tb: u8) -> u8 {
    let t = tb & 0x3F;
    if (t & RAIL_3WAY_NE) != RAIL_3WAY_NE {
        return 0;
    }
    if (t & RAIL_3WAY_SW) != RAIL_3WAY_SW {
        return 1;
    }
    if (t & RAIL_3WAY_NW) != RAIL_3WAY_NW {
        return 2;
    }
    if (t & RAIL_3WAY_SE) != RAIL_3WAY_SE {
        return 3;
    }
    4
}

/// Desplazamiento del overlay de riel respecto al centro del sprite compuesto 64×31.
/// Derivado de xrel/yrel OpenGFX (`ogfx21_base_32ez.nfo`) vs `1013` (UPPER, xrel=-31).
#[must_use]
pub fn rail_ghost_overlay_offset(sprite_id: u32) -> Vec2 {
    const FULL_XREL: f32 = -31.0;
    const FULL_W: f32 = 64.0;
    const FULL_YREL: f32 = 0.0;
    const FULL_H: f32 = 31.0;
    const FULL_CENTER_X: f32 = FULL_XREL + FULL_W / 2.0;
    const FULL_CENTER_Y: f32 = FULL_YREL + FULL_H / 2.0;

    // Overlays mono/maglev comparten anclas con 1005–1010.
    let base_id = match sprite_id {
        1087..=1092 => sprite_id - MONO_RAIL_SPRITE_OFFSET,
        1169..=1174 => sprite_id - MAGLEV_RAIL_SPRITE_OFFSET,
        other => other,
    };
    let (xrel, yrel, w, h) = match base_id {
        1005 => (-19.0, 5.0, 40.0, 21.0),
        1006 => (-19.0, 5.0, 40.0, 21.0),
        1007 => (-19.0, 5.0, 40.0, 7.0),
        1008 => (-18.0, 21.0, 38.0, 7.0),
        1009 => (11.0, 5.0, 12.0, 19.0),
        1010 => (-21.0, 5.0, 12.0, 20.0),
        _ => return Vec2::ZERO,
    };
    let cx = xrel + w / 2.0;
    let cy = yrel + h / 2.0;
    // Bevy: positivo en Y sube; el centro compuesto está en `-FULL_CENTER_Y` desde el vértice N.
    Vec2::new(cx - FULL_CENTER_X, FULL_CENTER_Y - cy)
}

/// Índice `SignalPositions` para `(track, sig_bit)` — mismo orden que `DrawSignals`.
#[must_use]
pub fn signal_draw_pos(ottd_track: u8, sig_bit: u8) -> u8 {
    match ottd_track {
        OTTD_TRACK_LEFT => {
            if sig_bit == 2 {
                0
            } else {
                1
            }
        }
        OTTD_TRACK_RIGHT => {
            if sig_bit == 0 {
                2
            } else {
                3
            }
        }
        OTTD_TRACK_UPPER => {
            if sig_bit == 3 {
                4
            } else {
                5
            }
        }
        OTTD_TRACK_LOWER => {
            if sig_bit == 1 {
                6
            } else {
                7
            }
        }
        OTTD_TRACK_X => {
            if sig_bit == 3 {
                8
            } else {
                9
            }
        }
        OTTD_TRACK_Y => {
            if sig_bit == 3 {
                10
            } else {
                11
            }
        }
        _ => 0,
    }
}

/// Sub-tesela OpenTTD (0–16) → desplazamiento desde el centro del rombo (`DrawSingleSignal`).
#[must_use]
pub fn rail_signal_subtile_offset(pos: u8) -> Vec2 {
    let (ox, oy) = signal_subtile_xy(pos);
    let dx = f32::from(ox) - 8.0;
    let dy = f32::from(oy) - 8.0;
    remap_tile_offset(dx, dy, 0.0)
}

/// Sprite de señal + carril para posicionamiento en pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalSpriteDraw {
    pub sprite_id: u32,
    pub track: u8,
    /// Índice en `SignalPositions` de OpenTTD (`DrawSingleSignal`, `rail_cmd.cpp`).
    pub pos: u8,
}

/// Sprites para el fantasma: mismos IDs que la vía colocada (`collect_rail_sprites`).
pub fn collect_rail_ghost_sprites(tb: u8, tileh: u8, out: &mut Vec<u32>) {
    collect_rail_sprites(tb, tileh, false, out);
}

/// Fantasma con tipo de vía activo del toolbar / partida.
pub fn collect_rail_ghost_sprites_for_type(
    tb: u8,
    tileh: u8,
    rail_type: openttdrs_core::RailType,
    out: &mut Vec<u32>,
) {
    collect_rail_sprites_for_type(tb, tileh, false, rail_type, out);
}

/// Lista de sprites planos (tesela nivelada o con cimiento nivelado).
///
/// Los tramos rectos y medias vías usan el sprite compuesto de OpenGFX
/// (`1011`/`1012`/`1013`…), que ya incluye su propio suelo alineado con el
/// terreno. Solo los cruces reales usan suelo `1018+` con overlays `1005-1010`.
fn collect_rail_flat_sprites(t: u8, snow_ground: bool, out: &mut Vec<u32>) {
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

/// Lista de sprites `OpenGFX` en orden de pintado (suelo de cruce y superposiciones).
/// Con `snow_ground`, tramos planos Y/X usan `1037`/`1038`; en pendiente se suma
/// [`RAIL_SPRITE_SNOW_OFFSET`] al sprite inclinado salvo cimiento nivelado (`GetRailFoundation` = 1).
pub fn collect_rail_sprites(tb: u8, tileh: u8, snow_ground: bool, out: &mut Vec<u32>) {
    collect_rail_sprites_for_type(tb, tileh, snow_ground, openttdrs_core::RailType::Rail, out);
}

/// Como [`collect_rail_sprites`], remapeando planos a mono/maglev cuando hay asset.
pub fn collect_rail_sprites_for_type(
    tb: u8,
    tileh: u8,
    snow_ground: bool,
    rail_type: openttdrs_core::RailType,
    out: &mut Vec<u32>,
) {
    out.clear();
    let t = tb & 0x3F;
    if t == 0 {
        return;
    }
    let foundation = openttdrs_core::rail_foundation_for_trackbits(tileh, t);
    if tileh != 0 && foundation != 1 {
        // Pendientes: sin set tipado en assets → vía clásica.
        if let Some(sid) = rail_sloped_track_sprite_id(tileh, snow_ground) {
            out.push(sid);
        }
        return;
    }
    collect_rail_flat_sprites(t, snow_ground, out);
    if matches!(
        rail_type,
        openttdrs_core::RailType::Monorail | openttdrs_core::RailType::Maglev
    ) {
        for sid in out.iter_mut() {
            *sid = remap_rail_sprite_id(*sid, rail_type);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::iso::{TILE_HALF_H, iso};
    use openttdrs_core::{Map, TileCoord, TileKind};

    #[test]
    fn collect_rail_on_leveled_foundation_uses_flat_track() {
        let mut out = Vec::new();
        // `SLOPE_EW` (5): vía X requiere cimiento nivelado → sprites planos.
        collect_rail_sprites(RAIL_TB_X, 5, false, &mut out);
        assert_eq!(out, vec![1012]);
        collect_rail_sprites(0x29, 5, false, &mut out);
        assert_eq!(out, vec![1018, 1005, 1008, 1009]);
    }

    #[test]
    fn collect_rail_sprites_depot_junction_uses_sw_ground_and_spaced_overlays() {
        let mut out = Vec::new();
        // Empalme depósito ↔ línea X (test `rail_depot_beside_x_line_connects_exit_tile`).
        collect_rail_sprites(0x29, 0, false, &mut out);
        assert_eq!(out, vec![1018, 1005, 1008, 1009]);
        // Salida depósito showcase (12,15): Y|LOWER|LEFT.
        collect_rail_sprites(0x1A, 0, false, &mut out);
        assert_eq!(out, vec![1018, 1006, 1008, 1010]);
    }

    #[test]
    fn junction_ground_off_matches_openttd_get_junction_offset() {
        assert_eq!(junction_ground_off(0x29), 0);
        assert_eq!(junction_ground_off(0x1A), 0);
        assert_eq!(junction_ground_off(RAIL_3WAY_NE), 1);
        assert_eq!(junction_ground_off(RAIL_3WAY_SW), 0);
        assert_eq!(junction_ground_off(RAIL_3WAY_NW), 0);
        assert_eq!(junction_ground_off(RAIL_3WAY_SE), 0);
        assert_eq!(junction_ground_off(0x3F), 4);
    }

    #[test]
    fn collect_rail_ghost_sprites_matches_flat_track_sprites() {
        let mut out = Vec::new();
        collect_rail_ghost_sprites(RAIL_TB_LEFT, 0, &mut out);
        assert_eq!(out, vec![1016]);
        collect_rail_ghost_sprites(RAIL_TB_UPPER, 0, &mut out);
        assert_eq!(out, vec![1013]);
        collect_rail_ghost_sprites(RAIL_TB_HORZ, 0, &mut out);
        assert_eq!(out, vec![1035]);
        collect_rail_ghost_sprites(RAIL_TB_X, 0, &mut out);
        assert_eq!(out, vec![1012]);
        collect_rail_ghost_sprites(RAIL_TB_Y, 0, &mut out);
        assert_eq!(out, vec![RAIL_SPRITE_TRACK_Y]);
    }

    #[test]
    fn collect_rail_sprites_remaps_mono_and_maglev_flat() {
        use openttdrs_core::RailType;
        let mut out = Vec::new();
        collect_rail_sprites_for_type(RAIL_TB_Y, 0, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1093]);
        collect_rail_sprites_for_type(RAIL_TB_X, 0, false, RailType::Maglev, &mut out);
        assert_eq!(out, vec![1176]);
        collect_rail_sprites_for_type(RAIL_TB_HORZ, 0, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1117]);
        // VERT sin asset tipado → se queda en clásico.
        collect_rail_sprites_for_type(RAIL_TB_VERT, 0, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1036]);
        // Junction: suelo SW + overlays tipados.
        collect_rail_sprites_for_type(0x29, 0, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1100, 1087, 1090, 1091]);
    }

    #[test]
    fn level_crossing_uses_typed_base() {
        use openttdrs_core::RailType;
        assert_eq!(
            level_crossing_rail_sprite_id_for_type(0x40, RailType::Monorail),
            1383
        );
        assert_eq!(
            level_crossing_rail_sprite_id_for_type(0x41, RailType::Maglev),
            1394
        );
    }

    #[test]
    fn rail_sprite_atlas_keys_prefer_named_mono() {
        let keys = rail_sprite_atlas_keys(1093);
        assert!(keys.iter().any(|k| k == "mono_track_y.png"));
        assert!(keys.iter().any(|k| k == "rail_1093.png"));
    }

    #[test]
    fn preload_includes_mono_maglev_ids() {
        let ids = rail_sprite_ids_for_preload();
        assert!(ids.contains(&1093));
        assert!(ids.contains(&1175));
        assert!(ids.contains(&1087));
        assert!(ids.contains(&1169));
        assert!(ids.contains(&1382));
        assert!(ids.contains(&1394));
        assert!(ids.contains(&WIRE_SPRITE_BASE));
        assert!(ids.contains(&WIRE_SPRITE_LAST));
    }

    #[test]
    fn collect_catenary_flat_maps_trackbits() {
        let mut out = Vec::new();
        collect_catenary_flat_sprites(RAIL_TB_X, &mut out);
        assert_eq!(out, vec![1039]);
        collect_catenary_flat_sprites(RAIL_TB_Y, &mut out);
        assert_eq!(out, vec![1040]);
        collect_catenary_flat_sprites(RAIL_TB_HORZ, &mut out);
        assert_eq!(out, vec![1041]);
        collect_catenary_flat_sprites(RAIL_TB_VERT, &mut out);
        assert_eq!(out, vec![1042]);
        collect_catenary_flat_sprites(RAIL_TB_CROSS, &mut out);
        assert_eq!(out, vec![1039, 1040]);
        collect_catenary_flat_sprites(0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn rail_ghost_overlay_offset_matches_opengfx_nfo() {
        assert_eq!(rail_ghost_overlay_offset(1005), Vec2::ZERO);
        assert_eq!(rail_ghost_overlay_offset(1006), Vec2::ZERO);
        assert_eq!(rail_ghost_overlay_offset(1007), Vec2::new(0.0, 7.0));
        assert_eq!(rail_ghost_overlay_offset(1008), Vec2::new(0.0, -9.0));
        assert_eq!(rail_ghost_overlay_offset(1009), Vec2::new(16.0, 1.0));
        assert_eq!(rail_ghost_overlay_offset(1010), Vec2::new(-16.0, 0.5));
    }

    #[test]
    fn rail_ghost_overlay_offset_separates_parallel_lanes() {
        assert!(rail_ghost_overlay_offset(1010).x < 0.0);
        assert!(rail_ghost_overlay_offset(1009).x > 0.0);
        assert_ne!(
            rail_ghost_overlay_offset(1007).y,
            rail_ghost_overlay_offset(1008).y
        );
    }

    #[test]
    fn collect_signal_draws_maps_electric_ids_to_classic_textures() {
        let m5 = (RAIL_TILE_SIGNALS << 6) | RAIL_TB_X;
        let m3 = 1 << (4 + 3); // SW present
        let m3hi = m3;
        let m2 = m2_for_signal_encoding(0, 1, OTTD_TRACK_X);
        let draws = collect_signal_sprite_draws(m2, m3, m3hi, m5);
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].sprite_id, 1276, "1417 eléctrica → PNG 1276");
    }

    #[test]
    fn signal_screen_position_anchors_to_track_tile_center() {
        let base = Vec2::new(iso(2, 2).x, iso(2, 2).y - TILE_HALF_H);
        let sw = signal_screen_position(2, 2, 8, 1276, TILE_HALF_H, 0);
        let ne = signal_screen_position(2, 2, 9, 1278, TILE_HALF_H, 0);
        assert_ne!(sw, ne);
        assert!(sw.distance(base) < 40.0);
        assert!(ne.distance(base) < 40.0);
        assert!((sw - ne).length() > 8.0, "lados opuestos del riel");
    }

    #[test]
    fn rail_signal_subtile_offset_places_x_track_on_diagonal() {
        let sw = rail_signal_subtile_offset(8); // X SW
        let ne = rail_signal_subtile_offset(9); // X NE
        assert_ne!(sw, Vec2::ZERO);
        assert_ne!(ne, Vec2::ZERO);
        assert_ne!(sw, ne);
    }

    #[test]
    fn signal_draw_pos_matches_draw_signals_order() {
        assert_eq!(signal_draw_pos(OTTD_TRACK_X, 3), 8);
        assert_eq!(signal_draw_pos(OTTD_TRACK_Y, 2), 11);
        assert_eq!(signal_draw_pos(OTTD_TRACK_UPPER, 3), 4);
    }

    #[test]
    fn collect_rail_sprites_uses_snow_track_ids() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_Y, 0, true, &mut out);
        assert_eq!(out, vec![RAIL_SPRITE_Y_SNOW]);
        collect_rail_sprites(RAIL_TB_X, 0, true, &mut out);
        assert_eq!(out, vec![RAIL_SPRITE_X_SNOW]);
    }

    #[test]
    fn collect_rail_sprites_uses_sloped_track_when_inclined_foundation() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_X, openttdrs_core::SLOPE_SW, false, &mut out);
        assert_eq!(out, vec![1033]);
        assert_ne!(
            openttdrs_core::rail_foundation_for_trackbits(openttdrs_core::SLOPE_SW, RAIL_TB_X),
            1
        );
    }

    #[test]
    fn collect_rail_sprites_uses_flat_on_leveled_foundation_slope() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_Y, openttdrs_core::SLOPE_NE, false, &mut out);
        assert_eq!(out, vec![1011]);
    }

    #[test]
    fn collect_rail_sprites_sloped_snow_adds_offset() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_X, openttdrs_core::SLOPE_SW, true, &mut out);
        assert_eq!(out, vec![1033 + RAIL_SPRITE_SNOW_OFFSET]);
    }

    #[test]
    fn collect_rail_sprites_horz_vert_flat_and_sloped() {
        let mut out = Vec::new();
        collect_rail_sprites(RAIL_TB_HORZ, 0, false, &mut out);
        assert_eq!(out, vec![1035]);
        collect_rail_sprites(RAIL_TB_VERT, 0, false, &mut out);
        assert_eq!(out, vec![1036]);
        collect_rail_sprites(RAIL_TB_HORZ, 12, false, &mut out);
        assert_eq!(out, vec![1031]);
        collect_rail_sprites(RAIL_TB_VERT, 6, false, &mut out);
        assert_eq!(out, vec![1032]);
        collect_rail_sprites(RAIL_TB_HORZ, 3, true, &mut out);
        assert_eq!(out, vec![1033 + RAIL_SPRITE_SNOW_OFFSET]);
    }

    #[test]
    fn collect_rail_sprites_sloped_junction_uses_sloped_base_only() {
        let mut out = Vec::new();
        collect_rail_sprites(0x07, 12, false, &mut out);
        assert_eq!(out, vec![1031]);
        collect_rail_sprites(RAIL_TB_CROSS, 6, false, &mut out);
        assert_eq!(out, vec![1032]);
        collect_rail_sprites(RAIL_TB_CROSS, 12, true, &mut out);
        assert_eq!(out, vec![1031 + RAIL_SPRITE_SNOW_OFFSET]);
    }

    const SP3_VISUAL_CHECKLIST: &[u8] =
        include_bytes!("../../../openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap");

    /// Regresión SP3: filas y=11/13/15 del checklist (recta/T/cruce en pendiente).
    #[test]
    fn sp3_visual_checklist_sloped_junction_sprite_ids() {
        use openttdrs_core::{Map, TileCoord, TileKind, tile_slope_and_z};

        let map = Map::from_ottd_binary(SP3_VISUAL_CHECKLIST).expect("checklist MAP1");
        let mut out = Vec::new();

        // y=11: recta Y en NE (cimiento nivelado → plano) + cruces en SE/SW/NW.
        let cases_y11: &[(i32, u8, &[u32])] = &[
            (9, 0x02, &[1011]),
            (12, 0x03, &[1032]),
            (15, 0x03, &[1033]),
            (18, 0x03, &[1034]),
        ];
        for &(x, m5, expect) in cases_y11 {
            let t = map.get(TileCoord::new(x, 11)).expect("tile y=11");
            assert_eq!(t.kind, TileKind::Rail);
            assert_eq!(t.m5 & 0x3F, m5);
            let tileh = tile_slope_and_z(&map, TileCoord::new(x, 11))
                .map(|(h, _)| h)
                .expect("slope");
            collect_rail_sprites(t.m5 & 0x3F, tileh, false, &mut out);
            assert_eq!(out, expect, "y=11 x={x} tileh={tileh}");
        }

        // y=13: T (0x07) en NE/SE/SW/NW → sprite inclinado único.
        for (x, expect) in [(1, 1031u32), (4, 1032), (7, 1033), (10, 1034)] {
            let t = map.get(TileCoord::new(x, 13)).expect("tile y=13");
            assert_eq!(t.m5 & 0x3F, 0x07);
            let tileh = tile_slope_and_z(&map, TileCoord::new(x, 13))
                .map(|(h, _)| h)
                .expect("slope");
            collect_rail_sprites(t.m5 & 0x3F, tileh, false, &mut out);
            assert_eq!(out, vec![expect], "y=13 x={x} tileh={tileh}");
        }

        // y=15: cruce X|Y en las 4 pendientes.
        for (x, expect) in [(1, 1031u32), (4, 1032), (7, 1033), (10, 1034)] {
            let t = map.get(TileCoord::new(x, 15)).expect("tile y=15");
            assert_eq!(t.m5 & 0x3F, RAIL_TB_CROSS);
            let tileh = tile_slope_and_z(&map, TileCoord::new(x, 15))
                .map(|(h, _)| h)
                .expect("slope");
            collect_rail_sprites(t.m5 & 0x3F, tileh, false, &mut out);
            assert_eq!(out, vec![expect], "y=15 x={x} tileh={tileh}");
        }
    }

    const SP3_SLOPE_LAB: &[u8] =
        include_bytes!("../../../openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap");

    #[test]
    fn sp3_slope_lab_horz_vert_from_fixture_map() {
        use openttdrs_core::{Map, TileCoord, TileKind, tile_slope_and_z};

        let map = Map::from_ottd_binary(SP3_SLOPE_LAB).expect("sp3_slope_lab MAP1");

        let horz_flat = map.get(TileCoord::new(13, 1)).expect("(13,1)");
        assert_eq!(horz_flat.kind, TileKind::Rail);
        assert_eq!(horz_flat.m5 & 0x3F, RAIL_TB_HORZ);
        let mut out = Vec::new();
        collect_rail_sprites(horz_flat.m5 & 0x3F, 0, false, &mut out);
        assert_eq!(out, vec![1035]);

        let vert_flat = map.get(TileCoord::new(15, 1)).expect("(15,1)");
        assert_eq!(vert_flat.m5 & 0x3F, RAIL_TB_VERT);
        collect_rail_sprites(vert_flat.m5 & 0x3F, 0, false, &mut out);
        assert_eq!(out, vec![1036]);

        let horz_slope = map.get(TileCoord::new(1, 16)).expect("(1,16)");
        assert_eq!(horz_slope.m5 & 0x3F, RAIL_TB_HORZ);
        let tileh = tile_slope_and_z(&map, TileCoord::new(1, 16))
            .map(|(h, _)| h)
            .expect("slope");
        collect_rail_sprites(horz_slope.m5 & 0x3F, tileh, false, &mut out);
        assert_eq!(out, vec![1031]);

        let vert_slope = map.get(TileCoord::new(1, 18)).expect("(1,18)");
        assert_eq!(vert_slope.m5 & 0x3F, RAIL_TB_VERT);
        let tileh = tile_slope_and_z(&map, TileCoord::new(1, 18))
            .map(|(h, _)| h)
            .expect("slope");
        collect_rail_sprites(vert_slope.m5 & 0x3F, tileh, false, &mut out);
        assert_eq!(out, vec![1031]);
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
    fn golden_rail_signal_sprite_texture_ids() {
        // Paridad con `crates/openttdrs-core/tests/fixtures/parity/rail_signals_golden.json`
        // (TRACK_X, cara NE, eléctrica, verde).
        const ROWS: &[(u8, u8, u8, u8, u32, &str)] = &[
            (8, 64, 64, 65, 1278, "block"),
            (9, 64, 64, 65, 1435, "entry"),
            (10, 64, 64, 65, 1451, "exit"),
            (11, 64, 64, 65, 1467, "combo"),
            (12, 64, 64, 65, 1547, "path"),
            (13, 64, 64, 65, 1563, "path_oneway"),
        ];
        for &(m2, m3, m3hi, m5, tex_id, label) in ROWS {
            let ids = collect_signal_sprite_ids(m2, m3, m3hi, m5);
            assert_eq!(ids.len(), 1, "{label}");
            assert_eq!(ids[0], tex_id, "{label}");
        }
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
