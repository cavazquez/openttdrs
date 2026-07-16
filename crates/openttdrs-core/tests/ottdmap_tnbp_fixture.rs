//! Fixture `fixtures/v5p12_tnbp.ottdmap`: mapa 2×2 con dos teselas `MP_TUNNELBRIDGE` y footer TNBP (tabla JGR mínima).

#![allow(clippy::expect_used)]

use openttdrs_core::openttd_tile_index_to_coord;
use openttdrs_core::prelude::{Map, TileCoord};
use openttdrs_core::tnbp_decode::{JgrTunnelRecord, tnbp_blob_to_json_value};

const FIXTURE: &[u8] = include_bytes!("fixtures/v5p12_tnbp.ottdmap");

#[test]
fn loads_tnbp_fixture_decode_and_tile_coords() {
    let (map, ex) = Map::from_ottd_binary_with_extras(FIXTURE).expect("fixture");
    assert_eq!(map.dimensions(), (2, 2));
    let t0 = map.get(TileCoord::new(0, 0)).expect("t0");
    assert!(t0.is_tunnel_bridge_tile());
    let t1 = map.get(TileCoord::new(1, 0)).expect("t1");
    assert!(t1.is_tunnel_bridge_tile());

    let blob = ex.tnbp_blob.as_deref().expect("TNBP");
    let jgr = ex.jgr_tunnels_from_tnbp();
    assert_eq!(
        jgr,
        vec![JgrTunnelRecord {
            tile_n: 0,
            tile_s: 1,
            height: 4,
            is_chunnel: false,
            style_n: None,
            style_s: None,
        }]
    );
    assert_eq!(
        openttd_tile_index_to_coord(0, 2, 2),
        Some(TileCoord::new(0, 0))
    );
    assert_eq!(
        openttd_tile_index_to_coord(1, 2, 2),
        Some(TileCoord::new(1, 0))
    );
    let (n_ok, s_ok, n_tot) = map.jgr_tunnel_endpoint_match_stats(&jgr);
    assert_eq!(n_tot, 1);
    assert_eq!(n_ok, 1);
    assert_eq!(s_ok, 1);

    let j = tnbp_blob_to_json_value(blob);
    assert_eq!(j["ok"], true);
    assert_eq!(j["kind"], "ch_table");
    assert_eq!(j["jgr_tunnel_count"], 1);
}
