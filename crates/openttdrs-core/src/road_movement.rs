//! Sub-tesela de vehículos en carretera/vía (`table/roadveh_movement.h`).

use crate::map::TileCoord;
use crate::vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, Vehicle, VehicleDirection,
    VehicleKind, direction_from_tile_step, reverse_direction,
};

type SubTile = (f32, f32);

/// Carriles rectos (índices 0/1/8/9 de `_road_road_drive_data`, carril izquierdo).
const STRAIGHT: [(f32, f32, f32, f32); 8] = [
    (8.0, 8.0, 8.0, 8.0),
    (15.0, 5.0, 0.0, 5.0),
    (8.0, 8.0, 8.0, 8.0),
    (5.0, 0.0, 5.0, 15.0),
    (8.0, 8.0, 8.0, 8.0),
    (0.0, 9.0, 15.0, 9.0),
    (8.0, 8.0, 8.0, 8.0),
    (9.0, 15.0, 9.0, 0.0),
];

// Giros 90° — `_roadveh_drive_data_{2,3,4,5,10,11,12,13}` (sin flags NEXT/TURNED).
const CURVE_NE_SE: &[SubTile] = &[
    (15.0, 5.0),
    (14.0, 5.0),
    (13.0, 5.0),
    (12.0, 5.0),
    (11.0, 5.0),
    (10.0, 5.0),
    (9.0, 6.0),
    (8.0, 7.0),
    (7.0, 8.0),
    (6.0, 9.0),
    (5.0, 10.0),
    (5.0, 11.0),
    (5.0, 12.0),
    (5.0, 13.0),
    (5.0, 14.0),
    (5.0, 15.0),
];
const CURVE_SE_SW: &[SubTile] = &[
    (5.0, 0.0),
    (5.0, 1.0),
    (5.0, 2.0),
    (5.0, 3.0),
    (5.0, 4.0),
    (5.0, 5.0),
    (6.0, 6.0),
    (7.0, 7.0),
    (8.0, 8.0),
    (9.0, 9.0),
    (10.0, 9.0),
    (11.0, 9.0),
    (12.0, 9.0),
    (13.0, 9.0),
    (14.0, 9.0),
    (15.0, 9.0),
];
const CURVE_SW_NW: &[SubTile] = &[
    (0.0, 9.0),
    (1.0, 9.0),
    (2.0, 9.0),
    (3.0, 9.0),
    (4.0, 9.0),
    (5.0, 9.0),
    (6.0, 8.0),
    (7.0, 7.0),
    (8.0, 6.0),
    (9.0, 5.0),
    (9.0, 4.0),
    (9.0, 3.0),
    (9.0, 2.0),
    (9.0, 1.0),
    (9.0, 0.0),
];
const CURVE_NW_NE: &[SubTile] = &[
    (5.0, 0.0),
    (5.0, 1.0),
    (5.0, 2.0),
    (4.0, 3.0),
    (3.0, 4.0),
    (2.0, 5.0),
    (1.0, 5.0),
    (0.0, 5.0),
];
const CURVE_NE_NW: &[SubTile] = &[
    (9.0, 15.0),
    (9.0, 14.0),
    (9.0, 13.0),
    (9.0, 12.0),
    (9.0, 11.0),
    (9.0, 10.0),
    (8.0, 9.0),
    (7.0, 8.0),
    (6.0, 7.0),
    (5.0, 6.0),
    (4.0, 5.0),
    (3.0, 5.0),
    (2.0, 5.0),
    (1.0, 5.0),
    (0.0, 5.0),
];
const CURVE_NW_SW: &[SubTile] = &[
    (9.0, 15.0),
    (9.0, 14.0),
    (9.0, 13.0),
    (10.0, 12.0),
    (11.0, 11.0),
    (12.0, 10.0),
    (13.0, 9.0),
    (14.0, 9.0),
    (15.0, 9.0),
];
const CURVE_SW_SE: &[SubTile] = &[
    (0.0, 9.0),
    (1.0, 9.0),
    (2.0, 9.0),
    (3.0, 10.0),
    (4.0, 11.0),
    (5.0, 12.0),
    (5.0, 13.0),
    (5.0, 14.0),
    (5.0, 15.0),
];
const CURVE_SE_NE: &[SubTile] = &[
    (15.0, 5.0),
    (14.0, 5.0),
    (13.0, 5.0),
    (12.0, 4.0),
    (11.0, 3.0),
    (10.0, 2.0),
    (9.0, 1.0),
    (9.0, 0.0),
];

// Media vuelta en parada/estación (fin de carril de entrada → inicio del opuesto).
const U_TURN_NW_SE: &[SubTile] = &[
    (9.0, 0.0),
    (8.0, 1.0),
    (7.0, 2.0),
    (6.0, 3.0),
    (5.0, 4.0),
    (5.0, 5.0),
    (5.0, 6.0),
    (5.0, 7.0),
    (5.0, 8.0),
    (5.0, 9.0),
    (5.0, 10.0),
    (5.0, 11.0),
    (5.0, 12.0),
    (5.0, 13.0),
    (5.0, 14.0),
    (5.0, 15.0),
];
const U_TURN_NE_SW: &[SubTile] = &[
    (0.0, 5.0),
    (1.0, 5.0),
    (2.0, 6.0),
    (3.0, 7.0),
    (4.0, 8.0),
    (5.0, 9.0),
    (6.0, 9.0),
    (7.0, 9.0),
    (8.0, 9.0),
    (9.0, 9.0),
    (10.0, 9.0),
    (11.0, 9.0),
    (12.0, 9.0),
    (13.0, 9.0),
    (14.0, 9.0),
    (15.0, 9.0),
];

const fn turn_curve(entry: VehicleDirection, exit: VehicleDirection) -> Option<&'static [SubTile]> {
    match (entry, exit) {
        (DIR_NE, DIR_SE) => Some(CURVE_NE_SE),
        (DIR_SE, DIR_SW) => Some(CURVE_SE_SW),
        (DIR_SW, DIR_NW) => Some(CURVE_SW_NW),
        (DIR_NW, DIR_NE) => Some(CURVE_NW_NE),
        (DIR_NE, DIR_NW) => Some(CURVE_NE_NW),
        (DIR_NW, DIR_SW) => Some(CURVE_NW_SW),
        (DIR_SW, DIR_SE) => Some(CURVE_SW_SE),
        (DIR_SE, DIR_NE) => Some(CURVE_SE_NE),
        _ => None,
    }
}

/// Puntos sub-tesela de la curva de giro (copias de `_roadveh_drive_data_*`;
/// expuesto para los tests golden contra las tablas C++ upstream).
#[must_use]
pub const fn turn_curve_points(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> Option<&'static [(f32, f32)]> {
    turn_curve(entry, exit)
}

/// Giro de 90° en la tesela actual (`entry` → `exit` en el camino).
#[must_use]
pub fn road_turn_entry_exit(v: &Vehicle) -> Option<(VehicleDirection, VehicleDirection)> {
    if !v.running {
        return None;
    }
    let next = v.movement_target()?;
    let after = v.path.get(1).copied()?;
    let entry = direction_from_tile_step(v.pos, next);
    let exit = direction_from_tile_step(next, after);
    if entry == exit || entry & 1 == 0 || exit & 1 == 0 {
        return None;
    }
    turn_curve(entry, exit).map(|_| (entry, exit))
}

/// Centro de vía (`OpenTTD` ~8; eje horizontal alinea con el centro visual de `TRACK_X`).
const TRAIN_TRACK_CENTER: f32 = 8.0;

#[must_use]
pub fn train_subtile_direction(v: &Vehicle) -> VehicleDirection {
    if v.movement_target().is_some() && (v.progress < 255 || v.cur_speed > 0) {
        return v.movement_direction();
    }
    v.direction
}

#[must_use]
pub fn train_straight_subtile(dir: VehicleDirection, progress: u8) -> (f32, f32) {
    let t = f32::from(progress) / 255.0;
    match dir {
        DIR_SW | DIR_E => (15.0 * t, TRAIN_TRACK_CENTER),
        DIR_NE | DIR_W => (15.0 * (1.0 - t), TRAIN_TRACK_CENTER),
        DIR_SE | DIR_S => (TRAIN_TRACK_CENTER, 15.0 * t),
        DIR_NW | DIR_N => (TRAIN_TRACK_CENTER, 15.0 * (1.0 - t)),
        _ => (TRAIN_TRACK_CENTER, TRAIN_TRACK_CENTER),
    }
}

#[must_use]
pub fn straight_subtile(dir: VehicleDirection, progress: u8) -> (f32, f32) {
    let i = dir.min(7) as usize;
    let (x0, y0, x1, y1) = STRAIGHT[i];
    let t = f32::from(progress) / 255.0;
    (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
}

#[must_use]
fn sample_curve(points: &[SubTile], progress: u8) -> (f32, f32) {
    let n = points.len();
    if n == 0 {
        return (8.0, 8.0);
    }
    if n == 1 {
        return points[0];
    }
    // Curvas OpenTTD: ≤16 puntos; `progress` 0..=255 recorre índice 0..=n-1.
    let last = u8::try_from(n - 1).unwrap_or(u8::MAX);
    let scaled = u16::from(progress) * u16::from(last);
    let i = (scaled / 255).min(u16::from(last));
    let j = i.saturating_add(1).min(u16::from(last));
    let frac = f32::from(scaled % 255) / 255.0;
    let (x0, y0) = points[usize::from(i)];
    let (x1, y1) = points[usize::from(j)];
    (x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
}

fn depart_u_turn_curve(inbound: VehicleDirection) -> Option<&'static [SubTile]> {
    match inbound {
        DIR_NW | DIR_SE => Some(U_TURN_NW_SE),
        DIR_NE | DIR_SW => Some(U_TURN_NE_SW),
        _ => None,
    }
}

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

/// Posición sub-tesela usada para dibujo (puede diferir del estado de sim tras extrapolar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehiclePose {
    pub pos: TileCoord,
    pub progress: u8,
    pub depart_turn: u8,
    /// Índice en `Vehicle::path` del siguiente paso desde `pos`.
    pub path_index: usize,
}

impl VehiclePose {
    #[must_use]
    pub fn from_vehicle(v: &Vehicle) -> Self {
        Self {
            pos: v.pos,
            progress: v.progress,
            depart_turn: v.depart_turn,
            path_index: 0,
        }
    }
}

fn movement_target_at(v: &Vehicle, pos: TileCoord, path_index: usize) -> Option<TileCoord> {
    if !v.running {
        return None;
    }
    if let Some(&next) = v.path.get(path_index) {
        return Some(next);
    }
    if pos == v.dest {
        return None;
    }
    if v.kind == VehicleKind::Train {
        return None;
    }
    if !v.orders.is_empty() && v.no_network_route_to_order {
        return None;
    }
    let dx = v.dest.x - pos.x;
    let dy = v.dest.y - pos.y;
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(if dx != 0 {
        TileCoord::new(pos.x + dx.signum(), pos.y)
    } else {
        TileCoord::new(pos.x, pos.y + dy.signum())
    })
}

fn needs_depart_turnaround_at(v: &Vehicle, pos: TileCoord, path_index: usize) -> bool {
    if v.kind == VehicleKind::Train {
        return false;
    }
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return false;
    };
    let outbound = direction_from_tile_step(pos, next);
    outbound == reverse_direction(v.direction)
}

fn virtual_advance_tile(
    v: &Vehicle,
    pos: TileCoord,
    path_index: usize,
) -> Option<(TileCoord, usize)> {
    if let Some(&next) = v.path.get(path_index) {
        return Some((next, path_index + 1));
    }
    if pos == v.dest {
        return None;
    }
    let dx = v.dest.x - pos.x;
    let dy = v.dest.y - pos.y;
    if dx == 0 && dy == 0 {
        return None;
    }
    Some((
        if dx != 0 {
            TileCoord::new(pos.x + dx.signum(), pos.y)
        } else {
            TileCoord::new(pos.x, pos.y + dy.signum())
        },
        path_index,
    ))
}

/// Extrapola posición sub-tesela entre ticks de sim (atraviesa límites de tesela sin saltos).
#[must_use]
pub fn extrapolate_vehicle_pose(v: &Vehicle, alpha: f32) -> VehiclePose {
    let mut pose = VehiclePose::from_vehicle(v);
    if alpha <= 0.0 || !v.running || v.cur_speed == 0 {
        return pose;
    }
    let step = v.progress_step();
    if step == 0 {
        return pose;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut budget = (f32::from(step) * alpha.clamp(0.0, 1.0)) as u16;
    if budget == 0 {
        return pose;
    }

    let mut path_index = 0_usize;

    if pose.depart_turn > 0 {
        let next = u16::from(pose.depart_turn).saturating_add(budget);
        if next < 255 {
            pose.depart_turn = u8::try_from(next).unwrap_or(255);
            pose.path_index = path_index;
            return pose;
        }
        budget = next - 255;
        pose.depart_turn = 0;
        pose.progress = 0;
    }

    if pose.progress == 255 && needs_depart_turnaround_at(v, pose.pos, path_index) && budget > 0 {
        pose.depart_turn = u8::try_from(budget.min(u16::from(u8::MAX))).unwrap_or(u8::MAX);
        pose.path_index = path_index;
        return pose;
    }

    let mut progress = u16::from(pose.progress);
    loop {
        progress = progress.saturating_add(budget);
        if progress < 255 {
            pose.progress = u8::try_from(progress).unwrap_or(254);
            pose.path_index = path_index;
            return pose;
        }
        progress -= 255;
        let Some((next, next_index)) = virtual_advance_tile(v, pose.pos, path_index) else {
            pose.progress = 255;
            pose.path_index = path_index;
            return pose;
        };
        pose.pos = next;
        path_index = next_index;
        pose.progress = 0;
        if progress > 0 {
            budget = progress;
            progress = 0;
            continue;
        }
        pose.path_index = path_index;
        return pose;
    }
}

/// Progreso sub-tesela para dibujo (permite extrapolación visual entre ticks de sim).
#[must_use]
pub fn vehicle_render_progress(v: &Vehicle, tick_alpha: f32) -> u8 {
    extrapolate_vehicle_pose(v, tick_alpha).progress
}

/// Sub-tesela `OpenTTD` para dibujo (recto, curva de giro o media vuelta en parada).
#[must_use]
pub fn vehicle_subtile(v: &Vehicle) -> (f32, f32) {
    vehicle_subtile_with_progress(v, v.progress)
}

/// Como [`vehicle_subtile`] con progreso explícito (p. ej. interpolación de render).
#[must_use]
pub fn vehicle_subtile_with_progress(v: &Vehicle, progress: u8) -> (f32, f32) {
    vehicle_subtile_at(
        v,
        VehiclePose {
            pos: v.pos,
            progress,
            depart_turn: v.depart_turn,
            path_index: 0,
        },
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

/// Sub-tesela dentro de la bahía siguiendo `_rv_station_left_*`:
/// entrada = frames `0..=stop`, parada = frame `stop`, salida = `stop..`.
fn bay_subtile(v: &Vehicle, pose: VehiclePose) -> Option<SubTile> {
    let has_target = movement_target_at(v, pose.pos, pose.path_index).is_some();
    // Saliendo: hay objetivo y el rumbo ya no exige media vuelta (la dirección
    // se invirtió al completar el giro). Antes/durante el giro sigue parado.
    let exiting = has_target
        && pose.depart_turn == 0
        && !needs_depart_turnaround_at(v, pose.pos, pose.path_index);
    let table = bay_station_table(bay_inbound_direction(v, pose, exiting), true)?;
    if exiting {
        // Lazo de retorno hacia la boca (retraza el carril con rumbo opuesto).
        return Some(sample_curve(&table.points[table.stop..], pose.progress));
    }
    if pose.progress < 255 && !has_target {
        // Entrando: de la boca al punto de parada.
        return Some(sample_curve(&table.points[..=table.stop], pose.progress));
    }
    // Detenido en el stop frame: cargando, esperando o girando en el vértice
    // del lazo (en OpenTTD el cambio de sentido en la dársena es instantáneo).
    Some(table.points[table.stop])
}

/// Sub-tesela para una pose concreta (sim actual o extrapolada).
#[must_use]
pub fn vehicle_subtile_at(v: &Vehicle, pose: VehiclePose) -> (f32, f32) {
    if matches!(v.kind, VehicleKind::Train) {
        return train_straight_subtile(train_subtile_direction(v), pose.progress);
    }
    if parked_inside_bay(v, pose.pos)
        && let Some(subtile) = bay_subtile(v, pose)
    {
        return subtile;
    }
    if pose.depart_turn > 0
        && let Some(curve) = depart_u_turn_curve(v.direction)
    {
        return sample_curve(curve, pose.depart_turn);
    }
    if pose.progress == 255
        && movement_target_at(v, pose.pos, pose.path_index).is_none()
        && pose.pos == v.dest
    {
        return straight_subtile(v.direction, 255);
    }
    if let Some((entry, exit)) = road_turn_entry_exit_at(v, pose.pos, pose.path_index)
        && let Some(curve) = turn_curve(entry, exit)
    {
        return sample_curve(curve, pose.progress);
    }
    let dir = if pose.progress == 255
        && movement_target_at(v, pose.pos, pose.path_index).is_some()
        && needs_depart_turnaround_at(v, pose.pos, pose.path_index)
    {
        v.direction
    } else {
        movement_direction_at(v, pose.pos, pose.path_index)
    };
    straight_subtile(dir, pose.progress)
}

fn movement_direction_at(v: &Vehicle, pos: TileCoord, path_index: usize) -> VehicleDirection {
    let Some(next) = movement_target_at(v, pos, path_index) else {
        return v.direction;
    };
    direction_from_tile_step(pos, next)
}

fn road_turn_entry_exit_at(
    v: &Vehicle,
    pos: TileCoord,
    path_index: usize,
) -> Option<(VehicleDirection, VehicleDirection)> {
    if !v.running {
        return None;
    }
    let next = movement_target_at(v, pos, path_index)?;
    let after = v.path.get(path_index + 1).copied()?;
    let entry = direction_from_tile_step(pos, next);
    let exit = direction_from_tile_step(next, after);
    if entry == exit || entry & 1 == 0 || exit & 1 == 0 {
        return None;
    }
    Some((entry, exit))
}

/// Dirección de sprite con progreso de render (giros suaves entre ticks).
#[must_use]
pub fn vehicle_render_direction(v: &Vehicle, progress: u8) -> VehicleDirection {
    vehicle_render_direction_at(
        v,
        VehiclePose {
            pos: v.pos,
            progress,
            depart_turn: v.depart_turn,
            path_index: 0,
        },
    )
}

/// Dirección 0–7 a partir del delta entre dos puntos sub-tesela (misma regla
/// que `OpenTTD`, que orienta el sprite con `new_pos - old_pos`): en estas
/// tablas el eje x crece hacia SW y el eje y hacia SE.
fn direction_from_subtile_delta(dx: f32, dy: f32) -> Option<VehicleDirection> {
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

/// Dirección del sprite dentro de la bahía: delta entre dos muestras cercanas
/// de la trayectoria `_rv_station_*` (los lazos incluyen tramos en S que el
/// rumbo lógico de entrada no captura).
fn bay_render_direction(v: &Vehicle, pose: VehiclePose) -> Option<VehicleDirection> {
    const PROBE: u8 = 16;
    let (a, b) = if pose.progress >= 255 - PROBE {
        let mut before = pose;
        before.progress = pose.progress.saturating_sub(PROBE);
        (bay_subtile(v, before)?, bay_subtile(v, pose)?)
    } else {
        let mut after = pose;
        after.progress = pose.progress.saturating_add(PROBE);
        (bay_subtile(v, pose)?, bay_subtile(v, after)?)
    };
    direction_from_subtile_delta(b.0 - a.0, b.1 - a.1)
}

/// Dirección de sprite para una pose concreta.
#[must_use]
pub fn vehicle_render_direction_at(v: &Vehicle, pose: VehiclePose) -> VehicleDirection {
    if matches!(v.kind, VehicleKind::Train) {
        return train_subtile_direction(v);
    }
    if parked_inside_bay(v, pose.pos)
        && pose.depart_turn == 0
        && pose.progress < 255
        && let Some(dir) = bay_render_direction(v, pose)
    {
        return dir;
    }
    if pose.depart_turn > 0 {
        let outbound = movement_target_at(v, pose.pos, pose.path_index)
            .map_or(v.direction, |next| direction_from_tile_step(pose.pos, next));
        if pose.progress < 128 {
            return v.direction;
        }
        return turn_cardinal_for_render(v.direction, outbound);
    }
    let Some(next) = movement_target_at(v, pose.pos, pose.path_index) else {
        return v.direction;
    };
    let entry = direction_from_tile_step(pose.pos, next);
    if pose.progress < 128 {
        return entry;
    }
    if let Some(after) = v.path.get(pose.path_index + 1) {
        let exit = direction_from_tile_step(next, *after);
        if exit != entry {
            return turn_cardinal_for_render(entry, exit);
        }
    }
    entry
}

#[must_use]
const fn turn_cardinal_for_render(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> VehicleDirection {
    match (entry, exit) {
        (DIR_NE, DIR_SE) | (DIR_SE, DIR_NE) => crate::vehicle::DIR_E,
        (DIR_SE, DIR_SW) | (DIR_SW, DIR_SE) => crate::vehicle::DIR_S,
        (DIR_SW, DIR_NW) | (DIR_NW, DIR_SW) => crate::vehicle::DIR_W,
        (DIR_NW, DIR_NE) | (DIR_NE, DIR_NW) => crate::vehicle::DIR_N,
        _ if exit == reverse_direction(entry) => entry,
        _ => entry,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;

    use crate::map::TileCoord;
    use crate::vehicle::{Vehicle, VehicleKind};

    use super::*;

    fn ne_to_se_turn_vehicle() -> Vehicle {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(0, 2),
        );
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(0, 2)]);
        v
    }

    #[test]
    fn detects_ne_to_se_turn() {
        let v = ne_to_se_turn_vehicle();
        assert_eq!(road_turn_entry_exit(&v), Some((DIR_NE, DIR_SE)));
    }

    #[test]
    fn turn_curve_endpoints_match_openrtd_data() {
        let v = ne_to_se_turn_vehicle();
        let (entry, exit) = road_turn_entry_exit(&v).unwrap();
        let curve = turn_curve(entry, exit).unwrap();
        let start = sample_curve(curve, 0);
        let end = sample_curve(curve, 255);
        assert_eq!(start, curve[0]);
        assert_eq!(end, curve[curve.len() - 1]);
    }

    #[test]
    fn straight_tile_uses_movement_direction_not_cardinal_sprite() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        v.path = VecDeque::from([
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
        ]);
        v.progress = 200;
        assert!(road_turn_entry_exit(&v).is_none());
        let (x, y) = vehicle_subtile(&v);
        let (sx, sy) = straight_subtile(DIR_SW, 200);
        assert_eq!((x, y), (sx, sy));
    }

    #[test]
    fn train_uses_center_track_not_road_lanes() {
        let (tx, ty) = train_straight_subtile(DIR_SW, 128);
        let (rx, ry) = straight_subtile(DIR_SW, 128);
        assert!(
            (ty - TRAIN_TRACK_CENTER).abs() < 0.1,
            "eje horizontal por el centro de la vía"
        );
        assert!((ty - ry).abs() > 0.5, "no usa carril de carretera (y={ry})");
        assert!(tx > 0.0 && tx < 15.0, "avance a lo largo de x (tx={tx})");
        let _ = rx;
    }

    #[test]
    fn parked_at_station_uses_inbound_lane_end() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(15, 3),
            TileCoord::new(15, 3),
        );
        v.direction = DIR_NW;
        v.progress = 255;
        let parked = vehicle_subtile_with_progress(&v, 255);
        let inbound_end = straight_subtile(DIR_NW, 255);
        assert_eq!(parked, inbound_end);
    }

    #[test]
    fn extrapolate_crosses_tile_between_sim_ticks() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(5, 6),
            TileCoord::new(6, 6),
        );
        v.path = VecDeque::from([TileCoord::new(6, 6)]);
        v.set_cruise_speed();
        v.progress = 230;
        let pose = extrapolate_vehicle_pose(&v, 1.0);
        assert_eq!(
            pose.pos,
            TileCoord::new(6, 6),
            "extrapolación debe cruzar la tesela como haría el siguiente tick"
        );
        assert!(pose.progress < v.progress_step());
    }

    /// Camión con orden en la bahía donde está parado (entró rumbo NW).
    fn parked_in_bay_vehicle() -> Vehicle {
        let bay = TileCoord::new(4, 5);
        let mut v = Vehicle::new(0, VehicleKind::Truck, bay, bay);
        v.direction = crate::vehicle::DIR_NW;
        v.progress = 255;
        v.set_station_orders(vec![bay, TileCoord::new(10, 5)]);
        v.progress = 255;
        v
    }

    #[test]
    fn parked_in_bay_sits_on_stop_frame_of_rv_station_table() {
        let v = parked_in_bay_vehicle();
        let table = bay_station_table(crate::vehicle::DIR_NW, true).unwrap();
        assert_eq!(
            vehicle_subtile(&v),
            table.points[table.stop],
            "detenido en el stop frame de `_rv_station_left_se_far` (9,5)"
        );
    }

    #[test]
    fn bay_entry_follows_rv_station_table_from_mouth_to_stop() {
        let mut v = parked_in_bay_vehicle();
        let table = bay_station_table(crate::vehicle::DIR_NW, true).unwrap();
        v.progress = 0;
        assert_eq!(vehicle_subtile(&v), table.points[0], "entra por la boca");
        v.progress = 255;
        assert_eq!(vehicle_subtile(&v), table.points[table.stop]);
    }

    #[test]
    fn bay_exit_retraces_loop_back_to_mouth() {
        let mut v = parked_in_bay_vehicle();
        let table = bay_station_table(crate::vehicle::DIR_NW, true).unwrap();
        // Tras el giro: rumbo de salida SE hacia la carretera de acceso.
        v.direction = crate::vehicle::DIR_SE;
        v.path = VecDeque::from([TileCoord::new(4, 6)]);
        v.progress = 0;
        assert_eq!(
            vehicle_subtile(&v),
            table.points[table.stop],
            "la salida arranca en el punto de parada"
        );
        v.progress = 255;
        assert_eq!(
            vehicle_subtile(&v),
            *table.points.last().unwrap(),
            "y termina en la boca (5,15)"
        );
    }

    #[test]
    fn bay_sprite_direction_follows_loop_not_logical_heading() {
        let mut v = parked_in_bay_vehicle();
        // Mitad de la entrada SE-far: tramo transversal del lazo (x decrece →
        // componente NE), distinto del rumbo lógico NW de entrada.
        v.progress = 40;
        let pose = VehiclePose::from_vehicle(&v);
        let dir = vehicle_render_direction_at(&v, pose);
        assert_ne!(
            dir,
            crate::vehicle::DIR_SE,
            "no debe usar el rumbo de salida"
        );
        let table = bay_station_table(crate::vehicle::DIR_NW, true).unwrap();
        let (x0, _) = table.points[0];
        let (x1, _) = table.points[7];
        assert!(x1 < x0, "el tramo inicial del lazo se mueve hacia -x (NE)");
    }

    #[test]
    fn turn_midpoint_differs_from_tile_center() {
        let mut v = ne_to_se_turn_vehicle();
        v.progress = 128;
        let (x, y) = vehicle_subtile(&v);
        // Punto medio de la curva NE→SE (~(7,8)), no centro del rombo.
        assert!(x > 5.0 && x < 10.0);
        assert!(y > 6.0 && y < 11.0);
    }
}
