//! Tablas Heliport (`_airport_moving_data_heliport`, `_airport_fta_heliport`).
//!
//! Oilrig reutiliza las mismas aristas FTA (`_airport_fta_oilrig`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_HELIPAD1, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_NO_SPEED_CLAMP,
    FLAG_SLOW_TURN,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;

/// Número de waypoints Heliport / Oilrig.
pub const HELIPORT_NOF_ELEMENTS: usize = 9;

/// Entradas de holding (`_airport_entries_heliport` / oilrig).
pub const HELIPORT_ENTRIES: [u8; 4] = [7, 7, 7, 7];

/// `_airport_moving_data_heliport[9]`.
pub static HELIPORT_MOVING_DATA: [AirportMovingData; HELIPORT_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 5,
        y: 9,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 0 pad
    AirportMovingData {
        x: 2,
        y: 9,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 1 takeoff
    AirportMovingData {
        x: -3,
        y: 9,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 2 above
    AirportMovingData {
        x: -3,
        y: 9,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 3 land
    AirportMovingData {
        x: 2,
        y: 9,
        flags: 0,
        direction: DIR_N,
    }, // 4 to terminal
    AirportMovingData {
        x: -31,
        y: 59,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 5 hold NE
    AirportMovingData {
        x: -31,
        y: -49,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 6 hold NW
    AirportMovingData {
        x: 49,
        y: -49,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 7 hold SW
    AirportMovingData {
        x: 70,
        y: 9,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 8 hold S
];

/// Filas `_airport_fta_heliport` (sin marcador final). También Oilrig.
static HELIPORT_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 8, BLOCK_HELIPAD1, 1),
    (1, 13, 0, 0),
    (2, 255, BLOCK_AIRPORT_BUSY, 0),
    (2, 17, 0, 3),
    (2, 13, 0, 1),
    (3, 17, BLOCK_AIRPORT_BUSY, 4),
    (4, 18, BLOCK_AIRPORT_BUSY, 4),
    (4, 8, BLOCK_HELIPAD1, 0),
    (4, 13, 0, 2),
    (5, 0, 0, 6),
    (6, 0, 0, 7),
    (7, 0, 0, 8),
    (8, 14, 0, 5),
    (8, 17, BLOCK_HELIPAD1, 2),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn heliport_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    HELIPORT_FTA_BUILDUP
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
