use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::aircraft_movement::straight_line_path;
use crate::map::{Map, Tile, TileCoord, TileKind, openttd_tile_index_to_coord};
use crate::ship_movement::{is_water_network_tile, water_tiles_connected};
use crate::station::is_rail_waypoint_tile;
use crate::tnbp_decode::JgrTunnelRecord;
use crate::vehicle::VehicleKind;

mod yapf;

const RAIL_TB_X: u8 = 0x01;
const RAIL_TB_Y: u8 = 0x02;

fn is_any_transport_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road
            | TileKind::Rail
            | TileKind::RoadDepot
            | TileKind::RailDepot
            | TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge
    )
}

fn is_road_stop_station(tile: &Tile) -> bool {
    tile.kind == TileKind::Station && matches!((tile.m6 >> 3) & 0x0F, 2 | 3)
}

fn is_rail_station_tile(tile: &Tile) -> bool {
    tile.kind == TileKind::Station && (tile.m6 >> 3).trailing_zeros() >= 4
}

fn is_network_tile(map: &Map, c: TileCoord, kind: TileKind, network: PathNetwork) -> bool {
    match network {
        PathNetwork::Road => is_road_network_tile(kind) || is_road_stop_station_tile(map, c),
        // Trenes no circulan por la tesela de plataforma; paran en la vía adyacente.
        PathNetwork::Rail => is_rail_network_tile(kind),
        PathNetwork::Water => is_water_network_tile(kind),
        PathNetwork::Air => true,
    }
}

/// Red de transporte para pathfinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathNetwork {
    Road,
    Rail,
    Water,
    Air,
}

#[must_use]
pub const fn path_network_for_vehicle(kind: VehicleKind) -> PathNetwork {
    match kind {
        VehicleKind::Train => PathNetwork::Rail,
        VehicleKind::Truck | VehicleKind::Bus => PathNetwork::Road,
        VehicleKind::Ship => PathNetwork::Water,
        VehicleKind::Aircraft => PathNetwork::Air,
    }
}

/// Misma condición que los tiles intermedios del pathfinder (carretera/vía/túnel/puente).
#[must_use]
pub fn tile_is_path_traversable(kind: TileKind) -> bool {
    is_any_transport_tile(kind)
}

/// Offset de tesela vecina hacia donde apunta la entrada (`OpenTTD` `TileOffsByDiagDir`).
#[must_use]
pub const fn diag_dir_offset(dir: u8) -> (i32, i32) {
    const OFFSETS: [(i32, i32); 4] = [
        (-1, 0), // DIAGDIR_NE
        (0, 1),  // DIAGDIR_SE
        (1, 0),  // DIAGDIR_SW
        (0, -1), // DIAGDIR_NW
    ];
    OFFSETS[dir as usize & 3]
}

#[must_use]
#[inline]
fn is_road_network_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
    )
}

/// Parada bus/camión con boca a carretera (`m3` = road bits de acceso).
#[must_use]
fn is_road_stop_station_tile(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .is_some_and(|t| is_road_stop_station(&t) && (t.m3 & 0x0F) != 0)
}

#[must_use]
#[inline]
pub(crate) fn is_rail_network_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
    )
}

#[must_use]
pub fn station_site_adjacent_to_rail(map: &Map, c: TileCoord) -> bool {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if map
            .get_kind(n)
            .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn station_site_adjacent_to_transport(map: &Map, c: TileCoord) -> bool {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        let k = map.get_kind(n).unwrap_or(TileKind::Grass);
        if is_road_network_tile(k) {
            return true;
        }
    }
    false
}

#[must_use]
pub fn station_site_tile_allows_build(kind: TileKind) -> bool {
    matches!(kind, TileKind::Grass | TileKind::Forest)
}

#[must_use]
pub fn station_site_tile_needs_clear(kind: TileKind) -> bool {
    matches!(kind, TileKind::Forest)
}

#[must_use]
pub fn station_entrance_faces_road(map: &Map, c: TileCoord, dir: u8) -> bool {
    let (dx, dy) = diag_dir_offset(dir);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    map.get_kind(n).is_some_and(is_road_network_tile)
}

#[must_use]
pub fn station_entrance_faces_rail(map: &Map, c: TileCoord, dir: u8) -> bool {
    let (dx, dy) = diag_dir_offset(dir);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    map.get_kind(n)
        .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
}

/// Enlaces «wormhole» entre entradas de túnel (p. ej. pool JGR `tile_n` ↔ `tile_s`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TunnelWormholes {
    links: HashMap<TileCoord, TileCoord>,
}

impl TunnelWormholes {
    /// Construye enlaces bidireccionales desde registros JGR y dimensiones del mapa.
    #[must_use]
    pub fn from_jgr_records(map: &Map, records: &[JgrTunnelRecord]) -> Self {
        let (w, h) = map.dimensions();
        let mut links = HashMap::new();
        for r in records {
            if let (Some(a), Some(b)) = (
                openttd_tile_index_to_coord(r.tile_n, w, h),
                openttd_tile_index_to_coord(r.tile_s, w, h),
            ) {
                links.insert(a, b);
                links.insert(b, a);
            }
        }
        Self { links }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    #[must_use]
    pub fn other_end(&self, c: TileCoord) -> Option<TileCoord> {
        self.links.get(&c).copied()
    }
}

/// Bit de carretera que sale de `cur` hacia `next` (`road_bits_toward_neighbor` del cliente).
#[must_use]
const fn road_bits_toward_neighbor(dx: i32, dy: i32) -> u8 {
    match (dx, dy) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0x0F,
    }
}

#[must_use]
fn effective_road_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => {
            let bits = t.m5 & 0x0F;
            if bits == 0 { 0x0F } else { bits }
        }
        TileKind::Station if is_road_stop_station(&t) => t.m3 & 0x0F,
        _ => 0,
    }
}

const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;

/// Trackbit que conecta dos lados (`DiagDir`) de una tesela (`track_type.h`):
/// X = NE↔SW, Y = SE↔NW, UPPER = NE↔NW, LOWER = SE↔SW, LEFT = SW↔NW, RIGHT = NE↔SE.
#[must_use]
pub const fn rail_bit_for_sides(a: u8, b: u8) -> u8 {
    let (lo, hi) = if (a & 3) < (b & 3) {
        (a & 3, b & 3)
    } else {
        (b & 3, a & 3)
    };
    match (lo, hi) {
        (0, 2) => 0x01, // X
        (1, 3) => 0x02, // Y
        (0, 3) => 0x04, // UPPER
        (1, 2) => 0x08, // LOWER
        (2, 3) => 0x10, // LEFT
        (0, 1) => 0x20, // RIGHT
        _ => 0,
    }
}

/// Máscara de trackbits que tocan un lado (`_track_bits_by_diagdir` de `OpenTTD`).
#[must_use]
const fn rail_bits_touching_side(side: u8) -> u8 {
    match side & 3 {
        0 => 0x25, // NE: X | UPPER | RIGHT
        1 => 0x2A, // SE: Y | LOWER | RIGHT
        2 => 0x19, // SW: X | LOWER | LEFT
        _ => 0x16, // NW: Y | UPPER | LEFT
    }
}

#[must_use]
const fn opposite_dir(d: u8) -> u8 {
    (d + 2) & 3
}

/// Trackbits transitables de una tesela de la red ferroviaria (sin depósitos,
/// que se tratan aparte porque solo conectan por su boca).
#[must_use]
fn rail_traversal_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Rail => {
            let tb = t.m5 & 0x3F;
            if tb == 0 { RAIL_TB_X } else { tb }
        }
        TileKind::RailTunnel | TileKind::RailBridge => RAIL_TB_CROSS,
        // Andenes y waypoints: vía a lo largo del eje en `m5` bit 0.
        TileKind::Station if is_rail_station_tile(&t) || is_rail_waypoint_tile(&t) => {
            if t.m5 & 1 != 0 { RAIL_TB_Y } else { RAIL_TB_X }
        }
        _ => 0,
    }
}

/// Boca del depósito de vía (`m5 & 3`) si la tesela es un depósito.
#[must_use]
fn rail_depot_mouth(map: &Map, c: TileCoord) -> Option<u8> {
    map.get(c)
        .filter(|t| t.kind == TileKind::RailDepot)
        .map(|t| t.m5 & 0x03)
}

#[must_use]
fn road_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    let exit = road_bits_toward_neighbor(dx, dy);
    let entry = road_bits_toward_neighbor(-dx, -dy);
    let cur_bits = effective_road_bits(map, cur);
    let next_bits = effective_road_bits(map, next);
    cur_bits & exit != 0 && next_bits & entry != 0
}

#[must_use]
fn rail_station_entrance_links_track(map: &Map, station: TileCoord, track: TileCoord) -> bool {
    let Some(tile) = map.get(station) else {
        return false;
    };
    if !is_rail_station_tile(&tile) {
        return false;
    }
    if !map
        .get_kind(track)
        .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
    {
        return false;
    }
    if station.x.abs_diff(track.x) + station.y.abs_diff(track.y) != 1 {
        return false;
    }
    (0..4).any(|dir| {
        let (dx, dy) = diag_dir_offset(dir);
        TileCoord::new(station.x + dx, station.y + dy) == track
            && station_entrance_faces_rail(map, station, dir)
    })
}

#[must_use]
fn tiles_connected(map: &Map, cur: TileCoord, next: TileCoord, network: PathNetwork) -> bool {
    match network {
        PathNetwork::Road => road_tiles_connected(map, cur, next),
        PathNetwork::Water => water_tiles_connected(map, cur, next),
        PathNetwork::Rail | PathNetwork::Air => {
            debug_assert!(false, "rail/air no usan tiles_connected genérico");
            false
        }
    }
}

/// Caché de rutas por tick (no se serializa; se invalida al avanzar la simulación).
#[derive(Debug, Default, Clone)]
pub struct PathCache {
    tick: u64,
    entries: HashMap<(i32, i32, i32, i32, u8), Vec<TileCoord>>,
}

impl PathCache {
    const MAX_ENTRIES: usize = 256;

    pub fn begin_tick(&mut self, tick: u64) {
        if self.tick != tick {
            self.entries.clear();
            self.tick = tick;
        }
    }

    #[must_use]
    pub fn get(
        &self,
        from: TileCoord,
        to: TileCoord,
        network: PathNetwork,
    ) -> Option<&Vec<TileCoord>> {
        let key = cache_key(from, to, network);
        self.entries.get(&key)
    }

    pub fn insert(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        network: PathNetwork,
        path: Vec<TileCoord>,
    ) {
        if self.entries.len() >= Self::MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries.insert(cache_key(from, to, network), path);
    }
}

#[must_use]
fn cache_key(from: TileCoord, to: TileCoord, network: PathNetwork) -> (i32, i32, i32, i32, u8) {
    (
        from.x,
        from.y,
        to.x,
        to.y,
        match network {
            PathNetwork::Road => 0,
            PathNetwork::Rail => 1,
            PathNetwork::Water => 2,
            PathNetwork::Air => 3,
        },
    )
}

/// Encuentra el camino más corto entre `from` y `to` (A* con conectividad por
/// road/track bits); ver [`find_path_with_wormholes`].
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn find_path(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
) -> Option<Vec<TileCoord>> {
    find_path_with_wormholes(map, from, to, network, None)
}

/// Encuentra el camino más corto entre `from` y `to` usando A* sobre una sola red (`Road…` o `Rail…`).
///
/// Los tiles `from` y `to` pueden ser de cualquier tipo (industria, estación, etc.);
/// los tiles **intermedios** deben pertenecer a la red elegida.
///
/// Con `wormholes`, una tesela en la red puede saltar a su pareja JGR en un paso (túnel real).
///
/// Devuelve `Some(path)` donde `path` es la secuencia de teselas desde la primera adyacente
/// a `from` hasta `to` inclusive. Si `from == to` devuelve `Some(vec![])`.
/// Devuelve `None` si no existe camino.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn find_path_with_wormholes(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(vec![]);
    }
    if network == PathNetwork::Rail {
        return find_rail_path(map, from, to, wormholes);
    }
    if network == PathNetwork::Air {
        return Some(straight_line_path(from, to));
    }
    if network == PathNetwork::Water {
        return find_water_path(map, from, to);
    }

    let (mw, mh) = map.dimensions();
    let mut g_score: HashMap<TileCoord, u32> = HashMap::new();
    let mut parent: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut heap = BinaryHeap::new();

    g_score.insert(from, 0);
    parent.insert(from, from);
    heap.push(AstarNode {
        est_total: manhattan(from, to),
        pos: from,
    });

    let dirs = [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)];

    while let Some(AstarNode {
        est_total: _,
        pos: cur,
    }) = heap.pop()
    {
        if cur == to {
            return Some(reconstruct(from, to, &parent));
        }

        let cur_g = g_score[&cur];
        for (dx, dy) in dirs {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw as i32 || next.y >= mh as i32 {
                continue;
            }
            let next_kind = map.get_kind(next).unwrap_or(TileKind::Grass);
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            let reachable = if is_network_tile(map, next, next_kind, network) {
                tiles_connected(map, cur, next, network)
            } else if network == PathNetwork::Road && next == to {
                // Paradas bus/camión: la tesela de destino puede no ser carretera pura.
                is_network_tile(map, cur, cur_kind, network)
            } else {
                false
            };
            if !reachable {
                continue;
            }

            let tentative = cur_g + step_cost(cur, next);
            if g_score.get(&next).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(next, tentative);
            parent.insert(next, cur);
            let f = tentative + manhattan(next, to);
            heap.push(AstarNode {
                est_total: f,
                pos: next,
            });
        }
        if let Some(wh) = wormholes {
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            if is_network_tile(map, cur, cur_kind, network)
                && let Some(other) = wh.other_end(cur)
            {
                let other_kind = map.get_kind(other).unwrap_or(TileKind::Grass);
                let reachable = is_network_tile(map, other, other_kind, network) || other == to;
                let tentative = cur_g + step_cost(cur, other);
                if reachable && g_score.get(&other).is_none_or(|&g| tentative < g) {
                    g_score.insert(other, tentative);
                    parent.insert(other, cur);
                    heap.push(AstarNode {
                        est_total: tentative + manhattan(other, to),
                        pos: other,
                    });
                }
            }
        }
    }
    None
}

/// Lado de entrada «libre»: el tren parte (o se rematerializa) sin restricción de giro.
const SIDE_ANY: u8 = 4;

/// A* sobre teselas de agua (vecinos ortogonales conectados).
#[must_use]
#[allow(clippy::cast_possible_wrap)]
fn find_water_path(map: &Map, from: TileCoord, to: TileCoord) -> Option<Vec<TileCoord>> {
    let (mw, mh) = map.dimensions();
    let mut g_score: HashMap<TileCoord, u32> = HashMap::new();
    let mut parent: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut heap = BinaryHeap::new();

    g_score.insert(from, 0);
    parent.insert(from, from);
    heap.push(AstarNode {
        est_total: manhattan(from, to),
        pos: from,
    });

    let dirs = [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)];

    while let Some(AstarNode {
        est_total: _,
        pos: cur,
    }) = heap.pop()
    {
        if cur == to {
            return Some(reconstruct(from, to, &parent));
        }

        let cur_g = g_score[&cur];
        for (dx, dy) in dirs {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw as i32 || next.y >= mh as i32 {
                continue;
            }
            let next_kind = map.get_kind(next).unwrap_or(TileKind::Grass);
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            let reachable = if is_network_tile(map, next, next_kind, PathNetwork::Water) {
                water_tiles_connected(map, cur, next)
            } else if next == to {
                is_network_tile(map, cur, cur_kind, PathNetwork::Water)
            } else {
                false
            };
            if !reachable {
                continue;
            }

            let tentative = cur_g + step_cost(cur, next);
            if g_score.get(&next).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(next, tentative);
            parent.insert(next, cur);
            heap.push(AstarNode {
                est_total: tentative + manhattan(next, to),
                pos: next,
            });
        }
    }
    None
}

/// A* direccional para vía: estado = (tesela, lado de entrada). Un giro dentro
/// de una tesela solo es válido si existe el trackbit que conecta el lado de
/// entrada con el de salida (piezas X/Y/curvas de `OpenTTD`). Los depósitos solo
/// conectan por su boca y las plataformas solo se usan como origen.
fn find_rail_path(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    yapf::find_rail_path_yapf(map, from, to, wormholes)
}

#[allow(dead_code)]
fn find_rail_path_legacy_astar(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    let mut g_score: HashMap<(TileCoord, u8), u32> = HashMap::new();
    let mut parent: HashMap<(TileCoord, u8), (TileCoord, u8)> = HashMap::new();
    let mut heap = BinaryHeap::new();

    let start = (from, SIDE_ANY);
    g_score.insert(start, 0);
    parent.insert(start, start);
    heap.push(RailAstarNode {
        est_total: manhattan(from, to),
        pos: from,
        in_side: SIDE_ANY,
    });

    while let Some(RailAstarNode {
        est_total: _,
        pos: cur,
        in_side,
    }) = heap.pop()
    {
        if cur == to {
            return Some(reconstruct_rail(from, (cur, in_side), &parent));
        }
        let cur_g = g_score[&(cur, in_side)];
        let cur_is_start = cur == from && in_side == SIDE_ANY;

        // Un tren que llega a un depósito termina ahí; solo se sale por la boca.
        let depot_mouth = rail_depot_mouth(map, cur);
        if depot_mouth.is_some() && !cur_is_start {
            continue;
        }
        let station_start = cur_is_start && map.get(cur).is_some_and(|t| is_rail_station_tile(&t));
        let cur_bits = rail_traversal_bits(map, cur);

        for out in 0..4u8 {
            let exit_allowed = if let Some(mouth) = depot_mouth {
                out == mouth
            } else if station_start {
                let (dx, dy) = diag_dir_offset(out);
                rail_station_entrance_links_track(map, cur, TileCoord::new(cur.x + dx, cur.y + dy))
            } else if in_side == SIDE_ANY {
                cur_bits & rail_bits_touching_side(out) != 0
            } else {
                cur_bits & rail_bit_for_sides(in_side, out) != 0
            };
            if !exit_allowed {
                continue;
            }
            let (dx, dy) = diag_dir_offset(out);
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if map.get_kind(next).is_none() {
                continue;
            }
            let entry = opposite_dir(out);
            let next_in = if let Some(mouth) = rail_depot_mouth(map, next) {
                if entry != mouth {
                    continue; // al depósito solo se entra por la boca
                }
                entry
            } else if station_start {
                // El enlace plataforma → vía deja al tren sobre la vía sin
                // restricción de giro (abstracción de estación).
                SIDE_ANY
            } else if rail_traversal_bits(map, next) & rail_bits_touching_side(entry) != 0 {
                entry
            } else {
                continue;
            };
            let tentative = cur_g + 1;
            let key = (next, next_in);
            if g_score.get(&key).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(key, tentative);
            parent.insert(key, (cur, in_side));
            heap.push(RailAstarNode {
                est_total: tentative + manhattan(next, to),
                pos: next,
                in_side: next_in,
            });
        }

        // Wormholes (túneles JGR): el túnel es recto, conserva el lado de entrada.
        if let Some(wh) = wormholes
            && map.get_kind(cur).is_some_and(is_rail_network_tile)
            && let Some(other) = wh.other_end(cur)
        {
            let ok = map.get_kind(other).is_some_and(is_rail_network_tile) || other == to;
            let tentative = cur_g + 1;
            let key = (other, in_side);
            if ok && g_score.get(&key).is_none_or(|&g| tentative < g) {
                g_score.insert(key, tentative);
                parent.insert(key, (cur, in_side));
                heap.push(RailAstarNode {
                    est_total: tentative + manhattan(other, to),
                    pos: other,
                    in_side,
                });
            }
        }
    }
    None
}

fn reconstruct_rail(
    from: TileCoord,
    goal: (TileCoord, u8),
    parent: &HashMap<(TileCoord, u8), (TileCoord, u8)>,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = goal;
    while cur.0 != from {
        path.push(cur.0);
        cur = parent[&cur];
    }
    path.reverse();
    path
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct RailAstarNode {
    est_total: u32,
    pos: TileCoord,
    in_side: u8,
}

impl Ord for RailAstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est_total
            .cmp(&self.est_total)
            .then_with(|| other.pos.x.cmp(&self.pos.x))
            .then_with(|| other.pos.y.cmp(&self.pos.y))
            .then_with(|| other.in_side.cmp(&self.in_side))
    }
}

impl PartialOrd for RailAstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Variante con caché por tick de simulación (los wormholes son constantes
/// por mapa, así que no forman parte de la clave de caché).
#[must_use]
pub fn find_path_cached(
    map: &Map,
    cache: &mut PathCache,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    if let Some(path) = cache.get(from, to, network) {
        return Some(path.clone());
    }
    let path = find_path_with_wormholes(map, from, to, network, wormholes)?;
    cache.insert(from, to, network, path.clone());
    Some(path)
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct AstarNode {
    est_total: u32,
    pos: TileCoord,
}

impl Ord for AstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est_total
            .cmp(&self.est_total)
            .then_with(|| other.pos.x.cmp(&self.pos.x))
            .then_with(|| other.pos.y.cmp(&self.pos.y))
    }
}

impl PartialOrd for AstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[must_use]
fn manhattan(a: TileCoord, b: TileCoord) -> u32 {
    a.x.abs_diff(b.x) + a.y.abs_diff(b.y)
}

#[must_use]
const fn step_cost(_from: TileCoord, _to: TileCoord) -> u32 {
    1
}

fn reconstruct(
    from: TileCoord,
    to: TileCoord,
    parent: &HashMap<TileCoord, TileCoord>,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = to;
    while cur != from {
        path.push(cur);
        cur = parent[&cur];
    }
    path.reverse();
    path
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tnbp_decode::JgrTunnelRecord;

    #[test]
    fn jgr_wormhole_connects_disconnected_rail_ends() {
        // OpenTTD `TileIndex` asume ancho potencia de 2 (p. ej. 8).
        let mut map = Map::new_flat(8, 1, 0);
        for x in [0_i32, 4] {
            map.set_kind(TileCoord::new(x, 0), TileKind::RailTunnel)
                .unwrap();
        }
        let wh = TunnelWormholes::from_jgr_records(
            &map,
            &[JgrTunnelRecord {
                tile_n: 0,
                tile_s: 4,
                height: 1,
                is_chunnel: false,
                style_n: None,
                style_s: None,
            }],
        );
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(4, 0);
        assert!(wh.other_end(from).is_some());
        assert!(find_path(&map, from, to, PathNetwork::Rail).is_none());
        let path = find_path_with_wormholes(&map, from, to, PathNetwork::Rail, Some(&wh))
            .expect("wormhole");
        assert_eq!(path.last(), Some(&to));
    }

    fn write_road(m: &mut Map, c: TileCoord, bits: u8) {
        m.set_kind(c, TileKind::Road).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = bits & 0x0F;
        m.set_tile(c, t).unwrap();
    }

    fn write_rail(m: &mut Map, c: TileCoord, trackbits: u8) {
        m.set_kind(c, TileKind::Rail).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = trackbits & 0x3F;
        m.set_tile(c, t).unwrap();
    }

    #[test]
    fn astar_finds_path_on_straight_road() {
        let mut m = Map::new_flat(8, 8, 0);
        for x in 0..=4_i32 {
            write_road(&mut m, TileCoord::new(x, 0), 0x0A);
        }
        let path = find_path(
            &m,
            TileCoord::new(0, 0),
            TileCoord::new(4, 0),
            PathNetwork::Road,
        );
        assert!(path.is_some());
        assert_eq!(*path.unwrap().last().unwrap(), TileCoord::new(4, 0));
    }

    #[test]
    fn astar_respects_road_bit_gap() {
        let mut m = Map::new_flat(8, 8, 0);
        write_road(&mut m, TileCoord::new(0, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 1), 0x03);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 0),
                TileCoord::new(1, 1),
                PathNetwork::Road,
            )
            .is_none()
        );
    }

    #[test]
    fn astar_finds_detour_when_direct_gap_blocked() {
        let mut m = Map::new_flat(8, 8, 0);
        write_road(&mut m, TileCoord::new(0, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 0), 0x0A);
        write_road(&mut m, TileCoord::new(2, 0), 0x0F);
        write_road(&mut m, TileCoord::new(2, 1), 0x0F);
        write_road(&mut m, TileCoord::new(1, 1), 0x0A);
        write_road(&mut m, TileCoord::new(0, 1), 0x0A);
        let path = find_path(
            &m,
            TileCoord::new(0, 0),
            TileCoord::new(0, 1),
            PathNetwork::Road,
        )
        .expect("debe rodear por (2,0)");
        assert_eq!(path.last().copied(), Some(TileCoord::new(0, 1)));
        assert!(path.len() >= 4);
    }

    #[test]
    fn astar_rail_requires_matching_axis() {
        let mut m = Map::new_flat(6, 6, 0);
        write_rail(&mut m, TileCoord::new(0, 0), RAIL_TB_X);
        write_rail(&mut m, TileCoord::new(1, 0), RAIL_TB_Y);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 0),
                TileCoord::new(1, 0),
                PathNetwork::Rail,
            )
            .is_none()
        );
        write_rail(&mut m, TileCoord::new(1, 0), RAIL_TB_X);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 0),
                TileCoord::new(1, 0),
                PathNetwork::Rail,
            )
            .is_some()
        );
    }

    #[test]
    fn astar_rail_no_turn_at_plain_crossing() {
        let mut m = Map::new_flat(8, 8, 0);
        // Línea X en y=2 y línea Y en x=2; (2,2) es cruce X|Y sin curvas.
        for x in 0..=4_i32 {
            write_rail(&mut m, TileCoord::new(x, 2), RAIL_TB_X);
        }
        for y in 0..=4_i32 {
            if y != 2 {
                write_rail(&mut m, TileCoord::new(2, y), RAIL_TB_Y);
            }
        }
        write_rail(&mut m, TileCoord::new(2, 2), RAIL_TB_X | RAIL_TB_Y);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 2),
                TileCoord::new(4, 2),
                PathNetwork::Rail
            )
            .is_some(),
            "recto a través del cruce"
        );
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 2),
                TileCoord::new(2, 0),
                PathNetwork::Rail
            )
            .is_none(),
            "el tren no debe doblar en un cruce sin curva"
        );
        // Con la pieza UPPER (NE↔NW) el giro sí es válido.
        write_rail(&mut m, TileCoord::new(2, 2), RAIL_TB_X | RAIL_TB_Y | 0x04);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 2),
                TileCoord::new(2, 0),
                PathNetwork::Rail
            )
            .is_some(),
            "con curva el giro es válido"
        );
    }

    #[test]
    fn astar_rail_station_reaches_platform_along_axis() {
        let mut m = Map::new_flat(12, 12, 0);
        let station = TileCoord::new(4, 5);
        let track = TileCoord::new(5, 5);
        m.set_kind(station, TileKind::Station).unwrap();
        let mut st = m.get(station).unwrap();
        st.m6 &= !0x78;
        st.m5 = 2;
        m.set_tile(station, st).unwrap();
        write_rail(&mut m, track, RAIL_TB_X);
        for x in 3..=6_i32 {
            write_rail(&mut m, TileCoord::new(x, 5), RAIL_TB_X);
        }
        assert!(
            find_path(&m, track, TileCoord::new(6, 5), PathNetwork::Rail).is_some(),
            "vía horizontal → vía (sin entrar en plataforma)"
        );
        assert!(
            find_path(&m, track, station, PathNetwork::Rail).is_some(),
            "el tren debe poder rutear hacia la plataforma conectada por el eje"
        );
    }

    #[test]
    fn path_cache_reuses_result_within_tick() {
        let mut m = Map::new_flat(8, 8, 0);
        write_road(&mut m, TileCoord::new(0, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 0), 0x0A);
        let mut cache = PathCache::default();
        cache.begin_tick(1);
        let a = find_path_cached(
            &m,
            &mut cache,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            PathNetwork::Road,
            None,
        );
        let b = find_path_cached(
            &m,
            &mut cache,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            PathNetwork::Road,
            None,
        );
        assert_eq!(a, b);
        cache.begin_tick(2);
        assert!(
            cache
                .get(
                    TileCoord::new(0, 0),
                    TileCoord::new(1, 0),
                    PathNetwork::Road
                )
                .is_none()
        );
    }
}
