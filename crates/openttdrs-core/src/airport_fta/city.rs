//! Tablas City (`_airport_moving_data_city`, `_airport_fta_city`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_IN_WAY, BLOCK_OUT_WAY, BLOCK_TAXIWAY_BUSY, BLOCK_TERM1, BLOCK_TERM2, BLOCK_TERM3,
    FLAG_BRAKE, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_HOLD, FLAG_LAND,
    FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN, FLAG_TAKEOFF,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;
const DIR_SW: u8 = 5;

/// Número de waypoints City.
pub const CITY_NOF_ELEMENTS: usize = 30;

/// Entradas de holding (`_airport_entries_city`).
pub const CITY_ENTRIES: [u8; 4] = [26, 29, 27, 28];

/// `_airport_moving_data_city[30]`.
pub static CITY_MOVING_DATA: [AirportMovingData; CITY_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 85,
        y: 3,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0 hangar
    AirportMovingData {
        x: 85,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 1 outside
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
        x: 44,
        y: 63,
        flags: 0,
        direction: DIR_N,
    }, // 7 center
    AirportMovingData {
        x: 58,
        y: 71,
        flags: 0,
        direction: DIR_N,
    }, // 8 towards takeoff
    AirportMovingData {
        x: 72,
        y: 85,
        flags: 0,
        direction: DIR_N,
    }, // 9 to runway
    AirportMovingData {
        x: 89,
        y: 85,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 10 runway start
    AirportMovingData {
        x: 3,
        y: 85,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 11 accelerate
    AirportMovingData {
        x: -79,
        y: 85,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 12 takeoff
    AirportMovingData {
        x: 177,
        y: 87,
        flags: FLAG_HOLD | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 13 approach
    AirportMovingData {
        x: 89,
        y: 87,
        flags: FLAG_HOLD | FLAG_LAND,
        direction: DIR_N,
    }, // 14 land
    AirportMovingData {
        x: 20,
        y: 87,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 15 brake
    AirportMovingData {
        x: 20,
        y: 87,
        flags: 0,
        direction: DIR_N,
    }, // 16 unused
    AirportMovingData {
        x: 36,
        y: 71,
        flags: 0,
        direction: DIR_N,
    }, // 17 taxi from runway
    AirportMovingData {
        x: 160,
        y: 87,
        flags: FLAG_HOLD | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 18 hold NE
    AirportMovingData {
        x: 140,
        y: 1,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 19 FAF
    AirportMovingData {
        x: 257,
        y: 1,
        flags: FLAG_HOLD | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 20 hold SW
    AirportMovingData {
        x: 273,
        y: 49,
        flags: FLAG_HOLD | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 21 hold S
    AirportMovingData {
        x: 44,
        y: 63,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 22 heli takeoff
    AirportMovingData {
        x: 28,
        y: 74,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 23 heli above
    AirportMovingData {
        x: 28,
        y: 74,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 24 heli land
    AirportMovingData {
        x: 145,
        y: 1,
        flags: FLAG_HOLD | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 25 hold NW
    AirportMovingData {
        x: -32,
        y: 1,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 26 IAF N
    AirportMovingData {
        x: 300,
        y: -48,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 27 IAF S
    AirportMovingData {
        x: 140,
        y: -48,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 28 IAF W
    AirportMovingData {
        x: -32,
        y: 120,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 29 IAF E
];

/// Filas `_airport_fta_city` (sin marcador final).
static CITY_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 1), // HANGAR
    (0, 10, BLOCK_OUT_WAY, 1),
    (0, 0, 0, 1),
    (1, 255, BLOCK_TAXIWAY_BUSY, 0),
    (1, 1, 0, 0),
    (1, 3, 0, 6),
    (1, 4, 0, 6),
    (1, 0, 0, 7),
    (2, 2, BLOCK_TERM1, 7),
    (2, 10, BLOCK_OUT_WAY, 7),
    (2, 0, 0, 7),
    (3, 3, BLOCK_TERM2, 5),
    (3, 10, BLOCK_OUT_WAY, 6),
    (3, 0, 0, 6),
    (4, 4, BLOCK_TERM3, 5),
    (4, 10, BLOCK_OUT_WAY, 5),
    (4, 0, 0, 5),
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
    (7, 10, BLOCK_OUT_WAY, 8),
    (7, 13, 0, 22),
    (7, 1, 0, 1),
    (7, 0, 0, 6),
    (8, 0, BLOCK_OUT_WAY, 9),
    (9, 0, BLOCK_AIRPORT_BUSY, 10),
    (10, 10, BLOCK_AIRPORT_BUSY, 11),
    (11, 11, 0, 12),
    (12, 12, 0, 0),
    (13, 14, 0, 18),
    (13, 15, 0, 14),
    (13, 17, 0, 23),
    (14, 15, BLOCK_AIRPORT_BUSY, 15),
    (15, 0, BLOCK_AIRPORT_BUSY, 17),
    (16, 0, BLOCK_AIRPORT_BUSY, 17),
    (17, 16, BLOCK_IN_WAY, 7),
    (18, 0, 0, 25),
    (19, 0, 0, 20),
    (20, 0, 0, 21),
    (21, 0, 0, 13),
    (22, 13, 0, 0),
    (23, 17, BLOCK_IN_WAY, 24),
    (24, 18, BLOCK_IN_WAY, 17),
    (25, 0, 0, 20),
    (26, 0, 0, 19),
    (27, 0, 0, 28),
    (28, 0, 0, 19),
    (29, 0, 0, 26),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn city_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    CITY_FTA_BUILDUP
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
