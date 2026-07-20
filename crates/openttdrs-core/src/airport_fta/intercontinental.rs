//! Tablas Intercontinental (`_airport_moving_data_intercontinental`, `_airport_fta_intercontinental`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_HANGAR1_AREA, BLOCK_HANGAR2_AREA, BLOCK_HELIPAD1, BLOCK_HELIPAD2, BLOCK_IN_WAY,
    BLOCK_IN_WAY2, BLOCK_OUT_WAY, BLOCK_OUT_WAY2, BLOCK_OUT_WAY3, BLOCK_PRE_HELIPAD,
    BLOCK_RUNWAY_IN2, BLOCK_RUNWAY_OUT, BLOCK_TAXIWAY_BUSY, BLOCK_TERM_GROUP1, BLOCK_TERM_GROUP2,
    BLOCK_TERM_GROUP2_ENTER1, BLOCK_TERM_GROUP2_ENTER2, BLOCK_TERM_GROUP2_EXIT1,
    BLOCK_TERM_GROUP2_EXIT2, BLOCK_TERM1, BLOCK_TERM2, BLOCK_TERM3, BLOCK_TERM4, BLOCK_TERM5,
    BLOCK_TERM6, BLOCK_TERM7, BLOCK_TERM8, FLAG_BRAKE, FLAG_EXACT, FLAG_HELI_LOWER,
    FLAG_HELI_RAISE, FLAG_LAND, FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN, FLAG_TAKEOFF,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;
const DIR_SW: u8 = 5;
const DIR_W: u8 = 6;
const DIR_NW: u8 = 7;

/// Número de waypoints Intercontinental.
pub const INTERCONTINENTAL_NOF_ELEMENTS: usize = 77;

/// Entradas de holding (`_airport_entries_intercontinental`).
pub const INTERCONTINENTAL_ENTRIES: [u8; 4] = [44, 43, 46, 45];

/// `_airport_moving_data_intercontinental[77]`.
pub static INTERCONTINENTAL_MOVING_DATA: [AirportMovingData; INTERCONTINENTAL_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 8,
        y: 87,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0
    AirportMovingData {
        x: 136,
        y: 72,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 1
    AirportMovingData {
        x: 8,
        y: 104,
        flags: 0,
        direction: DIR_N,
    }, // 2
    AirportMovingData {
        x: 136,
        y: 88,
        flags: 0,
        direction: DIR_N,
    }, // 3
    AirportMovingData {
        x: 56,
        y: 120,
        flags: FLAG_EXACT,
        direction: DIR_W,
    }, // 4
    AirportMovingData {
        x: 56,
        y: 104,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 5
    AirportMovingData {
        x: 56,
        y: 88,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 6
    AirportMovingData {
        x: 56,
        y: 72,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 7
    AirportMovingData {
        x: 88,
        y: 120,
        flags: FLAG_EXACT,
        direction: DIR_N,
    }, // 8
    AirportMovingData {
        x: 88,
        y: 104,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 9
    AirportMovingData {
        x: 88,
        y: 88,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 10
    AirportMovingData {
        x: 88,
        y: 72,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 11
    AirportMovingData {
        x: 88,
        y: 56,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 12
    AirportMovingData {
        x: 72,
        y: 56,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 13
    AirportMovingData {
        x: 40,
        y: 136,
        flags: 0,
        direction: DIR_N,
    }, // 14
    AirportMovingData {
        x: 56,
        y: 136,
        flags: 0,
        direction: DIR_N,
    }, // 15
    AirportMovingData {
        x: 88,
        y: 136,
        flags: 0,
        direction: DIR_N,
    }, // 16
    AirportMovingData {
        x: 104,
        y: 136,
        flags: 0,
        direction: DIR_N,
    }, // 17
    AirportMovingData {
        x: 104,
        y: 120,
        flags: 0,
        direction: DIR_N,
    }, // 18
    AirportMovingData {
        x: 104,
        y: 104,
        flags: 0,
        direction: DIR_N,
    }, // 19
    AirportMovingData {
        x: 104,
        y: 88,
        flags: 0,
        direction: DIR_N,
    }, // 20
    AirportMovingData {
        x: 104,
        y: 72,
        flags: 0,
        direction: DIR_N,
    }, // 21
    AirportMovingData {
        x: 104,
        y: 56,
        flags: 0,
        direction: DIR_N,
    }, // 22
    AirportMovingData {
        x: 104,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 23
    AirportMovingData {
        x: 56,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 24
    AirportMovingData {
        x: 40,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 25
    AirportMovingData {
        x: 40,
        y: 120,
        flags: 0,
        direction: DIR_N,
    }, // 26
    AirportMovingData {
        x: 40,
        y: 104,
        flags: 0,
        direction: DIR_N,
    }, // 27
    AirportMovingData {
        x: 40,
        y: 88,
        flags: 0,
        direction: DIR_N,
    }, // 28
    AirportMovingData {
        x: 40,
        y: 72,
        flags: 0,
        direction: DIR_N,
    }, // 29
    AirportMovingData {
        x: 18,
        y: 72,
        flags: 0,
        direction: DIR_NW,
    }, // 30
    AirportMovingData {
        x: 8,
        y: 40,
        flags: 0,
        direction: DIR_NW,
    }, // 31
    AirportMovingData {
        x: 8,
        y: 24,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 32
    AirportMovingData {
        x: 119,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 33
    AirportMovingData {
        x: 117,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 34
    AirportMovingData {
        x: 197,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 35
    AirportMovingData {
        x: 254,
        y: 84,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 36
    AirportMovingData {
        x: 117,
        y: 168,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_LAND,
        direction: DIR_N,
    }, // 37
    AirportMovingData {
        x: 8,
        y: 168,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 38
    AirportMovingData {
        x: 8,
        y: 168,
        flags: 0,
        direction: DIR_N,
    }, // 39
    AirportMovingData {
        x: 8,
        y: 144,
        flags: 0,
        direction: DIR_NW,
    }, // 40
    AirportMovingData {
        x: 8,
        y: 128,
        flags: 0,
        direction: DIR_NW,
    }, // 41
    AirportMovingData {
        x: 8,
        y: 120,
        flags: FLAG_EXACT,
        direction: DIR_NW,
    }, // 42
    AirportMovingData {
        x: 56,
        y: 344,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 43
    AirportMovingData {
        x: -200,
        y: 88,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 44
    AirportMovingData {
        x: 56,
        y: -168,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 45
    AirportMovingData {
        x: 312,
        y: 88,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 46
    AirportMovingData {
        x: 96,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 47
    AirportMovingData {
        x: 96,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 48
    AirportMovingData {
        x: 82,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 49
    AirportMovingData {
        x: 64,
        y: 56,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 50
    AirportMovingData {
        x: 81,
        y: 55,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 51
    AirportMovingData {
        x: 64,
        y: 56,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 52
    AirportMovingData {
        x: 80,
        y: 56,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 53
    AirportMovingData {
        x: 64,
        y: 56,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 54
    AirportMovingData {
        x: 136,
        y: 96,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 55
    AirportMovingData {
        x: 136,
        y: 96,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 56
    AirportMovingData {
        x: 126,
        y: 104,
        flags: 0,
        direction: DIR_SE,
    }, // 57
    AirportMovingData {
        x: 136,
        y: 136,
        flags: 0,
        direction: DIR_NE,
    }, // 58
    AirportMovingData {
        x: 136,
        y: 152,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 59
    AirportMovingData {
        x: 16,
        y: 152,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 60
    AirportMovingData {
        x: 20,
        y: 152,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 61
    AirportMovingData {
        x: -56,
        y: 152,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 62
    AirportMovingData {
        x: 24,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_LAND,
        direction: DIR_N,
    }, // 63
    AirportMovingData {
        x: 136,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 64
    AirportMovingData {
        x: 136,
        y: 8,
        flags: 0,
        direction: DIR_N,
    }, // 65
    AirportMovingData {
        x: 136,
        y: 24,
        flags: 0,
        direction: DIR_SE,
    }, // 66
    AirportMovingData {
        x: 136,
        y: 40,
        flags: 0,
        direction: DIR_SE,
    }, // 67
    AirportMovingData {
        x: 136,
        y: 56,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 68
    AirportMovingData {
        x: -56,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 69
    AirportMovingData {
        x: 88,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 70
    AirportMovingData {
        x: 72,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 71
    AirportMovingData {
        x: 88,
        y: 57,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 72
    AirportMovingData {
        x: 71,
        y: 56,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 73
    AirportMovingData {
        x: 8,
        y: 120,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 74
    AirportMovingData {
        x: 136,
        y: 104,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 75
    AirportMovingData {
        x: 197,
        y: 168,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 76
];

/// Filas `_airport_fta_intercontinental` (sin marcador final).
static INTERCONTINENTAL_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 2),
    (0, 255, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 0),
    (0, 255, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 1),
    (0, 10, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 2),
    (0, 0, 0, 2),
    (1, 1, 0, 3),
    (1, 255, BLOCK_HANGAR2_AREA, 1),
    (1, 255, BLOCK_HANGAR2_AREA, 0),
    (1, 0, 0, 3),
    (2, 255, BLOCK_HANGAR1_AREA, 0),
    (2, 255, BLOCK_TERM_GROUP1, 0),
    (2, 255, BLOCK_TERM_GROUP1, 1),
    (2, 1, 0, 0),
    (2, 10, BLOCK_TERM_GROUP1, 27),
    (2, 6, 0, 26),
    (2, 7, 0, 26),
    (2, 19, 0, 26),
    (2, 20, 0, 26),
    (2, 8, 0, 26),
    (2, 9, 0, 26),
    (2, 13, 0, 74),
    (2, 0, 0, 27),
    (3, 255, BLOCK_HANGAR2_AREA, 0),
    (3, 1, 0, 1),
    (3, 13, 0, 75),
    (3, 10, 0, 59),
    (3, 0, 0, 20),
    (4, 2, BLOCK_TERM1, 26),
    (4, 1, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 26),
    (4, 0, 0, 26),
    (5, 3, BLOCK_TERM2, 27),
    (5, 1, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 27),
    (5, 0, 0, 27),
    (6, 4, BLOCK_TERM3, 28),
    (6, 1, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 28),
    (6, 0, 0, 28),
    (7, 5, BLOCK_TERM4, 29),
    (7, 1, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 29),
    (7, 0, 0, 29),
    (8, 6, BLOCK_TERM5, 18),
    (8, 1, BLOCK_HANGAR2_AREA, 18),
    (8, 0, 0, 18),
    (9, 7, BLOCK_TERM6, 19),
    (9, 1, BLOCK_HANGAR2_AREA, 19),
    (9, 0, 0, 19),
    (10, 19, BLOCK_TERM7, 20),
    (10, 1, BLOCK_HANGAR2_AREA, 20),
    (10, 0, 0, 20),
    (11, 20, BLOCK_TERM8, 21),
    (11, 1, BLOCK_HANGAR2_AREA, 21),
    (11, 0, 0, 21),
    (12, 8, BLOCK_HELIPAD1, 12),
    (12, 1, 0, 70),
    (12, 13, 0, 72),
    (13, 9, BLOCK_HELIPAD2, 13),
    (13, 1, 0, 71),
    (13, 13, 0, 73),
    (14, 0, BLOCK_TERM_GROUP2_ENTER1, 15),
    (15, 0, BLOCK_TERM_GROUP2_ENTER1, 16),
    (16, 0, BLOCK_TERM_GROUP2_ENTER2, 17),
    (17, 0, BLOCK_TERM_GROUP2_ENTER2, 18),
    (18, 255, BLOCK_TERM_GROUP2, 0),
    (18, 6, BLOCK_TERM5, 8),
    (18, 10, 0, 19),
    (18, 13, BLOCK_HELIPAD1, 19),
    (18, 0, BLOCK_TERM_GROUP2_EXIT1, 19),
    (19, 255, BLOCK_TERM_GROUP2, 0),
    (19, 7, BLOCK_TERM6, 9),
    (19, 6, 0, 18),
    (19, 10, 0, 57),
    (19, 13, BLOCK_HELIPAD1, 20),
    (19, 0, BLOCK_TERM_GROUP2_EXIT1, 20),
    (20, 255, BLOCK_TERM_GROUP2, 0),
    (20, 19, BLOCK_TERM7, 10),
    (20, 6, 0, 19),
    (20, 7, 0, 19),
    (20, 1, BLOCK_HANGAR2_AREA, 3),
    (20, 10, 0, 19),
    (20, 0, BLOCK_TERM_GROUP2_EXIT1, 21),
    (21, 255, BLOCK_TERM_GROUP2, 0),
    (21, 20, BLOCK_TERM8, 11),
    (21, 1, BLOCK_HANGAR2_AREA, 20),
    (21, 6, 0, 20),
    (21, 7, 0, 20),
    (21, 19, 0, 20),
    (21, 10, 0, 20),
    (21, 0, BLOCK_TERM_GROUP2_EXIT1, 22),
    (22, 255, BLOCK_TERM_GROUP2, 0),
    (22, 1, 0, 21),
    (22, 6, 0, 21),
    (22, 7, 0, 21),
    (22, 19, 0, 21),
    (22, 20, 0, 21),
    (22, 10, 0, 21),
    (22, 0, 0, 23),
    (23, 0, BLOCK_TERM_GROUP2_EXIT1, 70),
    (24, 0, BLOCK_TERM_GROUP2_EXIT2, 25),
    (25, 255, BLOCK_TERM_GROUP2_EXIT2, 0),
    (25, 1, BLOCK_TERM_GROUP1 | BLOCK_HANGAR1_AREA, 29),
    (25, 0, 0, 29),
    (26, 255, BLOCK_TERM_GROUP1, 0),
    (26, 2, BLOCK_TERM1, 4),
    (26, 1, BLOCK_HANGAR1_AREA, 27),
    (26, 6, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 7, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 19, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 20, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 8, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 9, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 13, BLOCK_TERM_GROUP2_ENTER1, 14),
    (26, 0, 0, 27),
    (27, 255, BLOCK_TERM_GROUP1, 0),
    (27, 3, BLOCK_TERM2, 5),
    (27, 1, BLOCK_HANGAR1_AREA, 2),
    (27, 2, 0, 26),
    (27, 6, 0, 26),
    (27, 7, 0, 26),
    (27, 19, 0, 26),
    (27, 20, 0, 26),
    (27, 8, 0, 14),
    (27, 9, 0, 14),
    (27, 0, 0, 28),
    (28, 255, BLOCK_TERM_GROUP1, 0),
    (28, 4, BLOCK_TERM3, 6),
    (28, 1, BLOCK_HANGAR1_AREA, 27),
    (28, 2, 0, 27),
    (28, 3, 0, 27),
    (28, 5, 0, 29),
    (28, 6, 0, 14),
    (28, 7, 0, 14),
    (28, 19, 0, 14),
    (28, 20, 0, 14),
    (28, 8, 0, 14),
    (28, 9, 0, 14),
    (28, 0, 0, 29),
    (29, 255, BLOCK_TERM_GROUP1, 0),
    (29, 5, BLOCK_TERM4, 7),
    (29, 1, BLOCK_HANGAR1_AREA, 27),
    (29, 10, 0, 30),
    (29, 0, 0, 28),
    (30, 0, BLOCK_OUT_WAY3, 31),
    (31, 0, BLOCK_OUT_WAY, 32),
    (32, 10, BLOCK_RUNWAY_OUT, 33),
    (33, 0, BLOCK_RUNWAY_OUT, 34),
    (34, 11, 0, 35),
    (35, 12, 0, 0),
    (36, 0, 0, 0),
    (37, 15, BLOCK_AIRPORT_BUSY, 38),
    (38, 0, BLOCK_AIRPORT_BUSY, 39),
    (39, 0, BLOCK_AIRPORT_BUSY, 40),
    (40, 16, BLOCK_AIRPORT_BUSY, 41),
    (41, 0, BLOCK_IN_WAY, 42),
    (42, 255, BLOCK_IN_WAY, 0),
    (42, 255, BLOCK_TERM_GROUP1, 0),
    (42, 255, BLOCK_TERM_GROUP1, 1),
    (42, 1, 0, 2),
    (42, 0, 0, 26),
    (43, 0, 0, 44),
    (44, 14, 0, 45),
    (44, 17, 0, 47),
    (44, 15, 0, 69),
    (44, 0, 0, 45),
    (45, 0, 0, 46),
    (46, 14, 0, 43),
    (46, 15, 0, 76),
    (46, 0, 0, 43),
    (47, 17, BLOCK_PRE_HELIPAD, 48),
    (48, 18, BLOCK_PRE_HELIPAD, 48),
    (48, 8, 0, 49),
    (48, 9, 0, 50),
    (48, 1, 0, 55),
    (49, 0, 0, 51),
    (50, 0, 0, 52),
    (51, 255, 0, 0),
    (51, 8, BLOCK_HELIPAD1, 12),
    (51, 1, 0, 55),
    (51, 0, 0, 12),
    (52, 255, 0, 0),
    (52, 9, BLOCK_HELIPAD2, 13),
    (52, 1, 0, 55),
    (52, 0, 0, 13),
    (53, 13, 0, 0),
    (54, 13, 0, 0),
    (55, 0, BLOCK_HANGAR2_AREA, 56),
    (56, 0, BLOCK_HANGAR2_AREA, 3),
    (57, 255, BLOCK_OUT_WAY2, 0),
    (57, 10, 0, 58),
    (57, 0, 0, 58),
    (58, 0, BLOCK_OUT_WAY2, 59),
    (59, 10, BLOCK_TAXIWAY_BUSY, 60),
    (60, 0, BLOCK_TAXIWAY_BUSY, 61),
    (61, 11, 0, 62),
    (62, 12, 0, 0),
    (63, 15, BLOCK_RUNWAY_IN2, 64),
    (64, 0, BLOCK_RUNWAY_IN2, 65),
    (65, 0, BLOCK_RUNWAY_IN2, 66),
    (66, 16, BLOCK_RUNWAY_IN2, 0),
    (66, 255, 0, 1),
    (66, 255, 0, 0),
    (66, 0, 0, 67),
    (67, 0, BLOCK_IN_WAY2, 68),
    (68, 255, BLOCK_IN_WAY2, 0),
    (68, 255, BLOCK_TERM_GROUP2, 1),
    (68, 255, BLOCK_TERM_GROUP1, 0),
    (68, 1, BLOCK_HANGAR2_AREA, 22),
    (68, 0, 0, 22),
    (69, 255, BLOCK_RUNWAY_IN2, 0),
    (69, 0, BLOCK_RUNWAY_IN2, 63),
    (70, 255, BLOCK_TERM_GROUP2_EXIT1, 0),
    (70, 8, BLOCK_HELIPAD1, 12),
    (70, 13, BLOCK_HELIPAD1, 12),
    (70, 0, 0, 71),
    (71, 255, BLOCK_TERM_GROUP2_EXIT1, 0),
    (71, 9, BLOCK_HELIPAD2, 13),
    (71, 13, BLOCK_HELIPAD1, 12),
    (71, 0, 0, 24),
    (72, 0, BLOCK_HELIPAD1, 53),
    (73, 0, BLOCK_HELIPAD2, 54),
    (74, 13, 0, 0),
    (75, 13, 0, 0),
    (76, 255, BLOCK_AIRPORT_BUSY, 0),
    (76, 0, BLOCK_AIRPORT_BUSY, 37),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn intercontinental_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    INTERCONTINENTAL_FTA_BUILDUP
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
