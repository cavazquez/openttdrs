//! Tipos y helpers para construir la capa visual del mapa.

mod assets;
mod components;
mod grid;
mod tiles;
mod water;

pub(crate) use assets::WorldAssets;
pub(crate) use components::{MapSpriteBatches, MapVisualLayer, WaterTile};
pub(crate) use grid::{RenderGrid, TileRenderContext};
pub(crate) use tiles::{
    flush_map_batches, push_forest_tree, push_water_tile, spawn_generic_land_tile,
    spawn_house_tile, spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile,
};
pub(crate) use water::animate_water;
