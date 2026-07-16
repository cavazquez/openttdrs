use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::diag_dir_offset;
use crate::vehicle::VehicleKind;

use super::tile_encoding::station_type_from_m6;
use super::{Station, StopKind};

#[must_use]
fn is_rail_track_kind(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
    )
}

/// Teselas `Station` contiguas al ancla (huella de una estación multi-tesela).
///
/// Si dos estaciones rail son adyacentes, el flood-fill puede unificar ambas
/// huellas; usar [`rail_station_owned_tiles`] / [`station_at_tile`] para
/// asignar cada tesela al ancla más cercana.
#[must_use]
pub fn station_footprint_tiles(map: &Map, anchor: TileCoord) -> Vec<TileCoord> {
    const MAX_FOOTPRINT: usize = 64;
    let mut tiles = vec![anchor];
    let mut seen = std::collections::HashSet::from([anchor]);
    let mut i = 0;
    while i < tiles.len() && tiles.len() < MAX_FOOTPRINT {
        let c = tiles[i];
        i += 1;
        for dir in 0..4u8 {
            let (dx, dy) = diag_dir_offset(dir);
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if map.get_kind(n) == Some(TileKind::Station) && seen.insert(n) {
                tiles.push(n);
            }
        }
    }
    tiles
}

#[must_use]
fn manhattan(a: TileCoord, b: TileCoord) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Estación lógica en `tile` (ancla / joined / airport / huella rail por ancla más cercana).
#[must_use]
pub fn station_at_tile<'a>(
    map: &Map,
    stations: &'a [Station],
    tile: TileCoord,
) -> Option<&'a Station> {
    if let Some(s) = stations.iter().find(|s| s.covers_tile(tile)) {
        return Some(s);
    }
    if map.get_kind(tile) != Some(TileKind::Station) {
        return None;
    }
    stations
        .iter()
        .filter(|s| {
            matches!(s.stop_kind, StopKind::RailStation | StopKind::RailWaypoint)
                && station_footprint_tiles(map, s.pos).contains(&tile)
        })
        .min_by_key(|s| manhattan(s.pos, tile))
}

/// Teselas de plataforma/huella rail asignadas a esta estación (Voronoi por ancla).
#[must_use]
pub fn rail_station_owned_tiles(
    map: &Map,
    stations: &[Station],
    station: &Station,
) -> Vec<TileCoord> {
    if !matches!(station.stop_kind, StopKind::RailStation) {
        return Vec::new();
    }
    station_footprint_tiles(map, station.pos)
        .into_iter()
        .filter(|&tile| station_at_tile(map, stations, tile).is_some_and(|s| s.pos == station.pos))
        .collect()
}

/// `true` si alguna tesela de `a` comparte borde (Manhattan 1) con alguna de `b`.
#[must_use]
pub fn station_tile_sets_adjacent(a: &[TileCoord], b: &[TileCoord]) -> bool {
    a.iter()
        .any(|ta| b.iter().any(|tb| manhattan(*ta, *tb) == 1))
}

/// Eje de una estación rail (`true` = eje Y) a partir de `m5` de sus plataformas.
#[must_use]
pub fn rail_station_axis_y(map: &Map, stations: &[Station], station: &Station) -> Option<bool> {
    let owned = rail_station_owned_tiles(map, stations, station);
    let sample = owned
        .into_iter()
        .find(|&c| is_rail_platform_tile(map, c))
        .or_else(|| {
            station_footprint_tiles(map, station.pos)
                .into_iter()
                .find(|&c| is_rail_platform_tile(map, c))
        })?;
    Some(map.get(sample).is_some_and(|t| t.m5 & 1 != 0))
}

#[must_use]
fn is_rail_platform_tile(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .is_some_and(|t| t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0)
}

/// Plataformas rail del footprint de una estación, ordenadas a lo largo del eje.
#[must_use]
pub fn rail_station_platform_tiles(map: &Map, station_anchor: TileCoord) -> Vec<TileCoord> {
    let mut tiles: Vec<TileCoord> = station_footprint_tiles(map, station_anchor)
        .into_iter()
        .filter(|c| is_rail_platform_tile(map, *c))
        .collect();
    if let Some(&first) = tiles.first() {
        let axis_y = map.get(first).is_some_and(|t| t.m5 & 1 != 0);
        tiles.sort_by(|a, b| if axis_y { a.y.cmp(&b.y) } else { a.x.cmp(&b.x) });
    }
    tiles
}

/// Tesela de parada en plataforma (paridad simplificada con `GetTrainStopLocation`:
/// tren puntual → `Middle`; una sola tesela → esa tesela).
#[must_use]
pub fn rail_station_stop_tile(map: &Map, station_anchor: TileCoord) -> Option<TileCoord> {
    let platforms = rail_station_platform_tiles(map, station_anchor);
    match platforms.len() {
        0 => None,
        1 => Some(platforms[0]),
        n => Some(platforms[n / 2]),
    }
}

/// `true` si el tren está sobre una plataforma rail de alguna estación del mapa.
#[must_use]
pub fn train_on_rail_platform(map: &Map, pos: TileCoord) -> bool {
    is_rail_platform_tile(map, pos)
}

/// Tesela de vía de acceso junto a una estación (antes de subir a la plataforma).
#[must_use]
pub fn rail_station_approach_tile(map: &Map, station_pos: TileCoord) -> Option<TileCoord> {
    let is_rail_platform = |c: TileCoord| {
        map.get(c)
            .is_some_and(|t| t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0)
    };
    let mut adjacent: Option<(i32, TileCoord)> = None;
    let mut platform: Option<(i32, TileCoord)> = None;
    for c in station_footprint_tiles(map, station_pos) {
        if is_rail_platform(c) {
            let d = (c.x - station_pos.x).abs() + (c.y - station_pos.y).abs();
            if platform.is_none_or(|(bd, _)| d < bd) {
                platform = Some((d, c));
            }
        }
        for dir in 0..4u8 {
            let (dx, dy) = diag_dir_offset(dir);
            let track = TileCoord::new(c.x + dx, c.y + dy);
            if !map.get_kind(track).is_some_and(is_rail_track_kind) {
                continue;
            }
            let d = (track.x - station_pos.x).abs() + (track.y - station_pos.y).abs();
            if adjacent.is_none_or(|(bd, _)| d < bd) {
                adjacent = Some((d, track));
            }
        }
    }
    adjacent.or(platform).map(|(_, track)| track)
}

fn is_road_approach_kind(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
    )
}

/// Parada bahía bus/camión con boca conectada (`m3` con bits de acceso).
/// Solo estas paradas son destino de movimiento «dentro de la tesela»
/// (paridad con `OpenTTD`, donde el vehículo entra a la bahía).
#[must_use]
pub fn is_connected_bay_road_stop(map: &Map, station_pos: TileCoord) -> bool {
    map.get(station_pos).is_some_and(|t| {
        t.kind == TileKind::Station
            && matches!(station_type_from_m6(t.m6), 2 | 3)
            && (t.m3 & 0x0F) != 0
    })
}

/// Dirección de marcha con la que un vehículo ENTRA a la bahía (desde la
/// carretera frente a la boca hacia el interior de la tesela de estación).
#[must_use]
pub fn bay_entry_direction(map: &Map, station_pos: TileCoord) -> Option<crate::VehicleDirection> {
    let tile = map.get(station_pos)?;
    if tile.kind != TileKind::Station || !matches!(station_type_from_m6(tile.m6), 2 | 3) {
        return None;
    }
    let mouth = tile.m5 & 0x03;
    let (dx, dy) = diag_dir_offset(mouth);
    let approach = TileCoord::new(station_pos.x + dx, station_pos.y + dy);
    Some(crate::vehicle::direction_from_tile_step(
        approach,
        station_pos,
    ))
}

/// Tesela de carretera donde bus/camión debe detenerse junto a la parada (no sobre la hierba).
#[must_use]
pub fn road_stop_approach_tile(map: &Map, station_pos: TileCoord) -> Option<TileCoord> {
    let tile = map.get(station_pos)?;
    if tile.kind != TileKind::Station {
        return None;
    }
    match station_type_from_m6(tile.m6) {
        2 | 3 => {}
        _ => return None,
    }
    let dir = tile.m5 & 0x03;
    let (dx, dy) = diag_dir_offset(dir);
    let road = TileCoord::new(station_pos.x + dx, station_pos.y + dy);
    if map.get_kind(road).is_some_and(is_road_approach_kind) {
        return Some(road);
    }
    for d in 0..4u8 {
        let (odx, ody) = diag_dir_offset(d);
        let n = TileCoord::new(station_pos.x + odx, station_pos.y + ody);
        if map.get_kind(n).is_some_and(is_road_approach_kind) {
            return Some(n);
        }
    }
    None
}

/// El vehículo está en su posición de servicio de la parada.
///
/// Bus/camión en bahía conectada: SOLO dentro de la tesela de la estación
/// (paridad `OpenTTD`: la carga empieza al alcanzar el stop frame dentro de la
/// Bahía sin boca: carretera de acceso (fallback). Tren: sobre la plataforma rail.
#[must_use]
pub fn vehicle_physically_at_station(
    map: &Map,
    vehicle: &crate::Vehicle,
    station: &Station,
) -> bool {
    if !station.can_service_vehicle(vehicle.kind) {
        return false;
    }
    let vpos = vehicle.pos;
    if station.covers_tile(vpos) {
        return match vehicle.kind {
            VehicleKind::Aircraft => map
                .get(vpos)
                .is_some_and(|t| crate::airport::AirportPiece::from_m5(t.m5).is_loading()),
            _ => true,
        };
    }
    match vehicle.kind {
        VehicleKind::Truck | VehicleKind::Bus | VehicleKind::Tram => {
            // Acceso a bahía: ancla o cualquiera de las teselas unidas.
            station
                .joined_tiles
                .iter()
                .copied()
                .chain(std::iter::once(station.pos))
                .any(|stop| {
                    !is_connected_bay_road_stop(map, stop)
                        && road_stop_approach_tile(map, stop)
                            .is_some_and(|approach| vpos == approach)
                })
        }
        VehicleKind::Train => {
            station_footprint_tiles(map, station.pos).contains(&vpos)
                && train_on_rail_platform(map, vpos)
        }
        VehicleKind::Ship => {
            matches!(station.stop_kind, StopKind::Dock | StopKind::Buoy)
                && (if station.stop_kind == StopKind::Buoy {
                    vpos == station.pos
                } else {
                    vpos.x.abs_diff(station.pos.x) + vpos.y.abs_diff(station.pos.y) == 1
                        && crate::ship_movement::is_water_network_tile_at(map, vpos)
                })
        }
        VehicleKind::Aircraft => false,
    }
}

/// El vehículo llegó a la parada de la orden actual (dentro de la bahía; la
/// carretera de acceso solo cuenta como fallback si la bahía no tiene boca).
#[must_use]
pub fn vehicle_at_road_stop(map: &Map, vehicle: &crate::Vehicle) -> bool {
    if vehicle.manhattan_to_dest() == 0 {
        return true;
    }
    let Some(crate::vehicle::VehicleOrder::Station { station, .. }) = vehicle.orders.get(vehicle.current_order)
    else {
        return false;
    };
    if vehicle.pos == *station {
        return true;
    }
    !is_connected_bay_road_stop(map, *station)
        && road_stop_approach_tile(map, *station).is_some_and(|approach| vehicle.pos == approach)
}
