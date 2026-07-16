//! Port literal del pipeline linkgraph de `OpenTTD` (enteros / órdenes / fórmulas).
//! El stub BFS de [`crate::cargodist::legacy::mcf`] queda legado; el juego usa este módulo.

pub mod demands;
pub mod flow_stat;
pub mod flowmapper;
pub mod from_game;
pub mod math;
pub mod mcf;
pub mod path;
pub mod rng;
pub mod types;

use crate::cargo::CargoType;
use crate::cargodist::legacy::flow_stat::StationFlows;

pub use demands::calculate_demands;
pub use flow_stat::{FlowStat, FlowStatMap};
pub use flowmapper::FlowMapper;
pub use from_game::{build_jobs_from_game, settings_from_game};
pub use math::{distance_max_plus_manhattan, int_sqrt};
pub use mcf::{
    CapacityAnnotation, DistanceAnnotation, FlowEdgeIterator, GraphEdgeIterator, MCF1stPass,
    MCF2ndPass,
};
pub use path::{Path, PathId};
pub use rng::Randomizer;
pub use types::{
    BaseEdge, BaseNode, DAY_TICKS, DemandAnnotation, DistributionType, INVALID_NODE,
    INVALID_STATION, Job, LinkGraphSettings, NodeId, SimpleShare,
};

pub fn run_full_pipeline(job: &mut Job) {
    calculate_demands(job);
    MCF1stPass::run(job);
    FlowMapper::new(false).run(job);
    MCF2ndPass::run(job);
    FlowMapper::new(true).run(job);
}

#[must_use]
pub fn flows_as_simple_shares(job: &Job) -> Vec<SimpleShare> {
    let mut shares = Vec::new();
    for (node_index, flow_map) in job.flows.iter().enumerate() {
        let at_station = job.nodes[node_index].station;
        for (origin_station, flow_stat) in flow_map.iter() {
            let mut previous = 0_u32;
            for (cumulative, via_station) in flow_stat.shares() {
                let amount = cumulative.saturating_sub(previous);
                previous = *cumulative;
                if amount == 0 || *via_station == INVALID_STATION {
                    continue;
                }
                shares.push(SimpleShare {
                    at_station,
                    origin_station: *origin_station,
                    via_station: *via_station,
                    amount,
                });
            }
        }
    }
    shares.sort_by(|left, right| {
        left.at_station
            .cmp(&right.at_station)
            .then_with(|| left.origin_station.cmp(&right.origin_station))
            .then_with(|| left.via_station.cmp(&right.via_station))
            .then_with(|| left.amount.cmp(&right.amount))
    });
    shares
}

#[must_use]
pub fn to_station_flows_helper(job: &Job, cargo: CargoType) -> StationFlows {
    let mut station_flows = StationFlows::default();
    for share in flows_as_simple_shares(job) {
        let Some(at_node) = job.node_id_by_station(share.at_station) else {
            continue;
        };
        let Some(origin_node) = job.node_id_by_station(share.origin_station) else {
            continue;
        };
        let Some(via_node) = job.node_id_by_station(share.via_station) else {
            continue;
        };
        let Some(at_tile) = job.node_tile(at_node) else {
            continue;
        };
        let Some(origin_tile) = job.node_tile(origin_node) else {
            continue;
        };
        let Some(via_tile) = job.node_tile(via_node) else {
            continue;
        };
        station_flows
            .by_station
            .entry(at_tile)
            .or_default()
            .add_flow(cargo, origin_tile, via_tile, share.amount);
    }
    station_flows
}
