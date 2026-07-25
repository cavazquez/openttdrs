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

/// Frame inicial al entrar en tesela.
pub const RVC_DEFAULT_START_FRAME: u8 = 0;
/// Frame al girar en U.
pub const RVC_TURN_AROUND_START_FRAME: u8 = 1;
/// Frame al salir de depósito.
pub const RVC_DEPOT_START_FRAME: u8 = 6;

/// Trackdir recto según dirección de sprite del port (`DIR_NE`…).
#[must_use]
pub fn trackdir_from_direction(direction: u8) -> u8 {
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
pub fn direction_from_trackdir(trackdir: u8) -> u8 {
    use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW};
    match trackdir & RVSB_TRACKDIR_MASK {
        1 | 5 => DIR_SE,
        8 | 4 | 10 | 11 => DIR_SW,
        9 | 13 => DIR_NW,
        _ => DIR_NE,
    }
}
