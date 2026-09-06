//! Construye [`Job`]s desde estaciones + link graph observacional (ingesta MVP).
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::double_must_use,
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::mut_range_bound,
    clippy::needless_range_loop,
    clippy::should_implement_trait
)]

use crate::cargo::{ALL_CARGO_TYPES, CUSTOM_CARGO_COUNT, CargoType};
use crate::cargo_spec::CargoSpecDef;
use crate::cargodist::legacy::flow_stat::{
    CargoDistSettings, DistributionType as GameDistribution, ECONOMY_SECONDS_PER_DAY,
};
use crate::cargodist::legacy::link_graph::LinkGraphStats;
use crate::map::TileCoord;
use crate::station::Station;

use super::types::{BaseEdge, BaseNode, DistributionType, Job, LinkGraphSettings, NodeId};

/// Settings de partida → settings del job de paridad.
#[must_use]
pub fn settings_from_game(
    distribution: GameDistribution,
    map_width: u32,
    map_height: u32,
) -> LinkGraphSettings {
    let distribution = match distribution {
        GameDistribution::Manual => DistributionType::Manual,
        GameDistribution::Asymmetric => DistributionType::Asymmetric,
        GameDistribution::Symmetric => DistributionType::Symmetric,
    };
    LinkGraphSettings {
        distribution,
        map_max_x: map_width.saturating_sub(1).max(1),
        map_max_y: map_height.saturating_sub(1).max(1),
        ..LinkGraphSettings::default()
    }
}

/// Settings persistidos desde PATS para un job de una carga.
///
/// `OpenTTD` resuelve el modo de distribución para cada `CargoSpec`, no una vez
/// para toda la partida. El perfil conserva segundos del SAV, mientras el
/// pipeline trabaja con días económicos como el scheduler nativo.
#[must_use]
pub fn settings_from_cargo_dist(
    cargo_dist: CargoDistSettings,
    cargo: CargoType,
    cargo_catalog: &[CargoSpecDef],
    map_width: u32,
    map_height: u32,
) -> LinkGraphSettings {
    let native = cargo_dist.openttd_settings();
    let distribution = match cargo_dist.distribution_for(cargo, cargo_catalog) {
        GameDistribution::Manual => DistributionType::Manual,
        GameDistribution::Asymmetric => DistributionType::Asymmetric,
        GameDistribution::Symmetric => DistributionType::Symmetric,
    };
    LinkGraphSettings {
        accuracy: u32::from(native.accuracy),
        demand_size: u32::from(native.demand_size),
        demand_distance: u32::from(native.demand_distance),
        short_path_saturation: u32::from(native.short_path_saturation),
        distribution,
        recalc_time: u32::from(native.recalc_time_seconds / ECONOMY_SECONDS_PER_DAY).max(1),
        map_max_x: map_width.saturating_sub(1).max(1),
        map_max_y: map_height.saturating_sub(1).max(1),
    }
}

/// Un job por cargo con nodos (supply/demand) y aristas (capacity/usage).
#[must_use]
pub fn build_jobs_from_game(
    stations: &[Station],
    link_graph: &LinkGraphStats,
    distribution: GameDistribution,
    map_width: u32,
    map_height: u32,
) -> Vec<(CargoType, Job)> {
    let settings = settings_from_game(distribution, map_width, map_height);
    if matches!(distribution, GameDistribution::Manual) {
        return Vec::new();
    }

    let mut cargos = ALL_CARGO_TYPES.to_vec();
    for station in stations {
        for (cargo, _) in station.cargo_stock.custom_entries() {
            if !cargos.contains(&cargo) {
                cargos.push(cargo);
            }
        }
    }
    for key in link_graph.edges.keys() {
        if !cargos.contains(&key.cargo) {
            cargos.push(key.cargo);
        }
    }
    for slot in 0..CUSTOM_CARGO_COUNT {
        let cargo = crate::cargo::custom_cargo(slot);
        if !cargos.contains(&cargo) {
            cargos.push(cargo);
        }
    }
    let mut out = Vec::new();
    for cargo in cargos {
        if let Some(job) = build_job_for_cargo(stations, link_graph, cargo, settings) {
            out.push((cargo, job));
        }
    }
    out
}

/// Variante por perfil PATS: resuelve la distribución para cada cargo,
/// incluidos los cargos que `NewGRF` clasifica como pasajeros, correo o
/// blindados.
#[must_use]
pub fn build_jobs_from_cargo_dist(
    stations: &[Station],
    link_graph: &LinkGraphStats,
    cargo_dist: CargoDistSettings,
    cargo_catalog: &[CargoSpecDef],
    map_width: u32,
    map_height: u32,
) -> Vec<(CargoType, Job)> {
    let mut cargos = ALL_CARGO_TYPES.to_vec();
    for station in stations {
        for (cargo, _) in station.cargo_stock.custom_entries() {
            if !cargos.contains(&cargo) {
                cargos.push(cargo);
            }
        }
    }
    for key in link_graph.edges.keys() {
        if !cargos.contains(&key.cargo) {
            cargos.push(key.cargo);
        }
    }
    for slot in 0..CUSTOM_CARGO_COUNT {
        let cargo = crate::cargo::custom_cargo(slot);
        if !cargos.contains(&cargo) {
            cargos.push(cargo);
        }
    }

    let mut out = Vec::new();
    for cargo in cargos {
        let settings =
            settings_from_cargo_dist(cargo_dist, cargo, cargo_catalog, map_width, map_height);
        if matches!(settings.distribution, DistributionType::Manual) {
            continue;
        }
        if let Some(job) = build_job_for_cargo(stations, link_graph, cargo, settings) {
            out.push((cargo, job));
        }
    }
    out
}

fn build_job_for_cargo(
    stations: &[Station],
    link_graph: &LinkGraphStats,
    cargo: CargoType,
    settings: LinkGraphSettings,
) -> Option<Job> {
    let mut tiles: Vec<TileCoord> = stations.iter().map(|s| s.pos).collect();
    for key in link_graph.edges.keys() {
        if key.cargo != cargo {
            continue;
        }
        tiles.push(key.from);
        tiles.push(key.to);
    }
    tiles.sort_by(|a, b| a.x.cmp(&b.x).then_with(|| a.y.cmp(&b.y)));
    tiles.dedup();
    if tiles.len() < 2 {
        return None;
    }

    let index: std::collections::HashMap<TileCoord, usize> =
        tiles.iter().enumerate().map(|(i, t)| (*t, i)).collect();

    let nodes: Vec<BaseNode> = tiles
        .iter()
        .enumerate()
        .map(|(i, tile)| {
            let station = stations.iter().find(|s| s.pos == *tile);
            let supply = station.map_or(0, |s| s.cargo_stock.get(cargo));
            let demand = station.map_or(0, |s| if s.accepts_cargo(cargo) { 8 } else { 0 });
            BaseNode {
                station: u32::try_from(i).unwrap_or(0),
                x: u32::try_from(tile.x.max(0)).unwrap_or(0),
                y: u32::try_from(tile.y.max(0)).unwrap_or(0),
                supply,
                demand,
            }
        })
        .collect();

    let mut edges: Vec<Vec<BaseEdge>> = vec![Vec::new(); nodes.len()];
    let mut any_edge = false;
    for (key, sample) in &link_graph.edges {
        if key.cargo != cargo {
            continue;
        }
        let Some(&from_i) = index.get(&key.from) else {
            continue;
        };
        let Some(&to_i) = index.get(&key.to) else {
            continue;
        };
        let to = NodeId::try_from(to_i).ok()?;
        let capacity = u32::try_from(sample.capacity_total.min(u64::from(u32::MAX)))
            .unwrap_or(0)
            .max(u32::try_from(sample.units_total.min(u64::from(u32::MAX))).unwrap_or(0))
            .max(1);
        let usage = u32::try_from(sample.units_total.min(u64::from(u32::MAX))).unwrap_or(0);
        edges[from_i].push(BaseEdge {
            dest: to,
            capacity,
            usage,
            travel_time: sample.travel_time(),
        });
        any_edge = true;
    }
    if !any_edge {
        return None;
    }
    for node_edges in &mut edges {
        node_edges.sort_by_key(|e| e.dest);
    }

    let mut nodes = nodes;
    // Sin waiting real: inyectar supply desde capacidad saliente (estadística de enlace).
    for (i, node_edges) in edges.iter().enumerate() {
        let out_cap: u32 = node_edges.iter().map(|e| e.capacity).sum();
        if nodes[i].supply == 0 && out_cap > 0 {
            nodes[i].supply = out_cap;
        }
        if nodes[i].demand == 0 {
            let has_in = edges
                .iter()
                .any(|es| es.iter().any(|e| usize::from(e.dest) == i));
            let has_out = !node_edges.is_empty();
            if has_in && !has_out {
                nodes[i].demand = 8;
            }
        }
    }
    if nodes.iter().all(|n| n.supply == 0) || nodes.iter().all(|n| n.demand == 0) {
        return None;
    }

    Some(Job::new(nodes, edges, settings))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::cargodist::legacy::flow_stat::{
        CargoDistPerCargoSettings, CargoDistSettings, DistributionType as GameDistribution,
    };
    use crate::cargodist::legacy::link_graph::LinkGraphStats;
    use crate::map::TileCoord;
    use crate::station::Station;

    #[test]
    fn ingest_two_stations_builds_job_with_capacity_and_travel_time() {
        let a = TileCoord::new(10, 10);
        let b = TileCoord::new(20, 20);
        let mut stations = vec![Station::new(a), Station::new(b)];
        stations[0].cargo_stock.add(CargoType::Coal, 40);

        let mut link = LinkGraphStats::default();
        link.record_trip(a, b, CargoType::Coal, 10, 50, 120);

        let jobs = build_jobs_from_game(&stations, &link, GameDistribution::Asymmetric, 64, 64);
        let coal = jobs
            .iter()
            .find(|(c, _)| *c == CargoType::Coal)
            .map(|(_, j)| j)
            .expect("job coal");
        assert_eq!(coal.size(), 2);
        assert!(coal.nodes[0].supply >= 40);
        assert!(coal.nodes[1].demand > 0);
        let edge = coal.edges[0]
            .iter()
            .find(|e| e.dest == 1)
            .expect("edge 0→1");
        assert!(edge.capacity >= 50);
        assert_eq!(edge.travel_time, 120);
    }

    #[test]
    fn pats_profile_builds_only_the_enabled_cargo_class_with_its_knobs() {
        let a = TileCoord::new(10, 10);
        let b = TileCoord::new(20, 20);
        let stations = vec![Station::new(a), Station::new(b)];
        let mut link = LinkGraphStats::default();
        link.record_trip(a, b, CargoType::Passengers, 10, 50, 120);
        link.record_trip(a, b, CargoType::Coal, 10, 50, 120);

        let settings = CargoDistSettings {
            per_cargo: Some(CargoDistPerCargoSettings {
                recalc_time_seconds: 18,
                distribution_pax: GameDistribution::Asymmetric,
                distribution_default: GameDistribution::Manual,
                accuracy: 64,
                demand_size: 0,
                demand_distance: 255,
                short_path_saturation: 250,
                ..CargoDistPerCargoSettings::default()
            }),
            ..CargoDistSettings::default()
        };
        let jobs = build_jobs_from_cargo_dist(&stations, &link, settings, &[], 64, 64);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, CargoType::Passengers);
        assert_eq!(
            jobs[0].1.settings.distribution,
            DistributionType::Asymmetric
        );
        assert_eq!(jobs[0].1.settings.accuracy, 64);
        assert_eq!(jobs[0].1.settings.demand_size, 0);
        assert_eq!(jobs[0].1.settings.demand_distance, 255);
        assert_eq!(jobs[0].1.settings.short_path_saturation, 250);
        assert_eq!(jobs[0].1.settings.recalc_time, 9);
    }
}
