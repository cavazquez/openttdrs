use std::sync::OnceLock;

use bevy::prelude::*;
use openttdrs_core::{
    Map, TileCoord, TileKind, diag_dir_offset, rail_type_from_tile, tile_slope_and_z,
};

pub use openttdrs_core::{
    RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER,
    RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_DEPOT, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS,
    rail_tile_is_signals,
};

use super::road::RoadDepotLayerGfx;
use super::transparency::catenary_hidden;
use crate::config;
use crate::iso::remap_tile_offset;

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

/// Base de cables de catenaria Action5 (`WSO_*` 0..23 → `rail_1039..1062.png`).
pub const WIRE_SPRITE_BASE: u32 = 1039;
/// Último sprite de wire plano/inclinado en el set OpenGFX extraído.
pub const WIRE_SPRITE_LAST: u32 = 1062;
/// IDs virtuales de entrada de túnel (`WSO_ENTRANCE_*` → `rail_catenary_entrance_*.png`).
pub const CATENARY_ENTRANCE_SPRITE_BASE: u32 = 910_063;
/// IDs virtuales de postes PPP (`PSO_*` → `rail_pylon_*.png`).
pub const PYLON_SPRITE_BASE: u32 = 910_067;

/// `WireSpriteOffset` — `elrail_data.h`.
const WSO_X_SHORT: u32 = 0;
const WSO_Y_SHORT: u32 = 1;
const WSO_EW_SHORT: u32 = 2;
const WSO_NS_SHORT: u32 = 3;
const WSO_X_SHORT_DOWN: u32 = 4;
const WSO_Y_SHORT_UP: u32 = 5;
const WSO_X_SHORT_UP: u32 = 6;
const WSO_Y_SHORT_DOWN: u32 = 7;
const WSO_X_SW: u32 = 8;
const WSO_Y_SE: u32 = 9;
const WSO_X_SW_DOWN: u32 = 12;
const WSO_Y_SE_UP: u32 = 13;
const WSO_X_SW_UP: u32 = 14;
const WSO_Y_SE_DOWN: u32 = 15;
const WSO_X_NE: u32 = 16;
const WSO_Y_NW: u32 = 17;
const WSO_X_NE_DOWN: u32 = 20;
const WSO_Y_NW_UP: u32 = 21;
const WSO_X_NE_UP: u32 = 22;
const WSO_Y_NW_DOWN: u32 = 23;
const WSO_ENTRANCE_SW: u32 = 24;
const WSO_ENTRANCE_NW: u32 = 25;
const WSO_ENTRANCE_NE: u32 = 26;
const WSO_ENTRANCE_SE: u32 = 27;

/// `Direction` OpenTTD: N=0 … NW=7.
const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_E: u8 = 2;
const DIR_SE: u8 = 3;
const DIR_S: u8 = 4;
const DIR_SW: u8 = 5;
const DIR_W: u8 = 6;
const DIR_NW: u8 = 7;

/// `_pylon_sprites[DIR_*]` → offset PSO.
const PYLON_SPRITES: [u8; 8] = [4, 0, 7, 3, 5, 1, 6, 2]; // EW_N,Y_NE,NS_E,X_SE,EW_S,Y_SW,NS_W,X_NW
const X_PCP_OFF: [i8; 4] = [0, 8, 16, 8];
const Y_PCP_OFF: [i8; 4] = [8, 16, 8, 0];
const X_PPP_OFF: [i8; 8] = [-2, -4, -2, 0, 2, 4, 2, 0];
const Y_PPP_OFF: [i8; 8] = [-2, 0, 2, 4, 2, 0, -2, -4];

/// `_allowed_ppp_on_pcp` (bits DIR_*).
const ALLOWED_PPP: [u8; 4] = [
    (1 << DIR_N) | (1 << DIR_E) | (1 << DIR_SE) | (1 << DIR_S) | (1 << DIR_W) | (1 << DIR_NW),
    (1 << DIR_N) | (1 << DIR_NE) | (1 << DIR_E) | (1 << DIR_S) | (1 << DIR_SW) | (1 << DIR_W),
    (1 << DIR_N) | (1 << DIR_E) | (1 << DIR_SE) | (1 << DIR_S) | (1 << DIR_W) | (1 << DIR_NW),
    (1 << DIR_N) | (1 << DIR_NE) | (1 << DIR_E) | (1 << DIR_S) | (1 << DIR_SW) | (1 << DIR_W),
];

/// `_owned_ppp_on_pcp` (bits DIR_*).
const OWNED_PPP: [u8; 4] = [
    (1 << DIR_SE) | (1 << DIR_S) | (1 << DIR_SW) | (1 << DIR_W),
    (1 << DIR_N) | (1 << DIR_SW) | (1 << DIR_W) | (1 << DIR_NW),
    (1 << DIR_N) | (1 << DIR_NE) | (1 << DIR_E) | (1 << DIR_NW),
    (1 << DIR_NE) | (1 << DIR_E) | (1 << DIR_SE) | (1 << DIR_S),
];

/// `_ppp_order[pcp][tlg][0..8]`.
const PPP_ORDER: [[[u8; 8]; 4]; 4] = [
    [
        [DIR_NE, DIR_NW, DIR_SE, DIR_SW, DIR_N, DIR_E, DIR_S, DIR_W],
        [DIR_NE, DIR_SE, DIR_SW, DIR_NW, DIR_S, DIR_W, DIR_N, DIR_E],
        [DIR_SW, DIR_NW, DIR_NE, DIR_SE, DIR_S, DIR_W, DIR_N, DIR_E],
        [DIR_SW, DIR_SE, DIR_NE, DIR_NW, DIR_N, DIR_E, DIR_S, DIR_W],
    ],
    [
        [DIR_NE, DIR_NW, DIR_SE, DIR_SW, DIR_S, DIR_E, DIR_N, DIR_W],
        [DIR_NE, DIR_SE, DIR_SW, DIR_NW, DIR_N, DIR_W, DIR_S, DIR_E],
        [DIR_SW, DIR_NW, DIR_NE, DIR_SE, DIR_N, DIR_W, DIR_S, DIR_E],
        [DIR_SW, DIR_SE, DIR_NE, DIR_NW, DIR_S, DIR_E, DIR_N, DIR_W],
    ],
    [
        [DIR_NE, DIR_NW, DIR_SE, DIR_SW, DIR_S, DIR_W, DIR_N, DIR_E],
        [DIR_NE, DIR_SE, DIR_SW, DIR_NW, DIR_N, DIR_E, DIR_S, DIR_W],
        [DIR_SW, DIR_NW, DIR_NE, DIR_SE, DIR_N, DIR_E, DIR_S, DIR_W],
        [DIR_SW, DIR_SE, DIR_NE, DIR_NW, DIR_S, DIR_W, DIR_N, DIR_E],
    ],
    [
        [DIR_NE, DIR_NW, DIR_SE, DIR_SW, DIR_N, DIR_W, DIR_S, DIR_E],
        [DIR_NE, DIR_SE, DIR_SW, DIR_NW, DIR_S, DIR_E, DIR_N, DIR_W],
        [DIR_SW, DIR_NW, DIR_NE, DIR_SE, DIR_S, DIR_E, DIR_N, DIR_W],
        [DIR_SW, DIR_SE, DIR_NE, DIR_NW, DIR_N, DIR_W, DIR_S, DIR_E],
    ],
];

/// `DiagDirection`: NE=0, SE=1, SW=2, NW=3.
const DIAGDIR_NE: u8 = 0;
const DIAGDIR_SE: u8 = 1;
const DIAGDIR_SW: u8 = 2;
const DIAGDIR_NW: u8 = 3;

/// `_pcp_positions[TRACK_*]` — extremos PCP de cada track bit (`elrail_data.h`).
const PCP_POS: [[u8; 2]; 6] = [
    [DIAGDIR_NE, DIAGDIR_SW], // X
    [DIAGDIR_SE, DIAGDIR_NW], // Y
    [DIAGDIR_NW, DIAGDIR_NE], // UPPER
    [DIAGDIR_SE, DIAGDIR_SW], // LOWER
    [DIAGDIR_SW, DIAGDIR_NW], // LEFT
    [DIAGDIR_NE, DIAGDIR_SE], // RIGHT
];

/// Track bits que pueden encontrarse en cada borde PCP (`_tracks_at_pcp`).
const TRACKS_AT_PCP: [[u8; 6]; 4] = [
    [
        RAIL_TB_X,
        RAIL_TB_X,
        RAIL_TB_UPPER,
        RAIL_TB_LOWER,
        RAIL_TB_LEFT,
        RAIL_TB_RIGHT,
    ],
    [
        RAIL_TB_Y,
        RAIL_TB_Y,
        RAIL_TB_UPPER,
        RAIL_TB_LOWER,
        RAIL_TB_LEFT,
        RAIL_TB_RIGHT,
    ],
    [
        RAIL_TB_X,
        RAIL_TB_X,
        RAIL_TB_UPPER,
        RAIL_TB_LOWER,
        RAIL_TB_LEFT,
        RAIL_TB_RIGHT,
    ],
    [
        RAIL_TB_Y,
        RAIL_TB_Y,
        RAIL_TB_UPPER,
        RAIL_TB_LOWER,
        RAIL_TB_LEFT,
        RAIL_TB_RIGHT,
    ],
];

/// `TileSource::Home` (false) / `Neighbour` (true) para cada entrada de `TRACKS_AT_PCP`.
const TRACK_SOURCE_NEIGHBOUR: [[bool; 6]; 4] = [
    [false, true, false, true, true, false],
    [false, true, true, false, true, false],
    [false, true, true, false, false, true],
    [false, true, false, true, false, true],
];

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

/// ¿Hay PNG tipado (mono/maglev) para este ID de vía?
///
/// Incluye planos y pendientes (`1023..=1034` → `mono_track_*` / `mglv_track_*`).
/// Nieve plana `1037`/`1038` no tiene set tipado en el atlas → se deja clásica.
#[must_use]
pub fn rail_sprite_has_typed_asset(id: u32) -> bool {
    matches!(
        id,
        1005..=1012 | 1018..=1035 // overlays, Y/X, pendientes, junction, HORZ
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
        CATENARY_ENTRANCE_SPRITE_BASE..=910_066 => Some(format!(
            "rail_catenary_entrance_{}.png",
            id - CATENARY_ENTRANCE_SPRITE_BASE
        )),
        PYLON_SPRITE_BASE..=910_074 => Some(format!("rail_pylon_{}.png", id - PYLON_SPRITE_BASE)),
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

/// IDs de postes PPP + entradas de túnel para preload.
pub fn catenary_pylon_sprite_ids() -> impl Iterator<Item = u32> {
    (0..8)
        .map(|i| PYLON_SPRITE_BASE + i)
        .chain((0..4).map(|i| CATENARY_ENTRANCE_SPRITE_BASE + i))
}

/// Dibujo de un sprite de catenaria con offset sub-tesela (PCP+PPP).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatenarySpriteDraw {
    pub sprite_id: u32,
    /// Offset en coords de tesela OpenTTD (0..16), antes de `remap_tile_offset`.
    pub tile_dx: f32,
    pub tile_dy: f32,
    pub z_layer: f32,
}

/// Selector de pendiente para `_rail_wires` (`elrail.cpp`):
/// `!(tileh % 3) * tileh / 3` → 0 plano, 1=`SLOPE_SW`(3), 2=`SE`(6), 3=`NW`(9), 4=`NE`(12).
#[must_use]
pub fn catenary_tileh_selector(tileh: u8) -> u8 {
    let th = tileh & 0x0F;
    if th.is_multiple_of(3) { th / 3 } else { 0 }
}

/// Grupo de paridad de tesela (`GetTileLocationGroup`): `(x&1)<<1 | (y&1)`.
#[must_use]
pub fn catenary_tile_location_group(tx: i32, ty: i32) -> u8 {
    (((tx & 1) as u8) << 1) | ((ty & 1) as u8)
}

/// Cables de catenaria sin mapa (tests / fallback): PCP por paridad `(tx+ty)&1`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn collect_catenary_sprites(tb: u8, tileh: u8, tx: i32, ty: i32, out: &mut Vec<u32>) {
    let pcp = catenary_pcp_from_parity(tb, tx, ty);
    collect_catenary_sprites_with_pcp(tb, tileh, pcp, out);
}

/// Cables de catenaria con PCP real por vecinos (`DrawRailCatenaryRailway`).
#[allow(clippy::too_many_arguments)]
pub fn collect_catenary_sprites_from_map(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    tb: u8,
    tileh: u8,
    out: &mut Vec<u32>,
) {
    if catenary_hidden() {
        out.clear();
        return;
    }
    if map.get(pos).is_some_and(|tile| {
        tile.kind == TileKind::Station && !openttdrs_core::station_tile_can_have_wires(tile.m3)
    }) {
        out.clear();
        return;
    }
    let home_tb = electrified_trackbits_at(map, pos, mw, mh, mp_rail);
    let wire_tb = if home_tb != 0 { home_tb } else { tb & 0x3F };
    let pcp = compute_catenary_pcp_status(map, pos, mw, mh, mp_rail, wire_tb, tileh);
    collect_catenary_sprites_with_pcp(wire_tb, tileh, pcp, out);
}

/// Postes PPP sueltos (`DrawRailCatenaryRailway` pylon loop).
#[allow(clippy::too_many_arguments)]
pub fn collect_catenary_pylons_from_map(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    tb: u8,
    tileh: u8,
    out: &mut Vec<CatenarySpriteDraw>,
) {
    out.clear();
    if catenary_hidden() {
        return;
    }
    if map.get(pos).is_some_and(|tile| {
        tile.kind == TileKind::Station && !openttdrs_core::station_tile_can_have_pylons(tile.m3)
    }) {
        return;
    }
    let home_tb = electrified_trackbits_at(map, pos, mw, mh, mp_rail);
    let wire_tb = if home_tb != 0 { home_tb } else { tb & 0x3F };
    if wire_tb == 0 {
        return;
    }
    let edges = compute_catenary_edge_state(map, pos, mw, mh, mp_rail, wire_tb, tileh);
    let tlg = catenary_tile_location_group(pos.x, pos.y);
    for dir in 0..4u8 {
        if edges.pcp & (1 << dir) == 0 {
            continue;
        }
        let mut allowed = edges.allowed[dir as usize];
        let preferred = edges.preferred[dir as usize];
        if allowed & preferred != 0 {
            allowed &= preferred;
        }
        if allowed == 0 {
            continue;
        }
        let order = &PPP_ORDER[dir as usize][tlg as usize];
        for &ppp in order {
            if allowed & (1 << ppp) == 0 {
                continue;
            }
            if OWNED_PPP[dir as usize] & (1 << ppp) == 0 {
                // PPP en el borde: lo dibuja el vecino si tiene vía.
                let (dx, dy) = diag_dir_offset(dir);
                let npos = TileCoord::new(pos.x + dx, pos.y + dy);
                if electrified_trackbits_at(map, npos, mw, mh, mp_rail) != 0 {
                    break;
                }
                continue;
            }
            let tile_dx = f32::from(X_PCP_OFF[dir as usize] + X_PPP_OFF[ppp as usize]);
            let tile_dy = f32::from(Y_PCP_OFF[dir as usize] + Y_PPP_OFF[ppp as usize]);
            out.push(CatenarySpriteDraw {
                sprite_id: PYLON_SPRITE_BASE + u32::from(PYLON_SPRITES[ppp as usize]),
                tile_dx,
                tile_dy,
                z_layer: 0.036,
            });
            break;
        }
    }
}

/// Wire de portal de túnel (`DrawRailCatenaryOnTunnel`).
/// `dir` = `DiagDirection` de la boca (NE=0..NW=3).
#[must_use]
pub fn catenary_tunnel_wire_sprite(dir: u8) -> u32 {
    // Upstream: NE→ENTRANCE_SW, SE→NW, SW→NE, NW→SE.
    let wso = match dir & 3 {
        DIAGDIR_NE => WSO_ENTRANCE_SW,
        DIAGDIR_SE => WSO_ENTRANCE_NW,
        DIAGDIR_SW => WSO_ENTRANCE_NE,
        _ => WSO_ENTRANCE_SE,
    };
    CATENARY_ENTRANCE_SPRITE_BASE + (wso - WSO_ENTRANCE_SW)
}

/// Catenaria en vano de puente (`DrawRailCatenaryOnBridge` simplificado).
///
/// `axis_x`: eje X del puente; `num`: índice 1-based desde el extremo norte;
/// `length`: longitud del vano (teselas entre rampas, sin contar rampas).
pub fn collect_catenary_bridge_draws(
    axis_x: bool,
    num: u32,
    length: u32,
    tlg: u8,
    out: &mut Vec<CatenarySpriteDraw>,
) {
    out.clear();
    if catenary_hidden() || length == 0 || num == 0 {
        return;
    }
    let wire_wso = if length % 2 == 1 && num == length {
        if axis_x { WSO_X_SHORT } else { WSO_Y_SHORT }
    } else {
        // SW/NE o SE/NW según paridad de num (un poste cada dos teselas).
        let alt = num % 2 == 1;
        if axis_x {
            if alt { WSO_X_SW } else { WSO_X_NE }
        } else if alt {
            WSO_Y_SE
        } else {
            WSO_Y_NW
        }
    };
    out.push(CatenarySpriteDraw {
        sprite_id: WIRE_SPRITE_BASE + wire_wso,
        tile_dx: 8.0,
        tile_dy: 8.0,
        z_layer: 0.09,
    });
    // Poste en extremo norte cada 2 teselas.
    if num % 2 == 1 {
        let pcp = if axis_x { DIAGDIR_NE } else { DIAGDIR_NW };
        let mut ppp = if axis_x { DIR_NW } else { DIR_NE };
        let bit = if axis_x { 0 } else { 1 };
        if tlg & (1 << bit) != 0 {
            ppp = reverse_dir(ppp);
        }
        out.push(CatenarySpriteDraw {
            sprite_id: PYLON_SPRITE_BASE + u32::from(PYLON_SPRITES[ppp as usize]),
            tile_dx: f32::from(X_PCP_OFF[pcp as usize] + X_PPP_OFF[ppp as usize]),
            tile_dy: f32::from(Y_PCP_OFF[pcp as usize] + Y_PPP_OFF[ppp as usize]),
            z_layer: 0.091,
        });
    }
    // Poste en extremo sur del último vano.
    if num == length {
        let pcp = if axis_x { DIAGDIR_SW } else { DIAGDIR_SE };
        let mut ppp = if axis_x { DIR_NW } else { DIR_NE };
        let bit = if axis_x { 0 } else { 1 };
        if tlg & (1 << bit) != 0 {
            ppp = reverse_dir(ppp);
        }
        out.push(CatenarySpriteDraw {
            sprite_id: PYLON_SPRITE_BASE + u32::from(PYLON_SPRITES[ppp as usize]),
            tile_dx: f32::from(X_PCP_OFF[pcp as usize] + X_PPP_OFF[ppp as usize]),
            tile_dy: f32::from(Y_PCP_OFF[pcp as usize] + Y_PPP_OFF[ppp as usize]),
            z_layer: 0.091,
        });
    }
}

#[inline]
fn reverse_dir(dir: u8) -> u8 {
    dir ^ 4
}

/// Estado por borde para wires + postes.
struct CatenaryEdgeState {
    pcp: u8,
    preferred: [u8; 4],
    allowed: [u8; 4],
}

/// Máscara de 4 bits: bit `d` = PCP activo en `DiagDirection` d.
fn collect_catenary_sprites_with_pcp(tb: u8, tileh: u8, pcp: u8, out: &mut Vec<u32>) {
    out.clear();
    let t = tb & 0x3F;
    if t == 0 {
        return;
    }
    let foundation = openttdrs_core::rail_foundation_for_trackbits(tileh, t);
    let effective_tileh = if tileh == 0 || foundation == 1 {
        0
    } else {
        tileh
    };
    let sel = catenary_tileh_selector(effective_tileh);

    for track_idx in 0..6u8 {
        let bit = 1u8 << track_idx;
        if t & bit == 0 {
            continue;
        }
        let mut cfg = pcp_config_for_track(pcp, track_idx);
        if cfg == 0 {
            // Sin PCP en ningún extremo: forzar ambos (sprites BOTH / SHORT).
            cfg = 3;
        }
        if let Some(wso) = rail_wire_wso(sel, track_idx, cfg) {
            let sid = WIRE_SPRITE_BASE + wso;
            // MVP sin offsets por track: evitar duplicar el mismo WSO (HORZ/VERT).
            if !out.contains(&sid) {
                out.push(sid);
            }
        }
    }
}

fn pcp_config_for_track(pcp: u8, track_idx: u8) -> u8 {
    let ends = PCP_POS[track_idx as usize];
    let e0 = u8::from(pcp & (1 << ends[0]) != 0);
    let e1 = u8::from(pcp & (1 << ends[1]) != 0);
    e0 + (e1 << 1)
}

/// `_rail_wires[sel][track][pcp_config]` → offset WSO OpenGFX.
fn rail_wire_wso(sel: u8, track_idx: u8, pcp_config: u8) -> Option<u32> {
    // pcp_config: 1=end0, 2=end1, 3=both (0 inválido).
    let cfg = pcp_config.min(3) as usize;
    if cfg == 0 {
        return None;
    }
    match (sel, track_idx) {
        // TRACK_X
        (0, 0) => Some([0, WSO_X_NE, WSO_X_SW, WSO_X_SHORT][cfg]),
        (1, 0) => Some([0, WSO_X_NE_UP, WSO_X_SW_UP, WSO_X_SHORT_UP][cfg]),
        (4, 0) => Some([0, WSO_X_NE_DOWN, WSO_X_SW_DOWN, WSO_X_SHORT_DOWN][cfg]),
        // TRACK_Y
        (0, 1) => Some([0, WSO_Y_SE, WSO_Y_NW, WSO_Y_SHORT][cfg]),
        (2, 1) => Some([0, WSO_Y_SE_UP, WSO_Y_NW_UP, WSO_Y_SHORT_UP][cfg]),
        (3, 1) => Some([0, WSO_Y_SE_DOWN, WSO_Y_NW_DOWN, WSO_Y_SHORT_DOWN][cfg]),
        // UPPER / LOWER / LEFT / RIGHT — solo plano; MVP usa SHORT (ambos).
        (0, 2) | (0, 3) => Some(WSO_EW_SHORT),
        (0, 4) | (0, 5) => Some(WSO_NS_SHORT),
        _ => None,
    }
}

/// PCP por paridad (fallback sin vecinos): en recta X/Y un extremo según `(tx+ty)&1`.
#[cfg_attr(not(test), allow(dead_code))]
fn catenary_pcp_from_parity(tb: u8, tx: i32, ty: i32) -> u8 {
    let t = tb & 0x3F;
    let alt = ((tx + ty) & 1) == 0;
    let mut pcp = 0u8;
    if t & RAIL_TB_X != 0 {
        // end0=NE, end1=SW → alt: SW (2), else NE (1)
        pcp |= if alt {
            1 << DIAGDIR_SW
        } else {
            1 << DIAGDIR_NE
        };
    }
    if t & RAIL_TB_Y != 0 {
        // end0=SE, end1=NW → alt: SE (1), else NW (2)
        pcp |= if alt {
            1 << DIAGDIR_SE
        } else {
            1 << DIAGDIR_NW
        };
    }
    if t & (RAIL_TB_UPPER | RAIL_TB_LOWER | RAIL_TB_LEFT | RAIL_TB_RIGHT) != 0 {
        // Empalmes: ambos extremos de cada bit presente.
        for track_idx in 2..6u8 {
            if t & (1 << track_idx) != 0 {
                let ends = PCP_POS[track_idx as usize];
                pcp |= 1 << ends[0];
                pcp |= 1 << ends[1];
            }
        }
    }
    if pcp == 0 && t != 0 {
        pcp = 0b1111;
    }
    pcp
}

/// Track bits electrificados en una tesela (`GetRailTrackBitsUniversal` MVP: vía normal/señales).
fn electrified_trackbits_at(map: &Map, pos: TileCoord, mw: u32, mh: u32, mp_rail: u8) -> u8 {
    if pos.x < 0 || pos.y < 0 || pos.x >= mw as i32 || pos.y >= mh as i32 {
        return 0;
    }
    let Some(tile) = map.get(pos) else {
        return 0;
    };
    if !rail_type_from_tile(tile).has_catenary() {
        return 0;
    }
    match tile.kind {
        TileKind::Rail => {
            effective_rail_trackbits(tile.mapt, tile.m5, tile.kind, mp_rail).unwrap_or(0) & 0x3F
        }
        TileKind::Station
            if matches!(
                openttdrs_core::stop_kind_from_m6(tile.m6),
                openttdrs_core::StopKind::RailStation | openttdrs_core::StopKind::RailWaypoint
            ) && openttdrs_core::station_tile_can_have_wires(tile.m3) =>
        {
            if tile.m5 & 1 != 0 {
                RAIL_TB_Y
            } else {
                RAIL_TB_X
            }
        }
        _ => 0,
    }
}

/// Calcula máscara PCP de 4 bits (`pcp_status` en `DrawRailCatenaryRailway`).
fn compute_catenary_pcp_status(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    home_tb: u8,
    home_tileh: u8,
) -> u8 {
    compute_catenary_edge_state(map, pos, mw, mh, mp_rail, home_tb, home_tileh).pcp
}

/// PCP + preferred/allowed PPP por borde (`DrawRailCatenaryRailway`).
fn compute_catenary_edge_state(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    home_tb: u8,
    home_tileh: u8,
) -> CatenaryEdgeState {
    let home_tb = home_tb & 0x3F;
    let mut state = CatenaryEdgeState {
        pcp: 0,
        preferred: [0; 4],
        allowed: [0; 4],
    };
    if home_tb == 0 {
        return state;
    }
    let home_flat = home_tb & (RAIL_TB_HORZ | RAIL_TB_VERT) != 0;
    let home_foundation = openttdrs_core::rail_foundation_for_trackbits(home_tileh, home_tb);
    let home_eff_h = if home_tileh == 0 || home_foundation == 1 {
        0
    } else {
        home_tileh
    };
    let tlg = catenary_tile_location_group(pos.x, pos.y);

    for dir in 0..4u8 {
        let (dx, dy) = diag_dir_offset(dir);
        let npos = TileCoord::new(pos.x + dx, pos.y + dy);
        let neigh_tb = electrified_trackbits_at(map, npos, mw, mh, mp_rail);
        let neigh_slope = tile_slope_and_z(map, npos).map(|(h, _)| h).unwrap_or(0);
        let neigh_foundation = openttdrs_core::rail_foundation_for_trackbits(neigh_slope, neigh_tb);
        let neigh_eff_h = if neigh_tb == 0 || neigh_slope == 0 || neigh_foundation == 1 {
            0
        } else {
            neigh_slope
        };
        let neigh_flat = neigh_tb & (RAIL_TB_HORZ | RAIL_TB_VERT) != 0;

        let mut preferred_mask: u8 = 0xFF;
        let mut allowed_mask = ALLOWED_PPP[dir as usize];
        let mut used = false;
        for k in 0..6 {
            let track_bit = TRACKS_AT_PCP[dir as usize][k];
            let from_neigh = TRACK_SOURCE_NEIGHBOUR[dir as usize][k];
            let src_tb = if from_neigh { neigh_tb } else { home_tb };
            let pcp_pos = if from_neigh {
                reverse_diag_dir(dir)
            } else {
                dir
            };
            // Wire presente → preferred + PCP activo.
            if src_tb & track_bit != 0 {
                used = true;
                preferred_mask &= preferred_ppp_mask(track_bit, pcp_pos);
            }
            // Track (aunque sin wire en máscara) → disallowed PPP.
            let track_src = if from_neigh {
                // Vecino: usar trackbits electrificados (MVP = wire_config ≈ track).
                neigh_tb
            } else {
                home_tb
            };
            if track_src & track_bit != 0 {
                allowed_mask &= !disallowed_ppp_mask(track_bit, pcp_pos);
            }
        }
        if !used {
            continue;
        }
        state.pcp |= 1 << dir;
        state.preferred[dir as usize] = preferred_mask;
        state.allowed[dir as usize] = allowed_mask;

        // Recta nivelada: omitir PCP cada 2 teselas (`_ignored_pcp`).
        if (home_eff_h == neigh_eff_h || (home_flat && neigh_flat))
            && is_ignored_pcp(preferred_mask, tlg, dir)
        {
            state.pcp &= !(1 << dir);
            state.preferred[dir as usize] = 0;
            state.allowed[dir as usize] = 0;
        }
    }
    state
}

/// `_disallowed_ppp_of_track_at_pcp` (bits DIR_*).
fn disallowed_ppp_mask(track_bit: u8, pcp_pos: u8) -> u8 {
    match (track_bit, pcp_pos) {
        (RAIL_TB_X, DIAGDIR_NE) | (RAIL_TB_X, DIAGDIR_SW) => (1 << DIR_SW) | (1 << DIR_NE),
        (RAIL_TB_Y, DIAGDIR_SE) | (RAIL_TB_Y, DIAGDIR_NW) => (1 << DIR_NW) | (1 << DIR_SE),
        (RAIL_TB_UPPER, DIAGDIR_NE) | (RAIL_TB_UPPER, DIAGDIR_NW) => (1 << DIR_W) | (1 << DIR_E),
        (RAIL_TB_LOWER, DIAGDIR_SE) | (RAIL_TB_LOWER, DIAGDIR_SW) => (1 << DIR_W) | (1 << DIR_E),
        (RAIL_TB_LEFT, DIAGDIR_SW) | (RAIL_TB_LEFT, DIAGDIR_NW) => (1 << DIR_S) | (1 << DIR_N),
        (RAIL_TB_RIGHT, DIAGDIR_NE) | (RAIL_TB_RIGHT, DIAGDIR_SE) => (1 << DIR_S) | (1 << DIR_N),
        _ => 0,
    }
}

/// Máscara de PPP preferidos (bits DIR_*) para ignore-group; subset de `_preferred_ppp_of_track_at_pcp`.
fn preferred_ppp_mask(track_bit: u8, pcp_pos: u8) -> u8 {
    // DIR: N=0 NE=1 E=2 SE=3 S=4 SW=5 W=6 NW=7
    const ALL: u8 = 0xFF;
    match (track_bit, pcp_pos) {
        (RAIL_TB_X, DIAGDIR_NE) => (1 << 1) | (1 << 3) | (1 << 7), // NE,SE,NW
        (RAIL_TB_X, DIAGDIR_SW) => (1 << 3) | (1 << 5) | (1 << 7), // SE,SW,NW
        (RAIL_TB_X, _) => ALL,
        (RAIL_TB_Y, DIAGDIR_SE) => (1 << 1) | (1 << 3) | (1 << 5), // NE,SE,SW
        (RAIL_TB_Y, DIAGDIR_NW) => (1 << 5) | (1 << 7) | (1 << 1), // SW,NW,NE
        (RAIL_TB_Y, _) => ALL,
        (RAIL_TB_UPPER, DIAGDIR_NE) => (1 << 2) | (1 << 0) | (1 << 4), // E,N,S
        (RAIL_TB_UPPER, DIAGDIR_NW) => (1 << 6) | (1 << 0) | (1 << 4), // W,N,S
        (RAIL_TB_UPPER, _) => ALL,
        (RAIL_TB_LOWER, DIAGDIR_SE) => (1 << 2) | (1 << 0) | (1 << 4),
        (RAIL_TB_LOWER, DIAGDIR_SW) => (1 << 6) | (1 << 0) | (1 << 4),
        (RAIL_TB_LOWER, _) => ALL,
        (RAIL_TB_LEFT, DIAGDIR_SW) => (1 << 4) | (1 << 2) | (1 << 6), // S,E,W
        (RAIL_TB_LEFT, DIAGDIR_NW) => (1 << 0) | (1 << 2) | (1 << 6), // N,E,W
        (RAIL_TB_LEFT, _) => ALL,
        (RAIL_TB_RIGHT, DIAGDIR_NE) => (1 << 0) | (1 << 2) | (1 << 6),
        (RAIL_TB_RIGHT, DIAGDIR_SE) => (1 << 4) | (1 << 2) | (1 << 6),
        (RAIL_TB_RIGHT, _) => ALL,
        _ => ALL,
    }
}

/// `_ignored_pcp` grupos 0..2: si `preferred` coincide, se omite el PCP.
fn is_ignored_pcp(preferred: u8, tlg: u8, dir: u8) -> bool {
    // Grupo 1 (X/Y): máscaras por TLG×diagdir
    const IG1: [[u8; 4]; 4] = [
        // XEVEN_YEVEN
        [0xFF, (1 << 1) | (1 << 5), (1 << 7) | (1 << 3), 0xFF],
        // XEVEN_YODD
        [0xFF, 0xFF, (1 << 7) | (1 << 3), (1 << 1) | (1 << 5)],
        // XODD_YEVEN
        [(1 << 7) | (1 << 3), (1 << 1) | (1 << 5), 0xFF, 0xFF],
        // XODD_YODD
        [(1 << 7) | (1 << 3), 0xFF, 0xFF, (1 << 1) | (1 << 5)],
    ];
    // Grupo 2 (LEFT/RIGHT): E|W
    const EW: u8 = (1 << 2) | (1 << 6);
    const IG2: [[u8; 4]; 4] = [
        [EW, 0xFF, 0xFF, EW],
        [0xFF, EW, EW, 0xFF],
        [0xFF, EW, EW, 0xFF],
        [EW, 0xFF, 0xFF, EW],
    ];
    // Grupo 3 (UPPER/LOWER): N|S
    const NS: u8 = (1 << 0) | (1 << 4);
    const IG3: [[u8; 4]; 4] = [
        [NS, NS, 0xFF, 0xFF],
        [0xFF, 0xFF, NS, NS],
        [0xFF, 0xFF, NS, NS],
        [NS, NS, 0xFF, 0xFF],
    ];
    let tlg = (tlg & 3) as usize;
    let dir = (dir & 3) as usize;
    preferred == IG1[tlg][dir] || preferred == IG2[tlg][dir] || preferred == IG3[tlg][dir]
}

#[inline]
fn reverse_diag_dir(dir: u8) -> u8 {
    dir ^ 2
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
            for id in catenary_pylon_sprite_ids() {
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
    openttdrs_core::effective_rail_trackbits(mapt, m5, kind, mp_rail)
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
        if let Some(sid) = rail_sloped_track_sprite_id(tileh, snow_ground) {
            out.push(remap_rail_sprite_id(sid, rail_type));
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
    fn collect_rail_sprites_remaps_mono_sloped_track() {
        use openttdrs_core::RailType;
        let mut out = Vec::new();
        collect_rail_sprites_for_type(
            RAIL_TB_X,
            openttdrs_core::SLOPE_SW,
            false,
            RailType::Monorail,
            &mut out,
        );
        assert!(
            out.iter().any(|&id| (1087..=1117).contains(&id)),
            "pendiente mono remapeada: {out:?}"
        );
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
        // Fallback paridad (0,0): alt=true → SW / SE (un poste).
        collect_catenary_sprites(RAIL_TB_X, 0, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_X_SW]);
        collect_catenary_sprites(RAIL_TB_Y, 0, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_Y_SE]);
        collect_catenary_sprites(RAIL_TB_HORZ, 0, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_EW_SHORT]);
        collect_catenary_sprites(RAIL_TB_VERT, 0, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_NS_SHORT]);
        collect_catenary_sprites(0, 0, 0, 0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn collect_catenary_alternates_pylon_side_on_flat() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        collect_catenary_sprites(RAIL_TB_X, 0, 0, 0, &mut a);
        collect_catenary_sprites(RAIL_TB_X, 0, 1, 0, &mut b);
        assert_eq!(a, vec![WIRE_SPRITE_BASE + WSO_X_SW]);
        assert_eq!(b, vec![WIRE_SPRITE_BASE + WSO_X_NE]);
        assert_ne!(a, b);
    }

    #[test]
    fn collect_catenary_sloped_x_and_y() {
        use openttdrs_core::{SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW};
        let mut out = Vec::new();
        assert_eq!(catenary_tileh_selector(SLOPE_SW), 1);
        assert_eq!(catenary_tileh_selector(SLOPE_SE), 2);
        assert_eq!(catenary_tileh_selector(SLOPE_NW), 3);
        assert_eq!(catenary_tileh_selector(SLOPE_NE), 4);

        // Paridad (0,0): un PCP → wire de un extremo (UP/DOWN según pendiente).
        collect_catenary_sprites(RAIL_TB_X, SLOPE_SW, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_X_SW_UP]);
        collect_catenary_sprites(RAIL_TB_X, SLOPE_NE, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_X_SW_DOWN]);
        collect_catenary_sprites(RAIL_TB_Y, SLOPE_SE, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_Y_SE_UP]);
        collect_catenary_sprites(RAIL_TB_Y, SLOPE_NW, 0, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_Y_SE_DOWN]);
        // HORZ en pendiente: sin sprite.
        collect_catenary_sprites(RAIL_TB_HORZ, SLOPE_SW, 0, 0, &mut out);
        assert!(out.is_empty());
    }

    fn electric_rail_tile(trackbits: u8) -> openttdrs_core::Tile {
        use openttdrs_core::{RailType, Tile, set_rail_type_on_tile};
        set_rail_type_on_tile(
            Tile {
                height: 1,
                kind: TileKind::Rail,
                mapt: 0x10, // MP_RAILWAY
                m5: trackbits & 0x3F,
                m1: 0,
                m6: 0,
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            },
            RailType::Electric,
        )
    }

    fn electric_station_tile(axis_y: bool, wires: bool, pylons: bool) -> openttdrs_core::Tile {
        let mut tile = electric_rail_tile(if axis_y { RAIL_TB_Y } else { RAIL_TB_X });
        tile.kind = TileKind::Station;
        tile.mapt = 0x50;
        tile.m5 = u8::from(axis_y);
        tile.m3 = (u8::from(wires) << 1) | (u8::from(pylons) << 2);
        tile
    }

    #[test]
    fn collect_catenary_from_map_isolated_x_uses_both_ends() {
        let mut map = Map::new_flat(3, 3, 1);
        let c = TileCoord::new(1, 1);
        map.set_tile(c, electric_rail_tile(RAIL_TB_X)).unwrap();
        let mut out = Vec::new();
        collect_catenary_sprites_from_map(&map, c, 3, 3, 1, RAIL_TB_X, 0, &mut out);
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_X_SHORT]);
    }

    #[test]
    fn collect_catenary_from_map_x_line_alternates_pcp_ends() {
        let mut map = Map::new_flat(4, 3, 1);
        for x in 0..4 {
            map.set_tile(TileCoord::new(x, 1), electric_rail_tile(RAIL_TB_X))
                .unwrap();
        }
        let mut a = Vec::new();
        let mut b = Vec::new();
        collect_catenary_sprites_from_map(
            &map,
            TileCoord::new(0, 1),
            4,
            3,
            1,
            RAIL_TB_X,
            0,
            &mut a,
        );
        collect_catenary_sprites_from_map(
            &map,
            TileCoord::new(1, 1),
            4,
            3,
            1,
            RAIL_TB_X,
            0,
            &mut b,
        );
        // Extremo de línea / ignore-group: un solo PCP por tesela, lados opuestos.
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);
        assert_ne!(a, b);
        assert!(
            (a[0] == WIRE_SPRITE_BASE + WSO_X_NE || a[0] == WIRE_SPRITE_BASE + WSO_X_SW)
                && (b[0] == WIRE_SPRITE_BASE + WSO_X_NE || b[0] == WIRE_SPRITE_BASE + WSO_X_SW)
        );
    }

    #[test]
    fn collect_catenary_from_map_ignores_non_electric_neighbour() {
        use openttdrs_core::{RailType, set_rail_type_on_tile};
        let mut map = Map::new_flat(3, 3, 1);
        let a = TileCoord::new(0, 1);
        let b = TileCoord::new(1, 1);
        map.set_tile(a, electric_rail_tile(RAIL_TB_X)).unwrap();
        let mut plain = electric_rail_tile(RAIL_TB_X);
        plain = set_rail_type_on_tile(plain, RailType::Rail);
        map.set_tile(b, plain).unwrap();
        let mut out = Vec::new();
        collect_catenary_sprites_from_map(&map, a, 3, 3, 1, RAIL_TB_X, 0, &mut out);
        // Vecino sin catenaria → ambos PCP del tramo eléctrico aislado.
        assert_eq!(out, vec![WIRE_SPRITE_BASE + WSO_X_SHORT]);
    }

    #[test]
    fn collect_catenary_pylons_isolated_x_places_owned_post() {
        let mut map = Map::new_flat(3, 3, 1);
        let c = TileCoord::new(1, 1);
        map.set_tile(c, electric_rail_tile(RAIL_TB_X)).unwrap();
        let mut out = Vec::new();
        collect_catenary_pylons_from_map(&map, c, 3, 3, 1, RAIL_TB_X, 0, &mut out);
        assert!(!out.is_empty());
        assert!(
            out.iter()
                .all(|d| { (PYLON_SPRITE_BASE..PYLON_SPRITE_BASE + 8).contains(&d.sprite_id) })
        );
    }

    #[test]
    fn electric_station_participates_in_wire_neighbourhood() {
        let mut map = Map::new_flat(3, 3, 1);
        let rail = TileCoord::new(0, 1);
        let station = TileCoord::new(1, 1);
        map.set_tile(rail, electric_rail_tile(RAIL_TB_X)).unwrap();
        map.set_tile(station, electric_station_tile(false, true, true))
            .unwrap();
        let mut out = Vec::new();
        collect_catenary_sprites_from_map(&map, station, 3, 3, 1, RAIL_TB_X, 0, &mut out);
        assert!(!out.is_empty());
    }

    #[test]
    fn station_roof_keeps_wires_but_suppresses_pylons() {
        let mut map = Map::new_flat(3, 3, 1);
        let station = TileCoord::new(1, 1);
        map.set_tile(station, electric_station_tile(false, true, false))
            .unwrap();
        let mut wires = Vec::new();
        let mut pylons = Vec::new();
        collect_catenary_sprites_from_map(&map, station, 3, 3, 1, RAIL_TB_X, 0, &mut wires);
        collect_catenary_pylons_from_map(&map, station, 3, 3, 1, RAIL_TB_X, 0, &mut pylons);
        assert!(!wires.is_empty());
        assert!(pylons.is_empty());
    }

    #[test]
    fn station_without_wire_flag_suppresses_catenary() {
        let mut map = Map::new_flat(3, 3, 1);
        let station = TileCoord::new(1, 1);
        map.set_tile(station, electric_station_tile(false, false, false))
            .unwrap();
        let mut wires = Vec::new();
        collect_catenary_sprites_from_map(&map, station, 3, 3, 1, RAIL_TB_X, 0, &mut wires);
        assert!(wires.is_empty());
    }

    #[test]
    fn catenary_tunnel_wire_maps_diagdir_to_entrance() {
        assert_eq!(
            catenary_tunnel_wire_sprite(DIAGDIR_NE),
            CATENARY_ENTRANCE_SPRITE_BASE
        );
        assert_eq!(
            catenary_tunnel_wire_sprite(DIAGDIR_SE),
            CATENARY_ENTRANCE_SPRITE_BASE + 1
        );
        assert_eq!(
            catenary_tunnel_wire_sprite(DIAGDIR_SW),
            CATENARY_ENTRANCE_SPRITE_BASE + 2
        );
        assert_eq!(
            catenary_tunnel_wire_sprite(DIAGDIR_NW),
            CATENARY_ENTRANCE_SPRITE_BASE + 3
        );
    }

    #[test]
    fn collect_catenary_bridge_draws_odd_span_uses_short_and_end_pylon() {
        let mut out = Vec::new();
        collect_catenary_bridge_draws(true, 3, 3, 0, &mut out);
        assert!(
            out.iter()
                .any(|d| d.sprite_id == WIRE_SPRITE_BASE + WSO_X_SHORT)
        );
        assert!(
            out.iter()
                .any(|d| { (PYLON_SPRITE_BASE..PYLON_SPRITE_BASE + 8).contains(&d.sprite_id) })
        );
    }

    #[test]
    fn rail_sprite_aliases_resolve_pylon_and_entrance() {
        assert_eq!(
            rail_sprite_named_alias(PYLON_SPRITE_BASE),
            Some("rail_pylon_0.png".into())
        );
        assert_eq!(
            rail_sprite_named_alias(CATENARY_ENTRANCE_SPRITE_BASE + 2),
            Some("rail_catenary_entrance_2.png".into())
        );
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
