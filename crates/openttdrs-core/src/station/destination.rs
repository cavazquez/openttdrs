use crate::map::{Map, TileCoord, TileKind, diag_dir_offset};
use crate::vehicle::{VehicleKind, VehicleOrder};

use super::Station;
use super::geometry::{
    dock_station_tiles, is_connected_bay_road_stop, is_drive_through_road_stop,
    rail_station_approach_tile, rail_station_stop_tile_for_approach, road_stop_approach_tile,
};

/// Busca la tesela de amarre que `YapfShip` usaría para un muelle u oil rig.
///
/// El modelo local conserva la estación como ancla de la orden, mientras que
/// `OpenTTD` termina el path en una tesela acuática vecina marcada con
/// `DockingTile`. El marcador raw permite migrar esa semántica sin exigir que
/// partidas JSON antiguas ya tengan una lista de docking persistida.
#[must_use]
fn ship_docking_tile_for_station(
    map: &Map,
    stations: &[Station],
    station: TileCoord,
    from: TileCoord,
) -> Option<TileCoord> {
    let origins = if let Some(logical_station) = stations.iter().find(|candidate| {
        candidate.stop_kind == super::StopKind::Dock && candidate.covers_tile(station)
    }) {
        let mut origins = Vec::new();
        for physical_tile in dock_station_tiles(map, logical_station) {
            if let Some(footprint) = crate::station::dock_footprint_for_tile(map, physical_tile) {
                origins.push(footprint[1]);
            } else if map.get(physical_tile).is_some_and(|tile| {
                tile.kind == TileKind::Station
                    && crate::station::stop_kind_from_m6(tile.m6) == super::StopKind::Dock
            }) {
                // Conserva el fallback de partidas legacy con sólo una pieza
                // de muelle persistida.
                origins.push(physical_tile);
            }
        }
        if origins.is_empty() {
            return None;
        }
        origins
    } else {
        let station_tile = map.get(station)?;
        if station_tile.kind != TileKind::Station {
            return None;
        }
        match crate::station::stop_kind_from_m6(station_tile.m6) {
            super::StopKind::Dock => crate::station::dock_footprint_for_tile(map, station)
                .map_or_else(|| vec![station], |footprint| vec![footprint[1]]),
            super::StopKind::OilRig => vec![station],
            _ => return None,
        }
    };

    origins
        .into_iter()
        .flat_map(|origin| {
            (0..4).filter_map(move |dir| {
                let (dx, dy) = diag_dir_offset(dir);
                let candidate = TileCoord::new(origin.x + dx, origin.y + dy);
                map.get(candidate).and_then(|tile| {
                    (matches!(tile.kind, TileKind::Water | TileKind::ShipDepot)
                        && tile.m1 & 0x80 != 0
                        && crate::ship_movement::is_water_network_tile_at(map, candidate))
                    .then_some(candidate)
                })
            })
        })
        .min_by_key(|candidate| {
            (
                candidate.x.abs_diff(from.x) + candidate.y.abs_diff(from.y),
                candidate.x,
                candidate.y,
            )
        })
}

/// Destino de movimiento según tipo de vehículo y orden.
///
/// Bus/camión: la tesela de la bahía misma — como `OpenTTD`, el vehículo ENTRA
/// a la parada y se detiene dentro (`_rv_station_*` / `_road_stop_stop_frame`).
/// Si la bahía no tiene boca conectada, cae a la carretera de acceso.
/// Tren: la tesela de parada en la plataforma (`GetTrainStopLocation` simplificado).
#[must_use]
pub fn resolve_order_destination(map: &Map, kind: VehicleKind, order: VehicleOrder) -> TileCoord {
    resolve_order_destination_from(map, kind, order, order.destination())
}

/// Como [`resolve_order_destination`], eligiendo el andén alineado con `from`
/// cuando la orden es una estación rail multi-vía.
#[must_use]
pub fn resolve_order_destination_from(
    map: &Map,
    kind: VehicleKind,
    order: VehicleOrder,
    from: TileCoord,
) -> TileCoord {
    resolve_order_destination_from_with_stations(map, &[], kind, order, from)
}

/// Como `resolve_order_destination_from`, usando el estado lógico de las
/// estaciones para resolver todos los muelles unidos de una misma parada.
///
/// La variante legacy sólo conoce la tesela ancla de la orden. Eso alcanza para
/// un muelle aislado, pero en una estación unida desde SAV puede haber varias
/// huellas físicas y el amarre correcto depende de la posición actual del
/// barco.
#[must_use]
pub fn resolve_order_destination_from_with_stations(
    map: &Map,
    stations: &[Station],
    kind: VehicleKind,
    order: VehicleOrder,
    from: TileCoord,
) -> TileCoord {
    match (kind, order) {
        (
            VehicleKind::Train,
            VehicleOrder::Station {
                station,
                stop_location,
                ..
            },
        ) => {
            // Sin longitud de consist aquí (pathfinding pre-spawn); Middle/OSL de la orden.
            super::geometry::rail_station_stop_tile_for_approach_osl(
                map,
                station,
                from,
                stop_location,
                0,
            )
            .or_else(|| rail_station_approach_tile(map, station))
            .or_else(|| rail_station_stop_tile_for_approach(map, station, from))
            .unwrap_or(station)
        }
        (VehicleKind::Train, VehicleOrder::Waypoint { waypoint, .. }) => waypoint,
        (VehicleKind::Ship, VehicleOrder::Station { station, .. }) => {
            ship_docking_tile_for_station(map, stations, station, from).unwrap_or(station)
        }
        (_, VehicleOrder::Depot { depot, .. }) => depot,
        (
            VehicleKind::Truck | VehicleKind::Bus | VehicleKind::Tram,
            VehicleOrder::Station { station, .. },
        ) => {
            if is_connected_bay_road_stop(map, station) || is_drive_through_road_stop(map, station)
            {
                station
            } else {
                road_stop_approach_tile(map, station).unwrap_or(station)
            }
        }
        (_, order) => order.destination(),
    }
}

/// Destino aéreo de una orden de estación (apron/loading del aeropuerto).
#[must_use]
pub fn resolve_aircraft_station_dest(
    stations: &[Station],
    map: &Map,
    station_pos: TileCoord,
) -> TileCoord {
    stations
        .iter()
        .find(|s| s.stop_kind.has_airport_facility() && s.covers_tile(station_pos))
        .map_or(station_pos, |s| {
            crate::airport::airport_loading_tile(s, map)
        })
}
