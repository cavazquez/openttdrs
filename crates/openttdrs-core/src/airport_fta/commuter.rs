//! Tablas Commuter (`_airport_moving_data_commuter`, `_airport_fta_commuter`).

use super::types::{
    AirportBlockBits, AirportFtaEdge, AirportHeading, AirportMovingData, BLOCK_AIRPORT_BUSY,
    BLOCK_AIRPORT_ENTRANCE, BLOCK_HELIPAD1, BLOCK_HELIPAD2, BLOCK_IN_WAY, BLOCK_OUT_WAY,
    BLOCK_PRE_HELIPAD, BLOCK_TAXIWAY_BUSY, BLOCK_TERM1, BLOCK_TERM2, BLOCK_TERM3, FLAG_BRAKE,
    FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_LAND, FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN,
    FLAG_TAKEOFF,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;
const DIR_SE: u8 = 3;
const DIR_SW: u8 = 5;
const DIR_NW: u8 = 7;

/// Número de waypoints Commuter.
pub const COMMUTER_NOF_ELEMENTS: usize = 38;

/// Entradas de holding (`_airport_entries_commuter`).
pub const COMMUTER_ENTRIES: [u8; 4] = [22, 21, 24, 23];

/// `_airport_moving_data_commuter[38]`.
pub static COMMUTER_MOVING_DATA: [AirportMovingData; COMMUTER_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 69,
        y: 3,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 0 hangar
    AirportMovingData {
        x: 72,
        y: 22,
        flags: 0,
        direction: DIR_N,
    }, // 1 outside
    AirportMovingData {
        x: 8,
        y: 22,
        flags: FLAG_EXACT,
        direction: DIR_SW,
    }, // 2 entrance
    AirportMovingData {
        x: 24,
        y: 36,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 3 term1
    AirportMovingData {
        x: 40,
        y: 36,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 4 term2
    AirportMovingData {
        x: 56,
        y: 36,
        flags: FLAG_EXACT,
        direction: DIR_SE,
    }, // 5 term3
    AirportMovingData {
        x: 40,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 6 helipad1
    AirportMovingData {
        x: 56,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 7 helipad2
    AirportMovingData {
        x: 24,
        y: 22,
        flags: 0,
        direction: DIR_SW,
    }, // 8 taxi
    AirportMovingData {
        x: 40,
        y: 22,
        flags: 0,
        direction: DIR_SW,
    }, // 9 taxi
    AirportMovingData {
        x: 56,
        y: 22,
        flags: 0,
        direction: DIR_SW,
    }, // 10 taxi
    AirportMovingData {
        x: 72,
        y: 40,
        flags: 0,
        direction: DIR_SE,
    }, // 11 outway
    AirportMovingData {
        x: 72,
        y: 54,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 12 runway start
    AirportMovingData {
        x: 7,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 13 accelerate
    AirportMovingData {
        x: 5,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP,
        direction: DIR_N,
    }, // 14 end runway
    AirportMovingData {
        x: -79,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_TAKEOFF,
        direction: DIR_N,
    }, // 15 takeoff
    AirportMovingData {
        x: 145,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 16 approach
    AirportMovingData {
        x: 73,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_LAND,
        direction: DIR_N,
    }, // 17 land
    AirportMovingData {
        x: 3,
        y: 54,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_BRAKE,
        direction: DIR_N,
    }, // 18 brake
    AirportMovingData {
        x: 12,
        y: 54,
        flags: FLAG_SLOW_TURN,
        direction: DIR_NW,
    }, // 19 turn
    AirportMovingData {
        x: 8,
        y: 32,
        flags: 0,
        direction: DIR_NW,
    }, // 20 taxi from runway
    AirportMovingData {
        x: 1,
        y: 149,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 21 hold NE
    AirportMovingData {
        x: 1,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 22 hold NW
    AirportMovingData {
        x: 193,
        y: 6,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 23 hold SW
    AirportMovingData {
        x: 225,
        y: 62,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 24 hold S
    AirportMovingData {
        x: 80,
        y: 0,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 25 heli buffer
    AirportMovingData {
        x: 80,
        y: 0,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 26 heli buffer
    AirportMovingData {
        x: 32,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 27 pad1 air
    AirportMovingData {
        x: 48,
        y: 8,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 28 pad2 air
    AirportMovingData {
        x: 32,
        y: 8,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 29 land pad1
    AirportMovingData {
        x: 48,
        y: 8,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 30 land pad2
    AirportMovingData {
        x: 32,
        y: 8,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 31 takeoff pad1
    AirportMovingData {
        x: 48,
        y: 8,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 32 takeoff pad2
    AirportMovingData {
        x: 64,
        y: 22,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 33 hangar air
    AirportMovingData {
        x: 64,
        y: 22,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 34 hangar lower
    AirportMovingData {
        x: 40,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_N,
    }, // 35 pre-takeoff pad1
    AirportMovingData {
        x: 56,
        y: 8,
        flags: FLAG_EXACT,
        direction: DIR_N,
    }, // 36 pre-takeoff pad2
    AirportMovingData {
        x: 64,
        y: 25,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 37 takeoff hangar
];

/// Filas `_airport_fta_commuter` (sin marcador final).
#[allow(clippy::unreadable_literal)]
static COMMUTER_FTA_BUILDUP: &[(u8, u8, AirportBlockBits, u8)] = &[
    (0, 1, 0, 1),
    (0, 13, BLOCK_TAXIWAY_BUSY, 1),
    (0, 0, 0, 1),
    (1, 255, BLOCK_TAXIWAY_BUSY, 0),
    (1, 1, 0, 0),
    (1, 10, 0, 11),
    (1, 2, BLOCK_TAXIWAY_BUSY, 10),
    (1, 3, BLOCK_TAXIWAY_BUSY, 10),
    (1, 4, BLOCK_TAXIWAY_BUSY, 10),
    (1, 8, BLOCK_TAXIWAY_BUSY, 10),
    (1, 9, BLOCK_TAXIWAY_BUSY, 10),
    (1, 13, BLOCK_TAXIWAY_BUSY, 37),
    (1, 0, 0, 0),
    (2, 255, BLOCK_AIRPORT_ENTRANCE, 2),
    (2, 1, 0, 8),
    (2, 2, 0, 8),
    (2, 3, 0, 8),
    (2, 4, 0, 8),
    (2, 8, 0, 8),
    (2, 9, 0, 8),
    (2, 13, 0, 8),
    (2, 0, 0, 2),
    (3, 2, BLOCK_TERM1, 8),
    (3, 1, 0, 8),
    (3, 10, 0, 8),
    (3, 0, 0, 3),
    (4, 3, BLOCK_TERM2, 9),
    (4, 1, 0, 9),
    (4, 10, 0, 9),
    (4, 0, 0, 4),
    (5, 4, BLOCK_TERM3, 10),
    (5, 1, 0, 10),
    (5, 10, 0, 10),
    (5, 0, 0, 5),
    (6, 8, BLOCK_HELIPAD1, 6),
    (6, 1, BLOCK_TAXIWAY_BUSY, 9),
    (6, 13, 0, 35),
    (7, 9, BLOCK_HELIPAD2, 7),
    (7, 1, BLOCK_TAXIWAY_BUSY, 10),
    (7, 13, 0, 36),
    (8, 255, BLOCK_TAXIWAY_BUSY, 8),
    (8, 10, BLOCK_TAXIWAY_BUSY, 9),
    (8, 1, BLOCK_TAXIWAY_BUSY, 9),
    (8, 2, BLOCK_TERM1, 3),
    (8, 0, BLOCK_TAXIWAY_BUSY, 9),
    (9, 255, BLOCK_TAXIWAY_BUSY, 9),
    (9, 10, BLOCK_TAXIWAY_BUSY, 10),
    (9, 1, BLOCK_TAXIWAY_BUSY, 10),
    (9, 3, BLOCK_TERM2, 4),
    (9, 8, BLOCK_HELIPAD1, 6),
    (9, 13, BLOCK_HELIPAD1, 6),
    (9, 2, BLOCK_TAXIWAY_BUSY, 8),
    (9, 0, BLOCK_TAXIWAY_BUSY, 10),
    (10, 255, BLOCK_TAXIWAY_BUSY, 10),
    (10, 4, BLOCK_TERM3, 5),
    (10, 8, 0, 9),
    (10, 9, BLOCK_HELIPAD2, 7),
    (10, 13, 0, 1),
    (10, 10, BLOCK_TAXIWAY_BUSY, 1),
    (10, 1, BLOCK_TAXIWAY_BUSY, 1),
    (10, 0, BLOCK_TAXIWAY_BUSY, 9),
    (11, 0, BLOCK_OUT_WAY, 12),
    (12, 10, BLOCK_AIRPORT_BUSY, 13),
    (13, 0, BLOCK_AIRPORT_BUSY, 14),
    (14, 11, BLOCK_AIRPORT_BUSY, 15),
    (15, 12, 0, 0),
    (16, 14, 0, 21),
    (16, 15, BLOCK_IN_WAY, 17),
    (16, 17, 0, 25),
    (17, 15, BLOCK_AIRPORT_BUSY, 18),
    (18, 0, BLOCK_AIRPORT_BUSY, 19),
    (19, 0, BLOCK_AIRPORT_BUSY, 20),
    (20, 16, BLOCK_IN_WAY, 2),
    (21, 0, 0, 22),
    (22, 0, 0, 23),
    (23, 0, 0, 24),
    (24, 0, 0, 16),
    (25, 17, BLOCK_PRE_HELIPAD, 26),
    (26, 18, BLOCK_PRE_HELIPAD, 26),
    (26, 8, 0, 27),
    (26, 9, 0, 28),
    (26, 1, 0, 33),
    (27, 0, 0, 29),
    (28, 0, 0, 30),
    (29, 255, 0, 0),
    (29, 8, BLOCK_HELIPAD1, 6),
    (30, 255, 0, 0),
    (30, 9, BLOCK_HELIPAD2, 7),
    (31, 13, 0, 0),
    (32, 13, 0, 0),
    (33, 0, BLOCK_TAXIWAY_BUSY, 34),
    (34, 0, BLOCK_TAXIWAY_BUSY, 1),
    (35, 0, BLOCK_HELIPAD1, 31),
    (36, 0, BLOCK_HELIPAD2, 32),
    (37, 13, 0, 0),
];

/// Aristas FTA con `position == pos`.
#[must_use]
pub fn commuter_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    COMMUTER_FTA_BUILDUP
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
