//! Constantes y lógica de sprites de `OpenGFX`.

use bevy::prelude::Color;
use openttdrs_core::{Map, TileCoord, TileKind};

#[path = "sprites/rail.rs"]
mod rail;
#[path = "sprites/road.rs"]
mod road;
#[path = "sprites/industry.rs"]
mod industry;

// ── Constantes de renderizado de carreteras y vías ───────────────────────────

/// Tipos de tesela `OpenTTD` (nibble alto del byte MAPT).
pub const OTTD_MP_RAIL: u8 = 1;
pub const OTTD_MP_ROAD: u8 = 2;
pub const OTTD_MP_TUNNELBRIDGE: u8 = 9;

/// `RailGroundType::SnowOrDesert` en los 4 bits bajos de `m4`/`m3` en vía normal (`rail_map.h`).
pub const RAIL_GROUND_SNOW_OR_DESERT: u8 = 12;

/// `IsOnSnowOrDesert` (`road_map.h`): bit 5 de **MAP7** en teselas `MP_ROAD`.
#[must_use]
pub fn road_tile_snow_or_desert(mapt: u8, kind: TileKind, m7: u8) -> bool {
    kind == TileKind::Road && (mapt >> 4) & 0xF == OTTD_MP_ROAD && (m7 & 0x20) != 0
}

/// Color del sprite `road_flat_*` (nieve/desierto vía MAP7).
#[must_use]
pub fn road_flat_sprite_color(mapt: u8, kind: TileKind, m7: u8) -> Color {
    if road_tile_snow_or_desert(mapt, kind, m7) {
        Color::srgb(0.82, 0.88, 0.98)
    } else {
        Color::WHITE
    }
}

/// Color base de raíles (vía + nieve en suelo cuando `m3` bajo coincide con `RAIL_GROUND_SNOW_OR_DESERT`).
#[must_use]
pub fn rail_track_base_color(mapt: u8, kind: TileKind, m5: u8, m3: u8) -> Color {
    const BASE: Color = Color::srgb(0.88, 0.88, 0.97);
    if kind != TileKind::Rail {
        return BASE;
    }
    if (mapt >> 4) & 0xF != OTTD_MP_RAIL {
        return BASE;
    }
    let subtype = (m5 >> 6) & 0x3;
    if subtype > RAIL_TILE_SIGNALS {
        return BASE;
    }
    if (m3 & 0x0F) == RAIL_GROUND_SNOW_OR_DESERT {
        Color::srgb(0.72, 0.80, 0.94)
    } else {
        BASE
    }
}

/// Desplazamiento dentro del grupo `SPR_ROAD` para tesela plana.
pub const ROAD_FLAT_OFFSET_TBL: [u8; 16] = [0, 18, 17, 7, 16, 0, 10, 5, 15, 8, 1, 4, 9, 3, 6, 2];

/// Mitad de la altura en px de cada variante `road_flat_XX`.
pub const ROAD_FLAT_HALF_H: [f32; 19] = [
    15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 19.5, 11.5, 11.5, 19.5, 15.5,
    15.5, 15.5, 15.5,
];

#[allow(unused_imports)]
pub use rail::{
    RAIL_SPRITE_IDS, RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER,
    RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_DEPOT, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS,
    collect_rail_sprites, collect_signal_sprite_ids, level_crossing_has_rail_reservation,
    level_crossing_rail_sprite_id, rail_signal_present_mask, rail_signal_state_mask, rail_sprite_ids_for_preload,
    rail_tile_is_signals, signal_sprite_bases,
};
#[allow(unused_imports)]
pub use industry::{INDUSTRY_GFX_DATA, IndustryGfxSprite, industry_sprite_for_gfx};

/// Especificación de dibujo de una casa (stage completado).
///
/// `s1` es el sprite de suelo/base del tile (0 = omitir, se usa grass).
/// `s2` es el sprite del edificio principal (0 = sin overlay).
pub struct HouseDrawSpec {
    pub s1: u32,
    pub s1_w: f32,
    pub s1_h: f32,
    pub s1_xrel: f32,
    pub s1_yrel: f32,
    pub s2: u32,
    pub s2_w: f32,
    pub s2_h: f32,
    pub s2_xrel: f32,
    pub s2_yrel: f32,
}

#[allow(clippy::too_many_arguments)]
const fn house_spec(
    s1: u32,
    s1_w: f32,
    s1_h: f32,
    s1_xrel: f32,
    s1_yrel: f32,
    s2: u32,
    s2_w: f32,
    s2_h: f32,
    s2_xrel: f32,
    s2_yrel: f32,
) -> HouseDrawSpec {
    HouseDrawSpec {
        s1,
        s1_w,
        s1_h,
        s1_xrel,
        s1_yrel,
        s2,
        s2_w,
        s2_h,
        s2_xrel,
        s2_yrel,
    }
}

/// Primeras **128** filas de `_town_draw_tile_data` (`town_land.h`): **8** tipos de casa
/// × **16** filas (4 variantes `TileHash2Bit` × 4 etapas de obra).
///
/// OpenTTD usa `house_id * 16 + TileHash2Bit(x,y) * 4 + GetHouseBuildingStage`. Con solo
/// 128 filas cargadas, [`house_draw_data_index_for_tile`] aplica **módulo** para
/// `HouseID` altos (variedad visual; para fidelidad total habría que ampliar la tabla).
/// Dimensiones (w, h, xrel, yrel) extraídas del NFO de OpenGFX (ogfx1_base.nfo).
///
/// `s1 = 0` significa "solo grass base" (SPR_FLAT_BARE_LAND o SPR_FLAT_GRASS_TILE).
/// `s2 = 0` significa "sin edificio overlay".
/// Los sprites se cargan como `house_s{id}.png`.
pub const HOUSE_DRAW_DATA: [HouseDrawSpec; 128] = [
    // 0: Tall Office Block – s1=1424, s2=1423
    house_spec(
        1424, 64.0, 37.0, -31.0, -6.0, 1423, 65.0, 76.0, -32.0, -45.0,
    ),
    // 1-3: Office Block variants – s1=1424, s2=1425
    house_spec(
        1424, 64.0, 37.0, -31.0, -6.0, 1425, 65.0, 71.0, -31.0, -40.0,
    ),
    house_spec(
        1424, 64.0, 37.0, -31.0, -6.0, 1425, 65.0, 71.0, -31.0, -40.0,
    ),
    house_spec(
        1424, 64.0, 37.0, -31.0, -6.0, 1425, 65.0, 71.0, -31.0, -40.0,
    ),
    // 4-7: Large Office Block – s1=1429, s2=1428
    house_spec(
        1429, 64.0, 36.0, -31.0, -5.0, 1428, 66.0, 87.0, -32.0, -56.0,
    ),
    house_spec(
        1429, 64.0, 36.0, -31.0, -5.0, 1428, 66.0, 87.0, -32.0, -56.0,
    ),
    house_spec(
        1429, 64.0, 36.0, -31.0, -5.0, 1428, 66.0, 87.0, -32.0, -56.0,
    ),
    house_spec(
        1429, 64.0, 36.0, -31.0, -5.0, 1428, 66.0, 87.0, -32.0, -56.0,
    ),
    // 8-11: Small Block of Flats – s1=1433, s2=1432
    house_spec(
        1433, 64.0, 35.0, -31.0, -4.0, 1432, 35.0, 37.0, -18.0, -15.0,
    ),
    house_spec(
        1433, 64.0, 35.0, -31.0, -4.0, 1432, 35.0, 37.0, -18.0, -15.0,
    ),
    house_spec(
        1433, 64.0, 35.0, -31.0, -4.0, 1432, 35.0, 37.0, -18.0, -15.0,
    ),
    house_spec(
        1433, 64.0, 35.0, -31.0, -4.0, 1432, 35.0, 37.0, -18.0, -15.0,
    ),
    // 12-15: Church – s1=1437, s2=1436
    house_spec(
        1437, 64.0, 34.0, -31.0, -3.0, 1436, 38.0, 38.0, -19.0, -14.0,
    ),
    house_spec(
        1437, 64.0, 34.0, -31.0, -3.0, 1436, 38.0, 38.0, -19.0, -14.0,
    ),
    house_spec(
        1437, 64.0, 34.0, -31.0, -3.0, 1436, 38.0, 38.0, -19.0, -14.0,
    ),
    house_spec(
        1437, 64.0, 34.0, -31.0, -3.0, 1436, 38.0, 38.0, -19.0, -14.0,
    ),
    // 16-19: Large Office (suelo concreto) – s1=1311, s2=1442
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1442, 60.0, 77.0, -30.0, -48.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1442, 60.0, 77.0, -30.0, -48.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1442, 60.0, 77.0, -30.0, -48.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1442, 60.0, 77.0, -30.0, -48.0),
    // 20-23: Large Office v2 – s1=1311, s2=4569
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 4569, 60.0, 77.0, -30.0, -48.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 4569, 60.0, 77.0, -30.0, -48.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 4569, 60.0, 77.0, -30.0, -48.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 4569, 60.0, 77.0, -30.0, -48.0),
    // 24-25: Townhouse V1 – s1=1447, s2=1446
    house_spec(1447, 64.0, 34.0, -31.0, -3.0, 1446, 26.0, 29.0, -14.0, -5.0),
    house_spec(1447, 64.0, 34.0, -31.0, -3.0, 1446, 26.0, 29.0, -14.0, -5.0),
    // 26-27: Townhouse V2 – s1=1505, s2=1506
    house_spec(1505, 64.0, 34.0, -31.0, -3.0, 1506, 38.0, 24.0, -16.0, -1.0),
    house_spec(1505, 64.0, 34.0, -31.0, -3.0, 1506, 38.0, 24.0, -16.0, -1.0),
    // 28-31: Hotel NW – s1=1311, s2=1450
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1450, 58.0, 74.0, -25.0, -43.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1450, 58.0, 74.0, -25.0, -43.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1450, 58.0, 74.0, -25.0, -43.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1450, 58.0, 74.0, -25.0, -43.0),
    // 32-35: Hotel SE – s1=1311, s2=1453
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1453, 62.0, 71.0, -31.0, -43.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1453, 62.0, 71.0, -31.0, -43.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1453, 62.0, 71.0, -31.0, -43.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1453, 62.0, 71.0, -31.0, -43.0),
    // 36-39: Estatua ecuestre – s1=1311, s2=1454
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1454, 19.0, 23.0, -7.0, -13.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1454, 19.0, 23.0, -7.0, -13.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1454, 19.0, 23.0, -7.0, -13.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1454, 19.0, 23.0, -7.0, -13.0),
    // 40-43: Fuente – s1=1311, s2=1455
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1455, 30.0, 32.0, -15.0, -15.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1455, 30.0, 32.0, -15.0, -15.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1455, 30.0, 32.0, -15.0, -15.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1455, 30.0, 32.0, -15.0, -15.0),
    // 44-47: Estatua parque – s1=0(grass), s2=1456
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1456, 64.0, 79.0, -31.0, -48.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1456, 64.0, 79.0, -31.0, -48.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1456, 64.0, 79.0, -31.0, -48.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1456, 64.0, 79.0, -31.0, -48.0),
    // 48-51: Callejón parque – s1=0(grass), s2=1457
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1457, 64.0, 64.0, -31.0, -33.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1457, 64.0, 64.0, -31.0, -33.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1457, 64.0, 64.0, -31.0, -33.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1457, 64.0, 64.0, -31.0, -33.0),
    // 52-55: Oficina 0D – s1=1311, s2=1460
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1460, 52.0, 61.0, -25.0, -33.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1460, 52.0, 61.0, -25.0, -33.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1460, 52.0, 61.0, -25.0, -33.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1460, 52.0, 61.0, -25.0, -33.0),
    // 56-59: Tienda/Oficina 0E – s1=1311, s2=1463
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1463, 64.0, 68.0, -35.0, -41.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1463, 64.0, 68.0, -35.0, -41.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1463, 64.0, 68.0, -35.0, -41.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1463, 64.0, 68.0, -35.0, -41.0),
    // 60-63: Tienda/Oficina 0F – s1=1311, s2=1466
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1466, 66.0, 68.0, -28.0, -41.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1466, 66.0, 68.0, -28.0, -41.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1466, 66.0, 68.0, -28.0, -41.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1466, 66.0, 68.0, -28.0, -41.0),
    // 64-67: Torres altas – s1=1311, s2=1469
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1469, 66.0, 79.0, -28.0, -50.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1469, 66.0, 79.0, -28.0, -50.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1469, 66.0, 79.0, -28.0, -50.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1469, 66.0, 79.0, -28.0, -50.0),
    // 68-71: Torres muy altas – s1=1311, s2=1472
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1472, 54.0, 115.0, -25.0, -88.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1472, 54.0, 115.0, -25.0, -88.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1472, 54.0, 115.0, -25.0, -88.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1472, 54.0, 115.0, -25.0, -88.0),
    // 72-75: Torres NE – s1=1311, s2=1475
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1475, 48.0, 44.0, -23.0, -20.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1475, 48.0, 44.0, -23.0, -20.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1475, 48.0, 44.0, -23.0, -20.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1475, 48.0, 44.0, -23.0, -20.0),
    // 76-79: Oficina alta v2 – s1=1311, s2=1478
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1478, 65.0, 76.0, -28.0, -47.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1478, 65.0, 76.0, -28.0, -47.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1478, 65.0, 76.0, -28.0, -47.0),
    house_spec(1311, 27.0, 28.0, 0.0, 0.0, 1478, 65.0, 76.0, -28.0, -47.0),
    // 80-83: Casa pequeña – s1=0(grass), s2=1483
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1483, 64.0, 23.0, -31.0, -2.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1483, 64.0, 23.0, -31.0, -2.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1483, 64.0, 23.0, -31.0, -2.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1483, 64.0, 23.0, -31.0, -2.0),
    // 84-87: Cottage A – s1=0(grass), s2=1484
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1484, 38.0, 40.0, -17.0, -9.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1484, 38.0, 40.0, -17.0, -9.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1484, 38.0, 40.0, -17.0, -9.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1484, 38.0, 40.0, -17.0, -9.0),
    // 88-91: Cottage B – s1=0(grass), s2=1485
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1485, 32.0, 40.0, -19.0, -9.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1485, 32.0, 40.0, -19.0, -9.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1485, 32.0, 40.0, -19.0, -9.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1485, 32.0, 40.0, -19.0, -9.0),
    // 92-95: Cobertizo – s1=0(grass), s2=1486
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1486, 62.0, 18.0, -30.0, 7.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1486, 62.0, 18.0, -30.0, 7.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1486, 62.0, 18.0, -30.0, 7.0),
    house_spec(0, 0.0, 0.0, 0.0, 0.0, 1486, 62.0, 18.0, -30.0, 7.0),
    // 96: Casa pequeña tipo A – s1=1491, s2=1492
    house_spec(1491, 64.0, 34.0, -31.0, -3.0, 1492, 30.0, 28.0, -17.0, -5.0),
    // 97: Casa pequeña tipo B – s1=1493, s2=1494
    house_spec(1493, 64.0, 34.0, -31.0, -3.0, 1494, 28.0, 27.0, -14.0, -7.0),
    // 98: Casa pequeña tipo C – s1=1487, s2=1488
    house_spec(1487, 64.0, 34.0, -31.0, -3.0, 1488, 26.0, 28.0, -18.0, -5.0),
    // 99: Casa pequeña tipo D – s1=1489, s2=1490
    house_spec(1489, 64.0, 34.0, -31.0, -3.0, 1490, 26.0, 28.0, -8.0, -4.0),
    // 100-103: Cottage con camino – s1=1495, s2=1496
    house_spec(
        1495, 64.0, 34.0, -31.0, -3.0, 1496, 26.0, 29.0, -14.0, -10.0,
    ),
    house_spec(
        1495, 64.0, 34.0, -31.0, -3.0, 1496, 26.0, 29.0, -14.0, -10.0,
    ),
    house_spec(
        1495, 64.0, 34.0, -31.0, -3.0, 1496, 26.0, 29.0, -14.0, -10.0,
    ),
    house_spec(
        1495, 64.0, 34.0, -31.0, -3.0, 1496, 26.0, 29.0, -14.0, -10.0,
    ),
    // 104: Casa con tienda – s1=1499, s2=1500
    house_spec(1499, 64.0, 31.0, -31.0, 0.0, 1500, 36.0, 28.0, -12.0, -9.0),
    // 105: Townhouse alta – s1=1574, s2=1575
    house_spec(
        1574, 64.0, 34.0, -31.0, -3.0, 1575, 32.0, 27.0, -17.0, -11.0,
    ),
    // 106: Tienda A – s1=1511, s2=1512
    house_spec(1511, 64.0, 34.0, -31.0, -3.0, 1512, 36.0, 28.0, -18.0, -8.0),
    // 107: Tienda B – s1=1517, s2=1518
    house_spec(1517, 64.0, 34.0, -31.0, -3.0, 1518, 32.0, 27.0, -12.0, -9.0),
    // 108-109: Tienda C – s1=1522, s2=1523
    house_spec(
        1522, 64.0, 34.0, -31.0, -3.0, 1523, 40.0, 38.0, -19.0, -18.0,
    ),
    house_spec(
        1522, 64.0, 34.0, -31.0, -3.0, 1523, 40.0, 38.0, -19.0, -18.0,
    ),
    // 110-111: Casa con árboles – s1=1528, s2=1529
    house_spec(
        1528, 64.0, 35.0, -31.0, -4.0, 1529, 45.0, 46.0, -22.0, -20.0,
    ),
    house_spec(
        1528, 64.0, 35.0, -31.0, -4.0, 1529, 45.0, 46.0, -22.0, -20.0,
    ),
    // 112-113: Casa con torre – s1=1534, s2=1535
    house_spec(
        1534, 64.0, 31.0, -31.0, 0.0, 1535, 53.0, 102.0, -27.0, -75.0,
    ),
    house_spec(
        1534, 64.0, 31.0, -31.0, 0.0, 1535, 53.0, 102.0, -27.0, -75.0,
    ),
    // 114-115: Casa con aguja – s1=1550, s2=1551
    house_spec(1550, 64.0, 31.0, -31.0, 0.0, 1551, 56.0, 97.0, -25.0, -69.0),
    house_spec(1550, 64.0, 31.0, -31.0, 0.0, 1551, 56.0, 97.0, -25.0, -69.0),
    // 116-117: Oficina moderna A – s1=1536, s2=1537
    house_spec(
        1536, 64.0, 38.0, -31.0, -7.0, 1537, 66.0, 71.0, -32.0, -40.0,
    ),
    house_spec(
        1536, 64.0, 38.0, -31.0, -7.0, 1537, 66.0, 71.0, -32.0, -40.0,
    ),
    // 118-119: Oficina moderna B – s1=1538, s2=1539
    house_spec(
        1538, 64.0, 36.0, -31.0, -5.0, 1539, 66.0, 87.0, -32.0, -56.0,
    ),
    house_spec(
        1538, 64.0, 36.0, -31.0, -5.0, 1539, 66.0, 87.0, -32.0, -56.0,
    ),
    // 120-123: Bloques curvos – s1=1544, s2=1545
    house_spec(1544, 64.0, 31.0, -31.0, 0.0, 1545, 62.0, 83.0, -30.0, -55.0),
    house_spec(1544, 64.0, 31.0, -31.0, 0.0, 1545, 62.0, 83.0, -30.0, -55.0),
    house_spec(1544, 64.0, 31.0, -31.0, 0.0, 1545, 62.0, 83.0, -30.0, -55.0),
    house_spec(1544, 64.0, 31.0, -31.0, 0.0, 1545, 62.0, 83.0, -30.0, -55.0),
    // 124-127: Bloques modernos – s1=1552, s2=1553
    house_spec(1552, 64.0, 31.0, -31.0, 0.0, 1553, 54.0, 53.0, -25.0, -24.0),
    house_spec(1552, 64.0, 31.0, -31.0, 0.0, 1553, 54.0, 53.0, -25.0, -24.0),
    house_spec(1552, 64.0, 31.0, -31.0, 0.0, 1553, 54.0, 53.0, -25.0, -24.0),
    house_spec(1552, 64.0, 31.0, -31.0, 0.0, 1553, 54.0, 53.0, -25.0, -24.0),
];

/// Devuelve el nombre de archivo (relativo a `opengfx/tiles/`) para un sprite de casa.
/// Usa el naming genérico `house_s{id}.png` para todos los sprites extraídos.
pub fn house_sprite_filename(sprite_id: u32) -> String {
    format!("house_s{sprite_id}.png")
}

/// Índice en [`HOUSE_DRAW_DATA`] para una casa.
///
/// En este cliente la tabla tiene 128 entradas (una por `HouseID` base de OpenGFX),
/// ya "aplanadas" para render de edificio terminado. Por eso indexamos por `HouseID`
/// directo (con módulo para IDs extendidos/NewGRF), en vez de aplicar el stride de
/// `_town_draw_tile_data` (`*16 + hash*4 + stage`) que aquí colapsa variedad.
#[must_use]
pub fn house_draw_data_index_for_tile(clean_house_id: u16, _tx: i32, _ty: i32) -> usize {
    let hid = usize::from(clean_house_id);
    hid % HOUSE_DRAW_DATA.len()
}

/// `RoadTileType::Crossing` en bits 6–7 de `m5` (`road_map.h`).
#[must_use]
pub fn is_road_level_crossing(mapt: u8, m5: u8, kind: TileKind) -> bool {
    rail::is_road_level_crossing(mapt, m5, kind, OTTD_MP_ROAD)
}

/// Tranvía presente en la tesela de carretera (`GetRoadTypeTram` ≠ inválido; 6 bits altos de `m8`).
#[must_use]
pub fn road_tile_has_tram_track(m8: u16) -> bool {
    road::road_tile_has_tram_track(m8)
}

// ── Lógica de road bits ─────────────────────────────────────────────────────

/// Decodifica los road bits efectivos desde m5 según el tipo de tesela.
///
/// Los bits del savegame ya están en la orientación correcta de OpenTTD:
/// - NW (bit 0) = conexión hacia (x, y-1)  → visualmente arriba-izquierda
/// - SW (bit 1) = conexión hacia (x+1, y)  → visualmente abajo-izquierda
/// - SE (bit 2) = conexión hacia (x, y+1)  → visualmente abajo-derecha
/// - NE (bit 3) = conexión hacia (x-1, y)  → visualmente arriba-derecha
#[allow(dead_code)] // Wrapper de compatibilidad pública tras split a sprites/road.rs.
pub fn effective_road_bits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    road::effective_road_bits(mapt, m5, kind, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE)
}

#[inline]
#[allow(dead_code)] // Wrapper de compatibilidad pública tras split a sprites/road.rs.
pub fn road_flat_index(road_bits: u8) -> usize {
    road::road_flat_index(road_bits, &ROAD_FLAT_OFFSET_TBL)
}

/// Índice del PNG `road_flat_{idx:02}.png` (0–18), alineado con `GetRoadSpriteOffset` en
/// `road_cmd.cpp` de OpenTTD: en las cuatro pendientes “de borde” (dos esquinas contiguas
/// elevadas) el desplazamiento respecto a `SPR_ROAD_Y` (1332) es **11–14**; en terreno
/// plano se usa la tabla de cruces (`road_flat_index`).
///
/// Bitmask `tileh` igual que `Slope` (sin bit `STEEP`): `SLOPE_NE`=12, `SE`=6, `SW`=3, `NW`=9.
#[must_use]
pub fn road_flat_sprite_index(tileh: u8, road_bits: u8) -> usize {
    road::road_flat_sprite_index(tileh, road_bits, &ROAD_FLAT_OFFSET_TBL)
}

/// Road bits para dibujar: `m5` / vecinos (mapa procedural).
///
/// Asignación de bits conforme a OpenTTD (con iso correcta):
/// - NE (bit 3 = 8): vecino en (x-1, y) → arriba-derecha en pantalla
/// - NW (bit 0 = 1): vecino en (x, y-1) → arriba-izquierda en pantalla
/// - SW (bit 1 = 2): vecino en (x+1, y) → abajo-izquierda en pantalla
/// - SE (bit 2 = 4): vecino en (x, y+1) → abajo-derecha en pantalla
pub fn road_bits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    road::road_bits_for_render(map, pos, mw, mh, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE)
}

// ── Lógica de rail bits ─────────────────────────────────────────────────────

#[allow(dead_code)] // Wrapper de compatibilidad pública tras split a sprites/rail.rs.
pub fn effective_rail_trackbits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    rail::effective_rail_trackbits(mapt, m5, kind, OTTD_MP_RAIL)
}

pub fn rail_trackbits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    rail::rail_trackbits_for_render(map, pos, mw, mh, OTTD_MP_RAIL)
}

#[cfg(test)]
mod house_draw_index_tests {
    use super::house_draw_data_index_for_tile;

    #[test]
    fn low_house_ids_map_directly_to_table_rows() {
        assert_eq!(house_draw_data_index_for_tile(0, 0, 0), 0);
        assert_eq!(house_draw_data_index_for_tile(1, 0, 0), 1);
        assert_eq!(house_draw_data_index_for_tile(6, 0, 0), 6);
    }

    #[test]
    fn high_house_id_uses_modulo() {
        assert_eq!(house_draw_data_index_for_tile(127, 0, 0), 127);
        assert_eq!(house_draw_data_index_for_tile(128, 0, 0), 0);
        assert_eq!(house_draw_data_index_for_tile(129, 0, 0), 1);
    }
}

#[cfg(test)]
mod road_sprite_index_tests {
    use super::{road_flat_index, road_flat_sprite_index};

    #[test]
    fn flat_tile_uses_road_bits_table() {
        assert_eq!(road_flat_sprite_index(0, 0x05), road_flat_index(0x05));
    }

    #[test]
    fn simple_diagonal_slopes_use_openttd_offsets_11_to_14() {
        assert_eq!(road_flat_sprite_index(12, 0x0F), 11); // SLOPE_NE
        assert_eq!(road_flat_sprite_index(6, 0x0F), 12); // SLOPE_SE
        assert_eq!(road_flat_sprite_index(3, 0x0F), 13); // SLOPE_SW
        assert_eq!(road_flat_sprite_index(9, 0x0F), 14); // SLOPE_NW
    }

    #[test]
    fn other_slopes_keep_flat_road_variant_from_bits() {
        let bits = 0x0A;
        assert_eq!(road_flat_sprite_index(1, bits), road_flat_index(bits)); // SLOPE_W
    }
}

#[cfg(test)]
mod level_crossing_tests {
    use super::{
        OTTD_MP_ROAD, RAIL_TILE_SIGNALS, is_road_level_crossing, level_crossing_rail_sprite_id,
        rail_tile_is_signals,
    };
    use openttdrs_core::TileKind;

    #[test]
    fn crossing_detected_on_mp_road_subtype_1() {
        let mapt = OTTD_MP_ROAD << 4;
        let m5 = 0x40; // Crossing << 6, road axis 0
        assert!(is_road_level_crossing(mapt, m5, TileKind::Road));
        assert!(!is_road_level_crossing(mapt, 0, TileKind::Road));
    }

    #[test]
    fn crossing_rail_sprite_alternates_with_road_axis() {
        assert_eq!(level_crossing_rail_sprite_id(0x40), 1371);
        assert_eq!(level_crossing_rail_sprite_id(0x41), 1370);
    }

    #[test]
    fn crossing_rail_sprite_adds_two_when_barred() {
        assert_eq!(level_crossing_rail_sprite_id(0x40 | 0x20), 1373);
        assert_eq!(level_crossing_rail_sprite_id(0x41 | 0x20), 1372);
    }

    #[test]
    fn signals_subtype_is_bit_pattern() {
        assert!(rail_tile_is_signals(0x01 | (RAIL_TILE_SIGNALS << 6)));
        assert!(!rail_tile_is_signals(0x01));
    }
}

#[cfg(test)]
mod signal_sprite_collect_tests {
    use super::{RAIL_TB_Y, RAIL_TILE_SIGNALS, collect_signal_sprite_ids, rail_tile_is_signals};

    #[test]
    fn semaphore_variant_changes_sprite_id() {
        let m5 = (RAIL_TILE_SIGNALS << 6) | RAIL_TB_Y;
        let m3 = 0xC0;
        let m3hi = 0;
        let ids_e = collect_signal_sprite_ids(0, m3, m3hi, m5);
        // variant 1 en bit 3 del byte bajo de m2 para pistas que leen variante en bit 3 (TRACK_X/Y)
        let ids_s = collect_signal_sprite_ids(0x08, m3, m3hi, m5);
        assert_eq!(ids_e.len(), 2);
        assert_eq!(ids_s.len(), 2);
        assert_ne!(ids_e[0], ids_s[0]);
    }

    #[test]
    fn rail_preload_includes_crossings_and_signals_bounded() {
        use super::rail_sprite_ids_for_preload;
        let ids = rail_sprite_ids_for_preload();
        assert!(!ids.is_empty());
        assert!(ids.contains(&1372));
        assert!(ids.contains(&1279));
        let mx = ids.iter().copied().max().unwrap_or(0);
        assert!(
            mx < 1700,
            "máx sprite id {mx}: ampliar range(1275,…) en descargar_graficos.sh si hace falta"
        );
    }

    #[test]
    fn y_track_two_present_signals_two_sprites() {
        let m5 = (RAIL_TILE_SIGNALS << 6) | RAIL_TB_Y;
        assert!(rail_tile_is_signals(m5));
        let m3 = 0xC0; // bits 2,3 en nibble alto → presentes
        let m3hi = 0x80; // bit 3 del nibble alto de estados → verde en señal 3
        let ids = collect_signal_sprite_ids(0, m3, m3hi, m5);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn opengfx_default_signal_alt_is_seventy_seven_above_classic() {
        // Ancla documentada: bloque extendido de señales en `ogfx1_base.grf` sigue al rango 1275…
        assert_eq!(1352_u32 - 1275_u32, 77);
    }
}
