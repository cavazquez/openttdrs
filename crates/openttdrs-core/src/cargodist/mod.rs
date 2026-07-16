//! Sistema de distribución de carga (`CargoDist`).
//!
//! Este módulo unifica:
//! - [`legacy`]: Módulos legacy pre-paridad (`flow_stat`, `mcf`, `link_graph`)
//! - [`parity`]: Pipeline completo de paridad `OpenTTD` (el activo en juego)

pub mod legacy;
pub mod parity;

// Re-exportaciones del pipeline activo para acceso directo
pub use parity::{
    BaseEdge, BaseNode, DistributionType as ParityDistributionType, FlowMapper,
    FlowStat as ParityFlowStat, FlowStatMap as ParityFlowStatMap, Job, LinkGraphSettings,
    SimpleShare, calculate_demands, flows_as_simple_shares, run_full_pipeline,
    to_station_flows_helper,
};
