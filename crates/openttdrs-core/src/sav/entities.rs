//! Estaciones (`STNN`), ciudades (`CITY`), industrias (`INDY`), vehículos
//! (`VEHS`) y empresas (`PLYR`) desde tablas autodescriptivas.

use crate::map::{TileCoord, coord_from_linear_index};
use crate::town::Town;
use std::collections::HashMap;

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlRecord, SlValue, record_get};

/// Flag de waypoint en `BaseStation::facilities` (no es una estación jugable).
const FACIL_WAYPOINT: u64 = 0x80;

/// Instancia persistida del pool `Object` (`OBJS`).
///
/// `OpenTTD` utiliza el índice de la tabla como `ObjectID` y guarda la
/// ubicación/huella separadas del tipo de objeto. Mantener ese índice es
/// importante: las teselas `MP_OBJECT` sólo contienen los 24 bits del ID y lo
/// consultan al dibujar y al ejecutar callbacks `NewGRF`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SavObject {
    /// Índice del pool (`ObjectID`).
    pub object_id: u32,
    /// Tesela de origen (`Object::location.tile`).
    pub tile: TileCoord,
    /// Ancho y alto guardados (ya rotados según la orientación elegida).
    pub width: u16,
    pub height: u16,
    /// Referencia serializada a `TownID` (`0` = inválida, resto = índice + 1).
    /// Se conserva cruda para no confundir un ID de town ausente con uno real.
    pub town: u32,
    /// Fecha de construcción (`TimerGameEconomy::date`).
    pub build_date: u32,
    /// Color de la instancia (`Colours`).
    pub colour: u8,
    /// Vista/rotación seleccionada por `Object::view`.
    pub view: u8,
    /// Tipo de objeto (`ObjectType`).
    pub object_type: u16,
}

/// Mapeo `ObjectType` ↔ identidad `(GRFID, local ID)` del chunk `OBID`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SavObjectMapping {
    /// Índice del `ObjectType` asignado por `ObjectOverrideManager`.
    pub object_type: u16,
    pub grfid: u32,
    pub entity_id: u16,
    pub substitute_id: u16,
}

/// Estación decodificada del save (posición + nombre custom + facilities).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavStation {
    /// `StationID` de `OpenTTD` (clave real de la tabla `STNN`, no el índice
    /// del `Vec`); necesario para correlacionar con oráculos/trazas externas.
    pub station_id: u32,
    pub pos: TileCoord,
    /// Nombre puesto por el jugador; `None` si usa nombre generado.
    pub name: Option<String>,
    /// Bits `FACIL_*` de `OpenTTD` (1 tren, 2 camión, 4 bus, 8 aeropuerto, 0x10 muelle).
    pub facilities: u8,
    /// `BaseStation::string_id` (`STR_SV_STNAME_*`) cuando no hay nombre custom.
    pub string_id: Option<u16>,
    /// Índice de ciudad (`BaseStation::town`) para armar el nombre generado.
    pub town_id: Option<u32>,
    /// `Station::airport.type` (`AT_*` de `OpenTTD`); solo válido si `facilities` trae `FACIL_AIRPORT`.
    pub airport_type: u8,
    /// `Station::airport.w` / `airport.h` (footprint en teselas).
    pub airport_w: u16,
    pub airport_h: u16,
    /// `Station::airport.layout`.
    pub airport_layout: u8,
    /// `Station::airport.rotation` (`Direction`, bits 1..2).
    pub airport_rotation: u8,
    /// `Station::airport.blocks` (guardado como `airport.flags`).
    pub airport_blocks: u64,
    /// Paquetes de carga en espera, agrupados por slot de cargo de `OpenTTD`.
    ///
    /// La carga física vive en `CAPA`; `STNN.goods[].cargo[]` sólo conserva
    /// referencias a esos paquetes. Se mantiene esta relación para hidratar
    /// el estado core sin que el cliente tenga que releer chunks crudos.
    pub cargo: Vec<SavStationCargo>,
}

/// Identidad estable de una spec de road stop en la lista nativa de una
/// estación (`STNN.roadstopspeclist`). El índice dentro del vector es el que
/// guarda cada tesela en los seis bits bajos de `m8`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavRoadStopSpecMapping {
    pub grfid: u32,
    /// `OpenTTD` serializa el local ID como `uint16` desde `SLV_EXTEND_ENTITY_MAPPING`.
    pub localidx: u16,
}

/// Estado nativo por tesela de una parada vial (`STNN.roadstoptiledata`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavRoadStopTileData {
    pub tile: TileCoord,
    pub random_bits: u8,
    pub animation_frame: u8,
}

/// Datos de road stops agrupados por `StationID`, separados de [`SavStation`]
/// para no romper fixtures que construyen estaciones sintéticas a mano.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavRoadStopStationData {
    pub specs: Vec<SavRoadStopSpecMapping>,
    pub tiles: Vec<SavRoadStopTileData>,
}

/// Referencias de una entrada `Station::goods[cargo_slot]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavStationCargo {
    /// Slot de cargo del landscape activo (`0..11` para vanilla).
    pub cargo_slot: u8,
    /// IDs de `CargoPacket` (`CAPA`) en el mismo orden FIFO que el save.
    pub packet_ids: Vec<u32>,
    /// Unidades reservadas para un vehículo que ya inició la carga.
    pub reserved: u32,
}

/// `CargoPacket` decodificado del chunk `CAPA`.
///
/// `CAPA` no almacena el tipo de carga: lo aporta la entrada `goods` de la
/// estación que lo referencia. Los IDs de estación se resuelven recién al
/// convertir a [`crate::GameState`], cuando ya existe el índice de posiciones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavCargoPacket {
    pub packet_id: u32,
    /// `CargoPacket::first_station` (guardado bajo el nombre `source`).
    pub source_station_id: Option<u32>,
    /// Tesela geográfica de origen para el cálculo económico.
    pub source_xy: Option<TileCoord>,
    /// `CargoPacket::next_hop` (nombre histórico `loaded_at_xy`).
    pub next_hop_station_id: Option<u32>,
    pub count: u16,
    pub periods_in_transit: u16,
    pub feeder_share: i64,
    /// Tipo/ID del productor nativo (`SourceType`/`SourceID`).
    pub source_type: u8,
    pub source_id: Option<u16>,
    /// Vector `CargoPacket::travelled` usado para el cálculo de pago.
    pub travelled_x: i16,
    pub travelled_y: i16,
}

/// Entrada del índice de estación (`StationID`) en `STNN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavStationIndex {
    pub pos: TileCoord,
    pub is_waypoint: bool,
    pub facilities: u8,
    pub name: Option<String>,
    pub string_id: Option<u16>,
    pub town_id: Option<u32>,
    pub airport_type: u8,
    pub airport_w: u16,
    pub airport_h: u16,
    pub airport_layout: u8,
    pub airport_rotation: u8,
    pub airport_blocks: u64,
}

/// Primer (y único) registro de un campo struct de tabla.
fn nested_struct<'a>(record: &'a SlRecord, name: &str) -> Option<&'a SlRecord> {
    match record_get(record, name)? {
        SlValue::Structs(items) => items.first(),
        _ => None,
    }
}

/// Base de estación: top-level en saves legacy, anidada en `normal.base` o
/// `waypoint.base` en las tablas `STNN` modernas.
fn station_base_record(record: &SlRecord) -> &SlRecord {
    for station_kind in ["normal", "waypoint"] {
        if let Some(kind) = nested_struct(record, station_kind)
            && let Some(base) = nested_struct(kind, "base")
        {
            return base;
        }
    }
    record
}

/// Mapa `StationID` → tesela para resolver destinos de órdenes.
#[must_use]
pub(crate) fn station_index_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> std::collections::HashMap<u32, SavStationIndex> {
    let Some(stnn) = find_chunk(chunks, "STNN") else {
        return std::collections::HashMap::new();
    };
    let mut out = std::collections::HashMap::new();
    for (idx, record) in table_rows(stnn, save_version) {
        let base = station_base_record(&record);
        let Some(xy) = record_get(base, "xy").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = coord_from_linear_index(xy, map_w) else {
            continue;
        };
        let facilities = record_get(base, "facilities")
            .and_then(SlValue::as_u64)
            .or_else(|| record_get(&record, "facilities").and_then(SlValue::as_u64))
            .unwrap_or(0);
        let is_waypoint = facilities & FACIL_WAYPOINT != 0;
        let name = record_get(base, "name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let string_id = record_get(base, "string_id")
            .and_then(SlValue::as_u64)
            .and_then(|v| u16::try_from(v).ok());
        // `SLE_REF(..., REF_TOWN)`: 0 = null, resto = `Town::index + 1`.
        #[allow(clippy::cast_possible_truncation)]
        let town_id = record_get(base, "town")
            .and_then(SlValue::as_u64)
            .and_then(|v| (v > 0).then_some((v - 1) as u32));
        // Campos `airport.*` solo existen bajo el struct `normal` (ausentes en waypoints).
        let normal = nested_struct(&record, "normal");
        let airport_type = normal
            .and_then(|n| record_get(n, "airport.type"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let airport_w = normal
            .and_then(|n| record_get(n, "airport.w"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let airport_h = normal
            .and_then(|n| record_get(n, "airport.h"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let airport_layout = normal
            .and_then(|n| record_get(n, "airport.layout"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let airport_rotation = normal
            .and_then(|n| record_get(n, "airport.rotation"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        // Guardado en disco como `airport.flags` (`SLE_VARNAME`); `Station::airport.blocks` en memoria.
        let airport_blocks = normal
            .and_then(|n| record_get(n, "airport.flags"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        #[allow(clippy::cast_possible_truncation)]
        out.insert(
            idx,
            SavStationIndex {
                pos,
                is_waypoint,
                facilities: facilities as u8,
                name,
                string_id,
                town_id,
                airport_type: airport_type as u8,
                airport_w: airport_w as u16,
                airport_h: airport_h as u16,
                airport_layout: airport_layout as u8,
                airport_rotation: airport_rotation as u8,
                airport_blocks,
            },
        );
    }
    out
}

fn table_rows(chunk: &RawChunk, save_version: u16) -> Vec<(u32, super::table::SlRecord)> {
    super::array_legacy::chunk_rows(chunk, save_version)
}

/// Extrae las instancias del pool `Object` (`OBJS`) de saves modernos.
///
/// El índice denso de la tabla es el `ObjectID`; no se debe sustituir por la
/// posición de la tesela porque un mismo objeto ocupa varias teselas y el ID
/// puede tener huecos. Los campos que aparezcan en versiones futuras se
/// ignoran mediante el parser autodescriptivo, conservando el chunk crudo para
/// reexportarlo cuando la instancia no fue modificada.
#[must_use]
pub(crate) fn objects_from_chunks(chunks: &[RawChunk], map_w: u32, map_h: u32) -> Vec<SavObject> {
    let Some(objs) = find_chunk(chunks, "OBJS") else {
        return Vec::new();
    };
    if !matches!(
        objs.ch_type,
        super::chunks::CH_TABLE | super::chunks::CH_SPARSE_TABLE
    ) {
        return Vec::new();
    }
    let sparse = objs.ch_type == super::chunks::CH_SPARSE_TABLE;
    let Ok(rows) = super::table::parse_table_chunk(&objs.body, sparse) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter_map(|(object_id, record)| {
            let linear = record_get(&record, "location.tile")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())?;
            let tile = coord_from_linear_index(u64::from(linear), map_w)?;
            let in_bounds = tile.x >= 0
                && tile.y >= 0
                && u32::try_from(tile.x).ok()? < map_w
                && u32::try_from(tile.y).ok()? < map_h;
            if !in_bounds {
                return None;
            }
            // Width/height are `SLE_FILE_U8 | SLE_VAR_U16`: the current table
            // header therefore exposes them as U16. Old saves without the
            // fields are valid and represent the vanilla 1×1 footprint.
            let width = record_get(&record, "location.w")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|&value| value > 0)
                .unwrap_or(1);
            let height = record_get(&record, "location.h")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|&value| value > 0)
                .unwrap_or(1);
            let town = record_get(&record, "town")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(0);
            let build_date = record_get(&record, "build_date")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(0);
            let colour = record_get(&record, "colour")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0);
            let view = record_get(&record, "view")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0);
            let object_type = record_get(&record, "type")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0);
            // Empty pool slots are represented by a zero-length record and
            // were already skipped by `parse_table_chunk`; `type == 0xFFFF`
            // is the explicit invalid marker used by some converted saves.
            (object_type != u16::MAX).then_some(SavObject {
                object_id,
                tile,
                width,
                height,
                town,
                build_date,
                colour,
                view,
                object_type,
            })
        })
        .collect()
}

/// Extrae el mapping `NewGRF` de objetos (`OBID`).
#[must_use]
pub(crate) fn object_mappings_from_chunks(chunks: &[RawChunk]) -> Vec<SavObjectMapping> {
    let Some(obid) = find_chunk(chunks, "OBID") else {
        return Vec::new();
    };
    if !matches!(
        obid.ch_type,
        super::chunks::CH_TABLE | super::chunks::CH_SPARSE_TABLE
    ) {
        return Vec::new();
    }
    let sparse = obid.ch_type == super::chunks::CH_SPARSE_TABLE;
    let Ok(rows) = super::table::parse_table_chunk(&obid.body, sparse) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter_map(|(object_type, record)| {
            let object_type = u16::try_from(object_type).ok()?;
            let grfid = record_get(&record, "grfid")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())?;
            let entity_id = record_get(&record, "entity_id")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())?;
            let substitute_id = record_get(&record, "substitute_id")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0);
            (grfid != 0 || entity_id != 0 || substitute_id != 0).then_some(SavObjectMapping {
                object_type,
                grfid,
                entity_id,
                substitute_id,
            })
        })
        .collect()
}

/// Estaciones del chunk `STNN`; best-effort (tabla o array legacy).
#[must_use]
pub(crate) fn stations_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> Vec<SavStation> {
    let mut cargo_by_station = station_cargo_from_chunks(chunks, save_version);
    let mut indexed: Vec<_> = station_index_from_chunks(chunks, map_w, save_version)
        .into_iter()
        .filter(|(_, st)| !st.is_waypoint)
        .collect();
    // Orden determinístico por `StationID` (el `HashMap` no lo garantiza) y
    // para que el `Vec` resultante quede alineado con el índice real del save.
    indexed.sort_by_key(|(idx, _)| *idx);
    indexed
        .into_iter()
        .map(|(station_id, st)| SavStation {
            station_id,
            pos: st.pos,
            name: st.name,
            facilities: st.facilities,
            string_id: st.string_id,
            town_id: st.town_id,
            airport_type: st.airport_type,
            airport_w: st.airport_w,
            airport_h: st.airport_h,
            airport_layout: st.airport_layout,
            airport_rotation: st.airport_rotation,
            airport_blocks: st.airport_blocks,
            cargo: cargo_by_station.remove(&station_id).unwrap_or_default(),
        })
        .collect()
}

/// Extrae la tabla nativa de specs y el estado por tesela de road stops.
/// `STNN` moderno guarda ambos vectores al nivel superior del record de la
/// estación; en saves legacy la ausencia de cualquiera de ellos es válida.
pub(crate) fn road_stop_station_data_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> HashMap<u32, SavRoadStopStationData> {
    let Some(stnn) = find_chunk(chunks, "STNN") else {
        return HashMap::new();
    };
    table_rows(stnn, save_version)
        .into_iter()
        .filter_map(|(station_id, record)| {
            let specs: Vec<SavRoadStopSpecMapping> = record_get(&record, "roadstopspeclist")
                .and_then(|value| match value {
                    SlValue::Structs(entries) => Some(
                        entries
                            .iter()
                            .filter_map(|entry| {
                                Some(SavRoadStopSpecMapping {
                                    grfid: u32::try_from(record_get(entry, "grfid")?.as_u64()?)
                                        .ok()?,
                                    localidx: u16::try_from(
                                        record_get(entry, "localidx")?.as_u64()?,
                                    )
                                    .ok()?,
                                })
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            let tiles: Vec<SavRoadStopTileData> = record_get(&record, "roadstoptiledata")
                .and_then(|value| match value {
                    SlValue::Structs(entries) => Some(
                        entries
                            .iter()
                            .filter_map(|entry| {
                                let tile = record_get(entry, "tile")
                                    .and_then(SlValue::as_u64)
                                    .and_then(|value| coord_from_linear_index(value, map_w))?;
                                Some(SavRoadStopTileData {
                                    tile,
                                    random_bits: u8::try_from(
                                        record_get(entry, "random_bits")?.as_u64()?,
                                    )
                                    .ok()?,
                                    animation_frame: u8::try_from(
                                        record_get(entry, "animation_frame")?.as_u64()?,
                                    )
                                    .ok()?,
                                })
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            if specs.is_empty() && tiles.is_empty() {
                None
            } else {
                Some((station_id, SavRoadStopStationData { specs, tiles }))
            }
        })
        .collect()
}

/// Extrae las referencias `STNN.normal.goods[].cargo[]` por estación.
///
/// La lista de `goods` se serializa por slot de cargo, por eso el índice del
/// struct-list es el `cargo_slot` a resolver según el landscape del save.
fn station_cargo_from_chunks(
    chunks: &[RawChunk],
    save_version: u16,
) -> HashMap<u32, Vec<SavStationCargo>> {
    let Some(stnn) = find_chunk(chunks, "STNN") else {
        return HashMap::new();
    };
    table_rows(stnn, save_version)
        .into_iter()
        .filter_map(|(station_id, record)| {
            let cargo = station_cargo_from_record(&record);
            (!cargo.is_empty()).then_some((station_id, cargo))
        })
        .collect()
}

fn station_cargo_from_record(record: &SlRecord) -> Vec<SavStationCargo> {
    let goods = nested_struct(record, "normal")
        .and_then(|normal| record_get(normal, "goods"))
        .or_else(|| record_get(record, "goods"));
    let Some(SlValue::Structs(goods)) = goods else {
        return Vec::new();
    };

    goods
        .iter()
        .enumerate()
        .filter_map(|(slot, entry)| {
            let cargo_slot = u8::try_from(slot).ok()?;
            let reserved = record_get(entry, "cargo.reserved_count")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(0);
            let mut packet_ids = Vec::new();
            if let Some(SlValue::Structs(destinations)) = record_get(entry, "cargo") {
                for destination in destinations {
                    let Some(SlValue::List(refs)) = record_get(destination, "second") else {
                        continue;
                    };
                    // `SLE_REFLIST` codifica referencias como `index + 1`;
                    // cero representa una referencia nula y no es un paquete.
                    packet_ids.extend(refs.iter().filter_map(|value| {
                        value
                            .as_u64()
                            .and_then(|reference| reference.checked_sub(1))
                            .and_then(|id| u32::try_from(id).ok())
                    }));
                }
            }
            (!packet_ids.is_empty() || reserved != 0).then_some(SavStationCargo {
                cargo_slot,
                packet_ids,
                reserved,
            })
        })
        .collect()
}

/// Paquetes físicos del chunk `CAPA` (saves modernos con `CargoPackets`).
#[must_use]
pub(crate) fn cargo_packets_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> Vec<SavCargoPacket> {
    let Some(capa) = find_chunk(chunks, "CAPA") else {
        return Vec::new();
    };
    table_rows(capa, save_version)
        .into_iter()
        .filter_map(|(packet_id, record)| sav_cargo_packet_from_record(packet_id, &record, map_w))
        .collect()
}

fn sav_cargo_packet_from_record(
    packet_id: u32,
    record: &SlRecord,
    map_w: u32,
) -> Option<SavCargoPacket> {
    let count = record_get(record, "count")
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())?;
    let source_xy = record_get(record, "source_xy")
        .and_then(SlValue::as_u64)
        .and_then(|tile| coord_from_linear_index(tile, map_w));
    Some(SavCargoPacket {
        packet_id,
        source_station_id: station_id_from_scalar(record_get(record, "source")),
        source_xy,
        next_hop_station_id: station_id_from_scalar(record_get(record, "loaded_at_xy")),
        count,
        periods_in_transit: record_get(record, "periods_in_transit")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0),
        feeder_share: record_get(record, "feeder_share")
            .and_then(SlValue::as_i64)
            .unwrap_or(0),
        source_type: record_get(record, "source_type")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0),
        source_id: record_get(record, "source_id")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != u16::MAX),
        travelled_x: record_get(record, "travelled.x")
            .and_then(SlValue::as_i64)
            .and_then(|value| i16::try_from(value).ok())
            .unwrap_or(0),
        travelled_y: record_get(record, "travelled.y")
            .and_then(SlValue::as_i64)
            .and_then(|value| i16::try_from(value).ok())
            .unwrap_or(0),
    })
}

/// `StationID` se guarda como `u16` directo (no como `SLE_REF`); `0xFFFF`
/// es `INVALID_STATION`, mientras que el ID 0 es válido.
fn station_id_from_scalar(value: Option<&SlValue>) -> Option<u32> {
    value
        .and_then(SlValue::as_u64)
        .filter(|&id| id < u64::from(u16::MAX))
        .and_then(|id| u32::try_from(id).ok())
}

/// `STR_SV_STNAME` … `STR_SV_STNAME_FALLBACK` (`table/strings.h` de `OpenTTD`).
const STR_SV_STNAME: u16 = 0x6006;
const STR_SV_STNAME_FALLBACK: u16 = 0x6027;

/// Nombre de estación generado (plantillas españolas `STR_SV_STNAME_*`).
#[must_use]
pub fn format_generated_station_name(string_id: u16, town_name: &str) -> Option<String> {
    if !(STR_SV_STNAME..=STR_SV_STNAME_FALLBACK).contains(&string_id) {
        return None;
    }
    let t = town_name;
    let formatted = match string_id {
        0x6006 | 0x6017 | 0x6018 => t.to_string(), // BASE / BUOY / WAYPOINT
        0x6007 => format!("{t} Norte"),            // NORTH
        0x6008 => format!("{t} Sur"),              // SOUTH
        0x6009 => format!("{t} Este"),             // EAST
        0x600A => format!("{t} Oeste"),            // WEST
        0x600B => format!("{t} Central"),          // CENTRAL
        0x600C => format!("{t} Transferencia"),    // TRANSFER
        0x600D => format!("{t} Parada"),           // HALT
        0x600E => format!("Valle de {t}"),         // VALLEY
        0x600F => format!("Cumbres de {t}"),       // HEIGHTS
        0x6010 => format!("Arboleda de {t}"),      // WOODS
        0x6011 => format!("Lago de {t}"),          // LAKESIDE
        0x6012 => format!("{t} Intercambio"),      // EXCHANGE
        0x6013 => format!("Aeropuerto de {t}"),    // AIRPORT
        0x6014 => format!("Campo petrolífero de {t}"), // OILFIELD
        0x6015 => format!("Minas de {t}"),         // MINES
        0x6016 => format!("Muelles de {t}"),       // DOCKS
        0x6020 => format!("{t} Anexo"),            // ANNEXE
        0x6021 => format!("Proximidades de {t}"),  // SIDINGS
        0x6022 => format!("Ramal de {t}"),         // BRANCH
        0x6023 => format!("{t} Alto"),             // UPPER
        0x6024 => format!("{t} Bajo"),             // LOWER
        0x6025 => format!("Helipuerto de {t}"),    // HELIPORT
        0x6026 => format!("Bosque de {t}"),        // FOREST
        0x6027 => format!("{t} Estación"),         // FALLBACK (sin #{NUM})
        _ => return None,
    };
    Some(formatted)
}

/// Resuelve el nombre visible: custom, o plantilla `string_id` + ciudad.
#[must_use]
pub fn resolve_sav_station_name(station: &SavStation, towns: &[Town]) -> Option<String> {
    if let Some(name) = station.name.as_ref().filter(|n| !n.is_empty()) {
        return Some(name.clone());
    }
    let string_id = station.string_id?;
    let town_name = station
        .town_id
        .and_then(|id| towns.iter().find(|t| t.id == id))
        .map(|t| t.name.as_str())
        .filter(|n| !n.is_empty())?;
    format_generated_station_name(string_id, town_name)
}

/// Nombre generado con el generador nativo de `OpenTTD` a partir de los
/// campos `townnamegrfid`/`townnametype`/`townnameparts` del record.
fn generated_town_name(record: &super::table::SlRecord) -> Option<String> {
    let grfid = record_get(record, "townnamegrfid")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    let name_type = record_get(record, "townnametype").and_then(SlValue::as_u64)?;
    let parts = record_get(record, "townnameparts").and_then(SlValue::as_u64)?;
    crate::townname::town_name_from_save(
        u32::try_from(grfid).ok()?,
        u16::try_from(name_type).ok()?,
        u32::try_from(parts).ok()?,
    )
}

/// Ciudades del chunk `CITY` (tabla); nombre custom, nombre generado con el
/// generador nativo de `OpenTTD`, o «Ciudad N» como último recurso.
///
/// La población **no** viene en el save: `OpenTTD` la reconstruye al cargar
/// (`RebuildTownCaches`); ver `sav::rebuild_town_populations`.
#[must_use]
pub(crate) fn towns_from_chunks(chunks: &[RawChunk], map_w: u32, save_version: u16) -> Vec<Town> {
    let Some(city) = find_chunk(chunks, "CITY") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (idx, record) in table_rows(city, save_version) {
        let Some(xy) = record_get(&record, "xy").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = coord_from_linear_index(xy, map_w) else {
            continue;
        };
        let name = record_get(&record, "name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| generated_town_name(&record))
            .unwrap_or_else(|| format!("Ciudad {}", idx + 1));
        out.push(Town {
            id: idx,
            pos,
            name,
            population: 0,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        });
    }
    out
}

/// Industria decodificada del chunk `INDY` (saves con tablas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavIndustry {
    /// `IndustryID` (índice de la fila `INDY`), correlacionable con `m2`.
    pub industry_id: u32,
    /// Tesela de origen (`location.tile`).
    pub pos: TileCoord,
    /// Dimensiones del rectángulo (`location.w` × `location.h`).
    pub width: u8,
    pub height: u8,
    /// `IndustryType` de `OpenTTD` (índice en la tabla de specs).
    pub industry_type: u8,
    /// `Industry.random_colour` (`Colours`, 0–15) para `PALETTE_MODIFIER_COLOUR`.
    pub random_colour: u8,
    /// Fase exacta del ciclo de producción (`Industry::counter`).
    pub counter: u16,
    /// Layout elegido al fundar (`Industry::selected_layout`, `SLV_73`).
    pub selected_layout: u8,
    /// Bits aleatorios persistentes de la industria (`Industry::random`, `SLV_82`).
    pub random: u16,
    /// Último año económico con producción (`Industry::last_prod_year`).
    pub last_prod_year: u32,
    /// `Industry::was_cargo_delivered`; se conserva como booleano para no
    /// confundir el flag nativo con un contador.
    pub was_cargo_delivered: bool,
    /// Flags opacos de `GameScript` (`Industry::ctlflags`).
    pub control_flags: u8,
    /// Fundador serializado como `CompanyID`; `None` representa
    /// `INVALID_OWNER` (por ejemplo, una industria generada en el mapa).
    pub founder: Option<u8>,
    /// Fecha absoluta de construcción (`TimerGameCalendar::Date`).
    pub construction_date: u32,
    /// `IndustryConstructionType` (`ICT_*`).
    pub construction_type: u8,
    /// Nivel de producción (`Industry::prod_level`).
    pub prod_level: u8,
    /// Salidas y stock en espera (`Industry::produced`).
    pub produced: Vec<SavIndustryProducedCargo>,
    /// Insumos recibidos y en espera (`Industry::accepted`).
    pub accepted: Vec<SavIndustryAcceptedCargo>,
}

/// Entrada `Industry::produced` de `OpenTTD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavIndustryProducedCargo {
    pub cargo_slot: u8,
    pub waiting: u16,
    pub rate: u8,
}

/// Entrada `Industry::accepted` de `OpenTTD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavIndustryAcceptedCargo {
    pub cargo_slot: u8,
    pub waiting: u16,
    /// Último día económico absoluto (`Industry::AcceptedCargo::last_accepted`).
    pub last_accepted: u32,
}

/// Industrias del chunk `INDY` (solo saves con tablas); best-effort.
#[must_use]
pub(crate) fn industries_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> Vec<SavIndustry> {
    let Some(indy) = find_chunk(chunks, "INDY") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (industry_id, record) in table_rows(indy, save_version) {
        if let Some(ind) = sav_industry_from_record(industry_id, &record, map_w) {
            out.push(ind);
        }
    }
    if out.is_empty() {
        for &(index, industry_type) in &super::build::indy_pairs(chunks) {
            if let Some(pos) = coord_from_linear_index(u64::from(index), map_w) {
                out.push(SavIndustry {
                    industry_id: u32::from(index),
                    pos,
                    width: 1,
                    height: 1,
                    industry_type,
                    random_colour: 0,
                    counter: 0,
                    selected_layout: 0,
                    random: 0,
                    last_prod_year: 0,
                    was_cargo_delivered: false,
                    control_flags: 0,
                    founder: None,
                    construction_date: 0,
                    construction_type: crate::industry::INDUSTRY_CONSTRUCTION_UNKNOWN,
                    prod_level: crate::industry::PRODLEVEL_DEFAULT,
                    produced: Vec::new(),
                    accepted: Vec::new(),
                });
            }
        }
    }
    out
}

fn sav_industry_from_record(
    industry_id: u32,
    record: &SlRecord,
    map_w: u32,
) -> Option<SavIndustry> {
    let tile = record_get(record, "location.tile").and_then(SlValue::as_u64)?;
    let pos = coord_from_linear_index(tile, map_w)?;
    let width = record_get(record, "location.w")
        .and_then(SlValue::as_u64)
        .unwrap_or(1);
    let height = record_get(record, "location.h")
        .and_then(SlValue::as_u64)
        .unwrap_or(1);
    let industry_type = record_get(record, "type")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    let random_colour = record_get(record, "random_colour")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    let counter = record_get(record, "counter")
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(0);
    let selected_layout = record_get(record, "selected_layout")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0);
    let random = record_get(record, "random")
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(0);
    let last_prod_year = record_get(record, "last_prod_year")
        .and_then(SlValue::as_i64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let was_cargo_delivered = record_get(record, "was_cargo_delivered")
        .and_then(SlValue::as_u64)
        .is_some_and(|value| value != 0);
    let control_flags = record_get(record, "ctlflags")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0);
    let founder = record_get(record, "founder")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|&value| value != crate::industry::INDUSTRY_FOUNDER_INVALID);
    let construction_date = record_get(record, "construction_date")
        .and_then(SlValue::as_i64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    let construction_type = record_get(record, "construction_type")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(crate::industry::INDUSTRY_CONSTRUCTION_UNKNOWN);
    let prod_level = record_get(record, "prod_level")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(crate::industry::PRODLEVEL_DEFAULT);
    #[allow(clippy::cast_possible_truncation)]
    Some(SavIndustry {
        industry_id,
        pos,
        width: width.min(255) as u8,
        height: height.min(255) as u8,
        industry_type: industry_type.min(255) as u8,
        random_colour: (random_colour % 16) as u8,
        counter,
        selected_layout,
        random,
        last_prod_year,
        was_cargo_delivered,
        control_flags,
        founder,
        construction_date,
        construction_type,
        prod_level,
        produced: industry_produced_from_record(record),
        accepted: industry_accepted_from_record(record),
    })
}

fn industry_produced_from_record(record: &SlRecord) -> Vec<SavIndustryProducedCargo> {
    let Some(SlValue::Structs(produced)) = record_get(record, "produced") else {
        return Vec::new();
    };
    produced
        .iter()
        .filter_map(|entry| {
            let cargo_slot = record_get(entry, "cargo")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())?;
            let waiting = record_get(entry, "waiting")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0);
            let rate = record_get(entry, "rate")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0);
            Some(SavIndustryProducedCargo {
                cargo_slot,
                waiting,
                rate,
            })
        })
        .collect()
}

fn industry_accepted_from_record(record: &SlRecord) -> Vec<SavIndustryAcceptedCargo> {
    let Some(SlValue::Structs(accepted)) = record_get(record, "accepted") else {
        return Vec::new();
    };
    accepted
        .iter()
        .filter_map(|entry| {
            let cargo_slot = record_get(entry, "cargo")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())?;
            let waiting = record_get(entry, "waiting")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0);
            let last_accepted = record_get(entry, "last_accepted")
                .and_then(SlValue::as_i64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(0);
            Some(SavIndustryAcceptedCargo {
                cargo_slot,
                waiting,
                last_accepted,
            })
        })
        .collect()
}

/// Empresa mínima decodificada del chunk `PLYR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavCompany {
    /// Índice del pool `CompanyID` (`PLYR` es una tabla densa).
    pub id: u32,
    pub money: i64,
    /// Préstamo vigente (`PLYR.current_loan`).
    pub loan: Option<i64>,
    /// Límite individual de préstamo (`PLYR.max_loan`).
    ///
    /// `Some(COMPANY_MAX_LOAN_DEFAULT)` significa que la compañía debe seguir
    /// el límite global, no que tenga un límite negativo.
    pub max_loan: Option<i64>,
    pub colour: u8,
    /// Nombre personalizado, si el save usa el campo moderno `PLYR.name`.
    pub name: Option<String>,
    /// Nombre personalizado del presidente (`PLYR.president_name`).
    pub president_name: Option<String>,
    /// Bitfield del retrato del presidente (`PLYR.face`).
    pub manager_face: Option<u32>,
    /// Etiqueta del estilo de retrato (`PLYR.face_style`, SLV 355).
    pub manager_face_style: Option<String>,
    /// Marca de compañía controlada por IA, si está presente en el save.
    pub is_ai: Option<bool>,
    /// Meses consecutivos de bancarrota (`PLYR.months_of_bankruptcy`).
    pub bankruptcy_months: Option<u8>,
    /// Acumulador del trimestre actual (`PLYR.cur_economy`).
    pub cur_economy: Option<SavCompanyEconomy>,
    /// Trimestres cerrados en orden `OpenTTD`: más reciente primero (`PLYR.old_economy`).
    pub old_economy: Vec<SavCompanyEconomy>,
    /// Esquemas `PLYR.liveries` en orden `LiveryScheme`.
    pub liveries: Vec<crate::company::CompanyLivery>,
    /// Opciones de autorrenovación/servicio de `PLYR.settings`.
    /// Cabeza `EngineRenew` de la compañía, como índice de pool (no `index + 1`).
    pub engine_renew_list_head: Option<u16>,
    pub engine_renew: Option<bool>,
    pub engine_renew_months: Option<i16>,
    pub engine_renew_money: Option<u32>,
    pub renew_keep_length: Option<bool>,
    pub servint_ispercent: Option<bool>,
    pub servint_trains: Option<u16>,
    pub servint_roadveh: Option<u16>,
    pub servint_aircraft: Option<u16>,
    pub servint_ships: Option<u16>,
}

/// Entrada de `CompanyEconomyEntry` serializada en `PLYR`.
///
/// `income` y `expenses` se conservan con signo porque el wire format usa
/// `Money` (`i64`), aunque el runtime normal los acumule como valores positivos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavCompanyEconomy {
    pub income: i64,
    pub expenses: i64,
    pub company_value: i64,
    /// Las 64 entradas modernas de `delivered_cargo` (o menos en saves legacy).
    pub delivered_cargo: Vec<u32>,
    pub performance_history: i32,
}

/// Migra la lista de libreas de `PLYR` al orden actual de `LiveryScheme`.
///
/// `OpenTTD` insertó dos esquemas de vagones en SLV 85 y los de tranvía en
/// SLV 63; además, antes de SLV 205 un único flag activo significaba ambos
/// canales. Repetir esas migraciones aquí evita reinterpretar colores al abrir
/// un save histórico y volver a exportarlo como SLV moderno.
fn company_liveries_from_record(
    record: &SlRecord,
    company_colour: u8,
    save_version: u16,
) -> Vec<crate::company::CompanyLivery> {
    let Some(SlValue::Structs(entries)) = record_get(record, "liveries") else {
        return Vec::new();
    };
    if entries.is_empty() {
        return Vec::new();
    }

    let loaded_count = entries
        .len()
        .min(crate::company::COMPANY_LIVERY_SCHEME_COUNT);
    let mut liveries = crate::company::default_company_liveries(company_colour);
    for (target, entry) in liveries.iter_mut().zip(entries.iter()).take(loaded_count) {
        *target = crate::company::CompanyLivery {
            in_use: record_get(entry, "in_use")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0),
            colour1: record_get(entry, "colour1")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0),
            colour2: record_get(entry, "colour2")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0),
        };
    }

    // `SlCompanyLiveries::Load` normaliza los flags previos a las libreas de
    // grupo: antes eran un selector binario, no dos canales independientes.
    if save_version < 205 {
        let default = liveries[0];
        for livery in liveries.iter_mut().skip(1).take(loaded_count - 1) {
            if livery.in_use
                & (crate::company::COMPANY_LIVERY_FLAG_PRIMARY
                    | crate::company::COMPANY_LIVERY_FLAG_SECONDARY)
                == 0
            {
                livery.colour1 = default.colour1;
                livery.colour2 = default.colour2;
            } else {
                livery.in_use = crate::company::COMPANY_LIVERY_FLAG_PRIMARY
                    | crate::company::COMPANY_LIVERY_FLAG_SECONDARY;
            }
        }
    }

    if save_version < 85 {
        // `std::move_backward(livery + LS_FREIGHT_WAGON - 2, end - 2, end)`
        // de OpenTTD: abre lugar para Passenger Wagon Monorail/Maglev.
        liveries.copy_within(11..21, 13);
        liveries[11] = liveries[4];
        liveries[12] = liveries[5];
    }
    if save_version < 63 {
        // Los tranvías heredan bus/camión en saves anteriores a su introducción.
        liveries[21] = liveries[14];
        liveries[22] = liveries[15];
    }

    liveries
}

/// Lee un entero firmado de tabla, aceptando la codificación sin signo usada
/// por algunos saves históricos cuando el valor cabe en `i64`.
fn record_i64(record: &SlRecord, name: &str) -> Option<i64> {
    record_get(record, name)
        .and_then(SlValue::as_i64)
        .or_else(|| {
            record_get(record, name)
                .and_then(SlValue::as_u64)
                .and_then(|value| i64::try_from(value).ok())
        })
}

fn company_economy_from_record(record: &SlRecord) -> SavCompanyEconomy {
    let delivered_cargo = match record_get(record, "delivered_cargo") {
        Some(SlValue::List(values)) => values
            .iter()
            .filter_map(SlValue::as_u64)
            .filter_map(|value| u32::try_from(value).ok())
            .collect(),
        _ => Vec::new(),
    };
    SavCompanyEconomy {
        income: record_i64(record, "income").unwrap_or(0),
        expenses: record_i64(record, "expenses").unwrap_or(0),
        company_value: record_i64(record, "company_value").unwrap_or(0),
        delivered_cargo,
        performance_history: record_i64(record, "performance_history")
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(0),
    }
}

fn company_cur_economy_from_record(record: &SlRecord) -> Option<SavCompanyEconomy> {
    nested_struct(record, "cur_economy").map(company_economy_from_record)
}

fn company_old_economy_from_record(record: &SlRecord) -> Vec<SavCompanyEconomy> {
    let Some(SlValue::Structs(entries)) = record_get(record, "old_economy") else {
        return Vec::new();
    };
    entries.iter().map(company_economy_from_record).collect()
}

/// Empresas presentes en `PLYR`, conservando dinero y color por `CompanyID`.
#[must_use]
pub(crate) fn companies_from_chunks(chunks: &[RawChunk], save_version: u16) -> Vec<SavCompany> {
    let Some(plyr) = find_chunk(chunks, "PLYR") else {
        return Vec::new();
    };
    table_rows(plyr, save_version)
        .into_iter()
        .filter_map(|(id, record)| {
            let money = record_i64(&record, "money")?;
            let loan = record_i64(&record, "current_loan");
            let colour = record_get(&record, "colour")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value % 16).ok())?;
            let name = record_get(&record, "name")
                .and_then(SlValue::as_str)
                .map(str::to_owned);
            let president_name = record_get(&record, "president_name")
                .and_then(SlValue::as_str)
                .map(str::to_owned);
            let manager_face = record_get(&record, "face")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            let manager_face_style = record_get(&record, "face_style")
                .and_then(SlValue::as_str)
                .filter(|style| !style.is_empty())
                .map(str::to_owned);
            let is_ai = record_get(&record, "is_ai")
                .and_then(SlValue::as_u64)
                .map(|value| value != 0);
            let bankruptcy_months = record_get(&record, "months_of_bankruptcy")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok());
            let liveries = company_liveries_from_record(&record, colour, save_version);
            let settings = nested_struct(&record, "settings");
            let setting = |name: &str, legacy: &str| {
                settings
                    .and_then(|settings| record_get(settings, name))
                    .or_else(|| settings.and_then(|settings| record_get(settings, legacy)))
            };
            let engine_renew = setting("settings.engine_renew", "engine_renew")
                .and_then(SlValue::as_u64)
                .map(|value| value != 0);
            // `SLE_REF(..., REF_ENGINE_RENEWS)` se guarda como `index + 1`;
            // cero es null. Desde SLV_69 ocupa u32 aunque el pool use IDs u16.
            let engine_renew_list_head = setting("engine_renew_list", "engine_renew_list")
                .and_then(SlValue::as_u64)
                .and_then(|reference| reference.checked_sub(1))
                .and_then(|id| u16::try_from(id).ok())
                .filter(|id| *id < 64_000);
            let engine_renew_months =
                setting("settings.engine_renew_months", "engine_renew_months")
                    .and_then(SlValue::as_i64)
                    .and_then(|value| i16::try_from(value).ok());
            let engine_renew_money = setting("settings.engine_renew_money", "engine_renew_money")
                .and_then(SlValue::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            let renew_keep_length = setting("settings.renew_keep_length", "renew_keep_length")
                .and_then(SlValue::as_u64)
                .map(|value| value != 0);
            let servint_ispercent =
                setting("settings.vehicle.servint_ispercent", "servint_ispercent")
                    .and_then(SlValue::as_u64)
                    .map(|value| value != 0);
            let servint_trains = setting("settings.vehicle.servint_trains", "servint_trains")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            let servint_roadveh = setting("settings.vehicle.servint_roadveh", "servint_roadveh")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            let servint_aircraft = setting("settings.vehicle.servint_aircraft", "servint_aircraft")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            let servint_ships = setting("settings.vehicle.servint_ships", "servint_ships")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            Some(SavCompany {
                id,
                money,
                loan,
                max_loan: record_i64(&record, "max_loan"),
                colour,
                name,
                president_name,
                manager_face,
                manager_face_style,
                is_ai,
                bankruptcy_months,
                cur_economy: company_cur_economy_from_record(&record),
                old_economy: company_old_economy_from_record(&record),
                liveries,
                engine_renew_list_head,
                engine_renew,
                engine_renew_months,
                engine_renew_money,
                renew_keep_length,
                servint_ispercent,
                servint_trains,
                servint_roadveh,
                servint_aircraft,
                servint_ships,
            })
        })
        .collect()
}

/// Dinero de la primera empresa del chunk `PLYR` (la del jugador en partidas locales).
#[must_use]
pub(crate) fn company_money_from_chunks(chunks: &[RawChunk], save_version: u16) -> Option<i64> {
    let record = first_company_record(chunks, save_version)?;
    match record_get(&record, "money")? {
        SlValue::Int(v) => Some(*v),
        SlValue::Uint(v) => i64::try_from(*v).ok(),
        _ => None,
    }
}

/// Color de compañía (`Colours`) de la primera empresa en `PLYR`.
#[must_use]
pub(crate) fn company_colour_from_chunks(chunks: &[RawChunk], save_version: u16) -> Option<u8> {
    let record = first_company_record(chunks, save_version)?;
    let colour = record_get(&record, "colour")?.as_u64()?;
    Some((colour % 16) as u8)
}

fn first_company_record(chunks: &[RawChunk], save_version: u16) -> Option<SlRecord> {
    let plyr = find_chunk(chunks, "PLYR")?;
    let rows = table_rows(plyr, save_version);
    let (_, record) = rows.into_iter().min_by_key(|(idx, _)| *idx)?;
    Some(record)
}

/// Tipo de vehículo de `OpenTTD` (`VehicleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavVehicleKind {
    Train,
    RoadVehicle,
    Ship,
    Aircraft,
}

/// Vehículo decodificado del chunk `VEHS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavVehicle {
    /// ID de la fila `VEHS` (referenciado por `Vehicle::next`).
    pub sav_id: u32,
    /// Siguiente unidad del mismo convoy en la tabla `VEHS`.
    ///
    /// `OpenTTD` no garantiza que las unidades del convoy estén consecutivas en
    /// la tabla sparse; esta referencia es la fuente autoritativa para
    /// reconstruir la cadena del tren.
    pub next_sav_id: Option<u32>,
    /// Siguiente vehículo de la cadena nativa de órdenes compartidas.
    pub next_shared_sav_id: Option<u32>,
    /// Compañía propietaria (`Vehicle::owner`).
    pub owner: u8,
    /// Número visible de unidad (`Vehicle::unitnumber`).
    pub unit_number: u16,
    /// Grupo de flota (`Vehicle::group_id`) o `None` para el grupo por defecto.
    pub group_id: Option<u32>,
    /// Inicio del ciclo de horario (`Vehicle::timetable_start`) en ticks.
    ///
    /// El campo existe en `VEHS.common` desde SLV 129. Se conserva aunque el
    /// importador no pueda reconstruir todos los flags de timetable de
    /// `OpenTTD`, para que un round-trip no pierda el inicio escalonado.
    pub timetable_start: u64,
    /// Tiempo transcurrido en la orden actual (`current_order_time`).
    pub current_order_time: u32,
    /// Retraso acumulado del horario (`lateness_counter`).
    pub timetable_lateness: i32,
    /// Ventanas de salida de depósito para unbunching.
    pub depot_unbunching_last_departure: u64,
    pub depot_unbunching_next_departure: u64,
    pub round_trip_time: u32,
    /// Bits `Vehicle::vehicle_flags` relevantes para el runtime de horarios.
    ///
    /// `OpenTTD` conserva, entre otros, `TimetableStarted` (bit 3) y
    /// `AutofillTimetable` (bit 4) en el mismo struct común de `VEHS`.
    /// Mantener el bitset crudo permite round-trippear flags futuros sin
    /// confundirlos con los tiempos numéricos del horario.
    pub vehicle_flags: u16,
    /// Semilla aleatoria persistente usada por los callbacks/Action2 de
    /// `NewGRF` (`Vehicle::random_bits`).
    pub random_bits: u16,
    /// Triggers aleatorios pendientes de consumir por el `NewGRF` activo.
    pub waiting_random_triggers: u8,
    /// Última estación visitada, conservando el `StationID` nativo del save.
    pub last_station_visited: Option<u32>,
    /// Última estación desde la que pudo salir con carga
    /// (`Vehicle::last_loading_station`).
    pub last_loading_station: Option<u32>,
    /// Tick nativo de la última salida con carga (`last_loading_tick`).
    pub last_loading_tick: u64,
    /// Intervalo de servicio nativo (`Vehicle::service_interval`).
    pub service_interval: u16,
    /// Fiabilidad y estado de averías del vehículo al guardar.
    pub reliability: u16,
    pub reliability_spd_dec: u16,
    pub breakdown_ctr: u8,
    pub breakdown_delay: u8,
    pub breakdowns_since_last_service: u8,
    pub breakdown_chance: u8,
    /// Beneficios acumulados de la unidad/cabeza de consist.
    pub profit_this_year: i64,
    pub profit_last_year: i64,
    /// Índice de la lista `ORDL` referenciada por `VEHS.common.orders`.
    ///
    /// Se conserva para reconstruir shared orders al hidratar el `GameState`.
    /// `None` cubre listas legacy (`ORDR`) y vehículos sin órdenes.
    pub order_list_id: Option<u32>,
    pub kind: SavVehicleKind,
    /// Nombre personalizado (`Vehicle::name`); vacío cuando usa el nombre
    /// generado por el motor.
    pub name: Option<String>,
    /// Tesela utilizable por el motor. Para trenes/carretera es literal
    /// `Vehicle::tile`; para aviones se recalcula desde `x_pos`/`y_pos`
    /// (ver [`Self::raw_tile`] para el valor crudo del save).
    pub pos: TileCoord,
    /// `Vehicle::tile` crudo del save, sin recalcular. En aviones bajo FTA
    /// suele quedar vestigial en `(0, 0)` (`OpenTTD` no lo actualiza en
    /// vuelo); se conserva para trazas/oráculos que lo esperan tal cual.
    pub raw_tile: TileCoord,
    /// Destino nativo (`Vehicle::dest_tile`), cuando está disponible.
    pub dest: TileCoord,
    /// Progreso sub-tesela (`Vehicle::progress`, 0…255) al guardar.
    pub progress: u8,
    /// Contador de movimiento de 32 bits (`Vehicle::motion_counter`).
    pub motion_counter: u32,
    /// Coordenada píxel absoluta (`Vehicle::x_pos` / `y_pos`).
    pub x_pos: i32,
    pub y_pos: i32,
    /// Altura en píxeles (`Vehicle::z_pos`); solo relevante para aviones.
    pub z_pos: i32,
    /// Velocidad y fracción interna al guardar (`cur_speed` / `subspeed`).
    pub cur_speed: u16,
    pub subspeed: u8,
    /// Aceleración nativa (`Vehicle::acceleration`).
    pub acceleration: u8,
    /// Sprite base nativo (`Vehicle::spritenum`).
    pub sprite_num: u8,
    /// Estado de conducción de carretera (`RoadVehicle::state`).
    ///
    /// Es `0` para los demás tipos y para saves antiguos cuyo descriptor no
    /// contiene todavía el campo. Mantenerlo evita reiniciar la tabla
    /// `_road_drive_data` al importar un vehículo en movimiento.
    pub road_state: u8,
    /// Flags generales de vehículo de carretera (`RoadVehicle::gv_flags`).
    pub road_gv_flags: u16,
    /// Caché de ruta nativo (`trackdir` + `tile`) cuando el save lo incluye.
    pub road_path: Vec<crate::vehicle::RoadPathEntry>,
    /// Frame de la tabla de conducción de carretera (`RoadVehicle::frame`).
    pub road_frame: u8,
    /// Contador de bloqueo vial (`RoadVehicle::blocked_ctr`).
    pub road_blocked_ctr: u16,
    /// Carril de adelantamiento (`RoadVehicle::overtaking`).
    pub road_overtaking: u8,
    /// Contador de adelantamiento (`RoadVehicle::overtaking_ctr`).
    pub road_overtaking_ctr: u8,
    /// Animación de choque vial (`RoadVehicle::crashed_ctr`).
    pub road_crashed_ctr: u16,
    /// Contador de reversa vial (`RoadVehicle::reverse_ctr`).
    pub road_reverse_ctr: u8,
    /// Tren: posición de la animación de choque (`Train::crash_anim_pos`).
    pub train_crash_anim_pos: u16,
    /// Tren: `force_proceed` serializado como byte, sin normalizar flags.
    pub train_force_proceed: u8,
    /// Tren: índice de `Train::track` (no máscara `TrackBits`).
    pub train_track: u8,
    /// Tren: flags específicos (`Train::flags`).
    pub train_flags: u16,
    /// Tren: flags generales (`Train::gv_flags`).
    pub train_gv_flags: u16,
    /// Tren: contador de espera PBS (`Train::wait_counter`).
    pub train_wait_counter: u16,
    /// Bits de estado nativos de un barco (`Ship::state`).
    ///
    /// Además de `TrackBits`, `OpenTTD` usa este byte para depósito y wormhole;
    /// por eso no debe reducirse únicamente a `ship_track` al importar.
    pub ship_state: u8,
    /// Rotación gráfica persistida por `SlVehicleShip`.
    pub ship_rotation: u8,
    /// Caché de ruta nativo de barco (`Ship::path`, sólo `Trackdir`).
    pub ship_path: Vec<u8>,
    /// Track derivado de `ship_state`, cuando el estado representa una vía
    /// ordinaria. Es la forma que consume el controlador de movimiento Rust.
    pub ship_track: u8,
    /// Dirección visual/de movimiento (`Vehicle::direction`) al guardar.
    pub direction: u8,
    /// ID de motor vanilla de `OpenTTD` (`Vehicle::engine_type`).
    pub engine_type: u16,
    /// `CargoType` de `OpenTTD` (0 = pasajeros).
    pub cargo_type: u8,
    /// Subtipo de carga guardado por `Vehicle::cargo_subtype`.
    pub cargo_subtype: u8,
    /// Unidades de carga a bordo (`Vehicle::cargo.StoredCount()`).
    ///
    /// En saves modernos es la suma cacheada de `cargo.packets`; se conserva
    /// también cuando el port todavía no reconstruye cada packet vehicular,
    /// porque participa de la masa y aceleración realista de carretera.
    pub cargo: u16,
    /// Capacidad efectiva tras refit (`Vehicle::cargo_cap`).
    pub cargo_capacity: u16,
    /// Capacidad máxima de refit (`Vehicle::refit_cap`).
    pub refit_capacity: u16,
    /// Referencias físicas al pool `CAPA` (`Vehicle::cargo.packets`).
    pub cargo_packet_ids: Vec<u32>,
    /// Conteos de movimiento de carga (`VehicleCargoList::action_counts`).
    /// El orden nativo es transferir, entregar, conservar y cargar.
    pub cargo_action_counts: [u32; 4],
    /// Cuenta atrás de `Vehicle::cargo_age_counter`.
    pub cargo_age_counter: u16,
    /// Edad y servicio en días/fechas del calendario nativo.
    pub age_days: u32,
    /// Edad contable en días de economía (`Vehicle::economy_age`).
    pub economy_age_days: u32,
    pub max_age_days: u32,
    pub date_of_last_service: i32,
    /// Fecha de servicio protegida para callbacks `NewGRF`.
    pub date_of_last_service_newgrf: i32,
    /// Año calendario en que se compró la unidad (`Vehicle::build_year`).
    pub build_year: i32,
    /// Cuenta atrás entre ciclos de carga/descarga.
    pub load_unload_ticks: u16,
    /// Campo legacy de pago de carga, aún presente en el descriptor nativo.
    pub cargo_paid_for: u16,
    /// Valor contable de la unidad, en dinero entero (se elimina la fracción
    /// de 8 bits del wire format para alinearlo con el modelo core).
    pub value: i64,
    /// Órdenes de la lista referenciada (`ORDL`).
    pub orders: Vec<super::orders::SavOrder>,
    /// Índice de orden actual (`cur_real_order_index`).
    pub current_order: usize,
    /// Índice de orden implícita (`cur_implicit_order_index`).
    pub cur_implicit_order_index: usize,
    /// Snapshot crudo de `Vehicle::current_order`, incluidos flags de carga y
    /// los campos de horario que no están en `ORDL`.
    pub current_order_state: crate::vehicle::VehicleOrderRuntime,
    /// Contadores diarios de `Vehicle` usados por callbacks y costes.
    pub day_counter: u8,
    pub tick_counter: u8,
    pub running_ticks: u8,
    /// `false` si el jugador detuvo el vehículo (`VehState::Stopped`).
    pub running: bool,
    /// Tren: unidad sin `GVSF_FRONT` (vagón del consist anterior).
    pub is_wagon: bool,
    /// Avión: `subtype == AIR_HELICOPTER` (0) del save (heli vs. ala fija).
    pub is_helicopter: bool,
    /// Avión: waypoint FTA actual (`Aircraft::pos`).
    pub airport_pos: u8,
    /// Avión: waypoint FTA previo (`Aircraft::previous_pos`).
    pub airport_previous_pos: u8,
    /// Avión: heading FTA (`Aircraft::state` / `AirportMovementStates`).
    pub airport_state: u8,
    /// Avión: `StationID` destino FTA (`Aircraft::targetairport`).
    pub airport_targetairport: u16,
    /// Avión: contador de animación tras choque (`Aircraft::crashed_counter`).
    pub aircraft_crashed_counter: u16,
    /// Avión: giros consecutivos (`Aircraft::number_consecutive_turns`).
    pub aircraft_number_consecutive_turns: u8,
    /// Avión: contador de giro (`Aircraft::turn_counter`).
    pub aircraft_turn_counter: u8,
    /// Avión: flags nativos (`Aircraft::flags`).
    pub aircraft_flags: u8,
}

/// Bit `GVSF_FRONT` de `Vehicle::subtype` (cabeza de convoy en tren/camión).
const GVSF_FRONT: u64 = 0x01;

/// Proyecta `Ship::state` (`TrackBits`) al índice de track que usa el
/// controlador Rust. Los valores restantes son estados especiales de
/// `OpenTTD` (depósito, wormhole o vacío) y no tienen track navegable directo.
#[must_use]
fn ship_track_from_state(state: u8) -> u8 {
    match state {
        1 => crate::ship_movement::TRACK_X,
        2 => crate::ship_movement::TRACK_Y,
        4 => crate::ship_movement::TRACK_UPPER,
        8 => crate::ship_movement::TRACK_LOWER,
        16 => crate::ship_movement::TRACK_LEFT,
        32 => crate::ship_movement::TRACK_RIGHT,
        _ => 0,
    }
}

/// Lee las dos variantes del caché de ruta marítimo de `OpenTTD`: la lista
/// moderna de structs `path` y el vector legacy de `Trackdir`.
#[must_use]
fn ship_path_from_record(record: &SlRecord) -> Vec<u8> {
    if let Some(SlValue::Structs(items)) = record_get(record, "path") {
        return items
            .iter()
            .filter_map(|item| {
                record_get(item, "trackdir")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
            })
            .collect();
    }
    match record_get(record, "path") {
        Some(SlValue::List(items)) => items
            .iter()
            .filter_map(|value| value.as_u64().and_then(|raw| u8::try_from(raw).ok()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Lee las dos variantes del caché de ruta vial de `OpenTTD`: la lista moderna
/// de structs `path` y los vectores legacy `path.td`/`path.tile`.
#[must_use]
fn road_path_from_record(record: &SlRecord) -> Vec<crate::vehicle::RoadPathEntry> {
    if let Some(SlValue::Structs(items)) = record_get(record, "path") {
        return items
            .iter()
            .filter_map(|item| {
                let trackdir = record_get(item, "trackdir")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())?;
                let tile = record_get(item, "tile")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())?;
                Some(crate::vehicle::RoadPathEntry { trackdir, tile })
            })
            .collect();
    }
    let Some(SlValue::List(trackdirs)) = record_get(record, "path.td") else {
        return Vec::new();
    };
    let Some(SlValue::List(tiles)) = record_get(record, "path.tile") else {
        return Vec::new();
    };
    trackdirs
        .iter()
        .zip(tiles)
        .filter_map(|(trackdir, tile)| {
            Some(crate::vehicle::RoadPathEntry {
                trackdir: u8::try_from(trackdir.as_u64()?).ok()?,
                tile: u32::try_from(tile.as_u64()?).ok()?,
            })
        })
        .collect()
}

/// Vehículos del chunk `VEHS` (sparse table): tren/road/ship/aircraft.
/// Aviones: solo el primario (`subtype` ≤ 2); sombra/rotor se omiten.
#[must_use]
#[allow(clippy::too_many_lines)]
pub(crate) fn vehicles_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    order_import: &super::orders::SavOrderImport,
    save_version: u16,
) -> Vec<SavVehicle> {
    let Some(vehs) = find_chunk(chunks, "VEHS") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (sav_id, record) in table_rows(vehs, save_version) {
        let Some(vtype) = record_get(&record, "type").and_then(SlValue::as_u64) else {
            continue;
        };
        let (kind, sub_name) = match vtype {
            0 => (SavVehicleKind::Train, "train"),
            1 => (SavVehicleKind::RoadVehicle, "roadveh"),
            2 => (SavVehicleKind::Ship, "ship"),
            3 => (SavVehicleKind::Aircraft, "aircraft"),
            _ => continue,
        };
        let Some(sub) = nested_struct(&record, sub_name) else {
            continue;
        };
        let Some(common) = nested_struct(sub, "common") else {
            continue;
        };
        let next_sav_id = record_get(common, "next")
            .and_then(SlValue::as_u64)
            // `SLE_REF` codifica el id de la tabla + 1; el cero queda como
            // puntero nulo. Decodificarlo literal conectaba, por ejemplo, el
            // siguiente `33` con la fila 33 en vez de la 32.
            .and_then(|next| next.checked_sub(1))
            .and_then(|next| u32::try_from(next).ok());
        let next_shared_sav_id = record_get(common, "next_shared")
            .and_then(SlValue::as_u64)
            .and_then(|next| next.checked_sub(1))
            .and_then(|next| u32::try_from(next).ok());
        let owner = record_get(common, "owner")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let unit_number = record_get(common, "unitnumber")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let name = record_get(common, "name")
            .and_then(SlValue::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        // OpenTTD usa DEFAULT_GROUP/ALL_GROUP como sentinelas, no como grupos
        // persistibles. Mantenerlos como `None` evita crear grupos fantasma al
        // hidratar un save nativo.
        let group_id = record_get(common, "group_id")
            .and_then(SlValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value != 0xFFFE && *value != 0xFFFD);
        let subtype = record_get(common, "subtype")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        // `Aircraft::IsNormalAircraft`: subtype <= AIR_AIRCRAFT (2); descarta
        // sombra (4) y rotor (6), que no son vehículos primarios.
        if kind == SavVehicleKind::Aircraft && subtype > 2 {
            continue;
        }
        let is_front = subtype & GVSF_FRONT != 0;
        // Carretera: solo cabezas. Tren: cabeza y vagones.
        if kind == SavVehicleKind::RoadVehicle && !is_front {
            continue;
        }
        let is_wagon = kind == SavVehicleKind::Train && !is_front;
        // `AIR_HELICOPTER = 0` en `AirVehicleSubType`.
        let is_helicopter = kind == SavVehicleKind::Aircraft && subtype == 0;
        let Some(tile) = record_get(common, "tile").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = coord_from_linear_index(tile, map_w) else {
            continue;
        };
        let dest = record_get(common, "dest_tile")
            .and_then(SlValue::as_u64)
            .and_then(|value| coord_from_linear_index(value, map_w))
            .unwrap_or(pos);
        let progress = record_get(common, "progress")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let x_pos = record_get(common, "x_pos")
            .and_then(SlValue::as_i64)
            .unwrap_or(i64::from(pos.x) * 16);
        let y_pos = record_get(common, "y_pos")
            .and_then(SlValue::as_i64)
            .unwrap_or(i64::from(pos.y) * 16);
        let z_pos = record_get(common, "z_pos")
            .and_then(SlValue::as_i64)
            .unwrap_or(0);
        // `Vehicle::tile` es vestigial en aviones bajo control FTA (queda en 0
        // mientras `x_pos`/`y_pos` sí trackean la posición real); recalcular
        // desde el píxel absoluto para tener una tesela utilizable, pero
        // conservando el valor crudo aparte (`raw_tile`) para reportarlo.
        let raw_tile = pos;
        let pos = if kind == SavVehicleKind::Aircraft {
            TileCoord::new(
                i32::try_from(x_pos.div_euclid(16)).unwrap_or(pos.x),
                i32::try_from(y_pos.div_euclid(16)).unwrap_or(pos.y),
            )
        } else {
            pos
        };
        let cur_speed = record_get(common, "cur_speed")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u16::MAX);
        let subspeed = record_get(common, "subspeed")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let acceleration = record_get(common, "acceleration")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let sprite_num = record_get(common, "spritenum")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let motion_counter = record_get(common, "motion_counter")
            .and_then(SlValue::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0);
        let (
            road_state,
            road_frame,
            road_blocked_ctr,
            road_overtaking,
            road_overtaking_ctr,
            road_crashed_ctr,
            road_reverse_ctr,
        ) = if kind == SavVehicleKind::RoadVehicle {
            (
                record_get(sub, "state")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "frame")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "blocked_ctr")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "overtaking")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "overtaking_ctr")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "crashed_ctr")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "reverse_ctr")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
            )
        } else {
            (0, 0, 0, 0, 0, 0, 0)
        };
        let road_gv_flags = if kind == SavVehicleKind::RoadVehicle {
            record_get(sub, "gv_flags")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0)
        } else {
            0
        };
        let road_path = if kind == SavVehicleKind::RoadVehicle {
            road_path_from_record(sub)
        } else {
            Vec::new()
        };
        let ship_path = if kind == SavVehicleKind::Ship {
            ship_path_from_record(sub)
        } else {
            Vec::new()
        };
        let (ship_state, ship_rotation, ship_track) = if kind == SavVehicleKind::Ship {
            let state = record_get(sub, "state")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0);
            let rotation = record_get(sub, "rotation")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0);
            (state, rotation, ship_track_from_state(state))
        } else {
            (0, 0, 0)
        };
        let (
            train_crash_anim_pos,
            train_force_proceed,
            train_track,
            train_flags,
            train_gv_flags,
            train_wait_counter,
        ) = if kind == SavVehicleKind::Train {
            (
                record_get(sub, "crash_anim_pos")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "force_proceed")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "track")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "flags")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "gv_flags")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or(0),
                record_get(sub, "wait_counter")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .unwrap_or(0),
            )
        } else {
            (0, 0, 0, 0, 0, 0)
        };
        let direction = record_get(common, "direction")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let engine_type = record_get(common, "engine_type")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u16::MAX);
        let cargo_type = record_get(common, "cargo_type")
            .and_then(SlValue::as_u64)
            .unwrap_or(0xFF);
        let cargo_subtype = record_get(common, "cargo_subtype")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let cargo = record_get(common, "cargo_count")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let cargo_packet_ids = record_get(common, "cargo.packets")
            .and_then(|value| match value {
                SlValue::List(refs) => Some(
                    refs.iter()
                        .filter_map(|reference| {
                            reference
                                .as_u64()
                                .and_then(|value| value.checked_sub(1))
                                .and_then(|value| u32::try_from(value).ok())
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        let cargo_action_counts = record_get(common, "cargo.action_counts")
            .and_then(|value| match value {
                SlValue::List(values) => {
                    let mut counts = [0_u32; 4];
                    for (slot, value) in values.iter().take(counts.len()).enumerate() {
                        counts[slot] = value
                            .as_u64()
                            .and_then(|raw| u32::try_from(raw).ok())
                            .unwrap_or(0);
                    }
                    Some(counts)
                }
                _ => None,
            })
            .unwrap_or_else(|| [0, 0, u32::from(cargo), 0]);
        let cargo_capacity = record_get(common, "cargo_cap")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let refit_capacity = record_get(common, "refit_cap")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let cargo_age_counter = record_get(common, "cargo_age_counter")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let age_days = record_get(common, "age")
            .and_then(SlValue::as_i64)
            .and_then(|value| u32::try_from(value.max(0)).ok())
            .unwrap_or(0);
        let economy_age_days = record_get(common, "economy_age")
            .and_then(SlValue::as_i64)
            .and_then(|value| u32::try_from(value.max(0)).ok())
            .or_else(|| {
                record_get(common, "economy_age")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
            })
            .unwrap_or(age_days);
        let max_age_days = record_get(common, "max_age")
            .and_then(SlValue::as_i64)
            .and_then(|value| u32::try_from(value.max(0)).ok())
            .unwrap_or(0);
        let date_of_last_service = record_get(common, "date_of_last_service")
            .and_then(SlValue::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(0);
        let date_of_last_service_newgrf = record_get(common, "date_of_last_service_newgrf")
            .and_then(SlValue::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .or_else(|| {
                record_get(common, "date_of_last_service_newgrf")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| i32::try_from(value).ok())
            })
            .unwrap_or(date_of_last_service);
        let build_year = record_get(common, "build_year")
            .and_then(SlValue::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .or_else(|| {
                record_get(common, "build_year")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| i32::try_from(value).ok())
            })
            .unwrap_or(0);
        let load_unload_ticks = record_get(common, "load_unload_ticks")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let cargo_paid_for = record_get(common, "cargo_paid_for")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let value = record_get(common, "value")
            .and_then(SlValue::as_i64)
            .or_else(|| {
                record_get(common, "value")
                    .and_then(SlValue::as_u64)
                    .and_then(|raw| i64::try_from(raw).ok())
            })
            .map_or(0, |raw| raw / 256);
        let order_list_ref = record_get(common, "orders")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let order_list_id = (save_version >= super::orders::SLV_105 && order_list_ref != 0)
            .then(|| u32::try_from(order_list_ref - 1).ok())
            .flatten();
        let orders = if is_wagon {
            Vec::new()
        } else {
            order_import.orders_for_vehicle_ref(order_list_ref)
        };
        let current_order = record_get(common, "cur_real_order_index")
            .and_then(SlValue::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0);
        let current_order_state = crate::vehicle::VehicleOrderRuntime {
            order_type: record_get(common, "current_order.type")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0),
            flags: record_get(common, "current_order.flags")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0),
            dest: record_get(common, "current_order.dest")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0),
            refit_cargo: record_get(common, "current_order.refit_cargo")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0xFF),
            wait_time: record_get(common, "current_order.wait_time")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0),
            travel_time: record_get(common, "current_order.travel_time")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0),
            max_speed: record_get(common, "current_order.max_speed")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(u16::MAX),
        };
        let day_counter = record_get(common, "day_counter")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let tick_counter = record_get(common, "tick_counter")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let running_ticks = record_get(common, "running_ticks")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let cur_implicit_order_index = record_get(common, "cur_implicit_order_index")
            .and_then(SlValue::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(current_order);
        let timetable_start = record_get(common, "timetable_start")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let current_order_time = record_get(common, "current_order_time")
            .and_then(SlValue::as_i64)
            .and_then(|value| u32::try_from(value).ok())
            .or_else(|| {
                record_get(common, "current_order_time")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
            })
            .unwrap_or(0);
        let timetable_lateness = record_get(common, "lateness_counter")
            .and_then(SlValue::as_i64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or_else(|_| {
                record_get(common, "lateness_counter")
                    .and_then(SlValue::as_u64)
                    .and_then(|v| i32::try_from(v).ok())
                    .unwrap_or(0)
            });
        let depot_unbunching_last_departure = record_get(common, "depot_unbunching_last_departure")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let depot_unbunching_next_departure = record_get(common, "depot_unbunching_next_departure")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let round_trip_time = record_get(common, "round_trip_time")
            .and_then(SlValue::as_i64)
            .and_then(|value| u32::try_from(value.max(0)).ok())
            .or_else(|| {
                record_get(common, "round_trip_time")
                    .and_then(SlValue::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
            })
            .unwrap_or(0);
        let vehicle_flags = record_get(common, "vehicle_flags")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let random_bits = record_get(common, "random_bits")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let waiting_random_triggers = record_get(common, "waiting_triggers")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        // A diferencia de los campos REF, `last_station_visited` se serializa
        // como `StationID` plano. `0xFFFF` es `StationID::Invalid()`.
        let last_station_visited = record_get(common, "last_station_visited")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != u16::MAX)
            .map(u32::from);
        let last_loading_station = record_get(common, "last_loading_station")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value != u16::MAX)
            .map(u32::from);
        let last_loading_tick = record_get(common, "last_loading_tick")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let service_interval = record_get(common, "service_interval")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(crate::DEFAULT_SERVICE_INTERVAL_DAYS);
        let reliability = record_get(common, "reliability")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(8_500);
        let reliability_spd_dec = record_get(common, "reliability_spd_dec")
            .and_then(SlValue::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(crate::engine::DEFAULT_RELIABILITY_SPD_DEC);
        let breakdown_ctr = record_get(common, "breakdown_ctr")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let breakdown_delay = record_get(common, "breakdown_delay")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let breakdowns_since_last_service = record_get(common, "breakdowns_since_last_service")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let breakdown_chance = record_get(common, "breakdown_chance")
            .and_then(SlValue::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(0);
        let profit_this_year = record_get(common, "profit_this_year")
            .and_then(SlValue::as_i64)
            .map_or(0, |value| value / 256);
        let profit_last_year = record_get(common, "profit_last_year")
            .and_then(SlValue::as_i64)
            .map_or(0, |value| value / 256);
        let vehstatus = record_get(common, "vehstatus")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let running = vehstatus & 1 == 0;
        // Campos FTA (`Aircraft::pos/previous_pos/state/targetairport`); viven
        // en `sub` (nivel aeronave), no en `common` (nivel `Vehicle` base).
        let airport_pos = record_get(sub, "pos")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let airport_previous_pos = record_get(sub, "previous_pos")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let airport_state = record_get(sub, "state")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let airport_targetairport = record_get(sub, "targetairport")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u16::MAX);
        let aircraft_crashed_counter = if kind == SavVehicleKind::Aircraft {
            record_get(sub, "crashed_counter")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0)
        } else {
            0
        };
        let aircraft_number_consecutive_turns = if kind == SavVehicleKind::Aircraft {
            record_get(sub, "number_consecutive_turns")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0)
        } else {
            0
        };
        let aircraft_turn_counter = if kind == SavVehicleKind::Aircraft {
            record_get(sub, "turn_counter")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0)
        } else {
            0
        };
        let aircraft_flags = if kind == SavVehicleKind::Aircraft {
            record_get(sub, "flags")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(0)
        } else {
            0
        };
        #[allow(clippy::cast_possible_truncation)]
        out.push(SavVehicle {
            sav_id,
            next_sav_id,
            next_shared_sav_id,
            owner,
            unit_number,
            group_id,
            timetable_start,
            current_order_time,
            timetable_lateness,
            depot_unbunching_last_departure,
            depot_unbunching_next_departure,
            round_trip_time,
            vehicle_flags,
            random_bits,
            waiting_random_triggers,
            last_station_visited,
            last_loading_station,
            last_loading_tick,
            service_interval,
            reliability,
            reliability_spd_dec,
            breakdown_ctr,
            breakdown_delay,
            breakdowns_since_last_service,
            breakdown_chance,
            profit_this_year,
            profit_last_year,
            order_list_id,
            kind,
            name,
            pos,
            raw_tile,
            dest,
            progress,
            motion_counter,
            x_pos: i32::try_from(x_pos).unwrap_or(0),
            y_pos: i32::try_from(y_pos).unwrap_or(0),
            z_pos: i32::try_from(z_pos).unwrap_or(0),
            cur_speed,
            subspeed,
            acceleration,
            sprite_num,
            road_state,
            road_frame,
            road_blocked_ctr,
            road_overtaking,
            road_overtaking_ctr,
            road_crashed_ctr,
            road_reverse_ctr,
            road_gv_flags,
            road_path,
            train_crash_anim_pos,
            train_force_proceed,
            train_track,
            train_flags,
            train_gv_flags,
            train_wait_counter,
            ship_state,
            ship_rotation,
            ship_path,
            ship_track,
            direction,
            engine_type,
            cargo_type: cargo_type.min(255) as u8,
            cargo_subtype,
            cargo,
            cargo_capacity,
            refit_capacity,
            cargo_packet_ids,
            cargo_action_counts,
            cargo_age_counter,
            age_days,
            economy_age_days,
            max_age_days,
            date_of_last_service,
            date_of_last_service_newgrf,
            build_year,
            load_unload_ticks,
            cargo_paid_for,
            value,
            orders,
            current_order,
            cur_implicit_order_index,
            current_order_state,
            day_counter,
            tick_counter,
            running_ticks,
            running,
            is_wagon,
            is_helicopter,
            airport_pos,
            airport_previous_pos,
            airport_state,
            airport_targetairport,
            aircraft_crashed_counter,
            aircraft_number_consecutive_turns,
            aircraft_turn_counter,
            aircraft_flags,
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE, RawChunk, find_chunk, parse_chunks};
    use super::super::container;
    use super::super::orders::SavOrderImport;
    use super::super::table::tests::{build_table_body, write_gamma, write_str};
    use super::super::table::{SlValue, record_get};
    use super::*;

    /// Smoke del fixture oráculo FTA Helidepot (2 pads + 1 Tricario A↔B).
    #[test]
    fn helidepot_fta_cycle_fixture_shape() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/helidepot_fta_cycle_15_3.sav");
        let raw = std::fs::read(&path).expect("fixture helidepot_fta_cycle_15_3.sav");
        let (data, version) = container::decompress(&raw).expect("decompress");
        let chunks = parse_chunks(&data).expect("chunks");
        let map_w = 64u32;

        let stnn = find_chunk(&chunks, "STNN").expect("STNN");
        let mut helidepots = 0usize;
        for (_, record) in table_rows(stnn, version) {
            let Some(normal) = nested_struct(&record, "normal") else {
                continue;
            };
            let atype = record_get(normal, "airport.type").and_then(SlValue::as_u64);
            let aw = record_get(normal, "airport.w").and_then(SlValue::as_u64);
            let ah = record_get(normal, "airport.h").and_then(SlValue::as_u64);
            // OpenTTD `AT_HELIDEPOT = 6`.
            if atype == Some(6) && aw == Some(2) && ah == Some(2) {
                helidepots += 1;
            }
        }
        assert_eq!(helidepots, 2);

        let vehs = find_chunk(&chunks, "VEHS").expect("VEHS");
        let order_import = SavOrderImport::from_chunks(&chunks, version);
        let mut primary = 0usize;
        for (_, record) in table_rows(vehs, version) {
            if record_get(&record, "type").and_then(SlValue::as_u64) != Some(3) {
                continue;
            }
            let Some(sub) = nested_struct(&record, "aircraft") else {
                continue;
            };
            let Some(common) = nested_struct(sub, "common") else {
                continue;
            };
            let orders_ref = record_get(common, "orders").and_then(SlValue::as_u64);
            let orders = orders_ref
                .map(|r| order_import.orders_for_vehicle_ref(r))
                .unwrap_or_default();
            if orders.len() == 2 {
                primary += 1;
                let fta_pos = record_get(sub, "pos").and_then(SlValue::as_u64);
                let prev = record_get(sub, "previous_pos").and_then(SlValue::as_u64);
                assert_eq!(fta_pos, Some(11), "heli en raise takeoff Helidepot");
                assert_eq!(prev, Some(17));
                let _ = map_w;
            }
        }
        assert_eq!(primary, 1, "un helicóptero con órdenes A↔B");
    }

    fn station_chunk(records: &[Vec<u8>]) -> RawChunk {
        RawChunk {
            name: *b"STNN",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[(6, "xy"), (0x0A | 0x10, "name"), (2, "facilities")],
                records,
            ),
        }
    }

    #[test]
    fn decodes_station_with_custom_name_and_skips_waypoints() {
        let mut st = Vec::new();
        st.extend_from_slice(&(2u32 * 64 + 5).to_be_bytes()); // xy → (5,2) con w=64
        write_str("Mi Estación", &mut st);
        st.push(1); // FACIL_TRAIN

        let mut wp = Vec::new();
        wp.extend_from_slice(&10u32.to_be_bytes());
        write_str("", &mut wp);
        wp.push(0x80); // waypoint

        let chunks = vec![station_chunk(&[st, wp])];
        let stations = stations_from_chunks(&chunks, 64, 300);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].pos, TileCoord::new(5, 2));
        assert_eq!(stations[0].name.as_deref(), Some("Mi Estación"));
        assert_eq!(stations[0].facilities, 1);
    }

    #[test]
    fn decodes_native_road_stop_spec_and_tile_mapping() {
        let mut header = Vec::new();
        header.push(0x1B);
        write_str("roadstopspeclist", &mut header);
        header.push(0x1B);
        write_str("roadstoptiledata", &mut header);
        header.push(0);
        header.push(6);
        write_str("grfid", &mut header);
        header.push(4);
        write_str("localidx", &mut header);
        header.push(0);
        header.push(6);
        write_str("tile", &mut header);
        header.push(2);
        write_str("random_bits", &mut header);
        header.push(2);
        write_str("animation_frame", &mut header);
        header.push(0);

        let mut record = Vec::new();
        write_gamma(2, &mut record);
        record.extend_from_slice(&0u32.to_be_bytes());
        record.extend_from_slice(&0u16.to_be_bytes());
        record.extend_from_slice(&0x4455_6677u32.to_be_bytes());
        record.extend_from_slice(&0x1234u16.to_be_bytes());
        write_gamma(1, &mut record);
        record.extend_from_slice(&(2u32 * 64 + 3).to_be_bytes());
        record.push(0xA5);
        record.push(6);

        let mut body = Vec::new();
        write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        write_gamma(record.len() as u32 + 1, &mut body);
        body.extend_from_slice(&record);
        write_gamma(0, &mut body);
        let chunk = RawChunk {
            name: *b"STNN",
            ch_type: CH_TABLE,
            body,
        };

        let data = road_stop_station_data_from_chunks(&[chunk], 64, 300);
        let station = data.get(&0).expect("station road stop data");
        assert_eq!(station.specs.len(), 2);
        assert_eq!(station.specs[1].grfid, 0x4455_6677);
        assert_eq!(station.specs[1].localidx, 0x1234);
        assert_eq!(station.tiles.len(), 1);
        assert_eq!(station.tiles[0].tile, TileCoord::new(3, 2));
        assert_eq!(station.tiles[0].random_bits, 0xA5);
        assert_eq!(station.tiles[0].animation_frame, 6);
    }

    #[test]
    fn decodes_towns_with_fallback_name() {
        let mut t1 = Vec::new();
        t1.extend_from_slice(&(3u32 * 64 + 3).to_be_bytes());
        write_str("Rosario", &mut t1);

        let mut t2 = Vec::new();
        t2.extend_from_slice(&(7u32 * 64 + 1).to_be_bytes());
        write_str("", &mut t2);

        let chunk = RawChunk {
            name: *b"CITY",
            ch_type: CH_TABLE,
            body: build_table_body(&[(6, "xy"), (0x0A | 0x10, "name")], &[t1, t2]),
        };
        let towns = towns_from_chunks(&[chunk], 64, 300);
        assert_eq!(towns.len(), 2);
        assert_eq!(towns[0].name, "Rosario");
        assert_eq!(towns[0].population, 0, "la población se reconstruye aparte");
        assert_eq!(towns[0].pos, TileCoord::new(3, 3));
        assert_eq!(towns[1].name, "Ciudad 2");
    }

    #[test]
    fn town_without_custom_name_uses_native_generator() {
        // grfid=0, townnametype=0x20C0 (inglés original), parts=0 → "Invenville".
        let mut town_gen = Vec::new();
        town_gen.extend_from_slice(&65u32.to_be_bytes()); // (1,1) con w=64
        write_str("", &mut town_gen);
        town_gen.extend_from_slice(&0u32.to_be_bytes()); // townnamegrfid
        town_gen.extend_from_slice(&0x20C0u16.to_be_bytes()); // townnametype
        town_gen.extend_from_slice(&0u32.to_be_bytes()); // townnameparts

        // NewGRF (grfid != 0): no replicable → fallback «Ciudad N».
        let mut grf = Vec::new();
        grf.extend_from_slice(&(2u32 * 64 + 2).to_be_bytes());
        write_str("", &mut grf);
        grf.extend_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
        grf.extend_from_slice(&0x20C0u16.to_be_bytes());
        grf.extend_from_slice(&7u32.to_be_bytes());

        let chunk = RawChunk {
            name: *b"CITY",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (6, "xy"),
                    (0x0A | 0x10, "name"),
                    (6, "townnamegrfid"),
                    (4, "townnametype"),
                    (6, "townnameparts"),
                ],
                &[town_gen, grf],
            ),
        };
        let towns = towns_from_chunks(&[chunk], 64, 300);
        assert_eq!(towns.len(), 2);
        assert_eq!(towns[0].name, "Invenville");
        assert_eq!(towns[1].name, "Ciudad 2");
    }

    #[test]
    fn missing_chunks_yield_empty() {
        assert!(stations_from_chunks(&[], 64, 300).is_empty());
        assert!(towns_from_chunks(&[], 64, 300).is_empty());
        assert!(industries_from_chunks(&[], 64, 300).is_empty());
        assert!(
            vehicles_from_chunks(
                &[],
                64,
                &super::super::orders::SavOrderImport::from_chunks(&[], 300),
                300,
            )
            .is_empty()
        );
        assert!(company_money_from_chunks(&[], 300).is_none());
    }

    #[test]
    fn decodes_industries_with_location_and_type() {
        let mut i1 = Vec::new();
        i1.extend_from_slice(&(5u32 * 64 + 10).to_be_bytes()); // location.tile
        i1.push(2); // location.w
        i1.push(3); // location.h
        i1.push(7); // type
        let chunk = RawChunk {
            name: *b"INDY",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (6, "location.tile"),
                    (2, "location.w"),
                    (2, "location.h"),
                    (2, "type"),
                ],
                &[i1],
            ),
        };
        let industries = industries_from_chunks(&[chunk], 64, 300);
        assert_eq!(industries.len(), 1);
        assert_eq!(industries[0].pos, TileCoord::new(10, 5));
        assert_eq!((industries[0].width, industries[0].height), (2, 3));
        assert_eq!(industries[0].industry_type, 7);
    }

    #[test]
    fn decodes_industry_produced_stock_and_phase_from_nested_table() {
        let record = vec![
            ("location.tile".to_string(), SlValue::Uint(5 * 64 + 10)),
            ("location.w".to_string(), SlValue::Uint(2)),
            ("location.h".to_string(), SlValue::Uint(3)),
            ("type".to_string(), SlValue::Uint(0)),
            ("random_colour".to_string(), SlValue::Uint(14)),
            ("counter".to_string(), SlValue::Uint(123)),
            ("selected_layout".to_string(), SlValue::Uint(2)),
            ("random".to_string(), SlValue::Uint(0xBEEF)),
            ("prod_level".to_string(), SlValue::Uint(32)),
            (
                "accepted".to_string(),
                SlValue::Structs(vec![vec![
                    ("cargo".to_string(), SlValue::Uint(6)),
                    ("waiting".to_string(), SlValue::Uint(15)),
                ]]),
            ),
            (
                "produced".to_string(),
                SlValue::Structs(vec![vec![
                    ("cargo".to_string(), SlValue::Uint(1)),
                    ("waiting".to_string(), SlValue::Uint(77)),
                    ("rate".to_string(), SlValue::Uint(15)),
                ]]),
            ),
        ];

        let industry = sav_industry_from_record(9, &record, 64).expect("INDY record");

        assert_eq!(industry.industry_id, 9);
        assert_eq!(industry.counter, 123);
        assert_eq!(industry.selected_layout, 2);
        assert_eq!(industry.random, 0xBEEF);
        assert_eq!(industry.prod_level, 32);
        assert_eq!(industry.produced.len(), 1);
        assert_eq!(industry.produced[0].cargo_slot, 1);
        assert_eq!(industry.produced[0].waiting, 77);
        assert_eq!(industry.accepted[0].cargo_slot, 6);
        assert_eq!(industry.accepted[0].waiting, 15);
    }

    #[test]
    fn decodes_station_cargo_packet_references_and_capa_packet() {
        let station_record = vec![(
            "normal".to_string(),
            SlValue::Structs(vec![vec![(
                "goods".to_string(),
                SlValue::Structs(vec![
                    Vec::new(),
                    vec![
                        ("cargo.reserved_count".to_string(), SlValue::Uint(3)),
                        (
                            "cargo".to_string(),
                            SlValue::Structs(vec![vec![(
                                "second".to_string(),
                                // `SLE_REFLIST`: IDs guardados como index + 1.
                                SlValue::List(vec![SlValue::Uint(8), SlValue::Uint(12)]),
                            )]]),
                        ),
                    ],
                ]),
            )]]),
        )];
        let entries = station_cargo_from_record(&station_record);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].cargo_slot, 1);
        assert_eq!(entries[0].packet_ids, vec![7, 11]);
        assert_eq!(entries[0].reserved, 3);

        let packet_record = vec![
            ("source".to_string(), SlValue::Uint(4)),
            ("source_xy".to_string(), SlValue::Uint(2 * 64 + 5)),
            ("loaded_at_xy".to_string(), SlValue::Uint(9)),
            ("count".to_string(), SlValue::Uint(42)),
            ("periods_in_transit".to_string(), SlValue::Uint(7)),
            ("feeder_share".to_string(), SlValue::Int(-12)),
        ];
        let packet = sav_cargo_packet_from_record(7, &packet_record, 64).expect("CAPA record");
        assert_eq!(packet.packet_id, 7);
        assert_eq!(packet.source_station_id, Some(4));
        assert_eq!(packet.source_xy, Some(TileCoord::new(5, 2)));
        assert_eq!(packet.next_hop_station_id, Some(9));
        assert_eq!(packet.count, 42);
        assert_eq!(packet.periods_in_transit, 7);
        assert_eq!(packet.feeder_share, -12);
    }

    #[test]
    fn reads_colour_from_first_company() {
        let mut c0 = Vec::new();
        c0.extend_from_slice(&500_000i64.to_be_bytes());
        c0.push(6);
        let chunk = RawChunk {
            name: *b"PLYR",
            ch_type: CH_TABLE,
            body: build_table_body(&[(7, "money"), (2, "colour")], &[c0]),
        };
        assert_eq!(company_colour_from_chunks(&[chunk], 300), Some(6));
    }

    #[test]
    fn reads_money_from_first_company() {
        let mut c0 = Vec::new();
        c0.extend_from_slice(&500_000i64.to_be_bytes());
        let mut c1 = Vec::new();
        c1.extend_from_slice(&(-42i64).to_be_bytes());
        let chunk = RawChunk {
            name: *b"PLYR",
            ch_type: CH_TABLE,
            body: build_table_body(&[(7, "money")], &[c0, c1]),
        };
        assert_eq!(company_money_from_chunks(&[chunk], 300), Some(500_000));
    }

    #[test]
    fn migrates_legacy_company_liveries_to_current_scheme_order() {
        let entries = (0_u8..19)
            .map(|index| {
                let in_use = u64::from(matches!(index, 1 | 4 | 5 | 11 | 12 | 13));
                vec![
                    ("in_use".to_string(), SlValue::Uint(in_use)),
                    ("colour1".to_string(), SlValue::Uint(u64::from(index))),
                    ("colour2".to_string(), SlValue::Uint(u64::from(index) + 20)),
                ]
            })
            .collect();
        let record = vec![("liveries".to_string(), SlValue::Structs(entries))];

        // SLV 62 no tiene las dos libreas de vagones Monorail/Maglev ni las
        // de tranvía, y aún usa el flag binario previo a grupos.
        let liveries = company_liveries_from_record(&record, 6, 62);

        assert_eq!(liveries.len(), crate::company::COMPANY_LIVERY_SCHEME_COUNT);
        assert_eq!(liveries[1].in_use, 3);
        assert_eq!(liveries[11].colour1, 4);
        assert_eq!(liveries[12].colour1, 5);
        assert_eq!(liveries[13].colour1, 11);
        assert_eq!(liveries[14].colour1, 12);
        assert_eq!(liveries[15].colour1, 13);
        assert_eq!(liveries[21], liveries[14]);
        assert_eq!(liveries[22], liveries[15]);
    }

    /// Cuerpo VEHS (sparse) con un tren cabeza, un vagón y un bus.
    fn vehs_chunk() -> RawChunk {
        use super::super::table::tests::write_gamma;

        // Struct presente (gamma 1) + struct `common` (gamma 1) + campos.
        fn with_common(tile: u32, subtype: u8, cargo: u8, buf: &mut Vec<u8>) {
            buf.push(1); // train/roadveh: 1 registro
            buf.push(1); // common: 1 registro
            buf.extend_from_slice(&tile.to_be_bytes());
            buf.push(subtype);
            buf.push(cargo);
        }

        // Header: type u8 + structs train/roadveh (cada uno con struct common).
        let mut header = Vec::new();
        header.push(2);
        write_str("type", &mut header);
        header.push(11);
        write_str("train", &mut header);
        header.push(11);
        write_str("roadveh", &mut header);
        header.push(0);
        for _ in 0..2 {
            // Sub-lista de train/roadveh: un struct `common`…
            header.push(11);
            write_str("common", &mut header);
            header.push(0);
            // …cuyos campos son tile/subtype/cargo_type.
            header.push(6);
            write_str("tile", &mut header);
            header.push(2);
            write_str("subtype", &mut header);
            header.push(2);
            write_str("cargo_type", &mut header);
            header.push(0);
        }

        // Tren cabeza (subtype bit0), en (3,1) con w=64.
        let mut v0 = vec![1u8]; // índice sparse 1
        v0.push(0); // type 0 = tren
        with_common(64 + 3, 0x01, 9, &mut v0);
        v0.push(0); // roadveh ausente

        // Vagón del mismo tren: se omite (sin GVSF_FRONT).
        let mut v1 = vec![2u8];
        v1.push(0);
        with_common(64 + 4, 0x04, 9, &mut v1);
        v1.push(0);

        // Bus (roadveh con pasajeros) en (7,2).
        let mut v2 = vec![3u8];
        v2.push(1); // type 1 = carretera
        v2.push(0); // train ausente
        with_common(2 * 64 + 7, 0x01, 0, &mut v2);

        let mut body = Vec::new();
        write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        for r in [&v0, &v1, &v2] {
            write_gamma(r.len() as u32 + 1, &mut body);
            body.extend_from_slice(r);
        }
        write_gamma(0, &mut body);

        RawChunk {
            name: *b"VEHS",
            ch_type: CH_SPARSE_TABLE,
            body,
        }
    }

    #[test]
    fn decodes_front_vehicles_and_train_wagons() {
        let vehicles = vehicles_from_chunks(
            &[vehs_chunk()],
            64,
            &super::super::orders::SavOrderImport::from_chunks(&[], 300),
            300,
        );
        assert_eq!(vehicles.len(), 3);
        assert_eq!(vehicles[0].kind, SavVehicleKind::Train);
        assert!(!vehicles[0].is_wagon);
        assert_eq!(vehicles[0].pos, TileCoord::new(3, 1));
        assert_eq!(vehicles[0].cargo_type, 9);
        assert_eq!(vehicles[1].kind, SavVehicleKind::Train);
        assert!(vehicles[1].is_wagon);
        assert_eq!(vehicles[1].pos, TileCoord::new(4, 1));
        assert_eq!(vehicles[2].kind, SavVehicleKind::RoadVehicle);
        assert!(!vehicles[2].is_wagon);
        assert_eq!(vehicles[2].pos, TileCoord::new(7, 2));
        assert_eq!(vehicles[2].cargo_type, 0);
    }
}

#[cfg(test)]
mod generated_station_name_tests {
    use super::*;
    use crate::GameState;
    use crate::sav::{chunks, container};

    #[test]
    fn formats_valley_and_north_spanish_templates() {
        assert_eq!(
            format_generated_station_name(0x600E, "Sarnpool Bridge").as_deref(),
            Some("Valle de Sarnpool Bridge")
        );
        assert_eq!(
            format_generated_station_name(0x6007, "Sarnpool Bridge").as_deref(),
            Some("Sarnpool Bridge Norte")
        );
    }

    #[test]
    fn dual_fixture_imports_openttd_generated_station_names()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))?;
        let (data, version) = container::decompress(&raw)?;
        let chunk_list = chunks::parse_chunks(&data)?;
        let map_w = 64;
        let stations = stations_from_chunks(&chunk_list, map_w, version);
        let towns = towns_from_chunks(&chunk_list, map_w, version);
        let names: Vec<_> = stations
            .iter()
            .filter_map(|s| resolve_sav_station_name(s, &towns))
            .collect();
        assert!(
            names.iter().any(|n| n == "Valle de Sarnpool Bridge"),
            "nombres={names:?}"
        );
        assert!(
            names.iter().any(|n| n == "Sarnpool Bridge Norte"),
            "nombres={names:?}"
        );

        let state = GameState::from_sav_game(crate::sav::load(&raw)?);
        let state_names: Vec<_> = state
            .stations
            .iter()
            .filter_map(|s| s.name.clone())
            .collect();
        assert!(state_names.iter().any(|n| n == "Valle de Sarnpool Bridge"));
        assert!(state_names.iter().any(|n| n == "Sarnpool Bridge Norte"));
        Ok(())
    }
}

#[cfg(test)]
mod oil_refinery_colour_tests {
    use super::*;
    use crate::sav::{chunks, container};

    #[test]
    fn dual_fixture_oil_refinery_imports_grey_random_colour()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))?;
        let (data, version) = container::decompress(&raw)?;
        let chunk_list = chunks::parse_chunks(&data)?;
        let industries = industries_from_chunks(&chunk_list, 64, version);
        let refinery = industries
            .iter()
            .find(|i| i.industry_type == 4)
            .ok_or("oil refinery type 4")?;
        assert_eq!(refinery.random_colour, 14, "OpenTTD Grey");
        Ok(())
    }
}
