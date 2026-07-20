//! Tablas Oilrig (`_airport_moving_data_oilrig`; FTA = heliport).

use super::heliport::{HELIPORT_ENTRIES, HELIPORT_NOF_ELEMENTS, heliport_fta_edges};
use super::types::{
    AirportFtaEdge, AirportMovingData, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE,
    FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN,
};

const DIR_N: u8 = 0;
const DIR_NE: u8 = 1;

/// Número de waypoints Oilrig (igual que Heliport).
pub const OILRIG_NOF_ELEMENTS: usize = HELIPORT_NOF_ELEMENTS;

/// Entradas de holding (`_airport_entries_oilrig` = heliport).
pub const OILRIG_ENTRIES: [u8; 4] = HELIPORT_ENTRIES;

/// `_airport_moving_data_oilrig[9]`.
pub static OILRIG_MOVING_DATA: [AirportMovingData; OILRIG_NOF_ELEMENTS] = [
    AirportMovingData {
        x: 31,
        y: 9,
        flags: FLAG_EXACT,
        direction: DIR_NE,
    }, // 0 pad
    AirportMovingData {
        x: 28,
        y: 9,
        flags: FLAG_HELI_RAISE,
        direction: DIR_N,
    }, // 1 takeoff
    AirportMovingData {
        x: 23,
        y: 9,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 2 above
    AirportMovingData {
        x: 23,
        y: 9,
        flags: FLAG_HELI_LOWER,
        direction: DIR_N,
    }, // 3 land
    AirportMovingData {
        x: 28,
        y: 9,
        flags: 0,
        direction: DIR_N,
    }, // 4 to terminal
    AirportMovingData {
        x: -31,
        y: 69,
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
        x: 69,
        y: -49,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 7 hold SW
    AirportMovingData {
        x: 69,
        y: 9,
        flags: FLAG_NO_SPEED_CLAMP | FLAG_SLOW_TURN,
        direction: DIR_N,
    }, // 8 hold S
];

/// Aristas FTA Oilrig (= heliport).
#[must_use]
pub fn oilrig_fta_edges(pos: u8) -> Vec<AirportFtaEdge> {
    heliport_fta_edges(pos)
}
