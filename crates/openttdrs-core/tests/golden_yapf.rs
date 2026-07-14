//! Golden estático de rutas YAPF (#53 slice).
//!
//! Fija longitudes/waypoints de rutas en escenarios conocidos. No es tick-a-tick
//! vs OpenTTD (eso requiere captura externa — follow-up del issue #53).
//!
//! Regenerar: `OPENTTDRS_UPDATE_GOLDEN=1 cargo test -p openttdrs-core --test golden_yapf`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::{
    TileCoord,
    parity::{
        TRAIN_PBS_GOAL_X, TRAIN_PBS_NORTH_Y, TRAIN_PBS_SOUTH_Y, build_train_line, build_train_pbs,
    },
    pathfinder::yapf::find_rail_path_yapf,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RouteRow {
    label: String,
    from: (i32, i32),
    to: (i32, i32),
    path_len: usize,
    /// Primeras teselas del path (sin incluir `from`).
    path_prefix: Vec<(i32, i32)>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Fixture {
    note: String,
    routes: Vec<RouteRow>,
}

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity/yapf_routes_golden.json")
}

fn route_row(label: &str, map: &openttdrs_core::Map, from: TileCoord, to: TileCoord) -> RouteRow {
    let path = find_rail_path_yapf(map, from, to, None).expect("ruta YAPF");
    let path_prefix: Vec<(i32, i32)> = path.iter().take(4).map(|c| (c.x, c.y)).collect();
    RouteRow {
        label: label.into(),
        from: (from.x, from.y),
        to: (to.x, to.y),
        path_len: path.len(),
        path_prefix,
    }
}

fn collect_routes() -> Vec<RouteRow> {
    let pbs = build_train_pbs();
    let line = build_train_line();
    vec![
        route_row(
            "train_pbs_north",
            &pbs.map,
            TileCoord::new(1, TRAIN_PBS_NORTH_Y),
            TileCoord::new(TRAIN_PBS_GOAL_X, TRAIN_PBS_NORTH_Y),
        ),
        route_row(
            "train_pbs_south",
            &pbs.map,
            TileCoord::new(1, TRAIN_PBS_SOUTH_Y),
            TileCoord::new(TRAIN_PBS_GOAL_X, TRAIN_PBS_SOUTH_Y),
        ),
        route_row(
            "train_line_depot_exit_to_corner",
            &line.map,
            TileCoord::new(2, 6),
            TileCoord::new(12, 10),
        ),
    ]
}

#[test]
fn yapf_static_routes_match_golden() {
    let routes = collect_routes();
    for r in &routes {
        assert!(r.path_len >= 3, "{} path corto: {}", r.label, r.path_len);
    }

    let path = fixture_path();
    if std::env::var_os("OPENTTDRS_UPDATE_GOLDEN").is_some() {
        let fixture = Fixture {
            note: "Rutas YAPF estáticas (#53 slice). No es golden tick-a-tick vs OpenTTD.".into(),
            routes: routes.clone(),
        };
        let json = serde_json::to_string_pretty(&fixture).expect("serialize");
        std::fs::write(&path, format!("{json}\n")).expect("write");
        eprintln!("actualizado {}", path.display());
        return;
    }

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()));
    let fixture: Fixture = serde_json::from_str(&raw).expect("parse");
    assert_eq!(fixture.routes, routes);
}
