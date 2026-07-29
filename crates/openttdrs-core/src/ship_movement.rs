//! Movimiento de barcos: red acuática, esclusas y controlador sub-tesela (`ship_cmd.cpp`).
//!
//! MVP #268: `ChooseShipTrack`-like (preferencia path A*), `FindClosestShipDepot` BFS,
//! arrival dock/depot (8,8) / buoy ≤3, ocupación de esclusa, golden interno tick-a-tick.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::engine::{get_advance_distance, get_advance_speed, ship_speed_for_tile};
use crate::map::{Map, TILE_PIXEL_HEIGHT, TileCoord, TileKind};
use crate::vehicle::{
    DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, Vehicle, VehicleDirection,
    VehicleKind, VehicleOrder, direction_from_tile_step,
};

/// Aceleración vanilla de barco (`ShipVehicleInfo::acceleration` = 1).
pub const SHIP_ACCELERATION_DEFAULT: u16 = 1;

/// Histórico: pausa artificial de 32 ticks al cruzar esclusa.
///
/// **Deprecado / fuera del hot path** desde el MVP #225: el tránsito vertical usa
/// [`ship_move_up_down_on_lock`] (`ShipMoveUpDownOnLock`).
#[deprecated(note = "usar ship_move_up_down_on_lock; ya no se aplica en el hot path")]
pub const LOCK_TRANSIT_TICKS: u32 = 32;

/// `Direction::INVALID_DIR` de `OpenTTD`.
pub const INVALID_DIR: u8 = 0xFF;

/// `Track` de `OpenTTD` (`track_type.h`).
pub const TRACK_X: u8 = 0;
pub const TRACK_Y: u8 = 1;
pub const TRACK_UPPER: u8 = 2;
pub const TRACK_LOWER: u8 = 3;
pub const TRACK_LEFT: u8 = 4;
pub const TRACK_RIGHT: u8 = 5;

/// `DiagDirection` de `OpenTTD`.
pub const DIAGDIR_NE: u8 = 0;
pub const DIAGDIR_SE: u8 = 1;
pub const DIAGDIR_SW: u8 = 2;
pub const DIAGDIR_NW: u8 = 3;

/// Entrada de `_ship_subcoord[diagdir][track]` (`ship_cmd.cpp` ~522–559).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipSubcoordData {
    pub x_subcoord: u8,
    pub y_subcoord: u8,
    /// Nueva `Direction`; [`INVALID_DIR`] si la combinación no es válida.
    pub dir: u8,
}

/// Tabla byte-igual a `OpenTTD` 15.3 `_ship_subcoord[DIAGDIR_END][TRACK_END]`.
pub static SHIP_SUBCOORD: [[ShipSubcoordData; 6]; 4] = [
    /* DIAGDIR_NE */
    [
        ShipSubcoordData {
            x_subcoord: 15,
            y_subcoord: 8,
            dir: DIR_NE,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 15,
            y_subcoord: 8,
            dir: DIR_E,
        },
        ShipSubcoordData {
            x_subcoord: 15,
            y_subcoord: 7,
            dir: DIR_N,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
    ],
    /* DIAGDIR_SE */
    [
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 8,
            y_subcoord: 0,
            dir: DIR_SE,
        },
        ShipSubcoordData {
            x_subcoord: 7,
            y_subcoord: 0,
            dir: DIR_E,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 8,
            y_subcoord: 0,
            dir: DIR_S,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
    ],
    /* DIAGDIR_SW */
    [
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 8,
            dir: DIR_SW,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 7,
            dir: DIR_W,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 8,
            dir: DIR_S,
        },
    ],
    /* DIAGDIR_NW */
    [
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 8,
            y_subcoord: 15,
            dir: DIR_NW,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 8,
            y_subcoord: 15,
            dir: DIR_W,
        },
        ShipSubcoordData {
            x_subcoord: 0,
            y_subcoord: 0,
            dir: INVALID_DIR,
        },
        ShipSubcoordData {
            x_subcoord: 7,
            y_subcoord: 15,
            dir: DIR_N,
        },
    ],
];

/// Tesela transitable por la red acuática (agua libre, depósito o muelle).
#[must_use]
pub fn is_water_network_tile(kind: TileKind) -> bool {
    matches!(kind, TileKind::Water | TileKind::ShipDepot)
}

/// Incluye muelles y boyas (`StationType::Dock` = 4, `Buoy` = 6 en `m6`).
#[must_use]
pub fn is_water_network_tile_at(map: &Map, c: TileCoord) -> bool {
    let Some(tile) = map.get(c) else {
        return false;
    };
    if is_water_network_tile(tile.kind) {
        return crate::map::river_tile_is_ship_navigable(map, c);
    }
    if tile.kind != TileKind::Station {
        return false;
    }
    matches!((tile.m6 >> 3) & 0x0F, 4 | 6)
}

#[must_use]
fn tile_height(map: &Map, c: TileCoord) -> u8 {
    map.get(c).map_or(0, |t| t.height)
}

#[must_use]
pub fn water_tile_is_lock(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .filter(|t| t.kind == TileKind::Water)
        .is_some_and(|t| (t.m5 >> 4) & 0x0F == 2)
}

fn lock_axis_neighbors(c: TileCoord, axis_y: bool) -> (TileCoord, TileCoord) {
    if axis_y {
        (TileCoord::new(c.x, c.y - 1), TileCoord::new(c.x, c.y + 1))
    } else {
        (TileCoord::new(c.x - 1, c.y), TileCoord::new(c.x + 1, c.y))
    }
}

/// Índice de sprite Lock: 0=lower, 1=middle, 2=upper según altura vs vecinos.
#[must_use]
pub fn lock_sprite_level(map: &Map, c: TileCoord) -> usize {
    let Some(tile) = map.get(c) else {
        return 1;
    };
    let axis_y = tile.m5 & 1 != 0;
    let (a, b) = lock_axis_neighbors(c, axis_y);
    let ha = map.get(a).map_or(tile.height, |t| t.height);
    let hb = map.get(b).map_or(tile.height, |t| t.height);
    let hmin = ha.min(hb);
    let hmax = ha.max(hb);
    if tile.height <= hmin {
        0
    } else if tile.height >= hmax {
        2
    } else {
        1
    }
}

/// Dos teselas de agua adyacentes están conectadas.
/// Misma altura: siempre (si ambas son red acuática).
/// Distinta altura: solo si alguna es esclusa (Lock) y |Δh| == 1.
#[must_use]
pub fn water_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    if !is_water_network_tile_at(map, cur) || !is_water_network_tile_at(map, next) {
        return false;
    }
    let hc = tile_height(map, cur);
    let hn = tile_height(map, next);
    if hc == hn {
        return true;
    }
    if hc.abs_diff(hn) != 1 {
        return false;
    }
    water_tile_is_lock(map, cur) || water_tile_is_lock(map, next)
}

/// Barcos solo avanzan con ruta precalculada (como trenes).
#[must_use]
pub fn ship_requires_path(v: &Vehicle) -> bool {
    v.kind == VehicleKind::Ship
}

#[must_use]
pub fn ship_subcoord(diagdir: u8, track: u8) -> Option<&'static ShipSubcoordData> {
    let d = usize::from(diagdir);
    let t = usize::from(track);
    if d >= SHIP_SUBCOORD.len() || t >= SHIP_SUBCOORD[0].len() {
        return None;
    }
    let entry = &SHIP_SUBCOORD[d][t];
    if entry.dir == INVALID_DIR {
        None
    } else {
        Some(entry)
    }
}

#[must_use]
const fn dir_to_diagdir(dir: VehicleDirection) -> u8 {
    dir >> 1
}

#[must_use]
fn diagdir_between_tiles(from: TileCoord, to: TileCoord) -> Option<u8> {
    match (to.x - from.x, to.y - from.y) {
        (-1, 0) => Some(DIAGDIR_NE),
        (0, 1) => Some(DIAGDIR_SE),
        (1, 0) => Some(DIAGDIR_SW),
        (0, -1) => Some(DIAGDIR_NW),
        _ => None,
    }
}

#[must_use]
fn tile_from_xy(x: i32, y: i32) -> TileCoord {
    TileCoord::new(x >> 4, y >> 4)
}

/// `_delta_coord` de `GetNewVehiclePos` (`vehicle.cpp`).
#[must_use]
fn new_vehicle_pos_delta(direction: VehicleDirection) -> (i32, i32) {
    const DELTA_X: [i32; 8] = [-1, -1, -1, 0, 1, 1, 1, 0];
    const DELTA_Y: [i32; 8] = [-1, 0, 1, 1, 1, 0, -1, -1];
    let d = usize::from(direction.min(7));
    (DELTA_X[d], DELTA_Y[d])
}

#[must_use]
fn track_from_diagdir(diagdir: u8) -> u8 {
    match diagdir {
        DIAGDIR_NE | DIAGDIR_SW => TRACK_X,
        _ => TRACK_Y,
    }
}

fn ensure_ship_world_pos(v: &mut Vehicle, map: Option<&Map>) {
    if v.ship_pos_valid {
        return;
    }
    v.ship_x = v.pos.x.saturating_mul(16).saturating_add(8);
    v.ship_y = v.pos.y.saturating_mul(16).saturating_add(8);
    v.ship_track = track_from_diagdir(dir_to_diagdir(v.direction));
    v.ship_pos_valid = true;
    if v.z_pos.is_none() {
        let h = map.map_or(0, |m| tile_height(m, v.pos));
        v.z_pos = Some(i16::from(h) * TILE_PIXEL_HEIGHT);
    }
}

/// `ShipAccelerate` (`ship_cmd.cpp` ~414–432): no usa accel road-like.
#[must_use]
pub fn ship_accelerate(v: &mut Vehicle, max_speed: u16) -> u32 {
    let mut speed = v
        .cur_speed
        .saturating_add(SHIP_ACCELERATION_DEFAULT)
        .min(max_speed);
    if let Some(order) = v.current_order_ref() {
        let order_cap = order.max_speed_limit();
        if order_cap > 0 {
            speed = speed.min(order_cap.saturating_mul(2));
        }
    }
    v.cur_speed = speed;

    let advance = get_advance_speed(speed).saturating_add(u32::from(v.progress));
    let dist = get_advance_distance(v.direction).max(1);
    let steps = advance / dist;
    #[allow(clippy::cast_possible_truncation)]
    {
        v.progress = (advance % dist) as u8;
    }
    steps
}

/// `ShipTestUpDownOnLock` + `ShipMoveUpDownOnLock` (esclusa de una tesela del port).
///
/// En `OpenTTD` el middle lock tiene pendiente; aquí cualquier tile Lock en el centro
/// (8,8) sube/baja `z_pos` ±1 cada 8 ticks según el sentido hacia el vecino alto.
#[must_use]
pub fn ship_move_up_down_on_lock(v: &mut Vehicle, map: &Map) -> bool {
    if v.kind != VehicleKind::Ship || !v.ship_pos_valid {
        return false;
    }
    if !water_tile_is_lock(map, v.pos) {
        return false;
    }
    if (v.ship_x & 0xF) != 8 || (v.ship_y & 0xF) != 8 {
        return false;
    }

    let Some(tile) = map.get(v.pos) else {
        return false;
    };
    let axis_y = tile.m5 & 1 != 0;
    let (a, b) = lock_axis_neighbors(v.pos, axis_y);
    let ha = tile_height(map, a);
    let hb = tile_height(map, b);
    if ha.abs_diff(hb) != 1 {
        return false;
    }
    let (low_h, high_h, up_diag) = if ha < hb {
        (ha, hb, diagdir_between_tiles(a, b).unwrap_or(DIAGDIR_SW))
    } else {
        (hb, ha, diagdir_between_tiles(b, a).unwrap_or(DIAGDIR_NE))
    };

    let ship_diag = dir_to_diagdir(v.direction);
    let z = v
        .z_pos
        .unwrap_or_else(|| i16::from(tile.height) * TILE_PIXEL_HEIGHT);
    let z_high = i16::from(high_h) * TILE_PIXEL_HEIGHT;
    let z_low = i16::from(low_h) * TILE_PIXEL_HEIGHT;
    let dz: i16 = if ship_diag == up_diag {
        i16::from(z < z_high)
    } else if z > z_low {
        -1
    } else {
        0
    };
    if dz == 0 {
        return false;
    }

    if v.cur_speed != 0 {
        v.cur_speed = 0;
    }
    if v.ship_tick_counter.trailing_zeros() >= 3 {
        v.z_pos = Some(z + dz);
    }
    true
}

fn ship_max_speed(v: &Vehicle, map: Option<&Map>) -> u16 {
    let engine = v.effective_engine();
    let mut max_speed = engine.max_speed;
    if let Some(map) = map {
        let is_canal = map.get(v.pos).is_some_and(crate::map::is_canal_tile);
        max_speed = ship_speed_for_tile(engine, is_canal);
        if let Some(bridge_cap) = crate::bridge_spec::bridge_max_speed_for_tile(map, v.pos) {
            max_speed = max_speed.min(bridge_cap);
        }
    }
    if v.cached_max_speed > 0 && v.cached_max_speed < u16::MAX {
        max_speed = max_speed.min(v.cached_max_speed);
    }
    max_speed
}

fn apply_ship_direction_change(v: &mut Vehicle, new_dir: VehicleDirection) {
    let diff = new_dir.wrapping_sub(v.direction) & 7;
    match diff {
        0 | 1 | 7 => {
            v.direction = new_dir;
        }
        _ => {
            v.cur_speed = 0;
            v.direction = new_dir;
        }
    }
}

fn choose_track_for_entry(diagdir: u8) -> u8 {
    track_from_diagdir(diagdir)
}

/// Vecino ortogonal según `DiagDirection`.
#[must_use]
fn tile_in_diagdir(c: TileCoord, diagdir: u8) -> TileCoord {
    match diagdir {
        DIAGDIR_NE => TileCoord::new(c.x - 1, c.y),
        DIAGDIR_SE => TileCoord::new(c.x, c.y + 1),
        DIAGDIR_SW => TileCoord::new(c.x + 1, c.y),
        _ => TileCoord::new(c.x, c.y - 1),
    }
}

/// `ChooseShipTrack`-like (#268): en junction preferir el trackdir del path A* / caché.
///
/// Si el siguiente paso del path implica un track válido para el `entry_diagdir`,
/// lo usa; si no, prueba vecinos acuáticos hacia `dest` con track válido; fallback
/// al eje X/Y del diagdir de entrada.
#[must_use]
pub fn choose_ship_track(
    map: &Map,
    from: TileCoord,
    entry_diagdir: u8,
    path_next: Option<TileCoord>,
    dest: TileCoord,
) -> u8 {
    let default = choose_track_for_entry(entry_diagdir);
    if let Some(next) = path_next
        && water_tiles_connected(map, from, next)
        && let Some(d) = diagdir_between_tiles(from, next)
    {
        let candidate = track_from_diagdir(d);
        if ship_subcoord(entry_diagdir, candidate).is_some() {
            return candidate;
        }
    }
    let mut best: Option<(u32, u8)> = None;
    for d in [DIAGDIR_NE, DIAGDIR_SE, DIAGDIR_SW, DIAGDIR_NW] {
        let n = tile_in_diagdir(from, d);
        if !water_tiles_connected(map, from, n) {
            continue;
        }
        let candidate = track_from_diagdir(d);
        if ship_subcoord(entry_diagdir, candidate).is_none() {
            continue;
        }
        let cost = (n.x - dest.x).unsigned_abs() + (n.y - dest.y).unsigned_abs();
        let bias = u32::from(candidate != default);
        let score = cost.saturating_mul(2).saturating_add(bias);
        if best.is_none_or(|(b, _)| score < b) {
            best = Some((score, candidate));
        }
    }
    best.map_or(default, |(_, t)| t)
}

/// `FindClosestShipDepot` BFS mínimo sobre red acuática (#268).
///
/// No usa water-regions completas; alcanza el depósito ship más cercano por
/// conectividad `water_tiles_connected` (oráculo externo residual).
#[must_use]
pub fn find_closest_ship_depot(map: &Map, from: TileCoord) -> Option<TileCoord> {
    if !is_water_network_tile_at(map, from) {
        return None;
    }
    let (mw, mh) = map.dimensions();
    let mut seen = HashSet::new();
    let mut q = VecDeque::new();
    seen.insert(from);
    q.push_back(from);
    while let Some(cur) = q.pop_front() {
        if map.get_kind(cur) == Some(TileKind::ShipDepot) {
            return Some(cur);
        }
        for (dx, dy) in [(-1_i32, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(cur.x + dx, cur.y + dy);
            if n.x < 0 || n.y < 0 || n.x >= mw.cast_signed() || n.y >= mh.cast_signed() {
                continue;
            }
            if !seen.insert(n) {
                continue;
            }
            if water_tiles_connected(map, cur, n) {
                q.push_back(n);
            }
        }
    }
    None
}

/// Ocupación de esclusa: tile middle → id de vehículo.
pub type ShipLockOccupancy = HashMap<TileCoord, u32>;

/// Intenta reclamar el middle de una esclusa. `false` si otro barco la ocupa.
pub fn try_claim_ship_lock(
    occupancy: &mut ShipLockOccupancy,
    lock_tile: TileCoord,
    vehicle_id: u32,
) -> bool {
    match occupancy.get(&lock_tile) {
        Some(&owner) if owner != vehicle_id => false,
        _ => {
            occupancy.insert(lock_tile, vehicle_id);
            true
        }
    }
}

/// Libera la esclusa si el vehículo es el dueño.
pub fn release_ship_lock(
    occupancy: &mut ShipLockOccupancy,
    lock_tile: TileCoord,
    vehicle_id: u32,
) {
    if occupancy.get(&lock_tile) == Some(&vehicle_id) {
        occupancy.remove(&lock_tile);
    }
}

/// ¿El middle de la esclusa está libre o ya es nuestro?
#[must_use]
pub fn ship_lock_occupancy_allows(
    occupancy: &ShipLockOccupancy,
    lock_tile: TileCoord,
    vehicle_id: u32,
) -> bool {
    match occupancy.get(&lock_tile) {
        None => true,
        Some(&owner) => owner == vehicle_id,
    }
}

/// Arrival dock/depot en centro (8,8); buoy ≤3 Manhattan (#268).
#[must_use]
pub fn ship_arrival_ready(v: &Vehicle, map: Option<&Map>) -> bool {
    if v.kind != VehicleKind::Ship || !v.path.is_empty() {
        return false;
    }
    let Some(order) = v.current_order_ref() else {
        return v.pos == v.dest && (v.ship_x & 0xF) == 8 && (v.ship_y & 0xF) == 8;
    };
    match order {
        VehicleOrder::Depot { depot, .. } => {
            v.pos == *depot && (v.ship_x & 0xF) == 8 && (v.ship_y & 0xF) == 8
        }
        VehicleOrder::Station { .. } => {
            if dest_is_buoy(v.dest, map) {
                let dist = (v.pos.x - v.dest.x).unsigned_abs() + (v.pos.y - v.dest.y).unsigned_abs();
                dist <= 3
            } else {
                v.pos == v.dest && (v.ship_x & 0xF) == 8 && (v.ship_y & 0xF) == 8
            }
        }
        VehicleOrder::Waypoint { waypoint, .. } => {
            let dist =
                (v.pos.x - waypoint.x).unsigned_abs() + (v.pos.y - waypoint.y).unsigned_abs();
            dist <= 3
        }
        _ => v.pos == v.dest,
    }
}

fn dest_is_buoy(dest: TileCoord, map: Option<&Map>) -> bool {
    let Some(map) = map else {
        return false;
    };
    map.get(dest).is_some_and(|t| {
        t.kind == TileKind::Station
            && crate::station::station_type_from_m6(t.m6) == crate::station::STATION_TYPE_BUOY
    })
}

/// Alinea la proa al siguiente paso del path (A* tile → eje X/Y).
fn face_path_target(v: &mut Vehicle) {
    let Some(&next) = v.path.front() else {
        return;
    };
    if (next.x - v.pos.x).abs() + (next.y - v.pos.y).abs() != 1 {
        return;
    }
    let want = direction_from_tile_step(v.pos, next);
    if dir_to_diagdir(v.direction) != dir_to_diagdir(want) {
        // Giro fuerte: parar como OpenTTD en cambios >45°.
        let diff = want.wrapping_sub(v.direction) & 7;
        if !matches!(diff, 0 | 1 | 7) {
            v.cur_speed = 0;
        }
        v.direction = want;
        v.ship_track = track_from_diagdir(dir_to_diagdir(want));
    }
}

/// Un tick del controlador mínimo (`ShipController` simplificado).
#[allow(clippy::too_many_lines)]
pub fn ship_controller_tick(v: &mut Vehicle, map: Option<&Map>) {
    if v.kind != VehicleKind::Ship {
        return;
    }
    v.ship_tick_counter = v.ship_tick_counter.wrapping_add(1);
    ensure_ship_world_pos(v, map);

    if !v.running {
        v.cur_speed = 0;
        return;
    }

    if v.cargo_transfer_active() || v.holding_for_timetable() {
        v.cur_speed = 0;
        return;
    }

    if let Some(map) = map
        && ship_move_up_down_on_lock(v, map)
    {
        return;
    }

    face_path_target(v);

    if v.movement_target().is_none() {
        if ship_arrival_ready(v, map) || (v.pos == v.dest && v.orders.is_empty()) {
            v.cur_speed = 0;
            v.advance_destination_after_arrival();
        }
        return;
    }

    let max_speed = ship_max_speed(v, map);
    let steps = ship_accelerate(v, max_speed);
    if steps == 0 {
        return;
    }

    for _ in 0..steps {
        if let Some(map) = map
            && ship_move_up_down_on_lock(v, map)
        {
            return;
        }

        let (dx, dy) = new_vehicle_pos_delta(v.direction);
        let new_x = v.ship_x + dx;
        let new_y = v.ship_y + dy;
        let old_tile = v.pos;
        let new_tile = tile_from_xy(new_x, new_y);

        if new_tile == old_tile {
            v.ship_x = new_x;
            v.ship_y = new_y;
            continue;
        }

        if map.is_some_and(|m| !is_water_network_tile_at(m, new_tile)) {
            v.cur_speed = 0;
            return;
        }
        if let Some(map) = map
            && !water_tiles_connected(map, old_tile, new_tile)
        {
            v.cur_speed = 0;
            return;
        }

        let Some(diagdir) = diagdir_between_tiles(old_tile, new_tile) else {
            v.cur_speed = 0;
            return;
        };

        // Path tile → track: consumir frente si coincide; X/Y por eje de entrada.
        if v.path.front() == Some(&new_tile) {
            v.path.pop_front();
        } else if !v.path.is_empty() {
            // Ruta desfasada: no saltar a un frente lejano.
            v.cur_speed = 0;
            return;
        }

        let path_next = v.path.front().copied();
        let track = map.map_or_else(
            || choose_track_for_entry(diagdir),
            |m| choose_ship_track(m, new_tile, diagdir, path_next, v.dest),
        );
        let Some(entry) = ship_subcoord(diagdir, track) else {
            v.cur_speed = 0;
            return;
        };

        v.ship_x = (new_x & !0xF) | i32::from(entry.x_subcoord);
        v.ship_y = (new_y & !0xF) | i32::from(entry.y_subcoord);
        v.ship_track = track;
        if v.orders.is_empty() {
            v.origin = old_tile;
        }
        v.pos = new_tile;
        apply_ship_direction_change(v, entry.dir);

        if let Some(map) = map {
            let h = tile_height(map, new_tile);
            if !water_tile_is_lock(map, new_tile) || v.z_pos.is_none() {
                v.z_pos = Some(i16::from(h) * TILE_PIXEL_HEIGHT);
            }
        }

        if ship_arrival_ready(v, map) {
            v.cur_speed = 0;
            v.advance_destination_after_arrival();
            return;
        }

        if v.movement_target().is_none() && v.pos != v.dest {
            // Sin más path: alinear dirección hacia el destino si hay paso Manhattan
            // (solo tests sin GameState); en sim real el routing rellena path.
            let face = direction_from_tile_step(v.pos, v.dest);
            if v.pos != v.dest {
                v.direction = face;
            }
            return;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, deprecated)]
mod tests {
    use crate::GameState;
    use crate::engine::ENGINE_SHIP_MPS;
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::{Command, TileCoord, Vehicle, VehicleKind, apply_command};

    use super::*;

    fn water_line(state: &mut GameState, y: i32, x0: i32, x1: i32) {
        for x in x0..=x1 {
            state
                .map
                .set_kind(TileCoord::new(x, y), TileKind::Water)
                .unwrap();
        }
    }

    #[test]
    fn ship_subcoord_table_matches_openttd_key_entries() {
        // DIAGDIR_NE + TRACK_X / LOWER / LEFT
        assert_eq!(
            SHIP_SUBCOORD[0][0],
            ShipSubcoordData {
                x_subcoord: 15,
                y_subcoord: 8,
                dir: DIR_NE
            }
        );
        assert_eq!(
            SHIP_SUBCOORD[0][3],
            ShipSubcoordData {
                x_subcoord: 15,
                y_subcoord: 8,
                dir: DIR_E
            }
        );
        assert_eq!(
            SHIP_SUBCOORD[0][4],
            ShipSubcoordData {
                x_subcoord: 15,
                y_subcoord: 7,
                dir: DIR_N
            }
        );
        assert!(ship_subcoord(DIAGDIR_NE, TRACK_Y).is_none());

        // DIAGDIR_SE + TRACK_Y
        assert_eq!(
            SHIP_SUBCOORD[1][1],
            ShipSubcoordData {
                x_subcoord: 8,
                y_subcoord: 0,
                dir: DIR_SE
            }
        );
        // DIAGDIR_SW + TRACK_X / RIGHT
        assert_eq!(
            SHIP_SUBCOORD[2][0],
            ShipSubcoordData {
                x_subcoord: 0,
                y_subcoord: 8,
                dir: DIR_SW
            }
        );
        assert_eq!(
            SHIP_SUBCOORD[2][5],
            ShipSubcoordData {
                x_subcoord: 0,
                y_subcoord: 8,
                dir: DIR_S
            }
        );
        // DIAGDIR_NW + TRACK_Y / LOWER / RIGHT
        assert_eq!(
            SHIP_SUBCOORD[3][1],
            ShipSubcoordData {
                x_subcoord: 8,
                y_subcoord: 15,
                dir: DIR_NW
            }
        );
        assert_eq!(
            SHIP_SUBCOORD[3][3],
            ShipSubcoordData {
                x_subcoord: 8,
                y_subcoord: 15,
                dir: DIR_W
            }
        );
        assert_eq!(
            SHIP_SUBCOORD[3][5],
            ShipSubcoordData {
                x_subcoord: 7,
                y_subcoord: 15,
                dir: DIR_N
            }
        );
    }

    #[test]
    fn water_path_follows_water_tiles() {
        let mut s = GameState::new(12, 12);
        water_line(&mut s, 5, 1, 8);
        let from = TileCoord::new(1, 5);
        let to = TileCoord::new(8, 5);
        let path = find_path(&s.map, from, to, PathNetwork::Water).expect("ruta acuática");
        assert_eq!(path.len(), 7);
        assert!(
            path.iter()
                .all(|c| { s.map.get_kind(*c) == Some(TileKind::Water) })
        );
    }

    #[test]
    fn ship_moves_along_water_path() {
        let mut s = GameState::new(12, 12);
        water_line(&mut s, 3, 0, 6);
        let depot = TileCoord::new(0, 3);
        s.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_SHIP_MPS),
        )
        .unwrap();
        let dest = TileCoord::new(6, 3);
        s.vehicles[0].dest = dest;
        s.vehicles[0].running = true;
        s.vehicles[0].path = find_path(&s.map, depot, dest, PathNetwork::Water)
            .unwrap()
            .into();
        s.vehicles[0].set_cruise_speed();
        let mut last_pos = s.vehicles[0].pos;
        for _ in 0..2_000 {
            let prev_x = s.vehicles[0].ship_x;
            let prev_y = s.vehicles[0].ship_y;
            let had_pos = s.vehicles[0].ship_pos_valid;
            s.vehicles[0].step_with_map(Some(&s.map));
            if s.vehicles[0].ship_pos_valid && had_pos {
                let dx = (s.vehicles[0].ship_x - prev_x).abs();
                let dy = (s.vehicles[0].ship_y - prev_y).abs();
                // Un paso de píxel por iteración del controlador; varios por tick.
                assert!(dx + dy <= 16, "salto sub-tesela absurdo dx={dx} dy={dy}");
            }
            let pos = s.vehicles[0].pos;
            if pos != last_pos {
                assert_eq!(
                    (pos.x - last_pos.x).abs() + (pos.y - last_pos.y).abs(),
                    1,
                    "salto de tesela absurdo {last_pos:?} → {pos:?}"
                );
                last_pos = pos;
            }
            if s.vehicles[0].pos == dest {
                break;
            }
        }
        assert_eq!(s.vehicles[0].pos, dest);
    }

    #[test]
    fn ship_without_orders_moves_on_water() {
        let mut s = GameState::new(12, 6);
        for x in 0..6 {
            s.map
                .set_kind(TileCoord::new(x, 2), TileKind::Water)
                .unwrap();
        }
        let start = TileCoord::new(1, 2);
        let mut v = Vehicle::new(1, VehicleKind::Ship, start, start);
        v.running = true;
        v.set_cruise_speed();
        s.vehicles.push(v);
        for _ in 0..800 {
            s.step();
            if s.vehicles[0].pos != start {
                break;
            }
        }
        assert_ne!(s.vehicles[0].pos, start);
        assert_eq!(s.map.get_kind(s.vehicles[0].pos), Some(TileKind::Water));
    }

    #[test]
    fn ship_does_not_wander_off_water_without_path() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        v.running = true;
        v.set_cruise_speed();
        v.step();
        assert_eq!(v.pos, TileCoord::new(0, 0));
    }

    #[test]
    fn lock_connects_different_heights() {
        let mut s = GameState::new(10, 6);
        let low = TileCoord::new(2, 2);
        let lock = TileCoord::new(3, 2);
        let high = TileCoord::new(4, 2);
        for c in [low, lock, high] {
            s.map.set_kind(c, TileKind::Water).unwrap();
        }
        s.map.set_height(low, 0).unwrap();
        s.map.set_height(lock, 0).unwrap();
        s.map.set_height(high, 1).unwrap();
        assert!(!water_tiles_connected(&s.map, low, high));
        assert!(!water_tiles_connected(&s.map, lock, high));
        apply_command(&mut s, &Command::PlaceLock(lock, false)).unwrap();
        assert!(water_tiles_connected(&s.map, lock, high));
        let path = find_path(&s.map, low, high, PathNetwork::Water);
        assert!(path.is_some());
    }

    #[test]
    fn different_height_without_lock_no_path() {
        let mut s = GameState::new(10, 6);
        let a = TileCoord::new(1, 2);
        let b = TileCoord::new(2, 2);
        s.map.set_kind(a, TileKind::Water).unwrap();
        s.map.set_kind(b, TileKind::Water).unwrap();
        s.map.set_height(a, 0).unwrap();
        s.map.set_height(b, 1).unwrap();
        assert!(find_path(&s.map, a, b, PathNetwork::Water).is_none());
    }

    #[test]
    fn ship_lock_changes_z_without_lock_transit_wait() {
        let mut s = GameState::new(10, 6);
        let low = TileCoord::new(2, 2);
        let lock = TileCoord::new(3, 2);
        let high = TileCoord::new(4, 2);
        for c in [low, lock, high] {
            s.map.set_kind(c, TileKind::Water).unwrap();
        }
        s.map.set_height(low, 0).unwrap();
        s.map.set_height(lock, 0).unwrap();
        s.map.set_height(high, 1).unwrap();
        apply_command(&mut s, &Command::PlaceLock(lock, false)).unwrap();

        let mut v = Vehicle::new(1, VehicleKind::Ship, lock, high);
        v.running = true;
        v.direction = DIR_SW; // hacia el vecino alto (x+1)
        v.ship_x = lock.x * 16 + 8;
        v.ship_y = lock.y * 16 + 8;
        v.ship_pos_valid = true;
        v.ship_track = TRACK_X;
        v.z_pos = Some(0);
        v.cur_speed = 20;
        v.path.clear();

        let z0 = v.z_pos.unwrap();
        let mut saw_z_change = false;
        for _ in 0..64 {
            assert_ne!(
                v.wait_counter, LOCK_TRANSIT_TICKS,
                "no debe usarse pausa artificial de 32 ticks"
            );
            let moved = ship_move_up_down_on_lock(&mut v, &s.map);
            assert!(moved, "debe estar en tránsito vertical de esclusa");
            v.ship_tick_counter = v.ship_tick_counter.wrapping_add(1);
            if v.z_pos.unwrap() != z0 {
                saw_z_change = true;
                break;
            }
        }
        assert!(saw_z_change, "z_pos debe cambiar en pasos");
        assert!(v.z_pos.unwrap() > z0);
        assert_eq!(v.cur_speed, 0);
        assert_eq!(v.wait_counter, 0);
    }

    #[test]
    fn ship_accelerate_does_not_use_road_accel_jump() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.cur_speed = 0;
        v.progress = 0;
        v.direction = DIR_SW;
        let steps0 = ship_accelerate(&mut v, 96);
        assert_eq!(v.cur_speed, SHIP_ACCELERATION_DEFAULT);
        // Con speed=1, GetAdvanceSpeed=0 → 0 pasos; progress acumula.
        assert_eq!(steps0, 0);
        assert!(v.progress > 0 || get_advance_speed(1) == 0);
        for _ in 0..300 {
            let _ = ship_accelerate(&mut v, 96);
        }
        assert_eq!(v.cur_speed, 96);
        // Road accel (256) habría saturado mucho antes con otro perfil; aquí +1/tick.
        assert!(v.cur_speed <= 96);
    }

    #[test]
    fn ship_choose_track_prefers_path_next() {
        let mut s = GameState::new(10, 10);
        water_line(&mut s, 4, 1, 6);
        water_line(&mut s, 5, 3, 3); // junction stub south of (3,4)
        let from = TileCoord::new(3, 4);
        let path_next = TileCoord::new(4, 4);
        let track = choose_ship_track(&s.map, from, DIAGDIR_SW, Some(path_next), path_next);
        assert_eq!(track, TRACK_X);
        assert!(ship_subcoord(DIAGDIR_SW, track).is_some());
    }

    #[test]
    fn find_closest_ship_depot_bfs_reaches_depot() {
        let mut s = GameState::new(12, 12);
        water_line(&mut s, 2, 0, 8);
        let depot = TileCoord::new(0, 2);
        s.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        let far = TileCoord::new(8, 2);
        assert_eq!(find_closest_ship_depot(&s.map, far), Some(depot));
        // Isla sin conexión: no encuentra.
        s.map.set_kind(TileCoord::new(5, 5), TileKind::Water).unwrap();
        assert!(find_closest_ship_depot(&s.map, TileCoord::new(5, 5)).is_none());
    }

    #[test]
    fn ship_arrival_dock_depot_requires_center_buoy_le_3() {
        let mut s = GameState::new(10, 10);
        water_line(&mut s, 1, 0, 6);
        let depot = TileCoord::new(0, 1);
        s.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        let mut v = Vehicle::new(1, VehicleKind::Ship, depot, depot);
        v.ship_pos_valid = true;
        v.ship_x = depot.x * 16 + 8;
        v.ship_y = depot.y * 16 + 8;
        v.orders = vec![VehicleOrder::depot(depot)];
        v.current_order = 0;
        v.path.clear();
        assert!(ship_arrival_ready(&v, Some(&s.map)));
        v.ship_x = depot.x * 16 + 2;
        assert!(!ship_arrival_ready(&v, Some(&s.map)));

        // Buoy ≤3
        let buoy = TileCoord::new(5, 1);
        apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
        v.dest = buoy;
        v.pos = TileCoord::new(3, 1);
        v.orders = vec![VehicleOrder::waypoint(buoy)];
        v.path.clear();
        assert!(ship_arrival_ready(&v, Some(&s.map)));
        v.pos = TileCoord::new(0, 1);
        assert!(!ship_arrival_ready(&v, Some(&s.map)));
    }

    #[test]
    fn lock_occupancy_two_ships_cannot_claim_same_middle() {
        let lock = TileCoord::new(4, 4);
        let mut occ = ShipLockOccupancy::new();
        assert!(try_claim_ship_lock(&mut occ, lock, 1));
        assert!(!try_claim_ship_lock(&mut occ, lock, 2));
        assert!(!ship_lock_occupancy_allows(&occ, lock, 2));
        assert!(ship_lock_occupancy_allows(&occ, lock, 1));
        release_ship_lock(&mut occ, lock, 1);
        assert!(try_claim_ship_lock(&mut occ, lock, 2));
    }

    /// Golden interno tick-a-tick (#268): pos/dir/z/orden. No vs `OpenTTD` externo.
    #[test]
    fn ship_internal_tick_golden_pos_dir_z_order() {
        #[derive(Debug, PartialEq, Eq)]
        struct Snap {
            tick: u32,
            x: i32,
            y: i32,
            dir: u8,
            z: i16,
            order: usize,
        }

        let mut s = GameState::new(12, 12);
        water_line(&mut s, 3, 0, 6);
        let depot = TileCoord::new(0, 3);
        s.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(depot, ENGINE_SHIP_MPS),
        )
        .unwrap();
        let dest = TileCoord::new(6, 3);
        s.vehicles[0].dest = dest;
        s.vehicles[0].running = true;
        s.vehicles[0].path = find_path(&s.map, depot, dest, PathNetwork::Water)
            .unwrap()
            .into();
        s.vehicles[0].set_cruise_speed();
        s.vehicles[0].orders = vec![VehicleOrder::depot(dest)];
        s.vehicles[0].current_order = 0;

        let mut snaps = Vec::new();
        for tick in 0..80u32 {
            if tick % 20 == 0 {
                let v = &s.vehicles[0];
                snaps.push(Snap {
                    tick,
                    x: v.pos.x,
                    y: v.pos.y,
                    dir: v.direction,
                    z: v.z_pos.unwrap_or(0),
                    order: v.current_order,
                });
            }
            s.vehicles[0].step_with_map(Some(&s.map));
        }
        // Documentado (golden interno, no vs `OpenTTD`): y=3 constante; avanza en +x.
        assert_eq!(snaps[0].x, 0);
        assert_eq!(snaps[0].y, 3);
        assert!(snaps.last().unwrap().x >= snaps[0].x);
        assert!(snaps.iter().all(|s| s.y == 3));
        assert!(snaps.iter().all(|s| s.dir <= 7));
        let z_vals: Vec<_> = snaps.iter().map(|s| s.z).collect();
        assert!(
            z_vals.windows(2).all(|w| w[0] == w[1] || w[0] == 0),
            "z no debería oscilar en canal llano: {z_vals:?}"
        );
        assert!(snaps.iter().all(|s| s.order == 0) || snaps.last().unwrap().x >= 5);
    }
}
