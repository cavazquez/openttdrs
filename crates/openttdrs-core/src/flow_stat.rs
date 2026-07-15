//! `FlowStat` / `FlowStatMap` simplificados (#49).
//!
//! `FlowStat` de partida + `resolve_next_hop`.
//! Los shares los rellena [`crate::linkgraph_parity`] (Demand + MCF1/2).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::link_graph::LinkGraphStats;
use crate::map::TileCoord;

/// Modo de distribución (`linkgraph.distribution_*` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionType {
    /// Sin auto-routing: `next_hop` solo desde órdenes del vehículo.
    #[default]
    Manual,
    /// `FlowStat` vía pipeline `OpenTTD` (Demand Asymmetric + MCF).
    Asymmetric,
    /// Demand Symmetric `OpenTTD` (geografía + supply) + MCF.
    Symmetric,
}

/// Ajustes de `CargoDist` (persistidos; flows se reconstruyen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CargoDistSettings {
    #[serde(default)]
    pub distribution: DistributionType,
}

/// Shares `via → amount` para un origen (`FlowStat`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStat {
    /// Pares (via, amount); amount > 0.
    pub shares: Vec<(TileCoord, u32)>,
}

impl FlowStat {
    pub fn add_share(&mut self, via: TileCoord, amount: u32) {
        if amount == 0 {
            return;
        }
        if let Some((_, acc)) = self.shares.iter_mut().find(|(v, _)| *v == via) {
            *acc = acc.saturating_add(amount);
        } else {
            self.shares.push((via, amount));
        }
    }

    /// Siguiente hop: vía con mayor share (determinista; empate por coords).
    #[must_use]
    pub fn get_via(&self) -> Option<TileCoord> {
        self.shares
            .iter()
            .max_by(|a, b| {
                a.1.cmp(&b.1)
                    .then_with(|| a.0.x.cmp(&b.0.x))
                    .then_with(|| a.0.y.cmp(&b.0.y))
            })
            .map(|(via, _)| *via)
    }

    /// `GetVia` estilo `OpenTTD`: `RandomRange` ponderado por shares.
    pub fn get_via_random(
        &self,
        rng: &mut crate::linkgraph_parity::Randomizer,
    ) -> Option<TileCoord> {
        let total: u32 = self.shares.iter().map(|(_, a)| *a).sum();
        if total == 0 {
            return None;
        }
        let mut pick = rng.random_range(total);
        for (via, amount) in &self.shares {
            if pick < *amount {
                return Some(*via);
            }
            pick = pick.saturating_sub(*amount);
        }
        self.shares.last().map(|(via, _)| *via)
    }

    #[must_use]
    pub fn get_share(&self, via: TileCoord) -> u32 {
        self.shares
            .iter()
            .find(|(v, _)| *v == via)
            .map_or(0, |(_, a)| *a)
    }
}

/// Flows por origen en una estación y un cargo (`FlowStatMap`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStatMap {
    /// `origin → FlowStat`.
    pub by_origin: HashMap<TileCoord, FlowStat>,
}

impl FlowStatMap {
    pub fn add_flow(&mut self, origin: TileCoord, via: TileCoord, amount: u32) {
        if amount == 0 || origin == via {
            return;
        }
        self.by_origin
            .entry(origin)
            .or_default()
            .add_share(via, amount);
    }

    #[must_use]
    pub fn get_via(&self, origin: TileCoord) -> Option<TileCoord> {
        self.by_origin.get(&origin).and_then(FlowStat::get_via)
    }

    pub fn get_via_random(
        &self,
        origin: TileCoord,
        rng: &mut crate::linkgraph_parity::Randomizer,
    ) -> Option<TileCoord> {
        self.by_origin
            .get(&origin)
            .and_then(|fs| fs.get_via_random(rng))
    }
}

/// Tabla de flows de una estación: por cargo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationFlowTable {
    pub by_cargo: HashMap<CargoType, FlowStatMap>,
}

impl StationFlowTable {
    pub fn add_flow(&mut self, cargo: CargoType, origin: TileCoord, via: TileCoord, amount: u32) {
        self.by_cargo
            .entry(cargo)
            .or_default()
            .add_flow(origin, via, amount);
    }

    #[must_use]
    pub fn get_via(&self, cargo: CargoType, origin: TileCoord) -> Option<TileCoord> {
        self.by_cargo.get(&cargo).and_then(|m| m.get_via(origin))
    }

    pub fn get_via_random(
        &self,
        cargo: CargoType,
        origin: TileCoord,
        rng: &mut crate::linkgraph_parity::Randomizer,
    ) -> Option<TileCoord> {
        self.by_cargo
            .get(&cargo)
            .and_then(|m| m.get_via_random(origin, rng))
    }
}

/// Arista planificada agregada desde shares (`estación → via`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedFlowEdge {
    pub from: TileCoord,
    pub to: TileCoord,
    pub cargo: CargoType,
    pub amount: u32,
}

/// Flows por tesela de estación (reconstruidos; no hace falta persistir).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationFlows {
    pub by_station: HashMap<TileCoord, StationFlowTable>,
}

impl StationFlows {
    /// Mapper ingenuo: cada arista observada `from→to` es un share en `from`
    /// con origen=`from` (y también origen genérico si hace falta).
    #[must_use]
    pub fn from_link_graph(graph: &LinkGraphStats) -> Self {
        let mut out = Self::default();
        for (key, sample) in &graph.edges {
            let amount = u32::try_from(sample.units_total.min(u64::from(u32::MAX))).unwrap_or(0);
            if amount == 0 {
                continue;
            }
            // En la estación origen del enlace: cargo con origin=from via=to.
            out.by_station
                .entry(key.from)
                .or_default()
                .add_flow(key.cargo, key.from, key.to, amount);
        }
        out
    }

    #[must_use]
    pub fn get_via(
        &self,
        at_station: TileCoord,
        cargo: CargoType,
        origin: TileCoord,
    ) -> Option<TileCoord> {
        let table = self.by_station.get(&at_station)?;
        table
            .get_via(cargo, origin)
            .or_else(|| table.get_via(cargo, at_station))
    }

    pub fn get_via_random(
        &self,
        at_station: TileCoord,
        cargo: CargoType,
        origin: TileCoord,
        rng: &mut crate::linkgraph_parity::Randomizer,
    ) -> Option<TileCoord> {
        let table = self.by_station.get(&at_station)?;
        table
            .get_via_random(cargo, origin, rng)
            .or_else(|| table.get_via_random(cargo, at_station, rng))
    }

    /// Agrega shares como aristas planificadas (orden: amount desc).
    #[must_use]
    pub fn planned_edges_filtered(
        &self,
        cargo: Option<CargoType>,
        limit: usize,
    ) -> Vec<PlannedFlowEdge> {
        let mut acc: HashMap<(TileCoord, TileCoord, CargoType), u32> = HashMap::new();
        for (station, table) in &self.by_station {
            for (cargo_ty, map) in &table.by_cargo {
                if cargo.is_some_and(|c| c != *cargo_ty) {
                    continue;
                }
                for flow in map.by_origin.values() {
                    for (via, amount) in &flow.shares {
                        if *amount == 0 {
                            continue;
                        }
                        let entry = acc.entry((*station, *via, *cargo_ty)).or_default();
                        *entry = entry.saturating_add(*amount);
                    }
                }
            }
        }
        let mut edges: Vec<PlannedFlowEdge> = acc
            .into_iter()
            .map(|((from, to, cargo), amount)| PlannedFlowEdge {
                from,
                to,
                cargo,
                amount,
            })
            .collect();
        edges.sort_by(|a, b| {
            b.amount
                .cmp(&a.amount)
                .then_with(|| a.from.x.cmp(&b.from.x))
                .then_with(|| a.from.y.cmp(&b.from.y))
                .then_with(|| a.to.x.cmp(&b.to.x))
                .then_with(|| a.to.y.cmp(&b.to.y))
        });
        edges.truncate(limit);
        edges
    }
}

/// Elige `next_hop` según modo de distribución.
pub fn resolve_next_hop(
    distribution: DistributionType,
    flows: &StationFlows,
    at_station: TileCoord,
    cargo: CargoType,
    origin: TileCoord,
    order_hop: Option<TileCoord>,
    rng: &mut crate::linkgraph_parity::Randomizer,
) -> Option<TileCoord> {
    match distribution {
        DistributionType::Manual => order_hop,
        DistributionType::Asymmetric | DistributionType::Symmetric => flows
            .get_via_random(at_station, cargo, origin, rng)
            .or(order_hop),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn get_via_picks_largest_share() {
        let mut fs = FlowStat::default();
        fs.add_share(TileCoord::new(2, 2), 10);
        fs.add_share(TileCoord::new(3, 3), 30);
        fs.add_share(TileCoord::new(4, 4), 5);
        assert_eq!(fs.get_via(), Some(TileCoord::new(3, 3)));
    }

    #[test]
    fn from_link_graph_builds_station_flows() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(5, 5);
        g.record_flow(a, b, CargoType::Coal, 40);
        let flows = StationFlows::from_link_graph(&g);
        assert_eq!(
            flows.get_via(a, CargoType::Coal, a),
            Some(b),
            "desde A el hop de carbón es B"
        );
        let mut rng = crate::linkgraph_parity::Randomizer::new(1);
        assert_eq!(
            resolve_next_hop(
                DistributionType::Manual,
                &flows,
                a,
                CargoType::Coal,
                a,
                Some(TileCoord::new(9, 9)),
                &mut rng,
            ),
            Some(TileCoord::new(9, 9))
        );
        assert_eq!(
            resolve_next_hop(
                DistributionType::Asymmetric,
                &flows,
                a,
                CargoType::Coal,
                a,
                Some(TileCoord::new(9, 9)),
                &mut rng,
            ),
            Some(b)
        );
    }

    #[test]
    fn planned_edges_two_hop() {
        let mut flows = StationFlows::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(3, 3);
        let c = TileCoord::new(5, 5);
        flows
            .by_station
            .entry(a)
            .or_default()
            .add_flow(CargoType::Goods, a, b, 30);
        flows
            .by_station
            .entry(b)
            .or_default()
            .add_flow(CargoType::Goods, a, c, 30);
        let edges = flows.planned_edges_filtered(Some(CargoType::Goods), 10);
        assert!(
            edges
                .iter()
                .any(|e| e.from == a && e.to == b && e.amount == 30)
        );
        assert!(
            edges
                .iter()
                .any(|e| e.from == b && e.to == c && e.amount == 30)
        );
    }
}
