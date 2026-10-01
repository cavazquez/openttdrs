//! Persistent `CargoDist` publication frontier. Worker scratch and caches stay transient.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{PendingLinkGraphJob, SimulationRuntime};
use crate::cargo::CargoType;
use crate::flow_stat::StationFlows;
use crate::linkgraph_parity::{BaseEdge, BaseNode, Job, LinkGraphSettings};
use crate::map::TileCoord;

#[derive(Serialize, Deserialize)]
struct PublishedFlow {
    station: TileCoord,
    cargo: CargoType,
    origin: TileCoord,
    shares: Vec<(TileCoord, u32)>,
}

#[derive(Serialize, Deserialize)]
struct RoutingSnapshot {
    flows: Vec<PublishedFlow>,
    pending: Vec<PendingLinkGraphJob>,
}

impl Serialize for SimulationRuntime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct SnapshotRef<'a> {
            flows: &'a [PublishedFlow],
            pending: &'a [PendingLinkGraphJob],
        }
        // HashMaps have randomized iteration; shares retain their native order
        // because that order controls weighted RandomRange selection.
        let mut flows = Vec::new();
        for (&station, table) in &self.station_flows.by_station {
            for (&cargo, map) in &table.by_cargo {
                for (&origin, flow) in &map.by_origin {
                    flows.push(PublishedFlow {
                        station,
                        cargo,
                        origin,
                        shares: flow.shares.clone(),
                    });
                }
            }
        }
        flows.sort_unstable_by_key(|flow| (flow.station, flow.cargo, flow.origin));
        SnapshotRef {
            flows: &flows,
            pending: &self.pending_linkgraph_jobs,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SimulationRuntime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let snapshot = RoutingSnapshot::deserialize(deserializer)?;
        let mut station_flows = StationFlows::default();
        for flow in snapshot.flows {
            let map = station_flows
                .by_station
                .entry(flow.station)
                .or_default()
                .by_cargo
                .entry(flow.cargo)
                .or_default();
            map.by_origin.entry(flow.origin).or_default().shares = flow.shares;
        }
        Ok(Self {
            station_flows,
            pending_linkgraph_jobs: snapshot.pending,
            cargo_routing_initialized: true,
            ..Self::new()
        })
    }
}

#[derive(Serialize, Deserialize)]
struct JobInput {
    settings: LinkGraphSettings,
    nodes: Vec<BaseNode>,
    edges: Vec<Vec<BaseEdge>>,
    runtime: u32,
}

impl Serialize for Job {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct InputRef<'a> {
            settings: &'a LinkGraphSettings,
            nodes: &'a [BaseNode],
            edges: &'a [Vec<BaseEdge>],
            runtime: u32,
        }
        InputRef {
            settings: &self.settings,
            nodes: &self.nodes,
            edges: &self.edges,
            runtime: self.runtime,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Job {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = JobInput::deserialize(deserializer)?;
        if input.nodes.len() > usize::from(u16::MAX)
            || input.edges.len() != input.nodes.len()
            || input
                .edges
                .iter()
                .flatten()
                .any(|edge| usize::from(edge.dest) >= input.nodes.len())
        {
            return Err(serde::de::Error::custom(
                "invalid CargoDist job node or edge topology",
            ));
        }
        let mut job = Self::new(input.nodes, input.edges, input.settings);
        job.runtime = input.runtime;
        Ok(job)
    }
}
