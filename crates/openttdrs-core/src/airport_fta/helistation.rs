//! Tablas Helistation (`_airport_moving_data_helistation`, `_airport_fta_helistation`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_HANGAR2_AREA,
    BLOCK_HELIPAD1, BLOCK_HELIPAD2, BLOCK_HELIPAD3, BLOCK_PRE_HELIPAD, BLOCK_TAXIWAY_BUSY,
    FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;

/// Número de waypoints Helistation.
pub const HELISTATION_NOF_ELEMENTS: usize = 33;

/// Entradas de holding (`_airport_entries_helistation`).
pub const HELISTATION_ENTRIES: [u8; 4] = [25, 25, 25, 25];

/// `_airport_moving_data_helistation[33]`.
pub static HELISTATION_MOVING_DATA: [AirportMovingData; HELISTATION_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 8,
        y: 3,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0
    AirportMovingData {
        x: 8,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 1
    AirportMovingData {
        x: 116,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 2
    AirportMovingData {
        x: 14,
        y: 22,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 3
    AirportMovingData {
        x: 24,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 4
    AirportMovingData {
        x: 40,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 5
    AirportMovingData {
        x: 40,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 6
    AirportMovingData {
        x: 56,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 7
    AirportMovingData {
        x: 56,
        y: 24,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 8
    AirportMovingData {
        x: 40,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_N,
    }, // 9
    AirportMovingData {
        x: 56,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_N,
    }, // 10
    AirportMovingData {
        x: 56,
        y: 24,
        flags: FLAG_EXACT,
        direction: DIR_N,
    }, // 11
    AirportMovingData {
        x: 32,
        y: 8,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 12
    AirportMovingData {
        x: 48,
        y: 8,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 13
    AirportMovingData {
        x: 48,
        y: 24,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 14
    AirportMovingData {
        x: 84,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 15
    AirportMovingData {
        x: 68,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 16
    AirportMovingData {
        x: 32,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 17
    AirportMovingData {
        x: 48,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 18
    AirportMovingData {
        x: 48,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_NE,
    }, // 19
    AirportMovingData {
        x: 40,
        y: 8,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 20
    AirportMovingData {
        x: 48,
        y: 8,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 21
    AirportMovingData {
        x: 48,
        y: 24,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 22
    AirportMovingData {
        x: 0,
        y: 22,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 23
    AirportMovingData {
        x: 0,
        y: 22,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 24
    AirportMovingData {
        x: 148,
        y: -8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 25
    AirportMovingData {
        x: 148,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 26
    AirportMovingData {
        x: 132,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 27
    AirportMovingData {
        x: 100,
        y: 24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 28
    AirportMovingData {
        x: 84,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 29
    AirportMovingData {
        x: 84,
        y: -8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 30
    AirportMovingData {
        x: 100,
        y: -24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 31
    AirportMovingData {
        x: 132,
        y: -24,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 32
];

/// Filas `_airport_fta_helistation` (sin marcador final).
static HELISTATION_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 8),
    (0, 8, 0, 1),
    (0, 9, 0, 1),
    (0, 21, 0, 1),
    (0, 13, 0, 1),
    (0, 0, 0, 0),
    (1, 255, BLOCK_HANGAR2_AREA, 0),
    (1, 1, 0, 0),
    (1, 13, 0, 3),
    (1, 0, 0, 4),
    (2, 14, 0, 28),
    (2, 17, 0, 15),
    (2, 0, 0, 28),
    (3, 13, 0, 0),
    (4, 255, BLOCK_TAXIWAY_BUSY, 0),
    (4, 1, BLOCK_HANGAR2_AREA, 1),
    (4, 13, 0, 1),
    (4, 0, 0, 5),
    (5, 255, BLOCK_TAXIWAY_BUSY, 0),
    (5, 8, BLOCK_HELIPAD1, 6),
    (5, 9, BLOCK_HELIPAD2, 7),
    (5, 21, BLOCK_HELIPAD3, 8),
    (5, 0, 0, 4),
    (6, 8, BLOCK_HELIPAD1, 5),
    (6, 1, BLOCK_HANGAR2_AREA, 5),
    (6, 13, 0, 9),
    (6, 0, 0, 6),
    (7, 9, BLOCK_HELIPAD2, 5),
    (7, 1, BLOCK_HANGAR2_AREA, 5),
    (7, 13, 0, 10),
    (7, 0, 0, 7),
    (8, 21, BLOCK_HELIPAD3, 5),
    (8, 1, BLOCK_HANGAR2_AREA, 5),
    (8, 13, 0, 11),
    (8, 0, 0, 8),
    (9, 0, BLOCK_HELIPAD1, 12),
    (10, 0, BLOCK_HELIPAD2, 13),
    (11, 0, BLOCK_HELIPAD3, 14),
    (12, 13, 0, 0),
    (13, 13, 0, 0),
    (14, 13, 0, 0),
    (15, 17, BLOCK_PRE_HELIPAD, 16),
    (16, 18, BLOCK_PRE_HELIPAD, 16),
    (16, 8, 0, 17),
    (16, 9, 0, 18),
    (16, 21, 0, 19),
    (16, 1, 0, 23),
    (17, 0, 0, 20),
    (18, 0, 0, 21),
    (19, 0, 0, 22),
    (20, 255, 0, 0),
    (20, 8, BLOCK_HELIPAD1, 6),
    (20, 1, 0, 23),
    (20, 0, 0, 6),
    (21, 255, 0, 0),
    (21, 9, BLOCK_HELIPAD2, 7),
    (21, 1, 0, 23),
    (21, 0, 0, 7),
    (22, 255, 0, 0),
    (22, 21, BLOCK_HELIPAD3, 8),
    (22, 1, 0, 23),
    (22, 0, 0, 8),
    (23, 0, BLOCK_HANGAR2_AREA, 24),
    (24, 0, BLOCK_HANGAR2_AREA, 1),
    (25, 0, 0, 26),
    (26, 0, 0, 27),
    (27, 0, 0, 2),
    (28, 0, 0, 29),
    (29, 0, 0, 30),
    (30, 0, 0, 31),
    (31, 0, 0, 32),
    (32, 0, 0, 25),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn helistation_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    HELISTATION_FTA_BUILDUP
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
