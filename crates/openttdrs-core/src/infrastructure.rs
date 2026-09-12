//! Contadores efímeros de infraestructura derivados del mapa.
//!
//! `OpenTTD` no serializa `CompanyInfrastructure`: lo reconstruye al cargar un
//! save y lo mantiene actualizado mientras ejecuta comandos. El runtime
//! propio todavía no tiene el contador incremental en cada Company, por lo
//! que estas superficies reconstruyen los contratos de infraestructura desde
//! el mapa y las entidades persistentes.

use crate::bridge_spec::{bridge_line_tiles, rail_bridge_other_end};
use crate::company::CompanyId;
use crate::map::{
    Map, Tile, TileCoord, TileKind, WaterClass, diag_dir_offset, is_map_object_tile,
    is_road_level_crossing, resolve_existing_tunnel_end, water_class_from_m1,
};
use crate::rail_signals::{rail_signal_present_mask, rail_tile_is_signals, tracks_overlap};
use crate::rail_type::{RailType, rail_type_from_tile};
use crate::road_type::{
    RoadTramType, RoadType, RoadTypeDef, road_type_def, road_type_from_tile,
    tram_road_type_from_tile,
};
use crate::station::{
    Station, StopKind, is_rail_station_type, station_at_tile, station_type_from_m6,
    stop_kind_from_m6,
};

/// Cantidad de railtypes vanilla que puede tener una compañía.
pub const RAIL_INFRASTRUCTURE_RAILTYPE_COUNT: usize = 4;

/// IDs de `RoadType` que puede transportar la codificación `m3hi`/`m8`.
pub const ROAD_INFRASTRUCTURE_ROADTYPE_COUNT: usize = 64;

/// Parte ferroviaria de `CompanyInfrastructure`.
///
/// Los valores son piezas de infraestructura, no cantidad de teselas. Una
/// tesela plana con cruce puede aportar varias piezas; un túnel o puente se
/// pondera cuatro veces por su longitud, igual que el contador nativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RailInfrastructureSummary {
    /// Piezas por `RailType`: normal, eléctrica, monorail y maglev.
    pub rail: [u32; RAIL_INFRASTRUCTURE_RAILTYPE_COUNT],
    /// Señales presentes, no cantidad de teselas señalizadas.
    pub signals: u32,
}

impl RailInfrastructureSummary {
    /// Suma de piezas ferroviarias en todos los railtypes.
    #[must_use]
    pub fn rail_total(self) -> u32 {
        self.rail.iter().copied().sum()
    }

    /// Piezas de un railtype concreto.
    #[must_use]
    pub fn rail_type_count(self, rail_type: RailType) -> u32 {
        self.rail[usize::from(rail_type.as_u8())]
    }
}

/// Parte vial de `CompanyInfrastructure`.
///
/// Se conserva un contador por ID, en vez de reducirlo a carretera/tranvía,
/// porque los tipos `NewGRF` ocupan el mismo espacio nativo y cada uno puede
/// tener un propietario distinto en una tesela compartida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadInfrastructureSummary {
    /// Piezas por `RoadType`; los IDs 0..63 coinciden con `m3hi`/`m8`.
    pub road: [u32; ROAD_INFRASTRUCTURE_ROADTYPE_COUNT],
}

impl Default for RoadInfrastructureSummary {
    fn default() -> Self {
        Self {
            road: [0; ROAD_INFRASTRUCTURE_ROADTYPE_COUNT],
        }
    }
}

impl RoadInfrastructureSummary {
    /// Suma de piezas viales en todos los tipos road/tram.
    #[must_use]
    pub fn road_total(self) -> u32 {
        self.road.iter().copied().sum()
    }

    /// Piezas de un tipo vial concreto.
    #[must_use]
    pub fn road_type_count(self, road_type: RoadType) -> u32 {
        self.road[usize::from(road_type.as_u8())]
    }

    /// Suma de los tipos cuya clase efectiva es carretera o tranvía.
    #[must_use]
    pub fn road_class_total(self, class: RoadTramType, catalog: &[RoadTypeDef]) -> u32 {
        self.road
            .iter()
            .enumerate()
            .filter_map(|(id, pieces)| {
                let id = u8::try_from(id).ok()?;
                let road_type = RoadType::from_u8(id);
                let effective_class = road_type_def(catalog, road_type)
                    .map_or_else(|| road_type.road_tram_type(), |definition| definition.class);
                (effective_class == class).then_some(*pieces)
            })
            .sum()
    }
}

/// Parte acuática de `CompanyInfrastructure`.
///
/// `OpenTTD` sólo contabiliza agua que forma parte de la infraestructura de una
/// compañía: canales, las piezas estructurales de depósitos/esclusas y los
/// acueductos. El mar, los ríos y el agua libre no pertenecen a ninguna
/// compañía y por eso no entran en este contador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WaterInfrastructureSummary {
    /// Piezas acuáticas según `AfterLoadCompanyStats`.
    pub water: u32,
}

impl WaterInfrastructureSummary {
    /// Suma de piezas de infraestructura acuática.
    #[must_use]
    pub const fn water_total(self) -> u32 {
        self.water
    }
}

/// Parte de estaciones de `CompanyInfrastructure`.
///
/// stations cuenta teselas `MP_STATION` propias, excepto aeropuertos y boyas.
/// airports cuenta estaciones con facilidad aérea una sola vez por entidad;
/// esto incluye una plataforma petrolera cuando conserva esa facilidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StationInfrastructureSummary {
    /// Teselas de estación que no son aeropuerto ni boya.
    pub stations: u32,
    /// Entidades propias con facilidad aérea.
    pub airports: u32,
}

impl StationInfrastructureSummary {
    /// Cantidad de piezas de estación no aéreas.
    #[must_use]
    pub const fn station_total(self) -> u32 {
        self.stations
    }

    /// Cantidad de aeropuertos/facilidades aéreas.
    #[must_use]
    pub const fn airport_total(self) -> u32 {
        self.airports
    }
}

const TUNNELBRIDGE_TRACKBIT_FACTOR: u32 = 4;
const LEVELCROSSING_TRACKBIT_FACTOR: u32 = 2;
const ROAD_STOP_TRACKBIT_FACTOR: u32 = 2;
const LOCK_DEPOT_TILE_FACTOR: u32 = 2;
const INVALID_ROADTYPE: u8 = 0x3F;
const OTTD_TILETYPE_STATION: u8 = 5;
const STATION_TYPE_AIRPORT: u8 = 1;
const STATION_TYPE_BUOY: u8 = crate::station::STATION_TYPE_BUOY;
const WATER_TILE_TYPE_LOCK: u8 = 2;
const WATER_TILE_TYPE_DEPOT: u8 = 3;

fn tile_owned_by(tile: Tile, owner: CompanyId) -> bool {
    tile.m1 & 0x1F == owner.0 & 0x1F
}

fn add_rail_pieces(summary: &mut RailInfrastructureSummary, tile: Tile, pieces: u32) {
    let slot = &mut summary.rail[usize::from(rail_type_from_tile(tile).as_u8())];
    *slot = slot.saturating_add(pieces);
}

fn plain_rail_pieces(tile: Tile) -> u32 {
    // RailTileType::Normal es el único subtipo que cuenta sus TrackBits. Los
    // subtipos señales y depot aportan una pieza, aunque la máscara tenga más
    // de un bit.
    if (tile.m5 >> 6) & 0x03 != 0 {
        return 1;
    }
    let bits = tile.m5 & 0x3F;
    let pieces = bits.count_ones();
    if tracks_overlap(bits) {
        pieces.saturating_mul(pieces)
    } else {
        pieces
    }
}

fn same_tunnel_kind(map: &Map, coord: TileCoord, kind: TileKind) -> bool {
    map.get(coord)
        .is_some_and(|tile| tile.kind == kind && tile.is_tunnel_bridge_tile())
}

fn is_tunnel_entry(map: &Map, coord: TileCoord, tile: Tile, kind: TileKind) -> bool {
    let (dx, dy) = diag_dir_offset(tile.m5 & 0x03);
    let behind = TileCoord::new(coord.x - dx, coord.y - dy);
    !same_tunnel_kind(map, behind, kind)
}

fn tunnel_end_for_count(
    map: &Map,
    start: TileCoord,
    tile: Tile,
    kind: TileKind,
) -> Option<TileCoord> {
    let (dx, dy) = diag_dir_offset(tile.m5 & 0x03);
    let (width, height) = map.dimensions();
    let mut contiguous_end = start;
    for _ in 0..width.max(height) {
        let next = TileCoord::new(contiguous_end.x + dx, contiguous_end.y + dy);
        if !same_tunnel_kind(map, next, kind) {
            break;
        }
        contiguous_end = next;
    }
    if contiguous_end == start {
        // Los túneles importados normalmente sólo exponen las dos bocas en
        // MP_TUNNELBRIDGE; en ese formato no hay tramo contiguo que recorrer.
        resolve_existing_tunnel_end(map, start)
    } else {
        Some(contiguous_end)
    }
}

fn tunnel_span_len(start: TileCoord, end: TileCoord, direction: u8) -> Option<u32> {
    let (dx, dy) = diag_dir_offset(direction);
    let delta_x = end.x - start.x;
    let delta_y = end.y - start.y;
    let steps = if dx != 0 {
        if delta_y != 0 || delta_x.signum() != dx {
            return None;
        }
        delta_x.unsigned_abs()
    } else {
        if delta_x != 0 || delta_y.signum() != dy {
            return None;
        }
        delta_y.unsigned_abs()
    };
    (steps > 0).then(|| steps.saturating_add(1))
}

fn is_before_in_map_order(a: TileCoord, b: TileCoord) -> bool {
    // Map::tiles() y-major order es el mismo orden lineal que usa TileIndex
    // para elegir el extremo norte de un túnel/puente.
    (a.y, a.x) < (b.y, b.x)
}

fn add_rail_tunnel(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    summary: &mut RailInfrastructureSummary,
) {
    let Some(end) = tunnel_end_for_count(map, coord, tile, TileKind::RailTunnel) else {
        return;
    };
    if !is_before_in_map_order(coord, end)
        || !is_tunnel_entry(map, coord, tile, TileKind::RailTunnel)
    {
        return;
    }
    let Some(length) = tunnel_span_len(coord, end, tile.m5 & 0x03) else {
        return;
    };
    add_rail_pieces(
        summary,
        tile,
        length.saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR),
    );
}

fn add_rail_bridge(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    summary: &mut RailInfrastructureSummary,
) {
    let Some(end) = rail_bridge_other_end(map, coord) else {
        return;
    };
    if !is_before_in_map_order(coord, end) {
        return;
    }
    let length = u32::try_from(bridge_line_tiles(coord, end).len()).unwrap_or(u32::MAX);
    add_rail_pieces(
        summary,
        tile,
        length.saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR),
    );
}

fn add_road_pieces(summary: &mut RoadInfrastructureSummary, road_type: RoadType, pieces: u32) {
    if road_type.as_u8() == INVALID_ROADTYPE {
        return;
    }
    let slot = &mut summary.road[usize::from(road_type.as_u8())];
    *slot = slot.saturating_add(pieces);
}

#[must_use]
fn water_tile_type(tile: Tile) -> u8 {
    (tile.m5 >> 4) & 0x0F
}

#[must_use]
fn company_owner_matches(tile: Tile, owner: CompanyId) -> bool {
    owner.0 < crate::company::MAX_COMPANIES && owner_slot(tile.m1) == owner_slot(owner.0)
}

fn add_water_pieces(summary: &mut WaterInfrastructureSummary, pieces: u32) {
    summary.water = summary.water.saturating_add(pieces);
}

/// Cuenta una tesela `MP_WATER` con las mismas excepciones que
/// `AfterLoadCompanyStats`.
fn add_water_tile(summary: &mut WaterInfrastructureSummary, tile: Tile, owner: CompanyId) {
    let tile_type = water_tile_type(tile);
    if tile_type == WATER_TILE_TYPE_DEPOT && company_owner_matches(tile, owner) {
        add_water_pieces(summary, LOCK_DEPOT_TILE_FACTOR);
    }

    if tile_type == WATER_TILE_TYPE_LOCK && (tile.m5 >> 2).trailing_zeros() >= 2 {
        // La parte central guarda el owner de toda la esclusa y reemplaza el
        // posible canal subyacente por tres piezas estructurales.
        if company_owner_matches(tile, owner) {
            add_water_pieces(summary, 3 * LOCK_DEPOT_TILE_FACTOR);
        }
        return;
    }

    if water_class_from_m1(tile.m1) == WaterClass::Canal && company_owner_matches(tile, owner) {
        add_water_pieces(summary, 1);
    }
}

#[must_use]
fn is_water_aqueduct_endpoint(tile: Tile) -> bool {
    tile.is_tunnel_bridge_tile() && tile.m5 & 0x80 != 0 && (tile.m5 >> 2) & 0x03 == 2
}

fn water_aqueduct_other_end(map: &Map, start: TileCoord, tile: Tile) -> Option<TileCoord> {
    let (dx, dy) = diag_dir_offset(tile.m5 & 0x03);
    let reverse_direction = (tile.m5.wrapping_add(2)) & 0x03;
    let (width, height) = map.dimensions();
    let mut pos = start;
    for _ in 0..width.max(height) {
        pos = TileCoord::new(pos.x + dx, pos.y + dy);
        let probe = map.get(pos)?;
        if is_water_aqueduct_endpoint(probe) && probe.m5 & 0x03 == reverse_direction {
            return Some(pos);
        }
    }
    None
}

fn add_water_aqueduct(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    owner: CompanyId,
    summary: &mut WaterInfrastructureSummary,
) {
    let Some(end) = water_aqueduct_other_end(map, coord, tile) else {
        return;
    };
    if !is_before_in_map_order(coord, end) || !company_owner_matches(tile, owner) {
        return;
    }
    let length = u32::try_from(bridge_line_tiles(coord, end).len()).unwrap_or(u32::MAX);
    add_water_pieces(summary, length.saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR));
}

fn add_water_station_tile(summary: &mut WaterInfrastructureSummary, tile: Tile, owner: CompanyId) {
    if !matches!(stop_kind_from_m6(tile.m6), StopKind::Dock | StopKind::Buoy)
        || water_class_from_m1(tile.m1) != WaterClass::Canal
        || !company_owner_matches(tile, owner)
    {
        return;
    }
    add_water_pieces(summary, 1);
}

#[must_use]
const fn owner_slot(raw: u8) -> u8 {
    raw & 0x1F
}

fn road_type_present(tile: Tile, road_tram_type: RoadTramType) -> Option<RoadType> {
    match road_tram_type {
        RoadTramType::Road => {
            let road_type = road_type_from_tile(&tile);
            (road_type.as_u8() != INVALID_ROADTYPE).then_some(road_type)
        }
        RoadTramType::Tram => tram_road_type_from_tile(&tile),
    }
}

fn road_owner_raw(tile: Tile, road_tram_type: RoadTramType) -> u8 {
    match road_tram_type {
        RoadTramType::Road => match tile.kind {
            // `GetRoadOwner` uses MAP1 for normal road tiles and MAP7 for
            // crossings, stations and tunnel/bridge ramps. Depots are the
            // exception in `AfterLoadCompanyStats`: they use MAP1 for both
            // layers because their tile owner is the depot owner.
            TileKind::Road if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) => tile.m7,
            TileKind::RoadDepot | TileKind::Road => tile.m1,
            _ => tile.m7,
        },
        // Native tram ownership is a four-bit owner in the high nibble of M3;
        // OWNER_TOWN (0xF) represents OWNER_NONE and never matches a company.
        RoadTramType::Tram => tile.m3 >> 4,
    }
}

fn road_owner_matches(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    tile: Tile,
    road_tram_type: RoadTramType,
    expected_owner: CompanyId,
) -> bool {
    if expected_owner.0 >= crate::company::MAX_COMPANIES {
        return false;
    }
    let raw = road_owner_raw(tile, road_tram_type);
    if tile.kind == TileKind::Station
        && owner_slot(tile.m1) == 0
        && owner_slot(tile.m7) == 0
        && tile.m3 >> 4 == 0
        && let Some(station) = station_at_tile(map, stations, coord)
        && matches!(
            station.stop_kind,
            StopKind::BusStop | StopKind::TruckStop | StopKind::RoadWaypoint
        )
    {
        // Runtime road stops historically did not write GetRoadOwner's MAP7
        // or M3 owner nibble. Their Station entity is the only authoritative
        // owner in that representation; imported saves retain the raw bytes
        // and take the native path above.
        return station.owner == expected_owner;
    }
    // Runtime bridges/tunnels created before their road metadata was added
    // kept the owner in MAP1. An explicit MAP7 owner always wins, including
    // OWNER_NONE/TOWN values from an imported save.
    if !matches!(
        tile.kind,
        TileKind::Road | TileKind::RoadDepot | TileKind::Station
    ) && owner_slot(tile.m7) == 0
        && owner_slot(tile.m1) != 0
    {
        return owner_slot(tile.m1) == owner_slot(expected_owner.0);
    }
    owner_slot(raw) == owner_slot(expected_owner.0)
}

fn road_layer_pieces(tile: Tile, road_tram_type: RoadTramType) -> u32 {
    if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) || (tile.m5 >> 6) & 0x03 == 2 {
        return ROAD_STOP_TRACKBIT_FACTOR;
    }
    match road_tram_type {
        RoadTramType::Road => (tile.m5 & 0x0F).count_ones(),
        RoadTramType::Tram => (tile.m3 & 0x0F).count_ones(),
    }
}

fn road_tunnel_end_for_count(map: &Map, start: TileCoord, tile: Tile) -> Option<TileCoord> {
    tunnel_end_for_count(map, start, tile, TileKind::RoadTunnel)
}

fn add_road_tunnel(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    tile: Tile,
    expected_owner: CompanyId,
    summary: &mut RoadInfrastructureSummary,
) {
    let Some(end) = road_tunnel_end_for_count(map, coord, tile) else {
        return;
    };
    if !is_before_in_map_order(coord, end)
        || !is_tunnel_entry(map, coord, tile, TileKind::RoadTunnel)
    {
        return;
    }
    let Some(length) = tunnel_span_len(coord, end, tile.m5 & 0x03) else {
        return;
    };
    let pieces = length
        .saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR)
        .saturating_mul(ROAD_STOP_TRACKBIT_FACTOR);
    for class in [RoadTramType::Road, RoadTramType::Tram] {
        if road_owner_matches(map, stations, coord, tile, class, expected_owner)
            && let Some(road_type) = road_type_present(tile, class)
        {
            add_road_pieces(summary, road_type, pieces);
        }
    }
}

fn add_road_bridge(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    tile: Tile,
    expected_owner: CompanyId,
    summary: &mut RoadInfrastructureSummary,
) {
    let Some(end) = crate::bridge_spec::road_bridge_other_end(map, coord) else {
        return;
    };
    if !is_before_in_map_order(coord, end) {
        return;
    }
    let length = u32::try_from(bridge_line_tiles(coord, end).len()).unwrap_or(u32::MAX);
    let pieces = length
        .saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR)
        .saturating_mul(ROAD_STOP_TRACKBIT_FACTOR);
    for class in [RoadTramType::Road, RoadTramType::Tram] {
        if road_owner_matches(map, stations, coord, tile, class, expected_owner)
            && let Some(road_type) = road_type_present(tile, class)
        {
            add_road_pieces(summary, road_type, pieces);
        }
    }
}

fn is_road_station_tile(tile: Tile) -> bool {
    tile.kind == TileKind::Station
        && matches!(
            station_type_from_m6(tile.m6),
            2 | 3 | crate::station::STATION_TYPE_ROAD_WAYPOINT
        )
}

/// Reconstruye la parte vial de `CompanyInfrastructure` sin consultar el
/// contador histórico de `Company`.
///
/// La variante sin estaciones sirve para mapas importados o consumidores que
/// sólo disponen del mapa. El cliente usa la variante con estaciones para
/// cubrir también las paradas creadas en runtime, cuya entidad conserva el
/// owner mientras se completa la codificación nativa de MAP7/M3.
#[must_use]
pub fn road_infrastructure_for_company(map: &Map, owner: CompanyId) -> RoadInfrastructureSummary {
    road_infrastructure_for_company_with_stations(map, &[], owner)
}

/// Variante que aporta el owner lógico de las paradas viales runtime.
#[must_use]
pub fn road_infrastructure_for_company_with_stations(
    map: &Map,
    stations: &[Station],
    owner: CompanyId,
) -> RoadInfrastructureSummary {
    let (width, height) = map.dimensions();
    let mut summary = RoadInfrastructureSummary::default();
    for y in 0..height {
        let Ok(y) = i32::try_from(y) else {
            continue;
        };
        for x in 0..width {
            let Ok(x) = i32::try_from(x) else {
                continue;
            };
            let coord = TileCoord::new(x, y);
            let Some(tile) = map.get(coord) else {
                continue;
            };
            match tile.kind {
                TileKind::Road => {
                    for class in [RoadTramType::Road, RoadTramType::Tram] {
                        if road_owner_matches(map, stations, coord, tile, class, owner)
                            && let Some(road_type) = road_type_present(tile, class)
                        {
                            let pieces = road_layer_pieces(tile, class);
                            if pieces != 0 {
                                add_road_pieces(&mut summary, road_type, pieces);
                            }
                        }
                    }
                }
                TileKind::RoadDepot => {
                    for class in [RoadTramType::Road, RoadTramType::Tram] {
                        if road_owner_matches(map, stations, coord, tile, class, owner)
                            && let Some(road_type) = road_type_present(tile, class)
                        {
                            add_road_pieces(&mut summary, road_type, ROAD_STOP_TRACKBIT_FACTOR);
                        }
                    }
                }
                TileKind::RoadTunnel => {
                    add_road_tunnel(map, stations, coord, tile, owner, &mut summary);
                }
                TileKind::RoadBridge => {
                    add_road_bridge(map, stations, coord, tile, owner, &mut summary);
                }
                TileKind::Station if is_road_station_tile(tile) => {
                    for class in [RoadTramType::Road, RoadTramType::Tram] {
                        if road_owner_matches(map, stations, coord, tile, class, owner)
                            && let Some(road_type) = road_type_present(tile, class)
                        {
                            add_road_pieces(&mut summary, road_type, ROAD_STOP_TRACKBIT_FACTOR);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    summary
}

/// Reconstruye la parte acuática de `CompanyInfrastructure` a partir del mapa.
///
/// La superficie de mar/río sólo es terreno neutral. El contador nativo se
/// incrementa para canales propios, estaciones/boyas apoyadas en canal,
/// depósitos, esclusas y acueductos; un acueducto se cuenta una sola vez desde
/// su rampa anterior en el orden lineal del mapa.
#[must_use]
pub fn water_infrastructure_for_company(map: &Map, owner: CompanyId) -> WaterInfrastructureSummary {
    let (width, height) = map.dimensions();
    let mut summary = WaterInfrastructureSummary::default();
    for y in 0..height {
        let Ok(y) = i32::try_from(y) else {
            continue;
        };
        for x in 0..width {
            let Ok(x) = i32::try_from(x) else {
                continue;
            };
            let coord = TileCoord::new(x, y);
            let Some(tile) = map.get(coord) else {
                continue;
            };

            if is_water_aqueduct_endpoint(tile) {
                add_water_aqueduct(map, coord, tile, owner, &mut summary);
                continue;
            }

            if is_map_object_tile(tile.mapt) {
                if water_class_from_m1(tile.m1) == WaterClass::Canal
                    && company_owner_matches(tile, owner)
                {
                    add_water_pieces(&mut summary, 1);
                }
                continue;
            }

            match tile.kind {
                TileKind::Water | TileKind::ShipDepot => {
                    add_water_tile(&mut summary, tile, owner);
                }
                TileKind::Station => {
                    add_water_station_tile(&mut summary, tile, owner);
                }
                _ => {}
            }
        }
    }
    summary
}

/// Reconstruye las partes de estaciones y aeropuertos de
/// `CompanyInfrastructure`.
///
/// El contador nativo recorre las teselas `MP_STATION` para stations, pero
/// obtiene airports desde las entidades `BaseStation` con facilidad aérea.
/// Mantener ambas fuentes evita multiplicar un aeropuerto por cada pieza de su
/// huella y conserva Oil Rig como facilidad aérea cuando corresponde.
#[must_use]
pub fn station_infrastructure_for_company(
    map: &Map,
    stations: &[Station],
    owner: CompanyId,
) -> StationInfrastructureSummary {
    let mut summary = StationInfrastructureSummary {
        airports: u32::try_from(
            stations
                .iter()
                .filter(|station| station.owner == owner && station.has_airport_facility())
                .count(),
        )
        .unwrap_or(u32::MAX),
        ..StationInfrastructureSummary::default()
    };
    if owner.0 >= crate::company::MAX_COMPANIES {
        summary.airports = 0;
        return summary;
    }

    let (width, height) = map.dimensions();
    for y in 0..height {
        let Ok(y) = i32::try_from(y) else {
            continue;
        };
        for x in 0..width {
            let Ok(x) = i32::try_from(x) else {
                continue;
            };
            let Some(tile) = map.get(TileCoord::new(x, y)) else {
                continue;
            };
            if tile.mapt >> 4 != OTTD_TILETYPE_STATION
                || !company_owner_matches(tile, owner)
                || matches!(
                    station_type_from_m6(tile.m6),
                    STATION_TYPE_AIRPORT | STATION_TYPE_BUOY
                )
            {
                continue;
            }
            summary.stations = summary.stations.saturating_add(1);
        }
    }
    summary
}

/// Reconstruye la infraestructura ferroviaria de una compañía a partir del
/// mapa actual.
///
/// El recorrido cubre vía plana, señales, depósitos, estaciones/waypoints,
/// cruces a nivel y túneles/puentes. La parte de carretera, agua y aeropuertos
/// usa resúmenes separados con sus contratos de ownership propios.
#[must_use]
pub fn rail_infrastructure_for_company(map: &Map, owner: CompanyId) -> RailInfrastructureSummary {
    let (width, height) = map.dimensions();
    let mut summary = RailInfrastructureSummary::default();
    for y in 0..height {
        let Ok(y) = i32::try_from(y) else {
            continue;
        };
        for x in 0..width {
            let Ok(x) = i32::try_from(x) else {
                continue;
            };
            let coord = TileCoord::new(x, y);
            let Some(tile) = map.get(coord) else {
                continue;
            };
            if !tile_owned_by(tile, owner) {
                continue;
            }
            match tile.kind {
                TileKind::Rail => {
                    add_rail_pieces(&mut summary, tile, plain_rail_pieces(tile));
                    if rail_tile_is_signals(tile.m5) {
                        summary.signals = summary
                            .signals
                            .saturating_add(rail_signal_present_mask(tile.m3).count_ones());
                    }
                }
                TileKind::RailDepot => add_rail_pieces(&mut summary, tile, 1),
                TileKind::RailTunnel => add_rail_tunnel(map, coord, tile, &mut summary),
                TileKind::RailBridge => add_rail_bridge(map, coord, tile, &mut summary),
                TileKind::Station
                    if is_rail_station_type(station_type_from_m6(tile.m6)) && tile.m3 & 1 == 0 =>
                {
                    add_rail_pieces(&mut summary, tile, 1);
                }
                TileKind::Road if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) => {
                    add_rail_pieces(&mut summary, tile, LEVELCROSSING_TRACKBIT_FACTOR);
                }
                _ => {}
            }
        }
    }
    summary
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::Map;

    fn tile(kind: TileKind, mapt: u8, m5: u8, owner: CompanyId, m3: u8, m6: u8, m8: u16) -> Tile {
        Tile {
            height: 1,
            kind,
            mapt,
            m5,
            m1: owner.0,
            m6,
            m8,
            m3,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn counts_owned_rail_pieces_by_type_and_signals() {
        let mut map = Map::new_flat(8, 4, 1);
        map.set_tile(
            TileCoord::new(1, 1),
            tile(TileKind::Rail, 0x10, 0x03, CompanyId::PLAYER, 0, 0, 0),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(2, 1),
            tile(
                TileKind::Rail,
                0x10,
                (1 << 6) | 0x03,
                CompanyId::PLAYER,
                0x30,
                0,
                0,
            ),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(3, 1),
            tile(TileKind::RailDepot, 0x10, 0, CompanyId::PLAYER, 0, 0, 2),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(4, 1),
            tile(TileKind::Station, 0x50, 0, CompanyId::PLAYER, 0, 0, 3),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(5, 1),
            tile(TileKind::Station, 0x50, 0, CompanyId::PLAYER, 1, 0, 1),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(6, 1),
            tile(TileKind::Road, 0x20, 1 << 6, CompanyId::PLAYER, 0, 0, 1),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(1, 2),
            tile(TileKind::Rail, 0x10, 0x01, CompanyId(1), 0, 0, 0),
        )
        .unwrap();

        let summary = rail_infrastructure_for_company(&map, CompanyId::PLAYER);
        assert_eq!(summary.rail_type_count(RailType::Rail), 5);
        assert_eq!(summary.rail_type_count(RailType::Electric), 2);
        assert_eq!(summary.rail_type_count(RailType::Monorail), 1);
        assert_eq!(summary.rail_type_count(RailType::Maglev), 1);
        assert_eq!(summary.rail_total(), 9);
        assert_eq!(summary.signals, 2);
    }

    #[test]
    fn counts_tunnel_and_bridge_once_with_native_factor() {
        let mut map = Map::new_flat(8, 4, 1);
        for x in 1..=5 {
            map.set_tile(
                TileCoord::new(x, 1),
                tile(
                    TileKind::RailTunnel,
                    0x90,
                    if x == 1 {
                        2
                    } else if x == 5 {
                        0
                    } else {
                        0
                    },
                    CompanyId::PLAYER,
                    0,
                    0,
                    1,
                ),
            )
            .unwrap();
        }
        map.set_tile(
            TileCoord::new(1, 2),
            tile(TileKind::RailBridge, 0x90, 0x82, CompanyId::PLAYER, 0, 0, 1),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(5, 2),
            tile(TileKind::RailBridge, 0x90, 0x80, CompanyId::PLAYER, 0, 0, 1),
        )
        .unwrap();

        let summary = rail_infrastructure_for_company(&map, CompanyId::PLAYER);
        assert_eq!(summary.rail_type_count(RailType::Electric), 40);
    }

    #[test]
    fn counts_owned_road_layers_and_special_tiles() {
        let mut map = Map::new_flat(10, 8, 1);
        // Normal road: two road bits and two tram bits on the same tile.
        map.set_tile(
            TileCoord::new(1, 1),
            tile(
                TileKind::Road,
                0x20,
                0x03,
                CompanyId::PLAYER,
                0x05,
                0,
                1 << 6,
            ),
        )
        .unwrap();

        // In a crossing the rail owner is MAP1 and the road owner is MAP7.
        map.set_tile(
            TileCoord::new(2, 1),
            Tile {
                m7: CompanyId::PLAYER.0,
                m5: 1 << 6,
                ..tile(TileKind::Road, 0x20, 1 << 6, CompanyId(1), 0, 0, 0)
            },
        )
        .unwrap();

        // A depot always contributes two road bits.
        map.set_tile(
            TileCoord::new(3, 1),
            tile(
                TileKind::RoadDepot,
                0x20,
                2 << 6,
                CompanyId::PLAYER,
                0,
                0,
                0,
            ),
        )
        .unwrap();

        // Station road stops contribute two bits for each present road type.
        map.set_tile(
            TileCoord::new(4, 1),
            tile(
                TileKind::Station,
                0x50,
                0,
                CompanyId::PLAYER,
                0x02,
                3 << 3,
                1 << 6,
            ),
        )
        .unwrap();

        let summary = road_infrastructure_for_company(&map, CompanyId::PLAYER);
        assert_eq!(summary.road_type_count(RoadType::Road), 8);
        assert_eq!(summary.road_type_count(RoadType::Tram), 4);
        assert_eq!(summary.road_total(), 12);
    }

    #[test]
    fn counts_road_tunnel_and_bridge_once_with_native_factor() {
        let mut map = Map::new_flat(10, 8, 1);
        for x in 1..=5 {
            map.set_tile(
                TileCoord::new(x, 1),
                tile(
                    TileKind::RoadTunnel,
                    0x90,
                    if x == 1 {
                        2
                    } else if x == 5 {
                        0
                    } else {
                        0
                    },
                    CompanyId::PLAYER,
                    0,
                    0,
                    0,
                ),
            )
            .unwrap();
        }
        for (y, m5) in [(1, 0x85), (5, 0x87)] {
            map.set_tile(
                TileCoord::new(7, y),
                tile(TileKind::RoadBridge, 0x90, m5, CompanyId::PLAYER, 0, 0, 0),
            )
            .unwrap();
        }

        let summary = road_infrastructure_for_company(&map, CompanyId::PLAYER);
        assert_eq!(summary.road_type_count(RoadType::Road), 80);
    }

    #[test]
    fn runtime_road_stop_owner_comes_from_station_entity_when_map_is_legacy() {
        let mut map = Map::new_flat(4, 4, 1);
        let coord = TileCoord::new(1, 1);
        map.set_tile(
            coord,
            tile(
                TileKind::Station,
                0x50,
                0,
                CompanyId::PLAYER,
                0x02,
                3 << 3,
                0,
            ),
        )
        .unwrap();
        let station = Station::new_with_kind(coord, StopKind::BusStop);
        let summary = road_infrastructure_for_company_with_stations(
            &map,
            std::slice::from_ref(&station),
            station.owner,
        );
        assert_eq!(summary.road_total(), 2);
    }

    #[test]
    fn counts_station_tiles_and_airports_by_native_contract() {
        let mut map = Map::new_flat(12, 8, 1);
        let active = CompanyId::PLAYER;
        let rival = CompanyId(1);
        let station_tile =
            |owner, station_type| tile(TileKind::Station, 0x50, 0, owner, 0, station_type << 3, 0);

        map.set_tile(TileCoord::new(1, 1), station_tile(active, 0))
            .unwrap();
        map.set_tile(
            TileCoord::new(2, 1),
            station_tile(active, crate::station::STATION_TYPE_DOCK),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(3, 1),
            station_tile(active, crate::station::STATION_TYPE_DOCK),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(4, 1),
            station_tile(active, STATION_TYPE_BUOY),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(5, 1),
            tile(
                TileKind::Airport,
                0x50,
                0,
                active,
                0,
                STATION_TYPE_AIRPORT << 3,
                0,
            ),
        )
        .unwrap();
        map.set_tile(TileCoord::new(6, 1), station_tile(rival, 0))
            .unwrap();

        let mut airport = Station::new_with_kind(TileCoord::new(5, 2), StopKind::Airport);
        airport.owner = active;
        let mut oilrig = Station::new_with_kind(TileCoord::new(8, 2), StopKind::OilRig);
        oilrig.owner = rival;
        let stations = [airport, oilrig];

        let active_summary = station_infrastructure_for_company(&map, &stations, active);
        assert_eq!(active_summary.station_total(), 3);
        assert_eq!(active_summary.airport_total(), 1);

        let rival_summary = station_infrastructure_for_company(&map, &stations, rival);
        assert_eq!(rival_summary.station_total(), 1);
        assert_eq!(rival_summary.airport_total(), 1);
    }

    #[test]
    fn counts_owned_water_infrastructure_by_native_tile_contract() {
        let mut map = Map::new_flat(16, 8, 1);
        let active = CompanyId::PLAYER;
        let rival = CompanyId(1);
        let canal = |mut tile: Tile, owner: CompanyId| {
            tile.m1 = crate::map::set_water_class_m1(owner.0, WaterClass::Canal);
            tile
        };

        let plain_canal = canal(tile(TileKind::Water, 0x60, 0, active, 0, 0, 0), active);
        map.set_tile(TileCoord::new(1, 1), plain_canal).unwrap();
        let sea_depot = tile(TileKind::ShipDepot, 0x60, 0x30, active, 0, 0, 0);
        map.set_tile(TileCoord::new(3, 1), sea_depot).unwrap();
        let canal_depot = canal(
            tile(TileKind::ShipDepot, 0x60, 0x31, active, 0, 0, 0),
            active,
        );
        map.set_tile(TileCoord::new(4, 1), canal_depot).unwrap();

        // Middle = part 0; the lower and upper sections retain their own
        // canal ownership and count as ordinary canal pieces.
        for (coord, m5) in [
            (TileCoord::new(1, 3), 0x26),
            (TileCoord::new(2, 3), 0x22),
            (TileCoord::new(3, 3), 0x2A),
        ] {
            map.set_tile(
                coord,
                canal(tile(TileKind::Water, 0x60, m5, active, 0, 0, 0), active),
            )
            .unwrap();
        }

        let dock = canal(
            tile(
                TileKind::Station,
                0x50,
                4,
                active,
                0,
                crate::station::STATION_TYPE_DOCK << 3,
                0,
            ),
            active,
        );
        map.set_tile(TileCoord::new(5, 1), dock).unwrap();
        let buoy = canal(
            tile(
                TileKind::Station,
                0x50,
                0,
                active,
                0,
                crate::station::STATION_TYPE_BUOY << 3,
                0,
            ),
            active,
        );
        map.set_tile(TileCoord::new(6, 1), buoy).unwrap();

        // Imported MP_OBJECT tiles can be semantically Unknown while MAPT is
        // still authoritative for the water class.
        let object = canal(
            tile(TileKind::Unknown(10), 0xA0, 0, active, 0, 0, 0),
            active,
        );
        map.set_tile(TileCoord::new(7, 1), object).unwrap();

        let rival_canal = canal(tile(TileKind::Water, 0x60, 0, rival, 0, 0, 0), rival);
        map.set_tile(TileCoord::new(10, 1), rival_canal).unwrap();

        let summary = water_infrastructure_for_company(&map, active);
        // plain canal (1) + depot sections (2 + 3) + lock (6 + 1 + 1) +
        // dock + buoy + object.
        assert_eq!(summary.water_total(), 17);
        assert_eq!(
            water_infrastructure_for_company(&map, rival).water_total(),
            1
        );
    }

    #[test]
    fn counts_an_owned_aqueduct_once_with_structural_factor() {
        let mut map = Map::new_flat(12, 8, 0);
        let active = CompanyId::PLAYER;
        let west = TileCoord::new(2, 3);
        let east = TileCoord::new(6, 3);
        let mut west_tile = tile(TileKind::Water, 0x90, 0x8A, active, 0, 0, 0);
        west_tile.m1 = crate::map::set_water_class_m1(active.0, WaterClass::Sea);
        let mut east_tile = tile(TileKind::Water, 0x90, 0x88, active, 0, 0, 0);
        east_tile.m1 = crate::map::set_water_class_m1(active.0, WaterClass::Sea);
        map.set_tile(west, west_tile).unwrap();
        map.set_tile(east, east_tile).unwrap();

        for x in 3..6 {
            let mut middle = map.get(TileCoord::new(x, 3)).unwrap();
            middle.mapt = 0x64;
            middle.m1 =
                crate::map::set_water_class_m1(crate::company::OWNER_WATER_M1, WaterClass::Sea);
            map.set_tile(TileCoord::new(x, 3), middle).unwrap();
        }

        let summary = water_infrastructure_for_company(&map, active);
        assert_eq!(summary.water_total(), 5 * TUNNELBRIDGE_TRACKBIT_FACTOR);
    }
}
