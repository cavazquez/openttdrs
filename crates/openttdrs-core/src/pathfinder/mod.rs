//! Pathfinding multi-red: topología, A*, cache, agua, rail legacy y YAPF.

use crate::aircraft_movement::straight_line_path;
use crate::map::{Map, TileCoord};

mod astar;
mod build_corridor;
mod cache;
mod network;
#[allow(dead_code)]
mod rail_legacy;
mod reachable;
mod station_sites;
mod water;
pub mod yapf;

/// Reexport canónico `OpenTTD` (`map::diag_dir_offset`).
pub use crate::map::diag_dir_offset;
pub use build_corridor::{
    find_rail_build_path, find_road_build_path, tile_allows_rail_build, tile_allows_road_build,
};
pub use cache::PathCache;
pub use network::{
    PathNetwork, TunnelWormholes, path_network_for_vehicle, tile_is_path_traversable,
};
pub(crate) use network::{is_rail_network_tile, is_rail_station_tile, is_road_network_tile};
pub use reachable::farthest_reachable_tile;
pub use station_sites::{
    station_entrance_faces_rail, station_entrance_faces_road, station_site_adjacent_to_rail,
    station_site_adjacent_to_transport, station_site_tile_allows_build,
    station_site_tile_needs_clear,
};

/// Encuentra el camino más corto entre `from` y `to` (A* con conectividad por
/// road/track bits); ver [`find_path_with_wormholes`].
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn find_path(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
) -> Option<Vec<TileCoord>> {
    find_path_with_wormholes(map, from, to, network, None)
}

/// Encuentra el camino más corto entre `from` y `to` usando A* sobre una sola red (`Road…` o `Rail…`).
///
/// Los tiles `from` y `to` pueden ser de cualquier tipo (industria, estación, etc.);
/// los tiles **intermedios** deben pertenecer a la red elegida.
///
/// Con `wormholes`, una tesela en la red puede saltar a su pareja JGR en un paso (túnel real).
///
/// Devuelve `Some(path)` donde `path` es la secuencia de teselas desde la primera adyacente
/// a `from` hasta `to` inclusive. Si `from == to` devuelve `Some(vec![])`.
/// Devuelve `None` si no existe camino.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn find_path_with_wormholes(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(vec![]);
    }
    if network == PathNetwork::Rail {
        return find_rail_path(map, from, to, wormholes);
    }
    if network == PathNetwork::Air {
        return Some(straight_line_path(from, to));
    }
    if network == PathNetwork::Water {
        return water::find_water_path(map, from, to);
    }
    astar::find_road_or_tram_path_with_wormholes(map, from, to, network, wormholes)
}

/// A* direccional para vía vía YAPF.
fn find_rail_path(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    yapf::find_rail_path_yapf(map, from, to, wormholes)
}

/// Path ferroviario filtrado por tipo de vía del motor (Fase 6).
#[must_use]
pub fn find_rail_path_for_engine(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
    engine_id: Option<u16>,
) -> Option<Vec<TileCoord>> {
    let required = engine_id.map(crate::rail_type::required_rail_type_for_engine);
    yapf::find_rail_path_yapf_for_type(map, from, to, wormholes, required)
}

/// Variante con caché por tick de simulación (los wormholes son constantes
/// por mapa, así que no forman parte de la clave de caché).
#[must_use]
pub fn find_path_cached(
    map: &Map,
    cache: &mut PathCache,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    wormholes: Option<&TunnelWormholes>,
) -> Option<Vec<TileCoord>> {
    if let Some(path) = cache.get(from, to, network) {
        return Some(path.clone());
    }
    let path = find_path_with_wormholes(map, from, to, network, wormholes)?;
    cache.insert(from, to, network, path.clone());
    Some(path)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{RAIL_TB_X, RAIL_TB_Y, TileKind};
    use crate::tnbp_decode::JgrTunnelRecord;

    #[test]
    fn jgr_wormhole_connects_disconnected_rail_ends() {
        // OpenTTD `TileIndex` asume ancho potencia de 2 (p. ej. 8).
        let mut map = Map::new_flat(8, 1, 0);
        for x in [0_i32, 4] {
            map.set_kind(TileCoord::new(x, 0), TileKind::RailTunnel)
                .unwrap();
        }
        let wh = TunnelWormholes::from_jgr_records(
            &map,
            &[JgrTunnelRecord {
                tile_n: 0,
                tile_s: 4,
                height: 1,
                is_chunnel: false,
                style_n: None,
                style_s: None,
            }],
        );
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(4, 0);
        assert!(wh.other_end(from).is_some());
        assert!(find_path(&map, from, to, PathNetwork::Rail).is_none());
        let path = find_path_with_wormholes(&map, from, to, PathNetwork::Rail, Some(&wh))
            .expect("wormhole");
        assert_eq!(path.last(), Some(&to));
    }

    fn write_road(m: &mut Map, c: TileCoord, bits: u8) {
        m.set_kind(c, TileKind::Road).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = bits & 0x0F;
        m.set_tile(c, t).unwrap();
    }

    fn write_rail(m: &mut Map, c: TileCoord, trackbits: u8) {
        m.set_kind(c, TileKind::Rail).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = trackbits & 0x3F;
        m.set_tile(c, t).unwrap();
    }

    #[test]
    fn astar_finds_path_on_straight_road() {
        let mut m = Map::new_flat(8, 8, 0);
        for x in 0..=4_i32 {
            write_road(&mut m, TileCoord::new(x, 0), 0x0A);
        }
        let path = find_path(
            &m,
            TileCoord::new(0, 0),
            TileCoord::new(4, 0),
            PathNetwork::Road,
        );
        assert!(path.is_some());
        assert_eq!(*path.unwrap().last().unwrap(), TileCoord::new(4, 0));
    }

    #[test]
    fn astar_respects_road_bit_gap() {
        let mut m = Map::new_flat(8, 8, 0);
        write_road(&mut m, TileCoord::new(0, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 1), 0x03);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 0),
                TileCoord::new(1, 1),
                PathNetwork::Road,
            )
            .is_none()
        );
    }

    #[test]
    fn astar_finds_detour_when_direct_gap_blocked() {
        let mut m = Map::new_flat(8, 8, 0);
        write_road(&mut m, TileCoord::new(0, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 0), 0x0A);
        write_road(&mut m, TileCoord::new(2, 0), 0x0F);
        write_road(&mut m, TileCoord::new(2, 1), 0x0F);
        write_road(&mut m, TileCoord::new(1, 1), 0x0A);
        write_road(&mut m, TileCoord::new(0, 1), 0x0A);
        let path = find_path(
            &m,
            TileCoord::new(0, 0),
            TileCoord::new(0, 1),
            PathNetwork::Road,
        )
        .expect("debe rodear por (2,0)");
        assert_eq!(path.last().copied(), Some(TileCoord::new(0, 1)));
        assert!(path.len() >= 4);
    }

    #[test]
    fn astar_rail_requires_matching_axis() {
        let mut m = Map::new_flat(6, 6, 0);
        write_rail(&mut m, TileCoord::new(0, 0), RAIL_TB_X);
        write_rail(&mut m, TileCoord::new(1, 0), RAIL_TB_Y);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 0),
                TileCoord::new(1, 0),
                PathNetwork::Rail,
            )
            .is_none()
        );
        write_rail(&mut m, TileCoord::new(1, 0), RAIL_TB_X);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 0),
                TileCoord::new(1, 0),
                PathNetwork::Rail,
            )
            .is_some()
        );
    }

    #[test]
    fn astar_rail_no_turn_at_plain_crossing() {
        let mut m = Map::new_flat(8, 8, 0);
        // Línea X en y=2 y línea Y en x=2; (2,2) es cruce X|Y sin curvas.
        for x in 0..=4_i32 {
            write_rail(&mut m, TileCoord::new(x, 2), RAIL_TB_X);
        }
        for y in 0..=4_i32 {
            if y != 2 {
                write_rail(&mut m, TileCoord::new(2, y), RAIL_TB_Y);
            }
        }
        write_rail(&mut m, TileCoord::new(2, 2), RAIL_TB_X | RAIL_TB_Y);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 2),
                TileCoord::new(4, 2),
                PathNetwork::Rail
            )
            .is_some(),
            "recto a través del cruce"
        );
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 2),
                TileCoord::new(2, 0),
                PathNetwork::Rail
            )
            .is_none(),
            "el tren no debe doblar en un cruce sin curva"
        );
        // Con la pieza UPPER (NE↔NW) el giro sí es válido.
        write_rail(&mut m, TileCoord::new(2, 2), RAIL_TB_X | RAIL_TB_Y | 0x04);
        assert!(
            find_path(
                &m,
                TileCoord::new(0, 2),
                TileCoord::new(2, 0),
                PathNetwork::Rail
            )
            .is_some(),
            "con curva el giro es válido"
        );
    }

    #[test]
    fn astar_rail_station_reaches_platform_along_axis() {
        let mut m = Map::new_flat(12, 12, 0);
        let station = TileCoord::new(4, 5);
        let track = TileCoord::new(5, 5);
        m.set_kind(station, TileKind::Station).unwrap();
        let mut st = m.get(station).unwrap();
        st.m6 &= !0x78;
        st.m5 = 2;
        m.set_tile(station, st).unwrap();
        write_rail(&mut m, track, RAIL_TB_X);
        for x in 3..=6_i32 {
            write_rail(&mut m, TileCoord::new(x, 5), RAIL_TB_X);
        }
        assert!(
            find_path(&m, track, TileCoord::new(6, 5), PathNetwork::Rail).is_some(),
            "vía horizontal → vía (sin entrar en plataforma)"
        );
        assert!(
            find_path(&m, track, station, PathNetwork::Rail).is_some(),
            "el tren debe poder rutear hacia la plataforma conectada por el eje"
        );
    }

    #[test]
    fn path_cache_reuses_result_within_tick() {
        let mut m = Map::new_flat(8, 8, 0);
        write_road(&mut m, TileCoord::new(0, 0), 0x0A);
        write_road(&mut m, TileCoord::new(1, 0), 0x0A);
        let mut cache = PathCache::default();
        cache.begin_tick(1);
        let a = find_path_cached(
            &m,
            &mut cache,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            PathNetwork::Road,
            None,
        );
        let b = find_path_cached(
            &m,
            &mut cache,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            PathNetwork::Road,
            None,
        );
        assert_eq!(a, b);
        cache.begin_tick(2);
        assert!(
            cache
                .get(
                    TileCoord::new(0, 0),
                    TileCoord::new(1, 0),
                    PathNetwork::Road
                )
                .is_none()
        );
    }

    fn write_tram(map: &mut Map, c: TileCoord, bits: u8) {
        use crate::road_type::{RoadType, set_tram_road_type_on_tile, set_tram_track_bits_on_tile};
        map.set_kind(c, TileKind::Road).unwrap();
        let mut t = map.get(c).unwrap();
        t.m5 = 0; // sin carretera: solo overlay tram
        t = set_tram_track_bits_on_tile(t, bits);
        t = set_tram_road_type_on_tile(t, Some(RoadType::Tram));
        map.set_tile(c, t).unwrap();
    }

    #[test]
    fn tram_path_follows_m3_not_m5() {
        let mut m = Map::new_flat(6, 6, 0);
        write_tram(&mut m, TileCoord::new(1, 1), 0x0A); // E-W
        write_tram(&mut m, TileCoord::new(2, 1), 0x0A);
        write_tram(&mut m, TileCoord::new(3, 1), 0x0A);
        assert!(
            find_path(
                &m,
                TileCoord::new(1, 1),
                TileCoord::new(3, 1),
                PathNetwork::Tram
            )
            .is_some()
        );
        // Road pathfinder no ve tiles sin m5 (fallback 0x0F en Road vacío… wait)
        // Con m5=0 el road trata como 0x0F, así que Road SÍ conectaría.
        // Verificamos que un tile sin m3 no es red Tram:
        m.set_kind(TileCoord::new(4, 1), TileKind::Road).unwrap();
        let mut t = m.get(TileCoord::new(4, 1)).unwrap();
        t.m5 = 0x0A;
        m.set_tile(TileCoord::new(4, 1), t).unwrap();
        assert!(
            find_path(
                &m,
                TileCoord::new(3, 1),
                TileCoord::new(4, 1),
                PathNetwork::Tram
            )
            .is_none(),
            "tile solo-road sin m3 no es red de tranvía"
        );
    }
}
