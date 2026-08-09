use crate::map::{Map, TileCoord, TileKind};

use super::StopKind;

/// `station_map.h`: bit 1 de m3 permite cables de catenaria.
pub const STATION_TILE_WIRES: u8 = 1 << 1;
/// `station_map.h`: bit 2 de m3 permite postes de catenaria.
pub const STATION_TILE_PYLONS: u8 = 1 << 2;

/// ¿La tesela ferroviaria de estación permite cables?
#[must_use]
pub const fn station_tile_can_have_wires(m3: u8) -> bool {
    m3 & STATION_TILE_WIRES != 0
}

/// ¿La tesela ferroviaria de estación permite postes?
#[must_use]
pub const fn station_tile_can_have_pylons(m3: u8) -> bool {
    m3 & STATION_TILE_PYLONS != 0
}

/// Flags por defecto de una estación clásica (`GetStationTileFlags`).
///
/// Todas las piezas permiten cables; solo gfx 0..3 permiten postes bajo la
/// plataforma/edificio. Los techos gfx >= 4 ocultan el poste.
#[must_use]
pub const fn default_station_catenary_flags(gfx: u8) -> u8 {
    STATION_TILE_WIRES | if gfx < 4 { STATION_TILE_PYLONS } else { 0 }
}

/// `StationType::Oilrig` en bits 3–6 de `m6` (`station_type.h`).
///
/// No es un muelle: se conserva para no confundir las plataformas petroleras
/// importadas con estaciones navales.
pub const STATION_TYPE_OILRIG: u8 = 4;
/// `StationType::Dock` en bits 3–6 de `m6` (`station_type.h`).
pub const STATION_TYPE_DOCK: u8 = 5;
/// `StationType::Buoy` en bits 3–6 de `m6`.
pub const STATION_TYPE_BUOY: u8 = 6;
/// `StationType::RailWaypoint` en bits 3–6 de `m6` (`station_type.h`).
pub const STATION_TYPE_RAIL_WAYPOINT: u8 = 7;
/// `StationType::RoadWaypoint` en bits 3–6 de `m6` (`station_type.h`).
pub const STATION_TYPE_ROAD_WAYPOINT: u8 = 8;

#[must_use]
pub fn station_type_from_m6(m6: u8) -> u8 {
    (m6 >> 3) & 0x0F
}

#[must_use]
pub fn is_rail_waypoint_tile(tile: &crate::map::Tile) -> bool {
    tile.kind == TileKind::Station && station_type_from_m6(tile.m6) == STATION_TYPE_RAIL_WAYPOINT
}

#[must_use]
pub fn is_rail_waypoint_at(map: &Map, c: TileCoord) -> bool {
    map.get(c).is_some_and(|t| is_rail_waypoint_tile(&t))
}

#[must_use]
pub fn stop_kind_from_m6(m6: u8) -> StopKind {
    match station_type_from_m6(m6) {
        2 => StopKind::TruckStop,
        3 => StopKind::BusStop,
        STATION_TYPE_DOCK => StopKind::Dock,
        1 => StopKind::Airport,
        STATION_TYPE_BUOY => StopKind::Buoy,
        STATION_TYPE_RAIL_WAYPOINT => StopKind::RailWaypoint,
        STATION_TYPE_ROAD_WAYPOINT => StopKind::RoadWaypoint,
        _ => StopKind::RailStation,
    }
}
