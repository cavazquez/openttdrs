//! Port alineado a `OpenTTD`; casts/bucles intencionales.
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

use crate::map::TileCoord;

use super::flow_stat::FlowStatMap;
use super::path::{Path, PathId};

pub type NodeId = u16;

pub const INVALID_NODE: NodeId = u16::MAX;
pub const INVALID_STATION: u32 = u32::MAX;
pub const DAY_TICKS: u32 = 74;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistributionType {
    Asymmetric,
    Symmetric,
    #[default]
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkGraphSettings {
    pub accuracy: u32,
    pub demand_size: u32,
    pub demand_distance: u32,
    pub short_path_saturation: u32,
    pub distribution: DistributionType,
    pub recalc_time: u32,
    pub map_max_x: u32,
    pub map_max_y: u32,
}

impl Default for LinkGraphSettings {
    fn default() -> Self {
        Self {
            accuracy: 16,
            demand_size: 100,
            demand_distance: 100,
            short_path_saturation: 80,
            distribution: DistributionType::Manual,
            recalc_time: 30,
            map_max_x: 256,
            map_max_y: 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseNode {
    pub station: u32,
    pub x: u32,
    pub y: u32,
    pub supply: u32,
    pub demand: u32,
}

impl BaseNode {
    #[must_use]
    pub fn tile(self) -> Option<TileCoord> {
        let x = i32::try_from(self.x).ok()?;
        let y = i32::try_from(self.y).ok()?;
        Some(TileCoord::new(x, y))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseEdge {
    pub dest: u16,
    pub capacity: u32,
    pub usage: u32,
    pub travel_time: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DemandAnnotation {
    pub demand: u32,
    pub unsatisfied_demand: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleShare {
    pub at_station: u32,
    pub origin_station: u32,
    pub via_station: u32,
    pub amount: u32,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub settings: LinkGraphSettings,
    pub nodes: Vec<BaseNode>,
    pub edges: Vec<Vec<BaseEdge>>,
    pub demands: Vec<Vec<DemandAnnotation>>,
    pub undelivered_supply: Vec<u32>,
    pub edge_flow: Vec<Vec<u32>>,
    pub paths: Vec<Vec<PathId>>,
    pub flows: Vec<FlowStatMap>,
    pub path_arena: Vec<Path>,
    pub runtime: u32,
}

impl Job {
    #[must_use]
    pub fn new(
        nodes: Vec<BaseNode>,
        edges: Vec<Vec<BaseEdge>>,
        settings: LinkGraphSettings,
    ) -> Self {
        let size = nodes.len();
        let demands = vec![vec![DemandAnnotation::default(); size]; size];
        let undelivered_supply = nodes.iter().map(|node| node.supply).collect();
        let edge_flow = edges
            .iter()
            .map(|node_edges| vec![0; node_edges.len()])
            .collect();
        let paths = vec![Vec::new(); size];
        let flows = vec![FlowStatMap::default(); size];
        Self {
            settings,
            nodes,
            edges,
            demands,
            undelivered_supply,
            edge_flow,
            paths,
            flows,
            path_arena: Vec::new(),
            runtime: 30,
        }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn size_u16(&self) -> NodeId {
        u16::try_from(self.nodes.len()).unwrap_or(INVALID_NODE)
    }

    #[must_use]
    pub fn node_tile(&self, node: NodeId) -> Option<TileCoord> {
        self.nodes.get(usize::from(node)).copied()?.tile()
    }

    #[must_use]
    pub fn node_id_by_station(&self, station: u32) -> Option<NodeId> {
        self.nodes
            .iter()
            .position(|node| node.station == station)
            .and_then(|index| NodeId::try_from(index).ok())
    }

    #[must_use]
    pub fn edge_index(&self, from: NodeId, to: NodeId) -> Option<usize> {
        self.edges[usize::from(from)]
            .iter()
            .position(|edge| edge.dest == to)
    }

    #[must_use]
    pub fn edge(&self, from: NodeId, to: NodeId) -> Option<&BaseEdge> {
        let edge_index = self.edge_index(from, to)?;
        self.edges.get(usize::from(from))?.get(edge_index)
    }

    #[must_use]
    pub fn edge_flow_value(&self, from: NodeId, to: NodeId) -> Option<u32> {
        let edge_index = self.edge_index(from, to)?;
        self.edge_flow
            .get(usize::from(from))?
            .get(edge_index)
            .copied()
    }

    pub fn add_edge_flow(&mut self, from: NodeId, to: NodeId, flow: u32) {
        if let Some(edge_index) = self.edge_index(from, to) {
            self.edge_flow[usize::from(from)][edge_index] =
                self.edge_flow[usize::from(from)][edge_index].saturating_add(flow);
        }
    }

    pub fn remove_edge_flow(&mut self, from: NodeId, to: NodeId, flow: u32) {
        if let Some(edge_index) = self.edge_index(from, to) {
            let current = self.edge_flow[usize::from(from)][edge_index];
            self.edge_flow[usize::from(from)][edge_index] = current.saturating_sub(flow);
        }
    }

    #[must_use]
    pub fn demand_to(&self, from: NodeId, to: NodeId) -> u32 {
        self.demands[usize::from(from)][usize::from(to)].demand
    }

    #[must_use]
    pub fn unsatisfied_demand_to(&self, from: NodeId, to: NodeId) -> u32 {
        self.demands[usize::from(from)][usize::from(to)].unsatisfied_demand
    }

    pub fn satisfy_demand_to(&mut self, from: NodeId, to: NodeId, amount: u32) {
        let demand = &mut self.demands[usize::from(from)][usize::from(to)];
        demand.unsatisfied_demand = demand.unsatisfied_demand.saturating_sub(amount);
    }

    pub fn deliver_supply(&mut self, from: NodeId, to: NodeId, amount: u32) {
        self.undelivered_supply[usize::from(from)] =
            self.undelivered_supply[usize::from(from)].saturating_sub(amount);
        let demand = &mut self.demands[usize::from(from)][usize::from(to)];
        demand.demand = demand.demand.saturating_add(amount);
        demand.unsatisfied_demand = demand.unsatisfied_demand.saturating_add(amount);
    }

    #[must_use]
    pub fn station(&self, node: NodeId) -> u32 {
        self.nodes[usize::from(node)].station
    }

    pub fn move_path_to_back(&mut self, node: NodeId, path_id: PathId) {
        let path_list = &mut self.paths[usize::from(node)];
        if let Some(position) = path_list.iter().position(|existing| *existing == path_id) {
            let removed = path_list.remove(position);
            path_list.push(removed);
        }
    }

    pub fn clear_paths(&mut self) {
        for path_list in &mut self.paths {
            path_list.clear();
        }
        self.path_arena.clear();
    }
}
