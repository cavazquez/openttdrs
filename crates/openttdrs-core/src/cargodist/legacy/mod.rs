//! Módulos legacy de `CargoDist` pre-paridad.
//!
//! Estos módulos son anteriores al pipeline completo de paridad de `OpenTTD`.
//! El juego activo usa [`crate::cargodist::parity`].

pub mod flow_stat;
pub mod link_graph;
pub mod mcf;

// Re-exportaciones para compatibilidad
pub use flow_stat::{
    CargoDistSettings, DistributionType, FlowStat, FlowStatMap, PlannedFlowEdge, StationFlowTable,
    StationFlows, resolve_next_hop,
};
pub use link_graph::{LinkEdgeKey, LinkFlowSample, LinkGraphStats};
pub use mcf::{
    MCF_MAX_EDGES, MCF_MAX_NODES, McfAlgorithm, McfConfig, compute_station_flows,
    compute_station_flows_for_distribution, symmetrize_observed_edges,
};
