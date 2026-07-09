use crate::cargo::{CargoStock, CargoType};
use crate::industry::{Industry, IndustryKind};
use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::diag_dir_offset;
use crate::vehicle::{VehicleKind, VehicleOrder};

pub const STATION_COVERAGE_RADIUS: i32 = 4;
/// Máximo de días sin recogida antes de truncar (`station_cmd.cpp`).
pub const MAX_TIME_SINCE_PICKUP_DAYS: u8 = 255;

/// Días desde la última recogida por tipo de carga (0 = reciente).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoTimeSincePickup {
    pub passengers: u8,
    pub mail: u8,
    pub goods: u8,
    pub coal: u8,
    pub wood: u8,
    pub oil: u8,
}

impl CargoTimeSincePickup {
    #[must_use]
    pub const fn get(self, cargo: CargoType) -> u8 {
        match cargo {
            CargoType::Passengers => self.passengers,
            CargoType::Mail => self.mail,
            CargoType::Goods => self.goods,
            CargoType::Coal => self.coal,
            CargoType::Wood => self.wood,
            CargoType::Oil => self.oil,
        }
    }

    pub fn set(&mut self, cargo: CargoType, days: u8) {
        let slot = match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Mail => &mut self.mail,
            CargoType::Goods => &mut self.goods,
            CargoType::Coal => &mut self.coal,
            CargoType::Wood => &mut self.wood,
            CargoType::Oil => &mut self.oil,
        };
        *slot = days;
    }

    pub fn increment_waiting(&mut self, cargo: CargoType) {
        let slot = match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Mail => &mut self.mail,
            CargoType::Goods => &mut self.goods,
            CargoType::Coal => &mut self.coal,
            CargoType::Wood => &mut self.wood,
            CargoType::Oil => &mut self.oil,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: TileCoord,
    #[serde(default)]
    pub stop_kind: StopKind,
    /// Nombre de la estación (saves de `OpenTTD` con nombre custom).
    #[serde(default)]
    pub name: Option<String>,
    /// Cargo acumulado en el almacén de la estación.
    pub stock: u32,
    #[serde(default)]
    pub cargo_stock: CargoStock,
    /// Contador histórico total de unidades entregadas (análogo a `income` simplificado).
    pub income: u64,
    /// Días sin recogida por tipo de carga en espera.
    #[serde(default)]
    pub time_since_pickup: CargoTimeSincePickup,
    /// Rating global simplificado (0–255; mayor = mejor servicio).
    #[serde(default = "default_station_rating")]
    pub rating: u8,
}

const fn default_station_rating() -> u8 {
    255
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum StopKind {
    #[default]
    TruckStop,
    BusStop,
    RailStation,
    /// Muelle (`StationType::Dock`); carga de mercancía para barcos.
    Dock,
    /// Helipuerto / aeropuerto 1×1 (`StationType::Airport`).
    Airport,
    /// Punto de paso ferroviario (`StationType::RailWaypoint`); sin carga ni parada.
    RailWaypoint,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self::new_with_kind(pos, StopKind::TruckStop)
    }

    #[must_use]
    pub fn new_with_kind(pos: TileCoord, stop_kind: StopKind) -> Self {
        Self {
            pos,
            stop_kind,
            name: None,
            stock: 0,
            cargo_stock: CargoStock::default(),
            income: 0,
            time_since_pickup: CargoTimeSincePickup::default(),
            rating: default_station_rating(),
        }
    }

    #[must_use]
    pub fn can_service_vehicle(&self, vehicle_kind: VehicleKind) -> bool {
        matches!(
            (vehicle_kind, self.stop_kind),
            (
                VehicleKind::Train,
                StopKind::RailStation | StopKind::RailWaypoint
            ) | (VehicleKind::Bus, StopKind::BusStop)
                | (VehicleKind::Truck, StopKind::TruckStop)
                | (VehicleKind::Ship, StopKind::Dock)
                | (VehicleKind::Aircraft, StopKind::Airport)
        )
    }

    #[must_use]
    pub fn is_waypoint(&self) -> bool {
        self.stop_kind == StopKind::RailWaypoint
    }

    #[must_use]
    pub fn accepts_cargo(&self, cargo: CargoType) -> bool {
        if self.stop_kind == StopKind::RailWaypoint {
            return false;
        }
        match self.stop_kind {
            StopKind::BusStop => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::TruckStop | StopKind::RailStation => {
                !matches!(cargo, CargoType::Passengers | CargoType::Mail)
            }
            // Muelle: mercancía + pasajeros (ferry).
            StopKind::Dock => true,
            StopKind::Airport => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::RailWaypoint => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StationCoverage {
    /// Teselas `House` dentro del radio (origen de pasajeros/correo).
    pub house_tiles: u32,
    pub accepts_mail: u32,
    pub accepts_goods: u32,
    pub supplies_coal: u32,
    pub supplies_wood: u32,
    pub supplies_oil: u32,
    pub supplied_stock: u32,
}

impl StationCoverage {
    #[must_use]
    pub const fn accepts_anything(self) -> bool {
        self.accepts_mail > 0 || self.accepts_goods > 0
    }

    #[must_use]
    pub const fn supplies_anything(self) -> bool {
        self.supplies_coal > 0 || self.supplies_wood > 0 || self.supplies_oil > 0
    }
}

/// `StationType::RailWaypoint` en bits 3–6 de `m6` (`station_type.h`).
pub const STATION_TYPE_RAIL_WAYPOINT: u8 = 7;

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
fn is_rail_track_kind(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
    )
}

/// Teselas `Station` contiguas al ancla (huella de una estación multi-tesela).
#[must_use]
fn station_footprint_tiles(map: &Map, anchor: TileCoord) -> Vec<TileCoord> {
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
    if vpos == station.pos {
        return true;
    }
    match vehicle.kind {
        VehicleKind::Truck | VehicleKind::Bus => {
            !is_connected_bay_road_stop(map, station.pos)
                && road_stop_approach_tile(map, station.pos)
                    .is_some_and(|approach| vpos == approach)
        }
        VehicleKind::Train => {
            station_footprint_tiles(map, station.pos).contains(&vpos)
                && train_on_rail_platform(map, vpos)
        }
        VehicleKind::Ship => {
            vpos == station.pos || {
                // Barco en agua adyacente al muelle (acceso).
                station.stop_kind == StopKind::Dock
                    && vpos.x.abs_diff(station.pos.x) + vpos.y.abs_diff(station.pos.y) == 1
                    && crate::ship_movement::is_water_network_tile_at(map, vpos)
            }
        }
        VehicleKind::Aircraft => station.stop_kind == StopKind::Airport && vpos == station.pos,
    }
}

/// El vehículo llegó a la parada de la orden actual (dentro de la bahía; la
/// carretera de acceso solo cuenta como fallback si la bahía no tiene boca).
#[must_use]
pub fn vehicle_at_road_stop(map: &Map, vehicle: &crate::Vehicle) -> bool {
    if vehicle.manhattan_to_dest() == 0 {
        return true;
    }
    let Some(VehicleOrder::Station { station, .. }) = vehicle.orders.get(vehicle.current_order)
    else {
        return false;
    };
    if vehicle.pos == *station {
        return true;
    }
    !is_connected_bay_road_stop(map, *station)
        && road_stop_approach_tile(map, *station).is_some_and(|approach| vehicle.pos == approach)
}

/// Destino de movimiento según tipo de vehículo y orden.
///
/// Bus/camión: la tesela de la bahía misma — como `OpenTTD`, el vehículo ENTRA
/// a la parada y se detiene dentro (`_rv_station_*` / `_road_stop_stop_frame`).
/// Si la bahía no tiene boca conectada, cae a la carretera de acceso.
/// Tren: la tesela de parada en la plataforma (`GetTrainStopLocation` simplificado).
#[must_use]
pub fn resolve_order_destination(map: &Map, kind: VehicleKind, order: VehicleOrder) -> TileCoord {
    match (kind, order) {
        (VehicleKind::Train, VehicleOrder::Station { station, .. }) => {
            rail_station_stop_tile(map, station)
                .or_else(|| rail_station_approach_tile(map, station))
                .unwrap_or(station)
        }
        (VehicleKind::Train, VehicleOrder::Waypoint { waypoint, .. }) => waypoint,
        (_, VehicleOrder::Depot { depot, .. }) => depot,
        (VehicleKind::Truck | VehicleKind::Bus, VehicleOrder::Station { station, .. }) => {
            if is_connected_bay_road_stop(map, station) {
                station
            } else {
                road_stop_approach_tile(map, station).unwrap_or(station)
            }
        }
        (_, order) => order.destination(),
    }
}

#[must_use]
pub fn stop_kind_from_m6(m6: u8) -> StopKind {
    match station_type_from_m6(m6) {
        2 => StopKind::TruckStop,
        3 => StopKind::BusStop,
        4 => StopKind::Dock,
        1 => StopKind::Airport,
        STATION_TYPE_RAIL_WAYPOINT => StopKind::RailWaypoint,
        _ => StopKind::RailStation,
    }
}

/// Desajustes entre teselas `MP_STATION` y entradas en [`GameState::stations`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationMapCoherenceReport {
    pub tiles_without_station: Vec<TileCoord>,
    pub stations_without_tile: Vec<TileCoord>,
}

/// Comprueba que cada tesela `Station` tenga entrada en `state.stations` y viceversa.
#[must_use]
pub fn station_map_coherence(state: &crate::GameState) -> StationMapCoherenceReport {
    use std::collections::HashSet;

    let mut report = StationMapCoherenceReport::default();
    let state_positions: HashSet<(i32, i32)> =
        state.stations.iter().map(|s| (s.pos.x, s.pos.y)).collect();

    let (mw, mh) = state.map.dimensions();
    let mut tile_positions = HashSet::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if state.map.get_kind(c) == Some(TileKind::Station) {
                tile_positions.insert((c.x, c.y));
                if !state_positions.contains(&(c.x, c.y)) {
                    report.tiles_without_station.push(c);
                }
            }
        }
    }

    for station in &state.stations {
        let key = (station.pos.x, station.pos.y);
        if !tile_positions.contains(&key) {
            report.stations_without_tile.push(station.pos);
        }
    }

    report
}

#[must_use]
pub const fn station_covers_tile(station_pos: TileCoord, tile: TileCoord, radius: i32) -> bool {
    (tile.x - station_pos.x).abs() <= radius && (tile.y - station_pos.y).abs() <= radius
}

#[must_use]
pub fn industry_in_station_coverage(
    industry: &Industry,
    station_pos: TileCoord,
    radius: i32,
) -> bool {
    industry
        .tiles
        .iter()
        .copied()
        .chain(std::iter::once(industry.pos))
        .any(|tile| station_covers_tile(station_pos, tile, radius))
}

#[must_use]
pub fn industry_in_station_coverage_by_pos(
    industry_pos: TileCoord,
    station_or_source: TileCoord,
    radius: i32,
) -> bool {
    station_covers_tile(station_or_source, industry_pos, radius)
}

/// Rating 0–255 para un tipo de carga (255 = recién servido).
#[must_use]
pub fn station_rating_for_cargo(station: &Station, cargo: CargoType) -> u8 {
    255u8.saturating_sub(station.time_since_pickup.get(cargo))
}

/// Recalcula el rating global como mínimo entre cargas con stock en espera.
pub fn recompute_station_rating(station: &mut Station) {
    const CARGO_TYPES: [CargoType; 6] = [
        CargoType::Passengers,
        CargoType::Mail,
        CargoType::Goods,
        CargoType::Coal,
        CargoType::Wood,
        CargoType::Oil,
    ];
    let mut min_rating = 255u8;
    let mut any_waiting = false;
    for cargo in CARGO_TYPES {
        if station.cargo_stock.get(cargo) == 0 {
            continue;
        }
        any_waiting = true;
        min_rating = min_rating.min(station_rating_for_cargo(station, cargo));
    }
    station.rating = if any_waiting { min_rating } else { 255 };
}

/// Incrementa antigüedad de carga en espera (una vez por día simulado).
pub fn tick_station_cargo_age(stations: &mut [Station]) {
    const CARGO_TYPES: [CargoType; 6] = [
        CargoType::Passengers,
        CargoType::Mail,
        CargoType::Goods,
        CargoType::Coal,
        CargoType::Wood,
        CargoType::Oil,
    ];
    for station in stations {
        for cargo in CARGO_TYPES {
            if station.cargo_stock.get(cargo) > 0 {
                station.time_since_pickup.increment_waiting(cargo);
            }
        }
        recompute_station_rating(station);
    }
}

/// Marca recogida reciente de un tipo de carga.
pub fn on_station_cargo_pickup(station: &mut Station, cargo: CargoType) {
    station.time_since_pickup.set(cargo, 0);
    recompute_station_rating(station);
}

/// Factor 0–255 para limitar cantidad cargable según rating.
#[must_use]
pub fn load_amount_for_rating(requested: u32, rating: u8) -> u32 {
    if requested == 0 {
        return 0;
    }
    let scaled = (u64::from(requested) * u64::from(rating)) / 255;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

#[must_use]
pub fn station_coverage_at(
    map: &Map,
    industries: &[Industry],
    pos: TileCoord,
    radius: i32,
) -> StationCoverage {
    let mut coverage = StationCoverage::default();
    for y in pos.y - radius..=pos.y + radius {
        for x in pos.x - radius..=pos.x + radius {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            match tile.kind {
                TileKind::House => {
                    coverage.house_tiles += 1;
                    coverage.accepts_mail += 1;
                }
                TileKind::Industry => coverage.accepts_goods += 1,
                _ => {}
            }
        }
    }

    for industry in industries {
        if !industry_in_station_coverage(industry, pos, radius) {
            continue;
        }
        coverage.supplied_stock = coverage.supplied_stock.saturating_add(industry.stock);
        match industry.kind {
            IndustryKind::CoalMine => coverage.supplies_coal += 1,
            IndustryKind::Forest => coverage.supplies_wood += 1,
            IndustryKind::OilWell => coverage.supplies_oil += 1,
            IndustryKind::Factory => coverage.accepts_goods += 1,
        }
    }

    coverage
}

/// Parada donde el vehículo puede recoger mercancía primaria (mina, bosque, pozo).
#[must_use]
pub fn station_is_freight_pickup_stop(
    map: &Map,
    industries: &[Industry],
    station_pos: TileCoord,
    cargo: CargoType,
) -> bool {
    let coverage = station_coverage_at(map, industries, station_pos, STATION_COVERAGE_RADIUS);
    match cargo {
        CargoType::Coal => coverage.supplies_coal > 0,
        CargoType::Wood => coverage.supplies_wood > 0,
        CargoType::Oil => coverage.supplies_oil > 0,
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod coherence_tests {
    use super::*;
    use crate::{
        CargoType, Command, GameState, Industry, IndustryKind, PathNetwork, Vehicle, VehicleKind,
        command::apply_command, find_path,
    };

    #[test]
    fn rail_station_stop_tile_targets_platform_not_approach() {
        use crate::command::{Command, apply_command};
        let mut state = GameState::new(16, 12);
        for x in 2..=5 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 6))).unwrap();
        }
        let station = TileCoord::new(1, 6);
        apply_command(&mut state, &Command::PlaceRailStation(station, 2)).unwrap();
        assert_eq!(
            rail_station_approach_tile(&state.map, station),
            Some(TileCoord::new(2, 6))
        );
        assert_eq!(
            rail_station_stop_tile(&state.map, station),
            Some(station),
            "destino de orden = plataforma"
        );
        assert_eq!(
            resolve_order_destination(
                &state.map,
                VehicleKind::Train,
                VehicleOrder::station(station)
            ),
            station
        );
    }

    #[test]
    fn stop_kind_from_m6_maps_openttd_station_types() {
        assert_eq!(stop_kind_from_m6(2 << 3), StopKind::TruckStop);
        assert_eq!(stop_kind_from_m6(3 << 3), StopKind::BusStop);
        assert_eq!(stop_kind_from_m6(0), StopKind::RailStation);
        assert_eq!(stop_kind_from_m6(7 << 3), StopKind::RailWaypoint);
    }

    #[test]
    fn station_map_coherence_flags_orphan_tile_and_state() {
        let mut state = GameState::new(6, 6);
        state
            .map
            .set_kind(TileCoord::new(1, 1), TileKind::Station)
            .unwrap();
        state.stations.push(Station::new(TileCoord::new(3, 3)));
        let report = station_map_coherence(&state);
        assert_eq!(report.tiles_without_station, vec![TileCoord::new(1, 1)]);
        assert_eq!(report.stations_without_tile, vec![TileCoord::new(3, 3)]);
    }

    #[test]
    fn place_station_dir_keeps_map_and_state_aligned() {
        let mut state = GameState::new(8, 8);
        let road = TileCoord::new(4, 5);
        let stop = TileCoord::new(4, 4);
        apply_command(&mut state, &Command::PlaceRoad(road)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(stop, 1)).unwrap();
        let report = station_map_coherence(&state);
        assert!(report.tiles_without_station.is_empty());
        assert!(report.stations_without_tile.is_empty());
        assert_eq!(state.map.get_kind(stop), Some(TileKind::Station));
        assert_eq!(state.stations.len(), 1);
        assert_eq!(state.stations[0].pos, stop);
    }

    #[test]
    fn truck_does_not_reload_coal_at_deliver_on_load_order() {
        let mut state = GameState::new(16, 12);
        let load_stop = TileCoord::new(3, 5);
        let deliver_stop = TileCoord::new(10, 5);
        let deliver_road = TileCoord::new(10, 6);
        for x in 2..=12_i32 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(load_stop, 1)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();
        let deliver_idx = state
            .stations
            .iter()
            .position(|s| s.pos == deliver_stop)
            .expect("parada descarga");
        state.stations[deliver_idx].cargo_stock.coal = 20;

        let mut truck = Vehicle::new(9010, VehicleKind::Truck, deliver_road, load_stop);
        truck.running = true;
        truck.set_station_orders(vec![load_stop, deliver_stop]);
        truck.sync_order_destination(&state.map);
        state.vehicles.push(truck);

        state.step();
        assert_eq!(
            state.vehicles[0].cargo, 0,
            "orden de carga en mina: no tomar carbón en parada de descarga"
        );
    }

    #[test]
    fn truck_unloads_from_road_tile_adjacent_to_stop() {
        let mut state = GameState::new(16, 12);
        let load_road = TileCoord::new(3, 6);
        let load_stop = TileCoord::new(3, 5);
        let deliver_road = TileCoord::new(10, 6);
        let deliver_stop = TileCoord::new(10, 5);
        for x in 2..=12_i32 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(load_stop, 1)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();
        assert_eq!(
            road_stop_approach_tile(&state.map, load_stop),
            Some(load_road)
        );
        assert_eq!(
            road_stop_approach_tile(&state.map, deliver_stop),
            Some(deliver_road)
        );

        let mut mine = Industry::new(TileCoord::new(2, 3), IndustryKind::CoalMine);
        mine.stock = 64;
        state.industries.push(mine);

        let mut truck = Vehicle::new(9010, VehicleKind::Truck, load_road, load_stop);
        truck.running = true;
        truck.set_station_orders(vec![load_stop, deliver_stop]);
        truck.sync_order_destination(&state.map);
        assert_eq!(
            truck.dest, load_stop,
            "entra a la tesela de la bahía (Fase 2), no para en el acceso"
        );
        if let Some(path) = find_path(&state.map, load_road, truck.dest, PathNetwork::Road) {
            truck.path = path.into();
        }
        state.vehicles.push(truck);

        for t in 1..=400 {
            state.step();
            if state.stats.cargo_units_delivered > 0 {
                assert_eq!(
                    state.vehicles[0].cargo, 0,
                    "sin recarga instantánea tras la primera entrega (t={t})"
                );
                return;
            }
        }
        panic!("camión debe descargar en parada de destino");
    }

    #[test]
    fn truck_does_not_pick_up_wood_at_deliver_stop_after_unload() {
        let mut state = GameState::new(16, 12);
        let load_stop = TileCoord::new(3, 5);
        let deliver_stop = TileCoord::new(10, 5);
        let deliver_road = TileCoord::new(10, 6);
        for x in 2..=12_i32 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 6), 0x0A),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceStationDir(load_stop, 1)).unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();
        let deliver_idx = state
            .stations
            .iter()
            .position(|s| s.pos == deliver_stop)
            .expect("parada descarga");
        state.stations[deliver_idx].cargo_stock.wood = 160;

        // Fase 2: el camión descarga DENTRO de la bahía, no en el acceso.
        let mut truck = Vehicle::new(9010, VehicleKind::Truck, deliver_stop, deliver_stop);
        truck.running = true;
        truck.direction = crate::vehicle::DIR_NW;
        truck.cargo_type = Some(CargoType::Coal);
        truck.cargo = 20;
        truck.mark_cargo_loaded(TileCoord::new(2, 3));
        truck.set_station_orders(vec![load_stop, deliver_stop]);
        truck.current_order = 1;
        truck.sync_order_destination(&state.map);
        truck.progress = 255;
        state.vehicles.push(truck);
        let _ = deliver_road;

        state.step();
        assert_eq!(
            state.vehicles[0].cargo, 0,
            "debe descargar carbón en parada de entrega"
        );
        assert_eq!(
            state.vehicles[0].cargo_type, None,
            "sin recargar madera de stock de fábrica en el mismo tick"
        );
        assert_eq!(
            state.vehicles[0].current_order, 0,
            "tras entregar, la orden activa debe ser la de carga en mina"
        );
    }

    #[test]
    fn truck_unloads_wood_at_deliver_even_when_cargo_source_is_station() {
        let mut state = GameState::new(16, 12);
        let deliver_stop = TileCoord::new(10, 5);
        let deliver_road = TileCoord::new(10, 6);
        apply_command(
            &mut state,
            &Command::PlaceRoadBits(TileCoord::new(10, 6), 0x0A),
        )
        .unwrap();
        apply_command(&mut state, &Command::PlaceStationDir(deliver_stop, 1)).unwrap();

        // Fase 2: el camión descarga DENTRO de la bahía, no en el acceso.
        let mut truck = Vehicle::new(9010, VehicleKind::Truck, deliver_stop, deliver_stop);
        truck.running = true;
        truck.direction = crate::vehicle::DIR_NW;
        truck.cargo_type = Some(CargoType::Wood);
        truck.cargo = 20;
        truck.mark_cargo_loaded(deliver_stop);
        truck.set_station_orders(vec![deliver_stop]);
        truck.sync_order_destination(&state.map);
        truck.progress = 255;
        state.vehicles.push(truck);
        let _ = deliver_road;

        state.step();
        assert_eq!(
            state.vehicles[0].cargo, 0,
            "parada de entrega debe aceptar descarga aunque cargo_source sea la misma tesela"
        );
    }

    #[test]
    fn station_rating_decays_with_waiting_cargo() {
        let mut station = Station::new(TileCoord::new(0, 0));
        station.cargo_stock.coal = 50;
        tick_station_cargo_age(std::slice::from_mut(&mut station));
        assert_eq!(station.time_since_pickup.coal, 1);
        for _ in 0..300 {
            tick_station_cargo_age(std::slice::from_mut(&mut station));
        }
        assert_eq!(station.time_since_pickup.coal, MAX_TIME_SINCE_PICKUP_DAYS);
        assert!(station.rating < 255);
        on_station_cargo_pickup(&mut station, CargoType::Coal);
        assert_eq!(station.time_since_pickup.coal, 0);
        assert_eq!(station.rating, 255);
    }

    #[test]
    fn load_amount_for_rating_scales_down() {
        assert_eq!(load_amount_for_rating(100, 255), 100);
        assert_eq!(load_amount_for_rating(100, 128), 50);
        assert_eq!(load_amount_for_rating(100, 0), 0);
    }
}
