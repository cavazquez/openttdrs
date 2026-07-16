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

use std::collections::BTreeMap;

use super::math::distance_max_plus_manhattan;
use super::path::{Path, PathId};
use super::types::{DAY_TICKS, INVALID_NODE, INVALID_STATION, Job, NodeId};

pub struct DistanceAnnotation;
pub struct CapacityAnnotation;

pub struct GraphEdgeIterator<'a> {
    job: &'a Job,
    edges: Vec<NodeId>,
    index: usize,
}

impl<'a> GraphEdgeIterator<'a> {
    #[must_use]
    pub fn new(job: &'a Job) -> Self {
        Self {
            job,
            edges: Vec::new(),
            index: 0,
        }
    }

    pub fn set_node(&mut self, _source: NodeId, node: NodeId) {
        self.edges = self.job.edges[usize::from(node)]
            .iter()
            .map(|edge| edge.dest)
            .collect();
        self.index = 0;
    }

    #[must_use]
    pub fn next(&mut self) -> NodeId {
        let Some(next) = self.edges.get(self.index).copied() else {
            return INVALID_NODE;
        };
        self.index += 1;
        next
    }
}

pub struct FlowEdgeIterator<'a> {
    job: &'a Job,
    edges: Vec<NodeId>,
    index: usize,
}

impl<'a> FlowEdgeIterator<'a> {
    #[must_use]
    pub fn new(job: &'a Job) -> Self {
        Self {
            job,
            edges: Vec::new(),
            index: 0,
        }
    }

    pub fn set_node(&mut self, source: NodeId, node: NodeId) {
        self.edges.clear();
        self.index = 0;
        let source_station = self.job.station(source);
        if let Some(flow_stat) = self.job.flows[usize::from(node)].get(&source_station) {
            for station in flow_stat.shares().values() {
                if *station == INVALID_STATION {
                    continue;
                }
                if let Some(node_id) = self.job.node_id_by_station(*station) {
                    self.edges.push(node_id);
                }
            }
        }
    }

    #[must_use]
    pub fn next(&mut self) -> NodeId {
        let Some(next) = self.edges.get(self.index).copied() else {
            return INVALID_NODE;
        };
        self.index += 1;
        next
    }
}

trait Annotation {
    fn best_node(paths: &[Option<PathId>], arena: &[Path], unsettled: &[bool]) -> Option<NodeId>;
    fn is_better(current: &Path, base: &Path, cap: u32, free_cap: i32, dist: u32) -> bool;
}

impl Annotation for DistanceAnnotation {
    fn best_node(paths: &[Option<PathId>], arena: &[Path], unsettled: &[bool]) -> Option<NodeId> {
        paths
            .iter()
            .enumerate()
            .filter(|(index, _)| unsettled[*index])
            .filter_map(|(index, maybe_path)| maybe_path.map(|path_id| (index, &arena[path_id])))
            .min_by(|(left_index, left), (right_index, right)| {
                left.distance
                    .cmp(&right.distance)
                    .then_with(|| left_index.cmp(right_index))
            })
            .and_then(|(index, _)| NodeId::try_from(index).ok())
    }

    fn is_better(current: &Path, base: &Path, _cap: u32, free_cap: i32, dist: u32) -> bool {
        if base.distance == u32::MAX {
            return false;
        }
        if current.distance == u32::MAX {
            return true;
        }
        if free_cap > 0 && base.free_capacity > 0 {
            if current.free_capacity > 0 {
                base.distance.saturating_add(dist) < current.distance
            } else {
                true
            }
        } else if current.free_capacity > 0 {
            false
        } else {
            base.distance.saturating_add(dist) < current.distance
        }
    }
}

impl Annotation for CapacityAnnotation {
    fn best_node(paths: &[Option<PathId>], arena: &[Path], unsettled: &[bool]) -> Option<NodeId> {
        paths
            .iter()
            .enumerate()
            .filter(|(index, _)| unsettled[*index])
            .filter_map(|(index, maybe_path)| {
                maybe_path.map(|path_id| (index, arena[path_id].capacity_ratio()))
            })
            .max_by(|(left_index, left), (right_index, right)| {
                left.cmp(right).then_with(|| left_index.cmp(right_index))
            })
            .and_then(|(index, _)| NodeId::try_from(index).ok())
    }

    fn is_better(current: &Path, base: &Path, cap: u32, free_cap: i32, dist: u32) -> bool {
        let min_cap =
            Path::capacity_ratio_value(base.free_capacity.min(free_cap), base.capacity.min(cap));
        let current_cap = current.capacity_ratio();
        if min_cap == current_cap {
            if base.distance == u32::MAX {
                false
            } else {
                base.distance.saturating_add(dist) < current.distance
            }
        } else {
            min_cap > current_cap
        }
    }
}

struct MultiCommodityFlow<'a> {
    job: &'a mut Job,
    max_saturation: u32,
}

impl<'a> MultiCommodityFlow<'a> {
    fn new(job: &'a mut Job) -> Self {
        let max_saturation = job.settings.short_path_saturation;
        Self {
            job,
            max_saturation,
        }
    }

    fn dijkstra_graph<T: Annotation>(&mut self, source_node: NodeId) -> Vec<Option<PathId>> {
        self.dijkstra::<T>(source_node, false)
    }

    fn dijkstra_flow<T: Annotation>(&mut self, source_node: NodeId) -> Vec<Option<PathId>> {
        self.dijkstra::<T>(source_node, true)
    }

    fn dijkstra<T: Annotation>(
        &mut self,
        source_node: NodeId,
        use_flow_edges: bool,
    ) -> Vec<Option<PathId>> {
        let size = self.job.size();
        let mut paths = vec![None; size];
        let mut unsettled = vec![true; size];
        for node_index in 0..size {
            let node_id = NodeId::try_from(node_index).unwrap_or(INVALID_NODE);
            let path_id = self.job.path_arena.len();
            self.job
                .path_arena
                .push(Path::new(node_id, node_id == source_node));
            paths[node_index] = Some(path_id);
        }

        while let Some(from) = T::best_node(&paths, &self.job.path_arena, &unsettled) {
            unsettled[usize::from(from)] = false;
            let Some(source_path_id) = paths[usize::from(from)] else {
                continue;
            };

            let destinations = if use_flow_edges {
                let mut iter = FlowEdgeIterator::new(&*self.job);
                iter.set_node(source_node, from);
                let mut out = Vec::new();
                loop {
                    let to = iter.next();
                    if to == INVALID_NODE {
                        break;
                    }
                    out.push(to);
                }
                out
            } else {
                let mut iter = GraphEdgeIterator::new(&*self.job);
                iter.set_node(source_node, from);
                let mut out = Vec::new();
                loop {
                    let to = iter.next();
                    if to == INVALID_NODE {
                        break;
                    }
                    out.push(to);
                }
                out
            };

            for to in destinations {
                if to == from {
                    continue;
                }
                let Some(edge) = self.job.edge(from, to).copied() else {
                    continue;
                };
                let mut capacity = edge.capacity;
                if self.max_saturation != u32::MAX {
                    capacity = capacity.saturating_mul(self.max_saturation) / 100;
                    if capacity == 0 {
                        capacity = 1;
                    }
                }
                let edge_flow = self.job.edge_flow_value(from, to).unwrap_or(0);
                let distance = distance_max_plus_manhattan(
                    self.job.nodes[usize::from(from)].x,
                    self.job.nodes[usize::from(from)].y,
                    self.job.nodes[usize::from(to)].x,
                    self.job.nodes[usize::from(to)].y,
                )
                .saturating_add(1);
                // OpenTTD: express cargo usa tiempo; freight usa distancia.
                // Sin clases de cargo aún → freight (como CT sin Passengers/Mail/Express).
                let _time = if edge.travel_time != 0 {
                    edge.travel_time.saturating_add(DAY_TICKS)
                } else {
                    distance.saturating_mul(DAY_TICKS)
                };
                let distance_annotation = distance;
                let Some(dest_path_id) = paths[usize::from(to)] else {
                    continue;
                };
                let source_snapshot = self.job.path_arena[source_path_id].clone();
                let dest_snapshot = self.job.path_arena[dest_path_id].clone();
                let free_capacity = i32::try_from(capacity).unwrap_or(i32::MAX)
                    - i32::try_from(edge_flow).unwrap_or(i32::MAX);
                if T::is_better(
                    &dest_snapshot,
                    &source_snapshot,
                    capacity,
                    free_capacity,
                    distance_annotation,
                ) {
                    Path::fork(
                        dest_path_id,
                        source_path_id,
                        capacity,
                        free_capacity,
                        distance_annotation,
                        &mut self.job.path_arena,
                    );
                }
            }
        }
        paths
    }

    fn cleanup_paths(&mut self, source_id: NodeId, mut paths: Vec<Option<PathId>>) {
        let Some(source_path_id) = paths[usize::from(source_id)] else {
            return;
        };
        paths[usize::from(source_id)] = None;
        for maybe_path in paths.clone() {
            let Some(path_id) = maybe_path else {
                continue;
            };
            if self.job.path_arena[path_id].parent == Some(source_path_id) {
                Path::detach(path_id, &mut self.job.path_arena);
            }

            let mut current = Some(path_id);
            while let Some(active_path) = current {
                if active_path == source_path_id
                    || !self.job.path_arena[active_path].alive
                    || self.job.path_arena[active_path].flow > 0
                {
                    break;
                }
                let parent = self.job.path_arena[active_path].parent;
                Path::detach(active_path, &mut self.job.path_arena);
                if self.job.path_arena[active_path].num_children == 0 {
                    let node = self.job.path_arena[active_path].node;
                    self.job.path_arena[active_path].alive = false;
                    if let Some(slot) = paths.get_mut(usize::from(node)) {
                        *slot = None;
                    }
                }
                current = parent;
            }
        }
        self.job.path_arena[source_path_id].alive = false;
    }

    fn push_flow(
        &mut self,
        source: NodeId,
        to: NodeId,
        path_id: PathId,
        accuracy: u32,
        max_saturation: u32,
    ) -> u32 {
        let unsatisfied = self.job.unsatisfied_demand_to(source, to);
        let mut flow = (self.job.demand_to(source, to) / accuracy.max(1)).max(1);
        flow = flow.min(unsatisfied);
        let flow = Path::add_flow(path_id, flow, self.job, max_saturation);
        self.job.satisfy_demand_to(source, to, flow);
        flow
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitMark {
    Unvisited,
    Resolved,
    Active(PathId),
}

pub struct MCF1stPass;
pub struct MCF2ndPass;

impl MCF1stPass {
    pub fn run(job: &mut Job) {
        let mut solver = MultiCommodityFlow::new(job);
        let size = solver.job.size();
        let accuracy = solver.job.settings.accuracy.max(1);
        let mut finished_sources = vec![false; size];
        loop {
            let mut more_loops = false;
            for source_index in 0..size {
                if finished_sources[source_index] {
                    continue;
                }
                let source = NodeId::try_from(source_index).unwrap_or(INVALID_NODE);
                let paths = solver.dijkstra_graph::<DistanceAnnotation>(source);
                let mut source_demand_left = false;
                for dest_index in 0..size {
                    let dest = NodeId::try_from(dest_index).unwrap_or(INVALID_NODE);
                    if solver.job.unsatisfied_demand_to(source, dest) == 0 {
                        continue;
                    }
                    let Some(path_id) = paths[dest_index] else {
                        continue;
                    };
                    let free_capacity = solver.job.path_arena[path_id].free_capacity;
                    if free_capacity > 0
                        && solver.push_flow(source, dest, path_id, accuracy, solver.max_saturation)
                            > 0
                    {
                        more_loops |= solver.job.unsatisfied_demand_to(source, dest) > 0;
                    } else if solver.job.unsatisfied_demand_to(source, dest)
                        == solver.job.demand_to(source, dest)
                        && free_capacity > i32::MIN
                    {
                        let _ = solver.push_flow(source, dest, path_id, accuracy, u32::MAX);
                    }
                    if solver.job.unsatisfied_demand_to(source, dest) > 0 {
                        source_demand_left = true;
                    }
                }
                finished_sources[source_index] = !source_demand_left;
                solver.cleanup_paths(source, paths);
            }
            if !more_loops && !Self::eliminate_cycles(solver.job) {
                break;
            }
        }
    }

    fn find_cycle_flow(job: &Job, path: &[VisitMark], cycle_begin: PathId) -> u32 {
        let mut flow = u32::MAX;
        let cycle_end = cycle_begin;
        let mut current = cycle_begin;
        loop {
            flow = flow.min(job.path_arena[current].flow);
            let next_node = job.path_arena[current].node;
            let VisitMark::Active(next_path) = path[usize::from(next_node)] else {
                break;
            };
            current = next_path;
            if current == cycle_end {
                break;
            }
        }
        flow
    }

    fn eliminate_cycle(job: &mut Job, path: &[VisitMark], cycle_begin: PathId, flow: u32) {
        let cycle_end = cycle_begin;
        let mut current = cycle_begin;
        loop {
            let prev = job.path_arena[current].node;
            job.path_arena[current].flow = job.path_arena[current].flow.saturating_sub(flow);
            if job.path_arena[current].flow == 0
                && let Some(parent_id) = job.path_arena[current].parent
            {
                let parent_node = job.path_arena[parent_id].node;
                job.move_path_to_back(parent_node, current);
            }
            let VisitMark::Active(next_path) = path[usize::from(prev)] else {
                break;
            };
            let next_node = job.path_arena[next_path].node;
            job.remove_edge_flow(prev, next_node, flow);
            current = next_path;
            if current == cycle_end {
                break;
            }
        }
    }

    fn eliminate_cycles_from(
        job: &mut Job,
        path: &mut [VisitMark],
        origin_id: NodeId,
        next_id: NodeId,
    ) -> bool {
        match path[usize::from(next_id)] {
            VisitMark::Resolved => false,
            VisitMark::Unvisited => {
                let path_ids = job.paths[usize::from(next_id)].clone();
                let mut next_hops: BTreeMap<NodeId, PathId> = BTreeMap::new();
                for path_id in path_ids {
                    if job.path_arena[path_id].flow == 0 {
                        break;
                    }
                    if job.path_arena[path_id].origin == origin_id {
                        let next_hop = job.path_arena[path_id].node;
                        if let Some(existing_path) = next_hops.get(&next_hop).copied() {
                            let new_flow = job.path_arena[path_id].flow;
                            job.path_arena[existing_path].flow =
                                job.path_arena[existing_path].flow.saturating_add(new_flow);
                            job.path_arena[path_id].flow =
                                job.path_arena[path_id].flow.saturating_sub(new_flow);
                            job.move_path_to_back(next_id, path_id);
                        } else {
                            next_hops.insert(next_hop, path_id);
                        }
                    }
                }
                let mut found = false;
                for child_path in next_hops.values().copied() {
                    if job.path_arena[child_path].flow > 0 {
                        path[usize::from(next_id)] = VisitMark::Active(child_path);
                        found = Self::eliminate_cycles_from(
                            job,
                            path,
                            origin_id,
                            job.path_arena[child_path].node,
                        ) || found;
                    }
                }
                path[usize::from(next_id)] = if found {
                    VisitMark::Unvisited
                } else {
                    VisitMark::Resolved
                };
                found
            }
            VisitMark::Active(cycle_begin) => {
                let flow = Self::find_cycle_flow(job, path, cycle_begin);
                if flow > 0 {
                    Self::eliminate_cycle(job, path, cycle_begin, flow);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn eliminate_cycles(job: &mut Job) -> bool {
        let size = job.size();
        let mut cycles_found = false;
        let mut path = vec![VisitMark::Unvisited; size];
        for node_index in 0..size {
            path.fill(VisitMark::Unvisited);
            let node = NodeId::try_from(node_index).unwrap_or(INVALID_NODE);
            cycles_found |= Self::eliminate_cycles_from(job, &mut path, node, node);
        }
        cycles_found
    }
}

impl MCF2ndPass {
    pub fn run(job: &mut Job) {
        let mut solver = MultiCommodityFlow::new(job);
        solver.max_saturation = u32::MAX;
        let size = solver.job.size();
        let accuracy = solver.job.settings.accuracy.max(1);
        let mut finished_sources = vec![false; size];
        let mut demand_left = true;
        while demand_left {
            demand_left = false;
            for source_index in 0..size {
                if finished_sources[source_index] {
                    continue;
                }
                let source = NodeId::try_from(source_index).unwrap_or(INVALID_NODE);
                let paths = solver.dijkstra_flow::<CapacityAnnotation>(source);
                let mut source_demand_left = false;
                for dest_index in 0..size {
                    let dest = NodeId::try_from(dest_index).unwrap_or(INVALID_NODE);
                    let Some(path_id) = paths[dest_index] else {
                        continue;
                    };
                    if solver.job.unsatisfied_demand_to(source, dest) > 0
                        && solver.job.path_arena[path_id].free_capacity > i32::MIN
                    {
                        let _ = solver.push_flow(source, dest, path_id, accuracy, u32::MAX);
                        if solver.job.unsatisfied_demand_to(source, dest) > 0 {
                            demand_left = true;
                            source_demand_left = true;
                        }
                    }
                }
                finished_sources[source_index] = !source_demand_left;
                solver.cleanup_paths(source, paths);
            }
        }
    }
}
