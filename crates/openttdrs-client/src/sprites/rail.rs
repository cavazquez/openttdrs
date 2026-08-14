use std::sync::OnceLock;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    SignalTrack, bridge_middle_length, bridge_surface_slope_and_z, diag_dir_offset, m2_for_signal,
    partial_pixel_z, rail_bridge_other_end, rail_type_from_tile, signal_type_for_track,
    signal_variant_for_track, tile_slope_and_z,
};

pub use openttdrs_core::{
    RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER,
    RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_DEPOT, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS,
    rail_signal_present_mask, rail_signal_state_mask, rail_tile_is_signals,
};

#[path = "rail_depot_gfx_data_generated.rs"]
mod rail_depot_gfx_data_generated;

pub use rail_depot_gfx_data_generated::{RAIL_DEPOT_BUILD_LAYERS_BY_TYPE, RailDepotLayerGfx};

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
const WSO_EW_E: u32 = 10;
const WSO_NS_S: u32 = 11;
const WSO_X_SW_DOWN: u32 = 12;
const WSO_Y_SE_UP: u32 = 13;
const WSO_X_SW_UP: u32 = 14;
const WSO_Y_SE_DOWN: u32 = 15;
const WSO_X_NE: u32 = 16;
const WSO_Y_NW: u32 = 17;
const WSO_EW_W: u32 = 18;
const WSO_NS_N: u32 = 19;
const WSO_X_NE_DOWN: u32 = 20;
const WSO_Y_NW_UP: u32 = 21;
const WSO_X_NE_UP: u32 = 22;
const WSO_Y_NW_DOWN: u32 = 23;
const WSO_ENTRANCE_SW: u32 = 24;
const WSO_ENTRANCE_NW: u32 = 25;
const WSO_ENTRANCE_NE: u32 = 26;
const WSO_ENTRANCE_SE: u32 = 27;

/// Bases de los sprites Action5 que expone OpenTTD con OpenGFX por defecto.
///
/// El cliente conserva IDs locales para resolver los PNG extraídos, mientras
/// que el exportador de OpenTTD ya informa los IDs globales resueltos.
const OPENTTD_CATENARY_WIRE_BASE: u32 = 5632;
const OPENTTD_CATENARY_PYLON_BASE: u32 = 5660;

/// Convierte un ID local de catenaria al ID global de OpenTTD para trazas.
///
/// No se usa para buscar assets ni para dibujar: sólo permite comparar una
/// escena OpenGFX por defecto con `world_draw_export` sin confundir la
/// numeración local del atlas con una diferencia de selección de sprite.
#[must_use]
pub fn catenary_reference_sprite_id(sprite_id: u32) -> u32 {
    if (WIRE_SPRITE_BASE..=WIRE_SPRITE_LAST).contains(&sprite_id) {
        OPENTTD_CATENARY_WIRE_BASE + (sprite_id - WIRE_SPRITE_BASE)
    } else if (CATENARY_ENTRANCE_SPRITE_BASE..=CATENARY_ENTRANCE_SPRITE_BASE + 3)
        .contains(&sprite_id)
    {
        OPENTTD_CATENARY_WIRE_BASE + WSO_ENTRANCE_SW + (sprite_id - CATENARY_ENTRANCE_SPRITE_BASE)
    } else if (PYLON_SPRITE_BASE..=PYLON_SPRITE_BASE + 7).contains(&sprite_id) {
        OPENTTD_CATENARY_PYLON_BASE + (sprite_id - PYLON_SPRITE_BASE)
    } else {
        sprite_id
    }
}

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
    if t & RAIL_TB_LEFT != 0 {
        out.push(1010);
    }
    if t & RAIL_TB_RIGHT != 0 {
        out.push(1009);
    }
}

/// ¿Hay PNG tipado (mono/maglev) para este ID de vía?
///
/// Incluye rectas, curvas simples, cruces, pendientes y las dos variantes de
/// diagonal doble. `1095..=1099` / `1177..=1181` son precisamente las curvas
/// compuestas que antes faltaban del atlas y hacían que maglev se viera como
/// vía normal.
#[must_use]
pub fn rail_sprite_has_typed_asset(id: u32) -> bool {
    matches!(id, 1005..=1038)
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
        1083..=1086 => Some(format!("rail_roof_{}.png", id - 1079)),
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
    // Las dos últimas entradas de cada bloque son la segunda diagonal doble
    // y sus variantes de nieve. Excluirlas no cambiaba el ID seleccionado,
    // pero sí hacía que el renderer las tratara como una vía clásica y
    // activara el tinte de fallback. Es justamente el caso visible de
    // mono/maglev "convertido" en rail normal en diagonales.
    matches!(id, 1087..=1120 | 1169..=1202)
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

/// Cable de catenaria y su caja sortable exacta de OpenTTD.
///
/// El mismo `WSO_*` puede representar dos trozos de vía distintos (por
/// ejemplo, las curvas `UPPER` y `LOWER` comparten el sprite corto pero no la
/// caja). Conservar la geometría junto al selector evita convertir esa
/// ambigüedad en una falsa coincidencia del oráculo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatenaryWireDraw {
    pub sprite_id: u32,
    /// `SpriteBounds::origin` de `_rail_catenary_sprite_data`.
    pub bounds_origin: (i32, i32, i32),
    /// `SpriteBounds::extent` de `_rail_catenary_sprite_data`.
    pub bounds_extent: (i32, i32, i32),
}

/// Dibujo de un sprite de catenaria con offset sub-tesela (PCP+PPP).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatenarySpriteDraw {
    pub sprite_id: u32,
    /// Offset en coords de tesela OpenTTD (0..16), antes de `remap_tile_offset`.
    pub tile_dx: f32,
    pub tile_dy: f32,
    pub z_layer: f32,
    /// PCP que determina la elevación del poste normal. Los postes del vano
    /// de puente ya reciben la altura del tablero y no necesitan este dato.
    pub pcp_direction: Option<u8>,
}

/// Delta vertical del `AddSortableSpriteToDraw` de un cable normal.
///
/// `DrawRailCatenaryRailway` calcula el ancla con
/// `GetSlopePixelZ(x + origin.x, y + origin.y)` y la redondea a un nivel de
/// terreno completo. La caja, en cambio, conserva su `z_offset` propio.
#[must_use]
pub fn catenary_wire_world_z_delta(
    tileh: u8,
    base_z: u8,
    trackbits: u8,
    draw: CatenaryWireDraw,
) -> i32 {
    let (x, y, _) = draw.bounds_origin;
    catenary_surface_z_delta(tileh, base_z, trackbits, x, y, 8)
}

/// Delta vertical del `GetPCPElevation` de OpenTTD para un poste normal.
///
/// Los PCP están sobre el borde y la consulta se limita a `TILE_SIZE - 1`
/// antes de redondear al semiescalón de terreno, exactamente como
/// `elrail.cpp`.
#[must_use]
pub fn catenary_pylon_world_z_delta(
    tileh: u8,
    base_z: u8,
    trackbits: u8,
    pcp_direction: u8,
) -> i32 {
    let index = usize::from(pcp_direction & 3);
    let x = X_PCP_OFF[index].min(15);
    let y = Y_PCP_OFF[index].min(15);
    catenary_surface_z_delta(tileh, base_z, trackbits, i32::from(x), i32::from(y), 4)
}

/// `GetSlopePixelZ_Rail` aplicado a una coordenada local y redondeado para
/// catenaria. A diferencia del relieve crudo, la consulta de OpenTTD usa la
/// superficie posterior a `GetRailFoundation`.
fn catenary_surface_z_delta(
    tileh: u8,
    base_z: u8,
    trackbits: u8,
    x: i32,
    y: i32,
    rounding: i32,
) -> i32 {
    let (surface_tileh, surface_z_delta) =
        openttdrs_core::rail_surface_slope_and_z(tileh, trackbits);
    let base_px = i32::from(base_z) * 8;
    let slope_px = base_px
        + i32::from(surface_z_delta) * 8
        + i32::from(partial_pixel_z(x as f32, y as f32, surface_tileh));
    ((slope_px + rounding / 2) / rounding) * rounding - base_px
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
    let mut draws = Vec::new();
    collect_catenary_wire_draws_with_pcp(tb, tileh, pcp, &mut draws);
    out.clear();
    out.extend(draws.into_iter().map(|draw| draw.sprite_id));
}

/// Cables de catenaria con PCP real por vecinos (`DrawRailCatenaryRailway`).
#[cfg_attr(not(test), allow(dead_code))]
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
    let mut draws = Vec::new();
    collect_catenary_wire_draws_from_map(map, pos, mw, mh, mp_rail, tb, tileh, &mut draws);
    out.clear();
    out.extend(draws.into_iter().map(|draw| draw.sprite_id));
}

/// Igual que [`collect_catenary_sprites_from_map`], conservando la geometría
/// sortable seleccionada por `DrawRailCatenaryRailway`.
#[allow(clippy::too_many_arguments)]
pub fn collect_catenary_wire_draws_from_map(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    tb: u8,
    tileh: u8,
    out: &mut Vec<CatenaryWireDraw>,
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
    // `DrawRailCatenaryRailway` conserva dos configuraciones: los tracks
    // eléctricos físicos y los tracks que efectivamente deben llevar cable.
    // En una unión, `MaskWireBits` puede retirar sólo la rama que termina en
    // una vía no electrificada; usar una sola máscara dibujaba un cable y un
    // poste de más (Kale: 35,164).
    let home_track_tb = electrified_trackbits_at(map, pos, mw, mh, mp_rail);
    let home_track_tb = if home_track_tb != 0 {
        home_track_tb
    } else {
        tb & 0x3F
    };
    let wire_tb = mask_catenary_wire_bits(map, pos, mw, mh, mp_rail, home_track_tb);
    let effective_tileh = catenary_effective_tileh(map, pos, home_track_tb, tileh);
    let pcp = compute_catenary_pcp_status(
        map,
        pos,
        mw,
        mh,
        mp_rail,
        home_track_tb,
        wire_tb,
        effective_tileh,
    );
    collect_catenary_wire_draws_with_effective_tileh(wire_tb, effective_tileh, pcp, out);
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
    collect_catenary_pylons_from_map_with_pcp_override(
        map, pos, mw, mh, mp_rail, tb, tileh, 0, out,
    );
}

/// Igual que [`collect_catenary_pylons_from_map`], pero omite los PCP cuyo
/// bit está en `pcp_override`.
///
/// `DrawRailCatenaryRailway` marca los dos extremos del eje cuando hay un
/// puente bajo sobre la tesela. El cable se oculta bajo el tablero y esos
/// PCP no deben generar postes que atraviesen visualmente el puente.
#[allow(clippy::too_many_arguments)]
pub fn collect_catenary_pylons_from_map_with_pcp_override(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    tb: u8,
    tileh: u8,
    pcp_override: u8,
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
    let home_track_tb = electrified_trackbits_at(map, pos, mw, mh, mp_rail);
    let home_track_tb = if home_track_tb != 0 {
        home_track_tb
    } else {
        tb & 0x3F
    };
    let wire_tb = mask_catenary_wire_bits(map, pos, mw, mh, mp_rail, home_track_tb);
    if wire_tb == 0 {
        return;
    }
    let effective_tileh = catenary_effective_tileh(map, pos, home_track_tb, tileh);
    let edges = compute_catenary_edge_state(
        map,
        pos,
        mw,
        mh,
        mp_rail,
        home_track_tb,
        wire_tb,
        effective_tileh,
    );
    let bridge_pylon_override = bridge_pylon_override(map, pos);
    let tlg = catenary_tile_location_group(pos.x, pos.y);
    for dir in 0..4u8 {
        if pcp_override & (1 << dir) != 0 {
            continue;
        }
        if bridge_pylon_override == Some(dir) {
            continue;
        }
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
                pcp_direction: Some(dir),
            });
            break;
        }
    }
}

/// `GetRailTrackBitsUniversal(tile, &override_pcp)` para una cabeza de
/// puente. Cuando existe vano central, el poste que mira al vano se dibuja
/// por `DrawRailCatenaryOnBridge`, no por el algoritmo normal de la rampa.
fn bridge_pylon_override(map: &Map, pos: TileCoord) -> Option<u8> {
    let tile = map.get(pos)?;
    if tile.kind != TileKind::RailBridge || tile.m5 & 0x80 == 0 {
        return None;
    }
    let other = rail_bridge_other_end(map, pos)?;
    (bridge_middle_length(pos, other) > 0).then_some(tile.m5 & 0x03)
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

/// Borde PCP visible de una boca de túnel ferroviario.
///
/// `GetRailTrackBitsUniversal()` deja que la boca participe sólo por el
/// extremo exterior: el borde que apunta hacia el interior del túnel nunca
/// puede reclamar un PPP. El `DiagDirection` persistido en `m5` apunta hacia
/// el túnel, por lo que el extremo visible es su opuesto.
#[must_use]
pub const fn catenary_tunnel_exterior_pcp(dir: u8) -> u8 {
    (dir & 3) ^ 2
}

/// Cable especial de una salida de depósito ferroviario eléctrico.
///
/// `DrawRailCatenary` no entra por el algoritmo PCP/PPP para depósitos:
/// toma directamente `_rail_catenary_sprite_data_depot[dir]`. Conservar sus
/// bounds junto al sprite evita tratar la boca como una vía recta y colocar
/// el cable en el lado opuesto del edificio.
#[must_use]
pub fn catenary_depot_wire_draw(dir: u8) -> CatenaryWireDraw {
    let (wso, bounds_origin, bounds_extent) = match dir & 3 {
        // NE: WSO_ENTRANCE_NE, cable X.
        DIAGDIR_NE => (WSO_ENTRANCE_NE, (0, 7, 10), (15, 1, 1)),
        // SE: WSO_ENTRANCE_SE, cable Y.
        DIAGDIR_SE => (WSO_ENTRANCE_SE, (7, 0, 10), (1, 15, 1)),
        // SW: WSO_ENTRANCE_SW, cable X.
        DIAGDIR_SW => (WSO_ENTRANCE_SW, (0, 7, 10), (15, 1, 1)),
        // NW: WSO_ENTRANCE_NW, cable Y.
        _ => (WSO_ENTRANCE_NW, (7, 0, 10), (1, 15, 1)),
    };
    CatenaryWireDraw {
        sprite_id: CATENARY_ENTRANCE_SPRITE_BASE + (wso - WSO_ENTRANCE_SW),
        bounds_origin,
        bounds_extent,
    }
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
        // Port literal de `DrawRailCatenaryOnBridge`:
        // `WIRE_X_FLAT_SW + (num % 2)`. El enum interno de OpenTTD coloca
        // `*_NE/NW` en el índice impar, aunque el sprite se llame "SW/SE" en
        // el otro extremo. Invertirlo desplaza el remate blanco del cable una
        // tesela: era visible en todos los vanos de dos piezas de Kale.
        let odd = num % 2 == 1;
        if axis_x {
            if odd { WSO_X_NE } else { WSO_X_SW }
        } else if odd {
            WSO_Y_NW
        } else {
            WSO_Y_SE
        }
    };
    out.push(CatenarySpriteDraw {
        sprite_id: WIRE_SPRITE_BASE + wire_wso,
        tile_dx: 8.0,
        tile_dy: 8.0,
        z_layer: 0.09,
        pcp_direction: None,
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
            pcp_direction: Some(pcp),
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
            pcp_direction: Some(pcp),
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
fn collect_catenary_wire_draws_with_pcp(
    tb: u8,
    tileh: u8,
    pcp: u8,
    out: &mut Vec<CatenaryWireDraw>,
) {
    out.clear();
    let t = tb & 0x3F;
    if t == 0 {
        return;
    }
    let (surface_tileh, _) = openttdrs_core::rail_surface_slope_and_z(tileh, t);
    // `DrawRailCatenaryRailway` aplana las pendientes de medio bloque antes
    // de elegir wire/PCP; los demás cimientos ya quedaron aplicados arriba.
    let effective_tileh = if surface_tileh & 0x20 != 0 {
        0
    } else {
        surface_tileh
    };
    collect_catenary_wire_draws_with_effective_tileh(t, effective_tileh, pcp, out);
}

/// Variante de [`collect_catenary_wire_draws_with_pcp`] para un `tileh` que ya
/// pasó por `DrawFoundation` y, si corresponde, `AdjustTileh`. Las rampas de
/// puente no son una vía normal en pendiente: aplicar otra vez
/// `GetRailFoundation` sobre su pendiente ajustada cambiaba el wire elegido.
fn collect_catenary_wire_draws_with_effective_tileh(
    tb: u8,
    effective_tileh: u8,
    pcp: u8,
    out: &mut Vec<CatenaryWireDraw>,
) {
    out.clear();
    let t = tb & 0x3F;
    if t == 0 {
        return;
    }
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
        if let (Some(wso), Some((bounds_origin, bounds_extent))) = (
            rail_wire_wso(sel, track_idx, cfg),
            catenary_wire_trace_bounds(sel, track_idx),
        ) {
            let sid = WIRE_SPRITE_BASE + wso;
            // `DrawRailCatenaryRailway` itera cada TrackBit y emite un
            // `AddSortableSpriteToDraw` por cada uno. Dos curvas pueden
            // reutilizar el mismo PNG corto, pero tienen cajas distintas y
            // siguen siendo dos comandos: deduplicarlas quitaba cables de
            // uniones dobles de Kale (por ejemplo, 168,58: 5643 × 2).
            out.push(CatenaryWireDraw {
                sprite_id: sid,
                bounds_origin,
                bounds_extent,
            });
        }
    }
}

/// `SortableSpriteStruct` de `_rail_catenary_sprite_data`, sin depender del
/// WSO: varios índices reutilizan el mismo sprite con geometría diferente.
const fn catenary_wire_trace_bounds(
    selector: u8,
    track_idx: u8,
) -> Option<((i32, i32, i32), (i32, i32, i32))> {
    match (selector, track_idx) {
        // X: plano, ascenso SW y descenso NE.
        (0, 0) => Some(((0, 7, 10), (15, 1, 1))),
        (1, 0) => Some(((0, 7, 19), (15, 8, 1))),
        (4, 0) => Some(((0, 7, 9), (15, 8, 1))),
        // Y: plano, ascenso SE y descenso NW.
        (0, 1) => Some(((7, 0, 10), (1, 15, 1))),
        (2, 1) => Some(((7, 0, 19), (8, 15, 1))),
        (3, 1) => Some(((7, 0, 9), (8, 15, 1))),
        // Curvas ortogonales: UPPER/LOWER y LEFT/RIGHT.
        (0, 2) => Some(((7, 0, 10), (1, 1, 1))),
        (0, 3) => Some(((15, 8, 10), (3, 3, 1))),
        (0, 4) => Some(((8, 0, 10), (8, 8, 1))),
        (0, 5) => Some(((0, 8, 10), (8, 8, 1))),
        _ => None,
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
        // Curvas ortogonales. Los extremos no son intercambiables: el
        // selector de OpenTTD usa un wire distinto para cada PCP activo.
        // `_rail_wires[0][UPPER..RIGHT][cfg]` en `elrail_data.h`.
        (0, 2) => Some([0, WSO_EW_W, WSO_EW_E, WSO_EW_SHORT][cfg]),
        (0, 3) => Some([0, WSO_EW_E, WSO_EW_W, WSO_EW_SHORT][cfg]),
        (0, 4) => Some([0, WSO_NS_S, WSO_NS_N, WSO_NS_SHORT][cfg]),
        (0, 5) => Some([0, WSO_NS_N, WSO_NS_S, WSO_NS_SHORT][cfg]),
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

/// ¿La tesela puede transportar trenes y, por tanto, tiene un `RailType`?
///
/// Es el subconjunto de [`GetTileRailType`] que necesita `MaskWireBits`.
/// No basta con leer `m8`: en una tesela de terreno ese byte puede contener
/// datos no relacionados con el tipo de vía.
fn rail_type_at(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> Option<openttdrs_core::RailType> {
    if pos.x < 0 || pos.y < 0 || pos.x >= mw as i32 || pos.y >= mh as i32 {
        return None;
    }
    let tile = map.get(pos)?;
    let has_rail = match tile.kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailBridge | TileKind::RailTunnel => true,
        TileKind::Road => {
            is_road_level_crossing(tile.mapt, tile.m5, tile.kind, openttdrs_core::OTTD_MP_ROAD)
        }
        TileKind::Station => matches!(
            openttdrs_core::stop_kind_from_m6(tile.m6),
            openttdrs_core::StopKind::RailStation | openttdrs_core::StopKind::RailWaypoint
        ),
        _ => false,
    };
    has_rail.then(|| rail_type_from_tile(tile))
}

/// `TrackStatusToTrackBits(GetTileTrackStatus(..., TRANSPORT_RAIL, 0))`.
///
/// `MaskWireBits` pregunta la conectividad física incluso si la vía vecina no
/// es eléctrica. Por eso esta función no filtra por `RailType`.
fn rail_track_status_bits_at(map: &Map, pos: TileCoord, mw: u32, mh: u32, mp_rail: u8) -> u8 {
    if pos.x < 0 || pos.y < 0 || pos.x >= mw as i32 || pos.y >= mh as i32 {
        return 0;
    }
    let Some(tile) = map.get(pos) else {
        return 0;
    };
    match tile.kind {
        TileKind::Rail => {
            effective_rail_trackbits(tile.mapt, tile.m5, tile.kind, mp_rail).unwrap_or(0) & 0x3F
        }
        // `GetTileTrackStatus_Track` permite el único eje de salida del
        // depósito. `GetRailTrackBitsUniversal` no lo usa para catenaria
        // propia, pero `MaskWireBits` sí lo consulta como vecino.
        TileKind::RailDepot => {
            if tile.m5 & 1 == 0 {
                RAIL_TB_X
            } else {
                RAIL_TB_Y
            }
        }
        // Una rampa de puente es parte de la misma línea ferroviaria. Antes
        // caía en `_ => 0`, de modo que el tramo exterior cortaba su
        // catenaria justo al llegar al puente aunque ambos tiles fuesen
        // eléctricos. La dirección de la rampa define el eje: NE/SW = X,
        // SE/NW = Y (`GetTunnelBridgeDirection` / `DiagDirToAxis`).
        TileKind::RailBridge | TileKind::RailTunnel => {
            if tile.m5 & 1 == 0 {
                RAIL_TB_X
            } else {
                RAIL_TB_Y
            }
        }
        // `GetCrossingRailTrack`: el eje de la vía es el perpendicular al de
        // la carretera, codificado en el bit 0 de `m5`.
        TileKind::Road
            if is_road_level_crossing(
                tile.mapt,
                tile.m5,
                tile.kind,
                openttdrs_core::OTTD_MP_ROAD,
            ) =>
        {
            if tile.m5 & 1 == 0 {
                RAIL_TB_Y
            } else {
                RAIL_TB_X
            }
        }
        TileKind::Station
            if matches!(
                openttdrs_core::stop_kind_from_m6(tile.m6),
                openttdrs_core::StopKind::RailStation | openttdrs_core::StopKind::RailWaypoint
            ) && tile.m3 & 1 == 0 =>
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

/// Track bits electrificados en una tesela (`GetRailTrackBitsUniversal`).
fn electrified_trackbits_at(map: &Map, pos: TileCoord, mw: u32, mh: u32, mp_rail: u8) -> u8 {
    if !rail_type_at(map, pos, mw, mh).is_some_and(openttdrs_core::RailType::has_catenary) {
        return 0;
    }
    // El equivalente C++ devuelve NONE para depósitos: su cable se dibuja en
    // el camino especial de `DrawRailCatenary`, no como una vía normal.
    if map
        .get(pos)
        .is_some_and(|tile| tile.kind == TileKind::RailDepot)
    {
        return 0;
    }
    rail_track_status_bits_at(map, pos, mw, mh, mp_rail)
}

/// `IsPlainRailTile`: riel normal o con señales, pero no depósito.
fn is_plain_rail_tile(map: &Map, pos: TileCoord, mp_rail: u8) -> bool {
    map.get(pos).is_some_and(|tile| {
        tile.kind == TileKind::Rail
            && (tile.mapt >> 4) & 0x0F == mp_rail
            && matches!((tile.m5 >> 6) & 0x03, RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS)
    })
}

/// Máscaras de [`DiagdirReachesTrackdirs`] y `DiagdirReachesTracks`.
///
/// Los bits 0..5 son un sentido de cada track y 8..13 el inverso, igual que
/// `TrackdirBits` en `track_type.h`.
const DIAGDIR_REACHES_TRACKDIRS: [u16; 4] = [0x1009, 0x0016, 0x0520, 0x2A00];
const DIAGDIR_REACHES_TRACKS: [u8; 4] = [0x19, 0x16, 0x25, 0x2A];
const TRACKDIR_BIT_X_NE: u16 = 1 << 0;
const TRACKDIR_BIT_X_SW: u16 = 1 << 8;
const TRACKDIR_BIT_Y_SE: u16 = 1 << 1;
const TRACKDIR_BIT_Y_NW: u16 = 1 << 9;

#[inline]
const fn trackdir_bits_to_trackbits(bits: u16) -> u8 {
    ((bits | (bits >> 8)) & 0x003F) as u8
}

#[inline]
const fn tracks_overlap(trackbits: u8) -> bool {
    let trackbits = trackbits & 0x3F;
    trackbits != 0
        && trackbits & (trackbits - 1) != 0
        && trackbits != RAIL_TB_HORZ
        && trackbits != RAIL_TB_VERT
}

/// La excepción de `MaskWireBits` para plataformas bloqueadas: aunque no
/// ofrezcan `TrackStatus`, mantienen el cable si tienen el eje y flag correctos.
fn station_preserves_catenary_wire(map: &Map, pos: TileCoord, direction: u8) -> bool {
    map.get(pos).is_some_and(|tile| {
        tile.kind == TileKind::Station
            && matches!(
                openttdrs_core::stop_kind_from_m6(tile.m6),
                openttdrs_core::StopKind::RailStation | openttdrs_core::StopKind::RailWaypoint
            )
            && (tile.m5 & 1 != 0) == (direction & 1 != 0)
            && openttdrs_core::station_tile_can_have_wires(tile.m3)
    })
}

/// Port de `MaskWireBits` de `elrail.cpp`.
///
/// Las uniones con más de un track no deben tender cable por ramas que acaban
/// en una vía no electrificada o sin conexión. El track se conserva si
/// enmascararlo dejaría la tesela sin ningún cable, como en OpenTTD.
fn mask_catenary_wire_bits(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    trackbits: u8,
) -> u8 {
    let trackbits = trackbits & 0x3F;
    if trackbits.count_ones() <= 1 || !is_plain_rail_tile(map, pos, mp_rail) {
        return trackbits;
    }

    let mut neighbour_trackdirs = 0_u16;
    for direction in 0..4_u8 {
        let (dx, dy) = diag_dir_offset(direction);
        let neighbour = TileCoord::new(pos.x + dx, pos.y + dy);
        let electrically_powered = rail_type_at(map, neighbour, mw, mh)
            .is_some_and(openttdrs_core::RailType::has_catenary);
        let reachable = rail_track_status_bits_at(map, neighbour, mw, mh, mp_rail)
            & DIAGDIR_REACHES_TRACKS[direction as usize]
            != 0;
        if !electrically_powered
            || (!reachable && !station_preserves_catenary_wire(map, neighbour, direction))
        {
            neighbour_trackdirs |= DIAGDIR_REACHES_TRACKDIRS[reverse_diag_dir(direction) as usize];
        }
    }

    let mut mask = if trackbits == RAIL_TB_CROSS || !tracks_overlap(trackbits) {
        let mut mask =
            !trackdir_bits_to_trackbits(neighbour_trackdirs & (neighbour_trackdirs >> 8));
        if trackbits != RAIL_TB_CROSS && mask & 0x3F == 0x3F {
            mask = !trackdir_bits_to_trackbits(neighbour_trackdirs);
        }
        mask
    } else {
        let mut mask = !trackdir_bits_to_trackbits(neighbour_trackdirs);
        if trackbits & mask == 0 {
            if neighbour_trackdirs & TRACKDIR_BIT_X_NE == 0
                || neighbour_trackdirs & TRACKDIR_BIT_X_SW == 0
            {
                mask |= RAIL_TB_X;
            }
            if neighbour_trackdirs & TRACKDIR_BIT_Y_SE == 0
                || neighbour_trackdirs & TRACKDIR_BIT_Y_NW == 0
            {
                mask |= RAIL_TB_Y;
            }
            if trackbits & mask == 0 {
                mask =
                    !trackdir_bits_to_trackbits(neighbour_trackdirs & (neighbour_trackdirs >> 8));
            }
        }
        mask
    };

    // El `mask` anterior conserva el tipo de entero del C++; limitarlo deja
    // explícito que sólo hay seis tracks representables en el save.
    mask &= 0x3F;
    let masked = trackbits & mask;
    if masked != 0 { masked } else { trackbits }
}

/// Pendiente que usa `DrawRailCatenaryRailway` después de las fundaciones.
///
/// `DrawFoundation` ya modificó `TileInfo` para la tesela actual; al mirar
/// una vecina OpenTTD reproduce explícitamente esa fundación y recién después
/// llama a `AdjustTileh`. Aquí calculamos el mismo resultado desde el mapa,
/// incluyendo rampas y bocas de túnel. Sin esto una rampa vecina inclinada se
/// comparaba como vía normal y se conservaba un PCP que OpenTTD alterna.
fn catenary_effective_tileh(map: &Map, pos: TileCoord, trackbits: u8, fallback: u8) -> u8 {
    let Some(tile) = map.get(pos) else {
        return fallback;
    };
    let raw_tileh = tile_slope_and_z(map, pos).map_or(fallback, |(tileh, _)| tileh);
    match tile.kind {
        TileKind::Rail => {
            let (surface, _) = openttdrs_core::rail_surface_slope_and_z(raw_tileh, trackbits);
            // `DrawRailCatenaryRailway` aplana los medios bloques antes de
            // seleccionar wires y PCPs.
            if surface & 0x20 != 0 { 0 } else { surface }
        }
        TileKind::RailBridge => {
            let (foundation_tileh, _) = bridge_surface_slope_and_z(raw_tileh, tile.m5 & 1 == 0);
            adjust_catenary_tunnel_bridge_tileh(foundation_tileh, tile.m5, false)
        }
        // `AdjustTileh` fuerza una pendiente empinada en bocas de túnel para
        // que el algoritmo de PCP coloque el poste de entrada adecuado.
        TileKind::RailTunnel => adjust_catenary_tunnel_bridge_tileh(raw_tileh, tile.m5, true),
        // Las estaciones ferroviarias y los cruces a nivel son siempre
        // planos para la decisión de catenaria.
        TileKind::Station | TileKind::Road => 0,
        _ => raw_tileh,
    }
}

/// `AdjustTileh` de `elrail.cpp` para una tesela `TunnelBridge`.
#[inline]
fn adjust_catenary_tunnel_bridge_tileh(tileh: u8, m5: u8, is_tunnel: bool) -> u8 {
    if is_tunnel {
        openttdrs_core::SLOPE_STEEP
    } else if tileh != 0 {
        0
    } else {
        match m5 & 3 {
            0 => openttdrs_core::SLOPE_NE,
            1 => openttdrs_core::SLOPE_SE,
            2 => openttdrs_core::SLOPE_SW,
            _ => openttdrs_core::SLOPE_NW,
        }
    }
}

/// `IsBridgeTile(neighbour) && GetTunnelBridgeDirection(neighbour) ==
/// ReverseDiagDir(i)` de `DrawRailCatenaryRailway`.
///
/// El cabezal que mira hacia el vano no debe reclamar un poste: OpenTTD lo
/// dibuja desde el propio puente. Sin esta exclusión se acumulaba un poste
/// `5662` en cada rampa eléctrica de Kale, aunque el cable fuera correcto.
fn neighbour_is_far_bridge_head(map: &Map, pos: TileCoord, direction_from_home: u8) -> bool {
    map.get(pos).is_some_and(|tile| {
        tile.kind == TileKind::RailBridge
            && tile.m5 & 0x80 != 0
            && (tile.m5 & 0x03) == reverse_diag_dir(direction_from_home)
    })
}

/// Calcula máscara PCP de 4 bits (`pcp_status` en `DrawRailCatenaryRailway`).
#[allow(clippy::too_many_arguments)]
fn compute_catenary_pcp_status(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    home_track_tb: u8,
    home_wire_tb: u8,
    home_tileh: u8,
) -> u8 {
    compute_catenary_edge_state(
        map,
        pos,
        mw,
        mh,
        mp_rail,
        home_track_tb,
        home_wire_tb,
        home_tileh,
    )
    .pcp
}

/// PCP + preferred/allowed PPP por borde (`DrawRailCatenaryRailway`).
#[allow(clippy::too_many_arguments)]
fn compute_catenary_edge_state(
    map: &Map,
    pos: TileCoord,
    mw: u32,
    mh: u32,
    mp_rail: u8,
    home_track_tb: u8,
    home_wire_tb: u8,
    home_tileh: u8,
) -> CatenaryEdgeState {
    let home_track_tb = home_track_tb & 0x3F;
    let home_wire_tb = home_wire_tb & 0x3F;
    let mut state = CatenaryEdgeState {
        pcp: 0,
        preferred: [0; 4],
        allowed: [0; 4],
    };
    if home_track_tb == 0 {
        return state;
    }
    let home_flat = home_track_tb & (RAIL_TB_HORZ | RAIL_TB_VERT) != 0;
    // El caller entrega la pendiente posterior a fundación / `AdjustTileh`.
    let home_eff_h = home_tileh;
    let tlg = catenary_tile_location_group(pos.x, pos.y);

    for dir in 0..4u8 {
        let (dx, dy) = diag_dir_offset(dir);
        let npos = TileCoord::new(pos.x + dx, pos.y + dy);
        let mut neigh_track_tb = electrified_trackbits_at(map, npos, mw, mh, mp_rail);
        let mut neigh_wire_tb = mask_catenary_wire_bits(map, npos, mw, mh, mp_rail, neigh_track_tb);
        // Una boca de túnel sólo se conecta por su dirección de salida; los
        // otros tres bordes no deben participar en los PCP vecinos.
        if map
            .get(npos)
            .is_some_and(|tile| tile.kind == TileKind::RailTunnel && dir != (tile.m5 & 0x03))
        {
            neigh_track_tb = 0;
            neigh_wire_tb = 0;
        }
        // Igual que `DrawRailCatenaryRailway`: una plataforma ferroviaria
        // que no admite ni cables ni postes no participa de este borde. Las
        // plataformas bloqueadas que sí conservan cables se tratan arriba en
        // `station_preserves_catenary_wire`.
        if map.get(npos).is_some_and(|tile| {
            tile.kind == TileKind::Station
                && matches!(
                    openttdrs_core::stop_kind_from_m6(tile.m6),
                    openttdrs_core::StopKind::RailStation | openttdrs_core::StopKind::RailWaypoint
                )
                && !openttdrs_core::station_tile_can_have_pylons(tile.m3)
                && !openttdrs_core::station_tile_can_have_wires(tile.m3)
        }) {
            neigh_track_tb = 0;
            neigh_wire_tb = 0;
        }
        let neighbour_is_far_bridge = neighbour_is_far_bridge_head(map, npos, dir);
        let neigh_eff_h = catenary_effective_tileh(map, npos, neigh_track_tb, 0);
        let neigh_flat = neigh_track_tb & (RAIL_TB_HORZ | RAIL_TB_VERT) != 0;

        let mut preferred_mask: u8 = 0xFF;
        let mut allowed_mask = ALLOWED_PPP[dir as usize];
        let mut used = false;
        for k in 0..6 {
            let track_bit = TRACKS_AT_PCP[dir as usize][k];
            let from_neigh = TRACK_SOURCE_NEIGHBOUR[dir as usize][k];
            // El extremo lejano del cabezal de puente pertenece al vano;
            // `DrawRailCatenaryOnBridge` es quien coloca ese poste.
            if from_neigh && neighbour_is_far_bridge {
                continue;
            }
            let wire_src = if from_neigh {
                neigh_wire_tb
            } else {
                home_wire_tb
            };
            let pcp_pos = if from_neigh {
                reverse_diag_dir(dir)
            } else {
                dir
            };
            // Wire presente → preferred + PCP activo.
            if wire_src & track_bit != 0 {
                used = true;
                preferred_mask &= preferred_ppp_mask(track_bit, pcp_pos);
            }
            // Track (aunque sin wire en máscara) → disallowed PPP.
            let track_src = if from_neigh {
                neigh_track_tb
            } else {
                home_track_tb
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

/// Cantidad de conjuntos visuales de depósitos: rail/electric, mono, maglev.
pub const RAIL_DEPOT_VISUAL_TYPE_COUNT: usize = 3;

/// Índice de [`RAIL_DEPOT_BUILD_LAYERS_BY_TYPE`] para un tipo de vía.
///
/// Igual que `RailTypeInfo::GetRailtypeSpriteOffset()`: la vía eléctrica usa
/// las mismas capas de edificio que la vía normal; monorriel y maglev usan
/// los bloques +82 y +164 respectivamente.
#[must_use]
pub const fn rail_depot_visual_type_index(rail_type: openttdrs_core::RailType) -> usize {
    match rail_type {
        openttdrs_core::RailType::Rail | openttdrs_core::RailType::Electric => 0,
        openttdrs_core::RailType::Monorail => 1,
        openttdrs_core::RailType::Maglev => 2,
    }
}

/// Capas BUILD del depósito de vía (`_depot_gfx_NE..NW` en `track_land.h`).
///
/// El generador mantiene `dx`/`dy` de cada `TILE_SEQ_LINE` y los offsets NFO
/// sin hornearlos; `road_depot_build_sprite_center` aplica la escala de
/// `RemapCoords` de OpenTTD. Así las variantes mono/maglev conservan sus
/// dimensiones y puertas propias, en vez de caer visualmente al depósito
/// normal.
#[must_use]
pub fn rail_depot_build_layers(
    rail_type: openttdrs_core::RailType,
    dir: usize,
) -> &'static [RailDepotLayerGfx] {
    RAIL_DEPOT_BUILD_LAYERS_BY_TYPE[rail_depot_visual_type_index(rail_type)][dir.min(3)]
}

/// Convierte una capa de depósito ferroviario al contenedor de posición
/// isométrica compartido. El renderer la pasa por
/// `road_depot_build_sprite_center`, cuya escala local coincide con
/// `RemapCoords` de OpenTTD para `TILE_SEQ_LINE`.
#[must_use]
pub const fn rail_depot_seq_gfx(layer: &RailDepotLayerGfx) -> crate::iso::RoadStopSeqGfx {
    crate::iso::RoadStopSeqGfx {
        dx: layer.dx,
        dy: layer.dy,
        dz: layer.dz,
        x_offs: layer.x_offs,
        y_offs: layer.y_offs,
        remap_x_adj: 0.0,
    }
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

/// Sprite base completo de un cruce vanilla sin `RailType` overlay.
///
/// Después de elegir eje y barrera, `DrawTile_Road` aplica las cuatro
/// variantes pavimentadas o las ocho de nieve/desierto. Es una decisión del
/// roadside de la carretera, no de la vía, por lo que el renderer debe pasar
/// ambas banderas ya resueltas desde la tesela de cruce.
#[must_use]
pub fn level_crossing_ground_sprite_id_for_type(
    m5: u8,
    rail_type: openttdrs_core::RailType,
    paved: bool,
    snow_or_desert: bool,
) -> u32 {
    let mut sid = level_crossing_rail_sprite_id_for_type(m5, rail_type);
    if snow_or_desert {
        sid += 8;
    } else if paved {
        sid += 4;
    }
    sid
}

/// Nombre de atlas exclusivo para un sprite de suelo de cruce a nivel.
///
/// Los IDs lógicos 1370..=1405 comparten espacio numérico con sprites de
/// señales Action5. El renderer no puede usar el alias genérico
/// `rail_<id>.png`: según el tipo de gráfico activo ese alias puede resolver
/// una señal de 7×13 en lugar de una tesela de cruce de 64×31. Mantener este
/// namespace semántico separado conserva la decisión de `DrawTile_Road` sin
/// confundirla con la de `DrawSignal`.
#[must_use]
pub fn level_crossing_sprite_atlas_key(id: u32) -> Option<String> {
    let (family, offset) = match id {
        1370..=1381 => ("rail", id - 1370),
        1382..=1393 => ("mono", id - 1382),
        1394..=1405 => ("mglv", id - 1394),
        _ => return None,
    };
    Some(format!("crossing_{family}_{offset:02}.png"))
}

/// Reserva PBS en el cruce (bit 4 de `m5`, `HasCrossingReservation`).
#[must_use]
pub fn level_crossing_has_rail_reservation(m5: u8) -> bool {
    (m5 >> 4) & 1 != 0
}

pub use openttdrs_core::rail_tile_has_pbs_reservation;

/// Una capa PBS con el contexto de la pasada de `DrawTrackBits` que la emitió.
///
/// El renderer visual solo necesita `sprite_id`, pero la traza canónica debe
/// conocer la pista y pendiente efectiva para reproducir `DrawTrackSprite` y
/// su offset relativo a un cimiento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RailPbsReservationSpriteDraw {
    pub(crate) sprite_id: u32,
    pub(crate) track_bit: u8,
    pub(crate) sprite_tileh: u8,
    pub(crate) halftile_corner: Option<u8>,
}

/// Capas `PALETTE_CRASH` de una reserva PBS, siguiendo las mismas pasadas de
/// fundación que `DrawTrackBits` / `DrawTrackBitsOverlay`.
///
/// Sólo X/Y usan `SPR_TRACKS_FOR_SLOPES_*` sobre una pendiente. Las cuatro
/// pistas de esquina siempre usan el banco `SINGLE_N/S/E/W`; cuando hay una
/// fundación de medio bloque, la segunda pasada conserva además esa regla.
/// Esto evita convertir una reserva de una sola esquina en una vía diagonal
/// completa (el origen de varios falsos "cortes" en pendientes).
#[must_use]
pub(crate) fn collect_rail_pbs_reservation_draws(
    track_bits: u8,
    reservation_bits: u8,
    tileh: u8,
    rail_type: openttdrs_core::RailType,
) -> Vec<RailPbsReservationSpriteDraw> {
    let mut out = Vec::with_capacity(6);
    for pass in openttdrs_core::rail_track_draw_plan(tileh, track_bits & 0x3F)
        .passes
        .into_iter()
        .flatten()
    {
        let pbs = reservation_bits & pass.track_bits & 0x3F;
        for (track_bit, single_sprite) in [
            (RAIL_TB_X, 1005),
            (RAIL_TB_Y, 1006),
            (RAIL_TB_UPPER, 1007),
            (RAIL_TB_LOWER, 1008),
            (RAIL_TB_LEFT, 1010),
            (RAIL_TB_RIGHT, 1009),
        ] {
            if pbs & track_bit == 0 {
                continue;
            }

            // `DrawTrackBits` sólo sustituye X/Y por el banco inclinado. La
            // pasada alta de una fundación de medio bloque sigue usando el
            // sprite de esquina (con su recorte/subaltura correspondiente).
            let sprite = if pass.halftile_corner.is_none()
                && matches!(track_bit, RAIL_TB_X | RAIL_TB_Y)
                && !matches!(pass.sprite_tileh & 0x1F, 0 | 0x0F)
            {
                rail_pbs_sloped_sprite_id(pass.sprite_tileh, rail_type)
                    .unwrap_or_else(|| remap_rail_sprite_id(single_sprite, rail_type))
            } else {
                remap_rail_sprite_id(single_sprite, rail_type)
            };
            out.push(RailPbsReservationSpriteDraw {
                sprite_id: sprite,
                track_bit,
                sprite_tileh: pass.sprite_tileh,
                halftile_corner: pass.halftile_corner,
            });
        }
    }
    out
}

/// Adaptador de IDs para las pruebas de selección de PBS.
///
/// La ruta de producción debe consumir [`collect_rail_pbs_reservation_draws`]
/// para no perder la pendiente ni el contexto de fundación de cada capa.
#[cfg(test)]
#[must_use]
fn collect_rail_pbs_reservation_sprites(
    track_bits: u8,
    reservation_bits: u8,
    tileh: u8,
    rail_type: openttdrs_core::RailType,
) -> Vec<u32> {
    collect_rail_pbs_reservation_draws(track_bits, reservation_bits, tileh, rail_type)
        .into_iter()
        .map(|draw| draw.sprite_id)
        .collect()
}

/// `SPR_TRACKS_FOR_SLOPES_{RAIL,MONO,MAGLEV}_BASE` para una X/Y compatible
/// con la pendiente. Los demás valores de `_track_sloped_sprites` sólo se
/// alcanzan con pistas de esquina, que usan `SINGLE_*` en el overlay PBS.
fn rail_pbs_sloped_sprite_id(sprite_tileh: u8, rail_type: openttdrs_core::RailType) -> Option<u32> {
    let tileh = sprite_tileh & 0x1F;
    if !(1..=14).contains(&tileh) {
        return None;
    }
    let slope_offset = RAIL_TRACK_SLOPED_OFFSETS[(tileh - 1) as usize];
    let orientation = slope_offset.checked_sub(20)?;
    let base = match rail_type {
        openttdrs_core::RailType::Rail | openttdrs_core::RailType::Electric => 5401,
        openttdrs_core::RailType::Monorail => 5405,
        openttdrs_core::RailType::Maglev => 5409,
    };
    Some(base + u32::from(orientation))
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
/// `SPR_SIGNALS_BASE - 16` (`DrawSingleSignal`): banco Action5 tipo 04 (presignals/PBS).
/// `SPR_SIGNALS_BASE` = 4896+192 = 5088; los PNG viven en `rail_5088..rail_5327`.
const SPR_SIGNAL_ALT_BASE: u32 = 5072;

/// Bases de sprite para señales. Sobrescribibles con `OPENTTDRS_SIGNAL_BASE` /
/// `OPENTTDRS_SIGNAL_ALT_BASE` (512–8192; el banco Action5 llega a ~5327).
#[must_use]
pub fn signal_sprite_bases() -> (u32, u32) {
    static MAIN: OnceLock<u32> = OnceLock::new();
    static ALT: OnceLock<u32> = OnceLock::new();
    let main = *MAIN.get_or_init(|| {
        config::env_u32_in_range(
            "OPENTTDRS_SIGNAL_BASE",
            SPR_ORIGINAL_SIGNALS_BASE,
            512..=8192,
        )
    });
    let alt = *ALT.get_or_init(|| {
        config::env_u32_in_range("OPENTTDRS_SIGNAL_ALT_BASE", SPR_SIGNAL_ALT_BASE, 512..=8192)
    });
    (main, alt)
}
const SIGTYPE_LAST_NOPBS: u8 = 3;

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

/// Coordenadas sub-tesela `SignalPositions[signal_on_right][pos]` de OpenTTD.
const SIGNAL_SUBTILE_XY: [[(i8, i8); 12]; 2] = [
    [
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
    ],
    [
        (14, 1),
        (12, 10),
        (4, 6),
        (1, 14),
        (10, 4),
        (0, 1),
        (14, 14),
        (5, 12),
        (11, 13),
        (4, 3),
        (13, 4),
        (3, 11),
    ],
];

#[inline]
fn signal_subtile_xy(pos: u8, signals_on_right: bool) -> (i8, i8) {
    SIGNAL_SUBTILE_XY[usize::from(signals_on_right)][pos.min(11) as usize]
}

/// Posición local OpenTTD de una señal dentro de su tesela.
///
/// Es el punto que `DrawSingleSignal` entrega a `AddSortableSpriteToDraw`;
/// no incluye la corrección de pendiente específica del carril.
#[must_use]
pub fn signal_world_position_for_side(pos: u8, signals_on_right: bool) -> (i8, i8) {
    signal_subtile_xy(pos, signals_on_right)
}

/// Punto seguro para evaluar la pendiente de una señal.
///
/// Replica `GetSafeSlopeZ`: los cuatro carriles ortogonales se anclan a la
/// esquina estable de su fundación de media tesela; X/Y conservan la posición
/// concreta del poste.
#[must_use]
pub fn signal_safe_slope_position_for_side(pos: u8, track: u8, signals_on_right: bool) -> (i8, i8) {
    let (x, y) = signal_world_position_for_side(pos, signals_on_right);
    match track {
        OTTD_TRACK_UPPER => (0, 0),
        OTTD_TRACK_LOWER => (15, 15),
        OTTD_TRACK_LEFT => (15, 0),
        OTTD_TRACK_RIGHT => (0, 15),
        _ => (x, y),
    }
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

/// Bbox/offset NFO del sprite de señal elegido por el pipeline de assets.
#[must_use]
pub fn signal_sprite_metadata(tex_id: u32) -> Option<(i16, i16, i16, i16)> {
    let table = super::signal_sprite_meta_generated::SIGNAL_SPRITE_META;
    table
        .binary_search_by_key(&tex_id, |entry| entry.0)
        .ok()
        .map(|index| {
            let (_, width, height, xrel, yrel) = table[index];
            (width, height, xrel, yrel)
        })
}

/// Ajuste del centro del sprite respecto al ancla `DrawSingleSignal`.
///
/// OpenTTD posiciona la esquina superior izquierda con `xrel/yrel`; Bevy usa
/// el centro y eje Y ascendente. La metadata se genera desde NFO/PNG.
#[must_use]
pub fn signal_sprite_center_offset(tex_id: u32) -> Vec2 {
    let Some((width, height, xrel, yrel)) = signal_sprite_metadata(tex_id) else {
        return Vec2::ZERO;
    };
    Vec2::new(
        f32::from(xrel) + f32::from(width) * 0.5,
        -(f32::from(yrel) + f32::from(height) * 0.5),
    )
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
    signal_screen_position_for_side(tx, ty, pos, tex_id, half_h, base_z, false)
}

/// Variante de [`signal_screen_position`] que aplica el lado configurado.
#[must_use]
pub fn signal_screen_position_for_side(
    tx: i32,
    ty: i32,
    pos: u8,
    tex_id: u32,
    half_h: f32,
    base_z: u8,
    signals_on_right: bool,
) -> Vec2 {
    signal_screen_anchor_for_side(tx, ty, pos, half_h, base_z, signals_on_right)
        + signal_sprite_center_offset(tex_id)
}

/// Ancla `AddSortableSpriteToDraw` sin offsets internos del sprite.
/// Sirve tanto para OpenGFX como para imágenes HD decodificadas desde NewGRF.
#[must_use]
pub fn signal_screen_anchor_for_side(
    tx: i32,
    ty: i32,
    pos: u8,
    half_h: f32,
    base_z: u8,
    signals_on_right: bool,
) -> Vec2 {
    let p = crate::iso::iso(tx, ty);
    let elev = f32::from(base_z) * crate::iso::HEIGHT_PX;
    let track_base = Vec2::new(p.x, p.y - half_h + elev);
    let subtile = if signals_on_right {
        rail_signal_subtile_offset_for_side(pos, true)
    } else {
        rail_signal_subtile_offset(pos)
    };
    track_base + subtile
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
        let Some(sig_track) = SignalTrack::from_u8(track) else {
            return;
        };
        let ty = signal_type_for_track(m2, sig_track);
        let var = signal_variant_for_track(m2, sig_track);
        out.push(SignalSpriteDraw {
            sprite_id: signal_sprite_texture_id(signal_sprite_id(ty, var, image, green)),
            track,
            pos,
            image,
            signal_type: ty,
            variant: var,
            green,
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
            // `DrawSignals`: el bit 3 mira al sudoeste y el bit 2 al
            // nordeste. El orden importa también para los sortables.
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
    let sig_track = SignalTrack::from_u8(track).unwrap_or(SignalTrack::X);
    m2_for_signal(sig_type, variant, sig_track)
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
            // Mono / maglev: rectas, curvas, junctions, diagonales dobles y
            // sus variantes de nieve plana. Todos tienen un sprite tipado.
            for id in 1005u32..=1038 {
                set.insert(id + MONO_RAIL_SPRITE_OFFSET);
                set.insert(id + MAGLEV_RAIL_SPRITE_OFFSET);
            }
            // Overlays de reserva PBS de rampas planas de puente. A diferencia
            // de la vía común, viven en el GRF extra (`5401..=5412`).
            for id in 5401u32..=5412 {
                set.insert(id);
            }
            for id in catenary_wire_sprite_ids() {
                set.insert(id);
            }
            for id in catenary_pylon_sprite_ids() {
                set.insert(id);
            }
            // Cruces a nivel: cada railtype tiene cuatro orientaciones
            // (eje + barrera) y tres superficies (normal, pavimento,
            // nieve/desierto). El selector de `DrawTile_Road` puede llegar
            // a todo el bloque 1370..=1405. Antes sólo se precargaban las
            // cuatro bases mono/maglev y el renderer omitía el ground aunque
            // hubiese elegido correctamente el sprite (Kale 108,36).
            set.extend(1370u32..=1405);
            for id in signal_sprite_ids_for_preload() {
                if !SIGNAL_SPRITE_OPENGFX_GAPS.contains(&id) {
                    set.insert(id);
                }
            }
            set.into_iter().collect()
        })
        .clone()
}

/// IDs cuyo uso con `PALETTE_CRASH=804` requiere una copia ya remapeada.
///
/// La paleta de choque se define por índice, no por RGB. El atlas conserva
/// RGBA, por lo que el pipeline genera `rail_pbs_<id>.png` desde las hojas
/// paletizadas antes de construirlo. Esta lista es el contrato entre esa
/// generación y el renderer.
#[must_use]
pub fn rail_pbs_sprite_ids_for_preload() -> Vec<u32> {
    let mut ids = Vec::with_capacity(30);
    ids.extend(1005u32..=1010);
    ids.extend(1087u32..=1092);
    ids.extend(1169u32..=1174);
    ids.extend(5401u32..=5412);
    ids
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
///
/// Cada máscara 3-vías describe las pistas que llegan a una esquina. OpenTTD
/// pregunta si hay *alguna* de ellas (`TrackBits::Any`), no si está completo el
/// patrón. Esto importa en empalmes de dos pistas: por ejemplo `X | RIGHT`
/// cubre NE y SW por la pista X, pero no NW, así que usa el suelo con NW libre.
#[inline]
fn junction_ground_off(tb: u8) -> u8 {
    let t = tb & 0x3F;
    if (t & RAIL_3WAY_NE) == 0 {
        return 0;
    }
    if (t & RAIL_3WAY_SW) == 0 {
        return 1;
    }
    if (t & RAIL_3WAY_NW) == 0 {
        return 2;
    }
    if (t & RAIL_3WAY_SE) == 0 {
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

/// Desplazamiento de un overlay PBS respecto al centro de la tesela.
///
/// Los `SINGLE_*` comparten sus anclas con los overlays ferroviarios comunes.
/// Los doce sprites de pendiente se extraen directamente del GRF extra y son
/// rectángulos pequeños: el atlas no guarda `xrel/yrel`, así que restauramos
/// aquí su relación con el compuesto de 64×31 píxeles. Tener esta tabla junto
/// a la selección de PBS mantiene coherentes vías, túneles y rampas.
#[must_use]
pub fn rail_pbs_reservation_offset(sprite_id: u32) -> Vec2 {
    match sprite_id {
        // `ogfx2e_extra_32ez.nfo`, sprites 5401..=5412.
        5401 => Vec2::new(27.0, 0.5),
        5402 => Vec2::new(0.0, -12.0),
        5403 => Vec2::new(-27.0, 0.5),
        5404 => Vec2::new(0.0, 13.0),
        5405 => Vec2::new(13.0, 6.5),
        5406 => Vec2::new(13.0, -6.5),
        5407 => Vec2::new(-13.0, -6.5),
        5408 => Vec2::new(-13.0, 6.5),
        5409 => Vec2::new(23.0, 0.0),
        5410 => Vec2::new(0.0, -10.0),
        5411 => Vec2::new(-23.0, 0.0),
        5412 => Vec2::new(0.0, 11.5),
        other => rail_ghost_overlay_offset(other),
    }
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
    rail_signal_subtile_offset_for_side(pos, false)
}

/// Sub-tesela para el lado configurado de las señales.
#[must_use]
pub fn rail_signal_subtile_offset_for_side(pos: u8, signals_on_right: bool) -> Vec2 {
    let (ox, oy) = signal_world_position_for_side(pos, signals_on_right);
    let dx = f32::from(ox) - 8.0;
    let dy = f32::from(oy) - 8.0;
    // `iso(tx, ty)` usa `RemapCoords(16·tx, 16·ty) / 2`.
    remap_tile_offset(dx, dy, 0.0) * 0.5
}

/// Sprite de señal + carril para posicionamiento en pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalSpriteDraw {
    pub sprite_id: u32,
    pub track: u8,
    /// Índice en `SignalPositions` de OpenTTD (`DrawSingleSignal`, `rail_cmd.cpp`).
    pub pos: u8,
    /// Offset dentro del ResultSpriteGroup custom (`sprite += image`).
    pub image: u8,
    pub signal_type: u8,
    pub variant: u8,
    pub green: bool,
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
        RAIL_SPRITE_TRACK_X
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
/// [`RAIL_SPRITE_SNOW_OFFSET`] al sprite inclinado tras aplicar la superficie
/// efectiva del cimiento (`ApplyFoundationToSlope`).
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
    for pass in openttdrs_core::rail_track_draw_plan(tileh, t)
        .passes
        .into_iter()
        .flatten()
    {
        collect_rail_sprites_for_surface(
            pass.track_bits,
            pass.sprite_tileh,
            snow_ground,
            rail_type,
            out,
        );
    }
}

/// Selecciona los sprites de una pasada continua de `rail_track_draw_plan`.
///
/// La división de fundaciones de medio bloque ya ocurrió en core; acá solo
/// queda aplicar el selector clásico/plano y el set tipado mono/maglev.
pub(crate) fn collect_rail_sprites_for_surface(
    track_bits: u8,
    sprite_tileh: u8,
    snow_ground: bool,
    rail_type: openttdrs_core::RailType,
    out: &mut Vec<u32>,
) {
    if sprite_tileh != 0 {
        if let Some(sid) = rail_sloped_track_sprite_id(sprite_tileh, snow_ground) {
            out.push(remap_rail_sprite_id(sid, rail_type));
        }
        return;
    }
    let first = out.len();
    collect_rail_flat_sprites(track_bits, snow_ground, out);
    if matches!(
        rail_type,
        openttdrs_core::RailType::Monorail | openttdrs_core::RailType::Maglev
    ) {
        for sid in &mut out[first..] {
            *sid = remap_rail_sprite_id(*sid, rail_type);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::iso::{TILE_HALF_H, iso};

    #[test]
    fn collect_rail_on_leveled_foundation_uses_flat_track() {
        let mut out = Vec::new();
        // `SLOPE_EW` (5): vía X requiere cimiento nivelado → sprites planos.
        collect_rail_sprites(RAIL_TB_X, 5, false, &mut out);
        assert_eq!(out, vec![1012]);
        collect_rail_sprites(0x29, 5, false, &mut out);
        assert_eq!(out, vec![1020, 1005, 1008, 1009]);
    }

    #[test]
    fn collect_rail_sprites_depot_junction_uses_sw_ground_and_spaced_overlays() {
        let mut out = Vec::new();
        // Empalme depósito ↔ línea X (test `rail_depot_beside_x_line_connects_exit_tile`).
        collect_rail_sprites(0x29, 0, false, &mut out);
        assert_eq!(out, vec![1020, 1005, 1008, 1009]);
        // Salida depósito showcase (12,15): Y|LOWER|LEFT.
        collect_rail_sprites(0x1A, 0, false, &mut out);
        assert_eq!(out, vec![1018, 1006, 1008, 1010]);
    }

    #[test]
    fn junction_overlay_order_matches_openttd_left_before_right() {
        let mut out = Vec::new();
        // Kale_TitleGame (10,51), m5=0x31. `DrawTrackBits` dibuja X, luego
        // LEFT y por último RIGHT; invertir los dos últimos ramales cambia
        // cuál queda por encima en el cruce.
        collect_rail_sprites(RAIL_TB_X | RAIL_TB_LEFT | RAIL_TB_RIGHT, 0, false, &mut out);
        assert_eq!(out, vec![1022, 1005, 1010, 1009]);
    }

    #[test]
    fn typed_rail_depot_layers_match_vanilla_sprite_blocks_and_geometry() {
        use openttdrs_core::RailType;

        assert_eq!(rail_depot_visual_type_index(RailType::Rail), 0);
        assert_eq!(rail_depot_visual_type_index(RailType::Electric), 0);
        assert_eq!(rail_depot_visual_type_index(RailType::Monorail), 1);
        assert_eq!(rail_depot_visual_type_index(RailType::Maglev), 2);

        // `track_land.h` / `_depot_gfx_table`: cada conjunto parte de
        // 1063, 1145 y 1227, respectivamente. SE/SW tienen dos capas.
        assert_eq!(
            rail_depot_build_layers(RailType::Rail, 1)
                .iter()
                .map(|layer| layer.sprite_id)
                .collect::<Vec<_>>(),
            vec![1063, 1064]
        );
        assert_eq!(
            rail_depot_build_layers(RailType::Monorail, 2)
                .iter()
                .map(|layer| layer.sprite_id)
                .collect::<Vec<_>>(),
            vec![1147, 1148]
        );
        assert_eq!(
            rail_depot_build_layers(RailType::Maglev, 2)
                .iter()
                .map(|layer| layer.sprite_id)
                .collect::<Vec<_>>(),
            vec![1229, 1230]
        );

        // Debe conservar TILE_SEQ_LINE directamente, sin los offsets
        // reducidos a la mitad que alejaban la puerta del carril de salida.
        let normal_ne = rail_depot_build_layers(RailType::Rail, 0)[0];
        assert_eq!(
            (normal_ne.dx, normal_ne.dy, normal_ne.sx, normal_ne.sy),
            (2.0, 13.0, 13, 1)
        );
        let normal_ne_offset =
            crate::iso::remap_tile_offset(normal_ne.dx, normal_ne.dy, normal_ne.dz) * 0.5;
        assert_eq!(
            crate::iso::road_depot_overlay_rel(rail_depot_seq_gfx(&normal_ne)),
            (
                normal_ne_offset.x + normal_ne.x_offs,
                normal_ne.y_offs - normal_ne_offset.y,
            ),
            "TILE_SEQ + NFO debe conservar el ancla de la fachada NE en cualquier OpenGFX"
        );
        let maglev_sw_door = rail_depot_build_layers(RailType::Maglev, 2)[0];
        assert_eq!(
            (
                maglev_sw_door.dx,
                maglev_sw_door.dy,
                maglev_sw_door.sx,
                maglev_sw_door.sy,
            ),
            (2.0, 2.0, 13, 1)
        );
        let maglev_sw_offset =
            crate::iso::remap_tile_offset(maglev_sw_door.dx, maglev_sw_door.dy, maglev_sw_door.dz)
                * 0.5;
        assert_eq!(
            crate::iso::road_depot_overlay_rel(rail_depot_seq_gfx(&maglev_sw_door)),
            (
                maglev_sw_offset.x + maglev_sw_door.x_offs,
                maglev_sw_door.y_offs - maglev_sw_offset.y,
            ),
            "la puerta maglev SW conserva su propio recorte y ancla NFO en cualquier OpenGFX"
        );
    }

    #[test]
    fn junction_ground_off_matches_openttd_get_junction_offset() {
        // `TrackBits::Any` en OpenTTD prueba intersección, no inclusión total.
        assert_eq!(junction_ground_off(0x29), 2);
        assert_eq!(junction_ground_off(0x1A), 0);
        assert_eq!(junction_ground_off(RAIL_TB_X | RAIL_TB_RIGHT), 2);
        assert_eq!(junction_ground_off(RAIL_TB_X | RAIL_TB_LEFT), 3);
        assert_eq!(junction_ground_off(RAIL_TB_X | RAIL_TB_LOWER), 2);
        assert_eq!(junction_ground_off(RAIL_3WAY_NE), 4);
        assert_eq!(junction_ground_off(RAIL_3WAY_SW), 4);
        assert_eq!(junction_ground_off(RAIL_3WAY_NW), 4);
        assert_eq!(junction_ground_off(RAIL_3WAY_SE), 4);
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
    fn mono_inclined_foundation_uses_the_post_foundation_slope_sprite() {
        use openttdrs_core::RailType;

        let mut out = Vec::new();
        // SLOPE_E + TRACK_X → InclinedX → SLOPE_NE. OpenTTD selecciona
        // `SPR_MONO_TRACK_Y + 20`, es decir 1113.
        collect_rail_sprites_for_type(
            RAIL_TB_X,
            0x04, // SLOPE_E
            false,
            RailType::Monorail,
            &mut out,
        );
        assert_eq!(out, vec![1113]);
    }

    #[test]
    fn halftile_foundation_uses_the_upper_overlay_slope_for_all_railtypes() {
        use openttdrs_core::RailType;

        let mut out = Vec::new();
        // Kale_TitleGame (158,65): SLOPE_N + UPPER → fake SLOPE_NWE → 1030.
        collect_rail_sprites_for_type(RAIL_TB_UPPER, 8, false, RailType::Rail, &mut out);
        assert_eq!(out, vec![1030]);

        // (116,79): monorail, SLOPE_E + RIGHT → fake SLOPE_SEN.
        collect_rail_sprites_for_type(RAIL_TB_RIGHT, 4, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1109]);

        // La misma regla remapea el sprite inclinado de maglev, no lo deja
        // caer al set de vía normal.
        collect_rail_sprites_for_type(RAIL_TB_LOWER, 2, false, RailType::Maglev, &mut out);
        assert_eq!(out, vec![1192]);
    }

    #[test]
    fn steep_both_foundation_keeps_both_track_passes() {
        let mut out = Vec::new();
        // SLOPE_STEEP_W + LEFT|RIGHT: parte inferior W y overlay alto NWS.
        collect_rail_sprites(RAIL_TB_VERT, 27, false, &mut out);
        assert_eq!(out, vec![1025, 1029]);
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
        collect_rail_sprites_for_type(RAIL_TB_UPPER, 0, false, RailType::Maglev, &mut out);
        assert_eq!(out, vec![1177]);
        // Segunda diagonal doble (antes caía erróneamente al sprite clásico).
        collect_rail_sprites_for_type(RAIL_TB_VERT, 0, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1118]);
        // Junction: suelo SW + overlays tipados. 1020 tiene PNG monorriel,
        // por eso ya no debe caer al viejo 1018/1100 de vía clásica.
        collect_rail_sprites_for_type(0x29, 0, false, RailType::Monorail, &mut out);
        assert_eq!(out, vec![1102, 1087, 1090, 1091]);
    }

    #[test]
    fn pbs_overlays_follow_track_passes_and_rail_type() {
        use openttdrs_core::RailType;

        assert_eq!(
            collect_rail_pbs_reservation_sprites(
                RAIL_TB_X | RAIL_TB_RIGHT,
                RAIL_TB_X | RAIL_TB_RIGHT,
                0,
                RailType::Maglev,
            ),
            vec![1169, 1173]
        );
        assert_eq!(
            collect_rail_pbs_reservation_sprites(
                RAIL_TB_X | RAIL_TB_Y,
                RAIL_TB_X,
                0x0F,
                RailType::Monorail,
            ),
            vec![1087]
        );
        assert_eq!(
            collect_rail_pbs_reservation_sprites(RAIL_TB_Y, RAIL_TB_Y, 9, RailType::Rail,),
            vec![5404],
            "Kale_TitleGame (137,101): la Y reservada sigue la pendiente compatible"
        );
        assert_eq!(
            collect_rail_pbs_reservation_sprites(RAIL_TB_LOWER, RAIL_TB_LOWER, 8, RailType::Rail,),
            vec![1008],
            "las pistas de esquina no usan el banco inclinado"
        );
        assert_eq!(
            collect_rail_pbs_reservation_sprites(RAIL_TB_LEFT, RAIL_TB_LEFT, 3, RailType::Rail,),
            vec![1010],
            "una fundación de medio bloque mantiene la pieza izquierda"
        );
    }

    #[test]
    fn pbs_asset_contract_covers_every_typed_single_and_slope() {
        let ids = rail_pbs_sprite_ids_for_preload();
        assert_eq!(ids.len(), 30);
        for id in [1005, 1010, 1087, 1092, 1169, 1174, 5401, 5412] {
            assert!(ids.contains(&id), "falta PBS {id}");
        }
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
    fn level_crossing_ground_applies_roadside_and_snow_variants() {
        use openttdrs_core::RailType;

        // Kale (108,36): rail crossing barred sobre acera; el tipo de vía
        // escoge 1373 y `DrawTile_Road` suma el bloque pavimentado (+4).
        assert_eq!(
            level_crossing_ground_sprite_id_for_type(0x70, RailType::Rail, true, false),
            1377
        );
        // Kale (204,30): la misma regla aplica al bloque monorail.
        assert_eq!(
            level_crossing_ground_sprite_id_for_type(0x40, RailType::Monorail, true, false),
            1387
        );
        // Nieve/desierto tiene prioridad sobre pavimento y ocupa +8.
        assert_eq!(
            level_crossing_ground_sprite_id_for_type(0x41, RailType::Rail, true, true),
            1378
        );
    }

    #[test]
    fn level_crossing_assets_have_a_dedicated_namespace_for_every_variant() {
        let families = [(1370, "rail"), (1382, "mono"), (1394, "mglv")];
        let mut keys = std::collections::BTreeSet::new();
        for (base, family) in families {
            for offset in 0..12 {
                let id = base + offset;
                let key = level_crossing_sprite_atlas_key(id)
                    .unwrap_or_else(|| panic!("falta clave para cruce {id}"));
                assert_eq!(key, format!("crossing_{family}_{offset:02}.png"));
                assert!(
                    !key.starts_with("rail_"),
                    "el cruce {id} no puede resolver el namespace de señales"
                );
                assert!(keys.insert(key), "clave duplicada para cruce {id}");
            }
        }
        assert_eq!(keys.len(), 36);
        assert_eq!(level_crossing_sprite_atlas_key(1369), None);
        assert_eq!(level_crossing_sprite_atlas_key(1406), None);
    }

    #[test]
    fn rail_sprite_atlas_keys_prefer_named_mono() {
        let keys = rail_sprite_atlas_keys(1093);
        assert!(keys.iter().any(|k| k == "mono_track_y.png"));
        assert!(keys.iter().any(|k| k == "rail_1093.png"));
    }

    #[test]
    fn typed_sprite_range_keeps_double_diagonals_and_snow_variants_typed() {
        // `RAIL_TB_VERT` usa 1036, que se remapea a 1118 / 1200. Los dos
        // sprites siguientes son las variantes de nieve del mismo bloque.
        for id in [1118, 1119, 1120, 1200, 1201, 1202] {
            assert!(is_typed_rail_track_sprite(id), "sprite tipado {id}");
        }
    }

    #[test]
    fn preload_includes_mono_maglev_ids() {
        let ids = rail_sprite_ids_for_preload();
        assert!(ids.contains(&1093));
        assert!(ids.contains(&1175));
        assert!(ids.contains(&1087));
        assert!(ids.contains(&1169));
        assert!(ids.contains(&5401));
        assert!(ids.contains(&5408));
        assert!(ids.contains(&5412));
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
        assert_eq!(
            out,
            vec![
                WIRE_SPRITE_BASE + WSO_EW_SHORT,
                WIRE_SPRITE_BASE + WSO_EW_SHORT
            ]
        );
        collect_catenary_sprites(RAIL_TB_VERT, 0, 0, 0, &mut out);
        assert_eq!(
            out,
            vec![
                WIRE_SPRITE_BASE + WSO_NS_SHORT,
                WIRE_SPRITE_BASE + WSO_NS_SHORT
            ]
        );
        collect_catenary_sprites(0, 0, 0, 0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn catenary_orthogonal_wire_uses_the_active_pcp_end() {
        // `_rail_wires[0]`: las curvas usan sprites de extremo, no siempre
        // el wire corto con postes a ambos lados.
        assert_eq!(rail_wire_wso(0, 2, 1), Some(WSO_EW_W)); // UPPER, NW
        assert_eq!(rail_wire_wso(0, 2, 2), Some(WSO_EW_E)); // UPPER, NE
        assert_eq!(rail_wire_wso(0, 3, 1), Some(WSO_EW_E)); // LOWER, SE
        assert_eq!(rail_wire_wso(0, 3, 2), Some(WSO_EW_W)); // LOWER, SW
        assert_eq!(rail_wire_wso(0, 4, 1), Some(WSO_NS_S)); // LEFT, SW
        assert_eq!(rail_wire_wso(0, 4, 2), Some(WSO_NS_N)); // LEFT, NW
        assert_eq!(rail_wire_wso(0, 5, 1), Some(WSO_NS_N)); // RIGHT, NE
        assert_eq!(rail_wire_wso(0, 5, 2), Some(WSO_NS_S)); // RIGHT, SE
    }

    #[test]
    fn catenary_trace_ids_match_default_openttd_action5_ids() {
        assert_eq!(catenary_reference_sprite_id(WIRE_SPRITE_BASE), 5632);
        assert_eq!(
            catenary_reference_sprite_id(WIRE_SPRITE_BASE + WSO_EW_E),
            5642
        );
        assert_eq!(
            catenary_reference_sprite_id(CATENARY_ENTRANCE_SPRITE_BASE + 2),
            5658
        );
        assert_eq!(catenary_reference_sprite_id(PYLON_SPRITE_BASE + 1), 5661);
        assert_eq!(catenary_reference_sprite_id(1011), 1011);
    }

    #[test]
    fn catenary_wire_trace_bounds_keep_track_geometry_when_wso_is_shared() {
        // `WSO_EW_SHORT` se reutiliza para UPPER y LOWER, pero OpenTTD le
        // asigna dos SpriteBounds distintos en `_rail_catenary_sprite_data`.
        assert_eq!(
            catenary_wire_trace_bounds(0, 2),
            Some(((7, 0, 10), (1, 1, 1)))
        );
        assert_eq!(
            catenary_wire_trace_bounds(0, 3),
            Some(((15, 8, 10), (3, 3, 1)))
        );
        assert_eq!(
            catenary_wire_trace_bounds(0, 4),
            Some(((8, 0, 10), (8, 8, 1)))
        );
        assert_eq!(
            catenary_wire_trace_bounds(0, 5),
            Some(((0, 8, 10), (8, 8, 1)))
        );
    }

    #[test]
    fn catenary_wire_draws_keep_shared_short_sprite_for_each_track() {
        let mut draws = Vec::new();
        collect_catenary_wire_draws_with_effective_tileh(RAIL_TB_HORZ, 0, 0b1111, &mut draws);
        assert_eq!(
            draws,
            vec![
                CatenaryWireDraw {
                    sprite_id: WIRE_SPRITE_BASE + WSO_EW_SHORT,
                    bounds_origin: (7, 0, 10),
                    bounds_extent: (1, 1, 1),
                },
                CatenaryWireDraw {
                    sprite_id: WIRE_SPRITE_BASE + WSO_EW_SHORT,
                    bounds_origin: (15, 8, 10),
                    bounds_extent: (3, 3, 1),
                },
            ],
            "OpenTTD emite ambas curvas aunque compartan PNG corto"
        );
    }

    #[test]
    fn catenary_trace_heights_follow_wire_and_pcp_rounding() {
        use openttdrs_core::SLOPE_NE;

        let wire = CatenaryWireDraw {
            sprite_id: WIRE_SPRITE_BASE + WSO_X_SHORT,
            bounds_origin: (0, 7, 10),
            bounds_extent: (15, 1, 1),
        };
        // En NE, el punto (0, 7) y el PCP NE quedan a +8 píxeles. El cable
        // redondea a 8 y el poste al semiescalón más cercano, también 8.
        assert_eq!(catenary_wire_world_z_delta(SLOPE_NE, 3, RAIL_TB_X, wire), 8);
        assert_eq!(
            catenary_pylon_world_z_delta(SLOPE_NE, 3, RAIL_TB_X, DIAGDIR_NE),
            8
        );
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
        use openttdrs_core::prelude::*;
        use openttdrs_core::{RailType, set_rail_type_on_tile};
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
    fn mask_wire_bits_keeps_the_electrified_branch_of_a_mixed_junction() {
        use openttdrs_core::{RailType, set_rail_type_on_tile};

        // Misma topología que Kale_TitleGame (35,164): X llega por NE a una
        // vía eléctrica y acaba por SW en vía normal; RIGHT sigue hacia SE
        // por una rama eléctrica. `MaskWireBits` debe retirar sólo X.
        let mut map = Map::new_flat(3, 3, 1);
        let home = TileCoord::new(1, 1);
        map.set_tile(home, electric_rail_tile(RAIL_TB_X | RAIL_TB_RIGHT))
            .unwrap();
        map.set_tile(TileCoord::new(0, 1), electric_rail_tile(RAIL_TB_X))
            .unwrap();
        map.set_tile(
            TileCoord::new(1, 2),
            electric_rail_tile(RAIL_TB_LEFT | RAIL_TB_RIGHT),
        )
        .unwrap();
        let plain_x = set_rail_type_on_tile(electric_rail_tile(RAIL_TB_X), RailType::Rail);
        map.set_tile(TileCoord::new(2, 1), plain_x).unwrap();

        assert_eq!(
            mask_catenary_wire_bits(&map, home, 3, 3, 1, RAIL_TB_X | RAIL_TB_RIGHT),
            RAIL_TB_RIGHT
        );
    }

    #[test]
    fn catenary_mask_keeps_branch_that_reaches_an_electric_rail_depot() {
        // Entorno de Kale_TitleGame (133,166), reducido a las cuatro
        // vecinas que consulta `DrawRailCatenaryRailway`.
        let mut map = Map::new_flat(3, 3, 1);
        let home = TileCoord::new(1, 1);
        map.set_tile(home, electric_rail_tile(RAIL_TB_Y | RAIL_TB_LOWER))
            .unwrap();
        let mut southeast = electric_rail_tile(RAIL_TB_CROSS);
        southeast.kind = TileKind::RailDepot;
        southeast.m5 = 0xC3; // Depósito NW: `DiagDirToDiagTrack` = Y.
        map.set_tile(TileCoord::new(1, 2), southeast).unwrap();
        map.set_tile(TileCoord::new(2, 1), electric_rail_tile(RAIL_TB_UPPER))
            .unwrap();
        let mut northwest = electric_rail_tile(RAIL_TB_Y);
        northwest.m5 = 0x42; // RailTileType::Signals + Y.
        map.set_tile(TileCoord::new(1, 0), northwest).unwrap();

        let tracks = electrified_trackbits_at(&map, home, 3, 3, 1);
        let wires = mask_catenary_wire_bits(&map, home, 3, 3, 1, tracks);
        assert_eq!(tracks, RAIL_TB_Y | RAIL_TB_LOWER);
        assert_eq!(wires, RAIL_TB_Y | RAIL_TB_LOWER);
    }

    #[test]
    fn electric_bridge_ramp_is_an_electrified_rail_neighbour() {
        let mut map = Map::new_flat(3, 3, 1);
        let c = TileCoord::new(1, 1);
        let mut bridge = electric_rail_tile(RAIL_TB_X);
        bridge.kind = TileKind::RailBridge;
        bridge.mapt = 0x90;
        bridge.m5 = 0x82; // Rampa SW: eje X.
        map.set_tile(c, bridge).unwrap();

        assert_eq!(electrified_trackbits_at(&map, c, 3, 3, 1), RAIL_TB_X);
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
    fn catenary_tunnel_pylons_only_use_the_exterior_pcp() {
        assert_eq!(catenary_tunnel_exterior_pcp(DIAGDIR_NE), DIAGDIR_SW);
        assert_eq!(catenary_tunnel_exterior_pcp(DIAGDIR_SE), DIAGDIR_NW);
        assert_eq!(catenary_tunnel_exterior_pcp(DIAGDIR_SW), DIAGDIR_NE);
        assert_eq!(catenary_tunnel_exterior_pcp(DIAGDIR_NW), DIAGDIR_SE);
    }

    #[test]
    fn catenary_depot_wire_keeps_upstream_directional_bounds() {
        // `_rail_catenary_sprite_data_depot`, Kale (195,17) = depósito SE.
        let se = catenary_depot_wire_draw(DIAGDIR_SE);
        assert_eq!(catenary_reference_sprite_id(se.sprite_id), 5659);
        assert_eq!(se.bounds_origin, (7, 0, 10));
        assert_eq!(se.bounds_extent, (1, 15, 1));

        let ne = catenary_depot_wire_draw(DIAGDIR_NE);
        assert_eq!(catenary_reference_sprite_id(ne.sprite_id), 5658);
        assert_eq!(ne.bounds_origin, (0, 7, 10));
        assert_eq!(ne.bounds_extent, (15, 1, 1));
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
    fn bridge_catenary_long_wire_parity_matches_openttd_enum_order() {
        // `elrail.cpp`: WIRE_X_FLAT_SW + (num % 2). El índice impar apunta al
        // sprite NE/NW dentro de `_rail_catenary_sprite_data`.
        let mut x_first = Vec::new();
        let mut x_second = Vec::new();
        let mut y_first = Vec::new();
        let mut y_second = Vec::new();
        collect_catenary_bridge_draws(true, 1, 2, 0, &mut x_first);
        collect_catenary_bridge_draws(true, 2, 2, 0, &mut x_second);
        collect_catenary_bridge_draws(false, 1, 2, 0, &mut y_first);
        collect_catenary_bridge_draws(false, 2, 2, 0, &mut y_second);

        assert_eq!(x_first[0].sprite_id, WIRE_SPRITE_BASE + WSO_X_NE);
        assert_eq!(x_second[0].sprite_id, WIRE_SPRITE_BASE + WSO_X_SW);
        assert_eq!(y_first[0].sprite_id, WIRE_SPRITE_BASE + WSO_Y_NW);
        assert_eq!(y_second[0].sprite_id, WIRE_SPRITE_BASE + WSO_Y_SE);
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
    fn collect_signal_draws_matches_openttd_x_trackdir_mapping() {
        let m5 = (RAIL_TILE_SIGNALS << 6) | RAIL_TB_X;
        // OpenTTD: SIG_ELECTRIC = 0 → `SPR_ORIGINAL_SIGNALS_BASE` (1275).
        let m2 = m2_for_signal_encoding(0, 0, OTTD_TRACK_X);

        // `rail_cmd.cpp::DrawSignals`: bit 3 → SOUTHWEST, imagen 0, pos 8.
        let southwest = collect_signal_sprite_draws(m2, 1 << (4 + 3), 0, m5);
        assert_eq!(southwest.len(), 1);
        assert_eq!(southwest[0].pos, 8);
        assert_eq!(southwest[0].image, 0);
        assert_eq!(
            southwest[0].sprite_id, 1275,
            "block eléctrico rojo hacia SW → 1275"
        );

        // Bit 2 → NORTHEAST, imagen 1, pos 9. Está verde en `m3hi`.
        let northeast = collect_signal_sprite_draws(m2, 1 << (4 + 2), 1 << (4 + 2), m5);
        assert_eq!(northeast.len(), 1);
        assert_eq!(northeast[0].pos, 9);
        assert_eq!(northeast[0].image, 1);
        assert_eq!(
            northeast[0].sprite_id, 1278,
            "block eléctrico verde hacia NE → 1278"
        );
    }

    #[test]
    fn signal_screen_position_anchors_to_track_tile_center() {
        let base = Vec2::new(iso(2, 2).x, iso(2, 2).y - TILE_HALF_H);
        let sw = signal_screen_position(2, 2, 8, 1276, TILE_HALF_H, 0);
        let ne = signal_screen_position(2, 2, 9, 1278, TILE_HALF_H, 0);
        assert_ne!(sw, ne);
        assert_eq!(sw - base, Vec2::new(-12.5, 7.0));
        assert_eq!(ne - base, Vec2::new(22.5, 4.5));
        assert!((sw - ne).length() > 8.0, "lados opuestos del riel");
    }

    #[test]
    fn generated_signal_metadata_covers_every_renderable_sprite() {
        for id in signal_sprite_ids_for_preload() {
            assert!(
                signal_sprite_metadata(id).is_some(),
                "falta bbox/offset NFO para rail_{id}.png"
            );
        }
        assert_eq!(signal_sprite_center_offset(1275), Vec2::new(3.5, 5.0));
        assert_eq!(signal_sprite_center_offset(5088), Vec2::new(1.0, 8.0));
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
    fn golden_signal_positions_cover_both_sides_and_every_track_orientation() {
        const LEFT: [(i8, i8); 12] = [
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
        const RIGHT: [(i8, i8); 12] = [
            (14, 1),
            (12, 10),
            (4, 6),
            (1, 14),
            (10, 4),
            (0, 1),
            (14, 14),
            (5, 12),
            (11, 13),
            (4, 3),
            (13, 4),
            (3, 11),
        ];
        for pos in 0u8..12 {
            assert_eq!(signal_subtile_xy(pos, false), LEFT[usize::from(pos)]);
            assert_eq!(signal_subtile_xy(pos, true), RIGHT[usize::from(pos)]);
            assert_eq!(
                signal_world_position_for_side(pos, false),
                LEFT[usize::from(pos)]
            );
            assert_eq!(
                signal_world_position_for_side(pos, true),
                RIGHT[usize::from(pos)]
            );
            assert_ne!(
                rail_signal_subtile_offset_for_side(pos, false),
                rail_signal_subtile_offset_for_side(pos, true),
                "pos {pos}"
            );
        }
    }

    #[test]
    fn signal_safe_slope_position_matches_get_safe_slope_z() {
        // `GetSafeSlopeZ` cambia sólo el punto de lectura de altura de los
        // cuatro carriles ortogonales; X/Y usan la posición real del poste.
        assert_eq!(
            signal_safe_slope_position_for_side(8, OTTD_TRACK_X, true),
            (11, 13)
        );
        assert_eq!(
            signal_safe_slope_position_for_side(10, OTTD_TRACK_Y, true),
            (13, 4)
        );
        assert_eq!(
            signal_safe_slope_position_for_side(0, OTTD_TRACK_UPPER, true),
            (0, 0)
        );
        assert_eq!(
            signal_safe_slope_position_for_side(0, OTTD_TRACK_LOWER, true),
            (15, 15)
        );
        assert_eq!(
            signal_safe_slope_position_for_side(0, OTTD_TRACK_LEFT, true),
            (15, 0)
        );
        assert_eq!(
            signal_safe_slope_position_for_side(0, OTTD_TRACK_RIGHT, true),
            (0, 15)
        );
    }

    #[test]
    fn diagonal_pbs_draw_exposes_get_custom_signal_sprite_parameters() {
        let m2 = m2_for_signal_encoding(4, 1, OTTD_TRACK_LEFT);
        let draws = collect_signal_sprite_draws(m2, 0x40, 0x40, 0x40 | TB_LEFT);
        assert_eq!(draws.len(), 1);
        let draw = draws[0];
        assert_eq!(draw.track, OTTD_TRACK_LEFT);
        assert_eq!(draw.pos, 0);
        assert_eq!(draw.image, 7);
        assert_eq!(draw.signal_type, 4);
        assert_eq!(draw.variant, 1);
        assert!(draw.green);
        assert_ne!(
            signal_screen_anchor_for_side(3, 4, draw.pos, TILE_HALF_H, 0, false),
            signal_screen_anchor_for_side(3, 4, draw.pos, TILE_HALF_H, 0, true)
        );
    }

    #[test]
    fn signal_draw_pos_matches_draw_signals_order() {
        assert_eq!(signal_draw_pos(OTTD_TRACK_X, 3), 8);
        assert_eq!(signal_draw_pos(OTTD_TRACK_X, 2), 9);
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
        use openttdrs_core::prelude::*;
        use openttdrs_core::tile_slope_and_z;

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
        use openttdrs_core::prelude::*;
        use openttdrs_core::tile_slope_and_z;

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
        // (TRACK_X, cara lógica NE, SIG_ELECTRIC=0, verde). El bit 2 usa
        // `SIGNAL_TO_NORTHEAST` (imagen 1); banco alt = Action5
        // (`SPR_SIGNALS_BASE-16`).
        const ROWS: &[(u8, u8, u8, u8, u32, &str)] = &[
            (0, 64, 64, 65, 1278, "block"),
            (1, 64, 64, 65, 5091, "entry"),
            (2, 64, 64, 65, 5107, "exit"),
            (3, 64, 64, 65, 5123, "combo"),
            (4, 64, 64, 65, 5203, "path"),
            (5, 64, 64, 65, 5219, "path_oneway"),
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

    #[test]
    fn level_crossing_preload_covers_all_railtype_and_ground_variants() {
        let ids: std::collections::BTreeSet<_> =
            rail_sprite_ids_for_preload().into_iter().collect();
        for id in 1370u32..=1405 {
            assert!(ids.contains(&id), "falta el sprite de cruce {id}");
        }
    }
}
