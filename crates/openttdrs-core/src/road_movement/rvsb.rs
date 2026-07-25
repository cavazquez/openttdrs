//! Estados `RVSB_*` de vehículos de carretera (`roadveh.h`).

/// El vehículo está en un depósito.
pub const RVSB_IN_DEPOT: u8 = 0xFE;
/// El vehículo está en un túnel/puente (wormhole).
pub const RVSB_WORMHOLE: u8 = 0xFF;

/// Bit: segundo andén de parada.
pub const RVS_USING_SECOND_BAY: u8 = 1;
/// Bit: ha entrado en la parada.
pub const RVS_ENTERED_STOP: u8 = 2;
/// Bit: lado de conducción opuesto / overtaking.
pub const RVS_DRIVE_SIDE: u8 = 4;
/// Bit: dentro de road stop.
pub const RVS_IN_ROAD_STOP: u8 = 5;
/// Bit: dentro de drive-through stop.
pub const RVS_IN_DT_ROAD_STOP: u8 = 6;

pub const RVSB_IN_ROAD_STOP: u8 = 1 << RVS_IN_ROAD_STOP;
pub const RVSB_IN_DT_ROAD_STOP: u8 = 1 << RVS_IN_DT_ROAD_STOP;
pub const RVSB_DRIVE_SIDE: u8 = 1 << RVS_DRIVE_SIDE;
pub const RVSB_TRACKDIR_MASK: u8 = 0x0F;

/// Máscaras de los flags internos de una parada en bahía.
pub const RVSB_USING_SECOND_BAY: u8 = 1 << RVS_USING_SECOND_BAY;
pub const RVSB_ENTERED_STOP: u8 = 1 << RVS_ENTERED_STOP;

/// `state` pertenece a las tablas `_rv_station_*` de una parada en bahía.
#[must_use]
pub const fn is_bay_road_state(state: u8) -> bool {
    state & 0xE0 == RVSB_IN_ROAD_STOP
}

/// Frame inicial al entrar en tesela.
pub const RVC_DEFAULT_START_FRAME: u8 = 0;
/// Frame al girar en U.
pub const RVC_TURN_AROUND_START_FRAME: u8 = 1;
/// Frame al salir de depósito.
pub const RVC_DEPOT_START_FRAME: u8 = 6;

/// Trackdir recto según dirección de sprite del port (`DIR_NE`…).
#[must_use]
pub const fn trackdir_from_direction(direction: u8) -> u8 {
    use crate::vehicle::{DIR_NW, DIR_SE, DIR_SW};
    match direction {
        DIR_SE => 1,
        DIR_SW => 8,
        DIR_NW => 9,
        _ => 0, // DIR_NE y cardinales
    }
}

/// Dirección de sprite desde trackdir recto / curva aproximada.
#[must_use]
pub const fn direction_from_trackdir(trackdir: u8) -> u8 {
    use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW};
    match trackdir & RVSB_TRACKDIR_MASK {
        1 | 5 => DIR_SE,
        8 | 4 | 10 | 11 => DIR_SW,
        9 | 13 => DIR_NW,
        _ => DIR_NE,
    }
}

/// Trackdir de la tabla de carretera para el rumbo de entrada y de salida de
/// una tesela. Los ocho casos curvos son `TRACKDIR_UPPER/LOWER/LEFT/RIGHT` del
/// original; los demás conservan la recta de entrada.
#[must_use]
pub const fn trackdir_for_entry_exit(entry: u8, exit: u8) -> u8 {
    use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW};
    match (entry, exit) {
        (DIR_SE, DIR_NE) => 2,
        (DIR_NE, DIR_SE) => 3,
        (DIR_SE, DIR_SW) => 4,
        (DIR_SW, DIR_SE) => 5,
        (DIR_SW, DIR_NW) => 10,
        (DIR_NW, DIR_SW) => 11,
        (DIR_NE, DIR_NW) => 12,
        (DIR_NW, DIR_NE) => 13,
        _ => trackdir_from_direction(entry),
    }
}
