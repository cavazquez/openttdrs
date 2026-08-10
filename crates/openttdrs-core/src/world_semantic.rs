//! Contrato semántico por tesela para localizar divergencias de render de `.sav`.
//!
//! `world-raw` prueba que los bytes MAP* llegan intactos. Este stream expresa
//! la siguiente capa: qué significan esos bytes para el motor. Es deliberadamente
//! pequeño, estable y local a la tesela, salvo `other_end`, que usa el resolvedor
//! real de túneles/puentes del candidato para hacer visibles errores de topología.

use crate::bridge_spec::{
    bridge_above_axis_from_mapt, bridge_type_from_m6, rail_bridge_other_end, road_bridge_other_end,
    tunnel_bridge_rail_reserved,
};
use crate::map::rail_bits::{RAIL_TILE_DEPOT, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS};
use crate::map::slope::{resolve_existing_tunnel_end, tile_slope_and_z};
use crate::map::tree_tile_loop::{clear_counter, clear_density, clear_ground_type, tree_count};
use crate::map::water_class::water_class_from_m1;
use crate::map::{Map, Tile, TileCoord};
use crate::rail_type::rail_type_from_tile;
use crate::road_stop_spec::{drive_through_axis_y, is_drive_through_orientation};
use crate::road_type::{road_type_from_tile, tram_road_type_from_tile, tram_track_bits};
use crate::station::{
    STATION_TYPE_DOCK, STATION_TYPE_RAIL_WAYPOINT, station_tile_can_have_pylons,
    station_tile_can_have_wires, station_type_from_m6,
};
use crate::world_raw::WorldRawRegion;
use crate::{decode_rail_reservation_m2_hi, rail_signal_present_mask, rail_signal_state_mask};
use serde::Serialize;
use serde_json::{Value, json};
use std::io::{self, Write};

/// Versión del contrato `world-semantic`.
pub const WORLD_SEMANTIC_SCHEMA_VERSION: u32 = 1;
/// Nombre estable del contrato JSONL.
pub const WORLD_SEMANTIC_CONTRACT: &str = "world-semantic";

/// Contexto de un dump semántico que no puede deducirse de una tesela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSemanticContext {
    pub producer: String,
    pub stage: String,
    pub tick: Option<u64>,
    pub climate: Option<u8>,
    pub openttd_commit: String,
    pub source_path: String,
    pub save_sha256: String,
    pub save_version: Option<u16>,
    pub region: Option<WorldRawRegion>,
}

/// Primera fila del stream JSONL `world-semantic`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorldSemanticMetadata {
    #[serde(rename = "kind")]
    pub record_kind: &'static str,
    pub schema_version: u32,
    pub contract: &'static str,
    pub producer: String,
    pub stage: String,
    pub tick: Option<u64>,
    pub climate: Option<u8>,
    pub openttd_commit: String,
    pub source_path: String,
    pub save_sha256: String,
    pub save_version: Option<u16>,
    pub width: u32,
    pub height: u32,
    pub tile_count: u64,
    pub emitted_tile_count: u64,
    pub region: Option<WorldRawRegion>,
}

impl WorldSemanticMetadata {
    /// Crea la cabecera consistente con el mapa y el filtro solicitado.
    #[must_use]
    pub fn for_map(map: &Map, context: &WorldSemanticContext) -> Self {
        let (width, height) = map.dimensions();
        Self {
            record_kind: "metadata",
            schema_version: WORLD_SEMANTIC_SCHEMA_VERSION,
            contract: WORLD_SEMANTIC_CONTRACT,
            producer: context.producer.clone(),
            stage: context.stage.clone(),
            tick: context.tick,
            climate: context.climate,
            openttd_commit: context.openttd_commit.clone(),
            source_path: context.source_path.clone(),
            save_sha256: context.save_sha256.clone(),
            save_version: context.save_version,
            width,
            height,
            tile_count: u64::from(width) * u64::from(height),
            emitted_tile_count: emitted_tile_count(width, height, context.region),
            region: context.region,
        }
    }
}

/// Bytes MAP* que explican una fila semántica. Se mantienen como contexto de
/// diagnóstico; la igualdad se evalúa también sobre los campos interpretados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WorldSemanticRawTile {
    pub height: u8,
    #[serde(rename = "type")]
    pub tile_type: u8,
    pub m1: u8,
    pub m2: u16,
    pub m3: u8,
    pub m4: u8,
    pub m5: u8,
    pub m6: u8,
    pub m7: u8,
    pub m8: u16,
}

impl From<Tile> for WorldSemanticRawTile {
    fn from(tile: Tile) -> Self {
        Self {
            height: tile.height,
            tile_type: tile.mapt,
            m1: tile.m1,
            m2: u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8),
            m3: tile.m3,
            m4: tile.m3hi,
            m5: tile.m5,
            m6: tile.m6,
            m7: tile.m7,
            m8: tile.m8,
        }
    }
}

/// Una tesela ya interpretada por el candidato.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorldSemanticTile {
    #[serde(rename = "kind")]
    pub record_kind: &'static str,
    pub index: u64,
    pub x: u32,
    pub y: u32,
    #[serde(rename = "tile_type")]
    pub tile_type_nibble: u8,
    #[serde(rename = "class")]
    pub semantic_class: &'static str,
    pub tileh: u8,
    pub base_z: u8,
    pub owner: Option<u8>,
    pub bridge_above_axis: Option<u8>,
    pub supported: bool,
    pub unsupported_reason: Option<&'static str>,
    pub raw: WorldSemanticRawTile,
    pub details: Value,
}

impl WorldSemanticTile {
    fn from_map_tile(index: u64, x: u32, y: u32, coord: TileCoord, map: &Map, tile: Tile) -> Self {
        let raw = WorldSemanticRawTile::from(tile);
        let tile_type = tile.ottd_type_nibble();
        let (semantic_class, supported, unsupported_reason) = class_for_type(tile_type);
        let (tileh, base_z) = tile_slope_and_z(map, coord).unwrap_or((0, 0));
        Self {
            record_kind: "tile_semantic",
            index,
            x,
            y,
            tile_type_nibble: tile_type,
            semantic_class,
            tileh,
            base_z,
            owner: owner_for_type(tile_type, tile),
            bridge_above_axis: bridge_above_axis_from_mapt(tile.mapt).map(u8::from),
            supported,
            unsupported_reason,
            raw,
            details: semantic_details(map, coord, tile, tile_type),
        }
    }
}

/// Resultado compacto de una escritura de stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldSemanticDumpSummary {
    pub emitted_tile_count: u64,
}

/// Escribe el dump semántico en orden fila-mayor, sin acumular el mapa entero.
///
/// # Errors
///
/// Devuelve el error del escritor o una incoherencia de dimensiones/filtro.
pub fn write_world_semantic_jsonl<W: Write>(
    writer: &mut W,
    metadata: &WorldSemanticMetadata,
    map: &Map,
) -> io::Result<WorldSemanticDumpSummary> {
    let (width, height) = map.dimensions();
    if metadata.width != width || metadata.height != height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "world-semantic metadata no coincide con las dimensiones del mapa",
        ));
    }
    write_json_line(writer, metadata)?;
    let Some((min_x, min_y, max_x, max_y)) = effective_bounds(width, height, metadata.region)
    else {
        return Ok(WorldSemanticDumpSummary {
            emitted_tile_count: 0,
        });
    };

    let mut emitted = 0_u64;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let coord = TileCoord::new(
                i32::try_from(x).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "x fuera del rango i32")
                })?,
                i32::try_from(y).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "y fuera del rango i32")
                })?,
            );
            let tile = map.get(coord).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "world-semantic encontró una tesela fuera de mapa",
                )
            })?;
            let index = u64::from(y) * u64::from(width) + u64::from(x);
            write_json_line(
                writer,
                &WorldSemanticTile::from_map_tile(index, x, y, coord, map, tile),
            )?;
            emitted += 1;
        }
    }
    if emitted != metadata.emitted_tile_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "world-semantic emitió una cantidad distinta a metadata",
        ));
    }
    Ok(WorldSemanticDumpSummary {
        emitted_tile_count: emitted,
    })
}

fn write_json_line<W: Write, T: Serialize>(writer: &mut W, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| io::Error::other(error.to_string()))?;
    writer.write_all(b"\n")
}

fn class_for_type(tile_type: u8) -> (&'static str, bool, Option<&'static str>) {
    match tile_type {
        0 => ("clear", true, None),
        1 => ("railway", true, None),
        2 => ("road", true, None),
        3 => ("house", true, None),
        4 => ("trees", true, None),
        5 => ("station", true, None),
        6 => ("water", true, None),
        7 => ("void", true, None),
        8 => ("industry", true, None),
        9 => ("tunnel_bridge", true, None),
        10 => ("object", true, None),
        _ => ("unknown", false, Some("tile_type")),
    }
}

fn owner_for_type(tile_type: u8, tile: Tile) -> Option<u8> {
    // `GetTileOwner` no está definido para estas tres familias en OpenTTD.
    if matches!(tile_type, 3 | 7 | 8) {
        None
    } else {
        Some(tile.m1 & 0x1f)
    }
}

fn semantic_details(map: &Map, coord: TileCoord, tile: Tile, tile_type: u8) -> Value {
    match tile_type {
        0 => clear_details(tile),
        1 => railway_details(tile),
        2 => road_details(tile),
        3 => house_details(tile),
        4 => tree_details(tile),
        5 => station_details(tile),
        6 => water_details(tile),
        7 => json!({"family": "void"}),
        8 => industry_details(tile),
        9 => tunnel_bridge_details(map, coord, tile),
        10 => object_details(tile),
        _ => json!({"family": "unknown"}),
    }
}

fn clear_details(tile: Tile) -> Value {
    let ground = clear_ground_type(tile.m5);
    json!({
        "family": "clear",
        "ground": ground,
        "density": clear_density(tile.m5),
        "counter": clear_counter(tile.m5),
        "field_type": (ground == 3).then_some(tile.m3 & 0x0f),
        "snow": tile.m3 & 0x10 != 0,
    })
}

fn railway_details(tile: Tile) -> Value {
    let rail_tile_type = (tile.m5 >> 6) & 0x03;
    let is_plain = matches!(rail_tile_type, RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS);
    let is_signal = rail_tile_type == RAIL_TILE_SIGNALS;
    json!({
        "family": "railway",
        "rail_tile_type": rail_tile_type,
        "track_bits": is_plain.then_some(tile.m5 & 0x3f),
        "rail_type": rail_type_from_tile(tile).as_u8(),
        "depot_direction": (rail_tile_type == RAIL_TILE_DEPOT).then_some(tile.m5 & 0x03),
        "signal_present": is_signal.then_some(rail_signal_present_mask(tile.m3)),
        "signal_state": is_signal.then_some(rail_signal_state_mask(tile.m3hi)),
        "reservation_track_bits": is_plain.then_some(decode_rail_reservation_m2_hi(tile.m2_hi)),
    })
}

fn road_details(tile: Tile) -> Value {
    let road_tile_type = (tile.m5 >> 6) & 0x03;
    let is_normal = road_tile_type == 0;
    json!({
        "family": "road",
        "road_tile_type": road_tile_type,
        "road_bits": is_normal.then_some(tile.m5 & 0x0f),
        "tram_bits": is_normal.then_some(tram_track_bits(&tile)),
        // Esta llamada es intencional: compara el helper que consume el renderer,
        // no una segunda copia del layout de OpenTTD.
        "road_type": road_type_from_tile(&tile).as_u8(),
        "tram_type": tram_road_type_from_tile(&tile).map(crate::road_type::RoadType::as_u8),
        "crossing_road_axis": (road_tile_type == 1).then_some(tile.m5 & 0x01),
        "crossing_rail_axis": (road_tile_type == 1).then_some((tile.m5 & 0x01) ^ 1),
        "depot_direction": (road_tile_type == 2).then_some(tile.m5 & 0x03),
        "roadside": (tile.m6 >> 3) & 0x07,
    })
}

fn house_details(tile: Tile) -> Value {
    let completed = tile.m3 & 0x80 != 0;
    json!({
        "family": "house",
        "town_id": u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8),
        "house_type": tile.m8 & 0x0fff,
        "completed": completed,
        "building_stage": if completed { 3 } else { (tile.m5 >> 3) & 0x03 },
    })
}

fn tree_details(tile: Tile) -> Value {
    let m2 = u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8);
    json!({
        "family": "trees",
        "tree_type": tile.m3,
        "ground": (m2 >> 6) & 0x07,
        "density": (m2 >> 4) & 0x03,
        "count": tree_count(tile.m5),
        "growth": tile.m5 & 0x07,
        "water_class": water_class_from_m1(tile.m1).as_u8(),
    })
}

fn station_details(tile: Tile) -> Value {
    let station_type = station_type_from_m6(tile.m6);
    let gfx = tile.m5;
    let has_rail = station_type == 0 || station_type == STATION_TYPE_RAIL_WAYPOINT;
    let is_road_stop = matches!(station_type, 2 | 3 | 8);
    let is_bay = matches!(station_type, 2 | 3) && gfx < 4;
    let is_drive_through = is_road_stop && is_drive_through_orientation(gfx);
    let is_dock = station_type == STATION_TYPE_DOCK;
    json!({
        "family": "station",
        "station_id": u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8),
        "station_type": station_type,
        "station_gfx": gfx,
        "rail_type": has_rail.then_some(rail_type_from_tile(tile).as_u8()),
        "rail_axis": has_rail.then_some(u8::from(gfx & 0x01 != 0)),
        "catenary_wires": has_rail.then_some(station_tile_can_have_wires(tile.m3)),
        "catenary_pylons": has_rail.then_some(station_tile_can_have_pylons(tile.m3)),
        "station_custom_spec": has_rail.then_some(tile.m3hi),
        "road_stop_layout": if is_bay {
            Some("bay")
        } else if is_drive_through {
            Some("drive_through")
        } else {
            None
        },
        "road_stop_bay_direction": is_bay.then_some(gfx),
        "road_stop_drive_through_axis": is_drive_through.then_some(u8::from(drive_through_axis_y(gfx))),
        "road_stop_custom_spec": is_road_stop.then_some((tile.m8 & 0x003f) as u8),
        "dock_water_part": is_dock.then_some(gfx >= 4),
        "dock_direction": (is_dock && gfx < 4).then_some(gfx),
    })
}

fn water_details(tile: Tile) -> Value {
    let water_tile_type = (tile.m5 >> 4) & 0x0f;
    let is_depot = water_tile_type == 3;
    let is_lock = water_tile_type == 2;
    let depot_axis = (tile.m5 >> 1) & 0x01;
    let depot_part = tile.m5 & 0x01;
    json!({
        "family": "water",
        "water_tile_type": water_tile_type,
        "water_class": water_class_from_m1(tile.m1).as_u8(),
        "ship_depot_axis": is_depot.then_some(depot_axis),
        "ship_depot_part": is_depot.then_some(depot_part),
        "ship_depot_direction": is_depot.then_some((depot_axis * 3) ^ (depot_part * 2)),
        "lock_direction": is_lock.then_some(tile.m5 & 0x03),
        "lock_part": is_lock.then_some((tile.m5 >> 2) & 0x03),
    })
}

fn industry_details(tile: Tile) -> Value {
    let completed = tile.m1 & 0x80 != 0;
    json!({
        "family": "industry",
        "industry_id": u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8),
        "completed": completed,
        "construction_stage": if completed { 3 } else { tile.m1 & 0x03 },
        "gfx": u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 0x01) << 8),
    })
}

fn tunnel_bridge_details(map: &Map, coord: TileCoord, tile: Tile) -> Value {
    let is_tunnel = tile.m5 & 0x80 == 0;
    let transport_type = (tile.m5 >> 2) & 0x03;
    let other_end = if is_tunnel {
        resolve_existing_tunnel_end(map, coord)
    } else if transport_type == 0 {
        rail_bridge_other_end(map, coord)
    } else {
        road_bridge_other_end(map, coord)
    };
    let other_end = other_end.map(|end| json!({"x": end.x, "y": end.y}));
    json!({
        "family": "tunnel_bridge",
        "is_tunnel": is_tunnel,
        "transport_type": transport_type,
        "direction": tile.m5 & 0x03,
        "other_end": other_end,
        "bridge_type": (!is_tunnel).then_some(bridge_type_from_m6(tile.m6).as_u8()),
        "rail_type": (transport_type == 0).then_some(rail_type_from_tile(tile).as_u8()),
        "road_type": (transport_type != 0).then_some(road_type_from_tile(&tile).as_u8()),
        "tram_type": (transport_type != 0)
            .then(|| tram_road_type_from_tile(&tile).map(crate::road_type::RoadType::as_u8))
            .flatten(),
        "rail_reserved": (transport_type == 0).then_some(tunnel_bridge_rail_reserved(tile)),
    })
}

fn object_details(tile: Tile) -> Value {
    json!({
        "family": "object",
        "object_id": (u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8)) | (u32::from(tile.m5) << 16),
        "random": tile.m3,
    })
}

fn effective_bounds(
    width: u32,
    height: u32,
    region: Option<WorldRawRegion>,
) -> Option<(u32, u32, u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let full = WorldRawRegion {
        min_x: 0,
        min_y: 0,
        max_x: width - 1,
        max_y: height - 1,
    };
    let requested = region.unwrap_or(full);
    let min_x = requested.min_x.max(full.min_x);
    let min_y = requested.min_y.max(full.min_y);
    let max_x = requested.max_x.min(full.max_x);
    let max_y = requested.max_y.min(full.max_y);
    (min_x <= max_x && min_y <= max_y).then_some((min_x, min_y, max_x, max_y))
}

fn emitted_tile_count(width: u32, height: u32, region: Option<WorldRawRegion>) -> u64 {
    let Some((min_x, min_y, max_x, max_y)) = effective_bounds(width, height, region) else {
        return 0;
    };
    u64::from(max_x - min_x + 1) * u64::from(max_y - min_y + 1)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{WorldSemanticContext, WorldSemanticMetadata, write_world_semantic_jsonl};
    use crate::map::{Map, TileCoord, TileKind};

    #[test]
    fn road_row_uses_m4_road_type_and_keeps_raw_context() {
        let mut map = Map::new_flat(2, 2, 0);
        let coord = TileCoord::new(0, 0);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = TileKind::Road;
        tile.mapt = 0x20;
        tile.m5 = 0x0f;
        tile.m3hi = 7;
        tile.m8 = 2;
        map.set_tile(coord, tile).expect("set tile");
        let context = WorldSemanticContext {
            producer: "openttdrs".to_string(),
            stage: "sav_map".to_string(),
            tick: Some(1),
            climate: Some(0),
            openttd_commit: String::new(),
            source_path: "/tmp/test.sav".to_string(),
            save_sha256: "a".repeat(64),
            save_version: Some(300),
            region: None,
        };
        let metadata = WorldSemanticMetadata::for_map(&map, &context);
        let mut out = Vec::new();
        write_world_semantic_jsonl(&mut out, &metadata, &map).expect("dump");
        let rows: Vec<serde_json::Value> = std::str::from_utf8(&out)
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(rows[1]["kind"], "tile_semantic");
        assert_eq!(rows[1]["details"]["road_type"], 7);
        assert_eq!(rows[1]["raw"]["m4"], 7);
        assert_eq!(rows[1]["raw"]["m8"], 2);
    }

    #[test]
    fn region_preserves_absolute_index() {
        let map = Map::new_flat(4, 3, 0);
        let context = WorldSemanticContext {
            producer: "openttdrs".to_string(),
            stage: "sav_map".to_string(),
            tick: None,
            climate: None,
            openttd_commit: String::new(),
            source_path: String::new(),
            save_sha256: String::new(),
            save_version: None,
            region: Some(crate::world_raw::WorldRawRegion::new(2, 1, 8, 8).expect("region")),
        };
        let metadata = WorldSemanticMetadata::for_map(&map, &context);
        let mut out = Vec::new();
        write_world_semantic_jsonl(&mut out, &metadata, &map).expect("dump");
        let first = std::str::from_utf8(&out)
            .expect("utf8")
            .lines()
            .nth(1)
            .expect("first tile");
        let first: serde_json::Value = serde_json::from_str(first).expect("json");
        assert_eq!(first["index"], 6);
        assert_eq!(first["x"], 2);
        assert_eq!(first["y"], 1);
    }
}
