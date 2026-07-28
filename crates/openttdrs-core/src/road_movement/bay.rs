//! Bahías de estaciones (bus/truck stops).

use super::curves::sample_curve;
use super::drive_data::{RDE_NEXT_TILE, RoadDriveEntry};
use super::pose::{VehiclePose, movement_target_at};
use super::rvsb::{
    RVSB_ENTERED_STOP, RVSB_TRACKDIR_MASK, RVSB_USING_SECOND_BAY, direction_from_trackdir,
    is_bay_road_state,
};
use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleDirection, direction_from_tile_step, reverse_direction};

type SubTile = (f32, f32);

/// Trayectoria dentro de una bahía (`_rv_station_left_*`, lado izquierdo —
/// el mismo que las tablas rectas/curvas del port): entrada por la boca
/// (frames `0..=stop`), detención en `stop` (`_road_stop_stop_frame`) y lazo
/// de retorno hacia la boca (frames `stop..`).
pub struct BayStationTable {
    /// Puntos sub-tesela por frame (copia de `roadveh_movement.h:458-737`).
    pub points: &'static [SubTile],
    /// Índice del frame de parada (`_road_stop_stop_frame:1087-1093`).
    pub stop: usize,
}

/// `_rv_station_left_sw_far` (stop frame 20).
const BAY_LEFT_SW_FAR: BayStationTable = BayStationTable {
    points: &[
        (15.0, 5.0),
        (14.0, 5.0),
        (13.0, 6.0),
        (13.0, 7.0),
        (13.0, 8.0),
        (13.0, 9.0),
        (13.0, 10.0),
        (13.0, 11.0),
        (12.0, 12.0),
        (11.0, 12.0),
        (10.0, 12.0),
        (9.0, 12.0),
        (8.0, 12.0),
        (7.0, 12.0),
        (6.0, 12.0),
        (5.0, 11.0),
        (5.0, 10.0),
        (5.0, 9.0),
        (5.0, 8.0),
        (5.0, 7.0),
        (5.0, 6.0),
        (5.0, 7.0),
        (5.0, 8.0),
        (5.0, 9.0),
        (5.0, 10.0),
        (5.0, 11.0),
        (6.0, 12.0),
        (7.0, 12.0),
        (8.0, 12.0),
        (9.0, 12.0),
        (10.0, 12.0),
        (11.0, 12.0),
        (12.0, 12.0),
        (13.0, 11.0),
        (13.0, 10.0),
        (14.0, 9.0),
        (15.0, 9.0),
    ],
    stop: 20,
};

/// `_rv_station_left_nw_far` (stop frame 20).
const BAY_LEFT_NW_FAR: BayStationTable = BayStationTable {
    points: &[
        (5.0, 0.0),
        (5.0, 1.0),
        (6.0, 2.0),
        (7.0, 2.0),
        (8.0, 2.0),
        (9.0, 2.0),
        (10.0, 2.0),
        (11.0, 2.0),
        (12.0, 3.0),
        (12.0, 4.0),
        (12.0, 5.0),
        (12.0, 6.0),
        (12.0, 7.0),
        (12.0, 8.0),
        (12.0, 9.0),
        (11.0, 10.0),
        (10.0, 10.0),
        (9.0, 10.0),
        (8.0, 10.0),
        (7.0, 10.0),
        (6.0, 10.0),
        (7.0, 10.0),
        (8.0, 10.0),
        (9.0, 10.0),
        (10.0, 10.0),
        (11.0, 10.0),
        (12.0, 9.0),
        (12.0, 8.0),
        (12.0, 7.0),
        (12.0, 6.0),
        (12.0, 5.0),
        (12.0, 4.0),
        (12.0, 3.0),
        (11.0, 2.0),
        (10.0, 2.0),
        (9.0, 1.0),
        (9.0, 0.0),
    ],
    stop: 20,
};

/// `_rv_station_left_sw_near` (stop frame 16).
const BAY_LEFT_SW_NEAR: BayStationTable = BayStationTable {
    points: &[
        (15.0, 5.0),
        (14.0, 5.0),
        (13.0, 6.0),
        (13.0, 7.0),
        (13.0, 8.0),
        (13.0, 9.0),
        (13.0, 10.0),
        (13.0, 11.0),
        (12.0, 12.0),
        (11.0, 12.0),
        (10.0, 12.0),
        (9.0, 11.0),
        (9.0, 10.0),
        (9.0, 9.0),
        (9.0, 8.0),
        (9.0, 7.0),
        (9.0, 6.0),
        (9.0, 7.0),
        (9.0, 8.0),
        (9.0, 9.0),
        (9.0, 10.0),
        (9.0, 11.0),
        (10.0, 12.0),
        (11.0, 12.0),
        (12.0, 12.0),
        (13.0, 11.0),
        (13.0, 10.0),
        (14.0, 9.0),
        (15.0, 9.0),
    ],
    stop: 16,
};

/// `_rv_station_left_nw_near` (stop frame 16).
const BAY_LEFT_NW_NEAR: BayStationTable = BayStationTable {
    points: &[
        (5.0, 0.0),
        (5.0, 1.0),
        (6.0, 2.0),
        (7.0, 2.0),
        (8.0, 2.0),
        (9.0, 2.0),
        (10.0, 2.0),
        (11.0, 2.0),
        (12.0, 3.0),
        (12.0, 4.0),
        (12.0, 5.0),
        (11.0, 6.0),
        (10.0, 6.0),
        (9.0, 6.0),
        (8.0, 6.0),
        (7.0, 6.0),
        (6.0, 6.0),
        (7.0, 6.0),
        (8.0, 6.0),
        (9.0, 6.0),
        (10.0, 6.0),
        (11.0, 6.0),
        (12.0, 5.0),
        (12.0, 4.0),
        (12.0, 3.0),
        (11.0, 2.0),
        (10.0, 2.0),
        (9.0, 1.0),
        (9.0, 0.0),
    ],
    stop: 16,
};

/// `_rv_station_left_ne_far` (stop frame 19).
const BAY_LEFT_NE_FAR: BayStationTable = BayStationTable {
    points: &[
        (0.0, 9.0),
        (1.0, 9.0),
        (2.0, 8.0),
        (2.0, 7.0),
        (2.0, 6.0),
        (2.0, 5.0),
        (2.0, 4.0),
        (3.0, 3.0),
        (4.0, 3.0),
        (5.0, 3.0),
        (6.0, 3.0),
        (7.0, 3.0),
        (8.0, 3.0),
        (9.0, 3.0),
        (10.0, 4.0),
        (10.0, 5.0),
        (10.0, 6.0),
        (10.0, 7.0),
        (10.0, 8.0),
        (10.0, 9.0),
        (10.0, 8.0),
        (10.0, 7.0),
        (10.0, 6.0),
        (10.0, 5.0),
        (10.0, 4.0),
        (9.0, 3.0),
        (8.0, 3.0),
        (7.0, 3.0),
        (6.0, 3.0),
        (5.0, 3.0),
        (4.0, 3.0),
        (3.0, 3.0),
        (2.0, 4.0),
        (1.0, 5.0),
        (0.0, 5.0),
    ],
    stop: 19,
};

/// `_rv_station_left_se_far` (stop frame 19).
const BAY_LEFT_SE_FAR: BayStationTable = BayStationTable {
    points: &[
        (9.0, 15.0),
        (9.0, 14.0),
        (8.0, 13.0),
        (7.0, 13.0),
        (6.0, 13.0),
        (5.0, 13.0),
        (4.0, 13.0),
        (3.0, 12.0),
        (3.0, 11.0),
        (3.0, 10.0),
        (3.0, 9.0),
        (3.0, 8.0),
        (3.0, 7.0),
        (3.0, 6.0),
        (4.0, 5.0),
        (5.0, 5.0),
        (6.0, 5.0),
        (7.0, 5.0),
        (8.0, 5.0),
        (9.0, 5.0),
        (8.0, 5.0),
        (7.0, 5.0),
        (6.0, 5.0),
        (5.0, 5.0),
        (4.0, 5.0),
        (3.0, 6.0),
        (3.0, 7.0),
        (3.0, 8.0),
        (3.0, 9.0),
        (3.0, 10.0),
        (3.0, 11.0),
        (3.0, 12.0),
        (4.0, 13.0),
        (5.0, 14.0),
        (5.0, 15.0),
    ],
    stop: 19,
};

/// `_rv_station_left_ne_near` (stop frame 15).
const BAY_LEFT_NE_NEAR: BayStationTable = BayStationTable {
    points: &[
        (0.0, 9.0),
        (1.0, 9.0),
        (2.0, 8.0),
        (2.0, 7.0),
        (2.0, 6.0),
        (2.0, 5.0),
        (2.0, 4.0),
        (3.0, 3.0),
        (4.0, 3.0),
        (5.0, 3.0),
        (6.0, 4.0),
        (6.0, 5.0),
        (6.0, 6.0),
        (6.0, 7.0),
        (6.0, 8.0),
        (6.0, 9.0),
        (6.0, 8.0),
        (6.0, 7.0),
        (6.0, 6.0),
        (6.0, 5.0),
        (6.0, 4.0),
        (5.0, 3.0),
        (4.0, 3.0),
        (3.0, 3.0),
        (2.0, 4.0),
        (1.0, 5.0),
        (0.0, 5.0),
    ],
    stop: 15,
};

/// `_rv_station_left_se_near` (stop frame 15).
const BAY_LEFT_SE_NEAR: BayStationTable = BayStationTable {
    points: &[
        (9.0, 15.0),
        (9.0, 14.0),
        (8.0, 13.0),
        (7.0, 13.0),
        (6.0, 13.0),
        (5.0, 13.0),
        (4.0, 13.0),
        (3.0, 12.0),
        (3.0, 11.0),
        (3.0, 10.0),
        (4.0, 9.0),
        (5.0, 9.0),
        (6.0, 9.0),
        (7.0, 9.0),
        (8.0, 9.0),
        (9.0, 9.0),
        (8.0, 9.0),
        (7.0, 9.0),
        (6.0, 9.0),
        (5.0, 9.0),
        (4.0, 9.0),
        (3.0, 10.0),
        (3.0, 11.0),
        (3.0, 12.0),
        (4.0, 13.0),
        (5.0, 14.0),
        (5.0, 15.0),
    ],
    stop: 15,
};

// Tablas `_rv_station_right_*` (OpenTTD 15.3 roadveh_movement.h).

const BAY_RIGHT_SW_FAR: BayStationTable = BayStationTable {
    points: &[
        (15.0, 9.0),
        (14.0, 9.0),
        (13.0, 10.0),
        (13.0, 11.0),
        (12.0, 12.0),
        (11.0, 12.0),
        (10.0, 12.0),
        (9.0, 12.0),
        (8.0, 12.0),
        (7.0, 12.0),
        (6.0, 12.0),
        (5.0, 11.0),
        (5.0, 10.0),
        (5.0, 9.0),
        (5.0, 8.0),
        (5.0, 7.0),
        (5.0, 6.0),
        (5.0, 7.0),
        (5.0, 8.0),
        (5.0, 9.0),
        (5.0, 10.0),
        (5.0, 11.0),
        (6.0, 12.0),
        (7.0, 12.0),
        (8.0, 12.0),
        (9.0, 12.0),
        (10.0, 12.0),
        (11.0, 12.0),
        (12.0, 12.0),
        (13.0, 11.0),
        (13.0, 10.0),
        (13.0, 9.0),
        (13.0, 8.0),
        (13.0, 7.0),
        (13.0, 6.0),
        (14.0, 5.0),
        (15.0, 5.0),
    ],
    stop: 16,
};

const BAY_RIGHT_NW_FAR: BayStationTable = BayStationTable {
    points: &[
        (9.0, 0.0),
        (9.0, 1.0),
        (10.0, 2.0),
        (11.0, 2.0),
        (12.0, 3.0),
        (12.0, 4.0),
        (12.0, 5.0),
        (12.0, 6.0),
        (12.0, 7.0),
        (12.0, 8.0),
        (12.0, 9.0),
        (11.0, 10.0),
        (10.0, 10.0),
        (9.0, 10.0),
        (8.0, 10.0),
        (7.0, 10.0),
        (6.0, 10.0),
        (7.0, 10.0),
        (8.0, 10.0),
        (9.0, 10.0),
        (10.0, 10.0),
        (11.0, 10.0),
        (12.0, 9.0),
        (12.0, 8.0),
        (12.0, 7.0),
        (12.0, 6.0),
        (12.0, 5.0),
        (12.0, 4.0),
        (12.0, 3.0),
        (11.0, 2.0),
        (10.0, 2.0),
        (9.0, 2.0),
        (8.0, 2.0),
        (7.0, 2.0),
        (6.0, 2.0),
        (5.0, 1.0),
        (5.0, 0.0),
    ],
    stop: 16,
};

const BAY_RIGHT_SW_NEAR: BayStationTable = BayStationTable {
    points: &[
        (15.0, 9.0),
        (14.0, 9.0),
        (13.0, 10.0),
        (13.0, 11.0),
        (12.0, 12.0),
        (11.0, 12.0),
        (10.0, 12.0),
        (9.0, 11.0),
        (9.0, 10.0),
        (9.0, 9.0),
        (9.0, 8.0),
        (9.0, 7.0),
        (9.0, 6.0),
        (9.0, 7.0),
        (9.0, 8.0),
        (9.0, 9.0),
        (9.0, 10.0),
        (9.0, 11.0),
        (10.0, 12.0),
        (11.0, 12.0),
        (12.0, 12.0),
        (13.0, 11.0),
        (13.0, 10.0),
        (13.0, 9.0),
        (13.0, 8.0),
        (13.0, 7.0),
        (13.0, 6.0),
        (14.0, 5.0),
        (15.0, 5.0),
    ],
    stop: 12,
};

const BAY_RIGHT_NW_NEAR: BayStationTable = BayStationTable {
    points: &[
        (9.0, 0.0),
        (9.0, 1.0),
        (10.0, 2.0),
        (11.0, 2.0),
        (12.0, 3.0),
        (12.0, 4.0),
        (12.0, 5.0),
        (11.0, 6.0),
        (10.0, 6.0),
        (9.0, 6.0),
        (8.0, 6.0),
        (7.0, 6.0),
        (6.0, 6.0),
        (7.0, 6.0),
        (8.0, 6.0),
        (9.0, 6.0),
        (10.0, 6.0),
        (11.0, 6.0),
        (12.0, 5.0),
        (12.0, 4.0),
        (12.0, 3.0),
        (11.0, 2.0),
        (10.0, 2.0),
        (9.0, 2.0),
        (8.0, 2.0),
        (7.0, 2.0),
        (6.0, 2.0),
        (5.0, 1.0),
        (5.0, 0.0),
    ],
    stop: 12,
};

const BAY_RIGHT_NE_FAR: BayStationTable = BayStationTable {
    points: &[
        (0.0, 5.0),
        (1.0, 5.0),
        (2.0, 4.0),
        (3.0, 3.0),
        (4.0, 3.0),
        (5.0, 3.0),
        (6.0, 3.0),
        (7.0, 3.0),
        (8.0, 3.0),
        (9.0, 3.0),
        (10.0, 4.0),
        (10.0, 5.0),
        (10.0, 6.0),
        (10.0, 7.0),
        (10.0, 8.0),
        (10.0, 9.0),
        (10.0, 8.0),
        (10.0, 7.0),
        (10.0, 6.0),
        (10.0, 5.0),
        (10.0, 4.0),
        (9.0, 3.0),
        (8.0, 3.0),
        (7.0, 3.0),
        (6.0, 3.0),
        (5.0, 3.0),
        (4.0, 3.0),
        (3.0, 3.0),
        (2.0, 4.0),
        (2.0, 5.0),
        (2.0, 6.0),
        (2.0, 7.0),
        (2.0, 8.0),
        (1.0, 9.0),
        (0.0, 9.0),
    ],
    stop: 15,
};

const BAY_RIGHT_SE_FAR: BayStationTable = BayStationTable {
    points: &[
        (5.0, 15.0),
        (5.0, 14.0),
        (4.0, 13.0),
        (3.0, 12.0),
        (3.0, 11.0),
        (3.0, 10.0),
        (3.0, 9.0),
        (3.0, 8.0),
        (3.0, 7.0),
        (3.0, 6.0),
        (4.0, 5.0),
        (5.0, 5.0),
        (6.0, 5.0),
        (7.0, 5.0),
        (8.0, 5.0),
        (9.0, 5.0),
        (8.0, 5.0),
        (7.0, 5.0),
        (6.0, 5.0),
        (5.0, 5.0),
        (4.0, 5.0),
        (3.0, 6.0),
        (3.0, 7.0),
        (3.0, 8.0),
        (3.0, 9.0),
        (3.0, 10.0),
        (3.0, 11.0),
        (3.0, 12.0),
        (4.0, 13.0),
        (5.0, 13.0),
        (6.0, 13.0),
        (7.0, 13.0),
        (8.0, 13.0),
        (9.0, 14.0),
        (9.0, 15.0),
    ],
    stop: 15,
};

const BAY_RIGHT_NE_NEAR: BayStationTable = BayStationTable {
    points: &[
        (0.0, 5.0),
        (1.0, 5.0),
        (2.0, 4.0),
        (3.0, 3.0),
        (4.0, 3.0),
        (5.0, 3.0),
        (6.0, 4.0),
        (6.0, 5.0),
        (6.0, 6.0),
        (6.0, 7.0),
        (6.0, 8.0),
        (6.0, 9.0),
        (6.0, 8.0),
        (6.0, 7.0),
        (6.0, 6.0),
        (6.0, 5.0),
        (6.0, 4.0),
        (5.0, 3.0),
        (4.0, 3.0),
        (3.0, 3.0),
        (2.0, 4.0),
        (2.0, 5.0),
        (2.0, 6.0),
        (2.0, 7.0),
        (2.0, 8.0),
        (1.0, 9.0),
        (0.0, 9.0),
    ],
    stop: 11,
};

const BAY_RIGHT_SE_NEAR: BayStationTable = BayStationTable {
    points: &[
        (5.0, 15.0),
        (5.0, 14.0),
        (4.0, 13.0),
        (3.0, 12.0),
        (3.0, 11.0),
        (3.0, 10.0),
        (4.0, 9.0),
        (5.0, 9.0),
        (6.0, 9.0),
        (7.0, 9.0),
        (8.0, 9.0),
        (9.0, 9.0),
        (8.0, 9.0),
        (7.0, 9.0),
        (6.0, 9.0),
        (5.0, 9.0),
        (4.0, 9.0),
        (3.0, 10.0),
        (3.0, 11.0),
        (3.0, 12.0),
        (4.0, 13.0),
        (5.0, 13.0),
        (6.0, 13.0),
        (7.0, 13.0),
        (8.0, 13.0),
        (9.0, 14.0),
        (9.0, 15.0),
    ],
    stop: 11,
};

use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW};

/// Tabla de bahía según la dirección con la que el vehículo ENTRA por la boca
/// (opuesta a la orientación de la boca: boca SW → se entra rumbo NE) y la
/// dársena (far/near). El nombre C++ es la orientación de la boca. Con una
/// sola dársena por bahía en la sim, el render usa siempre la `far` (la
/// primera que asigna `RoadStop::AllocateBay` en `OpenTTD` con la parada vacía).
#[must_use]
pub const fn bay_station_table(
    inbound: VehicleDirection,
    far: bool,
) -> Option<&'static BayStationTable> {
    bay_station_table_for_side(inbound, far, false)
}

/// Como [`bay_station_table`], eligiendo tablas left (`false`) o right (`true`).
#[must_use]
pub const fn bay_station_table_for_side(
    inbound: VehicleDirection,
    far: bool,
    drive_on_right: bool,
) -> Option<&'static BayStationTable> {
    if drive_on_right {
        return match (inbound, far) {
            (DIR_NE, true) => Some(&BAY_RIGHT_SW_FAR),
            (DIR_NE, false) => Some(&BAY_RIGHT_SW_NEAR),
            (DIR_SE, true) => Some(&BAY_RIGHT_NW_FAR),
            (DIR_SE, false) => Some(&BAY_RIGHT_NW_NEAR),
            (DIR_SW, true) => Some(&BAY_RIGHT_NE_FAR),
            (DIR_SW, false) => Some(&BAY_RIGHT_NE_NEAR),
            (DIR_NW, true) => Some(&BAY_RIGHT_SE_FAR),
            (DIR_NW, false) => Some(&BAY_RIGHT_SE_NEAR),
            _ => None,
        };
    }
    match (inbound, far) {
        (DIR_NE, true) => Some(&BAY_LEFT_SW_FAR),
        (DIR_NE, false) => Some(&BAY_LEFT_SW_NEAR),
        (DIR_SE, true) => Some(&BAY_LEFT_NW_FAR),
        (DIR_SE, false) => Some(&BAY_LEFT_NW_NEAR),
        (DIR_SW, true) => Some(&BAY_LEFT_NE_FAR),
        (DIR_SW, false) => Some(&BAY_LEFT_NE_NEAR),
        (DIR_NW, true) => Some(&BAY_LEFT_SE_FAR),
        (DIR_NW, false) => Some(&BAY_LEFT_SE_NEAR),
        _ => None,
    }
}

/// Tabla elegida por el `state` de `RoadVehicle`, incluidos los bits de
/// dársena cercana y parada alcanzada. Es el índice de `_road_stop_stop_frame`
/// y `_road_drive_data` usado por `OpenTTD`.
#[must_use]
#[allow(dead_code)] // API left-side; el controlador usa `_side`.
pub(crate) fn bay_station_table_for_state(state: u8) -> Option<&'static BayStationTable> {
    bay_station_table_for_state_side(state, false)
}

#[must_use]
pub(crate) fn bay_station_table_for_state_side(
    state: u8,
    drive_on_right: bool,
) -> Option<&'static BayStationTable> {
    if !is_bay_road_state(state) {
        return None;
    }
    let inbound = direction_from_trackdir(state & (RVSB_TRACKDIR_MASK & 0x09));
    let far = state & RVSB_USING_SECOND_BAY == 0;
    bay_station_table_for_side(inbound, far, drive_on_right)
}

/// Frame exacto donde empieza la carga para este estado de bahía.
#[must_use]
#[allow(dead_code)] // API left-side; el controlador usa `_side`.
pub(crate) fn bay_stop_frame(state: u8) -> Option<u8> {
    bay_stop_frame_side(state, false)
}

#[must_use]
pub(crate) fn bay_stop_frame_side(state: u8, drive_on_right: bool) -> Option<u8> {
    u8::try_from(bay_station_table_for_state_side(state, drive_on_right)?.stop).ok()
}

/// Entrada de la tabla `_rv_station_*`, incluido su marcador de salida.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[allow(dead_code)] // API left-side; el controlador usa `_side`.
pub(crate) fn bay_drive_entry(state: u8, frame: u8) -> Option<RoadDriveEntry> {
    bay_drive_entry_side(state, frame, false)
}

#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn bay_drive_entry_side(
    state: u8,
    frame: u8,
    drive_on_right: bool,
) -> Option<RoadDriveEntry> {
    let table = bay_station_table_for_state_side(state, drive_on_right)?;
    let index = usize::from(frame);
    if let Some(&(x, y)) = table.points.get(index) {
        return Some(RoadDriveEntry {
            x: x as u8,
            y: y as u8,
        });
    }
    if index != table.points.len() {
        return None;
    }
    let outbound = reverse_direction(direction_from_trackdir(state & (RVSB_TRACKDIR_MASK & 0x09)));
    let diag = match outbound {
        DIR_NE => 0,
        DIR_SE => 1,
        DIR_SW => 2,
        DIR_NW => 3,
        _ => return None,
    };
    Some(RoadDriveEntry {
        x: RDE_NEXT_TILE | diag,
        y: 0,
    })
}

/// Posición interpolada por frame real del controlador, no por progreso
/// sintético 0..255. Esto mantiene simulación y dibujo sobre la misma tabla.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
#[allow(dead_code)] // API left-side; el render usa `_side`.
pub(crate) fn bay_subtile_at_frame(state: u8, frame_f: f32) -> Option<SubTile> {
    bay_subtile_at_frame_side(state, frame_f, false)
}

#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub(crate) fn bay_subtile_at_frame_side(
    state: u8,
    frame_f: f32,
    drive_on_right: bool,
) -> Option<SubTile> {
    let table = bay_station_table_for_state_side(state, drive_on_right)?;
    let max = (table.points.len().saturating_sub(1)) as f32;
    let frame_f = frame_f.clamp(0.0, max);
    let index = frame_f.floor() as usize;
    let next = (index + 1).min(table.points.len() - 1);
    let t = frame_f - index as f32;
    let a = table.points[index];
    let b = table.points[next];
    Some((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t))
}

/// Orientación del sprite derivada de los puntos contiguos de la tabla.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[allow(dead_code)] // API left-side; el render usa `_side`.
pub(crate) fn bay_direction_at_frame(state: u8, frame_f: f32) -> Option<VehicleDirection> {
    bay_direction_at_frame_side(state, frame_f, false)
}

#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn bay_direction_at_frame_side(
    state: u8,
    frame_f: f32,
    drive_on_right: bool,
) -> Option<VehicleDirection> {
    let table = bay_station_table_for_state_side(state, drive_on_right)?;
    let index = (frame_f.floor() as usize).min(table.points.len() - 1);
    let mut a = index.saturating_sub(1);
    let mut b = (index + 1).min(table.points.len() - 1);
    if table.points[a] == table.points[index] {
        a = index;
    }
    if table.points[b] == table.points[index] && b + 1 < table.points.len() {
        b += 1;
    }
    let p0 = table.points[a];
    let p1 = table.points[b];
    direction_from_subtile_delta(p1.0 - p0.0, p1.1 - p0.1)
}

/// El vehículo alcanzó el frame de servicio y conserva la dársena asignada.
#[must_use]
pub fn road_vehicle_stopped_in_bay(v: &Vehicle) -> bool {
    road_vehicle_stopped_in_bay_side(v, false)
}

/// Como [`road_vehicle_stopped_in_bay`], con lado de circulación.
#[must_use]
pub fn road_vehicle_stopped_in_bay_side(v: &Vehicle, drive_on_right: bool) -> bool {
    is_bay_road_state(v.road_state)
        && v.road_state & RVSB_ENTERED_STOP != 0
        && bay_stop_frame_side(v.road_state, drive_on_right).is_some_and(|stop| v.frame == stop)
}

/// El vehículo está detenido dentro de una bahía de sus órdenes (ancló en la
/// tesela de la estación tras entrar por la boca). Se comprueba contra todas
/// las órdenes porque al terminar de cargar la orden actual ya avanzó a la
/// siguiente parada mientras el vehículo sigue físicamente en la bahía.
#[must_use]
pub fn parked_inside_bay(v: &Vehicle, pos: TileCoord) -> bool {
    v.orders.iter().any(
        |o| matches!(o, crate::vehicle::VehicleOrder::Station { station, .. } if *station == pos),
    )
}

/// Dirección con la que el vehículo entró por la boca de la bahía para una
/// pose dada. Parado o esperando el giro: `v.direction` sigue siendo la de
/// entrada. Pose extrapolada que acaba de cruzar a la bahía: el paso desde la
/// tesela de sim. Saliendo (tras el giro): la inversa del rumbo de salida.
fn bay_inbound_direction(v: &Vehicle, pose: VehiclePose, exiting: bool) -> VehicleDirection {
    if exiting {
        return reverse_direction(movement_direction_at(v, pose.pos, pose.path_index));
    }
    if pose.pos != v.pos {
        return direction_from_tile_step(v.pos, pose.pos);
    }
    v.direction
}

fn movement_direction_at(v: &Vehicle, pos: TileCoord, path_index: usize) -> VehicleDirection {
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return v.direction;
    };
    direction_from_tile_step(pos, next)
}

fn needs_depart_turnaround_at(v: &Vehicle, pos: TileCoord, path_index: usize) -> bool {
    use crate::vehicle::VehicleKind;
    if v.kind == VehicleKind::Train {
        return false;
    }
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return false;
    };
    let outbound = direction_from_tile_step(pos, next);
    outbound == reverse_direction(v.direction)
}

/// Sub-tesela dentro de la bahía siguiendo `_rv_station_{left,right}_*`:
/// entrada = frames `0..=stop`, parada = frame `stop`, salida = `stop..`.
pub(super) fn bay_subtile(v: &Vehicle, pose: VehiclePose) -> Option<SubTile> {
    let has_target = movement_target_at(v, pose.pos, pose.path_index).is_some();
    // Saliendo: hay objetivo y el rumbo ya no exige media vuelta (la dirección
    // se invirtió al completar el giro). Antes/durante el giro sigue parado.
    let exiting = has_target
        && pose.depart_turn_f <= 0.0
        && !needs_depart_turnaround_at(v, pose.pos, pose.path_index);
    let table = bay_station_table_for_side(
        bay_inbound_direction(v, pose, exiting),
        true,
        pose.drive_on_right,
    )?;
    if exiting {
        // Lazo de retorno hacia la boca (retraza el carril con rumbo opuesto).
        return Some(sample_curve(&table.points[table.stop..], pose.progress_f));
    }
    if pose.progress_f < 255.0 && !has_target {
        // Entrando: de la boca al punto de parada.
        return Some(sample_curve(&table.points[..=table.stop], pose.progress_f));
    }
    // Detenido en el stop frame: cargando, esperando o girando en el vértice
    // del lazo (en OpenTTD el cambio de sentido en la dársena es instantáneo).
    Some(table.points[table.stop])
}

/// Dirección del sprite dentro de la bahía: delta entre dos muestras cercanas
/// de la trayectoria `_rv_station_*` (los lazos incluyen tramos en S que el
/// rumbo lógico de entrada no captura).
pub(super) fn bay_render_direction(v: &Vehicle, pose: VehiclePose) -> Option<VehicleDirection> {
    const PROBE: f32 = 16.0;
    let (a, b) = if pose.progress_f >= 255.0 - PROBE {
        let mut before = pose;
        before.progress_f = (pose.progress_f - PROBE).max(0.0);
        before.sync_discrete_fields();
        (bay_subtile(v, before)?, bay_subtile(v, pose)?)
    } else {
        let mut after = pose;
        after.progress_f = (pose.progress_f + PROBE).min(255.0);
        after.sync_discrete_fields();
        (bay_subtile(v, pose)?, bay_subtile(v, after)?)
    };
    direction_from_subtile_delta(b.0 - a.0, b.1 - a.1)
}

/// Dirección 0–7 a partir del delta entre dos puntos sub-tesela (misma regla
/// que `OpenTTD`, que orienta el sprite con `new_pos - old_pos`): en estas
/// tablas el eje x crece hacia SW y el eje y hacia SE.
pub(crate) fn direction_from_subtile_delta(dx: f32, dy: f32) -> Option<VehicleDirection> {
    use crate::vehicle::{DIR_E, DIR_N, DIR_S, DIR_W};
    let sx = if dx.abs() < 0.25 {
        0
    } else if dx > 0.0 {
        1
    } else {
        -1
    };
    let sy = if dy.abs() < 0.25 {
        0
    } else if dy > 0.0 {
        1
    } else {
        -1
    };
    match (sx, sy) {
        (-1, 0) => Some(DIR_NE),
        (1, 0) => Some(DIR_SW),
        (0, 1) => Some(DIR_SE),
        (0, -1) => Some(DIR_NW),
        (-1, -1) => Some(DIR_N),
        (-1, 1) => Some(DIR_E),
        (1, 1) => Some(DIR_S),
        (1, -1) => Some(DIR_W),
        _ => None,
    }
}
