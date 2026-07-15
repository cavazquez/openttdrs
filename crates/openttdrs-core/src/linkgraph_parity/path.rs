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

pub type PathId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub distance: u32,
    pub capacity: u32,
    pub free_capacity: i32,
    pub flow: u32,
    pub node: NodeId,
    pub origin: NodeId,
    pub num_children: u32,
    pub parent: Option<PathId>,
    pub alive: bool,
}

impl Path {
    const PATH_CAP_MULTIPLIER: i32 = 16;
    const PATH_CAP_MIN_FREE: i32 = (i32::MIN + 1) / Self::PATH_CAP_MULTIPLIER;
    const PATH_CAP_MAX_FREE: i32 = (i32::MAX - 1) / Self::PATH_CAP_MULTIPLIER;

    #[must_use]
    pub fn new(node: NodeId, source: bool) -> Self {
        Self {
            distance: if source { 0 } else { u32::MAX },
            capacity: if source { u32::MAX } else { 0 },
            free_capacity: if source { i32::MAX } else { i32::MIN },
            flow: 0,
            node,
            origin: if source {
                node
            } else {
                super::types::INVALID_NODE
            },
            num_children: 0,
            parent: None,
            alive: true,
        }
    }

    #[must_use]
    pub fn capacity_ratio_value(free: i32, total: u32) -> i32 {
        let clamped = free.clamp(Self::PATH_CAP_MIN_FREE, Self::PATH_CAP_MAX_FREE);
        clamped * Self::PATH_CAP_MULTIPLIER / i32::try_from(total.max(1)).unwrap_or(1)
    }

    #[must_use]
    pub fn capacity_ratio(&self) -> i32 {
        Self::capacity_ratio_value(self.free_capacity, self.capacity)
    }

    pub fn detach(path_id: PathId, arena: &mut [Path]) {
        if let Some(parent_id) = arena[path_id].parent {
            arena[parent_id].num_children = arena[parent_id].num_children.saturating_sub(1);
            arena[path_id].parent = None;
        }
    }

    pub fn fork(
        path_id: PathId,
        base_id: PathId,
        cap: u32,
        free_cap: i32,
        dist: u32,
        arena: &mut [Path],
    ) {
        let base_capacity = arena[base_id].capacity;
        let base_free_capacity = arena[base_id].free_capacity;
        let base_distance = arena[base_id].distance;
        let base_origin = arena[base_id].origin;
        {
            let path = &mut arena[path_id];
            path.capacity = base_capacity.min(cap);
            path.free_capacity = base_free_capacity.min(free_cap);
            path.distance = base_distance.saturating_add(dist);
        }

        if arena[path_id].parent != Some(base_id) {
            Self::detach(path_id, arena);
            arena[path_id].parent = Some(base_id);
            arena[base_id].num_children = arena[base_id].num_children.saturating_add(1);
        }
        arena[path_id].origin = base_origin;
    }

    pub fn add_flow(path_id: PathId, mut new_flow: u32, job: &mut Job, max_saturation: u32) -> u32 {
        let parent_id = job.path_arena[path_id].parent;
        if let Some(parent_id) = parent_id {
            let parent_node = job.path_arena[parent_id].node;
            let node = job.path_arena[path_id].node;
            if max_saturation != u32::MAX {
                let Some(edge) = job.edge(parent_node, node).copied() else {
                    return 0;
                };
                let usable_cap = edge.capacity.saturating_mul(max_saturation) / 100;
                let Some(current_flow) = job.edge_flow_value(parent_node, node) else {
                    return 0;
                };
                if usable_cap > current_flow {
                    new_flow = new_flow.min(usable_cap - current_flow);
                } else {
                    return 0;
                }
            }
            new_flow = Self::add_flow(parent_id, new_flow, job, max_saturation);
            let was_zero = job.path_arena[path_id].flow == 0 && new_flow > 0;
            if was_zero {
                job.paths[usize::from(parent_node)].insert(0, path_id);
            }
            job.add_edge_flow(parent_node, node, new_flow);
        }
        job.path_arena[path_id].flow = job.path_arena[path_id].flow.saturating_add(new_flow);
        new_flow
    }
}
