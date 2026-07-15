//! Chunks `LGRP` / `LGRJ` / `LGRS` (#102).
//!
//! MVP: decodifica/escribe el grafo observado (`LGRP` → [`LinkGraphStats`]).
//! `LGRJ`/`LGRS` se exportan vacíos (`OpenTTD` regenera jobs con `SpawnAll`).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::type_complexity
)]

use std::collections::{BTreeMap, HashMap};

use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::link_graph::{LinkEdgeKey, LinkFlowSample, LinkGraphStats};
use crate::map::TileCoord;

use super::SavError;
use super::chunks::{RawChunk, find_chunk};
use super::entities::SavStationIndex;
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};

/// Índice temperate `OpenTTD` ↔ [`CargoType`] (mismo orden que `TEMPERATE_CARGO_TYPES`).
#[must_use]
pub(crate) fn cargo_from_openttd_id(id: u8) -> Option<CargoType> {
    ALL_CARGO_TYPES.get(usize::from(id)).copied()
}

#[must_use]
pub(crate) fn cargo_to_openttd_id(cargo: CargoType) -> u8 {
    ALL_CARGO_TYPES
        .iter()
        .position(|&c| c == cargo)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

fn tile_to_coord(tile: u64, map_w: u32) -> Option<TileCoord> {
    if map_w == 0 {
        return None;
    }
    let x = i32::try_from(tile % u64::from(map_w)).ok()?;
    let y = i32::try_from(tile / u64::from(map_w)).ok()?;
    Some(TileCoord::new(x, y))
}

fn node_tile(
    node: &SlRecord,
    station_index: &HashMap<u32, SavStationIndex>,
    map_w: u32,
) -> Option<TileCoord> {
    if let Some(pos) = record_get(node, "xy")
        .and_then(SlValue::as_u64)
        .and_then(|xy| tile_to_coord(xy, map_w))
    {
        return Some(pos);
    }
    let station = record_get(node, "station").and_then(SlValue::as_u64)?;
    if station >= u64::from(u16::MAX) {
        return None;
    }
    station_index.get(&(station as u32)).map(|s| s.pos)
}

/// Decodifica `LGRP` → estadísticas de aristas observadas.
#[must_use]
pub(crate) fn link_graph_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    station_index: &HashMap<u32, SavStationIndex>,
    save_version: u16,
) -> LinkGraphStats {
    let Some(chunk) = find_chunk(chunks, "LGRP") else {
        return LinkGraphStats::default();
    };
    if save_version < 295 {
        // Pre-table: fuera del MVP (layout matriz/sparse distinto).
        return LinkGraphStats::default();
    }
    let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
        return LinkGraphStats::default();
    };

    let mut out = LinkGraphStats::default();
    for (_idx, record) in rows {
        let Some(cargo_id) = record_get(&record, "cargo").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(cargo) = cargo_from_openttd_id(cargo_id as u8) else {
            continue;
        };
        let Some(SlValue::Structs(nodes)) = record_get(&record, "nodes") else {
            continue;
        };
        let tiles: Vec<Option<TileCoord>> = nodes
            .iter()
            .map(|n| node_tile(n, station_index, map_w))
            .collect();

        for (from_i, node) in nodes.iter().enumerate() {
            let Some(from) = tiles[from_i] else {
                continue;
            };
            let Some(SlValue::Structs(edges)) = record_get(node, "edges") else {
                continue;
            };
            for edge in edges {
                let Some(dest) = record_get(edge, "dest_node").and_then(SlValue::as_u64) else {
                    continue;
                };
                let Some(to) = tiles.get(dest as usize).copied().flatten() else {
                    continue;
                };
                if from == to {
                    continue;
                }
                let capacity = record_get(edge, "capacity")
                    .and_then(SlValue::as_u64)
                    .unwrap_or(0);
                let usage = record_get(edge, "usage")
                    .and_then(SlValue::as_u64)
                    .unwrap_or(0);
                let travel_time_sum = record_get(edge, "travel_time_sum")
                    .and_then(SlValue::as_u64)
                    .unwrap_or(0);
                if capacity == 0 && usage == 0 {
                    continue;
                }
                let key = LinkEdgeKey { from, to, cargo };
                let sample = out.edges.entry(key).or_default();
                sample.capacity_total = sample.capacity_total.saturating_add(capacity);
                sample.units_total = sample.units_total.saturating_add(usage);
                sample.units_month = sample.units_month.saturating_add(usage);
                sample.travel_time_sum = sample.travel_time_sum.saturating_add(travel_time_sum);
            }
        }
    }
    out
}

/// Agrupa aristas de [`LinkGraphStats`] en grafos por cargo (nodos = tiles únicos).
#[must_use]
pub(crate) fn graphs_from_stats(
    stats: &LinkGraphStats,
) -> Vec<(CargoType, Vec<TileCoord>, Vec<(u16, u16, LinkFlowSample)>)> {
    let mut by_cargo: HashMap<CargoType, Vec<&LinkEdgeKey>> = HashMap::new();
    for key in stats.edges.keys() {
        by_cargo.entry(key.cargo).or_default().push(key);
    }
    let mut cargos: Vec<CargoType> = by_cargo.keys().copied().collect();
    cargos.sort_by_key(|c| cargo_to_openttd_id(*c));
    let mut out = Vec::new();
    for cargo in cargos {
        let keys = by_cargo.remove(&cargo).unwrap_or_default();
        let mut tiles: Vec<TileCoord> = Vec::new();
        for key in &keys {
            tiles.push(key.from);
            tiles.push(key.to);
        }
        tiles.sort_by(|a, b| a.x.cmp(&b.x).then_with(|| a.y.cmp(&b.y)));
        tiles.dedup();
        let index: HashMap<TileCoord, u16> = tiles
            .iter()
            .enumerate()
            .filter_map(|(i, t)| u16::try_from(i).ok().map(|id| (*t, id)))
            .collect();
        let mut edges = Vec::new();
        for key in keys {
            let Some(&from_i) = index.get(&key.from) else {
                continue;
            };
            let Some(&to_i) = index.get(&key.to) else {
                continue;
            };
            let sample = stats.edges.get(key).copied().unwrap_or_default();
            edges.push((from_i, to_i, sample));
        }
        edges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        if !edges.is_empty() {
            out.push((cargo, tiles, edges));
        }
    }
    out
}

/// Construye el cuerpo binario de un registro `LGRP` (sin gamma de longitud).
pub(crate) fn encode_lgrp_record(
    cargo: CargoType,
    tiles: &[TileCoord],
    edges: &[(u16, u16, LinkFlowSample)],
    map_w: u32,
    station_ids: &HashMap<TileCoord, u16>,
) -> Result<Vec<u8>, SavError> {
    let mut by_from: BTreeMap<u16, Vec<(u16, LinkFlowSample)>> = BTreeMap::new();
    for &(from, to, sample) in edges {
        by_from.entry(from).or_default().push((to, sample));
    }

    let mut rec = Vec::new();
    rec.extend_from_slice(&0_i32.to_be_bytes()); // last_compression
    rec.push(cargo_to_openttd_id(cargo));
    write_gamma(
        u32::try_from(tiles.len())
            .map_err(|_| SavError::BadFormat("demasiados nodos LGRP".into()))?,
        &mut rec,
    );

    for (i, tile) in tiles.iter().enumerate() {
        let node_id = u16::try_from(i).unwrap_or(u16::MAX);
        let xy = tile_index(*tile, map_w)
            .ok_or_else(|| SavError::BadFormat(format!("tile LGRP fuera de mapa: {tile:?}")))?;
        rec.extend_from_slice(&xy.to_be_bytes());
        rec.extend_from_slice(&0_u32.to_be_bytes()); // supply
        rec.extend_from_slice(&0_u32.to_be_bytes()); // demand
        let station = station_ids.get(tile).copied().unwrap_or(u16::MAX);
        rec.extend_from_slice(&station.to_be_bytes());
        rec.extend_from_slice(&0_i32.to_be_bytes()); // last_update

        let node_edges = by_from.get(&node_id).cloned().unwrap_or_default();
        write_gamma(u32::try_from(node_edges.len()).unwrap_or(0), &mut rec);
        for (dest, sample) in node_edges {
            let capacity = u32::try_from(sample.capacity_total.min(u64::from(u32::MAX)))
                .unwrap_or(u32::MAX)
                .max(1);
            let usage =
                u32::try_from(sample.units_total.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            rec.extend_from_slice(&capacity.to_be_bytes());
            rec.extend_from_slice(&usage.to_be_bytes());
            rec.extend_from_slice(&sample.travel_time_sum.to_be_bytes());
            rec.extend_from_slice(&0_i32.to_be_bytes()); // last_unrestricted_update
            rec.extend_from_slice(&0_i32.to_be_bytes()); // last_restricted_update
            rec.extend_from_slice(&dest.to_be_bytes());
        }
    }
    Ok(rec)
}

fn tile_index(pos: TileCoord, map_w: u32) -> Option<u32> {
    if pos.x < 0 || pos.y < 0 {
        return None;
    }
    let ux = u32::try_from(pos.x).ok()?;
    let uy = u32::try_from(pos.y).ok()?;
    Some(uy.saturating_mul(map_w).saturating_add(ux))
}

fn write_gamma(v: u32, buf: &mut Vec<u8>) {
    assert!(v < (1 << 14), "export usa gammas < 2^14");
    if v < (1 << 7) {
        buf.push(v as u8);
    } else {
        buf.push(0x80 | ((v >> 8) as u8));
        buf.push((v & 0xFF) as u8);
    }
}

fn write_str(s: &str, buf: &mut Vec<u8>) {
    write_gamma(s.len() as u32, buf);
    buf.extend_from_slice(s.as_bytes());
}

fn raw_table_chunk(name: [u8; 4], header: &[u8], records: &[Vec<u8>]) -> Vec<u8> {
    use super::chunks::CH_TABLE;
    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(CH_TABLE);
    write_gamma(header.len() as u32 + 1, &mut out);
    out.extend_from_slice(header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out);
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out);
    out
}

fn lgrp_table_header() -> Vec<u8> {
    let mut header = Vec::new();
    header.push(5);
    write_str("last_compression", &mut header);
    header.push(2);
    write_str("cargo", &mut header);
    header.push(0x1B);
    write_str("nodes", &mut header);
    header.push(0);
    header.push(6);
    write_str("xy", &mut header);
    header.push(6);
    write_str("supply", &mut header);
    header.push(6);
    write_str("demand", &mut header);
    header.push(4);
    write_str("station", &mut header);
    header.push(5);
    write_str("last_update", &mut header);
    header.push(0x1B);
    write_str("edges", &mut header);
    header.push(0);
    header.push(6);
    write_str("capacity", &mut header);
    header.push(6);
    write_str("usage", &mut header);
    header.push(8);
    write_str("travel_time_sum", &mut header);
    header.push(5);
    write_str("last_unrestricted_update", &mut header);
    header.push(5);
    write_str("last_restricted_update", &mut header);
    header.push(4);
    write_str("dest_node", &mut header);
    header.push(0);
    header
}

/// Emite `LGRP` (+ `LGRJ`/`LGRS` vacíos) desde el grafo observado.
pub(crate) fn encode_linkgraph_chunks(
    stats: &LinkGraphStats,
    stations: &[crate::station::Station],
    map_w: u32,
) -> Result<Vec<u8>, SavError> {
    let station_ids: HashMap<TileCoord, u16> = stations
        .iter()
        .enumerate()
        .filter_map(|(i, s)| u16::try_from(i).ok().map(|id| (s.pos, id)))
        .collect();

    let graphs = graphs_from_stats(stats);
    let mut records = Vec::new();
    for (cargo, tiles, edges) in &graphs {
        records.push(encode_lgrp_record(
            *cargo,
            tiles,
            edges,
            map_w,
            &station_ids,
        )?);
    }

    let mut out = Vec::new();
    // Siempre emitir LGRP (puede estar vacío): OpenTTD lo tolera.
    out.extend_from_slice(&raw_table_chunk(*b"LGRP", &lgrp_table_header(), &records));
    // Jobs / schedule vacíos (SpawnAll regenera).
    out.extend_from_slice(&raw_table_chunk(*b"LGRJ", &[0], &[]));
    out.extend_from_slice(&raw_table_chunk(
        *b"LGRS",
        &[
            // schedule / running como listas vacías no modeladas: header vacío.
            0,
        ],
        &[],
    ));
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::sav::chunks::{CH_TABLE, RawChunk};
    use crate::sav::table::tests::write_gamma as tg;

    #[test]
    fn decode_synthetic_lgrp_coal_edge() {
        let map_w = 16u32;
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(3, 2);
        let a_xy = (1u32 + map_w).to_be_bytes();
        let b_xy = (3u32 + 2 * map_w).to_be_bytes();

        let mut rec = Vec::new();
        rec.extend_from_slice(&0_i32.to_be_bytes());
        rec.push(1); // Coal
        tg(2, &mut rec);
        rec.extend_from_slice(&a_xy);
        rec.extend_from_slice(&40_u32.to_be_bytes());
        rec.extend_from_slice(&0_u32.to_be_bytes());
        rec.extend_from_slice(&0_u16.to_be_bytes());
        rec.extend_from_slice(&0_i32.to_be_bytes());
        tg(1, &mut rec);
        rec.extend_from_slice(&50_u32.to_be_bytes());
        rec.extend_from_slice(&10_u32.to_be_bytes());
        rec.extend_from_slice(&500_u64.to_be_bytes());
        rec.extend_from_slice(&0_i32.to_be_bytes());
        rec.extend_from_slice(&0_i32.to_be_bytes());
        rec.extend_from_slice(&1_u16.to_be_bytes());
        rec.extend_from_slice(&b_xy);
        rec.extend_from_slice(&0_u32.to_be_bytes());
        rec.extend_from_slice(&8_u32.to_be_bytes());
        rec.extend_from_slice(&1_u16.to_be_bytes());
        rec.extend_from_slice(&0_i32.to_be_bytes());
        tg(0, &mut rec);

        let header = lgrp_table_header();
        let mut body = Vec::new();
        tg(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        tg(rec.len() as u32 + 1, &mut body);
        body.extend_from_slice(&rec);
        tg(0, &mut body);

        let chunks = [RawChunk {
            name: *b"LGRP",
            ch_type: CH_TABLE,
            body,
        }];
        let stats = link_graph_from_chunks(&chunks, map_w, &HashMap::new(), 350);
        let sample = stats.edges[&LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Coal,
        }];
        assert_eq!(sample.capacity_total, 50);
        assert_eq!(sample.units_total, 10);
        assert_eq!(sample.travel_time_sum, 500);
        assert_eq!(sample.travel_time(), 10);
    }

    #[test]
    fn encode_then_decode_roundtrip_in_memory() {
        let mut stats = LinkGraphStats::default();
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(5, 4);
        stats.record_trip(a, b, CargoType::Goods, 7, 40, 120);
        let stations = [crate::Station::new(a), crate::Station::new(b)];
        let bytes = encode_linkgraph_chunks(&stats, &stations, 32).expect("encode");
        // Reparse solo el primer chunk LGRP del blob.
        let chunks = crate::sav::chunks::parse_chunks(&bytes).expect("parse");
        let loaded = link_graph_from_chunks(&chunks, 32, &HashMap::new(), 350);
        let sample = loaded.edges[&LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Goods,
        }];
        assert_eq!(sample.units_total, 7);
        assert!(sample.capacity_total >= 40);
        assert_eq!(sample.travel_time(), 120);
    }
}
