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
pub(crate) use network::{
    is_rail_network_tile, is_rail_station_tile, is_road_network_tile, tunnel_other_end,
};
pub use reachable::farthest_reachable_tile;
pub use station_sites::{
    station_entrance_faces_rail, station_entrance_faces_road, station_site_adjacent_to_rail,
    station_site_adjacent_to_transport, station_site_tile_allows_build,
    station_site_tile_needs_clear,
};
pub use water::ShipPathCost;

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

/// Encuentra una ruta naval ponderando las reducciones de velocidad de
/// `YapfShip` para mar/canal-río.
#[must_use]
pub fn find_ship_path_with_cost(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    cost: ShipPathCost,
) -> Option<Vec<TileCoord>> {
    water::find_ship_path(map, from, to, cost)
}

/// Encuentra una ruta naval iniciando únicamente desde el `Trackdir` físico
/// actual del barco, como `YapfShipChooseTrack`.
#[must_use]
pub fn find_ship_path_with_cost_and_trackdir(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    cost: ShipPathCost,
    origin_trackdir: u8,
) -> Option<Vec<TileCoord>> {
    water::find_ship_path_with_trackdir(map, from, to, cost, origin_trackdir)
}

/// Coste acumulado de una ruta naval según las propiedades de `YapfShip`.
#[must_use]
pub fn ship_path_cost_for_path(
    map: &Map,
    from: TileCoord,
    path: &[TileCoord],
    cost: ShipPathCost,
) -> u32 {
    cost.path_cost(map, from, path)
}

/// Coste naval de una ruta usando el `Trackdir` físico de origen.
#[must_use]
pub fn ship_path_cost_for_path_with_trackdir(
    map: &Map,
    from: TileCoord,
    path: &[TileCoord],
    cost: ShipPathCost,
    origin_trackdir: u8,
) -> u32 {
    cost.path_cost_with_trackdir(map, from, path, origin_trackdir)
}

/// Variante naval que extrae las propiedades de velocidad del motor.
#[must_use]
pub fn find_ship_path_for_engine(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    engine: &crate::engine::EngineDef,
) -> Option<Vec<TileCoord>> {
    find_ship_path_with_cost(map, from, to, ShipPathCost::from_engine(engine))
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
    find_rail_path_for_engine_with_catalog(map, from, to, wormholes, engine_id, &[])
}

/// Variante de [`find_rail_path_for_engine`] que resuelve primero el motor en
/// el catálogo runtime. Los motores `NewGRF` pueden declarar su `RailType`
/// requerido mediante Action0; si el catálogo no contiene el ID, se conserva
/// el fallback de los motores vanilla.
#[must_use]
pub fn find_rail_path_for_engine_with_catalog(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    wormholes: Option<&TunnelWormholes>,
    engine_id: Option<u16>,
    engine_catalog: &[crate::engine::EngineDef],
) -> Option<Vec<TileCoord>> {
    let required = engine_id.map(|id| {
        let engine = crate::engine::engine_in_catalog(engine_catalog, id)
            .or_else(|| crate::engine::engine_by_id(id));
        engine.map_or_else(
            || crate::rail_type::required_rail_type_for_engine(id),
            |engine| {
                engine.required_rail_type.map_or_else(
                    || crate::rail_type::required_rail_type_for_engine(engine.id),
                    crate::rail_type::RailType::from_u8,
                )
            },
        )
    });
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

/// Variante con caché de [`find_ship_path_with_cost`]. La clave incluye las
/// propiedades de velocidad y penalizaciones para no reutilizar una ruta con
/// otro perfil naval.
#[must_use]
pub fn find_ship_path_cached(
    map: &Map,
    cache: &mut PathCache,
    from: TileCoord,
    to: TileCoord,
    cost: ShipPathCost,
) -> Option<Vec<TileCoord>> {
    if let Some(path) = cache.get_ship(from, to, cost) {
        return Some(path.clone());
    }
    let path = find_ship_path_with_cost(map, from, to, cost)?;
    cache.insert_ship(from, to, cost, path.clone());
    Some(path)
}

/// Variante cacheada que separa también la orientación física de origen.
#[must_use]
pub fn find_ship_path_cached_with_trackdir(
    map: &Map,
    cache: &mut PathCache,
    from: TileCoord,
    to: TileCoord,
    cost: ShipPathCost,
    origin_trackdir: u8,
) -> Option<Vec<TileCoord>> {
    if let Some(path) = cache.get_ship_with_trackdir(from, to, cost, Some(origin_trackdir)) {
        return Some(path.clone());
    }
    let path = find_ship_path_with_cost_and_trackdir(map, from, to, cost, origin_trackdir)?;
    cache.insert_ship_with_trackdir(from, to, cost, Some(origin_trackdir), path.clone());
    Some(path)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::{ENGINE_SHIP_MPS, ENGINE_TRAIN_KIRBY, NEWGRF_ENGINE_ID_BASE, engine_by_id};
    use crate::map::{RAIL_TB_X, RAIL_TB_Y, TileKind, WaterClass, make_water_tile};
    use crate::rail_type::{RailType, set_rail_type_on_tile};
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

    fn write_water(m: &mut Map, c: TileCoord) {
        m.set_kind(c, TileKind::Water).unwrap();
    }

    fn write_rail(m: &mut Map, c: TileCoord, trackbits: u8) {
        m.set_kind(c, TileKind::Rail).unwrap();
        let mut t = m.get(c).unwrap();
        t.m5 = trackbits & 0x3F;
        m.set_tile(c, t).unwrap();
    }

    fn write_tunnel(m: &mut Map, c: TileCoord, kind: TileKind, m5: u8) {
        m.set_kind(c, kind).unwrap();
        m.set_mapt_m5(c, 0x90, m5).unwrap();
    }

    fn write_bridge(m: &mut Map, c: TileCoord, kind: TileKind, m5: u8) {
        m.set_kind(c, kind).unwrap();
        m.set_mapt_m5(c, 0x90, 0x80 | m5).unwrap();
    }

    fn write_aqueduct_ramp(m: &mut Map, c: TileCoord, direction: u8) {
        m.set_kind(c, TileKind::Water).unwrap();
        m.set_mapt_m5(c, 0x90, 0x80 | (2 << 2) | (direction & 0x03))
            .unwrap();
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
    fn astar_jumps_vanilla_road_tunnel_without_surface_tiles() {
        let mut map = Map::new_flat(8, 3, 0);
        write_road(&mut map, TileCoord::new(0, 1), 0x0A);
        write_tunnel(
            &mut map,
            TileCoord::new(1, 1),
            TileKind::RoadTunnel,
            0x04 | 0x02,
        );
        write_tunnel(&mut map, TileCoord::new(5, 1), TileKind::RoadTunnel, 0x04);
        write_road(&mut map, TileCoord::new(6, 1), 0x0A);

        let path = find_path(
            &map,
            TileCoord::new(0, 1),
            TileCoord::new(6, 1),
            PathNetwork::Road,
        )
        .expect("A* debe saltar entre las dos bocas vanilla");
        assert_eq!(
            path,
            vec![
                TileCoord::new(1, 1),
                TileCoord::new(5, 1),
                TileCoord::new(6, 1)
            ]
        );

        write_road(&mut map, TileCoord::new(1, 0), 0x0F);
        assert!(
            find_path(
                &map,
                TileCoord::new(1, 0),
                TileCoord::new(6, 1),
                PathNetwork::Road,
            )
            .is_none(),
            "la boca no debe aceptar una entrada lateral"
        );
    }

    #[test]
    fn astar_road_bridge_accepts_only_its_outer_ramp_side() {
        let mut map = Map::new_flat(8, 3, 0);
        write_road(&mut map, TileCoord::new(0, 1), 0x0A);
        write_bridge(
            &mut map,
            TileCoord::new(1, 1),
            TileKind::RoadBridge,
            0x04 | 0x02,
        );
        write_bridge(&mut map, TileCoord::new(5, 1), TileKind::RoadBridge, 0x04);
        write_road(&mut map, TileCoord::new(6, 1), 0x0A);

        assert_eq!(
            find_path(
                &map,
                TileCoord::new(0, 1),
                TileCoord::new(6, 1),
                PathNetwork::Road,
            )
            .expect("A* debe saltar entre rampas del puente"),
            vec![
                TileCoord::new(1, 1),
                TileCoord::new(5, 1),
                TileCoord::new(6, 1)
            ]
        );

        write_road(&mut map, TileCoord::new(1, 0), 0x0F);
        assert!(
            find_path(
                &map,
                TileCoord::new(1, 0),
                TileCoord::new(6, 1),
                PathNetwork::Road,
            )
            .is_none(),
            "la rampa del puente no debe aceptar una entrada lateral"
        );
    }

    #[test]
    fn yapf_jumps_vanilla_rail_tunnel_without_surface_tiles() {
        let mut map = Map::new_flat(8, 3, 0);
        write_rail(&mut map, TileCoord::new(0, 1), RAIL_TB_X);
        write_tunnel(&mut map, TileCoord::new(1, 1), TileKind::RailTunnel, 0x02);
        write_tunnel(&mut map, TileCoord::new(5, 1), TileKind::RailTunnel, 0);
        write_rail(&mut map, TileCoord::new(6, 1), RAIL_TB_X);

        let path = find_path(
            &map,
            TileCoord::new(0, 1),
            TileCoord::new(6, 1),
            PathNetwork::Rail,
        )
        .expect("YAPF debe saltar entre las dos bocas vanilla");
        assert_eq!(
            path,
            vec![
                TileCoord::new(1, 1),
                TileCoord::new(5, 1),
                TileCoord::new(6, 1)
            ]
        );
    }

    #[test]
    fn benchmark_fixture_paths_keep_lengths_and_hot_cache_result() {
        use crate::parity::{
            TRAIN_LINE_DEPOT, TRAIN_LINE_STATION_A, TRAIN_LINE_STATION_B, TRUCK_BAY_DELIVER_ROAD,
            TRUCK_BAY_LOAD_ROAD, build_train_line, build_truck_bay,
        };

        let truck = build_truck_bay();
        let road = find_path(
            &truck.map,
            TRUCK_BAY_LOAD_ROAD,
            TRUCK_BAY_DELIVER_ROAD,
            PathNetwork::Road,
        )
        .expect("ruta road de la fixture de benchmark");
        assert_eq!(road.len(), 18);

        let mut cache = PathCache::default();
        cache.begin_tick(1);
        let miss = find_path_cached(
            &truck.map,
            &mut cache,
            TRUCK_BAY_LOAD_ROAD,
            TRUCK_BAY_DELIVER_ROAD,
            PathNetwork::Road,
            None,
        )
        .expect("miss inicial de la fixture hot");
        let hit = find_path_cached(
            &truck.map,
            &mut cache,
            TRUCK_BAY_LOAD_ROAD,
            TRUCK_BAY_DELIVER_ROAD,
            PathNetwork::Road,
            None,
        )
        .expect("hit de la fixture hot");
        assert_eq!(miss, road);
        assert_eq!(hit, road);

        let train = build_train_line();
        let depot_to_a = find_path(
            &train.map,
            TRAIN_LINE_DEPOT,
            TRAIN_LINE_STATION_A,
            PathNetwork::Rail,
        )
        .expect("ruta depósito a estación A de la fixture");
        assert_eq!(depot_to_a.len(), 4);
        let a_to_b = find_path(
            &train.map,
            TRAIN_LINE_STATION_A,
            TRAIN_LINE_STATION_B,
            PathNetwork::Rail,
        )
        .expect("ruta estación A a B de la fixture");
        assert_eq!(a_to_b.len(), 15);
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
    fn ship_path_jumps_aqueduct_and_rejects_inner_ramp_side() {
        let mut map = Map::new_flat(8, 3, 0);
        write_water(&mut map, TileCoord::new(0, 2));
        write_aqueduct_ramp(&mut map, TileCoord::new(1, 2), 2);
        write_aqueduct_ramp(&mut map, TileCoord::new(5, 2), 0);
        write_water(&mut map, TileCoord::new(6, 2));

        let path = find_path(
            &map,
            TileCoord::new(0, 2),
            TileCoord::new(6, 2),
            PathNetwork::Water,
        )
        .expect("el barco debe cruzar el acueducto por sus rampas");
        assert_eq!(
            path,
            vec![
                TileCoord::new(1, 2),
                TileCoord::new(5, 2),
                TileCoord::new(6, 2)
            ]
        );
        assert_eq!(
            crate::bridge_middle_length(TileCoord::new(1, 2), TileCoord::new(5, 2)),
            3
        );
        assert_eq!(
            ShipPathCost::default().path_cost(&map, TileCoord::new(0, 2), &path),
            600,
            "el salto debe cobrar las tres teselas omitidas"
        );
        assert!(
            find_path(
                &map,
                TileCoord::new(1, 1),
                TileCoord::new(6, 2),
                PathNetwork::Water,
            )
            .is_none(),
            "una rampa no debe aceptar una entrada lateral"
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
    fn catalog_rail_path_uses_custom_required_rail_type() {
        let mut map = Map::new_flat(8, 1, 0);
        for x in 0..=4_i32 {
            let tile = TileCoord::new(x, 0);
            write_rail(&mut map, tile, RAIL_TB_X);
            if (1..4).contains(&x) {
                let typed = set_rail_type_on_tile(map.get(tile).unwrap(), RailType::Maglev);
                map.set_tile(tile, typed).unwrap();
            }
        }

        let mut custom = engine_by_id(ENGINE_TRAIN_KIRBY)
            .expect("el catálogo vanilla debe contener Kirby")
            .clone();
        custom.id = NEWGRF_ENGINE_ID_BASE + 61;
        custom.required_rail_type = Some(RailType::Maglev.as_u8());
        custom.from_newgrf = true;
        let catalog = [custom];
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(4, 0);

        assert!(
            find_rail_path_for_engine(&map, from, to, None, Some(catalog[0].id)).is_none(),
            "el wrapper legacy debe mantener el fallback Rail para IDs desconocidos"
        );
        assert!(
            find_rail_path_for_engine_with_catalog(
                &map,
                from,
                to,
                None,
                Some(catalog[0].id),
                &catalog,
            )
            .is_some(),
            "el motor NewGRF debe poder seguir su corredor Maglev"
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

    #[test]
    fn ship_yapf_cost_prefers_sea_detour_over_slow_canal() {
        let mut map = Map::new_flat(7, 5, 0);
        for y in [1_i32, 2, 3] {
            for x in 0..=6_i32 {
                make_water_tile(&mut map, TileCoord::new(x, y), WaterClass::Sea).expect("agua");
            }
        }
        for x in 1..=5_i32 {
            make_water_tile(&mut map, TileCoord::new(x, 2), WaterClass::Canal).expect("canal");
        }

        let cost = ShipPathCost {
            ocean_speed_frac: 0,
            canal_speed_frac: 192,
            max_speed: 0,
            ..ShipPathCost::default()
        };
        assert_eq!(cost.tile_cost(Some(WaterClass::Sea)), 100);
        assert_eq!(cost.tile_cost(Some(WaterClass::Canal)), 400);

        let mut cache = PathCache::default();
        cache.begin_tick(1);
        let direct = find_ship_path_cached(
            &map,
            &mut cache,
            TileCoord::new(0, 2),
            TileCoord::new(6, 2),
            ShipPathCost::default(),
        )
        .expect("ruta directa");
        assert_eq!(direct.len(), 6);

        let weighted = find_ship_path_cached(
            &map,
            &mut cache,
            TileCoord::new(0, 2),
            TileCoord::new(6, 2),
            cost,
        )
        .expect("desvío por mar");
        assert_eq!(weighted.len(), 8);
        assert!(
            weighted
                .iter()
                .all(|tile| tile.y != 2 || tile.x == 0 || tile.x == 6)
        );

        let mut lock_map = Map::new_flat(7, 5, 0);
        for y in [1_i32, 2, 3] {
            for x in 0..=6_i32 {
                make_water_tile(&mut lock_map, TileCoord::new(x, y), WaterClass::Sea)
                    .expect("agua");
            }
        }
        let lock_tile = TileCoord::new(3, 2);
        let mut raw_lock = lock_map.get(lock_tile).expect("esclusa");
        raw_lock.m5 = 0x20; // WaterTileType::Lock + LockPart::Middle.
        lock_map.set_tile(lock_tile, raw_lock).expect("esclusa raw");
        let lock_cost = ShipPathCost {
            ocean_speed_frac: 0,
            canal_speed_frac: 0,
            max_speed: 128,
            ..ShipPathCost::default()
        };
        let around_lock = find_ship_path_with_cost(
            &lock_map,
            TileCoord::new(0, 2),
            TileCoord::new(6, 2),
            lock_cost,
        )
        .expect("desvío de esclusa");
        assert_eq!(around_lock.len(), 8);
        assert!(!around_lock.contains(&lock_tile));
        assert_eq!(
            lock_cost.path_cost(
                &lock_map,
                TileCoord::new(0, 2),
                &[TileCoord::new(1, 2), TileCoord::new(2, 2), lock_tile]
            ),
            1_100,
            "el centro añade 800 a tres pasos de mar"
        );

        let mut altered_curve_cost = ShipPathCost::default();
        altered_curve_cost.curve45_penalty = 0;
        assert!(
            cache
                .get_ship(
                    TileCoord::new(0, 2),
                    TileCoord::new(6, 2),
                    altered_curve_cost,
                )
                .is_none(),
            "la caché naval debe separar penalizaciones de curva"
        );
    }

    #[test]
    fn ship_path_cache_separates_physical_origin_trackdir() {
        let mut cache = PathCache::default();
        cache.begin_tick(1);
        let from = TileCoord::new(2, 2);
        let to = TileCoord::new(4, 2);
        let path = vec![TileCoord::new(3, 2), to];
        let cost = ShipPathCost::default();

        cache.insert_ship_with_trackdir(from, to, cost, Some(8), path.clone());
        assert_eq!(
            cache.get_ship_with_trackdir(from, to, cost, Some(8)),
            Some(&path)
        );
        assert!(
            cache
                .get_ship_with_trackdir(from, to, cost, Some(0))
                .is_none()
        );
        assert!(cache.get_ship(from, to, cost).is_none());
    }

    #[test]
    fn ship_yapf_profile_uses_game_curve_settings() {
        let engine = engine_by_id(ENGINE_SHIP_MPS).expect("motor naval vanilla");
        let settings = crate::PathfindingSettings {
            ship_curve45_penalty: 12,
            ship_curve90_penalty: 34,
            ..crate::PathfindingSettings::default()
        };
        let cost = ShipPathCost::from_engine_with_settings(engine, &settings);
        assert_eq!(cost.curve45_penalty, 12);
        assert_eq!(cost.curve90_penalty, 34);

        let capped = crate::PathfindingSettings {
            ship_curve45_penalty: u32::MAX,
            ship_curve90_penalty: u32::MAX,
            ..settings
        };
        let cost = ShipPathCost::from_engine_with_settings(engine, &capped);
        assert_eq!(cost.curve45_penalty, crate::MAX_SHIP_CURVE_PENALTY);
        assert_eq!(cost.curve90_penalty, crate::MAX_SHIP_CURVE_PENALTY);
    }

    #[test]
    fn ship_yapf_cost_keeps_curve_trackdir_state() {
        let mut map = Map::new_flat(4, 4, 0);
        for y in 0..4_i32 {
            for x in 0..4_i32 {
                make_water_tile(&mut map, TileCoord::new(x, y), WaterClass::Sea).expect("agua");
            }
        }

        let from = TileCoord::new(0, 1);
        let path = [TileCoord::new(1, 1), TileCoord::new(1, 2)];
        assert_eq!(
            ShipPathCost::default().path_cost(&map, from, &path),
            571,
            "la curva y la dirección no preferida deben conservar el coste YAPF"
        );
        assert_eq!(
            ShipPathCost::default().path_cost(&map, TileCoord::new(0, 2), &[TileCoord::new(1, 2)],),
            100,
            "un tramo recto no debe añadir penalización de curva"
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

    #[test]
    fn road_stop_connectivity_uses_station_m5_not_m3() {
        let mut map = Map::new_flat(8, 8, 0);
        write_road(&mut map, TileCoord::new(1, 2), 0x0A);
        write_road(&mut map, TileCoord::new(2, 2), 0x0A);
        let stop = TileCoord::new(3, 2);
        map.set_kind(stop, TileKind::Station).unwrap();
        let mut tile = map.get(stop).unwrap();
        tile.mapt = 0x50; // MP_STATION.
        tile.m5 = 0; // boca hacia el oeste, conectada con (2, 2).
        tile.m6 = 3 << 3; // parada de bus.
        tile.m3 = 0; // m3 no contiene la topología de la parada nativa.
        map.set_tile(stop, tile).unwrap();

        assert!(find_path(&map, TileCoord::new(1, 2), stop, PathNetwork::Road).is_some());

        let mut tile = map.get(stop).unwrap();
        tile.m5 = 1; // boca hacia el sur, sin carretera en esa dirección.
        map.set_tile(stop, tile).unwrap();
        assert!(find_path(&map, TileCoord::new(1, 2), stop, PathNetwork::Road).is_none());
    }
}
