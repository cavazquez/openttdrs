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

use super::types::{Job, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowMapper {
    scale: bool,
}

impl FlowMapper {
    #[must_use]
    pub const fn new(scale: bool) -> Self {
        Self { scale }
    }

    pub fn run(self, job: &mut Job) {
        for node_index in 0..job.size() {
            let node_id = NodeId::try_from(node_index).unwrap_or_default();
            let prev_station = job.station(node_id);
            let path_ids = job.paths[node_index].clone();
            for path_id in path_ids {
                let flow = job.path_arena[path_id].flow;
                if flow == 0 {
                    break;
                }
                let via_node = job.path_arena[path_id].node;
                let via_station = job.station(via_node);
                let origin_station = job.station(job.path_arena[path_id].origin);
                job.flows[usize::from(via_node)].add_flow(origin_station, via_station, flow);
                if prev_station != origin_station {
                    job.flows[node_index].pass_on_flow(origin_station, via_station, flow);
                } else {
                    job.flows[node_index].add_flow(origin_station, via_station, flow);
                }
            }
        }

        for node_index in 0..job.size() {
            let self_station = job.nodes[node_index].station;
            job.flows[node_index].finalize_local_consumption(self_station);
            if self.scale {
                let runtime = job.runtime.max(1);
                for (_, flow_stat) in job.flows[node_index].iter() {
                    let mut cloned = flow_stat.clone();
                    cloned.scale_to_monthly(runtime);
                }
                let origins: Vec<u32> = job.flows[node_index]
                    .iter()
                    .map(|(origin, _)| *origin)
                    .collect();
                for origin in origins {
                    if let Some(flow_stat) = job.flows[node_index].get_mut(&origin) {
                        flow_stat.scale_to_monthly(runtime);
                    }
                }
            }
        }

        job.clear_paths();
    }
}
