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

/// Primer `StationGfx` de la parte acuática de un muelle.
///
/// `MakeDock` escribe la pieza sobre tierra con `gfx = DiagDirection` y la
/// pieza sobre agua con `gfx = GFX_DOCK_BASE_WATER_PART + Axis`. Mantener el
/// umbral en core permite que navegación, demolición y renderer distingan la
/// mitad transitable de la rampa aun cuando el mapa provenga de un SAV.
pub const DOCK_WATER_PART_GFX: u8 = 4;

/// Devuelve la tesela de agua de un muelle a partir de su pieza de tierra.
///
/// La dirección es la que `MakeDock` guarda en `MAP5` de la pieza de tierra:
/// la parte acuática está exactamente una tesela en `TileOffsByDiagDir(dir)`.
#[must_use]
pub const fn dock_water_tile(land: crate::map::TileCoord, dir: u8) -> crate::map::TileCoord {
    let (dx, dy) = crate::map::diag_dir_offset(dir);
    crate::map::TileCoord::new(land.x + dx, land.y + dy)
}

/// Busca la pieza de tierra que acompaña a una pieza de muelle.
///
/// La parte acuática sólo conserva el eje (`gfx = 4`/`5`), por lo que para
/// recuperar la dirección completa se comprueban las cuatro piezas vecinas y
/// se exige que su `MAP5` apunte de vuelta a la tesela consultada.
#[must_use]
pub fn dock_land_tile(map: &Map, tile: crate::map::TileCoord) -> Option<crate::map::TileCoord> {
    let raw = map.get(tile)?;
    if raw.kind != TileKind::Station || stop_kind_from_m6(raw.m6) != StopKind::Dock {
        return None;
    }
    if raw.m5 < DOCK_WATER_PART_GFX {
        return Some(tile);
    }
    (0..4).find_map(|dir| {
        let (dx, dy) = crate::map::diag_dir_offset(dir);
        let land = crate::map::TileCoord::new(tile.x - dx, tile.y - dy);
        let candidate = map.get(land)?;
        (candidate.kind == TileKind::Station
            && stop_kind_from_m6(candidate.m6) == StopKind::Dock
            && candidate.m5 & 0x03 == dir
            && dock_water_tile(land, dir) == tile)
            .then_some(land)
    })
}

/// Devuelve la huella nativa `[tierra, agua]` de un muelle completo.
#[must_use]
pub fn dock_footprint_for_tile(
    map: &Map,
    tile: crate::map::TileCoord,
) -> Option<[crate::map::TileCoord; 2]> {
    let land = dock_land_tile(map, tile)?;
    let land_raw = map.get(land)?;
    let dir = land_raw.m5 & 0x03;
    let water = dock_water_tile(land, dir);
    let water_raw = map.get(water)?;
    (water_raw.kind == TileKind::Station
        && stop_kind_from_m6(water_raw.m6) == StopKind::Dock
        && water_raw.m5 >= DOCK_WATER_PART_GFX
        && u16::from(land_raw.m2) | (u16::from(land_raw.m2_hi) << 8)
            == u16::from(water_raw.m2) | (u16::from(water_raw.m2_hi) << 8))
        .then_some([land, water])
}

/// `station_map.h`: bit 2 de `m6` indica una reserva PBS en una tesela rail.
///
/// El byte `m6` también contiene el tipo de estación en los bits 3–6; mantener
/// la máscara aquí evita que los consumidores del scope `NewGRF` confundan ambos
/// campos y permite sincronizar reservas sin tocar la geometría de la parada.
pub const STATION_TILE_RESERVATION: u8 = 1 << 2;

#[must_use]
pub fn station_type_from_m6(m6: u8) -> u8 {
    (m6 >> 3) & 0x0F
}

/// ¿La tesela es una estación o waypoint ferroviario con estado PBS en `m6`?
#[must_use]
pub const fn is_rail_station_type(station_type: u8) -> bool {
    matches!(station_type, 0 | STATION_TYPE_RAIL_WAYPOINT)
}

/// Lee `HasStationReservation` para una tesela ferroviaria.
#[must_use]
pub const fn station_tile_has_reservation(m6: u8) -> bool {
    m6 & STATION_TILE_RESERVATION != 0
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
        STATION_TYPE_OILRIG => StopKind::OilRig,
        STATION_TYPE_DOCK => StopKind::Dock,
        1 => StopKind::Airport,
        STATION_TYPE_BUOY => StopKind::Buoy,
        STATION_TYPE_RAIL_WAYPOINT => StopKind::RailWaypoint,
        STATION_TYPE_ROAD_WAYPOINT => StopKind::RoadWaypoint,
        _ => StopKind::RailStation,
    }
}
