//! Exact input cache: changing one cargo must not solve every other cargo again.

use std::collections::HashMap;

use crate::CargoType;
use crate::flow_stat::StationFlows;

use super::{
    BaseEdge, BaseNode, Job, LinkGraphSettings, run_full_pipeline, to_station_flows_helper,
};

#[derive(Debug, Clone)]
struct CachedJob {
    nodes: Vec<BaseNode>,
    edges: Vec<Vec<BaseEdge>>,
    settings: LinkGraphSettings,
    runtime: u32,
    flows: StationFlows,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StationFlowCache {
    jobs: HashMap<CargoType, CachedJob>,
    pub(crate) solves: u64,
}

impl StationFlowCache {
    pub(crate) fn resolve(&mut self, cargo: CargoType, mut job: Job) -> &StationFlows {
        let unchanged = self.jobs.get(&cargo).is_some_and(|cached| {
            cached.nodes == job.nodes
                && cached.edges == job.edges
                && cached.settings == job.settings
                && cached.runtime == job.runtime
        });
        if !unchanged {
            let timing =
                std::env::var_os("OPENTTDRS_PERF_CARGODIST").map(|_| std::time::Instant::now());
            let nodes = job.nodes.clone();
            let edges = job.edges.clone();
            run_full_pipeline(&mut job);
            if let Some(timing) = timing {
                eprintln!(
                    "cargodist {cargo:?}: {} nodes, {} edges, accuracy {}, {:.3} ms",
                    nodes.len(),
                    edges.iter().map(Vec::len).sum::<usize>(),
                    job.settings.accuracy,
                    timing.elapsed().as_secs_f64() * 1000.0
                );
            }
            self.solves = self.solves.saturating_add(1);
            self.jobs.insert(
                cargo,
                CachedJob {
                    nodes,
                    edges,
                    settings: job.settings,
                    runtime: job.runtime,
                    flows: to_station_flows_helper(&job, cargo),
                },
            );
        }
        &self.jobs[&cargo].flows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linkgraph_parity::DistributionType;

    fn job() -> Job {
        Job::new(
            vec![
                BaseNode {
                    station: 42,
                    x: 1,
                    y: 1,
                    supply: 40,
                    demand: 8,
                },
                BaseNode {
                    station: 99,
                    x: 10,
                    y: 10,
                    supply: 20,
                    demand: 8,
                },
            ],
            vec![
                vec![BaseEdge {
                    dest: 1,
                    capacity: 40,
                    usage: 0,
                    travel_time: 74,
                }],
                vec![BaseEdge {
                    dest: 0,
                    capacity: 40,
                    usage: 0,
                    travel_time: 74,
                }],
            ],
            LinkGraphSettings {
                distribution: DistributionType::Asymmetric,
                ..Default::default()
            },
        )
    }

    fn assert_matches_uncached(cache: &mut StationFlowCache, cargo: CargoType, input: &Job) {
        let actual = cache.resolve(cargo, input.clone()).clone();
        let mut uncached = input.clone();
        run_full_pipeline(&mut uncached);
        assert_eq!(actual, to_station_flows_helper(&uncached, cargo));
    }

    #[test]
    fn reuse_is_per_cargo_and_every_solver_input_invalidates() {
        let mut cache = StationFlowCache::default();
        let original = job();
        assert_matches_uncached(&mut cache, CargoType::Passengers, &original);
        assert_matches_uncached(&mut cache, CargoType::Mail, &original);
        assert_eq!(cache.solves, 2);
        for _ in 0..5 {
            assert_matches_uncached(&mut cache, CargoType::Passengers, &original);
            assert_matches_uncached(&mut cache, CargoType::Mail, &original);
        }
        assert_eq!(cache.solves, 2);
        let mut changed = original.clone();
        changed.nodes[0].supply += 1;
        assert_matches_uncached(&mut cache, CargoType::Passengers, &changed);
        assert_matches_uncached(&mut cache, CargoType::Mail, &original);
        assert_eq!(cache.solves, 3);
        changed.nodes[1].x += 1;
        assert_matches_uncached(&mut cache, CargoType::Passengers, &changed);
        changed.edges[0][0].capacity += 1;
        assert_matches_uncached(&mut cache, CargoType::Passengers, &changed);
        changed.edges[0][0].usage += 1;
        assert_matches_uncached(&mut cache, CargoType::Passengers, &changed);
        changed.settings.accuracy += 1;
        assert_matches_uncached(&mut cache, CargoType::Passengers, &changed);
        changed.runtime += 1;
        assert_matches_uncached(&mut cache, CargoType::Passengers, &changed);
        assert_eq!(cache.solves, 8);
    }
}
