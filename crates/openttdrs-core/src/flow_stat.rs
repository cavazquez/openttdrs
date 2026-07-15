//! `FlowStat` / `FlowStatMap` simplificados (#49).
//!
//! En `OpenTTD` el MCF rellena shares. Aquí: mapper ingenuo o stub
//! [`crate::mcf`] (`GreedyShortest`). Modo `Manual` ignora flows y usa órdenes.

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
    /// Usa `FlowStat` vía MCF greedy stub (`GreedyShortest`).
    Asymmetric,
    /// Igual que [`Asymmetric`] por ahora (sin matching bidireccional).
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
}

/// Elige `next_hop` según modo de distribución.
#[must_use]
pub fn resolve_next_hop(
    distribution: DistributionType,
    flows: &StationFlows,
    at_station: TileCoord,
    cargo: CargoType,
    origin: TileCoord,
    order_hop: Option<TileCoord>,
) -> Option<TileCoord> {
    match distribution {
        DistributionType::Manual => order_hop,
        DistributionType::Asymmetric | DistributionType::Symmetric => {
            flows.get_via(at_station, cargo, origin).or(order_hop)
        }
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
        assert_eq!(
            resolve_next_hop(
                DistributionType::Manual,
                &flows,
                a,
                CargoType::Coal,
                a,
                Some(TileCoord::new(9, 9))
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
                Some(TileCoord::new(9, 9))
            ),
            Some(b)
        );
    }
}
