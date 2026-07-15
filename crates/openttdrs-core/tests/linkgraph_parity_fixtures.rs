//! Fixtures de paridad linkgraph (Demand + pipeline MCF + GetVia).
#![allow(clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use openttdrs_core::linkgraph_parity::{
    BaseEdge, BaseNode, DistributionType, Job, LinkGraphSettings, calculate_demands,
    flows_as_simple_shares, run_full_pipeline,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FixtureFile {
    settings: FixtureSettings,
    nodes: Vec<FixtureNode>,
    edges: Vec<FixtureEdge>,
    #[serde(default)]
    expected_demand: Vec<FixtureDemand>,
    #[serde(default)]
    expected_flows: Vec<FixtureFlow>,
}

#[derive(Debug, Deserialize)]
struct FixtureSettings {
    accuracy: u32,
    demand_size: u32,
    demand_distance: u32,
    short_path_saturation: u32,
    distribution: String,
    map_max_x: u32,
    map_max_y: u32,
    #[serde(default = "default_runtime")]
    runtime: u32,
}

const fn default_runtime() -> u32 {
    30
}

#[derive(Debug, Deserialize)]
struct FixtureNode {
    station: u32,
    x: u32,
    y: u32,
    supply: u32,
    demand: u32,
}

#[derive(Debug, Deserialize)]
struct FixtureEdge {
    from: u16,
    to: u16,
    capacity: u32,
    usage: u32,
    travel_time: u32,
}

#[derive(Debug, Deserialize)]
struct FixtureDemand {
    from: u16,
    to: u16,
    demand: u32,
}

#[derive(Debug, Deserialize)]
struct FixtureFlow {
    at: u32,
    origin: u32,
    via: u32,
    amount: u32,
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/linkgraph")
        .join(name)
}

fn load_job(name: &str) -> (Job, FixtureFile) {
    let text = fs::read_to_string(fixture_path(name)).expect("fixture");
    let fx: FixtureFile = serde_json::from_str(&text).expect("json");
    let distribution = match fx.settings.distribution.as_str() {
        "symmetric" => DistributionType::Symmetric,
        "manual" => DistributionType::Manual,
        _ => DistributionType::Asymmetric,
    };
    let settings = LinkGraphSettings {
        accuracy: fx.settings.accuracy,
        demand_size: fx.settings.demand_size,
        demand_distance: fx.settings.demand_distance,
        short_path_saturation: fx.settings.short_path_saturation,
        distribution,
        recalc_time: 30,
        map_max_x: fx.settings.map_max_x,
        map_max_y: fx.settings.map_max_y,
    };
    let nodes = fx
        .nodes
        .iter()
        .map(|n| BaseNode {
            station: n.station,
            x: n.x,
            y: n.y,
            supply: n.supply,
            demand: n.demand,
        })
        .collect();
    let size = fx.nodes.len();
    let mut edges = vec![Vec::new(); size];
    for e in &fx.edges {
        edges[usize::from(e.from)].push(BaseEdge {
            dest: e.to,
            capacity: e.capacity,
            usage: e.usage,
            travel_time: e.travel_time,
        });
    }
    let mut job = Job::new(nodes, edges, settings);
    job.runtime = fx.settings.runtime;
    (job, fx)
}

fn assert_fixture(name: &str) {
    let (mut job, fx) = load_job(name);
    calculate_demands(&mut job);
    for exp in &fx.expected_demand {
        let got = job.demand_to(exp.from, exp.to);
        assert_eq!(
            got, exp.demand,
            "{name}: demand {}→{}: got {got}, expected {}",
            exp.from, exp.to, exp.demand
        );
    }
    // Recargar job limpio para el pipeline completo (demands se recalculan).
    let (mut job, fx) = load_job(name);
    run_full_pipeline(&mut job);
    let shares = flows_as_simple_shares(&job);
    if !fx.expected_flows.is_empty() {
        assert_eq!(
            shares.len(),
            fx.expected_flows.len(),
            "{name}: número de shares; got={shares:?}"
        );
        for (got, exp) in shares.iter().zip(fx.expected_flows.iter()) {
            assert_eq!(
                (
                    got.at_station,
                    got.origin_station,
                    got.via_station,
                    got.amount
                ),
                (exp.at, exp.origin, exp.via, exp.amount),
                "{name}: share mismatch; got={shares:?}"
            );
        }
    }
}

#[test]
fn fixture_asymmetric_two_node() {
    assert_fixture("asymmetric_two_node.json");
}

#[test]
fn fixture_symmetric_two_node() {
    assert_fixture("symmetric_mirror_nodes.json");
}

#[test]
fn fixture_three_node_linear() {
    assert_fixture("three_node_linear.json");
}

#[test]
fn fixture_three_node_cycle() {
    assert_fixture("three_node_cycle.json");
}

#[test]
fn fixture_express_vs_local() {
    assert_fixture("express_vs_local.json");
}

#[test]
fn get_via_random_golden_sequence() {
    use openttdrs_core::linkgraph_parity::{FlowStat, Randomizer};

    let mut fs = FlowStat::new(1, 10, false);
    fs.append_share(2, 30, false);
    fs.append_share(3, 60, false);
    let mut rng = Randomizer::new(1);
    let mut vias = Vec::new();
    for _ in 0..16 {
        vias.push(fs.get_via(u32::MAX, u32::MAX, &mut rng));
    }
    let expected: [u32; 16] = [2, 3, 2, 2, 3, 2, 2, 3, 2, 3, 3, 1, 3, 2, 2, 3];
    assert_eq!(vias, expected);
}

#[test]
fn get_via_random_10k_checksum() {
    use openttdrs_core::linkgraph_parity::{FlowStat, Randomizer};

    let mut fs = FlowStat::new(1, 10, false);
    fs.append_share(2, 30, false);
    fs.append_share(3, 60, false);
    let mut rng = Randomizer::new(1);
    let mut sum = 0_u64;
    let mut counts = [0_u64; 4];
    for i in 0..10_000_u64 {
        let v = fs.get_via(u32::MAX, u32::MAX, &mut rng);
        sum = sum.wrapping_add(u64::from(v).wrapping_mul(i.wrapping_add(1)));
        if (v as usize) < 4 {
            counts[v as usize] += 1;
        }
    }
    assert_eq!(sum, 124_719_258);
    assert_eq!(counts, [0, 1030, 2967, 6003]);
}
