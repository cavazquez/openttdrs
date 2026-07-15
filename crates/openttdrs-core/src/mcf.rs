//! Esbozo de MCF para `CargoDist` (#49).
//!
//! No replica los dos pases Dijkstra de `OpenTTD` (`MCF1stPass` / `MCF2ndPass`).
//! `GreedyShortest` empuja flujo por caminos BFS hop-count sobre el grafo
//! observado y materializa shares en cada nodo del path (estilo `FlowMapper`).

use std::collections::{HashMap, VecDeque};

use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::flow_stat::StationFlows;
use crate::link_graph::LinkGraphStats;
use crate::map::TileCoord;

/// Tope de nodos por cargo; por encima se cae a [`McfAlgorithm::Naive`].
pub const MCF_MAX_NODES: usize = 64;
/// Tope de aristas por cargo.
pub const MCF_MAX_EDGES: usize = 256;

/// Configuración mínima del stub MCF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McfConfig {
    /// Cuántos chunks de flujo por commodity (mayor = más coarse).
    pub accuracy: u32,
}

impl Default for McfConfig {
    fn default() -> Self {
        Self { accuracy: 16 }
    }
}

/// Algoritmo de reconstrucción de flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McfAlgorithm {
    /// 1 arista observada = 1 share directo (`StationFlows::from_link_graph`).
    #[default]
    Naive,
    /// Multi-origen greedy: BFS + push de capacidad residual.
    GreedyShortest,
}

/// Reconstruye [`StationFlows`] según el algoritmo elegido.
#[must_use]
pub fn compute_station_flows(
    graph: &LinkGraphStats,
    algo: McfAlgorithm,
    config: McfConfig,
) -> StationFlows {
    match algo {
        McfAlgorithm::Naive => StationFlows::from_link_graph(graph),
        McfAlgorithm::GreedyShortest => greedy_shortest(graph, config),
    }
}

fn edge_amount(units_total: u64) -> u32 {
    u32::try_from(units_total.min(u64::from(u32::MAX))).unwrap_or(0)
}

fn greedy_shortest(graph: &LinkGraphStats, config: McfConfig) -> StationFlows {
    let mut by_cargo: HashMap<CargoType, Vec<(TileCoord, TileCoord, u32)>> = HashMap::new();
    for (key, sample) in &graph.edges {
        let amount = edge_amount(sample.units_total);
        if amount == 0 {
            continue;
        }
        by_cargo
            .entry(key.cargo)
            .or_default()
            .push((key.from, key.to, amount));
    }

    let mut out = StationFlows::default();
    let mut cargos: Vec<CargoType> = by_cargo.keys().copied().collect();
    cargos.sort_by_key(|c| {
        ALL_CARGO_TYPES
            .iter()
            .position(|x| x == c)
            .unwrap_or(usize::MAX)
    });

    for cargo in cargos {
        let Some(edges) = by_cargo.remove(&cargo) else {
            continue;
        };
        if !run_greedy_cargo(&mut out, cargo, &edges, config) {
            for (from, to, amount) in edges {
                out.by_station
                    .entry(from)
                    .or_default()
                    .add_flow(cargo, from, to, amount);
            }
        }
    }
    out
}

/// `true` si aplicó greedy; `false` si el grafo es demasiado grande.
#[allow(clippy::too_many_lines)]
fn run_greedy_cargo(
    out: &mut StationFlows,
    cargo: CargoType,
    edges: &[(TileCoord, TileCoord, u32)],
    config: McfConfig,
) -> bool {
    let mut nodes: Vec<TileCoord> = edges.iter().flat_map(|(a, b, _)| [*a, *b]).collect();
    nodes.sort_by(|a, b| a.x.cmp(&b.x).then_with(|| a.y.cmp(&b.y)));
    nodes.dedup();
    if nodes.len() > MCF_MAX_NODES || edges.len() > MCF_MAX_EDGES {
        return false;
    }
    if nodes.is_empty() {
        return true;
    }

    let index: HashMap<TileCoord, usize> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let n = nodes.len();

    let mut residual = Vec::with_capacity(edges.len());
    let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    let mut total_out = vec![0_u32; n];
    let mut total_in = vec![0_u32; n];

    for (from, to, amount) in edges {
        let ei = residual.len();
        residual.push(*amount);
        let (Some(&u), Some(&v)) = (index.get(from), index.get(to)) else {
            continue;
        };
        if u == v || *amount == 0 {
            continue;
        }
        adj[u].push((v, ei));
        total_out[u] = total_out[u].saturating_add(*amount);
        total_in[v] = total_in[v].saturating_add(*amount);
    }
    for neighbors in &mut adj {
        neighbors.sort_by(|a, b| {
            let na = nodes[a.0];
            let nb = nodes[b.0];
            na.x.cmp(&nb.x).then_with(|| na.y.cmp(&nb.y))
        });
    }

    // Distancias hop iniciales (capacidad > 0) para priorizar sinks lejanos.
    let hop_dist = all_pairs_hop_dist(&adj, &residual, n);

    let accuracy = config.accuracy.max(1);
    let mut supply = total_out;
    let mut demand = total_in;

    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (src, &src_supply) in supply.iter().enumerate() {
        if src_supply == 0 {
            continue;
        }
        for (dst, &dst_demand) in demand.iter().enumerate() {
            if src == dst || dst_demand == 0 {
                continue;
            }
            pairs.push((src, dst));
        }
    }
    pairs.sort_by(|a, b| {
        let da = hop_dist[a.0][a.1];
        let db = hop_dist[b.0][b.1];
        // Más hops primero; inalcanzables (u32::MAX) al final.
        dist_rank(db)
            .cmp(&dist_rank(da))
            .then_with(|| {
                let sa = supply[a.0].min(demand[a.1]);
                let sb = supply[b.0].min(demand[b.1]);
                sb.cmp(&sa)
            })
            .then_with(|| nodes[a.0].x.cmp(&nodes[b.0].x))
            .then_with(|| nodes[a.0].y.cmp(&nodes[b.0].y))
            .then_with(|| nodes[a.1].x.cmp(&nodes[b.1].x))
            .then_with(|| nodes[a.1].y.cmp(&nodes[b.1].y))
    });

    for (src, dst) in pairs {
        let mut guard = 0_u32;
        while supply[src] > 0 && demand[dst] > 0 && guard < accuracy.saturating_mul(4) {
            guard += 1;
            let Some(path) = bfs_path(&adj, &residual, src, dst, n) else {
                break;
            };
            let mut bottleneck = u32::MAX;
            for window in path.windows(2) {
                let u = window[0];
                let v = window[1];
                let Some(ei) = edge_index(&adj[u], v) else {
                    bottleneck = 0;
                    break;
                };
                bottleneck = bottleneck.min(residual[ei]);
            }
            let step = (supply[src].max(1) / accuracy).max(1);
            let chunk = bottleneck.min(supply[src]).min(demand[dst]).min(step);
            if chunk == 0 {
                break;
            }
            for window in path.windows(2) {
                let u = window[0];
                let v = window[1];
                if let Some(ei) = edge_index(&adj[u], v) {
                    residual[ei] = residual[ei].saturating_sub(chunk);
                }
            }
            supply[src] = supply[src].saturating_sub(chunk);
            demand[dst] = demand[dst].saturating_sub(chunk);
            materialize_path(out, cargo, &nodes, &path, chunk);
        }
    }

    // Shares directos residuales (aristas no usadas del todo).
    for (ei, (from, to, _)) in edges.iter().enumerate() {
        let left = residual.get(ei).copied().unwrap_or(0);
        if left > 0 {
            out.by_station
                .entry(*from)
                .or_default()
                .add_flow(cargo, *from, *to, left);
        }
    }
    true
}

fn dist_rank(d: u32) -> u32 {
    if d == u32::MAX { 0 } else { d }
}

fn edge_index(neighbors: &[(usize, usize)], to: usize) -> Option<usize> {
    neighbors.iter().find(|(v, _)| *v == to).map(|(_, e)| *e)
}

fn all_pairs_hop_dist(adj: &[Vec<(usize, usize)>], residual: &[u32], n: usize) -> Vec<Vec<u32>> {
    let mut dist = vec![vec![u32::MAX; n]; n];
    for (src, row) in dist.iter_mut().enumerate() {
        row[src] = 0;
        let mut q = VecDeque::new();
        q.push_back(src);
        while let Some(u) = q.pop_front() {
            let base = row[u];
            for &(v, ei) in &adj[u] {
                if residual[ei] == 0 {
                    continue;
                }
                let next = base.saturating_add(1);
                if next < row[v] {
                    row[v] = next;
                    q.push_back(v);
                }
            }
        }
    }
    dist
}

fn bfs_path(
    adj: &[Vec<(usize, usize)>],
    residual: &[u32],
    src: usize,
    dst: usize,
    n: usize,
) -> Option<Vec<usize>> {
    let mut prev = vec![None; n];
    let mut seen = vec![false; n];
    let mut q = VecDeque::new();
    seen[src] = true;
    q.push_back(src);
    while let Some(u) = q.pop_front() {
        if u == dst {
            break;
        }
        for &(v, ei) in &adj[u] {
            if residual[ei] == 0 || seen[v] {
                continue;
            }
            seen[v] = true;
            prev[v] = Some(u);
            q.push_back(v);
        }
    }
    if !seen[dst] {
        return None;
    }
    let mut path = Vec::new();
    let mut cur = dst;
    path.push(cur);
    while cur != src {
        cur = prev[cur]?;
        path.push(cur);
    }
    path.reverse();
    Some(path)
}

fn materialize_path(
    out: &mut StationFlows,
    cargo: CargoType,
    nodes: &[TileCoord],
    path: &[usize],
    amount: u32,
) {
    if amount == 0 || path.len() < 2 {
        return;
    }
    let origin = nodes[path[0]];
    for window in path.windows(2) {
        let at = nodes[window[0]];
        let via = nodes[window[1]];
        out.by_station
            .entry(at)
            .or_default()
            .add_flow(cargo, origin, via, amount);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::flow_stat::DistributionType;
    use crate::flow_stat::resolve_next_hop;

    #[test]
    fn naive_matches_from_link_graph() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        g.record_flow(a, b, CargoType::Coal, 40);
        let naive = compute_station_flows(&g, McfAlgorithm::Naive, McfConfig::default());
        assert_eq!(naive, StationFlows::from_link_graph(&g));
    }

    #[test]
    fn greedy_single_edge_same_via_as_naive() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(5, 5);
        g.record_flow(a, b, CargoType::Coal, 40);
        let flows = compute_station_flows(&g, McfAlgorithm::GreedyShortest, McfConfig::default());
        assert_eq!(flows.get_via(a, CargoType::Coal, a), Some(b));
    }

    #[test]
    fn greedy_two_hop_sets_via_at_intermediate() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(3, 3);
        let c = TileCoord::new(5, 5);
        g.record_flow(a, b, CargoType::Goods, 30);
        g.record_flow(b, c, CargoType::Goods, 30);
        let flows = compute_station_flows(&g, McfAlgorithm::GreedyShortest, McfConfig::default());
        assert_eq!(
            flows.get_via(a, CargoType::Goods, a),
            Some(b),
            "en origen A el hop es B"
        );
        assert_eq!(
            flows.get_via(b, CargoType::Goods, a),
            Some(c),
            "en B el cargo con origin=A sigue a C"
        );
        assert_eq!(
            resolve_next_hop(
                DistributionType::Asymmetric,
                &flows,
                b,
                CargoType::Goods,
                a,
                Some(TileCoord::new(9, 9))
            ),
            Some(c)
        );
    }

    #[test]
    fn greedy_is_deterministic() {
        let mut graph = LinkGraphStats::default();
        let src = TileCoord::new(0, 0);
        let via_b = TileCoord::new(1, 0);
        let via_c = TileCoord::new(0, 1);
        let sink = TileCoord::new(2, 2);
        graph.record_flow(src, via_b, CargoType::Mail, 10);
        graph.record_flow(src, via_c, CargoType::Mail, 10);
        graph.record_flow(via_b, sink, CargoType::Mail, 5);
        graph.record_flow(via_c, sink, CargoType::Mail, 20);
        let f1 = compute_station_flows(&graph, McfAlgorithm::GreedyShortest, McfConfig::default());
        let f2 = compute_station_flows(&graph, McfAlgorithm::GreedyShortest, McfConfig::default());
        assert_eq!(f1, f2);
    }

    #[test]
    fn empty_graph_yields_empty_flows() {
        let g = LinkGraphStats::default();
        let flows = compute_station_flows(&g, McfAlgorithm::GreedyShortest, McfConfig::default());
        assert!(flows.by_station.is_empty());
    }
}
