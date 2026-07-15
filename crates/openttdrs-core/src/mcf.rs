//! Stub MCF legado (#49 MVP). El camino de juego usa [`crate::linkgraph_parity`].
//!
//! Se mantiene para tests de regresión del aproximador BFS:
//! - [`McfAlgorithm::GreedyShortest`]: un pase BFS hop-count.
//! - [`McfAlgorithm::CapacityScaled`]: pase 1 con tope `short_path_saturation`
//!   + pase 2 priorizando bottleneck residual en aristas ya usadas.

use std::collections::{HashMap, VecDeque};

use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::flow_stat::{DistributionType, StationFlows};
use crate::link_graph::{LinkEdgeKey, LinkGraphStats};
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
    /// % de capacidad usable en el 1.er pase (`1..=100`).
    pub short_path_saturation: u32,
}

impl Default for McfConfig {
    fn default() -> Self {
        Self {
            accuracy: 16,
            short_path_saturation: 80,
        }
    }
}

/// Algoritmo de reconstrucción de flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McfAlgorithm {
    /// 1 arista observada = 1 share directo.
    #[default]
    Naive,
    /// Un pase greedy (BFS hop-count).
    GreedyShortest,
    /// Pase 1 (corto, saturación limitada) + pase 2 (capacidad residual).
    CapacityScaled,
}

/// Espeja aristas observadas A→B ⇒ B→A (mínimo viable Symmetric).
#[must_use]
pub fn symmetrize_observed_edges(graph: &LinkGraphStats) -> LinkGraphStats {
    let mut out = graph.clone();
    let snapshot: Vec<(LinkEdgeKey, u32)> = graph
        .edges
        .iter()
        .map(|(k, s)| (*k, edge_amount(s.units_total)))
        .filter(|(_, a)| *a > 0)
        .collect();
    for (key, amount) in snapshot {
        let rev = LinkEdgeKey {
            from: key.to,
            to: key.from,
            cargo: key.cargo,
        };
        let existing = out
            .edges
            .get(&rev)
            .map_or(0, |s| edge_amount(s.units_total));
        if existing < amount {
            out.record_flow(rev.from, rev.to, rev.cargo, amount - existing);
        }
    }
    out
}

/// Elige algoritmo según modo de distribución.
#[must_use]
pub fn compute_station_flows_for_distribution(
    graph: &LinkGraphStats,
    distribution: DistributionType,
    config: McfConfig,
) -> StationFlows {
    match distribution {
        DistributionType::Manual => compute_station_flows(graph, McfAlgorithm::Naive, config),
        DistributionType::Asymmetric => {
            compute_station_flows(graph, McfAlgorithm::CapacityScaled, config)
        }
        DistributionType::Symmetric => {
            let mirrored = symmetrize_observed_edges(graph);
            compute_station_flows(&mirrored, McfAlgorithm::CapacityScaled, config)
        }
    }
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
        McfAlgorithm::GreedyShortest => run_by_cargo(graph, config, PassMode::GreedyOnePass),
        McfAlgorithm::CapacityScaled => run_by_cargo(graph, config, PassMode::CapacityScaled),
    }
}

#[derive(Clone, Copy)]
enum PassMode {
    GreedyOnePass,
    CapacityScaled,
}

fn edge_amount(units_total: u64) -> u32 {
    u32::try_from(units_total.min(u64::from(u32::MAX))).unwrap_or(0)
}

fn run_by_cargo(graph: &LinkGraphStats, config: McfConfig, mode: PassMode) -> StationFlows {
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
        if !run_cargo_mcf(&mut out, cargo, &edges, config, mode) {
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

#[allow(clippy::too_many_lines)]
fn run_cargo_mcf(
    out: &mut StationFlows,
    cargo: CargoType,
    edges: &[(TileCoord, TileCoord, u32)],
    config: McfConfig,
    mode: PassMode,
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

    let hop_dist = all_pairs_hop_dist(&adj, &residual, n);
    let accuracy = config.accuracy.max(1);
    let sat = config.short_path_saturation.clamp(1, 100);
    let mut supply = total_out;
    let mut demand = total_in;
    let mut used = vec![false; residual.len()];
    // Tope del pase 1 por arista.
    let mut pass1_left: Vec<u32> = residual
        .iter()
        .map(|&cap| {
            if matches!(mode, PassMode::CapacityScaled) {
                let limited = (u64::from(cap) * u64::from(sat)) / 100;
                u32::try_from(limited.max(1)).unwrap_or(1)
            } else {
                cap
            }
        })
        .collect();

    let pairs = sorted_pairs(&supply, &demand, &hop_dist, &nodes);

    // Pase 1: caminos cortos (BFS hop), con tope de saturación si CapacityScaled.
    push_pairs(
        out,
        cargo,
        &nodes,
        &adj,
        &mut residual,
        &mut pass1_left,
        &mut used,
        &mut supply,
        &mut demand,
        &pairs,
        accuracy,
        PathPrefer::ShortestHop,
        true, // respetar pass1_left
    );

    if matches!(mode, PassMode::CapacityScaled) {
        // Pase 2: solo aristas usadas; preferir mayor bottleneck.
        push_pairs(
            out,
            cargo,
            &nodes,
            &adj,
            &mut residual,
            &mut pass1_left,
            &mut used,
            &mut supply,
            &mut demand,
            &pairs,
            accuracy,
            PathPrefer::MaxBottleneckUsed,
            false,
        );
    }

    // Greedy one-pass: shares directos residuales (compat tests 1-hop).
    if matches!(mode, PassMode::GreedyOnePass) {
        for (ei, (from, to, _)) in edges.iter().enumerate() {
            let left = residual.get(ei).copied().unwrap_or(0);
            if left > 0 {
                out.by_station
                    .entry(*from)
                    .or_default()
                    .add_flow(cargo, *from, *to, left);
            }
        }
    }
    true
}

#[derive(Clone, Copy)]
enum PathPrefer {
    ShortestHop,
    MaxBottleneckUsed,
}

#[allow(clippy::too_many_arguments)]
fn push_pairs(
    out: &mut StationFlows,
    cargo: CargoType,
    nodes: &[TileCoord],
    adj: &[Vec<(usize, usize)>],
    residual: &mut [u32],
    pass1_left: &mut [u32],
    used: &mut [bool],
    supply: &mut [u32],
    demand: &mut [u32],
    pairs: &[(usize, usize)],
    accuracy: u32,
    prefer: PathPrefer,
    respect_pass1_cap: bool,
) {
    let n = nodes.len();
    for &(src, dst) in pairs {
        let mut guard = 0_u32;
        while supply[src] > 0 && demand[dst] > 0 && guard < accuracy.saturating_mul(4) {
            guard += 1;
            let Some(path) = (match prefer {
                PathPrefer::ShortestHop => bfs_path(adj, residual, used, src, dst, n, false),
                PathPrefer::MaxBottleneckUsed => {
                    bfs_path_max_bottleneck(adj, residual, used, src, dst, n)
                }
            }) else {
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
                let mut edge_cap = residual[ei];
                if respect_pass1_cap {
                    edge_cap = edge_cap.min(pass1_left[ei]);
                }
                bottleneck = bottleneck.min(edge_cap);
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
                    if respect_pass1_cap {
                        pass1_left[ei] = pass1_left[ei].saturating_sub(chunk);
                    }
                    used[ei] = true;
                }
            }
            supply[src] = supply[src].saturating_sub(chunk);
            demand[dst] = demand[dst].saturating_sub(chunk);
            materialize_path(out, cargo, nodes, &path, chunk);
        }
    }
}

fn sorted_pairs(
    supply: &[u32],
    demand: &[u32],
    hop_dist: &[Vec<u32>],
    nodes: &[TileCoord],
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
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
    pairs
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
    used: &[bool],
    src: usize,
    dst: usize,
    n: usize,
    only_used: bool,
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
            if only_used && !used[ei] {
                continue;
            }
            seen[v] = true;
            prev[v] = Some(u);
            q.push_back(v);
        }
    }
    reconstruct_path(&prev, seen[dst], src, dst)
}

/// Dijkstra-lite: maximiza el bottleneck mínimo; solo aristas `used`.
fn bfs_path_max_bottleneck(
    adj: &[Vec<(usize, usize)>],
    residual: &[u32],
    used: &[bool],
    src: usize,
    dst: usize,
    n: usize,
) -> Option<Vec<usize>> {
    let mut best_bot = vec![0_u32; n];
    let mut prev = vec![None; n];
    best_bot[src] = u32::MAX;
    // Cola simple re-escaneada (n pequeño).
    let mut active = vec![false; n];
    active[src] = true;
    while let Some(u) = (0..n).filter(|&i| active[i]).max_by_key(|&i| best_bot[i]) {
        active[u] = false;
        if u == dst {
            break;
        }
        for &(v, ei) in &adj[u] {
            if !used[ei] || residual[ei] == 0 {
                continue;
            }
            let cand = best_bot[u].min(residual[ei]);
            if cand > best_bot[v] {
                best_bot[v] = cand;
                prev[v] = Some(u);
                active[v] = true;
            }
        }
    }
    if best_bot[dst] == 0 {
        return None;
    }
    reconstruct_path(&prev, true, src, dst)
}

fn reconstruct_path(
    prev: &[Option<usize>],
    reached: bool,
    src: usize,
    dst: usize,
) -> Option<Vec<usize>> {
    if !reached {
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
        assert_eq!(flows.get_via(a, CargoType::Goods, a), Some(b));
        assert_eq!(flows.get_via(b, CargoType::Goods, a), Some(c));
        let mut rng = crate::linkgraph_parity::Randomizer::new(1);
        assert_eq!(
            resolve_next_hop(
                DistributionType::Asymmetric,
                &flows,
                b,
                CargoType::Goods,
                a,
                Some(TileCoord::new(9, 9)),
                &mut rng,
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
        let flows = compute_station_flows(&g, McfAlgorithm::CapacityScaled, McfConfig::default());
        assert!(flows.by_station.is_empty());
    }

    #[test]
    fn capacity_scaled_pass1_respects_saturation() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(0, 0);
        let b = TileCoord::new(2, 0);
        g.record_flow(a, b, CargoType::Coal, 100);
        let cfg = McfConfig {
            accuracy: 1,
            short_path_saturation: 50,
        };
        let flows = compute_station_flows(&g, McfAlgorithm::CapacityScaled, cfg);
        // Con un solo edge, pase1 empuja ≤50 y pase2 el resto → hop sigue siendo B.
        assert_eq!(flows.get_via(a, CargoType::Coal, a), Some(b));
        let share = flows
            .by_station
            .get(&a)
            .and_then(|t| t.by_cargo.get(&CargoType::Coal))
            .and_then(|m| m.by_origin.get(&a))
            .map_or(0, |fs| fs.get_share(b));
        assert!(share >= 50, "debe materializar al menos el pase1 ({share})");
    }

    #[test]
    fn capacity_scaled_pass2_uses_fat_edge() {
        // A→thin→D (capa 10) y A→fat→D (capa 100); saturation 10% → pase1
        // satura thin parcialmente; pase2 debe preferir fat.
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(0, 0);
        let thin = TileCoord::new(1, 0);
        let fat = TileCoord::new(0, 1);
        let d = TileCoord::new(2, 2);
        g.record_flow(a, thin, CargoType::Mail, 10);
        g.record_flow(thin, d, CargoType::Mail, 10);
        g.record_flow(a, fat, CargoType::Mail, 100);
        g.record_flow(fat, d, CargoType::Mail, 100);
        let cfg = McfConfig {
            accuracy: 8,
            short_path_saturation: 20,
        };
        let flows = compute_station_flows(&g, McfAlgorithm::CapacityScaled, cfg);
        let via_fat = flows
            .by_station
            .get(&a)
            .and_then(|t| t.by_cargo.get(&CargoType::Mail))
            .and_then(|m| m.by_origin.get(&a))
            .map_or(0, |fs| fs.get_share(fat));
        let via_thin = flows
            .by_station
            .get(&a)
            .and_then(|t| t.by_cargo.get(&CargoType::Mail))
            .and_then(|m| m.by_origin.get(&a))
            .map_or(0, |fs| fs.get_share(thin));
        assert!(
            via_fat >= via_thin,
            "pase2 debe preferir arista gruesa: fat={via_fat} thin={via_thin}"
        );
    }

    #[test]
    fn capacity_scaled_deterministic() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(0, 0);
        let b = TileCoord::new(1, 1);
        let c = TileCoord::new(2, 2);
        g.record_flow(a, b, CargoType::Goods, 40);
        g.record_flow(b, c, CargoType::Goods, 40);
        let f1 = compute_station_flows(&g, McfAlgorithm::CapacityScaled, McfConfig::default());
        let f2 = compute_station_flows(&g, McfAlgorithm::CapacityScaled, McfConfig::default());
        assert_eq!(f1, f2);
    }

    #[test]
    fn symmetric_mirrors_one_way_edge() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(5, 5);
        g.record_flow(a, b, CargoType::Coal, 40);
        let asym = compute_station_flows_for_distribution(
            &g,
            DistributionType::Asymmetric,
            McfConfig::default(),
        );
        let sym = compute_station_flows_for_distribution(
            &g,
            DistributionType::Symmetric,
            McfConfig::default(),
        );
        assert_eq!(asym.get_via(a, CargoType::Coal, a), Some(b));
        assert_eq!(
            asym.get_via(b, CargoType::Coal, b),
            None,
            "Asymmetric no espeja"
        );
        assert_eq!(
            sym.get_via(b, CargoType::Coal, b),
            Some(a),
            "Symmetric espeja B→A"
        );
    }

    #[test]
    fn symmetrize_observed_edges_adds_reverse() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(0, 0);
        let b = TileCoord::new(1, 0);
        g.record_flow(a, b, CargoType::Wood, 25);
        let mirrored = symmetrize_observed_edges(&g);
        let rev = mirrored.edges.get(&LinkEdgeKey {
            from: b,
            to: a,
            cargo: CargoType::Wood,
        });
        assert_eq!(rev.map(|s| s.units_total), Some(25));
    }
}
