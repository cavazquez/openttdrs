//! Tipos y helpers para construir la capa visual del mapa.

mod action5_newgrf;
mod airport_radar_anim;
mod animation_gate;
mod assets;
mod atlas;
mod bubble;
mod catenary_newgrf;
mod company_recolor;
mod components;
mod disaster_craft;
pub(crate) mod effect_fx;
mod effect_vehicle;
mod fizzy_drink;
mod grid;
mod house_lift_anim;
pub(crate) mod house_newgrf;
mod house_viewport_sort;
mod industry_anim;
mod industry_draw_proc;
pub(crate) mod industry_newgrf;
mod label_spatial_index;
mod lighthouse_anim;
pub(crate) mod newgrf_cache;
pub(crate) mod object_newgrf;
mod refinery_fire;
mod road_newgrf;
mod shore_newgrf;
mod sign_labels;
mod signal_newgrf;
mod smoke;
mod station_labels;
mod station_newgrf;
mod tile_anims;
mod tiles;
mod town_labels;
mod train_smoke;
mod vehicles;
mod viewport;
// La captura de bounds/children de todos los spawners se integra por etapas;
// el port puro ya se prueba contra la semántica de `ViewportSortParentSprites`.
#[allow(dead_code)]
pub(crate) mod viewport_sort;
mod water;
mod world;
pub(crate) mod world_draw_trace;

pub(crate) use airport_radar_anim::{AirportRadarAnim, AirportRadarAnimPlugin};
pub(crate) use animation_gate::palette_animations_should_run;
pub(crate) use assets::{OverviewRenderAssets, WorldAssets};
pub(crate) use atlas::{AtlasSprite, TileAtlas};
pub(crate) use bubble::{BubbleEffectPlugin, BubbleSpawnQueue};
pub(crate) use company_recolor::{
    CompanyColoredSprites, sprite_from_atlas_or_company_colour,
    sprite_from_atlas_or_company_white_colour, sprite_from_atlas_or_industry_palette,
    sprite_from_company_or_asset,
};
pub(crate) use components::{
    FizzyDrinkAnimFrames, IndustryPreviewCamera, LighthouseAnimFrames, MapLabelLod, MapLabelText,
    MapPreviewCamera, MapSpriteBatches, MapTileChunk, MapVisualLayer, PrimaryGameCamera,
    RefineryFireAnimFrames, ShoreTile, WaterAnimFrames, WaterAtlasAnimation, WaterTile,
};
pub(crate) use disaster_craft::DisasterCraftPlugin;
pub(crate) use effect_fx::EffectVehiclePlugin;
pub(crate) use effect_vehicle::EffectVehicleFrames;
pub(crate) use fizzy_drink::{FizzyDrinkAnim, FizzyDrinkAnimPlugin};
pub(crate) use grid::{RenderGrid, TileRenderContext};
pub(crate) use house_lift_anim::{
    HOUSE_LIFT_SCREEN_X, HOUSE_LIFT_SCREEN_Y, HouseLiftAnim, HouseLiftAnimPlugin,
    house_sprite_has_lift,
};
pub(crate) use house_viewport_sort::{
    EMPTY_BOUNDING_BOX_SPRITE_ID, ViewportSortableChild, ViewportSortableChildDepthWindows,
    ViewportSortableParent, sort_viewport_sortable_parents, sync_viewport_sortable_children,
    viewport_insertion_key, viewport_source_depth,
};
pub(crate) use industry_anim::{
    IndustryBuildingAnim, IndustryBuildingAnimPlugin, IndustryOverlayContext, industry_anim_phase,
    spawn_industry_anim_layer,
};
pub(crate) use industry_draw_proc::{IndustryDrawProcPlugin, spawn_industry_draw_proc_overlays};
pub(crate) use label_spatial_index::{MapLabelCandidates, MapLabelSpatialIndex};
pub(crate) use lighthouse_anim::{LighthouseAnim, LighthouseAnimPlugin};
pub(crate) use refinery_fire::{RefineryFireAnim, RefineryFireAnimPlugin};
pub(crate) use sign_labels::SignLabel;
pub(crate) use smoke::{
    ChimneySmokeFrames, CopperMineSmokeFrames, GFX_COPPER_MINE_CHIMNEY, GFX_POWERPLANT_CHIMNEY,
    IndustrySmokePlugin, spawn_chimney_smoke, spawn_copper_mine_smoke,
};
pub(crate) use station_labels::StationLabel;
pub(crate) use tile_anims::TileAnimPlugin;
pub(crate) use town_labels::{TownLabel, town_id_at_label_pos};
pub(crate) use train_smoke::TrainSmokePlugin;

pub(crate) use action5_newgrf::NewGrfAction5SpriteCache;
pub(crate) use catenary_newgrf::NewGrfCatenarySpriteCache;
pub(crate) use house_newgrf::NewGrfHouseSpriteCache;
pub(crate) use industry_newgrf::NewGrfIndustrySpriteCache;
pub(crate) use object_newgrf::NewGrfObjectSpriteCache;
pub(crate) use road_newgrf::NewGrfRoadSpriteCache;
pub(crate) use shore_newgrf::NewGrfShoreSpriteCache;
pub(crate) use signal_newgrf::NewGrfSignalSpriteCache;
pub(crate) use station_newgrf::NewGrfStationSpriteCache;
pub(crate) use tiles::{
    HouseSpawnResources, flush_map_batches, leveled_foundation_overlay_pos, push_forest_tree,
    push_water_tile, spawn_bridge_middle, spawn_generic_land_tile, spawn_house_tile,
    spawn_industry_tile, spawn_rail_tile, spawn_road_tile, spawn_station_tile_with_world,
    spawn_transport_object_tile, spawn_void_tile,
};
pub(crate) use vehicles::{
    AircraftRotorSprite, AircraftShadowSprite, ConsistUnitSprite, NewGrfTrainSpriteCache,
    TruckHandles, VehicleCargoLabel, VehicleIndex, VehicleRenderPlugin, VehicleSprite,
    pick_vehicle_id_at_world, vehicle_sprite_pos_at, vehicle_world_position,
};
pub(crate) use viewport::{
    ABSOLUTE_MAX_ORTHO_SCALE, MIN_ORTHO_SCALE, TileViewportBounds, chunk_tile_bounds,
    chunks_in_bounds, clamp_ortho_scale, large_map_viewport_cull_enabled,
};
pub(crate) use water::WaterAnimationPlugin;
pub(crate) use water::water_anim_frames_from_assets;
pub(crate) use world::{
    LoadedMapTileChunks, MapTileSpawnViewport, RemapMapVisualsPending, WorldRenderPlugin,
    initial_map_camera_pose, request_map_visual_remap, request_map_visual_remap_with_labels,
    spawn_intro_map_render,
};
