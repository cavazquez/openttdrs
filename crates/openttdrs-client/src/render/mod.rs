//! Tipos y helpers para construir la capa visual del mapa.

mod assets;
mod components;
mod grid;
mod smoke;
mod tiles;
mod town_labels;
mod vehicles;
mod viewport;
mod water;
mod world;

pub(crate) use assets::WorldAssets;
pub(crate) use components::{
    IndustryPreviewCamera, MapPreviewCamera, MapSpriteBatches, MapVisualLayer, PrimaryGameCamera,
    ShoreTile, VehiclePreviewCamera, WaterAnimFrames, WaterTile,
};
pub(crate) use grid::{RenderGrid, TileRenderContext};
pub(crate) use smoke::{
    ChimneySmokeFrames, GFX_POWERPLANT_CHIMNEY, IndustrySmokePlugin, spawn_chimney_smoke,
};
pub(crate) use tiles::{
    flush_map_batches, leveled_foundation_overlay_pos, push_forest_tree, push_water_tile,
    spawn_bridge_middle, spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile,
    spawn_rail_tile, spawn_road_tile, spawn_station_tile, spawn_transport_object_tile,
};
pub(crate) use vehicles::{
    VehicleIndex, VehicleRenderPlugin, pick_vehicle_id_at_world, vehicle_world_position,
};
pub(crate) use viewport::{
    TileViewportBounds, large_map_viewport_cull_enabled, ortho_visible_tile_bounds,
};
pub(crate) use water::WaterAnimationPlugin;
pub(crate) use world::{RemapMapVisualsPending, WorldRenderPlugin};
