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

const TOWN_DRAW_COMPLETED_STAGE: usize = 3;

/// Índice en [`HOUSE_DRAW_DATA`] para casa terminada, alineado con `DrawTile_Town` en
/// `town_cmd.cpp`: `house_id * 16 + TileHash2Bit(x,y) * 4 + stage` con `stage = 3`.
///
/// Si el índice supera el tamaño de la tabla cargada, se usa **módulo** para no colapsar
/// todos los `HouseID` altos en la misma fila.
#[must_use]
pub fn house_draw_data_index_for_tile(clean_house_id: u16, tx: i32, ty: i32) -> usize {
    let hid = usize::from(clean_house_id);
    let h = crate::iso::tile_hash_2bit(tx, ty);
    let idx = hid
        .saturating_mul(16)
        .saturating_add(h.saturating_mul(4))
        .saturating_add(TOWN_DRAW_COMPLETED_STAGE);
    idx % HOUSE_DRAW_DATA.len()
}

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

/// Tabla gfx → sprite para todos los climas de OpenTTD.
/// Índice = gfx (valor de m5 para tile de industria completada, stage 3).
/// Derivado de `_industry_draw_tile_data` en `table/industry_land.h` de OpenTTD.
///
/// Rangos por industria:
/// |   gfx   | Industria               |
/// |---------|-------------------------|
/// |   0-  6 | Coal Mine               |
/// |   7- 10 | Power Station           |
/// |  11- 15 | Sawmill                 |
/// |  16- 23 | Oil Refinery            |
/// |  24- 28 | Forest                  |
/// |  29- 32 | Printing Works          |
/// |  33- 38 | Oil Rig                 |
/// |  39- 42 | Steel Mill              |
/// |  43- 46 | Factory                 |
/// |  47- 51 | Oil Wells               |
/// |  52- 57 | Farm                    |
/// |  58- 59 | Bank (Templado)         |
/// |  60- 71 | Copper Ore Mine         |
/// |  72- 88 | (Plantaciones/otros)    |
/// |  89- 90 | Gold Mine               |
/// |  91- 99 | Iron Ore Mine           |
/// | 100-119 | (Otros climas)          |
pub const INDUSTRY_GFX_DATA: [IndustryGfxSprite; 120] = [
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
    // ── Copper Ore Mine (gfx 60-65) ──────────────────────────────────────────
    // Sprites 0x88C-0x8A6 (2188-2214), 6 tiles
    gfx_building(2190), // 60 → 0x88E
    gfx_building(2193), // 61 → 0x891
    gfx_building(2196), // 62 → 0x894
    gfx_building(2199), // 63 → 0x897
    gfx_building(2202), // 64 → 0x89A
    gfx_building(2214), // 65 → 0x8A6 (suelo especial)
    // ── Copper Ore Mine (gfx 66-71) continuación ─────────────────────────────
    gfx_building(2205), // 66 → 0x89D
    gfx_building(2206), // 67 → 0x89E
    gfx_building(2208), // 68 → 0x8A0
    gfx_building(2209), // 69 → 0x8A1
    gfx_building(2212), // 70 → 0x8A4
    gfx_building(2213), // 71 → 0x8A5
    // ── Plantaciones/otros (gfx 72-88) ───────────────────────────────────────
    // Mayoría ground-only; algunos tiles tienen edificios animados.
    gfx_building(2247), // 72 → 0x8C7
    gfx_ground(),       // 73
    gfx_building(2249), // 74 → 0x8C9
    gfx_building(2250), // 75 → 0x8CA
    gfx_ground(),       // 76
    gfx_ground(),       // 77
    gfx_ground(),       // 78
    gfx_building(2263), // 79 → 0x8D7
    gfx_ground(),       // 80
    gfx_ground(),       // 81
    gfx_ground(),       // 82
    gfx_ground(),       // 83
    gfx_ground(),       // 84
    gfx_ground(),       // 85
    gfx_ground(),       // 86
    gfx_ground(),       // 87
    gfx_building(2265), // 88 → 0x8D9
    // ── Gold Mine (gfx 89-90) ────────────────────────────────────────────────
    gfx_building(2186), // 89 → 0x88A
    gfx_building(2187), // 90 → 0x88B
    // ── Iron Ore Mine (gfx 91-99) ────────────────────────────────────────────
    // Sprites 0x8EC-0x8F4 (2284-2292)
    gfx_building(2284), // 91 → 0x8EC
    gfx_building(2285), // 92 → 0x8ED
    gfx_building(2286), // 93 → 0x8EE
    gfx_building(2287), // 94 → 0x8EF
    gfx_ground(),       // 95
    gfx_ground(),       // 96
    gfx_building(2290), // 97 → 0x8F2
    gfx_ground(),       // 98
    gfx_ground(),       // 99
    // ── Otros climas (gfx 100-119) ───────────────────────────────────────────
    // Todos ground-only en este rango (plantaciones, fábricas trópico, etc.)
    gfx_ground(),       // 100
    gfx_ground(),       // 101
    gfx_ground(),       // 102
    gfx_ground(),       // 103
    gfx_ground(),       // 104
    gfx_ground(),       // 105
    gfx_ground(),       // 106
    gfx_ground(),       // 107
    gfx_ground(),       // 108
    gfx_ground(),       // 109
    gfx_ground(),       // 110
    gfx_ground(),       // 111
    gfx_ground(),       // 112
    gfx_ground(),       // 113
    gfx_ground(),       // 114
    gfx_ground(),       // 115
    gfx_building(2342), // 116 → 0x926
    gfx_building(2343), // 117 → 0x927
    gfx_building(2349), // 118 → 0x92D
    gfx_building(2352), // 119 → 0x930
];

/// Devuelve los metadatos del sprite de industria para el gfx dado (byte m5).
/// Retorna `None` si el gfx no tiene overlay de edificio (solo suelo) o está fuera del rango.
/// Devuelve el sprite de industria para el índice gfx de 9 bits:
/// `gfx = m5 | ((m6 >> 2) & 1) << 8`
pub fn industry_sprite_for_gfx(gfx: u16) -> Option<&'static IndustryGfxSprite> {
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

/// Índice del PNG `road_flat_{idx:02}.png` (0–18), alineado con `GetRoadSpriteOffset` en
/// `road_cmd.cpp` de OpenTTD: en las cuatro pendientes “de borde” (dos esquinas contiguas
/// elevadas) el desplazamiento respecto a `SPR_ROAD_Y` (1332) es **11–14**; en terreno
/// plano se usa la tabla de cruces (`road_flat_index`).
///
/// Bitmask `tileh` igual que `Slope` (sin bit `STEEP`): `SLOPE_NE`=12, `SE`=6, `SW`=3, `NW`=9.
#[must_use]
pub fn road_flat_sprite_index(tileh: u8, road_bits: u8) -> usize {
    match tileh.min(14) {
        0 => road_flat_index(road_bits),
        // Dos esquinas elevadas en diagonal visual → sprites específicos en OpenGFX.
        12 => 11, // SLOPE_NE (N|E)
        6 => 12,  // SLOPE_SE (S|E)
        3 => 13,  // SLOPE_SW (S|W)
        9 => 14,  // SLOPE_NW (N|W)
        _ => road_flat_index(road_bits),
    }
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

#[cfg(test)]
mod house_draw_index_tests {
    use super::house_draw_data_index_for_tile;

    #[test]
    fn low_house_ids_match_openttd_stride_at_origin() {
        assert_eq!(house_draw_data_index_for_tile(0, 0, 0), 3);
        assert_eq!(house_draw_data_index_for_tile(1, 0, 0), 19);
        assert_eq!(house_draw_data_index_for_tile(6, 0, 0), 99);
    }

    #[test]
    fn high_house_id_uses_modulo_not_single_row() {
        let i = house_draw_data_index_for_tile(24, 0, 0);
        assert!(i < 128);
        assert_ne!(i, 127);
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
