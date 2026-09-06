//! Chunks `LGRP` / `LGRJ` / `LGRS` (#102).
//!
//! Decodifica/escribe el grafo observado (`LGRP` → [`LinkGraphStats`]) y los
//! jobs en vuelo de `CargoDist` (`LGRJ`/`LGRS`).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::type_complexity
)]

use std::collections::{BTreeMap, HashMap};

use crate::Climate;
use crate::cargo::{CARGO_CLASS_ARMOURED, CARGO_CLASS_MAIL, CARGO_CLASS_PASSENGERS, CargoType};
use crate::cargodist::legacy::flow_stat::{
    CargoDistPerCargoSettings, DistributionType as GameDistribution, ECONOMY_SECONDS_PER_DAY,
};
use crate::cargodist::parity::{BaseEdge, BaseNode, DistributionType, Job, LinkGraphSettings};
use crate::link_graph::{LinkEdgeKey, LinkFlowSample, LinkGraphRuntimeChunk, LinkGraphStats};
use crate::map::{TileCoord, coord_from_linear_index, coord_to_linear_index};

use super::SavError;
use super::chunks::{CH_SPARSE_TABLE, RawChunk, find_chunk};
use super::entities::SavStationIndex;
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};

/// ID global de cargo del `.sav` moderno → [`CargoType`].
#[must_use]
#[allow(dead_code)]
pub(crate) fn cargo_from_openttd_id(id: u8) -> Option<CargoType> {
    cargo_from_openttd_id_in(Climate::Temperate, id)
}

/// Resuelve el cargo del ID global de `OpenTTD`.
///
/// `LGRP` sólo existe en el formato de tablas (`SLV_295+`), cuando los cargos
/// ya dejaron de ser slots relativos al clima. Se mantiene `climate` en la
/// firma para no romper los callers históricos y como fallback para fixtures
/// que construyen registros legacy a mano.
#[must_use]
pub(crate) fn cargo_from_openttd_id_in(_climate: Climate, id: u8) -> Option<CargoType> {
    CargoType::from_cargo_id(id)
}

#[must_use]
pub(crate) fn cargo_to_openttd_id(cargo: CargoType) -> u8 {
    cargo_to_openttd_id_in(Climate::Temperate, cargo)
}

#[must_use]
pub(crate) fn cargo_to_openttd_id_in(_climate: Climate, cargo: CargoType) -> u8 {
    cargo.cargo_id()
}

/// Snapshot de un job `LGRJ` que todavía no fue integrado por `JoinNext`.
///
/// `OpenTTD` serializa el grafo completo junto con la fecha de unión; mantener
/// el `Job` ya materializado permite reanudar el pipeline sin reconstruirlo a
/// partir del grafo observado (que puede haber cambiado después del spawn).
#[derive(Debug, Clone)]
pub(crate) struct SavLinkGraphJob {
    pub(crate) pool_index: u32,
    pub(crate) join_date: u32,
    pub(crate) graph_index: u16,
    pub(crate) cargo: CargoType,
    pub(crate) job: Job,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SavLinkGraphSchedule {
    pub(crate) chunk_present: bool,
    /// Referencias a `LinkGraph` que `OpenTTD` conserva para `SpawnNext`.
    pub(crate) schedule: Vec<u32>,
    /// Referencias a jobs actualmente en ejecución (`JoinNext`).
    pub(crate) running: Vec<u32>,
}

fn record_u64(record: &SlRecord, name: &str) -> Option<u64> {
    record_get(record, name).and_then(SlValue::as_u64)
}

fn record_u8(record: &SlRecord, name: &str, default: u8) -> u8 {
    record_u64(record, name)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(default)
}

fn distribution(value: u8) -> GameDistribution {
    GameDistribution::from_openttd(value).unwrap_or(GameDistribution::Manual)
}

fn distribution_for_job(cargo: CargoType, settings: CargoDistPerCargoSettings) -> DistributionType {
    let classes = cargo.classes();
    let distribution = if classes & CARGO_CLASS_PASSENGERS != 0 {
        settings.distribution_pax
    } else if classes & CARGO_CLASS_MAIL != 0 {
        settings.distribution_mail
    } else if classes & CARGO_CLASS_ARMOURED != 0 {
        settings.distribution_armoured
    } else {
        settings.distribution_default
    };
    match distribution {
        GameDistribution::Manual => DistributionType::Manual,
        GameDistribution::Asymmetric => DistributionType::Asymmetric,
        GameDistribution::Symmetric => DistributionType::Symmetric,
    }
}

fn job_settings(record: &SlRecord, cargo: CargoType, map_w: u32, map_h: u32) -> LinkGraphSettings {
    let settings = CargoDistPerCargoSettings {
        recalc_interval_seconds: record_u64(record, "linkgraph.recalc_interval")
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(8),
        recalc_time_seconds: record_u64(record, "linkgraph.recalc_time")
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(32),
        distribution_pax: distribution(record_u8(record, "linkgraph.distribution_pax", 0)),
        distribution_mail: distribution(record_u8(record, "linkgraph.distribution_mail", 0)),
        distribution_armoured: distribution(record_u8(
            record,
            "linkgraph.distribution_armoured",
            0,
        )),
        distribution_default: distribution(record_u8(record, "linkgraph.distribution_default", 0)),
        accuracy: record_u8(record, "linkgraph.accuracy", 16),
        demand_size: record_u8(record, "linkgraph.demand_size", 100),
        demand_distance: record_u8(record, "linkgraph.demand_distance", 100),
        short_path_saturation: record_u8(record, "linkgraph.short_path_saturation", 80),
    };
    LinkGraphSettings {
        accuracy: u32::from(settings.accuracy),
        demand_size: u32::from(settings.demand_size),
        demand_distance: u32::from(settings.demand_distance),
        short_path_saturation: u32::from(settings.short_path_saturation),
        distribution: distribution_for_job(cargo, settings),
        recalc_time: u32::from(settings.recalc_time_seconds / ECONOMY_SECONDS_PER_DAY).max(1),
        map_max_x: map_w.saturating_sub(1).max(1),
        map_max_y: map_h.saturating_sub(1).max(1),
    }
}

fn parse_runtime_job(
    pool_index: u32,
    record: &SlRecord,
    map_w: u32,
    map_h: u32,
    climate: Climate,
) -> Option<SavLinkGraphJob> {
    let join_date = u32::try_from(record_get(record, "join_date")?.as_i64()?).ok()?;
    let graph_index = u32::try_from(record_u64(record, "link_graph.index")?).ok()?;
    let graph_index_u16 = u16::try_from(graph_index).ok()?;
    let SlValue::Structs(graphs) = record_get(record, "linkgraph")? else {
        return None;
    };
    let graph = graphs.first()?;
    let cargo_id = u8::try_from(record_u64(graph, "cargo")?).ok()?;
    let cargo = cargo_from_openttd_id_in(climate, cargo_id)?;
    let SlValue::Structs(saved_nodes) = record_get(graph, "nodes")? else {
        return None;
    };
    if saved_nodes.len() > usize::from(u16::MAX) {
        return None;
    }

    let mut nodes = Vec::with_capacity(saved_nodes.len());
    for node in saved_nodes {
        let xy = record_u64(node, "xy")?;
        let tile = coord_from_linear_index(xy, map_w)?;
        if tile.x < 0
            || tile.y < 0
            || u32::try_from(tile.x).ok()? >= map_w
            || u32::try_from(tile.y).ok()? >= map_h
        {
            return None;
        }
        nodes.push(BaseNode {
            station: record_u64(node, "station")?.min(u64::from(u32::MAX)) as u32,
            x: u32::try_from(tile.x).ok()?,
            y: u32::try_from(tile.y).ok()?,
            supply: u32::try_from(record_u64(node, "supply")?.min(u64::from(u32::MAX))).ok()?,
            demand: u32::try_from(record_u64(node, "demand")?.min(u64::from(u32::MAX))).ok()?,
        });
    }

    let mut edges = Vec::with_capacity(saved_nodes.len());
    for node in saved_nodes {
        let Some(SlValue::Structs(saved_edges)) = record_get(node, "edges") else {
            edges.push(Vec::new());
            continue;
        };
        let mut node_edges = Vec::with_capacity(saved_edges.len());
        for edge in saved_edges {
            let dest = u16::try_from(record_u64(edge, "dest_node")?).ok()?;
            if usize::from(dest) >= saved_nodes.len() {
                // A corrupt reference must not make the complete save unloadable.
                continue;
            }
            let capacity =
                u32::try_from(record_u64(edge, "capacity")?.min(u64::from(u32::MAX))).ok()?;
            let usage = u32::try_from(record_u64(edge, "usage")?.min(u64::from(u32::MAX))).ok()?;
            let travel_time_sum = record_u64(edge, "travel_time_sum")?;
            let travel_time = if capacity == 0 {
                0
            } else {
                u32::try_from(travel_time_sum / u64::from(capacity)).unwrap_or(u32::MAX)
            };
            node_edges.push(BaseEdge {
                dest,
                capacity,
                usage,
                travel_time,
            });
        }
        edges.push(node_edges);
    }

    let settings = job_settings(record, cargo, map_w, map_h);
    // `usage` pertenece a la arista persistida del link graph; `edge_flow`
    // es anotación temporal del pipeline y OpenTTD tampoco la serializa.
    let job = Job::new(nodes, edges, settings);
    Some(SavLinkGraphJob {
        pool_index,
        join_date,
        graph_index: graph_index_u16,
        cargo,
        job,
    })
}

/// Decodifica `LGRJ` y `LGRS` sin tocar las columnas desconocidas de sus
/// chunks originales. Las referencias de `running` se validan al instalar la
/// cola; un índice imposible sólo descarta ese job.
pub(crate) fn linkgraph_runtime_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    map_h: u32,
    climate: Climate,
) -> (Vec<SavLinkGraphJob>, SavLinkGraphSchedule) {
    let mut jobs = Vec::new();
    if let Some(chunk) = find_chunk(chunks, "LGRJ")
        && (chunk.ch_type == super::chunks::CH_TABLE || chunk.ch_type == CH_SPARSE_TABLE)
        && let Ok(rows) = parse_table_chunk(&chunk.body, chunk.ch_type == CH_SPARSE_TABLE)
    {
        for (pool_index, record) in rows {
            if let Some(job) = parse_runtime_job(pool_index, &record, map_w, map_h, climate) {
                jobs.push(job);
            }
        }
    }

    let mut schedule = SavLinkGraphSchedule::default();
    if let Some(chunk) = find_chunk(chunks, "LGRS")
        && (chunk.ch_type == super::chunks::CH_TABLE || chunk.ch_type == CH_SPARSE_TABLE)
        && let Ok(rows) = parse_table_chunk(&chunk.body, chunk.ch_type == CH_SPARSE_TABLE)
        && let Some((_, record)) = rows.first()
    {
        schedule.chunk_present = true;
        if let Some(SlValue::List(values)) = record_get(record, "schedule") {
            schedule.schedule = values
                .iter()
                .filter_map(SlValue::as_u64)
                .map(|v| v as u32)
                .collect();
        }
        if let Some(SlValue::List(values)) = record_get(record, "running") {
            schedule.running = values
                .iter()
                .filter_map(SlValue::as_u64)
                .map(|v| v as u32)
                .collect();
        }
    }
    (jobs, schedule)
}

fn node_tile(
    node: &SlRecord,
    station_index: &HashMap<u32, SavStationIndex>,
    map_w: u32,
) -> Option<TileCoord> {
    if let Some(pos) = record_get(node, "xy")
        .and_then(SlValue::as_u64)
        .and_then(|xy| coord_from_linear_index(xy, map_w))
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
    climate: Climate,
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

    let runtime_chunks = chunks
        .iter()
        .filter(|chunk| {
            (chunk.name == *b"LGRJ" || chunk.name == *b"LGRS")
                && chunk.ch_type == super::chunks::CH_TABLE
        })
        .map(|chunk| LinkGraphRuntimeChunk {
            name: chunk.name,
            ch_type: chunk.ch_type,
            body: chunk.body.clone(),
        })
        .collect();
    let mut out = LinkGraphStats {
        runtime_chunks,
        ..Default::default()
    };
    for (_idx, record) in rows {
        let Some(cargo_id) = record_get(&record, "cargo").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(cargo) = cargo_from_openttd_id_in(climate, cargo_id as u8) else {
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

/// `EconomyTime::INVALID_DATE` en `OpenTTD`.
const LGRP_INVALID_DATE: i32 = -1;
/// `AddEdge(..., Unrestricted)` con `TimerGameEconomy::date = 0`.
const LGRP_EDGE_UNRESTRICTED_AT_EPOCH: i32 = 0;

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
    )?;

    for (i, tile) in tiles.iter().enumerate() {
        let node_id = u16::try_from(i).unwrap_or(u16::MAX);
        let xy = coord_to_linear_index(*tile, map_w)
            .ok_or_else(|| SavError::BadFormat(format!("tile LGRP fuera de mapa: {tile:?}")))?;
        rec.extend_from_slice(&xy.to_be_bytes());
        rec.extend_from_slice(&0_u32.to_be_bytes()); // supply
        rec.extend_from_slice(&0_u32.to_be_bytes()); // demand
        let station = station_ids.get(tile).copied().unwrap_or(u16::MAX);
        rec.extend_from_slice(&station.to_be_bytes());
        rec.extend_from_slice(&LGRP_INVALID_DATE.to_be_bytes()); // last_update

        let node_edges = by_from.get(&node_id).cloned().unwrap_or_default();
        write_gamma(u32::try_from(node_edges.len()).unwrap_or(0), &mut rec)?;
        for (dest, sample) in node_edges {
            let capacity = u32::try_from(sample.capacity_total.min(u64::from(u32::MAX)))
                .unwrap_or(u32::MAX)
                .max(1);
            let usage =
                u32::try_from(sample.units_total.min(u64::from(u32::MAX))).unwrap_or(u32::MAX);
            rec.extend_from_slice(&capacity.to_be_bytes());
            rec.extend_from_slice(&usage.to_be_bytes());
            rec.extend_from_slice(&sample.travel_time_sum.to_be_bytes());
            rec.extend_from_slice(&LGRP_EDGE_UNRESTRICTED_AT_EPOCH.to_be_bytes());
            rec.extend_from_slice(&LGRP_INVALID_DATE.to_be_bytes()); // last_restricted_update
            rec.extend_from_slice(&dest.to_be_bytes());
        }
    }
    Ok(rec)
}

use super::write::codec::{write_gamma, write_str};

/// Construye un chunk TABLE con header y records arbitrarios.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
fn raw_table_chunk(name: [u8; 4], header: &[u8], records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    use super::chunks::CH_TABLE;
    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(CH_TABLE);
    write_gamma(header.len() as u32 + 1, &mut out)?;
    out.extend_from_slice(header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out)?;
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out)?;
    Ok(out)
}

/// Construye el header TABLE de LGRP.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
fn lgrp_table_header() -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    header.push(5);
    write_str("last_compression", &mut header)?;
    header.push(2);
    write_str("cargo", &mut header)?;
    header.push(0x1B);
    write_str("nodes", &mut header)?;
    header.push(0);
    header.push(6);
    write_str("xy", &mut header)?;
    header.push(6);
    write_str("supply", &mut header)?;
    header.push(6);
    write_str("demand", &mut header)?;
    header.push(4);
    write_str("station", &mut header)?;
    header.push(5);
    write_str("last_update", &mut header)?;
    header.push(0x1B);
    write_str("edges", &mut header)?;
    header.push(0);
    header.push(6);
    write_str("capacity", &mut header)?;
    header.push(6);
    write_str("usage", &mut header)?;
    header.push(8);
    write_str("travel_time_sum", &mut header)?;
    header.push(5);
    write_str("last_unrestricted_update", &mut header)?;
    header.push(5);
    write_str("last_restricted_update", &mut header)?;
    header.push(4);
    write_str("dest_node", &mut header)?;
    header.push(0);
    Ok(header)
}

/// Emite `LGRP` (+ `LGRJ`/`LGRS` vacíos) desde el grafo observado.
pub(crate) fn lgrp_records(
    stats: &LinkGraphStats,
    stations: &[crate::station::Station],
    map_w: u32,
) -> Result<Vec<Vec<u8>>, SavError> {
    let station_ids: HashMap<TileCoord, u16> = stations
        .iter()
        .enumerate()
        .filter_map(|(i, s)| u16::try_from(i).ok().map(|id| (s.pos, id)))
        .collect();
    graphs_from_stats(stats)
        .iter()
        .map(|(cargo, tiles, edges)| encode_lgrp_record(*cargo, tiles, edges, map_w, &station_ids))
        .collect()
}

pub(crate) fn encode_lgrp_chunk(
    stats: &LinkGraphStats,
    stations: &[crate::station::Station],
    map_w: u32,
) -> Result<Vec<u8>, SavError> {
    let records = lgrp_records(stats, stations, map_w)?;
    raw_table_chunk(*b"LGRP", &lgrp_table_header()?, &records)
}

pub(crate) fn encode_linkgraph_runtime_chunks(stats: &LinkGraphStats) -> Result<Vec<u8>, SavError> {
    let mut out = Vec::new();
    if stats.runtime_chunks.is_empty() {
        // Sin passthrough válido, no exportar snapshots obsoletos: OpenTTD
        // reconstruirá la cola desde el grafo observado en el siguiente spawn.
        out.extend_from_slice(&raw_table_chunk(*b"LGRJ", &[0], &[])?);
        out.extend_from_slice(&raw_table_chunk(*b"LGRS", &[0], &[])?);
    } else {
        // Preservación opaca: el cuerpo ya contiene header, registros y
        // terminador gamma; no reinterpretar ni reconstruir sus referencias.
        for chunk in &stats.runtime_chunks {
            out.extend_from_slice(&chunk.name);
            out.push(chunk.ch_type);
            out.extend_from_slice(&chunk.body);
        }
    }
    Ok(out)
}

#[allow(dead_code)]
pub(crate) fn encode_linkgraph_chunks(
    stats: &LinkGraphStats,
    stations: &[crate::station::Station],
    map_w: u32,
) -> Result<Vec<u8>, SavError> {
    let mut out = Vec::new();
    // Siempre emitir LGRP (puede estar vacío): OpenTTD lo tolera.
    out.extend_from_slice(&encode_lgrp_chunk(stats, stations, map_w)?);
    out.extend_from_slice(&encode_linkgraph_runtime_chunks(stats)?);
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::sav::chunks::{CH_TABLE, RawChunk};
    use crate::sav::table::tests::write_gamma as tg;

    #[test]
    fn modern_lgrp_uses_global_cargo_ids() {
        assert_eq!(
            cargo_from_openttd_id_in(Climate::SubArctic, 6),
            Some(CargoType::Grain)
        );
        assert_eq!(
            cargo_from_openttd_id_in(Climate::Temperate, 42),
            Some(CargoType::Custom(11))
        );
        assert_eq!(
            cargo_to_openttd_id_in(Climate::SubArctic, CargoType::Wheat),
            11
        );
        assert_eq!(
            cargo_to_openttd_id_in(Climate::Temperate, CargoType::Custom(11)),
            42
        );
    }

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

        let header = lgrp_table_header().expect("header");
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
        let stats =
            link_graph_from_chunks(&chunks, map_w, &HashMap::new(), 350, Climate::Temperate);
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
        let loaded = link_graph_from_chunks(&chunks, 32, &HashMap::new(), 350, Climate::Temperate);
        let sample = loaded.edges[&LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Goods,
        }];
        assert_eq!(sample.units_total, 7);
        assert!(sample.capacity_total >= 40);
        assert_eq!(sample.travel_time(), 120);
    }

    fn lgrp_chunk_bytes(
        stats: &LinkGraphStats,
        stations: &[crate::Station],
        map_w: u32,
    ) -> Vec<u8> {
        let blob = encode_linkgraph_chunks(stats, stations, map_w).expect("encode");
        let chunks = crate::sav::chunks::parse_chunks(&blob).expect("parse");
        assert_eq!(&chunks[0].name, b"LGRP");
        let mut out = Vec::with_capacity(5 + chunks[0].body.len());
        out.extend_from_slice(b"LGRP");
        out.push(CH_TABLE);
        out.extend_from_slice(&chunks[0].body);
        out
    }

    fn fixture_lgrp(name: &str) -> Vec<u8> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/linkgraph")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("leer {}: {e}", path.display()))
    }

    #[test]
    fn lgrp_empty_matches_openttd_dump() {
        let expected = fixture_lgrp("lgrp_empty.bin");
        let got = lgrp_chunk_bytes(&LinkGraphStats::default(), &[], 256);
        assert_eq!(got, expected);
    }

    #[test]
    fn lgrp_two_node_goods_matches_openttd_dump() {
        let expected = fixture_lgrp("lgrp_two_node_goods.bin");
        let a = TileCoord::new(10, 10);
        let b = TileCoord::new(20, 20);
        let mut stats = LinkGraphStats::default();
        stats.record_trip(a, b, CargoType::Goods, 7, 50, 120);
        let stations = [crate::Station::new(a), crate::Station::new(b)];
        let got = lgrp_chunk_bytes(&stats, &stations, 256);
        assert_eq!(got, expected, "got={got:02x?} expected={expected:02x?}");
    }

    #[test]
    fn runtime_chunks_survive_opaque_roundtrip() {
        let stats = LinkGraphStats {
            runtime_chunks: vec![
                LinkGraphRuntimeChunk {
                    name: *b"LGRJ",
                    ch_type: CH_TABLE,
                    body: vec![2, 0, 0],
                },
                LinkGraphRuntimeChunk {
                    name: *b"LGRS",
                    ch_type: CH_TABLE,
                    body: vec![2, 0, 0],
                },
            ],
            ..Default::default()
        };
        let bytes = encode_linkgraph_chunks(&stats, &[], 32).expect("encode");
        let chunks = crate::sav::chunks::parse_chunks(&bytes).expect("parse");
        let runtime: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.name == *b"LGRJ" || chunk.name == *b"LGRS")
            .collect();
        assert_eq!(runtime.len(), 2);
        assert_eq!(runtime[0].body, vec![2, 0, 0]);
        assert_eq!(runtime[1].body, vec![2, 0, 0]);
    }

    #[test]
    fn runtime_chunks_load_and_reemit_unchanged() {
        let stats = LinkGraphStats {
            runtime_chunks: vec![LinkGraphRuntimeChunk {
                name: *b"LGRJ",
                ch_type: CH_TABLE,
                body: vec![2, 0, 0],
            }],
            ..Default::default()
        };
        let bytes = encode_linkgraph_chunks(&stats, &[], 32).expect("encode");
        let chunks = crate::sav::chunks::parse_chunks(&bytes).expect("parse");
        let loaded = link_graph_from_chunks(&chunks, 32, &HashMap::new(), 350, Climate::Temperate);
        assert_eq!(loaded.runtime_chunks, stats.runtime_chunks);

        let reemitted = encode_linkgraph_chunks(&loaded, &[], 32).expect("re-encode");
        let reparsed = crate::sav::chunks::parse_chunks(&reemitted).expect("reparse");
        let original = chunks
            .iter()
            .find(|chunk| chunk.name == *b"LGRJ")
            .expect("original LGRJ");
        let roundtripped = reparsed
            .iter()
            .find(|chunk| chunk.name == *b"LGRJ")
            .expect("roundtripped LGRJ");
        assert_eq!(roundtripped.body, original.body);
    }

    fn runtime_job_header() -> Vec<u8> {
        let mut header = Vec::new();
        for name in ["linkgraph.recalc_interval", "linkgraph.recalc_time"] {
            header.push(4);
            write_str(name, &mut header).expect("header");
        }
        for name in [
            "linkgraph.distribution_pax",
            "linkgraph.distribution_mail",
            "linkgraph.distribution_armoured",
            "linkgraph.distribution_default",
            "linkgraph.accuracy",
            "linkgraph.demand_distance",
            "linkgraph.demand_size",
            "linkgraph.short_path_saturation",
        ] {
            header.push(2);
            write_str(name, &mut header).expect("header");
        }
        header.push(5);
        write_str("join_date", &mut header).expect("header");
        header.push(4);
        write_str("link_graph.index", &mut header).expect("header");
        header.push(0x1B);
        write_str("linkgraph", &mut header).expect("header");
        header.push(0);
        header.push(5);
        write_str("last_compression", &mut header).expect("header");
        header.push(2);
        write_str("cargo", &mut header).expect("header");
        header.push(0x1B);
        write_str("nodes", &mut header).expect("header");
        header.push(0);
        for (kind, name) in [
            (6, "xy"),
            (6, "supply"),
            (6, "demand"),
            (4, "station"),
            (5, "last_update"),
        ] {
            header.push(kind);
            write_str(name, &mut header).expect("header");
        }
        header.push(0x1B);
        write_str("edges", &mut header).expect("header");
        header.push(0);
        for (kind, name) in [
            (6, "capacity"),
            (6, "usage"),
            (8, "travel_time_sum"),
            (5, "last_unrestricted_update"),
            (5, "last_restricted_update"),
            (4, "dest_node"),
        ] {
            header.push(kind);
            write_str(name, &mut header).expect("header");
        }
        header.push(0);
        header
    }

    #[test]
    fn runtime_jobs_decode_snapshot_and_schedule_order() {
        let mut record = Vec::new();
        record.extend_from_slice(&8_u16.to_be_bytes());
        record.extend_from_slice(&32_u16.to_be_bytes());
        record.extend_from_slice(&[0, 1, 2, 1, 16, 100, 100, 80]);
        record.extend_from_slice(&42_i32.to_be_bytes());
        record.extend_from_slice(&7_u16.to_be_bytes());
        tg(1, &mut record);
        record.extend_from_slice(&0_i32.to_be_bytes());
        record.push(1); // Coal
        tg(2, &mut record);
        record.extend_from_slice(&17_u32.to_be_bytes());
        record.extend_from_slice(&10_u32.to_be_bytes());
        record.extend_from_slice(&4_u32.to_be_bytes());
        record.extend_from_slice(&2_u16.to_be_bytes());
        record.extend_from_slice(&0_i32.to_be_bytes());
        tg(1, &mut record);
        record.extend_from_slice(&50_u32.to_be_bytes());
        record.extend_from_slice(&7_u32.to_be_bytes());
        record.extend_from_slice(&500_u64.to_be_bytes());
        record.extend_from_slice(&0_i32.to_be_bytes());
        record.extend_from_slice(&0_i32.to_be_bytes());
        record.extend_from_slice(&1_u16.to_be_bytes());
        record.extend_from_slice(&33_u32.to_be_bytes());
        record.extend_from_slice(&0_u32.to_be_bytes());
        record.extend_from_slice(&8_u32.to_be_bytes());
        record.extend_from_slice(&3_u16.to_be_bytes());
        record.extend_from_slice(&0_i32.to_be_bytes());
        tg(0, &mut record);

        let job_chunk = raw_table_chunk(*b"LGRJ", &runtime_job_header(), &[record]).expect("LGRJ");
        let mut schedule_header = Vec::new();
        schedule_header.push(6 | 0x10);
        write_str("schedule", &mut schedule_header).expect("header");
        schedule_header.push(6 | 0x10);
        write_str("running", &mut schedule_header).expect("header");
        schedule_header.push(0);
        let mut schedule_record = Vec::new();
        tg(1, &mut schedule_record);
        schedule_record.extend_from_slice(&3_u32.to_be_bytes());
        tg(1, &mut schedule_record);
        schedule_record.extend_from_slice(&0_u32.to_be_bytes());
        let schedule_chunk =
            raw_table_chunk(*b"LGRS", &schedule_header, &[schedule_record]).expect("LGRS");
        let mut bytes = job_chunk;
        bytes.extend_from_slice(&schedule_chunk);
        let chunks = crate::sav::chunks::parse_chunks(&bytes).expect("chunks");
        let (jobs, schedule) = linkgraph_runtime_from_chunks(&chunks, 16, 16, Climate::Temperate);
        assert_eq!(schedule.schedule, vec![3]);
        assert_eq!(schedule.running, vec![0]);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].pool_index, 0);
        assert_eq!(jobs[0].join_date, 42);
        assert_eq!(jobs[0].graph_index, 7);
        assert_eq!(jobs[0].cargo, CargoType::Coal);
        assert_eq!(jobs[0].job.nodes[0].station, 2);
        assert_eq!(jobs[0].job.nodes[1].tile(), Some(TileCoord::new(1, 2)));
        assert_eq!(jobs[0].job.edges[0][0].usage, 7);
        assert_eq!(jobs[0].job.edge_flow[0][0], 0);
        assert_eq!(jobs[0].job.edges[0][0].travel_time, 10);
    }

    #[test]
    fn runtime_jobs_reject_out_of_bounds_node_without_aborting_save() {
        let mut record = Vec::new();
        record.extend_from_slice(&8_u16.to_be_bytes());
        record.extend_from_slice(&32_u16.to_be_bytes());
        record.extend_from_slice(&[0; 8]);
        record.extend_from_slice(&1_i32.to_be_bytes());
        record.extend_from_slice(&0_u16.to_be_bytes());
        tg(1, &mut record);
        record.extend_from_slice(&0_i32.to_be_bytes());
        record.push(1);
        tg(1, &mut record);
        record.extend_from_slice(&999_u32.to_be_bytes());
        record.extend_from_slice(&0_u32.to_be_bytes());
        record.extend_from_slice(&0_u32.to_be_bytes());
        record.extend_from_slice(&0_u16.to_be_bytes());
        record.extend_from_slice(&0_i32.to_be_bytes());
        tg(0, &mut record);
        let lgrj = raw_table_chunk(*b"LGRJ", &runtime_job_header(), &[record]).expect("LGRJ");
        let chunks = crate::sav::chunks::parse_chunks(&lgrj).expect("chunks");
        let (jobs, _) = linkgraph_runtime_from_chunks(&chunks, 16, 16, Climate::Temperate);
        assert!(jobs.is_empty());
    }
}
