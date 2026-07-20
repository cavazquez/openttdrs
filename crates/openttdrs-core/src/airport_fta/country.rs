//! Tablas Country / Small (`_airport_moving_data_country`, `_airport_fta_country`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_TERM1, BLOCK_TERM2, FLAG_BRAKE, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_LAND,
    FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN, FLAG_TAKEOFF,
};

/// Direcciones `OpenTTD` usadas en `MovingData` Country.
const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;
const DIR_NW: u8 = 7;

/// Número de waypoints Country.
pub const COUNTRY_NOF_ELEMENTS: usize = 22;

/// Entradas de holding según dirección de llegada NE,NW,SW,SE (`_airport_entries_country`).
pub const COUNTRY_ENTRIES: [u8; 4] = [16, 15, 18, 17];

/// `_airport_moving_data_country[22]`.
pub static COUNTRY_MOVING_DATA: [AirportMovingData; COUNTRY_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 53,
        y: 3,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0 hangar
    AirportMovingData {
        x: 53,
        y: 27,
        flags: 0,
        direction: DIR_N,
    }, // 1 outside depot
    AirportMovingData {
        x: 32,
        y: 23,
        flags: FLAG_EXACT,
        direction: DIR_NW,
    }, // 2 term1
    AirportMovingData {
        x: 10,
        y: 23,
        flags: FLAG_EXACT,
        direction: DIR_NW,
    }, // 3 term2
    AirportMovingData {
        x: 43,
        y: 37,
        flags: 0,
        direction: DIR_N,
    }, // 4
    AirportMovingData {
        x: 24,
        y: 37,
        flags: 0,
        direction: DIR_N,
    }, // 5
    AirportMovingData {
        x: 53,
        y: 37,
        flags: 0,
        direction: DIR_N,
    }, // 6 for takeoff
    AirportMovingData {
        x: 61,
        y: 40,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 7 runway start
    AirportMovingData {
        x: 3,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 8 accelerate
    AirportMovingData {
        x: -79,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 9 takeoff
    AirportMovingData {
        x: 177,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 10 approach
    AirportMovingData {
        x: 56,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_LAND,
        direction: DIR_N,
    }, // 11 land
    AirportMovingData {
        x: 3,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 12 brake
    AirportMovingData {
        x: 7,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 13 turn
    AirportMovingData {
        x: 53,
        y: 40,
        flags: 0,
        direction: DIR_N,
    }, // 14 taxi from runway
    AirportMovingData {
        x: 1,
        y: 193,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 15 hold NE
    AirportMovingData {
        x: 1,
        y: 1,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 16 hold NW
    AirportMovingData {
        x: 257,
        y: 1,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 17 hold SW
    AirportMovingData {
        x: 273,
        y: 47,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 18 hold S
    AirportMovingData {
        x: 44,
        y: 37,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 19 heli takeoff
    AirportMovingData {
        x: 44,
        y: 40,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 20 heli above
    AirportMovingData {
        x: 44,
        y: 40,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 21 heli land
];

/// Filas `_airport_fta_country` (sin marcador final).
static COUNTRY_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 1), // HANGAR
    (1, 255, BLOCK_AIRPORT_BUSY, 0),
    (1, 1, 0, 0),
    (1, 2, BLOCK_TERM1, 2),
    (1, 3, 0, 4),
    (1, 13, 0, 19),
    (1, 0, 0, 6),
    (2, 2, BLOCK_TERM1, 1),
    (3, 3, BLOCK_TERM2, 5),
    (4, 255, BLOCK_AIRPORT_BUSY, 0),
    (4, 3, 0, 5),
    (4, 1, 0, 1),
    (4, 10, 0, 6),
    (4, 13, 0, 1),
    (5, 255, BLOCK_AIRPORT_BUSY, 0),
    (5, 3, BLOCK_TERM2, 3),
    (5, 0, 0, 4),
    (6, 0, BLOCK_AIRPORT_BUSY, 7),
    (7, 10, BLOCK_AIRPORT_BUSY, 8),
    (8, 11, 0, 9),
    (9, 12, 0, 0),
    (10, 14, 0, 15),
    (10, 15, 0, 11),
    (10, 17, 0, 20),
    (11, 15, BLOCK_AIRPORT_BUSY, 12),
    (12, 0, BLOCK_AIRPORT_BUSY, 13),
    (13, 16, BLOCK_AIRPORT_BUSY, 14),
    (13, 3, 0, 5),
    (13, 0, 0, 14),
    (14, 0, BLOCK_AIRPORT_BUSY, 1),
    (15, 0, 0, 16),
    (16, 0, 0, 17),
    (17, 0, 0, 18),
    (18, 0, 0, 10),
    (19, 13, 0, 0),
    (20, 17, BLOCK_AIRPORT_BUSY, 21),
    (21, 18, BLOCK_AIRPORT_BUSY, 1),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn country_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    COUNTRY_FTA_BUILDUP
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
