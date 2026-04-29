//! Tipos y helpers para construir la capa visual del mapa.

mod assets;
mod components;
mod grid;
mod tiles;
mod vehicles;
mod water;
mod world;

pub(crate) use assets::WorldAssets;
pub(crate) use components::{MapSpriteBatches, MapVisualLayer, WaterTile};
pub(crate) use grid::{RenderGrid, TileRenderContext};
pub(crate) use tiles::{
    flush_map_batches, push_forest_tree, push_water_tile, spawn_generic_land_tile,
    spawn_house_tile, spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
};
pub(crate) use vehicles::{VehicleIndex, VehicleRenderPlugin};
pub(crate) use water::WaterAnimationPlugin;
pub(crate) use world::{RemapMapVisualsPending, WorldRenderPlugin};
