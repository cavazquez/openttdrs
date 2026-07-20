//! Tablas Metropolitan (`_airport_moving_data_metropolitan`, `_airport_fta_metropolitan`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_IN_WAY, BLOCK_OUT_WAY, BLOCK_RUNWAY_OUT, BLOCK_TAXIWAY_BUSY, BLOCK_TERM1, BLOCK_TERM2,
    BLOCK_TERM3, FLAG_BRAKE, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_LAND,
    FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN, FLAG_TAKEOFF,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;
const DIR_SW: u8 = 5;

/// Número de waypoints Metropolitan.
pub const METROPOLITAN_NOF_ELEMENTS: usize = 28;

/// Entradas de holding (`_airport_entries_metropolitan`).
pub const METROPOLITAN_ENTRIES: [u8; 4] = [20, 19, 22, 21];

/// `_airport_moving_data_metropolitan[28]`.
pub static METROPOLITAN_MOVING_DATA: [AirportMovingData; METROPOLITAN_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 85,
        y: 3,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0
    AirportMovingData {
        x: 85,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 1
    AirportMovingData {
        x: 26,
        y: 41,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 2 term1
    AirportMovingData {
        x: 56,
        y: 22,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 3 term2
    AirportMovingData {
        x: 38,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 4 term3
    AirportMovingData {
        x: 65,
        y: 6,
        flags: 0,
        direction: DIR_N,
    }, // 5
    AirportMovingData {
        x: 80,
        y: 27,
        flags: 0,
        direction: DIR_N,
    }, // 6
    AirportMovingData {
        x: 49,
        y: 58,
        flags: 0,
        direction: DIR_N,
    }, // 7 center
    AirportMovingData {
        x: 72,
        y: 58,
        flags: 0,
        direction: DIR_N,
    }, // 8
    AirportMovingData {
        x: 72,
        y: 69,
        flags: 0,
        direction: DIR_N,
    }, // 9
    AirportMovingData {
        x: 89,
        y: 69,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 10 runway out start
    AirportMovingData {
        x: 3,
        y: 69,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 11
    AirportMovingData {
        x: -79,
        y: 69,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 12 takeoff
    AirportMovingData {
        x: 177,
        y: 85,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 13 approach
    AirportMovingData {
        x: 89,
        y: 85,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_LAND,
        direction: DIR_N,
    }, // 14 land (runway in)
    AirportMovingData {
        x: 3,
        y: 85,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 15 brake
    AirportMovingData {
        x: 21,
        y: 85,
        flags: 0,
        direction: DIR_N,
    }, // 16
    AirportMovingData {
        x: 21,
        y: 69,
        flags: 0,
        direction: DIR_N,
    }, // 17 runway-out taxi
    AirportMovingData {
        x: 21,
        y: 58,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 18
    AirportMovingData {
        x: 1,
        y: 193,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 19 hold NE
    AirportMovingData {
        x: 1,
        y: 1,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 20 hold NW
    AirportMovingData {
        x: 257,
        y: 1,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 21 hold SW
    AirportMovingData {
        x: 273,
        y: 49,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 22 hold S
    AirportMovingData {
        x: 44,
        y: 58,
        flags: 0,
        direction: DIR_N,
    }, // 23 heli ground
    AirportMovingData {
        x: 44,
        y: 63,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 24 heli takeoff
    AirportMovingData {
        x: 15,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 25 heli above
    AirportMovingData {
        x: 15,
        y: 54,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 26 heli land
    AirportMovingData {
        x: 21,
        y: 58,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 27 post-landing
];

/// Filas `_airport_fta_metropolitan` (sin marcador final).
static METROPOLITAN_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 1),
    (1, 255, BLOCK_TAXIWAY_BUSY, 0),
    (1, 1, 0, 0),
    (1, 3, 0, 6),
    (1, 4, 0, 6),
    (1, 0, 0, 7),
    (2, 2, BLOCK_TERM1, 7),
    (3, 3, BLOCK_TERM2, 6),
    (4, 4, BLOCK_TERM3, 5),
    (5, 255, BLOCK_TAXIWAY_BUSY, 0),
    (5, 3, BLOCK_TERM2, 3),
    (5, 4, BLOCK_TERM3, 4),
    (5, 0, 0, 6),
    (6, 255, BLOCK_TAXIWAY_BUSY, 0),
    (6, 3, BLOCK_TERM2, 3),
    (6, 4, 0, 5),
    (6, 1, 0, 1),
    (6, 0, 0, 7),
    (7, 255, BLOCK_TAXIWAY_BUSY, 0),
    (7, 2, BLOCK_TERM1, 2),
    (7, 10, 0, 8),
    (7, 13, 0, 23),
    (7, 1, 0, 1),
    (7, 0, 0, 6),
    (8, 0, BLOCK_OUT_WAY, 9),
    (9, 0, BLOCK_RUNWAY_OUT, 10),
    (10, 10, BLOCK_RUNWAY_OUT, 11),
    (11, 11, 0, 12),
    (12, 12, 0, 0),
    (13, 14, 0, 19),
    (13, 15, 0, 14),
    (13, 17, 0, 25),
    (14, 15, BLOCK_AIRPORT_BUSY, 15),
    (15, 0, BLOCK_AIRPORT_BUSY, 16),
    (16, 255, BLOCK_AIRPORT_BUSY, 0),
    (16, 16, BLOCK_IN_WAY, 17),
    (17, 255, BLOCK_RUNWAY_OUT, 0),
    (17, 16, BLOCK_IN_WAY, 18),
    (18, 16, BLOCK_IN_WAY, 27),
    (19, 0, 0, 20),
    (20, 0, 0, 21),
    (21, 0, 0, 22),
    (22, 0, 0, 13),
    (23, 0, 0, 24),
    (24, 13, 0, 0),
    (25, 17, BLOCK_IN_WAY, 26),
    (26, 18, BLOCK_IN_WAY, 18),
    (27, 255, BLOCK_TAXIWAY_BUSY, 27),
    (27, 2, BLOCK_TERM1, 2),
    (27, 0, 0, 7),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn metropolitan_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    METROPOLITAN_FTA_BUILDUP
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
