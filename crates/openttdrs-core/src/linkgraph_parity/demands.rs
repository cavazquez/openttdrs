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

use std::collections::VecDeque;

use super::math::{distance_max_plus_manhattan, int_sqrt};
use super::types::{DistributionType, Job, NodeId};

trait Scaler {
    fn add_node(&mut self, _job: &Job, _node: NodeId) {}
    fn set_demand_per_node(&mut self, _num_demands: u32) {}
    fn effective_supply(&self, job: &Job, from: NodeId, to: NodeId) -> u32;
    fn has_demand_left(&self, job: &Job, to: NodeId) -> bool;
    fn set_demands(&self, job: &mut Job, from: NodeId, to: NodeId, demand_forw: u32) {
        job.deliver_supply(from, to, demand_forw);
    }
}

#[derive(Debug, Clone, Copy)]
struct SymmetricScaler {
    mod_size: u32,
    supply_sum: u32,
    demand_per_node: u32,
}

impl SymmetricScaler {
    const fn new(mod_size: u32) -> Self {
        Self {
            mod_size,
            supply_sum: 0,
            demand_per_node: 0,
        }
    }
}

impl Scaler for SymmetricScaler {
    fn add_node(&mut self, job: &Job, node: NodeId) {
        self.supply_sum = self
            .supply_sum
            .saturating_add(job.nodes[usize::from(node)].supply);
    }

    fn set_demand_per_node(&mut self, num_demands: u32) {
        self.demand_per_node = (self.supply_sum / num_demands).max(1);
    }

    fn effective_supply(&self, job: &Job, from: NodeId, to: NodeId) -> u32 {
        let from_node = job.nodes[usize::from(from)];
        let to_node = job.nodes[usize::from(to)];
        let remote_supply = to_node.supply.max(1);
        from_node
            .supply
            .saturating_mul(remote_supply)
            .saturating_mul(self.mod_size)
            / 100
            / self.demand_per_node.max(1).max(1)
    }

    fn has_demand_left(&self, job: &Job, to: NodeId) -> bool {
        let to_node = job.nodes[usize::from(to)];
        (to_node.supply == 0 || job.undelivered_supply[usize::from(to)] > 0) && to_node.demand > 0
    }

    fn set_demands(&self, job: &mut Job, from: NodeId, to: NodeId, mut demand_forw: u32) {
        if job.nodes[usize::from(from)].demand > 0 {
            let mut demand_back = demand_forw.saturating_mul(self.mod_size) / 100;
            let undelivered = job.undelivered_supply[usize::from(to)];
            if demand_back > undelivered {
                demand_back = undelivered;
                demand_forw = (demand_back.saturating_mul(100) / self.mod_size.max(1)).max(1);
            }
            job.deliver_supply(to, from, demand_back);
        }
        job.deliver_supply(from, to, demand_forw);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AsymmetricScaler;

impl Scaler for AsymmetricScaler {
    fn effective_supply(&self, job: &Job, from: NodeId, _to: NodeId) -> u32 {
        job.nodes[usize::from(from)].supply
    }

    fn has_demand_left(&self, job: &Job, to: NodeId) -> bool {
        job.nodes[usize::from(to)].demand > 0
    }
}

#[derive(Debug, Clone, Copy)]
struct DemandCalculator {
    base_distance: i32,
    mod_dist: i32,
    accuracy: i32,
}

impl DemandCalculator {
    fn new(job: &Job) -> Self {
        let mut mod_dist = job.settings.demand_distance as i32;
        if mod_dist > 100 {
            let over_100 = mod_dist - 100;
            mod_dist = 100 + ((over_100 * over_100) / 12);
        }
        let base_distance = int_sqrt(distance_max_plus_manhattan(
            0,
            0,
            job.settings.map_max_x,
            job.settings.map_max_y,
        )) as i32;
        Self {
            base_distance,
            mod_dist,
            accuracy: job.settings.accuracy as i32,
        }
    }

    fn calc_demand<T: Scaler>(self, job: &mut Job, mut scaler: T) {
        let mut supplies = VecDeque::new();
        let mut demands = VecDeque::new();
        let mut num_supplies = 0_u32;
        let mut num_demands = 0_u32;

        for node_index in 0..job.size() {
            let node = NodeId::try_from(node_index).unwrap_or_default();
            scaler.add_node(job, node);
            if job.nodes[node_index].supply > 0 {
                supplies.push_back(node);
                num_supplies += 1;
            }
            if job.nodes[node_index].demand > 0 {
                demands.push_back(node);
                num_demands += 1;
            }
        }

        if num_supplies == 0 || num_demands == 0 {
            return;
        }

        scaler.set_demand_per_node(num_demands);
        let mut chance = 0_i32;

        while !supplies.is_empty() && !demands.is_empty() {
            let Some(from_id) = supplies.pop_front() else {
                break;
            };

            for _ in 0..num_demands {
                let Some(to_id) = demands.pop_front() else {
                    break;
                };
                if from_id == to_id {
                    if demands.is_empty() && supplies.is_empty() {
                        return;
                    }
                    demands.push_back(to_id);
                    continue;
                }

                let supply = scaler.effective_supply(job, from_id, to_id) as i32;
                const DIVISOR_SCALE: i32 = 16;

                let mut scaled_distance = self.base_distance;
                if self.mod_dist > 0 {
                    let from = job.nodes[usize::from(from_id)];
                    let to = job.nodes[usize::from(to_id)];
                    let distance = distance_max_plus_manhattan(from.x, from.y, to.x, to.y) as i32;
                    scaled_distance = 0.max(
                        self.base_distance
                            + (((distance - self.base_distance) * self.mod_dist) / 1024),
                    );
                }

                let divisor = DIVISOR_SCALE
                    + ((self.accuracy * scaled_distance * DIVISOR_SCALE)
                        / (self.base_distance.max(1) * 2));

                let mut demand_forw = 0_u32;
                if divisor <= supply.saturating_mul(DIVISOR_SCALE) {
                    demand_forw = ((supply * DIVISOR_SCALE) / divisor.max(1)) as u32;
                } else {
                    chance += 1;
                    if chance > self.accuracy * num_demands as i32 * num_supplies as i32 {
                        demand_forw = 1;
                    }
                }

                demand_forw = demand_forw.min(job.undelivered_supply[usize::from(from_id)]);
                scaler.set_demands(job, from_id, to_id, demand_forw);

                if scaler.has_demand_left(job, to_id) {
                    demands.push_back(to_id);
                } else {
                    num_demands = num_demands.saturating_sub(1);
                }

                if job.undelivered_supply[usize::from(from_id)] == 0 {
                    break;
                }
            }

            if job.undelivered_supply[usize::from(from_id)] != 0 {
                supplies.push_back(from_id);
            } else {
                num_supplies = num_supplies.saturating_sub(1);
            }
        }
    }
}

pub fn calculate_demands(job: &mut Job) {
    let calculator = DemandCalculator::new(job);
    match job.settings.distribution {
        DistributionType::Symmetric => {
            calculator.calc_demand(job, SymmetricScaler::new(job.settings.demand_size));
        }
        DistributionType::Asymmetric => {
            calculator.calc_demand(job, AsymmetricScaler);
        }
        DistributionType::Manual => {}
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_demands;
    use crate::linkgraph_parity::types::{BaseNode, DistributionType, Job, LinkGraphSettings};

    #[test]
    fn linkgraph_parity_asymmetric_two_node_hand_calc() {
        let settings = LinkGraphSettings {
            accuracy: 1,
            demand_size: 100,
            demand_distance: 0,
            short_path_saturation: 80,
            distribution: DistributionType::Asymmetric,
            recalc_time: 30,
            map_max_x: 10,
            map_max_y: 10,
        };
        let nodes = vec![
            BaseNode {
                station: 1,
                x: 0,
                y: 0,
                supply: 10,
                demand: 0,
            },
            BaseNode {
                station: 2,
                x: 10,
                y: 10,
                supply: 0,
                demand: 6,
            },
        ];
        let mut job = Job::new(nodes, vec![Vec::new(), Vec::new()], settings);
        calculate_demands(&mut job);
        assert_eq!(job.demand_to(0, 1), 10);
        assert_eq!(job.unsatisfied_demand_to(0, 1), 10);
        assert_eq!(job.undelivered_supply[0], 0);
    }
}
