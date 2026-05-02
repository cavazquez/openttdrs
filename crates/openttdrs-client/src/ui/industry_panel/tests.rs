#![allow(clippy::unwrap_used, clippy::expect_used)]

use bevy::asset::AssetPlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use openttdrs_core::{IndustryKind, Map, TileCoord, TileKind};

use crate::state::SimWorld;

use super::logic::{
    flood_industry_tiles, format_panel_title, industry_stats_for_component, kind_label, spec_label,
};
use super::{
    IndustryPanelState, industry_panel_close_interaction, setup_industry_panel, sync_industry_panel,
};

#[test]
fn setup_industry_panel_runs() {
    let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
        file_path: asset_root.into(),
        ..default()
    });
    app.world_mut().init_resource::<Assets<Image>>();
    app.init_asset::<Font>();
    app.world_mut()
        .run_system_once(setup_industry_panel)
        .unwrap();
}

#[test]
fn industry_panel_close_no_entities_is_noop() {
    let mut world = World::new();
    world.insert_resource(IndustryPanelState::default());
    world
        .run_system_once(industry_panel_close_interaction)
        .unwrap();
}

#[test]
fn sync_industry_panel_closed_is_noop() {
    let mut world = World::new();
    world.insert_resource(IndustryPanelState::default());
    world.insert_resource(SimWorld::default());
    world.run_system_once(sync_industry_panel).unwrap();
}

#[test]
fn industry_helper_functions_cover_paths() {
    let mut map = Map::new_flat(5, 5, 0);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_kind(c(2, 2), TileKind::Industry).unwrap();
    map.set_kind(c(2, 3), TileKind::Industry).unwrap();
    map.set_kind(c(3, 3), TileKind::Industry).unwrap();

    let mut sim = SimWorld::default();
    sim.state.industries.clear();
    sim.state.industries.push(openttdrs_core::Industry {
        pos: c(2, 2),
        tiles: vec![c(2, 2), c(2, 3), c(3, 3)],
        spec: Some(openttdrs_core::IndustrySpec::Forest),
        kind: IndustryKind::Forest,
        stock: 0,
        capacity: 100,
    });

    let tiles = flood_industry_tiles(&map, c(2, 2));
    assert!(tiles.len() >= 3);
    assert!(flood_industry_tiles(&map, c(0, 0)).is_empty());
    let stats = industry_stats_for_component(&map, &sim, c(2, 2)).unwrap();
    assert_eq!(stats.0, IndustryKind::Forest);
    assert_eq!(stats.1, Some(openttdrs_core::IndustrySpec::Forest));
    assert_eq!(stats.2, 0);
    assert_eq!(stats.3, 100);
    assert!(spec_label(openttdrs_core::IndustrySpec::OilRefinery).contains("Refinería"));
    assert_eq!(kind_label(IndustryKind::CoalMine), "Carbon");
    assert!(format_panel_title(&map, &sim, c(2, 2)).contains("Industria"));
}

#[test]
fn flood_industry_tiles_respects_m1_components_when_present() {
    let mut map = Map::new_flat(3, 1, 0);
    let c = |x: i32| TileCoord::new(x, 0);
    let mut t0 = map.get(c(0)).expect("tile 0");
    t0.kind = TileKind::Industry;
    t0.m1 = 5;
    let mut t1 = map.get(c(1)).expect("tile 1");
    t1.kind = TileKind::Industry;
    t1.m1 = 6;
    let _ = map.set_tile(c(0), t0);
    let _ = map.set_tile(c(1), t1);

    let from_left = flood_industry_tiles(&map, c(0));
    let from_right = flood_industry_tiles(&map, c(1));
    assert_eq!(from_left.len(), 1);
    assert_eq!(from_right.len(), 1);
}

#[test]
fn flood_industry_tiles_respects_gfx_group_when_m1_matches() {
    let mut map = Map::new_flat(2, 1, 0);
    let c0 = TileCoord::new(0, 0);
    let c1 = TileCoord::new(1, 0);
    let mut t0 = map.get(c0).expect("tile 0");
    t0.kind = TileKind::Industry;
    t0.m1 = 7;
    t0.m5 = 18; // Oil Refinery
    let mut t1 = map.get(c1).expect("tile 1");
    t1.kind = TileKind::Industry;
    t1.m1 = 7;
    t1.m5 = 16; // Forest
    let _ = map.set_tile(c0, t0);
    let _ = map.set_tile(c1, t1);

    let from_left = flood_industry_tiles(&map, c0);
    let from_right = flood_industry_tiles(&map, c1);
    assert_eq!(from_left.len(), 1);
    assert_eq!(from_right.len(), 1);
}
