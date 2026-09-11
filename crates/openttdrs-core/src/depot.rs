//! Consultas sobre depósitos en el mapa (sin lógica de UI).

use std::collections::BTreeSet;

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::VehicleKind;

const WATER_TILE_TYPE_DEPOT: u8 = 3;

/// Cantidad máxima de entradas del `DepotPool` nativo (`DepotID`).
///
/// `OpenTTD` reserva `0xFFFF` como inválido y limita el pool a los IDs
/// `0..64000`; mantener el límite aquí evita que dos clases de depósito
/// escriban el mismo `MAP2` al construir en una partida nueva.
pub const DEPOT_POOL_SIZE: u16 = 64_000;

/// Distancia máxima en teselas para el servicio automático de barcos.
///
/// Coincide con `MAX_SHIP_DEPOT_SEARCH_DISTANCE` de `ship_cmd.cpp`. La
/// búsqueda real además exige una ruta navegable y el mismo propietario.
pub const MAX_SHIP_DEPOT_SEARCH_DISTANCE: u32 = 80;

/// Devuelve el `DepotID` almacenado en `MAP2` para un depósito de tierra o
/// naval. Los hangares de aeropuerto son estaciones y no pertenecen a este
/// pool nativo.
#[must_use]
pub fn depot_id_from_tile(tile: crate::map::Tile) -> Option<u16> {
    matches!(
        tile.kind,
        TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot
    )
    .then(|| u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8))
}

/// Busca el primer `DepotID` libre en el pool común de `OpenTTD`.
///
/// El barrido deduplica las dos secciones de un depósito naval porque ambas
/// teselas contienen el mismo ID. Más adelante el pool persistente podrá
/// reemplazar esta consulta por una estructura incremental; mientras tanto,
/// reconstruir desde `MAP2` también cubre mapas importados y `.ottdmap`.
#[must_use]
pub fn next_free_depot_id(map: &Map) -> Option<u16> {
    let mut used = BTreeSet::new();
    for &tile in map.tiles() {
        if let Some(id) = depot_id_from_tile(tile) {
            used.insert(id);
        }
    }
    (0..DEPOT_POOL_SIZE).find(|id| !used.contains(id))
}

/// Eje de una sección de depósito naval (`WBL_DEPOT_AXIS` en `m5`).
#[must_use]
pub fn ship_depot_axis(tile: crate::map::Tile) -> u8 {
    (tile.m5 >> 1) & 0x01
}

/// Parte norte/sur de una sección de depósito naval (`WBL_DEPOT_PART`).
#[must_use]
pub fn ship_depot_part(tile: crate::map::Tile) -> u8 {
    tile.m5 & 0x01
}

/// Dirección diagonal de salida del depósito naval (`GetShipDepotDirection`).
///
/// El depósito se identifica por su sección norte: `XYNSToDiagDir(axis, part)`
/// codifica la diagonal y `DiagDirToDir` la convierte a una `Direction` de
/// vehículo. El resultado es la orientación inicial física y gráfica de un
/// barco recién construido.
#[must_use]
pub fn ship_depot_facing(tile: crate::map::Tile) -> crate::vehicle::VehicleDirection {
    let diagdir = (ship_depot_axis(tile) * 3) ^ (ship_depot_part(tile) * 2);
    diagdir * 2 + 1
}

/// Devuelve las dos teselas de la huella naval para una dirección de construcción.
///
/// El primer elemento es la tesela que recibe `PlaceShipDepotDir`; el segundo
/// es la sección opuesta que `MakeShipDepot` materializa mediante
/// `GetOtherShipDepotTile`. La dirección es la convención diagonal nativa
/// (`0=NE`, `1=SE`, `2=SW`, `3=NW`) y se reduce a sus dos bits bajos.
#[must_use]
pub fn ship_depot_footprint(depot_pos: TileCoord, dir: u8) -> [TileCoord; 2] {
    let other = match dir & 0x03 {
        0 => TileCoord::new(depot_pos.x + 1, depot_pos.y),
        1 => TileCoord::new(depot_pos.x, depot_pos.y - 1),
        2 => TileCoord::new(depot_pos.x - 1, depot_pos.y),
        _ => TileCoord::new(depot_pos.x, depot_pos.y + 1),
    };
    [depot_pos, other]
}

#[must_use]
fn ship_depot_is_section(tile: crate::map::Tile) -> bool {
    tile.kind == TileKind::ShipDepot && (tile.m5 >> 4) & 0x0F == WATER_TILE_TYPE_DEPOT
}

/// Devuelve la otra tesela de la huella 2×1/1×2 del depósito naval.
///
/// El cálculo sigue `GetOtherShipDepotTile`: la parte norte apunta hacia el
/// incremento del eje y la parte sur hacia el decremento. Se exige que la
/// sección vecina tenga el mismo eje y la parte opuesta para no enlazar por
/// accidente dos depósitos contiguos importados.
#[must_use]
pub fn ship_depot_other_tile(map: &Map, pos: TileCoord) -> Option<TileCoord> {
    let tile = map.get(pos).filter(|tile| ship_depot_is_section(*tile))?;
    let delta = if ship_depot_axis(tile) == 0 {
        (1, 0)
    } else {
        (0, 1)
    };
    let part = ship_depot_part(tile);
    let other = if part == 0 {
        TileCoord::new(pos.x + delta.0, pos.y + delta.1)
    } else {
        TileCoord::new(pos.x - delta.0, pos.y - delta.1)
    };
    map.get(other)
        .is_some_and(|other_tile| {
            ship_depot_is_section(other_tile)
                && ship_depot_axis(other_tile) == ship_depot_axis(tile)
                && ship_depot_part(other_tile) != part
        })
        .then_some(other)
}

/// Devuelve la sección norte que identifica al depósito completo.
///
/// Una sección aislada (por ejemplo, un save antiguo o un mapa sintético de
/// prueba) se devuelve a sí misma para no desaparecer de las consultas.
#[must_use]
pub fn ship_depot_north_tile(map: &Map, pos: TileCoord) -> Option<TileCoord> {
    let tile = map
        .get(pos)
        .filter(|tile| tile.kind == TileKind::ShipDepot)?;
    if ship_depot_part(tile) == 0 {
        return Some(pos);
    }
    Some(ship_depot_other_tile(map, pos).unwrap_or(pos))
}

#[must_use]
fn is_depot_candidate(map: &Map, pos: TileCoord, kind: VehicleKind) -> bool {
    let target = depot_tile_kind_for_vehicle(kind);
    map.get_kind(pos) == Some(target)
        && (kind != VehicleKind::Ship
            || ship_depot_north_tile(map, pos).is_some_and(|north| north == pos))
}

/// Bit de reserva PBS en depósitos ferroviarios (`HasDepotReservation` / `m5` bit 4).
///
/// Coincide con el bit de cruces a nivel; la interpretación depende de `TileKind`.
pub const DEPOT_RESERVATION_M5_BIT: u8 = 1 << 4;

/// Índice espacial efímero de depósitos; el primer uso hace un solo barrido
/// del mapa y las consultas posteriores recorren únicamente depósitos.
#[derive(Debug, Clone, Default)]
pub struct DepotSpatialIndex {
    road: BTreeSet<TileCoord>,
    rail: BTreeSet<TileCoord>,
    ship: BTreeSet<TileCoord>,
    airport: BTreeSet<TileCoord>,
    initialized: bool,
    full_map_scans: u64,
}

impl DepotSpatialIndex {
    fn ensure_initialized(&mut self, map: &Map) {
        if self.initialized {
            return;
        }
        self.road.clear();
        self.rail.clear();
        self.ship.clear();
        self.airport.clear();
        let (width, height) = map.dimensions();
        for y in 0..height.cast_signed() {
            for x in 0..width.cast_signed() {
                let pos = TileCoord::new(x, y);
                if let Some(kind) = map.get_kind(pos) {
                    self.insert_kind(map, pos, kind);
                }
            }
        }
        self.initialized = true;
        self.full_map_scans = self.full_map_scans.saturating_add(1);
    }

    fn insert_kind(&mut self, map: &Map, pos: TileCoord, kind: TileKind) {
        match kind {
            TileKind::RoadDepot => {
                self.road.insert(pos);
            }
            TileKind::RailDepot => {
                self.rail.insert(pos);
            }
            TileKind::ShipDepot if ship_depot_north_tile(map, pos) == Some(pos) => {
                self.ship.insert(pos);
            }
            TileKind::Airport => {
                self.airport.insert(pos);
            }
            _ => {}
        }
    }

    /// Invalida tras una mutación de mapa; el próximo lookup reconstruye una vez.
    pub fn invalidate(&mut self) {
        self.initialized = false;
    }

    #[must_use]
    pub const fn full_map_scans(&self) -> u64 {
        self.full_map_scans
    }

    #[must_use]
    pub fn len_for(&mut self, map: &Map, kind: VehicleKind) -> usize {
        self.ensure_initialized(map);
        self.candidates(kind).len()
    }

    fn candidates(&self, kind: VehicleKind) -> &BTreeSet<TileCoord> {
        match kind {
            VehicleKind::Train => &self.rail,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => &self.road,
            VehicleKind::Ship => &self.ship,
            VehicleKind::Aircraft => &self.airport,
        }
    }
}

/// `HasDepotReservation` (`rail_map.h`): bit 4 de `m5` en `RailDepot`.
#[must_use]
pub fn has_depot_reservation(map: &Map, pos: TileCoord) -> bool {
    map.get(pos)
        .is_some_and(|t| t.kind == TileKind::RailDepot && t.m5 & DEPOT_RESERVATION_M5_BIT != 0)
}

/// `SetDepotReservation` (`rail_map.h`). Devuelve `true` si el mapa cambió.
pub fn set_depot_reservation(map: &mut Map, pos: TileCoord, reserved: bool) -> bool {
    let Some(mut tile) = map.get(pos) else {
        return false;
    };
    if tile.kind != TileKind::RailDepot {
        return false;
    }
    let had = tile.m5 & DEPOT_RESERVATION_M5_BIT != 0;
    if reserved {
        tile.m5 |= DEPOT_RESERVATION_M5_BIT;
    } else {
        tile.m5 &= !DEPOT_RESERVATION_M5_BIT;
    }
    if had == reserved {
        return false;
    }
    let _ = map.set_tile(pos, tile);
    true
}

/// Limpia reservas de depósito huérfanas tras importar un `.sav` (`afterload.cpp`).
pub fn clear_all_depot_reservations(map: &mut Map) {
    let (w, h) = map.dimensions();
    for y in 0..h.cast_signed() {
        for x in 0..w.cast_signed() {
            let c = TileCoord::new(x, y);
            let _ = set_depot_reservation(map, c, false);
        }
    }
}

/// Tesela de depósito compatible con el tipo de vehículo.
#[must_use]
pub fn depot_tile_kind_for_vehicle(kind: VehicleKind) -> TileKind {
    match kind {
        VehicleKind::Train => TileKind::RailDepot,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => TileKind::RoadDepot,
        VehicleKind::Ship => TileKind::ShipDepot,
        VehicleKind::Aircraft => TileKind::Airport,
    }
}

/// Depósito más cercano en distancia Manhattan desde `from`.
#[must_use]
pub fn nearest_depot_tile(map: &Map, from: TileCoord, kind: VehicleKind) -> Option<TileCoord> {
    let (mw, mh) = map.dimensions();
    let mut best: Option<(u32, TileCoord)> = None;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if is_depot_candidate(map, c, kind) {
                let dist = from.x.abs_diff(c.x) + from.y.abs_diff(c.y);
                if best.is_none_or(|(d, best_c)| (dist, c.y, c.x) < (d, best_c.y, best_c.x)) {
                    best = Some((dist, c));
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Variante indexada: O(cantidad de depósitos), independiente del área del mapa.
#[must_use]
pub fn nearest_depot_tile_indexed(
    map: &Map,
    from: TileCoord,
    kind: VehicleKind,
    index: &mut DepotSpatialIndex,
) -> Option<TileCoord> {
    index.ensure_initialized(map);
    index
        .candidates(kind)
        .iter()
        .copied()
        .min_by_key(|c| (from.x.abs_diff(c.x) + from.y.abs_diff(c.y), c.y, c.x))
}

/// Depósito alcanzable más cercano por pathfinding (road/tram).
#[must_use]
pub fn nearest_reachable_depot_tile(
    map: &Map,
    from: TileCoord,
    kind: VehicleKind,
) -> Option<TileCoord> {
    use crate::pathfinder::{PathNetwork, find_path};

    let target = depot_tile_kind_for_vehicle(kind);
    let network = if kind == VehicleKind::Tram {
        PathNetwork::Tram
    } else {
        PathNetwork::Road
    };
    let (mw, mh) = map.dimensions();
    let mut best: Option<(u32, TileCoord)> = None;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if map.get_kind(c) != Some(target) {
                continue;
            }
            let path_target = road_depot_entrance_tile(map, c).unwrap_or(c);
            if find_path(map, from, path_target, network).is_none() {
                continue;
            }
            let dist = from.x.abs_diff(c.x) + from.y.abs_diff(c.y);
            if best.is_none_or(|(d, _)| dist < d) {
                best = Some((dist, c));
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Variante alcanzable apoyada en [`DepotSpatialIndex`].
#[must_use]
pub fn nearest_reachable_depot_tile_indexed(
    map: &Map,
    from: TileCoord,
    kind: VehicleKind,
    index: &mut DepotSpatialIndex,
) -> Option<TileCoord> {
    use crate::pathfinder::{PathNetwork, find_path};

    index.ensure_initialized(map);
    let network = if kind == VehicleKind::Tram {
        PathNetwork::Tram
    } else {
        PathNetwork::Road
    };
    // El primer depósito alcanzable en este orden es precisamente el mínimo
    // que devolvía el `filter(...).min_by_key(...)` anterior. Parar al
    // encontrarlo evita correr A* contra todos los depósitos más lejanos.
    let mut candidates: Vec<_> = index.candidates(kind).iter().copied().collect();
    candidates.sort_by_key(|c| (from.x.abs_diff(c.x) + from.y.abs_diff(c.y), c.y, c.x));
    candidates.into_iter().find(|&depot| {
        let target = road_depot_entrance_tile(map, depot).unwrap_or(depot);
        find_path(map, from, target, network).is_some()
    })
}

/// Depósito naval propio más cercano y alcanzable dentro de una distancia
/// euclidiana máxima.
///
/// `FindClosestShipDepot` nativo primero restringe la búsqueda a regiones de
/// agua alcanzables y después compara `DistanceSquare`, sin aceptar depósitos
/// de otra compañía. El pathfinder local aún no conserva regiones de agua;
/// probar la ruta completa contra los pocos depósitos indexados proporciona
/// el mismo contrato observable y evita elegir un depósito de una cuenca
/// aislada.
#[must_use]
pub fn nearest_reachable_ship_depot_tile_indexed(
    map: &Map,
    from: TileCoord,
    owner: crate::company::CompanyId,
    max_distance: u32,
    index: &mut DepotSpatialIndex,
) -> Option<TileCoord> {
    use crate::pathfinder::{PathNetwork, find_path};

    index.ensure_initialized(map);
    let max_distance_squared = max_distance.saturating_mul(max_distance);
    index
        .candidates(VehicleKind::Ship)
        .iter()
        .filter_map(|&depot| {
            let tile = map.get(depot)?;
            if tile.m1 & 0x1F != owner.0 {
                return None;
            }
            let dx = from.x.abs_diff(depot.x);
            let dy = from.y.abs_diff(depot.y);
            let distance_squared = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy));
            if distance_squared > max_distance_squared {
                return None;
            }
            find_path(map, from, depot, PathNetwork::Water)?;
            Some((distance_squared, depot.y, depot.x, depot))
        })
        .min_by_key(|(distance_squared, y, x, _)| (*distance_squared, *y, *x))
        .map(|(_, _, _, depot)| depot)
}

/// Boca del depósito de vía (`m5 & 3`) si la tesela es un depósito ferroviario.
#[must_use]
pub fn rail_depot_mouth_dir(map: &Map, pos: TileCoord) -> Option<u8> {
    map.get(pos)
        .filter(|t| t.kind == TileKind::RailDepot)
        .map(|t| t.m5 & 0x03)
}

/// Tesela de vía contigua a la boca del depósito (entrada/salida).
#[must_use]
pub fn rail_depot_entrance_tile(map: &Map, depot_pos: TileCoord) -> Option<TileCoord> {
    let mouth = rail_depot_mouth_dir(map, depot_pos)?;
    let ((dx, dy), _) = match mouth & 0x03 {
        0 => ((-1_i32, 0_i32), 0x01_u8),
        1 => ((0_i32, 1_i32), 0x02_u8),
        2 => ((1_i32, 0_i32), 0x01_u8),
        _ => ((0_i32, -1_i32), 0x02_u8),
    };
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some(c)
}

/// Depósito ferroviario cuya boca es `entrance` (tesela de vía vecina).
#[must_use]
pub fn rail_depot_for_entrance_tile(map: &Map, entrance: TileCoord) -> Option<TileCoord> {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let depot = TileCoord::new(entrance.x + dx, entrance.y + dy);
        if map.get_kind(depot) == Some(TileKind::RailDepot)
            && rail_depot_entrance_tile(map, depot) == Some(entrance)
        {
            return Some(depot);
        }
    }
    None
}

/// Boca del depósito road (`m5 & 3`) si la tesela corresponde a uno.
#[must_use]
pub fn road_depot_mouth_dir(map: &Map, pos: TileCoord) -> Option<u8> {
    map.get(pos)
        .filter(|t| t.kind == TileKind::RoadDepot)
        .map(|t| t.m5 & 0x03)
}

/// Tesela road contigua a la boca del depósito.
#[must_use]
pub fn road_depot_entrance_tile(map: &Map, depot_pos: TileCoord) -> Option<TileCoord> {
    let mouth = road_depot_mouth_dir(map, depot_pos)?;
    let (dx, dy) = match mouth {
        0 => (-1_i32, 0_i32),
        1 => (0_i32, 1_i32),
        2 => (1_i32, 0_i32),
        _ => (0_i32, -1_i32),
    };
    let entrance = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    (entrance.x >= 0
        && entrance.y >= 0
        && entrance.x < mw.cast_signed()
        && entrance.y < mh.cast_signed())
    .then_some(entrance)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::map::TileKind;
    use crate::{Command, GameState, apply_command};

    use super::*;

    #[test]
    fn nearest_depot_picks_closest_manhattan() {
        let mut s = GameState::new(12, 12);
        let near = TileCoord::new(4, 4);
        let far = TileCoord::new(9, 9);
        apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(4, 3))).unwrap();
        apply_command(&mut s, &Command::PlaceRoadDepotDir(near, 3)).unwrap();
        apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(9, 8))).unwrap();
        apply_command(&mut s, &Command::PlaceRoadDepotDir(far, 3)).unwrap();
        let from = TileCoord::new(3, 4);
        assert_eq!(
            nearest_depot_tile(&s.map, from, VehicleKind::Bus),
            Some(near)
        );
    }

    #[test]
    fn indexed_lookup_scans_large_map_only_once() {
        let mut s = GameState::new(256, 256);
        let depot = TileCoord::new(240, 240);
        s.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        let mut index = DepotSpatialIndex::default();
        assert_eq!(
            nearest_depot_tile_indexed(
                &s.map,
                TileCoord::new(0, 0),
                VehicleKind::Truck,
                &mut index,
            ),
            Some(depot)
        );
        assert_eq!(index.full_map_scans(), 1);
        let _ = nearest_depot_tile_indexed(
            &s.map,
            TileCoord::new(1, 1),
            VehicleKind::Truck,
            &mut index,
        );
        assert_eq!(index.full_map_scans(), 1);
    }

    #[test]
    fn ship_depot_queries_keep_only_the_north_section() {
        let mut s = GameState::new(12, 12);
        let depot = TileCoord::new(5, 5);
        let north = TileCoord::new(4, 5);
        let mouth = TileCoord::new(6, 5);
        for coord in [depot, north, mouth] {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }
        apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 2)).unwrap();

        assert_eq!(ship_depot_north_tile(&s.map, depot), Some(north));
        assert_eq!(ship_depot_north_tile(&s.map, north), Some(north));
        assert_eq!(
            nearest_depot_tile(&s.map, TileCoord::new(0, 5), VehicleKind::Ship),
            Some(north)
        );

        let mut index = DepotSpatialIndex::default();
        assert_eq!(index.len_for(&s.map, VehicleKind::Ship), 1);
        assert_eq!(
            nearest_depot_tile_indexed(&s.map, TileCoord::new(0, 5), VehicleKind::Ship, &mut index,),
            Some(north)
        );
    }

    #[test]
    fn reachable_ship_depot_filters_owner_and_distance() {
        let mut s = GameState::new(24, 8);
        for y in [2_i32, 3_i32] {
            for x in 0..24_i32 {
                crate::map::make_water_tile(
                    &mut s.map,
                    TileCoord::new(x, y),
                    crate::WaterClass::Sea,
                )
                .unwrap();
            }
        }
        let rival = TileCoord::new(5, 2);
        let own = TileCoord::new(17, 2);
        crate::apply_command(&mut s, &crate::Command::PlaceShipDepotDir(rival, 3)).unwrap();
        crate::apply_command(&mut s, &crate::Command::PlaceShipDepotDir(own, 3)).unwrap();
        for tile in crate::ship_depot_footprint(rival, 3) {
            let mut raw = s.map.get(tile).unwrap();
            raw.m1 = (raw.m1 & !0x1F) | crate::CompanyId(1).0;
            s.map.set_tile(tile, raw).unwrap();
        }

        let from = TileCoord::new(7, 2);
        let mut index = DepotSpatialIndex::default();
        assert_eq!(
            nearest_reachable_ship_depot_tile_indexed(
                &s.map,
                from,
                crate::CompanyId::PLAYER,
                MAX_SHIP_DEPOT_SEARCH_DISTANCE,
                &mut index,
            ),
            Some(own)
        );
        assert_eq!(
            nearest_reachable_ship_depot_tile_indexed(
                &s.map,
                from,
                crate::CompanyId(1),
                MAX_SHIP_DEPOT_SEARCH_DISTANCE,
                &mut index,
            ),
            Some(rival)
        );
        assert_eq!(
            nearest_reachable_ship_depot_tile_indexed(
                &s.map,
                from,
                crate::CompanyId::PLAYER,
                9,
                &mut index,
            ),
            None,
            "la distancia máxima usa DistanceSquare, no el largo de la ruta"
        );
    }

    #[test]
    fn reachable_ship_depot_ignores_closer_isolated_basin() {
        let mut s = GameState::new(24, 12);
        for x in 0..24_i32 {
            for y in [2_i32, 3_i32] {
                crate::map::make_water_tile(
                    &mut s.map,
                    TileCoord::new(x, y),
                    crate::WaterClass::Sea,
                )
                .unwrap();
            }
        }
        for y in [7_i32, 8_i32] {
            crate::map::make_water_tile(&mut s.map, TileCoord::new(5, y), crate::WaterClass::Sea)
                .unwrap();
        }
        let reachable = TileCoord::new(17, 2);
        let isolated = TileCoord::new(5, 7);
        crate::apply_command(&mut s, &crate::Command::PlaceShipDepotDir(reachable, 3)).unwrap();
        crate::apply_command(&mut s, &crate::Command::PlaceShipDepotDir(isolated, 3)).unwrap();

        let mut index = DepotSpatialIndex::default();
        assert_eq!(
            nearest_reachable_ship_depot_tile_indexed(
                &s.map,
                TileCoord::new(5, 2),
                crate::CompanyId::PLAYER,
                MAX_SHIP_DEPOT_SEARCH_DISTANCE,
                &mut index,
            ),
            Some(reachable),
            "una cuenca aislada no puede ganar sólo por distancia geométrica"
        );
    }

    #[test]
    fn ship_depot_footprint_matches_native_direction_offsets() {
        let origin = TileCoord::new(5, 5);
        assert_eq!(
            ship_depot_footprint(origin, 0),
            [origin, TileCoord::new(6, 5)]
        );
        assert_eq!(
            ship_depot_footprint(origin, 1),
            [origin, TileCoord::new(5, 4)]
        );
        assert_eq!(
            ship_depot_footprint(origin, 2),
            [origin, TileCoord::new(4, 5)]
        );
        assert_eq!(
            ship_depot_footprint(origin, 3),
            [origin, TileCoord::new(5, 6)]
        );
        assert_eq!(
            ship_depot_footprint(origin, 7),
            ship_depot_footprint(origin, 3)
        );
    }

    #[test]
    fn ship_depot_facing_matches_north_section_for_each_axis() {
        let cases = [(0u8, 1u8), (1, 7), (2, 1), (3, 7)];
        for (dir, expected) in cases {
            let mut s = GameState::new(12, 12);
            let depot = TileCoord::new(5, 5);
            let [origin, other] = ship_depot_footprint(depot, dir);
            s.map.set_kind(origin, TileKind::Water).unwrap();
            s.map.set_kind(other, TileKind::Water).unwrap();
            apply_command(&mut s, &Command::PlaceShipDepotDir(depot, dir)).unwrap();

            let north = ship_depot_north_tile(&s.map, depot).unwrap();
            assert_eq!(ship_depot_facing(s.map.get(north).unwrap()), expected);
        }
    }

    #[test]
    fn train_uses_rail_depot_only() {
        let mut s = GameState::new(8, 8);
        let road = TileCoord::new(1, 1);
        let rail = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 0))).unwrap();
        apply_command(&mut s, &Command::PlaceRoadDepotDir(road, 3)).unwrap();
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(5, 4))).unwrap();
        apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
        assert_eq!(
            nearest_depot_tile(&s.map, TileCoord::new(0, 0), VehicleKind::Train),
            Some(rail)
        );
        assert_eq!(s.map.get_kind(road), Some(TileKind::RoadDepot));
    }

    #[test]
    fn depot_reservation_bit_roundtrips() {
        let mut s = GameState::new(8, 8);
        let rail = TileCoord::new(5, 5);
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(5, 4))).unwrap();
        apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
        assert!(!has_depot_reservation(&s.map, rail));
        assert!(set_depot_reservation(&mut s.map, rail, true));
        assert!(has_depot_reservation(&s.map, rail));
        assert!(set_depot_reservation(&mut s.map, rail, false));
        assert!(!has_depot_reservation(&s.map, rail));
        // Tesela no-depósito: no-op.
        assert!(!set_depot_reservation(
            &mut s.map,
            TileCoord::new(5, 4),
            true
        ));
    }

    #[test]
    fn depot_id_allocator_reads_map2_and_reuses_the_first_free_id() {
        let mut s = GameState::new(8, 8);
        let road = TileCoord::new(1, 1);
        let rail = TileCoord::new(2, 1);
        let ship = TileCoord::new(3, 1);

        s.map.set_kind(road, TileKind::RoadDepot).unwrap();
        s.map.set_m2_u16(road, 0).unwrap();
        s.map.set_kind(rail, TileKind::RailDepot).unwrap();
        s.map.set_m2_u16(rail, 2).unwrap();
        s.map.set_kind(ship, TileKind::ShipDepot).unwrap();
        s.map.set_m2_u16(ship, 5).unwrap();

        assert_eq!(depot_id_from_tile(s.map.get(road).unwrap()), Some(0));
        assert_eq!(depot_id_from_tile(s.map.get(rail).unwrap()), Some(2));
        assert_eq!(depot_id_from_tile(s.map.get(ship).unwrap()), Some(5));
        assert_eq!(next_free_depot_id(&s.map), Some(1));
    }

    #[test]
    fn airport_map2_does_not_consume_depot_pool_id() {
        let mut s = GameState::new(8, 8);
        let airport = TileCoord::new(1, 1);
        s.map.set_kind(airport, TileKind::Airport).unwrap();
        s.map.set_m2_u16(airport, 0).unwrap();

        assert_eq!(depot_id_from_tile(s.map.get(airport).unwrap()), None);
        assert_eq!(next_free_depot_id(&s.map), Some(0));
    }
}
