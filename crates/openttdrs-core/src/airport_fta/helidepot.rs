//! Tablas Helidepot (`_airport_moving_data_helidepot`, `_airport_fta_helidepot`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_HANGAR2_AREA,
    BLOCK_HELIPAD1, BLOCK_PRE_HELIPAD, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE,
    FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_E: u8 = 2;
const DIR_SW: u8 = 5;
const DIR_NW: u8 = 7;

/// Número de waypoints Helidepot.
pub const HELIDEPOT_NOF_ELEMENTS: usize = 18;

/// Entradas de holding (`_airport_entries_helidepot`).
pub const HELIDEPOT_ENTRIES: [u8; 4] = [4, 4, 4, 4];

/// `_airport_moving_data_helidepot[18]`.
pub static HELIDEPOT_MOVING_DATA: [AirportMovingData; HELIDEPOT_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 24,
        y: 4,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 0 hangar
    AirportMovingData {
        x: 24,
        y: 28,
        flags: 0,
        direction: DIR_N,
    }, // 1 outside depot
    AirportMovingData {
        x: 5,
        y: 38,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 2 flying
    AirportMovingData {
        x: -15,
        y: -15,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 3 circle NE
    AirportMovingData {
        x: -15,
        y: -49,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 4 circle NW (entry)
    AirportMovingData {
        x: 49,
        y: -49,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 5 circle SW
    AirportMovingData {
        x: 49,
        y: -15,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 6 circle SE
    AirportMovingData {
        x: 8,
        y: 32,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_NW,
    }, // 7 PreHelipad
    AirportMovingData {
        x: 8,
        y: 32,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_NW,
    }, // 8 Helipad air
    AirportMovingData {
        x: 8,
        y: 16,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_NW,
    }, // 9 Land approach
    AirportMovingData {
        x: 8,
        y: 16,
        flags: FLAG_HELI_LOWER,
        direction: DIR_NW,
    }, // 10 Land lower
    AirportMovingData {
        x: 8,
        y: 24,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 11 Takeoff raise
    AirportMovingData {
        x: 32,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_NW,
    }, // 12 air to hangar
    AirportMovingData {
        x: 32,
        y: 24,
        flags: FLAG_HELI_LOWER,
        direction: DIR_NW,
    }, // 13 lower to hangar area
    AirportMovingData {
        x: 8,
        y: 24,
        flags: FLAG_EXACT,
        direction: DIR_NW,
    }, // 14 on helipad1
    AirportMovingData {
        x: 24,
        y: 28,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 15 takeoff outside depot
    AirportMovingData {
        x: 8,
        y: 24,
        flags: FLAG_HELI_RAISE,
        direction: DIR_SW,
    }, // 16 takeoff from pad air
    AirportMovingData {
        x: 8,
        y: 24,
        flags: FLAG_SLOW_TURN | FLAG_EXACT,
        direction: DIR_E,
    }, // 17 turn on pad for takeoff
];

/// Filas `_airport_fta_helidepot` (sin marcador final).
static HELIDEPOT_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 1), // HANGAR
    (1, 255, BLOCK_HANGAR2_AREA, 0),
    (1, 1, 0, 0),
    (1, 8, BLOCK_HELIPAD1, 14), // HELIPAD1
    (1, 13, 0, 15),             // HELITAKEOFF
    (1, 0, 0, 0),
    (2, 14, 0, 3),                 // FLYING
    (2, 17, BLOCK_PRE_HELIPAD, 7), // HELILANDING
    (2, 1, 0, 12),                 // HANGAR
    (2, 13, 0, 16),                // HELITAKEOFF
    (3, 0, 0, 4),
    (4, 0, 0, 5),
    (5, 0, 0, 6),
    (6, 0, 0, 2),
    (7, 17, BLOCK_PRE_HELIPAD, 8), // HELILANDING
    (8, 18, BLOCK_PRE_HELIPAD, 8), // HELIENDLANDING
    (8, 8, 0, 9),                  // HELIPAD1
    (8, 1, 0, 12),                 // HANGAR
    (8, 0, 0, 2),
    (9, 0, 0, 10),
    (10, 255, 0, 10),
    (10, 8, BLOCK_HELIPAD1, 14), // HELIPAD1
    (10, 1, 0, 1),               // HANGAR
    (10, 0, 0, 14),
    (11, 13, 0, 0), // HELITAKEOFF → fly
    (12, 0, BLOCK_HANGAR2_AREA, 13),
    (13, 0, BLOCK_HANGAR2_AREA, 1),
    (14, 8, BLOCK_HELIPAD1, 14), // HELIPAD1 stay
    (14, 1, 0, 1),               // HANGAR
    (14, 13, 0, 17),             // HELITAKEOFF
    (15, 13, 0, 0),              // HELITAKEOFF → fly
    (16, 13, 0, 14),
    (17, 0, 0, 11),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn helidepot_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    HELIDEPOT_FTA_BUILDUP
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
