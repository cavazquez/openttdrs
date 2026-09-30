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

use std::collections::{BTreeMap, BinaryHeap};

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
    /// Max-heap key, including the exact tie break of the scan oracle.
    fn priority(path: &Path, node: NodeId) -> (i64, i64);
    #[cfg(test)]
    fn best_node(paths: &[Option<PathId>], arena: &[Path], unsettled: &[bool]) -> Option<NodeId>;
    fn is_better(current: &Path, base: &Path, cap: u32, free_cap: i32, dist: u32) -> bool;
}

impl Annotation for DistanceAnnotation {
    fn priority(path: &Path, node: NodeId) -> (i64, i64) {
        (-i64::from(path.distance), -i64::from(node))
    }

    #[cfg(test)]
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
    fn priority(path: &Path, node: NodeId) -> (i64, i64) {
        (i64::from(path.capacity_ratio()), i64::from(node))
    }

    #[cfg(test)]
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
    node_by_station: ahash::AHashMap<u32, NodeId>,
}

impl<'a> MultiCommodityFlow<'a> {
    fn new(job: &'a mut Job) -> Self {
        let max_saturation = job.settings.short_path_saturation;
        let mut node_by_station = ahash::AHashMap::with_capacity(job.size());
        for (index, node) in job.nodes.iter().enumerate() {
            if let Ok(id) = NodeId::try_from(index) {
                // The public scan returns the first node for a station.
                node_by_station.entry(node.station).or_insert(id);
            }
        }
        Self {
            job,
            max_saturation,
            node_by_station,
        }
    }

    fn destinations(&self, source: NodeId, from: NodeId, use_flow_edges: bool) -> Vec<NodeId> {
        if use_flow_edges {
            self.job.flows[usize::from(from)]
                .get(&self.job.station(source))
                .into_iter()
                .flat_map(|stat| stat.shares().values())
                .filter(|&&station| station != INVALID_STATION)
                .filter_map(|station| self.node_by_station.get(station).copied())
                .take_while(|&node| node != INVALID_NODE)
                .collect()
        } else {
            self.job.edges[usize::from(from)]
                .iter()
                .map(|edge| edge.dest)
                .take_while(|&node| node != INVALID_NODE)
                .collect()
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
        self.dijkstra_with_queue::<T>(
            source_node,
            use_flow_edges,
            #[cfg(test)]
            false,
        )
    }

    fn dijkstra_with_queue<T: Annotation>(
        &mut self,
        source_node: NodeId,
        use_flow_edges: bool,
        #[cfg(test)] reference_scan: bool,
    ) -> Vec<Option<PathId>> {
        let size = self.job.size();
        let mut paths = vec![None; size];
        let mut unsettled = vec![true; size];
        let mut queue = BinaryHeap::with_capacity(size);
        for node_index in 0..size {
            let node_id = NodeId::try_from(node_index).unwrap_or(INVALID_NODE);
            let path_id = self.job.path_arena.len();
            self.job
                .path_arena
                .push(Path::new(node_id, node_id == source_node));
            paths[node_index] = Some(path_id);
            queue.push((T::priority(&self.job.path_arena[path_id], node_id), node_id));
        }

        loop {
            // The former full-vector scan cost O(N²) per source. Lazy entries
            // preserve its selection order, including unreachable nodes and
            // signed capacity ratios, while replacing each scan with a pop.
            let from = loop {
                #[cfg(test)]
                if reference_scan {
                    break T::best_node(&paths, &self.job.path_arena, &unsettled);
                }
                let Some((priority, node)) = queue.pop() else {
                    break None;
                };
                let index = usize::from(node);
                let Some(path_id) = paths[index] else {
                    continue;
                };
                if unsettled[index] && priority == T::priority(&self.job.path_arena[path_id], node)
                {
                    break Some(node);
                }
            };
            let Some(from) = from else {
                break;
            };
            unsettled[usize::from(from)] = false;
            let Some(source_path_id) = paths[usize::from(from)] else {
                continue;
            };

            let destinations = self.destinations(source_node, from, use_flow_edges);

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
                    // The original algorithm does not revisit settled nodes.
                    if unsettled[usize::from(to)] {
                        queue.push((T::priority(&self.job.path_arena[dest_path_id], to), to));
                    }
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
        Self::run_with_demand_filter(job, true);
    }

    fn run_with_demand_filter(job: &mut Job, filter: bool) {
        let mut solver = MultiCommodityFlow::new(job);
        let size = solver.job.size();
        let accuracy = solver.job.settings.accuracy.max(1);
        let reachable = filter.then(|| graph_reachability(solver.job));
        let mut finished_sources = initially_finished_sources(solver.job, reachable.as_deref());
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
                    if solver.job.unsatisfied_demand_to(source, dest) > 0
                        && reachable
                            .as_ref()
                            .is_none_or(|nodes| nodes[source_index][dest_index])
                    {
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
        Self::run_with_demand_filter(job, true);
    }

    fn run_with_demand_filter(job: &mut Job, filter: bool) {
        let mut solver = MultiCommodityFlow::new(job);
        solver.max_saturation = u32::MAX;
        let size = solver.job.size();
        let accuracy = solver.job.settings.accuracy.max(1);
        let reachable = filter.then(|| graph_reachability(solver.job));
        let mut finished_sources = initially_finished_sources(solver.job, reachable.as_deref());
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

/// Demand to a disconnected station can never acquire flow. Such demands must
/// remain unsatisfied, but must not keep an otherwise completed source in
/// every global accuracy iteration. Topology is fixed throughout both passes.
fn graph_reachability(job: &Job) -> Vec<Vec<bool>> {
    (0..job.size())
        .map(|source| {
            let mut seen = vec![false; job.size()];
            let mut pending = vec![source];
            seen[source] = true;
            while let Some(from) = pending.pop() {
                for edge in &job.edges[from] {
                    let to = usize::from(edge.dest);
                    if to < seen.len() && !seen[to] {
                        seen[to] = true;
                        pending.push(to);
                    }
                }
            }
            seen
        })
        .collect()
}

fn initially_finished_sources(job: &Job, reachable: Option<&[Vec<bool>]>) -> Vec<bool> {
    (0..job.size())
        .map(|source| {
            reachable.is_some_and(|nodes| {
                !job.demands[source]
                    .iter()
                    .enumerate()
                    .any(|(dest, demand)| demand.unsatisfied_demand > 0 && nodes[source][dest])
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)] // All fixture sizes are at most 245 nodes.
mod tests {
    use super::*;
    use crate::cargodist::parity::types::{BaseEdge, BaseNode, LinkGraphSettings};

    fn graph(size: usize, mut seed: u32) -> Job {
        let nodes = (0..size)
            .map(|index| BaseNode {
                station: index as u32 + 42,
                // Equal distances exercise both annotations' opposite tie breaks.
                x: index as u32 % 5,
                y: index as u32 % 7,
                supply: 100,
                demand: 8,
            })
            .collect();
        let mut edges = vec![Vec::new(); size];
        for (from, outgoing) in edges.iter_mut().enumerate() {
            for to in 0..size {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                // A disconnected last node, self edges, cycles and multiple
                // improvements of a queued node must match the previous scan.
                if from + 1 < size && to + 1 < size && seed.is_multiple_of(5) {
                    outgoing.push(BaseEdge {
                        dest: to as NodeId,
                        capacity: seed % 100,
                        usage: 0,
                        travel_time: seed % 74,
                    });
                }
            }
        }
        let mut job = Job::new(nodes, edges, LinkGraphSettings::default());
        for from in 0..size {
            for (index, edge) in job.edges[from].iter().enumerate() {
                // Include saturated/overloaded edges and negative free capacity.
                job.edge_flow[from][index] = (seed.wrapping_add(index as u32)) % 150;
                for source in 0..size {
                    let station = job.nodes[source].station;
                    job.flows[from].add_flow(station, job.nodes[usize::from(edge.dest)].station, 1);
                }
            }
        }
        job
    }

    fn assert_search_matches_scan<T: Annotation>(job: &Job, source: NodeId, flow_edges: bool) {
        let mut queued = job.clone();
        let mut scanned = job.clone();
        let queue_paths = MultiCommodityFlow::new(&mut queued)
            .dijkstra_with_queue::<T>(source, flow_edges, false);
        let scan_paths = MultiCommodityFlow::new(&mut scanned)
            .dijkstra_with_queue::<T>(source, flow_edges, true);
        assert_eq!(queue_paths, scan_paths);
        assert_eq!(queued.path_arena, scanned.path_arena);
        assert_eq!(queued.edge_flow, scanned.edge_flow);
    }

    #[test]
    fn priority_queue_preserves_distance_capacity_paths_and_tie_breaks() {
        for size in [2, 7, 32, 128, 245] {
            for seed in [0, 1, 42, 2026] {
                let mut job = graph(size, seed);
                for saturation in [80, u32::MAX] {
                    job.settings.short_path_saturation = saturation;
                    for source in [0, size / 2, size - 1] {
                        for flow_edges in [false, true] {
                            assert_search_matches_scan::<DistanceAnnotation>(
                                &job,
                                source as NodeId,
                                flow_edges,
                            );
                            assert_search_matches_scan::<CapacityAnnotation>(
                                &job,
                                source as NodeId,
                                flow_edges,
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn indexed_adjacency_keeps_iterator_order_duplicate_stations_and_sentinel() {
        let mut job = graph(7, 42);
        job.nodes[2].station = job.nodes[1].station;
        job.edges[0] = [2, INVALID_NODE, 3]
            .into_iter()
            .map(|dest| BaseEdge {
                dest,
                capacity: 10,
                usage: 0,
                travel_time: 1,
            })
            .collect();
        job.flows[0].add_flow(job.nodes[0].station, INVALID_STATION, 2);
        job.flows[0].add_flow(job.nodes[0].station, 9000, 3);
        let solver = MultiCommodityFlow::new(&mut job);
        for source in 0..7 {
            for from in 0..7 {
                let mut graph = GraphEdgeIterator::new(solver.job);
                graph.set_node(source, from);
                let mut original = Vec::new();
                loop {
                    let dest = graph.next();
                    if dest == INVALID_NODE {
                        break;
                    }
                    original.push(dest);
                }
                assert_eq!(solver.destinations(source, from, false), original);
                let mut flow = FlowEdgeIterator::new(solver.job);
                flow.set_node(source, from);
                original.clear();
                loop {
                    let dest = flow.next();
                    if dest == INVALID_NODE {
                        break;
                    }
                    original.push(dest);
                }
                assert_eq!(solver.destinations(source, from, true), original);
            }
        }
    }

    #[test]
    fn skipping_finished_or_isolated_sources_preserves_complete_pipeline_flows() {
        use crate::cargodist::parity::types::DistributionType;
        use crate::cargodist::parity::{FlowMapper, calculate_demands, flows_as_simple_shares};
        for size in [2, 7, 32, 128] {
            for seed in [0, 1, 42] {
                for distribution in [DistributionType::Asymmetric, DistributionType::Symmetric] {
                    let mut generated = graph(size, seed);
                    if seed == 42 {
                        for (from, outgoing) in generated.edges.iter_mut().enumerate() {
                            outgoing.retain(|edge| usize::from(edge.dest) / 4 == from / 4);
                        }
                    }
                    let mut filtered = Job::new(
                        generated.nodes,
                        generated.edges,
                        LinkGraphSettings {
                            distribution,
                            ..Default::default()
                        },
                    );
                    for (index, node) in filtered.nodes.iter_mut().enumerate() {
                        node.supply = if index.is_multiple_of(3) { 20 } else { 0 };
                        node.demand = if index.is_multiple_of(4) { 8 } else { 0 };
                    }
                    filtered.nodes[size - 1].supply = 20;
                    filtered.nodes[size - 1].demand = 8;
                    calculate_demands(&mut filtered);
                    let mut original = filtered.clone();
                    MCF1stPass::run_with_demand_filter(&mut filtered, true);
                    MCF1stPass::run_with_demand_filter(&mut original, false);
                    FlowMapper::new(false).run(&mut filtered);
                    FlowMapper::new(false).run(&mut original);
                    assert_eq!(filtered.demands, original.demands);
                    assert_eq!(filtered.edge_flow, original.edge_flow);
                    assert_eq!(
                        flows_as_simple_shares(&filtered),
                        flows_as_simple_shares(&original)
                    );
                    MCF2ndPass::run_with_demand_filter(&mut filtered, true);
                    MCF2ndPass::run_with_demand_filter(&mut original, false);
                    FlowMapper::new(true).run(&mut filtered);
                    FlowMapper::new(true).run(&mut original);
                    assert_eq!(filtered.demands, original.demands);
                    assert_eq!(filtered.edge_flow, original.edge_flow);
                    assert_eq!(
                        flows_as_simple_shares(&filtered),
                        flows_as_simple_shares(&original)
                    );
                }
            }
        }
    }
}
