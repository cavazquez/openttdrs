//! Tablas International (`_airport_moving_data_international`, `_airport_fta_international`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_AIRPORT_ENTRANCE, BLOCK_HANGAR2_AREA, BLOCK_HELIPAD1, BLOCK_HELIPAD2, BLOCK_IN_WAY,
    BLOCK_OUT_WAY, BLOCK_PRE_HELIPAD, BLOCK_RUNWAY_OUT, BLOCK_TAXIWAY_BUSY, BLOCK_TERM_GROUP1,
    BLOCK_TERM_GROUP2, BLOCK_TERM_GROUP2_ENTER1, BLOCK_TERM_GROUP2_ENTER2, BLOCK_TERM_GROUP2_EXIT1,
    BLOCK_TERM_GROUP2_EXIT2, BLOCK_TERM1, BLOCK_TERM2, BLOCK_TERM3, BLOCK_TERM4, BLOCK_TERM5,
    BLOCK_TERM6, FLAG_BRAKE, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_LAND,
    FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN, FLAG_TAKEOFF,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;
const DIR_SW: u8 = 5;
const DIR_NW: u8 = 7;

/// Número de waypoints International.
pub const INTERNATIONAL_NOF_ELEMENTS: usize = 53;

/// Entradas de holding (`_airport_entries_international`).
pub const INTERNATIONAL_ENTRIES: [u8; 4] = [38, 37, 40, 39];

/// `_airport_moving_data_international[53]`.
pub static INTERNATIONAL_MOVING_DATA: [AirportMovingData; INTERNATIONAL_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 7,
        y: 55,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0
    AirportMovingData {
        x: 100,
        y: 21,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 1
    AirportMovingData {
        x: 7,
        y: 70,
        flags: 0,
        direction: DIR_N,
    }, // 2
    AirportMovingData {
        x: 100,
        y: 36,
        flags: 0,
        direction: DIR_N,
    }, // 3
    AirportMovingData {
        x: 38,
        y: 70,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 4
    AirportMovingData {
        x: 38,
        y: 54,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 5
    AirportMovingData {
        x: 38,
        y: 38,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 6
    AirportMovingData {
        x: 70,
        y: 70,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 7
    AirportMovingData {
        x: 70,
        y: 54,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 8
    AirportMovingData {
        x: 70,
        y: 38,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 9
    AirportMovingData {
        x: 104,
        y: 71,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 10
    AirportMovingData {
        x: 104,
        y: 55,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 11
    AirportMovingData {
        x: 22,
        y: 87,
        flags: 0,
        direction: DIR_N,
    }, // 12
    AirportMovingData {
        x: 60,
        y: 87,
        flags: 0,
        direction: DIR_N,
    }, // 13
    AirportMovingData {
        x: 66,
        y: 87,
        flags: 0,
        direction: DIR_N,
    }, // 14
    AirportMovingData {
        x: 86,
        y: 87,
        flags: FLAG_EXACT,
        direction: DIR_NW,
    }, // 15
    AirportMovingData {
        x: 86,
        y: 70,
        flags: 0,
        direction: DIR_N,
    }, // 16
    AirportMovingData {
        x: 86,
        y: 54,
        flags: 0,
        direction: DIR_N,
    }, // 17
    AirportMovingData {
        x: 86,
        y: 38,
        flags: 0,
        direction: DIR_N,
    }, // 18
    AirportMovingData {
        x: 86,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 19
    AirportMovingData {
        x: 66,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 20
    AirportMovingData {
        x: 60,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 21
    AirportMovingData {
        x: 38,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 22
    AirportMovingData {
        x: 22,
        y: 70,
        flags: 0,
        direction: DIR_N,
    }, // 23
    AirportMovingData {
        x: 22,
        y: 58,
        flags: 0,
        direction: DIR_N,
    }, // 24
    AirportMovingData {
        x: 22,
        y: 38,
        flags: 0,
        direction: DIR_N,
    }, // 25
    AirportMovingData {
        x: 22,
        y: 22,
        flags: FLAG_EXACT,
        direction: DIR_NW,
    }, // 26
    AirportMovingData {
        x: 22,
        y: 6,
        flags: 0,
        direction: DIR_N,
    }, // 27
    AirportMovingData {
        x: 3,
        y: 6,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 28
    AirportMovingData {
        x: 60,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 29
    AirportMovingData {
        x: 105,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 30
    AirportMovingData {
        x: 190,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 31
    AirportMovingData {
        x: 193,
        y: 104,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 32
    AirportMovingData {
        x: 105,
        y: 104,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_LAND,
        direction: DIR_N,
    }, // 33
    AirportMovingData {
        x: 3,
        y: 104,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 34
    AirportMovingData {
        x: 12,
        y: 104,
        flags: FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 35
    AirportMovingData {
        x: 7,
        y: 84,
        flags: 0,
        direction: DIR_N,
    }, // 36
    AirportMovingData {
        x: 1,
        y: 209,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 37
    AirportMovingData {
        x: 1,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 38
    AirportMovingData {
        x: 273,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 39
    AirportMovingData {
        x: 305,
        y: 81,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 40
    AirportMovingData {
        x: 128,
        y: 80,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 41
    AirportMovingData {
        x: 128,
        y: 80,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 42
    AirportMovingData {
        x: 96,
        y: 71,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 43
    AirportMovingData {
        x: 96,
        y: 55,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 44
    AirportMovingData {
        x: 96,
        y: 71,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 45
    AirportMovingData {
        x: 96,
        y: 55,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 46
    AirportMovingData {
        x: 104,
        y: 71,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 47
    AirportMovingData {
        x: 104,
        y: 55,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 48
    AirportMovingData {
        x: 104,
        y: 32,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 49
    AirportMovingData {
        x: 104,
        y: 32,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 50
    AirportMovingData {
        x: 7,
        y: 70,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 51
    AirportMovingData {
        x: 100,
        y: 36,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 52
];

/// Filas `_airport_fta_international` (sin marcador final).
static INTERNATIONAL_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 2),
    (0, 255, BLOCK_TERM_GROUP1, 0),
    (0, 255, BLOCK_TERM_GROUP2_ENTER1, 1),
    (0, 13, BLOCK_AIRPORT_ENTRANCE, 2),
    (0, 0, 0, 2),
    (1, 1, 0, 3),
    (1, 255, BLOCK_HANGAR2_AREA, 1),
    (1, 13, BLOCK_HANGAR2_AREA, 3),
    (1, 0, 0, 3),
    (2, 255, BLOCK_AIRPORT_ENTRANCE, 0),
    (2, 1, 0, 0),
    (2, 5, 0, 12),
    (2, 6, 0, 12),
    (2, 7, 0, 12),
    (2, 8, 0, 12),
    (2, 9, 0, 12),
    (2, 13, 0, 51),
    (2, 0, 0, 23),
    (3, 255, BLOCK_HANGAR2_AREA, 0),
    (3, 1, 0, 1),
    (3, 13, 0, 52),
    (3, 0, 0, 18),
    (4, 2, BLOCK_TERM1, 23),
    (4, 1, BLOCK_AIRPORT_ENTRANCE, 23),
    (4, 0, 0, 23),
    (5, 3, BLOCK_TERM2, 24),
    (5, 1, BLOCK_AIRPORT_ENTRANCE, 24),
    (5, 0, 0, 24),
    (6, 4, BLOCK_TERM3, 25),
    (6, 1, BLOCK_AIRPORT_ENTRANCE, 25),
    (6, 0, 0, 25),
    (7, 5, BLOCK_TERM4, 16),
    (7, 1, BLOCK_HANGAR2_AREA, 16),
    (7, 0, 0, 16),
    (8, 6, BLOCK_TERM5, 17),
    (8, 1, BLOCK_HANGAR2_AREA, 17),
    (8, 0, 0, 17),
    (9, 7, BLOCK_TERM6, 18),
    (9, 1, BLOCK_HANGAR2_AREA, 18),
    (9, 0, 0, 18),
    (10, 8, BLOCK_HELIPAD1, 10),
    (10, 1, BLOCK_HANGAR2_AREA, 16),
    (10, 13, 0, 47),
    (11, 9, BLOCK_HELIPAD2, 11),
    (11, 1, BLOCK_HANGAR2_AREA, 17),
    (11, 13, 0, 48),
    (12, 0, BLOCK_TERM_GROUP2_ENTER1, 13),
    (13, 0, BLOCK_TERM_GROUP2_ENTER1, 14),
    (14, 0, BLOCK_TERM_GROUP2_ENTER2, 15),
    (15, 0, BLOCK_TERM_GROUP2_ENTER2, 16),
    (16, 255, BLOCK_TERM_GROUP2, 0),
    (16, 5, BLOCK_TERM4, 7),
    (16, 8, BLOCK_HELIPAD1, 10),
    (16, 13, BLOCK_HELIPAD1, 10),
    (16, 0, 0, 17),
    (17, 255, BLOCK_TERM_GROUP2, 0),
    (17, 6, BLOCK_TERM5, 8),
    (17, 5, 0, 16),
    (17, 8, 0, 16),
    (17, 9, BLOCK_HELIPAD2, 11),
    (17, 13, BLOCK_HELIPAD2, 11),
    (17, 0, 0, 18),
    (18, 255, BLOCK_TERM_GROUP2, 0),
    (18, 7, BLOCK_TERM6, 9),
    (18, 10, 0, 19),
    (18, 1, BLOCK_HANGAR2_AREA, 3),
    (18, 0, 0, 17),
    (19, 0, BLOCK_TERM_GROUP2_EXIT1, 20),
    (20, 0, BLOCK_TERM_GROUP2_EXIT1, 21),
    (21, 0, BLOCK_TERM_GROUP2_EXIT2, 22),
    (22, 0, BLOCK_TERM_GROUP2_EXIT2, 26),
    (23, 255, BLOCK_TERM_GROUP1, 0),
    (23, 2, BLOCK_TERM1, 4),
    (23, 1, BLOCK_AIRPORT_ENTRANCE, 2),
    (23, 0, 0, 24),
    (24, 255, BLOCK_TERM_GROUP1, 0),
    (24, 3, BLOCK_TERM2, 5),
    (24, 2, 0, 23),
    (24, 1, 0, 23),
    (24, 0, 0, 25),
    (25, 255, BLOCK_TERM_GROUP1, 0),
    (25, 4, BLOCK_TERM3, 6),
    (25, 10, 0, 26),
    (25, 0, 0, 24),
    (26, 255, BLOCK_TAXIWAY_BUSY, 0),
    (26, 10, 0, 27),
    (26, 0, 0, 25),
    (27, 0, BLOCK_OUT_WAY, 28),
    (28, 10, BLOCK_OUT_WAY, 29),
    (29, 0, BLOCK_RUNWAY_OUT, 30),
    (30, 11, 0, 31),
    (31, 12, 0, 0),
    (32, 14, 0, 37),
    (32, 15, 0, 33),
    (32, 17, 0, 41),
    (33, 15, BLOCK_AIRPORT_BUSY, 34),
    (34, 0, BLOCK_AIRPORT_BUSY, 35),
    (35, 0, BLOCK_AIRPORT_BUSY, 36),
    (36, 16, BLOCK_IN_WAY, 36),
    (36, 255, BLOCK_TERM_GROUP1, 0),
    (36, 255, BLOCK_TERM_GROUP2_ENTER1, 1),
    (36, 5, 0, 12),
    (36, 6, 0, 12),
    (36, 7, 0, 12),
    (36, 0, 0, 2),
    (37, 0, 0, 38),
    (38, 0, 0, 39),
    (39, 0, 0, 40),
    (40, 0, 0, 32),
    (41, 17, BLOCK_PRE_HELIPAD, 42),
    (42, 18, BLOCK_PRE_HELIPAD, 42),
    (42, 8, 0, 43),
    (42, 9, 0, 44),
    (42, 1, 0, 49),
    (43, 0, 0, 45),
    (44, 0, 0, 46),
    (45, 255, 0, 0),
    (45, 8, BLOCK_HELIPAD1, 10),
    (46, 255, 0, 0),
    (46, 9, BLOCK_HELIPAD2, 11),
    (47, 13, 0, 0),
    (48, 13, 0, 0),
    (49, 0, BLOCK_HANGAR2_AREA, 50),
    (50, 0, BLOCK_HANGAR2_AREA, 3),
    (51, 13, 0, 0),
    (52, 13, 0, 0),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn international_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    INTERNATIONAL_FTA_BUILDUP
        .iter()
        .filter(|(p, _, _, _)| *p == pos)
        .map(
            |&(position, heading, blocks, next_position)| AirportFtaEdge {
                position,
                heading: AirportHeading::from_u8(heading),
                blocks,
                next_position,
            },
        )
        .collect()
}
