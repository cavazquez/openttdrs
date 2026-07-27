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

/// Teselas del andén individual que contiene `stop_tile`.
///
/// Una estación ferroviaria puede tener varias vías paralelas dentro del mismo
/// footprint. Las reservas y la asignación de llegada deben operar sobre una
/// sola de ellas para permitir un tren por andén, no bloquear la estación
/// completa cuando entra el primer consist.
#[must_use]
pub fn rail_station_platform_track_tiles(
    map: &Map,
    station_anchor: TileCoord,
    stop_tile: TileCoord,
) -> Vec<TileCoord> {
    let platforms = rail_station_platform_tiles(map, station_anchor);
    let Some(&first) = platforms.first() else {
        return Vec::new();
    };
    let axis_y = map.get(first).is_some_and(|t| t.m5 & 1 != 0);
    let same_track = |c: TileCoord| {
        if axis_y {
            c.x == stop_tile.x
        } else {
            c.y == stop_tile.y
        }
    };
    let mut track: Vec<_> = platforms.into_iter().filter(|&c| same_track(c)).collect();
    track.sort_by(|a, b| if axis_y { a.y.cmp(&b.y) } else { a.x.cmp(&b.x) });
    track
}

/// Tesela de parada en plataforma (paridad simplificada con `GetTrainStopLocation`:
/// tren puntual → `Middle`; una sola tesela → esa tesela).
///
/// En estaciones multi-andén sin contexto de aproximación, toma el medio de
/// **todas** las plataformas. Preferí [`rail_station_stop_tile_for_approach`]
/// cuando se conoce la posición del tren.
#[must_use]
pub fn rail_station_stop_tile(map: &Map, station_anchor: TileCoord) -> Option<TileCoord> {
    rail_station_stop_tile_with_osl(
        map,
        station_anchor,
        crate::vehicle::OrderStopLocation::Middle,
        0,
    )
}

/// Como [`rail_station_stop_tile`] con `OrderStopLocation` y longitud del consist.
#[must_use]
pub fn rail_station_stop_tile_with_osl(
    map: &Map,
    station_anchor: TileCoord,
    osl: crate::vehicle::OrderStopLocation,
    train_length: u16,
) -> Option<TileCoord> {
    let platforms = rail_station_platform_tiles(map, station_anchor);
    match platforms.len() {
        0 => None,
        1 => Some(platforms[0]),
        _ => {
            let mut tiles = platforms;
            let axis_y = map.get(tiles[0]).is_some_and(|t| t.m5 & 1 != 0);
            tiles.sort_by(|a, b| if axis_y { a.y.cmp(&b.y) } else { a.x.cmp(&b.x) });
            Some(pick_stop_tile(&tiles, osl, train_length))
        }
    }
}

/// Parada según OSL en el andén alineado con `from` (misma vía / columna).
///
/// `OpenTTD` `GetTrainStopLocation` usa la tesela actual del tren para medir el
/// andén (`GetPlatformLength(tile, …)`); sin eso, un destino en el andén
/// paralelo deja el pathfinder sin ruta (señales path-oneway / sin cruce).
#[must_use]
pub fn rail_station_stop_tile_for_approach(
    map: &Map,
    station_anchor: TileCoord,
    from: TileCoord,
) -> Option<TileCoord> {
    rail_station_stop_tile_for_approach_osl(
        map,
        station_anchor,
        from,
        crate::vehicle::OrderStopLocation::Middle,
        0,
    )
}

/// Como [`rail_station_stop_tile_for_approach`] con `OrderStopLocation`.
#[must_use]
pub fn rail_station_stop_tile_for_approach_osl(
    map: &Map,
    station_anchor: TileCoord,
    from: TileCoord,
    osl: crate::vehicle::OrderStopLocation,
    train_length: u16,
) -> Option<TileCoord> {
    rail_station_stop_candidates_osl(map, station_anchor, from, osl, train_length)
        .into_iter()
        .next()
}

/// Paradas candidatas: primero el andén alineado con `from`, luego el resto
/// (middle de cada vía). Sirve para reintentar YAPF si la vía preferida no
/// tiene ruta (red dual / PBS one-way).
#[must_use]
pub fn rail_station_stop_candidates(
    map: &Map,
    station_anchor: TileCoord,
    from: TileCoord,
) -> Vec<TileCoord> {
    rail_station_stop_candidates_osl(
        map,
        station_anchor,
        from,
        crate::vehicle::OrderStopLocation::Middle,
        0,
    )
}

/// Candidatos con `OrderStopLocation` (`GetTrainStopLocation` por tesela).
#[must_use]
pub fn rail_station_stop_candidates_osl(
    map: &Map,
    station_anchor: TileCoord,
    from: TileCoord,
    osl: crate::vehicle::OrderStopLocation,
    train_length: u16,
) -> Vec<TileCoord> {
    let platforms = rail_station_platform_tiles(map, station_anchor);
    if platforms.is_empty() {
        return Vec::new();
    }
    let axis_y = map.get(platforms[0]).is_some_and(|t| t.m5 & 1 != 0);
    let track_key = |c: TileCoord| if axis_y { c.x } else { c.y };
    let want = track_key(from);
    let mut by_track: std::collections::BTreeMap<i32, Vec<TileCoord>> =
        std::collections::BTreeMap::new();
    for c in platforms {
        by_track.entry(track_key(c)).or_default().push(c);
    }
    let pick = |tiles: &mut Vec<TileCoord>| -> TileCoord {
        tiles.sort_by(|a, b| if axis_y { a.y.cmp(&b.y) } else { a.x.cmp(&b.x) });
        // `GetTrainStopLocation` mide siempre desde el extremo de entrada. Esto
        // también importa para Middle: sin orientar, un consist que llega desde
        // el extremo decreciente deja la cola fuera del andén.
        let approaching_positive = if axis_y {
            from.y <= tiles[0].y
        } else {
            from.x <= tiles[0].x
        };
        let oriented = if approaching_positive {
            tiles.clone()
        } else {
            tiles.iter().rev().copied().collect()
        };
        pick_stop_tile(&oriented, osl, train_length)
    };
    let mut out = Vec::with_capacity(by_track.len());
    if let Some(tiles) = by_track.remove(&want) {
        let mut tiles = tiles;
        out.push(pick(&mut tiles));
    }
    for (_, mut tiles) in by_track {
        out.push(pick(&mut tiles));
    }
    if out.is_empty()
        && let Some(fallback) =
            rail_station_stop_tile_with_osl(map, station_anchor, osl, train_length)
    {
        out.push(fallback);
    }
    out
}

/// Índice de tesela de parada según OSL (andén ordenado entrada→salida).
///
/// Tren más largo que el andén → `FarEnd` (como `OpenTTD`).
#[must_use]
pub fn pick_stop_tile(
    platform_entry_to_exit: &[TileCoord],
    osl: crate::vehicle::OrderStopLocation,
    train_length: u16,
) -> TileCoord {
    use crate::vehicle::OrderStopLocation;
    let n = platform_entry_to_exit.len();
    debug_assert!(n > 0);
    // Longitud de andén en unidades de vehículo (tile ≈ 16 = 2×VEHICLE_LENGTH).
    let station_len = u16::try_from(n.saturating_mul(16)).unwrap_or(u16::MAX);
    let effective = if train_length > 0 && train_length >= station_len {
        OrderStopLocation::FarEnd
    } else {
        osl
    };
    let idx = if train_length == 0 {
        match effective {
            OrderStopLocation::NearEnd => 0,
            OrderStopLocation::Middle => n / 2,
            OrderStopLocation::FarEnd => n.saturating_sub(1),
        }
    } else {
        // `GetTrainStopLocation`: posición del frente, menos media locomotora.
        // El controlador actual detiene por tesela; elegir la que contiene esa
        // coordenada evita parar una tesela antes y dejar la cola afuera.
        let front_center = match effective {
            OrderStopLocation::NearEnd => train_length,
            OrderStopLocation::Middle => station_len - station_len.saturating_sub(train_length) / 2,
            OrderStopLocation::FarEnd => station_len,
        };
        let front_stop = front_center
            .saturating_sub(u16::from(crate::train_consist::VEHICLE_LENGTH.div_ceil(2)));
        usize::from(front_stop.saturating_sub(1) / 16).min(n.saturating_sub(1))
    };
    platform_entry_to_exit[idx]
}

/// Fracción de andén más allá del punto de parada (`(station_length - stop_at) / TILE_SIZE`).
#[must_use]
pub fn platform_past_stop_tiles(
    platform_len: i32,
    osl: crate::vehicle::OrderStopLocation,
    train_length_tiles: i32,
) -> i32 {
    use crate::vehicle::OrderStopLocation;
    if platform_len <= 0 {
        return 0;
    }
    let osl = if train_length_tiles >= platform_len {
        OrderStopLocation::FarEnd
    } else {
        osl
    };
    match osl {
        // Near: stop ≈ longitud del tren → casi todo el andén queda por delante.
        OrderStopLocation::NearEnd => (platform_len - train_length_tiles.max(1)).max(0),
        OrderStopLocation::Middle => platform_len / 2,
        OrderStopLocation::FarEnd => 0,
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
            VehicleKind::Aircraft => map.get(vpos).is_some_and(|tile| {
                let piece = if tile.kind == crate::map::TileKind::Station {
                    crate::airport::AirportPiece::from_station_gfx(tile.m5)
                } else {
                    crate::airport::AirportPiece::from_m5(tile.m5)
                };
                piece.is_loading()
            }),
            VehicleKind::Truck | VehicleKind::Bus | VehicleKind::Tram
                if is_connected_bay_road_stop(map, vpos) =>
            {
                !crate::road_movement::rvsb::is_bay_road_state(vehicle.road_state)
                    || crate::road_movement::bay::road_vehicle_stopped_in_bay(vehicle)
            }
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
    if is_connected_bay_road_stop(map, vehicle.pos) {
        return !crate::road_movement::rvsb::is_bay_road_state(vehicle.road_state)
            || crate::road_movement::bay::road_vehicle_stopped_in_bay(vehicle);
    }
    if vehicle.manhattan_to_dest() == 0 {
        return true;
    }
    let Some(crate::vehicle::VehicleOrder::Station { station, .. }) =
        vehicle.orders.get(vehicle.current_order)
    else {
        return false;
    };
    if vehicle.pos == *station {
        return !is_connected_bay_road_stop(map, *station)
            || crate::road_movement::bay::road_vehicle_stopped_in_bay(vehicle);
    }
    !is_connected_bay_road_stop(map, *station)
        && road_stop_approach_tile(map, *station).is_some_and(|approach| vehicle.pos == approach)
}
