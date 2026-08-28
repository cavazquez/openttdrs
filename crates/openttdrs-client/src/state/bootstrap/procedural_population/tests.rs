use super::should_populate_procedurally;
use crate::state::bootstrap::{NewGameSettings, PopulationDensity, build_procedural_demo_world};
use openttdrs_core::Climate;
use openttdrs_core::prelude::*;

#[test]
fn skips_population_on_compact_demo() {
    assert!(!should_populate_procedurally(&NewGameSettings::default()));
}

#[test]
fn populates_large_procedural_island() {
    let settings = NewGameSettings::procedural_island(Climate::Temperate, 99);
    assert!(should_populate_procedurally(&settings));
    let state = build_procedural_demo_world(&settings);
    assert!(!state.towns.is_empty(), "debe haber al menos un pueblo");
    assert!(
        !state.industries.is_empty(),
        "debe haber al menos una industria"
    );
}

#[test]
fn dense_population_places_more_towns_than_sparse() {
    let base = NewGameSettings::procedural_island(Climate::Temperate, 1234);
    let sparse = build_procedural_demo_world(&NewGameSettings {
        town_density: PopulationDensity::Sparse,
        industry_density: PopulationDensity::Sparse,
        ..base
    });
    let dense = build_procedural_demo_world(&NewGameSettings {
        town_density: PopulationDensity::Dense,
        industry_density: PopulationDensity::Dense,
        ..base
    });
    assert!(dense.towns.len() >= sparse.towns.len());
    assert!(dense.industries.len() >= sparse.industries.len());
}

#[test]
fn procedural_houses_keep_native_town_bytes_with_valid_specs() {
    let settings = NewGameSettings::procedural_island(Climate::Temperate, 555);
    let state = build_procedural_demo_world(&settings);
    let (mw, mh) = state.map.dimensions();
    let mut house_tiles = Vec::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if state.map.get_kind(c) == Some(TileKind::House)
                && let Some(tile) = state.map.get(c)
            {
                house_tiles.push(tile);
            }
        }
    }
    assert!(house_tiles.len() >= 3);
    assert!(
        house_tiles.iter().all(|t| t.mapt & 0xF0 == 0x30),
        "cada casa debe conservar el TileType MP_HOUSE"
    );
    assert!(
        house_tiles.iter().all(|t| t.m3 & 0x80 == 0 || t.m5 == 0),
        "una casa terminada debe reiniciar MAP5; las demás conservan su obra"
    );
    let distinct_ids: std::collections::HashSet<u16> =
        house_tiles.iter().map(|t| t.m8 & 0x0FFF).collect();
    assert!(distinct_ids.len() > 1, "debe haber más de un HouseID");
    assert!(
        distinct_ids
            .iter()
            .all(|&id| openttdrs_core::HouseSpec::get(id).is_some()),
        "cada HouseID procedural debe pertenecer al catálogo vanilla; ids={distinct_ids:?}"
    );
}

#[test]
fn procedural_town_population_matches_house_specs() {
    let settings = NewGameSettings::procedural_island(Climate::Temperate, 777);
    let state = build_procedural_demo_world(&settings);
    assert!(!state.towns.is_empty());
    let (mw, mh) = state.map.dimensions();
    let mut pop_from_houses = 0_u32;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if state.map.get_kind(c) != Some(TileKind::House) {
                continue;
            }
            let Some(tile) = state.map.get(c) else {
                continue;
            };
            if tile.m3 & 0x80 == 0 {
                continue;
            }
            pop_from_houses = pop_from_houses.saturating_add(u32::from(
                openttdrs_core::house_spec_population(tile.m8 & 0x0FFF),
            ));
        }
    }
    let labeled: u32 = state.towns.iter().map(|t| t.population).sum();
    assert_eq!(
        labeled, pop_from_houses,
        "población de pueblos debe ser suma HouseSpec, no casas×8"
    );
    assert!(
        labeled > 8 * 3,
        "con houses reales la pop debe superar el viejo casas×8 mínimo; pop={labeled}"
    );
}

#[test]
fn procedural_towns_place_houses_and_roads() {
    let settings = NewGameSettings::procedural_island(Climate::Temperate, 888);
    let state = build_procedural_demo_world(&settings);
    let (mw, mh) = state.map.dimensions();
    let mut houses = Vec::new();
    let mut road_tiles = 0_u32;
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            match state.map.get_kind(c) {
                Some(TileKind::House) => houses.push(c),
                Some(TileKind::Road) => road_tiles += 1,
                _ => {}
            }
        }
    }
    assert!(road_tiles > 0, "debe haber calles en pueblos procedurales");
    assert!(!houses.is_empty());
    assert!(
        houses.iter().any(|&c| {
            [(0, 1), (0, -1), (1, 0), (-1, 0)]
                .into_iter()
                .map(|(dx, dy)| TileCoord::new(c.x + dx, c.y + dy))
                .any(|neighbour| state.map.get_kind(neighbour) == Some(TileKind::Road))
        }),
        "al menos una parcela residencial debe quedar junto a una calle"
    );
}

#[test]
fn procedural_town_roads_keep_native_bytes_on_slopes() {
    let settings = NewGameSettings::procedural_island(Climate::Temperate, 12345);
    let state = build_procedural_demo_world(&settings);
    let (mw, mh) = state.map.dimensions();
    let mut road_tiles = Vec::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if state.map.get_kind(c) == Some(TileKind::Road) {
                road_tiles.push(c);
            }
        }
    }
    assert!(!road_tiles.is_empty());
    assert!(
        road_tiles.iter().all(|&c| {
            state.map.get(c).is_some_and(|tile| {
                tile.mapt & 0xF0 == 0x20
                    && tile.m1 == openttdrs_core::company::OWNER_TOWN_M1
                    && tile.m5 & 0x0F != 0
            }) && openttdrs_core::tile_slope_and_z(&state.map, c).is_some()
        }),
        "cada calle procedural debe conservar los bytes municipales válidos"
    );
    assert!(
        road_tiles.iter().any(|&c| {
            openttdrs_core::tile_slope_and_z(&state.map, c).is_some_and(|(slope, _)| slope != 0)
        }),
        "la fixture debe cubrir una calle municipal inclinada permitida por OpenTTD"
    );
}

#[test]
fn populate_same_seed_is_deterministic() {
    let settings = NewGameSettings::procedural_island(Climate::Temperate, 4242);
    let a = build_procedural_demo_world(&settings);
    let b = build_procedural_demo_world(&settings);
    assert_eq!(a.towns.len(), b.towns.len());
    assert_eq!(a.industries.len(), b.industries.len());
    for (ta, tb) in a.towns.iter().zip(b.towns.iter()) {
        assert_eq!(ta.pos, tb.pos);
        assert_eq!(ta.name, tb.name);
    }
}
