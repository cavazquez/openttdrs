use crate::map::{Map, TileCoord};
use crate::vehicle::{VehicleKind, VehicleOrder};

use super::geometry::{
    is_connected_bay_road_stop, is_drive_through_road_stop, rail_station_approach_tile,
    rail_station_stop_tile_for_approach, road_stop_approach_tile,
};
use super::{Station, StopKind};

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
        .find(|s| s.stop_kind == StopKind::Airport && s.covers_tile(station_pos))
        .map_or(station_pos, |s| {
            crate::airport::airport_loading_tile(s, map)
        })
}
