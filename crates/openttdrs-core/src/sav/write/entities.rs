//! Serialización de entidades: estaciones, ciudades, industrias.

use super::super::SavError;
use super::super::SavPersistentStorage;
use super::super::chunks::CH_TABLE;
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use crate::cargo::CargoType;
use crate::cargo_packet::CargoPacket;
use crate::game_state::GameState;
use crate::industry::{Industry, IndustryKind, IndustrySpec};
use crate::map::{Map, TileCoord, coord_to_linear_index};
use crate::station::{RoadStopTileState, Station, StopKind};
use crate::town::Town;
use std::collections::{BTreeMap, HashMap};

/// Bits `FACIL_*` al escribir `STNN` (alineados con el import).
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;
const FACIL_AIRPORT: u8 = 0x08;
const FACIL_DOCK: u8 = 0x10;
const FACIL_WAYPOINT: u8 = 0x80;

/// `INVALID_TILE` en `OpenTTD`.
const INVALID_TILE: u32 = 0xFFFF_FFFF;

/// `NUM_CARGO` (`OpenTTD` moderno).
const NUM_CARGO: u32 = 64;

/// `STR_SV_STNAME` — plantilla de nombre generado.
const STR_SV_STNAME: u16 = 0x6006;

/// `VEH_INVALID`.
const VEH_INVALID: u8 = 0xFF;

/// Identidad nativa `(GRFID, localidx)` de una spec de road stop.
type RoadStopSpecIdentity = (u32, u16);

/// IDs asignados a los storages PSA de industrias, estaciones y pueblos.
///
/// El tercer componente conserva la relación `GRFID → storage_id` de cada
/// pueblo. Los refs importados sin fila `PSAC` siguen viviendo en el mapa
/// histórico de `GameState` y no se compactan.
type PersistentStorageIds = (Vec<Option<u32>>, Vec<Option<u32>>, Vec<HashMap<u32, u32>>);

/// Registro serializable del pool `CAPA`.
///
/// El tipo de carga no forma parte del registro nativo: `OpenTTD` lo obtiene de
/// la entrada `STNN.goods` o de `VEHS.common.cargo_type`. Por eso el exportador
/// conserva aquí sólo los campos propios de `CargoPacket` y mantiene los
/// enlaces por separado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CargoPacketWire {
    pub source: u16,
    pub source_xy: u32,
    pub loaded_at_xy: u16,
    pub count: u16,
    pub periods_in_transit: u16,
    pub feeder_share: i64,
    pub source_type: u8,
    pub source_id: u16,
    pub travelled_x: i16,
    pub travelled_y: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StationCargoGroupWire {
    pub next_hop: u16,
    pub packet_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StationCargoWire {
    pub cargo_slot: u8,
    pub groups: Vec<StationCargoGroupWire>,
    pub reserved: u32,
}

/// Referencias CAPA que se escribirán junto con `STNN` y `VEHS`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CargoPacketExport {
    pub packets: Vec<CargoPacketWire>,
    pub station_refs: HashMap<u32, Vec<StationCargoWire>>,
    pub vehicle_refs: HashMap<u32, Vec<u32>>,
}

/// Entrada que se escribe en `STNN.roadstoptiledata` y que también determina
/// los seis bits bajos de `MAP8` para esa tesela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RoadStopTileExport {
    tile: TileCoord,
    spec_index: u8,
    random_bits: u8,
    animation_frame: u8,
}

fn facilities_for_stop(kind: StopKind) -> u8 {
    match kind {
        StopKind::RailStation => FACIL_TRAIN,
        StopKind::TruckStop => FACIL_TRUCK_STOP,
        StopKind::BusStop => FACIL_BUS_STOP,
        StopKind::Dock | StopKind::Buoy => FACIL_DOCK,
        StopKind::Airport => FACIL_AIRPORT,
        StopKind::RailWaypoint => FACIL_WAYPOINT | FACIL_TRAIN,
        StopKind::RoadWaypoint => FACIL_WAYPOINT | FACIL_BUS_STOP | FACIL_TRUCK_STOP,
    }
}

fn is_waypoint(facilities: u8) -> bool {
    facilities & FACIL_WAYPOINT != 0
}

fn station_id_for_pos(state: &GameState, pos: TileCoord) -> Option<u16> {
    state
        .stations
        .iter()
        .find(|station| station.pos == pos)
        .and_then(|station| station.ottd_station_id)
        .and_then(|id| u16::try_from(id).ok())
        .or_else(|| {
            state
                .stations
                .iter()
                .position(|station| station.pos == pos)
                .and_then(|index| u16::try_from(index).ok())
        })
}

fn station_id_for_index(state: &GameState, index: usize) -> Option<u16> {
    state
        .stations
        .get(index)
        .and_then(|station| station.ottd_station_id)
        .and_then(|id| u16::try_from(id).ok())
        .or_else(|| u16::try_from(index).ok())
}

fn cargo_slot_for_climate(climate: crate::Climate, cargo: CargoType) -> Option<u8> {
    crate::cargo::CargoType::for_climate(climate)
        .iter()
        .position(|candidate| *candidate == cargo)
        .and_then(|slot| u8::try_from(slot).ok())
}

fn source_for_packet(state: &GameState, packet: &CargoPacket) -> (u8, u16) {
    if let Some(industry) = state
        .industries
        .iter()
        .find(|industry| industry.pos == packet.source || industry.tiles.contains(&packet.source))
    {
        return (0, industry.instance_id);
    }
    if let Some(town) = state.towns.iter().find(|town| town.pos == packet.source) {
        return (1, u16::try_from(town.id).unwrap_or(u16::MAX));
    }
    (0, u16::MAX)
}

fn packet_wire_for(state: &GameState, map_w: u32, packet: &CargoPacket) -> CargoPacketWire {
    let source_xy = packet
        .source_xy
        .or(Some(packet.source))
        .and_then(|pos| coord_to_linear_index(pos, map_w))
        .unwrap_or(INVALID_TILE);
    let source = packet
        .first_station
        .and_then(|pos| station_id_for_pos(state, pos))
        .unwrap_or(u16::MAX);
    let loaded_at_xy = packet
        .next_hop
        .and_then(|pos| station_id_for_pos(state, pos))
        .unwrap_or(u16::MAX);
    let (source_type, source_id) = source_for_packet(state, packet);
    CargoPacketWire {
        source,
        source_xy,
        loaded_at_xy,
        count: packet.count,
        periods_in_transit: packet.periods_in_transit,
        feeder_share: packet.feeder_share,
        source_type,
        source_id,
        travelled_x: packet.travelled.x,
        travelled_y: packet.travelled.y,
    }
}

fn push_cargo_packet(
    export: &mut CargoPacketExport,
    state: &GameState,
    map_w: u32,
    packet: &CargoPacket,
) -> Option<u32> {
    let packet_id = u32::try_from(export.packets.len()).ok()?;
    export.packets.push(packet_wire_for(state, map_w, packet));
    Some(packet_id)
}

/// Reúne una sola numeración CAPA para packets en estaciones y vehículos.
///
/// El orden es estable (estaciones, luego vehículos, y FIFO dentro de cada
/// lista), de modo que los `REF_CARGO_PACKET` son reproducibles entre saves.
pub(crate) fn cargo_packet_export(state: &GameState, map_w: u32) -> CargoPacketExport {
    let mut export = CargoPacketExport::default();

    for (station_index, source_station) in state.stations.iter().enumerate() {
        let Some(station_id) = station_id_for_index(state, station_index).map(u32::from) else {
            continue;
        };
        // `ensure_packets_from_stock` también cubre estados JSON antiguos que
        // aún sólo tenían el balance agregado de la estación.
        let mut station = source_station.clone();
        station.ensure_packets_from_stock();
        let mut groups: BTreeMap<(u8, u16), Vec<u32>> = BTreeMap::new();
        for packet in station.cargo_packets.packets() {
            let Some(cargo_slot) = cargo_slot_for_climate(state.climate, packet.cargo) else {
                continue;
            };
            let Some(packet_id) = push_cargo_packet(&mut export, state, map_w, packet) else {
                continue;
            };
            let next_hop = packet
                .next_hop
                .and_then(|pos| station_id_for_pos(state, pos))
                .unwrap_or(u16::MAX);
            groups
                .entry((cargo_slot, next_hop))
                .or_default()
                .push(packet_id);
        }

        let mut by_slot: BTreeMap<u8, Vec<StationCargoGroupWire>> = BTreeMap::new();
        for ((cargo_slot, next_hop), packet_ids) in groups {
            by_slot
                .entry(cargo_slot)
                .or_default()
                .push(StationCargoGroupWire {
                    next_hop,
                    packet_ids,
                });
        }
        let mut remaining_reserved = station.cargo_packets.reserved;
        let mut refs = Vec::with_capacity(by_slot.len());
        for (cargo_slot, groups) in by_slot {
            let total = groups
                .iter()
                .flat_map(|group| group.packet_ids.iter())
                .filter_map(|id| export.packets.get(*id as usize))
                .map(|packet| u32::from(packet.count))
                .fold(0, u32::saturating_add);
            let reserved = remaining_reserved.min(total);
            remaining_reserved = remaining_reserved.saturating_sub(reserved);
            refs.push(StationCargoWire {
                cargo_slot,
                groups,
                reserved,
            });
        }
        if !refs.is_empty() {
            export.station_refs.insert(station_id, refs);
        }
    }

    for vehicle in &state.vehicles {
        let mut refs = Vec::new();
        for packet in &vehicle.cargo_packets.packets {
            if let Some(packet_id) = push_cargo_packet(&mut export, state, map_w, packet) {
                refs.push(packet_id);
            }
        }
        if !refs.is_empty() {
            export.vehicle_refs.insert(vehicle.id, refs);
        }
    }
    export
}

fn road_stop_spec_identity(
    state: &GameState,
    tile_state: &RoadStopTileState,
) -> Option<RoadStopSpecIdentity> {
    if let (Some(grfid), Some(local_id)) = (tile_state.saved_grfid, tile_state.saved_local_id) {
        return Some((grfid, local_id));
    }
    tile_state.spec.and_then(|spec_id| {
        state
            .road_stop_spec_catalog
            .iter()
            .find(|spec| spec.id == spec_id && spec.from_newgrf)
            .map(|spec| (spec.grfid, u16::from(spec.newgrf_local_id)))
    })
}

fn station_road_stop_spec_identity(
    state: &GameState,
    station: &Station,
    spec_id: Option<u16>,
) -> Option<RoadStopSpecIdentity> {
    spec_id.and_then(|id| {
        state
            .road_stop_spec_catalog
            .iter()
            .find(|spec| spec.id == id && spec.from_newgrf)
            .map(|spec| (spec.grfid, u16::from(spec.newgrf_local_id)))
            .or_else(|| {
                // Saves imported without an installed GRF retain the stable
                // identity on the legacy anchor when available.
                let anchor = station.road_stop_tile_state(station.pos)?;
                Some((anchor.saved_grfid?, anchor.saved_local_id?))
            })
    })
}

/// Reúne las specs custom de una estación y su estado por tesela en el orden
/// estable que exige `MAP8`/`roadstopspeclist`.
fn road_stop_export_data(
    state: &GameState,
    station: &Station,
    map_w: u32,
) -> Result<(Vec<RoadStopSpecIdentity>, Vec<RoadStopTileExport>), SavError> {
    let mut candidates: Vec<(TileCoord, RoadStopSpecIdentity, u8, u8)> = Vec::new();
    if !station.road_stop_tile_states.is_empty() {
        for (tile, tile_state) in &station.road_stop_tile_states {
            let Some(identity) = road_stop_spec_identity(state, tile_state) else {
                continue;
            };
            if coord_to_linear_index(*tile, map_w).is_some() {
                candidates.push((
                    *tile,
                    identity,
                    tile_state.random_bits,
                    tile_state.animation_frame,
                ));
            }
        }
    }
    if candidates.is_empty()
        && let Some(identity) =
            station_road_stop_spec_identity(state, station, station.road_stop_spec)
    {
        let mut tiles = Vec::with_capacity(station.joined_tiles.len() + 1);
        tiles.push(station.pos);
        tiles.extend(station.joined_tiles.iter().copied());
        for tile in tiles {
            if coord_to_linear_index(tile, map_w).is_some() {
                candidates.push((
                    tile,
                    identity,
                    station.road_stop_newgrf_random_bits,
                    station.road_stop_animation_frame,
                ));
            }
        }
    }

    candidates.sort_unstable_by_key(|(tile, ..)| *tile);
    candidates.dedup_by_key(|(tile, ..)| *tile);

    if candidates.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut specs = vec![(0, 0)];
    for (_, identity, ..) in &candidates {
        if !specs.contains(identity) {
            if specs.len() >= 64 {
                return Err(SavError::ValueOutOfRange {
                    field: "roadstopspeclist",
                    value: u32::try_from(specs.len() + 1).unwrap_or(u32::MAX),
                });
            }
            specs.push(*identity);
        }
    }

    let tiles = candidates
        .into_iter()
        .filter_map(|(tile, identity, random_bits, animation_frame)| {
            let spec_index = specs
                .iter()
                .position(|candidate| *candidate == identity)
                .and_then(|index| u8::try_from(index).ok())?;
            Some(RoadStopTileExport {
                tile,
                spec_index,
                random_bits,
                animation_frame,
            })
        })
        .collect();
    Ok((specs, tiles))
}

fn write_road_stop_data(
    buf: &mut Vec<u8>,
    state: &GameState,
    station: &Station,
    map_w: u32,
) -> Result<(), SavError> {
    let (specs, tiles) = road_stop_export_data(state, station, map_w)?;
    write_gamma(u32::try_from(specs.len()).unwrap_or(u32::MAX), buf)?;
    for (grfid, localidx) in specs {
        buf.extend_from_slice(&grfid.to_be_bytes());
        buf.extend_from_slice(&localidx.to_be_bytes());
    }
    write_gamma(u32::try_from(tiles.len()).unwrap_or(u32::MAX), buf)?;
    for entry in tiles {
        let tile_idx = coord_to_linear_index(entry.tile, map_w).unwrap_or(0);
        buf.extend_from_slice(&tile_idx.to_be_bytes());
        buf.push(entry.random_bits);
        buf.push(entry.animation_frame);
    }
    Ok(())
}

/// Clona el mapa de salida y escribe el índice de spec de cada road stop en
/// `MAP8`, preservando los bits altos usados por otros tipos de tesela.
pub(super) fn map_with_road_stop_indices(state: &GameState, map_w: u32) -> Result<Map, SavError> {
    let mut map = state.map.clone();
    for station in &state.stations {
        let (_, tiles) = road_stop_export_data(state, station, map_w)?;
        for entry in tiles {
            let Some(mut tile) = map.get(entry.tile) else {
                continue;
            };
            tile.m8 = (tile.m8 & !0x3F) | u16::from(entry.spec_index);
            map.set_tile(entry.tile, tile)
                .map_err(|error| SavError::BadFormat(format!("MAP8 road stop: {error:?}")))?;
        }
    }
    Ok(map)
}

fn append_field(header: &mut Vec<u8>, ftype: u8, name: &str) -> Result<(), SavError> {
    header.push(ftype);
    write_str(name, header)
}

/// Header `CITY` moderno (SLV 355), incluidos los structs anidados de
/// historiales de carga. El orden coincide con `_town_desc` de `OpenTTD`.
pub(super) fn append_city_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 6, "xy")?;
    append_field(header, 6, "townnamegrfid")?;
    append_field(header, 4, "townnametype")?;
    append_field(header, 6, "townnameparts")?;
    append_field(header, 0x0A | 0x10, "name")?;
    append_field(header, 2, "flags")?;
    append_field(header, 4, "statues")?;
    append_field(header, 4, "have_ratings")?;
    append_field(header, 3 | 0x10, "ratings")?;
    append_field(header, 1 | 0x10, "unwanted")?;
    append_field(header, 6 | 0x10, "goal")?;
    append_field(header, 0x0A | 0x10, "text")?;
    append_field(header, 4, "time_until_rebuild")?;
    append_field(header, 4, "grow_counter")?;
    append_field(header, 4, "growth_rate")?;
    append_field(header, 2, "fund_buildings_months")?;
    append_field(header, 2, "road_build_months")?;
    append_field(header, 2, "exclusivity")?;
    append_field(header, 2, "exclusive_counter")?;
    append_field(header, 1, "larger_town")?;
    append_field(header, 2, "layout")?;
    append_field(header, 8, "valid_history")?;
    append_field(header, 0x16, "psa_list")?;
    append_field(header, 0x1B, "supplied")?;
    append_field(header, 0x1B, "received")?;
    header.push(0);

    // SlTownSupplied.
    append_field(header, 2, "cargo")?;
    append_field(header, 0x1B, "history")?;
    header.push(0);

    // SlTownSuppliedHistory.
    append_field(header, 6, "production")?;
    append_field(header, 6, "transported")?;
    header.push(0);

    // SlTownReceived.
    append_field(header, 4, "old_max")?;
    append_field(header, 4, "new_max")?;
    append_field(header, 4, "old_act")?;
    append_field(header, 4, "new_act")?;
    header.push(0);
    Ok(())
}

/// Header `STNN` moderno (SLV ≥ 340): SAVEBYTE + structs anidados.
///
/// Orden alineado con `station_sl.cpp` / save 15.3 (ver fixtures `*_15_3.sav`).
fn append_stnn_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    // Top: facilities (SAVEBYTE→U8), normal, waypoint, speclist*, roadstoptiledata.
    append_field(header, 2, "facilities")?; // SLE_FILE_U8
    append_field(header, 0x1B, "normal")?; // STRUCT | HAS_LENGTH
    append_field(header, 0x1B, "waypoint")?;
    append_field(header, 0x1B, "speclist")?;
    append_field(header, 0x1B, "roadstopspeclist")?;
    append_field(header, 0x1B, "roadstoptiledata")?;
    header.push(0);

    // Nest: SlStationNormal
    append_field(header, 0x1B, "base")?;
    append_field(header, 6, "train_station.tile")?;
    append_field(header, 2, "train_station.w")?;
    append_field(header, 2, "train_station.h")?;
    append_field(header, 6, "bus_stops")?; // REF → U32
    append_field(header, 6, "truck_stops")?;
    append_field(header, 6, "ship_station.tile")?;
    append_field(header, 2, "ship_station.w")?;
    append_field(header, 2, "ship_station.h")?;
    append_field(header, 6, "docking_station.tile")?;
    append_field(header, 2, "docking_station.w")?;
    append_field(header, 2, "docking_station.h")?;
    append_field(header, 6, "airport.tile")?;
    append_field(header, 2, "airport.w")?;
    append_field(header, 2, "airport.h")?;
    append_field(header, 2, "airport.type")?;
    append_field(header, 2, "airport.layout")?;
    append_field(header, 8, "airport.flags")?;
    append_field(header, 2, "airport.rotation")?;
    append_field(header, 6, "airport.psa")?;
    append_field(header, 2, "indtype")?;
    append_field(header, 2, "time_since_load")?;
    append_field(header, 2, "time_since_unload")?;
    append_field(header, 2, "last_vehicle_type")?;
    append_field(header, 2, "had_vehicle_of_type")?;
    append_field(header, 0x16, "loading_vehicles")?; // REF U32 | HAS_LENGTH
    append_field(header, 8, "always_accepted")?;
    append_field(header, 0x1B, "goods")?;
    header.push(0);

    // Nest: SlStationBase (normal.base)
    append_stnn_base_header(header)?;

    // Nest: SlStationGoods
    append_field(header, 2, "status")?;
    append_field(header, 2, "time_since_pickup")?;
    append_field(header, 2, "rating")?;
    append_field(header, 2, "last_speed")?;
    append_field(header, 2, "last_age")?;
    append_field(header, 2, "amount_fract")?;
    append_field(header, 6, "cargo.reserved_count")?;
    append_field(header, 4, "link_graph")?;
    append_field(header, 4, "node")?;
    append_field(header, 6, "max_waiting_cargo")?;
    append_field(header, 0x1B, "flow")?;
    append_field(header, 0x1B, "cargo")?;
    header.push(0);

    // Nest: SlStationFlow
    append_field(header, 4, "source")?;
    append_field(header, 4, "via")?;
    append_field(header, 6, "share")?;
    append_field(header, 1, "restricted")?; // SLE_FILE_I8 / bool
    header.push(0);

    // Nest: SlStationCargo
    append_field(header, 4, "first")?;
    append_field(header, 0x16, "second")?; // REFLIST
    header.push(0);

    // Nest: SlStationWaypoint
    append_field(header, 0x1B, "base")?;
    append_field(header, 4, "town_cn")?;
    append_field(header, 6, "train_station.tile")?;
    append_field(header, 2, "train_station.w")?;
    append_field(header, 2, "train_station.h")?;
    append_field(header, 4, "waypoint_flags")?;
    append_field(header, 6, "road_waypoint_area.tile")?;
    append_field(header, 2, "road_waypoint_area.w")?;
    append_field(header, 2, "road_waypoint_area.h")?;
    header.push(0);

    // Nest: waypoint.base
    append_stnn_base_header(header)?;

    // Nest: speclist (StationSpec)
    append_field(header, 6, "grfid")?;
    append_field(header, 4, "localidx")?;
    header.push(0);

    // Nest: roadstopspeclist
    append_field(header, 6, "grfid")?;
    append_field(header, 4, "localidx")?;
    header.push(0);

    // Nest: roadstoptiledata
    append_field(header, 6, "tile")?;
    append_field(header, 2, "random_bits")?;
    append_field(header, 2, "animation_frame")?;
    header.push(0);

    Ok(())
}

fn append_stnn_base_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 6, "xy")?;
    append_field(header, 6, "town")?;
    append_field(header, 9, "string_id")?; // SLE_FILE_STRINGID
    append_field(header, 0x1A, "name")?; // STRING | HAS_LENGTH
    append_field(header, 2, "delete_ctr")?;
    append_field(header, 2, "owner")?;
    append_field(header, 2, "facilities")?;
    append_field(header, 5, "build_date")?; // I32
    append_field(header, 4, "random_bits")?;
    append_field(header, 2, "waiting_triggers")?;
    header.push(0);
    Ok(())
}

fn write_cargo_groups(buf: &mut Vec<u8>, groups: &[StationCargoGroupWire]) -> Result<(), SavError> {
    write_gamma(
        u32::try_from(groups.len()).map_err(|_| SavError::ValueOutOfRange {
            field: "station cargo group count",
            value: u32::MAX,
        })?,
        buf,
    )?;
    for group in groups {
        buf.extend_from_slice(&group.next_hop.to_be_bytes());
        write_gamma(
            u32::try_from(group.packet_ids.len()).map_err(|_| SavError::ValueOutOfRange {
                field: "station cargo packet count",
                value: u32::MAX,
            })?,
            buf,
        )?;
        for packet_id in &group.packet_ids {
            buf.extend_from_slice(&packet_id.saturating_add(1).to_be_bytes());
        }
    }
    Ok(())
}

fn write_station_goods_entry(
    buf: &mut Vec<u8>,
    station: &Station,
    cargo: Option<CargoType>,
    saved: Option<&StationCargoWire>,
) -> Result<(), SavError> {
    let Some(cargo) = cargo else {
        return write_empty_goods_entry(buf);
    };
    let entry = station.goods.get(cargo);
    let groups = saved.map_or(&[][..], |saved| saved.groups.as_slice());
    let reserved = saved.map_or(0, |saved| saved.reserved);
    // `GoodsEntry::State::EverAccepted` queda implícito cuando hay paquetes;
    // el resto del estado mensual se conserva en la entrada JSON propia.
    buf.push(u8::from(!groups.is_empty())); // status
    buf.push(station.time_since_pickup.get(cargo));
    buf.push(entry.rating);
    buf.push(entry.last_speed);
    buf.push(entry.last_age);
    buf.push(entry.amount_fract);
    buf.extend_from_slice(&reserved.to_be_bytes()); // cargo.reserved_count
    buf.extend_from_slice(&u16::MAX.to_be_bytes()); // link_graph
    buf.extend_from_slice(&u16::MAX.to_be_bytes()); // node
    buf.extend_from_slice(&entry.max_waiting_cargo.to_be_bytes());
    write_gamma(0, buf)?; // flow (el exportador aún no materializa shares)
    write_cargo_groups(buf, groups)
}

fn write_empty_goods_entry(buf: &mut Vec<u8>) -> Result<(), SavError> {
    // Defaults típicos de GoodsEntry vacía en saves 15.3.
    buf.push(0); // status
    buf.push(255); // time_since_pickup
    buf.push(175); // rating
    buf.push(0); // last_speed
    buf.push(255); // last_age
    buf.push(0); // amount_fract
    buf.extend_from_slice(&0u32.to_be_bytes()); // cargo.reserved_count
    buf.extend_from_slice(&u16::MAX.to_be_bytes()); // link_graph
    buf.extend_from_slice(&u16::MAX.to_be_bytes()); // node
    buf.extend_from_slice(&0u32.to_be_bytes()); // max_waiting_cargo
    write_gamma(0, buf)?; // flow
    write_gamma(0, buf)?; // cargo
    Ok(())
}

fn write_stnn_base(
    buf: &mut Vec<u8>,
    tile_idx: u32,
    name: &str,
    facilities: u8,
    town_ref: u32,
    owner: crate::company::CompanyId,
) -> Result<(), SavError> {
    buf.extend_from_slice(&tile_idx.to_be_bytes());
    buf.extend_from_slice(&town_ref.to_be_bytes());
    buf.extend_from_slice(&STR_SV_STNAME.to_be_bytes());
    write_str(name, buf)?;
    buf.push(0); // delete_ctr
    buf.push(owner.0);
    buf.push(facilities);
    buf.extend_from_slice(&0i32.to_be_bytes()); // build_date
    buf.extend_from_slice(&0u16.to_be_bytes()); // random_bits
    buf.push(0); // waiting_triggers
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_stnn_normal(
    buf: &mut Vec<u8>,
    st: &Station,
    tile_idx: u32,
    map_w: u32,
    facilities: u8,
    town_ref: u32,
    owner: crate::company::CompanyId,
    station_id: u32,
    cargo_export: &CargoPacketExport,
    climate: crate::Climate,
    persistent_storage_id: Option<u32>,
) -> Result<(), SavError> {
    let name = st.name.as_deref().unwrap_or("");
    buf.push(1); // base presente
    write_stnn_base(buf, tile_idx, name, facilities, town_ref, owner)?;

    let (train_tile, train_w, train_h) =
        if facilities & FACIL_TRAIN != 0 && !is_waypoint(facilities) {
            (tile_idx, 1u8, 1u8)
        } else {
            (INVALID_TILE, 0u8, 0u8)
        };
    buf.extend_from_slice(&train_tile.to_be_bytes());
    buf.push(train_w);
    buf.push(train_h);

    // bus_stops / truck_stops: sin chunk ROAD → null.
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());

    buf.extend_from_slice(&INVALID_TILE.to_be_bytes()); // ship_station.tile
    buf.push(0);
    buf.push(0);
    buf.extend_from_slice(&INVALID_TILE.to_be_bytes()); // docking_station.tile
    buf.push(0);
    buf.push(0);

    let (air_tile, air_w, air_h, air_type) = if facilities & FACIL_AIRPORT != 0 {
        let (air_tile, air_w, air_h) = airport_wire_footprint(st, tile_idx, map_w);
        let air_type = st
            .airport_newgrf_spec_id
            .and_then(|id| u8::try_from(id).ok())
            .unwrap_or_else(|| st.airport_spec.as_ottd_airport_type());
        (air_tile, air_w, air_h, air_type)
    } else {
        (INVALID_TILE, 0u8, 0u8, 0u8)
    };
    buf.extend_from_slice(&air_tile.to_be_bytes());
    buf.push(air_w);
    buf.push(air_h);
    buf.push(air_type);
    buf.push(st.airport_layout); // layout
    buf.extend_from_slice(&0u64.to_be_bytes()); // airport.flags
    buf.push(st.airport_rotation & 6); // rotation
    let psa = if facilities & FACIL_AIRPORT != 0 {
        persistent_storage_id
            .map(|id| {
                id.checked_add(1).ok_or(SavError::ValueOutOfRange {
                    field: "airport persistent storage id",
                    value: id,
                })
            })
            .transpose()?
            .unwrap_or(0)
    } else {
        0
    };
    buf.extend_from_slice(&psa.to_be_bytes()); // airport.psa (REF_STORAGE)

    buf.push(0); // indtype
    buf.push(0); // time_since_load
    buf.push(0); // time_since_unload
    buf.push(VEH_INVALID); // last_vehicle_type
    buf.push(0); // had_vehicle_of_type
    write_gamma(0, buf)?; // loading_vehicles
    buf.extend_from_slice(&0u64.to_be_bytes()); // always_accepted

    write_gamma(NUM_CARGO, buf)?;
    for slot in 0..NUM_CARGO {
        let slot_u8 = u8::try_from(slot).ok();
        let cargo = slot_u8.and_then(|slot| CargoType::from_climate_slot(climate, slot));
        let saved = slot_u8.and_then(|slot| {
            cargo_export
                .station_refs
                .get(&station_id)
                .and_then(|entries| entries.iter().find(|entry| entry.cargo_slot == slot))
        });
        write_station_goods_entry(buf, st, cargo, saved)?;
    }
    Ok(())
}

/// Convierte la huella materializada de una estación en los campos compactos
/// `airport.tile/w/h` de `STNN`. El origen se elige de forma determinista
/// (mínimo Y/X), que coincide con el ancla de los layouts vanilla y custom.
fn airport_wire_footprint(st: &Station, fallback_tile: u32, map_w: u32) -> (u32, u8, u8) {
    let Some((min, rest)) = st.airport_tiles.split_first().map(|(first, rest)| {
        let min = rest.iter().copied().fold(*first, |best, coord| {
            if (coord.y, coord.x).lt(&(best.y, best.x)) {
                coord
            } else {
                best
            }
        });
        (min, rest)
    }) else {
        return (fallback_tile, 1, 1);
    };
    let max = rest.iter().copied().fold(min, |best, coord| {
        TileCoord::new(best.x.max(coord.x), best.y.max(coord.y))
    });
    let tile = coord_to_linear_index(min, map_w).unwrap_or(fallback_tile);
    let width = u8::try_from(max.x.saturating_sub(min.x).saturating_add(1)).unwrap_or(u8::MAX);
    let height = u8::try_from(max.y.saturating_sub(min.y).saturating_add(1)).unwrap_or(u8::MAX);
    (tile, width, height)
}

fn write_stnn_waypoint(
    buf: &mut Vec<u8>,
    st: &Station,
    tile_idx: u32,
    facilities: u8,
    town_ref: u32,
    owner: crate::company::CompanyId,
) -> Result<(), SavError> {
    let name = st.name.as_deref().unwrap_or("");
    buf.push(1); // base presente
    write_stnn_base(buf, tile_idx, name, facilities, town_ref, owner)?;
    buf.extend_from_slice(&0u16.to_be_bytes()); // town_cn
    let (train_tile, w, h) = if facilities & FACIL_TRAIN != 0 {
        (tile_idx, 1u8, 1u8)
    } else {
        (INVALID_TILE, 0u8, 0u8)
    };
    buf.extend_from_slice(&train_tile.to_be_bytes());
    buf.push(w);
    buf.push(h);
    buf.extend_from_slice(&0u16.to_be_bytes()); // waypoint_flags
    let (road_tile, rw, rh) = if facilities & (FACIL_BUS_STOP | FACIL_TRUCK_STOP) != 0 {
        (tile_idx, 1u8, 1u8)
    } else {
        (INVALID_TILE, 0u8, 0u8)
    };
    buf.extend_from_slice(&road_tile.to_be_bytes());
    buf.push(rw);
    buf.push(rh);
    Ok(())
}

/// Construye records STNN modernos (SAVEBYTE + structs) desde estaciones del estado.
///
/// # Errors
///
/// Falla si algún nombre de estación es demasiado largo.
#[cfg(test)]
pub(super) fn stnn_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    let cargo_export = cargo_packet_export(state, map_w);
    stnn_records_with_cargo(state, map_w, &cargo_export)
}

pub(crate) fn stnn_records_with_cargo(
    state: &GameState,
    map_w: u32,
    cargo_export: &CargoPacketExport,
) -> Result<Vec<Vec<u8>>, SavError> {
    // CITY sintético o real: el primer municipio es índice 0 → ref 1.
    let town_ref = 1u32;
    let station_persistent_storage_ids = station_persistent_storage_ids(state)?;
    let mut out = Vec::with_capacity(state.stations.len());
    for (station_index, st) in state.stations.iter().enumerate() {
        if st.pos.x < 0 || st.pos.y < 0 {
            continue;
        }
        let ux = st.pos.x.cast_unsigned();
        let uy = st.pos.y.cast_unsigned();
        let tile_idx = uy.saturating_mul(map_w).saturating_add(ux);
        // Una estación importada puede conservar varias facilidades en una
        // sola entidad (por ejemplo tren + aeropuerto). `StopKind` representa
        // la facilidad principal, por lo que la huella aérea también debe
        // volver a activar `FACIL_AIRPORT` al exportar.
        let mut facilities = facilities_for_stop(st.stop_kind);
        if !st.airport_tiles.is_empty() || st.airport_newgrf_spec_id.is_some() {
            facilities |= FACIL_AIRPORT;
        }
        let mut rec = Vec::new();
        // SAVEBYTE: leído a mano por OpenTTD antes de SlObject.
        rec.push(facilities);
        if is_waypoint(facilities) {
            rec.push(0); // normal ausente
            rec.push(1); // waypoint presente
            write_stnn_waypoint(&mut rec, st, tile_idx, facilities, town_ref, st.owner)?;
        } else {
            rec.push(1); // normal presente
            let station_id = station_id_for_index(state, station_index).map_or(u32::MAX, u32::from);
            write_stnn_normal(
                &mut rec,
                st,
                tile_idx,
                map_w,
                facilities,
                town_ref,
                st.owner,
                station_id,
                cargo_export,
                state.climate,
                station_persistent_storage_ids
                    .get(station_index)
                    .copied()
                    .flatten(),
            )?;
            rec.push(0); // waypoint ausente
        }
        write_gamma(0, &mut rec)?; // speclist
        write_road_stop_data(&mut rec, state, st, map_w)?;
        out.push(rec);
    }
    Ok(out)
}

/// Chunk `STNN` `CH_TABLE` con schema moderno.
///
/// # Errors
///
/// Falla si el header o algún gamma está fuera de rango.
pub(super) fn stnn_chunk(records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    append_stnn_header(&mut header)?;
    raw_table_chunk(*b"STNN", &header, records, CH_TABLE)
}

fn append_capa_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 4, "source")?; // StationID
    append_field(header, 6, "source_xy")?; // TileIndex
    append_field(header, 4, "loaded_at_xy")?; // next_hop StationID
    append_field(header, 4, "count")?;
    append_field(header, 4, "periods_in_transit")?;
    append_field(header, 7, "feeder_share")?;
    append_field(header, 2, "source_type")?;
    append_field(header, 4, "source_id")?;
    append_field(header, 3, "travelled.x")?;
    append_field(header, 3, "travelled.y")?;
    header.push(0);
    Ok(())
}

/// Construye el pool `CAPA` y enlaza sus registros desde `STNN`/`VEHS`.
pub(crate) fn capa_chunk(export: &CargoPacketExport) -> Result<Option<Vec<u8>>, SavError> {
    let records = capa_records(export);
    if records.is_empty() {
        return Ok(None);
    }
    let mut header = Vec::new();
    append_capa_header(&mut header)?;
    raw_table_chunk(*b"CAPA", &header, &records, CH_TABLE).map(Some)
}

/// Serializa las filas semánticas del pool `CAPA`.
pub(crate) fn capa_records(export: &CargoPacketExport) -> Vec<Vec<u8>> {
    export
        .packets
        .iter()
        .map(|packet| {
            let mut record = Vec::with_capacity(31);
            record.extend_from_slice(&packet.source.to_be_bytes());
            record.extend_from_slice(&packet.source_xy.to_be_bytes());
            record.extend_from_slice(&packet.loaded_at_xy.to_be_bytes());
            record.extend_from_slice(&packet.count.to_be_bytes());
            record.extend_from_slice(&packet.periods_in_transit.to_be_bytes());
            record.extend_from_slice(&packet.feeder_share.to_be_bytes());
            record.push(packet.source_type);
            record.extend_from_slice(&packet.source_id.to_be_bytes());
            record.extend_from_slice(&packet.travelled_x.to_be_bytes());
            record.extend_from_slice(&packet.travelled_y.to_be_bytes());
            record
        })
        .collect()
}

/// Banderas `TownFlag` reconstruidas desde el modelo semántico.
fn city_flags_for_save(town: &Town) -> u8 {
    let mut flags = town.native_flags & !0x07;
    if town.is_growing {
        flags |= 1;
    }
    if town.has_church {
        flags |= 1 << 1;
    }
    if town.has_stadium {
        flags |= 1 << 2;
    }
    flags
}

fn write_city_supplied(
    entries: &[crate::town::TownSuppliedCargo],
    buf: &mut Vec<u8>,
) -> Result<(), SavError> {
    let count = u32::try_from(entries.len()).map_err(|_| SavError::ValueOutOfRange {
        field: "town supplied cargo count",
        value: u32::MAX,
    })?;
    write_gamma(count, buf)?;
    for entry in entries {
        buf.push(entry.cargo);
        let history_count =
            u32::try_from(entry.history.len()).map_err(|_| SavError::ValueOutOfRange {
                field: "town supplied history count",
                value: u32::MAX,
            })?;
        write_gamma(history_count, buf)?;
        for sample in &entry.history {
            buf.extend_from_slice(&sample.production.to_be_bytes());
            buf.extend_from_slice(&sample.transported.to_be_bytes());
        }
    }
    Ok(())
}

fn write_city_received(
    entries: &[crate::town::TownReceivedCargo],
    buf: &mut Vec<u8>,
) -> Result<(), SavError> {
    // `Town::received` es `std::array<..., NUM_TAE>` en OpenTTD, por lo que
    // la longitud de la lista también es fija (6 con el slot `TAE_NONE`).
    write_gamma(
        u32::try_from(crate::town::TOWN_GROWTH_EFFECT_COUNT + 1).unwrap_or(u32::MAX),
        buf,
    )?;
    for index in 0..=crate::town::TOWN_GROWTH_EFFECT_COUNT {
        let entry = entries.get(index).cloned().unwrap_or_default();
        buf.extend_from_slice(&entry.old_max.to_be_bytes());
        buf.extend_from_slice(&entry.new_max.to_be_bytes());
        buf.extend_from_slice(&entry.old_act.to_be_bytes());
        buf.extend_from_slice(&entry.new_act.to_be_bytes());
    }
    Ok(())
}

/// Serializa una fila `CITY` moderna, incluidos los campos nativos que el
/// importador conserva para scopes `NewGRF` y para el round-trip SAV.
fn city_record(town: &Town, tile_idx: u32, psa_ids: &[u32]) -> Result<Vec<u8>, SavError> {
    let mut rec = Vec::new();
    rec.extend_from_slice(&tile_idx.to_be_bytes());
    rec.extend_from_slice(&town.townnamegrfid.to_be_bytes());
    // Las ciudades creadas por el runtime aún no tienen un generador nativo
    // asignado. Usar el generador inglés vanilla evita que OpenTTD rechace la
    // fila aunque el nombre visible sea custom.
    let townname_type = if town.townnamegrfid == 0 && town.townnametype == 0 {
        0x20C0
    } else {
        town.townnametype
    };
    rec.extend_from_slice(&townname_type.to_be_bytes());
    rec.extend_from_slice(&town.townnameparts.to_be_bytes());
    write_str(&town.name, &mut rec)?;
    rec.push(city_flags_for_save(town));
    rec.extend_from_slice(&town.statues.to_be_bytes());
    rec.extend_from_slice(&town.have_ratings.to_be_bytes());

    // `ratings` es un array de tamaño fijo (`MAX_COMPANIES`) aunque el
    // header use `HAS_LENGTH`: OpenTTD valida exactamente 15 elementos.
    write_gamma(
        u32::try_from(crate::town::MAX_TOWN_AUTHORITY_COMPANIES).unwrap_or(u32::MAX),
        &mut rec,
    )?;
    for index in 0..crate::town::MAX_TOWN_AUTHORITY_COMPANIES {
        let rating = town
            .authority_ratings
            .get(index)
            .copied()
            .unwrap_or(crate::town::TOWN_RATING_INITIAL);
        rec.extend_from_slice(&rating.to_be_bytes());
    }

    // Igual que `ratings`, `unwanted` es un array fijo por compañía.
    write_gamma(
        u32::try_from(crate::town::MAX_TOWN_AUTHORITY_COMPANIES).unwrap_or(u32::MAX),
        &mut rec,
    )?;
    for index in 0..crate::town::MAX_TOWN_AUTHORITY_COMPANIES {
        rec.push(town.unwanted.get(index).copied().unwrap_or(0));
    }

    // `NUM_TAE` es también fijo en la tabla nativa. OpenTTD reserva el slot 0
    // para `TAE_NONE`; el modelo compacto guarda los cinco efectos restantes.
    write_gamma(
        u32::try_from(crate::town::TOWN_GROWTH_EFFECT_COUNT + 1).unwrap_or(u32::MAX),
        &mut rec,
    )?;
    rec.extend_from_slice(&0_u32.to_be_bytes());
    // TAE order: NONE, PASSENGERS, MAIL, GOODS, WATER, FOOD.
    for index in [0, 1, 2, 4, 3] {
        let goal = town.goals[index];
        rec.extend_from_slice(&goal.to_be_bytes());
    }

    write_str(&town.native_text, &mut rec)?;
    rec.extend_from_slice(&town.time_until_rebuild.to_be_bytes());
    rec.extend_from_slice(&town.grow_counter.to_be_bytes());
    rec.extend_from_slice(&town.growth_rate.to_be_bytes());
    rec.push(town.fund_buildings_months);
    rec.push(town.road_build_months);
    rec.push(town.exclusivity.map_or(u8::MAX, |company| company.0));
    rec.push(town.exclusive_counter);
    rec.push(u8::from(town.larger_town));
    rec.push(town.layout as u8);
    rec.extend_from_slice(&town.valid_history.to_be_bytes());
    write_persistent_storage_refs(psa_ids, &mut rec)?;
    write_city_supplied(&town.supplied_cargo, &mut rec)?;
    write_city_received(&town.received_cargo, &mut rec)?;
    Ok(rec)
}

/// Record CITY canónico (`OpenTTD` exige ≥1 municipio:
/// `STR_ERROR_NO_TOWN_IN_SCENARIO`).
pub(super) fn default_city_record(map_w: u32, map_h: u32) -> Result<Vec<u8>, SavError> {
    let x = map_w / 2;
    let y = map_h / 2;
    let tile_idx = y.saturating_mul(map_w).saturating_add(x);
    let town = Town {
        pos: TileCoord::new(
            i32::try_from(x).unwrap_or(i32::MAX),
            i32::try_from(y).unwrap_or(i32::MAX),
        ),
        name: "Town".into(),
        townnametype: 0x20C0,
        population: 500,
        ..Default::default()
    };
    city_record(&town, tile_idx, &[])
}

/// Escribe un `REFVECTOR(REF_STORAGE)` usando índices cero basados en runtime.
fn write_persistent_storage_refs(ids: &[u32], buf: &mut Vec<u8>) -> Result<(), SavError> {
    let count = u32::try_from(ids.len()).map_err(|_| SavError::ValueOutOfRange {
        field: "town persistent storage count",
        value: u32::MAX,
    })?;
    write_gamma(count, buf)?;
    for &id in ids {
        let reference = id.checked_add(1).ok_or(SavError::ValueOutOfRange {
            field: "town persistent storage id",
            value: id,
        })?;
        buf.extend_from_slice(&reference.to_be_bytes());
    }
    Ok(())
}

/// Construye records CITY desde ciudades del estado.
///
/// Si no hay towns, emite un municipio sintético (requerido por `OpenTTD` al load).
///
/// # Errors
///
/// Falla si algún nombre de ciudad es demasiado largo.
pub(super) fn city_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    let assigned_town_storage_ids = persistent_storage_ids(state)?.2;
    let mut out = Vec::with_capacity(state.towns.len().max(1));
    for (town_index, town) in state.towns.iter().enumerate() {
        let Some(tile_idx) = coord_to_linear_index(town.pos, map_w) else {
            continue;
        };
        let mut ids = state
            .sav_town_persistent_storage_ids
            .get(&town.id)
            .cloned()
            .unwrap_or_default();
        if let Some(assigned) = assigned_town_storage_ids.get(town_index) {
            let mut extras: Vec<u32> = assigned.values().copied().collect();
            extras.sort_unstable();
            for id in extras {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        out.push(city_record(town, tile_idx, &ids)?);
    }
    if out.is_empty() {
        let (_, h) = state.map.dimensions();
        out.push(default_city_record(map_w, h)?);
    }
    Ok(out)
}

fn industry_ottd_type(ind: &Industry) -> u8 {
    // Índices temperate OpenTTD (`table/industry.h`); best-effort.
    let spec = ind.spec.unwrap_or(match ind.kind {
        IndustryKind::CoalMine => IndustrySpec::CoalMine,
        IndustryKind::Forest => IndustrySpec::Forest,
        IndustryKind::OilWell => IndustrySpec::OilWells,
        IndustryKind::Factory => IndustrySpec::Factory,
    });
    match spec {
        IndustrySpec::CoalMine => 0,
        IndustrySpec::PowerStation => 1,
        IndustrySpec::Sawmill => 2,
        IndustrySpec::Forest => 3,
        IndustrySpec::OilRefinery => 4,
        IndustrySpec::OilWells => 5,
        IndustrySpec::Farm => 6,
        IndustrySpec::Factory => 7,
        IndustrySpec::IronOreMine => 8,
        IndustrySpec::GoldMine => 18,
        IndustrySpec::CopperOreMine => 24,
        other => {
            let _ = other;
            0
        }
    }
}

fn industry_footprint(ind: &Industry) -> (u8, u8) {
    if ind.tiles.is_empty() {
        return (1, 1);
    }
    let min_x = ind.tiles.iter().map(|t| t.x).min().unwrap_or(ind.pos.x);
    let max_x = ind.tiles.iter().map(|t| t.x).max().unwrap_or(ind.pos.x);
    let min_y = ind.tiles.iter().map(|t| t.y).min().unwrap_or(ind.pos.y);
    let max_y = ind.tiles.iter().map(|t| t.y).max().unwrap_or(ind.pos.y);
    let w = u8::try_from((max_x - min_x + 1).clamp(1, 255)).unwrap_or(1);
    let h = u8::try_from((max_y - min_y + 1).clamp(1, 255)).unwrap_or(1);
    (w, h)
}

fn append_indy_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    // El header es deliberadamente un subconjunto compatible con
    // `_industry_desc`: los campos omitidos quedan con sus defaults al
    // cargar en OpenTTD, mientras que las listas de carga sí se preservan.
    append_field(header, 6, "location.tile")?;
    append_field(header, 2, "location.w")?;
    append_field(header, 2, "location.h")?;
    append_field(header, 6, "neutral_station")?; // REF_STATION
    append_field(header, 2, "type")?;
    append_field(header, 2, "random_colour")?;
    append_field(header, 2, "prod_level")?;
    append_field(header, 4, "counter")?;
    append_field(header, 5, "last_prod_year")?;
    append_field(header, 2, "was_cargo_delivered")?;
    append_field(header, 2, "ctlflags")?;
    append_field(header, 2, "exclusive_supplier")?;
    append_field(header, 2, "founder")?;
    append_field(header, 5, "construction_date")?;
    append_field(header, 2, "construction_type")?;
    append_field(header, 2, "selected_layout")?;
    append_field(header, 6, "psa")?;
    append_field(header, 4, "random")?;
    append_field(header, 8, "valid_history")?;
    append_field(header, 0x1B, "accepted")?;
    append_field(header, 0x1B, "produced")?;
    header.push(0);

    // SlIndustryAccepted: sólo los campos que el modelo puede reconstruir.
    append_field(header, 2, "cargo")?;
    append_field(header, 4, "waiting")?;
    append_field(header, 5, "last_accepted")?;
    append_field(header, 6, "accumulated_waiting")?;
    append_field(header, 0x1B, "history")?;
    header.push(0);

    // SlIndustryAcceptedHistory.
    append_field(header, 4, "accepted")?;
    append_field(header, 4, "waiting")?;
    header.push(0);

    // SlIndustryProduced: cargo, stock, rate e historial por salida.
    append_field(header, 2, "cargo")?;
    append_field(header, 4, "waiting")?;
    append_field(header, 2, "rate")?;
    append_field(header, 0x1B, "history")?;
    header.push(0);

    // SlIndustryProducedHistory.
    append_field(header, 4, "production")?;
    append_field(header, 4, "transported")?;
    header.push(0);
    Ok(())
}

fn write_indy_accepted(
    buf: &mut Vec<u8>,
    industry: &Industry,
    climate: crate::Climate,
    saved: Option<&crate::sav::SavIndustry>,
) -> Result<(), SavError> {
    let mut cargos = crate::cargo::CargoType::for_climate(climate).to_vec();
    for &cargo in industry.accepted_history.keys() {
        if !cargos.contains(&cargo) {
            cargos.push(cargo);
        }
    }
    if let Some(saved_industry) = saved {
        for entry in &saved_industry.accepted {
            let Some(cargo) = crate::CargoType::from_climate_slot(climate, entry.cargo_slot) else {
                continue;
            };
            if !cargos.contains(&cargo) {
                cargos.push(cargo);
            }
        }
    }
    let entries: Vec<(u8, crate::CargoType, u16, u32)> = cargos
        .into_iter()
        .filter_map(|cargo| {
            let slot = cargo_slot_for_climate(climate, cargo)?;
            let waiting = industry.accepted_cargo_waiting(cargo);
            let last_accepted = industry.last_accepted_date(cargo);
            let saved_entry = saved.and_then(|saved_industry| {
                saved_industry.accepted.iter().find(|entry| {
                    crate::CargoType::from_climate_slot(climate, entry.cargo_slot) == Some(cargo)
                })
            });
            let has_runtime_history = industry
                .accepted_history
                .get(&cargo)
                .is_some_and(|history| !history.is_empty());
            let has_saved_history = saved_entry.is_some_and(|entry| !entry.history.is_empty());
            let accumulated_waiting = if has_runtime_history {
                industry.accepted_accumulated_waiting.get(cargo)
            } else {
                saved_entry.map_or(0, |entry| entry.accumulated_waiting)
            };
            if waiting == 0
                && last_accepted == 0
                && accumulated_waiting == 0
                && !has_runtime_history
                && !has_saved_history
            {
                return None;
            }
            Some((
                slot,
                cargo,
                waiting.min(u32::from(u16::MAX)) as u16,
                last_accepted,
            ))
        })
        .collect();
    // At most 12 entries for the built-in climate catalog, well below the
    // simple-gamma limit used by the TABLE codec.
    write_gamma(entries.len() as u32, buf)?;
    for (cargo_slot, cargo_type, waiting, last_accepted) in entries {
        buf.push(cargo_slot);
        buf.extend_from_slice(&waiting.to_be_bytes());
        let last_accepted = i32::try_from(last_accepted).unwrap_or(i32::MAX);
        buf.extend_from_slice(&last_accepted.to_be_bytes());
        let saved_entry = saved.and_then(|industry| {
            industry.accepted.iter().find(|entry| {
                crate::CargoType::from_climate_slot(climate, entry.cargo_slot) == Some(cargo_type)
            })
        });
        let accumulated_waiting = if industry.accepted_history.contains_key(&cargo_type) {
            industry.accepted_accumulated_waiting.get(cargo_type)
        } else {
            saved_entry.map_or(0, |entry| entry.accumulated_waiting)
        };
        buf.extend_from_slice(&accumulated_waiting.to_be_bytes());
        let runtime_history = industry.accepted_history.get(&cargo_type);
        let history = runtime_history
            .filter(|history| !history.is_empty())
            .map(|history| {
                history
                    .iter()
                    .map(|sample| crate::sav::SavIndustryAcceptedHistory {
                        accepted: sample.accepted,
                        waiting: sample.waiting,
                    })
                    .collect::<Vec<_>>()
            })
            .or_else(|| saved_entry.map(|entry| entry.history.clone()))
            .unwrap_or_default();
        write_gamma(
            u32::try_from(history.len()).map_err(|_| SavError::ValueOutOfRange {
                field: "industry accepted history length",
                value: u32::MAX,
            })?,
            buf,
        )?;
        for sample in history {
            buf.extend_from_slice(&sample.accepted.to_be_bytes());
            buf.extend_from_slice(&sample.waiting.to_be_bytes());
        }
    }
    Ok(())
}

fn write_indy_produced(
    buf: &mut Vec<u8>,
    industry: &Industry,
    climate: crate::Climate,
    saved: Option<&crate::sav::SavIndustry>,
) -> Result<(), SavError> {
    let mut outputs = industry.produced_cargos();
    for &cargo in industry.produced_history.keys() {
        if !outputs.contains(&cargo) {
            outputs.push(cargo);
        }
    }
    if let Some(saved_industry) = saved {
        for entry in &saved_industry.produced {
            let Some(cargo) = crate::CargoType::from_climate_slot(climate, entry.cargo_slot) else {
                continue;
            };
            if !outputs.contains(&cargo) {
                outputs.push(cargo);
            }
        }
    }
    // Conserva stocks extra que no forman parte del catálogo actual. Los
    // labels custom siguen limitados al mapeo fijo de `CargoType`, igual que
    // el resto del writer de `INDY`.
    for &cargo in &crate::cargo::ALL_CARGO_TYPES {
        if !outputs.contains(&cargo) && industry.extra_produced_cargo(cargo) > 0 {
            outputs.push(cargo);
        }
    }
    let secondary_rate = industry
        .newgrf_secondary_production_rate
        .or_else(|| {
            industry
                .spec
                .and_then(IndustrySpec::production_rate_secondary)
        })
        .unwrap_or(0);
    let mut entries: Vec<(u8, u16, u8, Vec<crate::sav::SavIndustryProducedHistory>)> =
        Vec::with_capacity(outputs.len() + 4);
    for cargo in outputs.iter().copied() {
        let output_index = industry
            .produced_cargos()
            .iter()
            .position(|&output| output == cargo);
        let waiting = match output_index {
            Some(0) => industry.stock,
            Some(1) => industry.secondary_stock,
            Some(_) | None => industry.extra_produced_cargo(cargo),
        };
        let saved_entry = saved.and_then(|saved_industry| {
            saved_industry.produced.iter().find(|entry| {
                crate::CargoType::from_climate_slot(climate, entry.cargo_slot) == Some(cargo)
            })
        });
        let rate = match output_index {
            Some(0) => industry.production_rate(),
            Some(1) => secondary_rate,
            Some(output_index) => industry
                .newgrf_extra_production_rates
                .get(output_index - 2)
                .copied()
                .unwrap_or_else(|| saved_entry.map_or(0, |entry| entry.rate)),
            None => saved_entry.map_or(0, |entry| entry.rate),
        };
        let history = industry
            .produced_history
            .get(&cargo)
            .filter(|history| !history.is_empty())
            .map(|history| {
                history
                    .iter()
                    .take(crate::entity_history::INDUSTRY_HISTORY_RECORDS)
                    .map(|sample| crate::sav::SavIndustryProducedHistory {
                        production: sample.production,
                        transported: sample.transported,
                    })
                    .collect::<Vec<_>>()
            })
            .or_else(|| saved_entry.map(|entry| entry.history.clone()))
            .unwrap_or_default();
        if waiting == 0 && history.is_empty() && saved_entry.is_none() {
            continue;
        }
        if let Some(slot) = cargo_slot_for_climate(climate, cargo) {
            entries.push((slot, waiting.min(u32::from(u16::MAX)) as u16, rate, history));
        }
    }
    write_gamma(entries.len() as u32, buf)?;
    for (cargo, waiting, rate, history) in entries {
        buf.push(cargo);
        buf.extend_from_slice(&waiting.to_be_bytes());
        buf.push(rate);
        write_gamma(
            u32::try_from(history.len()).map_err(|_| SavError::ValueOutOfRange {
                field: "industry produced history length",
                value: u32::MAX,
            })?,
            buf,
        )?;
        for sample in history {
            buf.extend_from_slice(&sample.production.to_be_bytes());
            buf.extend_from_slice(&sample.transported.to_be_bytes());
        }
    }
    Ok(())
}

fn saved_industry<'a>(
    state: &'a GameState,
    industry: &Industry,
) -> Option<&'a crate::sav::SavIndustry> {
    let id = u32::from(industry.instance_id);
    if id != 0 {
        return state
            .sav_industry_histories
            .iter()
            .find(|saved| saved.industry_id == id && saved.pos == industry.pos);
    }
    state
        .sav_industry_histories
        .iter()
        .find(|saved| saved.pos == industry.pos)
}

fn neutral_station_ref(state: &GameState, industry: &Industry) -> Result<u32, SavError> {
    let Some(station_id) = industry.neutral_station_id else {
        return Ok(0);
    };
    // `REF_STATION` is an index + 1; zero is the null reference.
    station_id
        .checked_add(1)
        .ok_or(SavError::ValueOutOfRange {
            field: "industry neutral station id",
            value: station_id,
        })
        .and_then(|reference| {
            state
                .stations
                .iter()
                .any(|station| station.ottd_station_id == Some(station_id))
                .then_some(reference)
                .ok_or(SavError::ValueOutOfRange {
                    field: "industry neutral station reference",
                    value: station_id,
                })
        })
}

pub(super) fn indy_records_with_cargo(
    state: &GameState,
    map_w: u32,
) -> Result<Vec<Vec<u8>>, SavError> {
    let persistent_storage_ids = industry_persistent_storage_ids(state)?;
    let mut out = Vec::with_capacity(state.industries.len());
    for (industry_index, ind) in state.industries.iter().enumerate() {
        let Some(tile_idx) = coord_to_linear_index(ind.pos, map_w) else {
            continue;
        };
        let (w, h) = industry_footprint(ind);
        let saved = saved_industry(state, ind);
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        rec.push(w);
        rec.push(h);
        rec.extend_from_slice(&neutral_station_ref(state, ind)?.to_be_bytes());
        rec.push(industry_ottd_type(ind));
        rec.push(ind.random_colour % 16);
        rec.push(ind.prod_level);
        rec.extend_from_slice(&ind.counter.to_be_bytes());
        let last_prod_year = i32::try_from(ind.last_prod_year).unwrap_or(i32::MAX);
        rec.extend_from_slice(&last_prod_year.to_be_bytes());
        rec.push(u8::from(ind.was_cargo_delivered));
        rec.push(ind.control_flags);
        rec.push(
            ind.exclusive_supplier
                .map_or(crate::company::CompanyId::INVALID.0, |owner| owner.0),
        );
        rec.push(
            ind.founder
                .map_or(crate::industry::INDUSTRY_FOUNDER_INVALID, |id| id.0),
        );
        let construction_date = i32::try_from(ind.construction_date).unwrap_or(i32::MAX);
        rec.extend_from_slice(&construction_date.to_be_bytes());
        rec.push(ind.construction_type);
        rec.push(ind.selected_layout);
        let psa = match persistent_storage_ids
            .get(industry_index)
            .copied()
            .flatten()
        {
            Some(id) => id.checked_add(1).ok_or(SavError::ValueOutOfRange {
                field: "persistent storage id",
                value: id,
            })?,
            None => 0,
        };
        rec.extend_from_slice(&psa.to_be_bytes());
        rec.extend_from_slice(&ind.newgrf_random.to_be_bytes());
        let valid_history = if ind.valid_history != 0 {
            ind.valid_history
        } else {
            saved.map_or(0, |saved| saved.valid_history)
        };
        rec.extend_from_slice(&valid_history.to_be_bytes());
        write_indy_accepted(&mut rec, ind, state.climate, saved)?;
        write_indy_produced(&mut rec, ind, state.climate, saved)?;
        out.push(rec);
    }
    Ok(out)
}

/// Determina los índices de pool que deben aparecer en `INDY.psa`.
///
/// Las industrias importadas conservan su referencia nativa. Una industria
/// creada localmente obtiene el primer índice libre sólo cuando tiene registros
/// `7C`; esto hace determinista el export y evita compactar storages ajenos.
fn industry_persistent_storage_ids(state: &GameState) -> Result<Vec<Option<u32>>, SavError> {
    Ok(persistent_storage_ids(state)?.0)
}

fn station_persistent_storage_ids(state: &GameState) -> Result<Vec<Option<u32>>, SavError> {
    Ok(persistent_storage_ids(state)?.1)
}

fn station_has_persistent_storage(station: &Station) -> bool {
    matches!(station.stop_kind, StopKind::Airport)
        || !station.airport_tiles.is_empty()
        || station.airport_newgrf_spec_id.is_some()
}

fn next_free_persistent_storage_id(
    used: &mut std::collections::BTreeSet<u32>,
    next_free: &mut u32,
) -> Result<u32, SavError> {
    while used.contains(next_free) {
        *next_free = next_free.checked_add(1).ok_or(SavError::ValueOutOfRange {
            field: "persistent storage id",
            value: u32::MAX,
        })?;
    }
    let id = *next_free;
    used.insert(id);
    *next_free = next_free.checked_add(1).ok_or(SavError::ValueOutOfRange {
        field: "persistent storage id",
        value: u32::MAX,
    })?;
    Ok(id)
}

/// Asigna IDs estables para los storages de industrias y aeropuertos.
///
/// Los IDs explícitos y las filas importadas ocupan el espacio antes de
/// asignar una fila a registros runtime nuevos. El orden de asignación es
/// industrias y luego estaciones, igual que el orden de emisión de los
/// chunks, para que `STNN.airport.psa` y `PSAC` siempre coincidan.
fn persistent_storage_ids(state: &GameState) -> Result<PersistentStorageIds, SavError> {
    let mut used: std::collections::BTreeSet<u32> = state
        .sav_persistent_storages
        .iter()
        .map(|storage| storage.storage_id)
        .collect();
    used.extend(
        state
            .industries
            .iter()
            .filter_map(|industry| industry.newgrf_persistent_storage_id),
    );
    used.extend(
        state
            .stations
            .iter()
            .filter_map(|station| station.newgrf_persistent_storage_id),
    );
    used.extend(
        state
            .sav_town_persistent_storage_ids
            .values()
            .flatten()
            .copied(),
    );
    used.extend(
        state
            .towns
            .iter()
            .flat_map(|town| town.newgrf_persistent_storage_ids.values())
            .copied(),
    );

    let mut next_free = 0u32;
    let mut industry_ids = Vec::with_capacity(state.industries.len());
    for industry in &state.industries {
        if let Some(id) = industry.newgrf_persistent_storage_id {
            industry_ids.push(Some(id));
        } else if industry.newgrf_persistent_regs.is_empty() {
            industry_ids.push(None);
        } else {
            industry_ids.push(Some(next_free_persistent_storage_id(
                &mut used,
                &mut next_free,
            )?));
        }
    }

    let mut station_ids = Vec::with_capacity(state.stations.len());
    for station in &state.stations {
        if !station_has_persistent_storage(station) {
            station_ids.push(None);
        } else if let Some(id) = station.newgrf_persistent_storage_id {
            station_ids.push(Some(id));
        } else if station.newgrf_persistent_regs.is_empty() {
            station_ids.push(None);
        } else {
            station_ids.push(Some(next_free_persistent_storage_id(
                &mut used,
                &mut next_free,
            )?));
        }
    }

    // Las filas nuevas de pueblo se asignan después de industrias y
    // estaciones, siguiendo el orden de emisión de los pools y evitando
    // renumerar referencias ya presentes en un SAV importado.
    let mut town_ids = Vec::with_capacity(state.towns.len());
    for town in &state.towns {
        let mut ids = town.newgrf_persistent_storage_ids.clone();
        let mut grfids: Vec<u32> = town.newgrf_persistent_regs.keys().copied().collect();
        grfids.sort_unstable();
        for grfid in grfids {
            if ids.contains_key(&grfid) {
                continue;
            }
            let Some(regs) = town.newgrf_persistent_regs.get(&grfid) else {
                continue;
            };
            if regs.is_empty() {
                continue;
            }
            let id = next_free_persistent_storage_id(&mut used, &mut next_free)?;
            ids.insert(grfid, id);
        }
        town_ids.push(ids);
    }
    Ok((industry_ids, station_ids, town_ids))
}

fn industry_persistent_storage_grfid(state: &GameState, industry: &Industry) -> u32 {
    industry
        .newgrf_type_id
        .and_then(|type_id| {
            state
                .industry_spec_catalog
                .iter()
                .find(|spec| spec.id == type_id)
        })
        .map_or(0, |spec| spec.grfid)
}

fn station_persistent_storage_grfid(state: &GameState, station: &Station) -> u32 {
    station
        .airport_newgrf_spec_id
        .and_then(|id| state.airport_spec_catalog.iter().find(|spec| spec.id == id))
        .map_or(0, |spec| spec.newgrf_grfid)
}

fn normalized_storage_values(values: &[u32]) -> Vec<u32> {
    let mut out = vec![0; 256];
    let len = values.len().min(out.len());
    out[..len].copy_from_slice(&values[..len]);
    out
}

fn merge_persistent_storage(
    storages: &mut Vec<SavPersistentStorage>,
    by_id: &mut HashMap<u32, usize>,
    storage_id: u32,
    explicit_storage_id: Option<u32>,
    grfid: u32,
    regs: &HashMap<u8, u32>,
) {
    let storage_index = if let Some(&index) = by_id.get(&storage_id) {
        index
    } else {
        // Una referencia importada sin fila PSAC válida no debe fabricar un
        // storage vacío; sólo se crea una fila al guardar registros runtime.
        if regs.is_empty() && explicit_storage_id.is_some() {
            return;
        }
        let index = storages.len();
        storages.push(SavPersistentStorage {
            storage_id,
            grfid,
            storage: vec![0; 256],
        });
        by_id.insert(storage_id, index);
        index
    };
    let storage = &mut storages[storage_index];
    if storage.grfid == 0 {
        storage.grfid = grfid;
    }
    let mut values = normalized_storage_values(&storage.storage);
    for (&register, &value) in regs {
        values[usize::from(register)] = value;
    }
    storage.storage = values;
}

/// Registros densos del chunk nativo `PSAC`.
///
/// Las filas existentes se conservan, incluso si pertenecen a una estación o
/// pueblo que aún no tiene runtime PSA. Las industrias sólo parchean sus
/// índices asignados, sin descartar storages ajenos.
pub(super) fn persistent_storage_records(state: &GameState) -> Result<Vec<Vec<u8>>, SavError> {
    let (industry_ids, station_ids, town_ids) = persistent_storage_ids(state)?;
    let mut storages = state.sav_persistent_storages.clone();
    let mut by_id = std::collections::HashMap::new();
    for (index, storage) in storages.iter().enumerate() {
        by_id.insert(storage.storage_id, index);
    }
    for (industry_index, industry) in state.industries.iter().enumerate() {
        let Some(storage_id) = industry_ids.get(industry_index).copied().flatten() else {
            continue;
        };
        merge_persistent_storage(
            &mut storages,
            &mut by_id,
            storage_id,
            industry.newgrf_persistent_storage_id,
            industry_persistent_storage_grfid(state, industry),
            &industry.newgrf_persistent_regs,
        );
    }
    for (station_index, station) in state.stations.iter().enumerate() {
        let Some(storage_id) = station_ids.get(station_index).copied().flatten() else {
            continue;
        };
        merge_persistent_storage(
            &mut storages,
            &mut by_id,
            storage_id,
            station.newgrf_persistent_storage_id,
            station_persistent_storage_grfid(state, station),
            &station.newgrf_persistent_regs,
        );
    }
    for (town_index, town) in state.towns.iter().enumerate() {
        let Some(ids) = town_ids.get(town_index) else {
            continue;
        };
        for (&grfid, &storage_id) in ids {
            let Some(regs) = town.newgrf_persistent_regs.get(&grfid) else {
                continue;
            };
            merge_persistent_storage(
                &mut storages,
                &mut by_id,
                storage_id,
                town.newgrf_persistent_storage_ids.get(&grfid).copied(),
                grfid,
                regs,
            );
        }
    }
    if storages.is_empty() {
        return Ok(Vec::new());
    }
    let max_id = storages
        .iter()
        .map(|storage| storage.storage_id)
        .max()
        .ok_or(SavError::BadFormat("PSAC sin filas".into()))?;
    let max_id = usize::try_from(max_id).map_err(|_| SavError::ValueOutOfRange {
        field: "persistent storage id",
        value: max_id,
    })?;
    if max_id >= (1 << 14) {
        return Err(SavError::ValueOutOfRange {
            field: "persistent storage id",
            value: u32::try_from(max_id).unwrap_or(u32::MAX),
        });
    }
    let mut records = vec![Vec::new(); max_id.saturating_add(1)];
    for storage in storages {
        let index = usize::try_from(storage.storage_id).map_err(|_| SavError::ValueOutOfRange {
            field: "persistent storage id",
            value: storage.storage_id,
        })?;
        let mut record = Vec::with_capacity(4 + 2 + 256 * 4);
        record.extend_from_slice(&storage.grfid.to_be_bytes());
        write_gamma(256, &mut record)?;
        for value in normalized_storage_values(&storage.storage) {
            record.extend_from_slice(&value.to_be_bytes());
        }
        records[index] = record;
    }
    Ok(records)
}

pub(super) fn persistent_storage_chunk(state: &GameState) -> Result<Option<Vec<u8>>, SavError> {
    let records = persistent_storage_records(state)?;
    if records.is_empty() {
        return Ok(None);
    }
    let mut header = Vec::new();
    append_field(&mut header, 6, "grfid")?;
    append_field(&mut header, 0x16, "storage")?;
    header.push(0);
    Ok(Some(raw_table_chunk(
        *b"PSAC", &header, &records, CH_TABLE,
    )?))
}

pub(super) fn indy_chunk(state: &GameState, map_w: u32) -> Result<Vec<u8>, SavError> {
    let records = indy_records_with_cargo(state, map_w)?;
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let mut header = Vec::new();
    append_indy_header(&mut header)?;
    raw_table_chunk(*b"INDY", &header, &records, CH_TABLE)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::sav::table::{SlValue, record_get};
    use crate::station::{Station, StopKind};

    #[test]
    fn stnn_header_starts_with_savebyte_facilities() {
        let mut header = Vec::new();
        append_stnn_header(&mut header).unwrap();
        assert_eq!(header[0], 2); // U8
        assert_eq!(&header[2..12], b"facilities");
        assert!(header.windows(6).any(|w| w == b"normal"));
        assert!(header.windows(5).any(|w| w == b"goods"));
    }

    #[test]
    fn stnn_rail_record_has_savebyte_and_goods64() {
        let mut state = GameState::new(64, 64);
        let mut rail = Station::new_with_kind(TileCoord::new(28, 39), StopKind::RailStation);
        rail.name = Some("Central".into());
        state.stations = vec![rail];
        let recs = stnn_records(&state, 64).unwrap();
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r[0], FACIL_TRAIN);
        assert_eq!(r[1], 1); // normal
        // goods count 64 aparece como gamma 64 = 0x40 tras always_accepted.
        assert!(r.windows(1).any(|w| w[0] == 64) || r.contains(&0x40));
        let chunk = stnn_chunk(&recs).unwrap();
        assert!(chunk.starts_with(b"STNN"));
        assert_eq!(chunk[4], CH_TABLE);
    }

    #[test]
    fn stnn_airport_record_preserves_custom_type_and_footprint() {
        let mut state = GameState::new(16, 16);
        let mut airport = Station::new_with_kind(TileCoord::new(7, 7), StopKind::Airport);
        airport.owner = crate::company::CompanyId::NONE;
        airport.ottd_station_id = Some(10);
        airport.airport_newgrf_spec_id = Some(10);
        airport.airport_layout = 3;
        airport.airport_rotation = 6;
        airport.newgrf_persistent_storage_id = Some(3);
        airport.airport_tiles = vec![
            TileCoord::new(4, 5),
            TileCoord::new(5, 5),
            TileCoord::new(4, 6),
            TileCoord::new(5, 6),
        ];
        state.stations.push(airport);

        let records = stnn_records(&state, 16).expect("STNN records");
        assert_eq!(records[0][0], FACIL_AIRPORT);
        let chunk = stnn_chunk(&records).expect("STNN chunk");
        let rows = crate::sav::table::parse_table_chunk(&chunk[5..], false).expect("table");
        let normal = match record_get(&rows[0].1, "normal") {
            Some(SlValue::Structs(items)) => items.first().expect("normal"),
            other => panic!("normal ausente: {other:?}"),
        };
        let base = match record_get(normal, "base") {
            Some(SlValue::Structs(items)) => items.first().expect("base"),
            other => panic!("base ausente: {other:?}"),
        };
        assert_eq!(
            record_get(base, "owner").and_then(SlValue::as_u64),
            Some(16)
        );
        assert_eq!(
            record_get(normal, "airport.type").and_then(SlValue::as_u64),
            Some(10)
        );
        assert_eq!(
            record_get(normal, "airport.tile").and_then(SlValue::as_u64),
            Some(84)
        );
        assert_eq!(
            record_get(normal, "airport.w").and_then(SlValue::as_u64),
            Some(2)
        );
        assert_eq!(
            record_get(normal, "airport.h").and_then(SlValue::as_u64),
            Some(2)
        );
        assert_eq!(
            record_get(normal, "airport.layout").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            record_get(normal, "airport.rotation").and_then(SlValue::as_u64),
            Some(6)
        );
        assert_eq!(
            record_get(normal, "airport.psa").and_then(SlValue::as_u64),
            Some(4)
        );
    }

    #[test]
    fn stnn_road_stop_record_emits_native_spec_and_tile_data() {
        let mut state = GameState::new(8, 8);
        let tile = TileCoord::new(3, 2);
        let mut raw = state.map.get(tile).unwrap();
        raw.kind = crate::map::TileKind::Station;
        raw.mapt = 0x50;
        raw.m2 = 0;
        raw.m6 = 3 << 3;
        state.map.set_tile(tile, raw).unwrap();

        let mut station = Station::new_with_kind(tile, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        let tile_state = station.ensure_road_stop_tile_state(tile);
        tile_state.spec = Some(7);
        tile_state.random_bits = 0xA5;
        tile_state.animation_frame = 6;
        state.road_stop_spec_catalog.push(crate::RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "Custom bus".into(),
            short_label: "CBUS".into(),
            stop_type: crate::road_stop_spec::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 0x4455_6677,
            newgrf_local_id: 0x12,
            newgrf_grf_version: 8,
            draw_mode: crate::road_stop_spec::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            callback_mask: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        });
        state.stations.push(station);

        let records = stnn_records(&state, 8).unwrap();
        let chunk = stnn_chunk(&records).unwrap();
        let rows = crate::sav::table::parse_table_chunk(&chunk[5..], false).unwrap();
        let record = &rows[0].1;
        let specs = record_get(record, "roadstopspeclist")
            .and_then(|value| match value {
                crate::sav::table::SlValue::Structs(items) => Some(items),
                _ => None,
            })
            .unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(
            record_get(&specs[1], "grfid").and_then(crate::sav::table::SlValue::as_u64),
            Some(0x4455_6677)
        );
        assert_eq!(
            record_get(&specs[1], "localidx").and_then(crate::sav::table::SlValue::as_u64),
            Some(0x12)
        );
        let tiles = record_get(record, "roadstoptiledata")
            .and_then(|value| match value {
                crate::sav::table::SlValue::Structs(items) => Some(items),
                _ => None,
            })
            .unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(
            record_get(&tiles[0], "tile").and_then(crate::sav::table::SlValue::as_u64),
            Some(19)
        );
        assert_eq!(
            record_get(&tiles[0], "random_bits").and_then(crate::sav::table::SlValue::as_u64),
            Some(0xA5)
        );
        assert_eq!(
            record_get(&tiles[0], "animation_frame").and_then(crate::sav::table::SlValue::as_u64),
            Some(6)
        );

        let export_map = map_with_road_stop_indices(&state, 8).unwrap();
        assert_eq!(export_map.get(tile).unwrap().m8 & 0x3F, 1);
    }

    #[test]
    fn indy_chunk_preserves_native_cargo_lists() {
        let mut state = GameState::new(8, 8);
        let mut industry = Industry::with_tiles_spec(
            TileCoord::new(3, 3),
            IndustryKind::Factory,
            IndustrySpec::Factory,
            vec![TileCoord::new(3, 3)],
            0,
        );
        industry.stock = 42;
        industry.selected_layout = 3;
        industry.newgrf_random = 0xBEEF;
        industry.last_prod_year = 1972;
        industry.was_cargo_delivered = true;
        industry.control_flags = 5;
        industry.neutral_station_id = Some(42);
        industry.exclusive_supplier = Some(crate::company::CompanyId(2));
        industry.founder = Some(crate::company::CompanyId(2));
        industry.construction_date = crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17;
        industry.construction_type = crate::industry::INDUSTRY_CONSTRUCTION_MAP_GENERATION;
        industry.add_accepted_cargo_waiting(CargoType::Livestock, 9);
        industry.set_last_accepted_date(CargoType::Livestock, 10_974);
        // Steel no es la salida legacy de la fábrica y debe viajar en la
        // lista adicional, no desaparecer ni sobrescribir `stock`.
        industry.add_newgrf_produced_cargo(CargoType::Steel, 7);
        let mut neutral_station =
            Station::new_with_kind(TileCoord::new(4, 3), crate::station::StopKind::Dock);
        neutral_station.ottd_station_id = Some(42);
        neutral_station.owner = crate::company::CompanyId::NONE;
        state.stations.push(neutral_station);
        state.industries.push(industry);
        state.sav_industry_histories.push(crate::sav::SavIndustry {
            industry_id: 0,
            pos: TileCoord::new(3, 3),
            width: 1,
            height: 1,
            neutral_station_id: None,
            industry_type: 3,
            random_colour: 0,
            counter: 0,
            selected_layout: 0,
            random: 0,
            last_prod_year: 0,
            was_cargo_delivered: false,
            control_flags: 0,
            exclusive_supplier: None,
            founder: None,
            construction_date: 0,
            construction_type: crate::industry::INDUSTRY_CONSTRUCTION_UNKNOWN,
            prod_level: crate::industry::PRODLEVEL_DEFAULT,
            valid_history: 0x55,
            persistent_storage_id: None,
            produced: vec![crate::sav::SavIndustryProducedCargo {
                cargo_slot: 5,
                waiting: 42,
                rate: 7,
                history: vec![
                    crate::sav::SavIndustryProducedHistory {
                        production: 321,
                        transported: 123,
                    },
                    crate::sav::SavIndustryProducedHistory {
                        production: 222,
                        transported: 111,
                    },
                ],
            }],
            accepted: vec![crate::sav::SavIndustryAcceptedCargo {
                cargo_slot: 4,
                waiting: 9,
                last_accepted: 10_974,
                accumulated_waiting: 88,
                history: vec![crate::sav::SavIndustryAcceptedHistory {
                    accepted: 12,
                    waiting: 7,
                }],
            }],
        });

        let chunk = indy_chunk(&state, 8).expect("INDY chunk");
        let rows = crate::sav::table::parse_table_chunk(&chunk[5..], false).expect("INDY table");
        let record = &rows[0].1;
        assert_eq!(
            crate::sav::table::record_get(record, "selected_layout")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "random")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(0xBEEF)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "valid_history")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(0x55)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "last_prod_year")
                .and_then(crate::sav::table::SlValue::as_i64),
            Some(1972)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "was_cargo_delivered")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(1)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "ctlflags")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(5)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "exclusive_supplier")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(2)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "neutral_station")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(43)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "founder")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(2)
        );
        assert_eq!(
            crate::sav::table::record_get(record, "construction_date")
                .and_then(crate::sav::table::SlValue::as_i64),
            Some(i64::from(
                crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17
            ))
        );
        assert_eq!(
            crate::sav::table::record_get(record, "construction_type")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(u64::from(
                crate::industry::INDUSTRY_CONSTRUCTION_MAP_GENERATION
            ))
        );
        let accepted = match crate::sav::table::record_get(record, "accepted") {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("accepted ausente: {other:?}"),
        };
        assert_eq!(accepted.len(), 1);
        assert_eq!(
            crate::sav::table::record_get(&accepted[0], "cargo")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(4)
        );
        assert_eq!(
            crate::sav::table::record_get(&accepted[0], "waiting")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(9)
        );
        assert_eq!(
            crate::sav::table::record_get(&accepted[0], "last_accepted")
                .and_then(crate::sav::table::SlValue::as_i64),
            Some(10_974)
        );
        assert_eq!(
            crate::sav::table::record_get(&accepted[0], "accumulated_waiting")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(88)
        );
        let accepted_history = crate::sav::table::record_get(&accepted[0], "history");
        let accepted_history = match accepted_history {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("accepted history ausente: {other:?}"),
        };
        assert_eq!(accepted_history.len(), 1);
        assert_eq!(
            crate::sav::table::record_get(&accepted_history[0], "accepted")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(12)
        );

        let produced = match crate::sav::table::record_get(record, "produced") {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("produced ausente: {other:?}"),
        };
        assert_eq!(produced.len(), 2);
        assert!(produced.iter().any(|entry| {
            crate::sav::table::record_get(entry, "cargo")
                .and_then(crate::sav::table::SlValue::as_u64)
                == Some(5)
                && crate::sav::table::record_get(entry, "waiting")
                    .and_then(crate::sav::table::SlValue::as_u64)
                    == Some(42)
        }));
        assert!(produced.iter().any(|entry| {
            crate::sav::table::record_get(entry, "cargo")
                .and_then(crate::sav::table::SlValue::as_u64)
                == Some(9)
                && crate::sav::table::record_get(entry, "waiting")
                    .and_then(crate::sav::table::SlValue::as_u64)
                    == Some(7)
        }));
        let history = produced
            .iter()
            .find(|entry| {
                crate::sav::table::record_get(entry, "cargo")
                    .and_then(crate::sav::table::SlValue::as_u64)
                    == Some(5)
            })
            .and_then(|entry| crate::sav::table::record_get(entry, "history"));
        let history = match history {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("history ausente: {other:?}"),
        };
        assert_eq!(history.len(), 2);
        assert_eq!(
            crate::sav::table::record_get(&history[0], "production")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(321)
        );
        assert_eq!(
            crate::sav::table::record_get(&history[1], "transported")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(111)
        );
    }

    #[test]
    fn psac_chunk_and_indy_reference_preserve_sparse_storage_id() {
        let mut state = GameState::new(8, 8);
        let mut industry = Industry::new(TileCoord::new(3, 3), IndustryKind::Factory);
        industry.newgrf_persistent_storage_id = Some(3);
        industry.newgrf_persistent_regs.insert(7, 0xDEAD_BEEF);
        state.industries.push(industry);

        let indy = indy_chunk(&state, 8).expect("INDY chunk");
        let indy_rows =
            crate::sav::table::parse_table_chunk(&indy[5..], false).expect("INDY table");
        assert_eq!(
            crate::sav::table::record_get(&indy_rows[0].1, "psa")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(4)
        );

        let psac = persistent_storage_chunk(&state)
            .expect("PSAC result")
            .expect("PSAC chunk");
        let rows = crate::sav::table::parse_table_chunk(&psac[5..], false).expect("PSAC table");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, 3);
        let values = match crate::sav::table::record_get(&rows[0].1, "storage") {
            Some(crate::sav::table::SlValue::List(values)) => values,
            other => panic!("storage ausente: {other:?}"),
        };
        assert_eq!(values.len(), 256);
        assert_eq!(values[7].as_u64(), Some(u64::from(0xDEAD_BEEF_u32)));
    }

    #[test]
    fn indy_chunk_emits_runtime_accepted_history() {
        let mut state = GameState::new(8, 8);
        let mut industry = Industry::new(TileCoord::new(2, 2), IndustryKind::Factory);
        industry.record_accepted_cargo(CargoType::Livestock, 12, 10_974);
        industry
            .accepted_accumulated_waiting
            .set(CargoType::Livestock, 88);
        industry.valid_history = 0x55;
        state.industries.push(industry);

        let indy = indy_chunk(&state, 8).expect("INDY chunk");
        let rows = crate::sav::table::parse_table_chunk(&indy[5..], false).expect("INDY table");
        let record = &rows[0].1;
        assert_eq!(
            crate::sav::table::record_get(record, "valid_history")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(0x55)
        );
        let accepted = match crate::sav::table::record_get(record, "accepted") {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("accepted ausente: {other:?}"),
        };
        assert_eq!(accepted.len(), 1);
        assert_eq!(
            crate::sav::table::record_get(&accepted[0], "accumulated_waiting")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(88)
        );
        let history = match crate::sav::table::record_get(&accepted[0], "history") {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("accepted history ausente: {other:?}"),
        };
        assert_eq!(history.len(), 1);
        assert_eq!(
            crate::sav::table::record_get(&history[0], "accepted")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(12)
        );
    }

    #[test]
    fn indy_chunk_emits_runtime_produced_history() {
        let mut state = GameState::new(8, 8);
        let mut industry = Industry::new(TileCoord::new(2, 2), IndustryKind::CoalMine);
        industry.record_produced_cargo(CargoType::Coal, 31, 11);
        industry.rollover_accepted_history();
        industry.record_produced_cargo(CargoType::Coal, 7, 5);
        state.industries.push(industry);

        let indy = indy_chunk(&state, 8).expect("INDY chunk");
        let rows = crate::sav::table::parse_table_chunk(&indy[5..], false).expect("INDY table");
        let produced = match crate::sav::table::record_get(&rows[0].1, "produced") {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("produced ausente: {other:?}"),
        };
        let coal = produced
            .iter()
            .find(|entry| {
                crate::sav::table::record_get(entry, "cargo")
                    .and_then(crate::sav::table::SlValue::as_u64)
                    == Some(1)
            })
            .expect("coal output");
        let history = match crate::sav::table::record_get(coal, "history") {
            Some(crate::sav::table::SlValue::Structs(items)) => items,
            other => panic!("produced history ausente: {other:?}"),
        };
        assert_eq!(history.len(), 2);
        assert_eq!(
            crate::sav::table::record_get(&history[0], "production")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(7)
        );
        assert_eq!(
            crate::sav::table::record_get(&history[0], "transported")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(5)
        );
        assert_eq!(
            crate::sav::table::record_get(&history[1], "production")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(31)
        );
    }
}
