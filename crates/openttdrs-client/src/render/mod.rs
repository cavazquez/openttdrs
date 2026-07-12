//! Tipos y helpers para construir la capa visual del mapa.

mod animation_gate;
mod assets;
mod atlas;
mod company_recolor;
mod components;
pub(crate) mod effect_fx;
mod effect_vehicle;
mod fizzy_drink;
mod grid;
mod industry_anim;
mod industry_draw_proc;
mod lighthouse_anim;
mod refinery_fire;
mod sign_labels;
mod smoke;
mod station_labels;
mod tile_anims;
mod tiles;
mod town_labels;
mod train_smoke;
mod vehicles;
mod viewport;
mod water;
mod world;

pub(crate) use animation_gate::palette_animations_should_run;
pub(crate) use assets::WorldAssets;
pub(crate) use atlas::{AtlasSprite, TileAtlas};
pub(crate) use company_recolor::{
    CompanyColoredSprites, sprite_from_atlas_or_company_white,
    sprite_from_atlas_or_industry_palette, sprite_from_company_or_asset,
};
pub(crate) use components::{
    FizzyDrinkAnimFrames, IndustryPreviewCamera, LighthouseAnimFrames, MapPreviewCamera,
    MapSpriteBatches, MapTileChunk, MapVisualLayer, PrimaryGameCamera, RefineryFireAnimFrames,
    ShoreTile, WaterAnimFrames, WaterTile,
};
pub(crate) use effect_fx::EffectVehiclePlugin;
pub(crate) use effect_vehicle::EffectVehicleFrames;
pub(crate) use fizzy_drink::{FizzyDrinkAnim, FizzyDrinkAnimPlugin};
pub(crate) use grid::{RenderGrid, TileRenderContext};
pub(crate) use industry_anim::{
    IndustryBuildingAnim, IndustryBuildingAnimPlugin, IndustryOverlayContext, industry_anim_phase,
    spawn_industry_anim_layer,
};
pub(crate) use industry_draw_proc::{IndustryDrawProcPlugin, spawn_industry_draw_proc_overlays};
pub(crate) use lighthouse_anim::{LighthouseAnim, LighthouseAnimPlugin};
pub(crate) use refinery_fire::{RefineryFireAnim, RefineryFireAnimPlugin};
pub(crate) use smoke::{
    ChimneySmokeFrames, CopperMineSmokeFrames, GFX_COPPER_MINE_CHIMNEY, GFX_POWERPLANT_CHIMNEY,
    IndustrySmokePlugin, spawn_chimney_smoke, spawn_copper_mine_smoke,
};
pub(crate) use tile_anims::TileAnimPlugin;
pub(crate) use town_labels::town_id_at_label_pos;
pub(crate) use train_smoke::TrainSmokePlugin;

pub(crate) use tiles::{
    flush_map_batches, leveled_foundation_overlay_pos, push_forest_tree, push_water_tile,
    spawn_bridge_middle, spawn_generic_land_tile, spawn_house_tile, spawn_industry_tile,
    spawn_rail_tile, spawn_road_tile, spawn_station_tile, spawn_transport_object_tile,
};
pub(crate) use vehicles::{
    NewGrfTrainSpriteCache, TruckHandles, VehicleIndex, VehicleRenderPlugin,
    pick_vehicle_id_at_world, vehicle_sprite_pos_at, vehicle_world_position,
};
pub(crate) use viewport::{
    TileViewportBounds, chunk_tile_bounds, chunks_in_bounds, large_map_viewport_cull_enabled,
    ortho_visible_tile_bounds,
};
pub(crate) use water::WaterAnimationPlugin;
pub(crate) use world::{
    LoadedMapTileChunks, MapTileSpawnViewport, RemapMapVisualsPending, WorldRenderPlugin,
    initial_map_camera_pose, request_map_visual_remap, spawn_intro_map_render,
};
