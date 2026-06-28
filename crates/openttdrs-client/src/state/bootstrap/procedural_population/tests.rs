use super::should_populate_procedurally;
use super::towns::{house_beside_road, road_tiles_are_flat};
use crate::state::bootstrap::{NewGameSettings, PopulationDensity, build_procedural_demo_world};
use openttdrs_core::{Climate, TileCoord, TileKind};

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
fn procedural_houses_are_completed_with_varied_ids() {
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
    assert!(house_tiles.iter().all(|t| t.m3 & 0x80 != 0));
    let distinct_ids: std::collections::HashSet<u16> = house_tiles.iter().map(|t| t.m8).collect();
    assert!(distinct_ids.len() > 1, "debe haber más de un HouseID");
}

#[test]
fn procedural_towns_place_houses_beside_roads() {
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
        houses.iter().all(|&c| house_beside_road(&state.map, c)),
        "cada casa debe tener calle adyacente"
    );
}

#[test]
fn procedural_town_roads_stay_on_flat_terrain() {
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
        road_tiles_are_flat(&state.map, &road_tiles),
        "todas las calles procedurales deben estar en terreno plano"
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
