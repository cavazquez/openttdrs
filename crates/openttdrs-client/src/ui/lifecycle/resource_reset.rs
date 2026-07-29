//! Reset / remove / especiales de recursos de sesión InGame.

use bevy::ecs::component::Mutable;
use bevy::prelude::*;

use crate::audio::PendingSimEvents;
use crate::render::effect_fx::FxSpawnQueue;
use crate::render::{
    ChimneySmokeFrames, CompanyColoredSprites, CopperMineSmokeFrames, EffectVehicleFrames,
    FizzyDrinkAnimFrames, LighthouseAnimFrames, LoadedMapTileChunks, MapTileSpawnViewport,
    NewGrfAction5SpriteCache, NewGrfCatenarySpriteCache, NewGrfObjectSpriteCache,
    NewGrfRoadSpriteCache, NewGrfShoreSpriteCache, NewGrfStationSpriteCache,
    NewGrfTrainSpriteCache, RefineryFireAnimFrames, RemapMapVisualsPending, TileAtlas,
    TruckHandles, VehicleIndex, WaterAnimFrames, WorldAssets,
};
use crate::simulation::SimClock;
use crate::state::{EditorSession, OrderPickState};

use super::super::SaveWindowState;
use super::super::ai_settings_window::AiSettingsWindowState;
use super::super::audio_settings_window::SoundMusicWindowState;
use super::super::autoreplace_window::AutoreplaceWindowState;
use super::super::buy_window::{BuyVehicleWindowState, NewGrfTrainPreviewCache};
use super::super::cargo_dist_settings_window::CargoDistSettingsWindowState;
use super::super::cargo_payment_window::CargoPaymentWindowState;
use super::super::cheat_window::CheatWindowState;
use super::super::destination_window::DestinationPickerState;
use super::super::dev_console::DevConsoleState;
use super::super::endscreen::{EndScreenState, RetireGameRequested};
use super::super::finances_window::FinancesWindowState;
use super::super::genland_window::GenLandWindowState;
use super::super::goal_list_window::GoalListWindowState;
use super::super::graph_window::GraphWindowState;
use super::super::help_window::HelpWindowState;
use super::super::industry_directory::IndustryDirectoryState;
use super::super::industry_panel::IndustryPanelState;
use super::super::league_window::LeagueWindowState;
use super::super::navigation::ToolbarMenuState;
use super::super::newgrf_window::NewGrfWindowState;
use super::super::news_settings_window::NewsSettingsWindowState;
use super::super::pathfinding_settings_window::PathfindingSettingsWindowState;
use super::super::refit_window::RefitWindowState;
use super::super::shared_orders_window::SharedOrdersWindowState;
use super::super::station_directory::StationDirectoryState;
use super::super::statusbar::{NewsHistoryState, NewsUiState};
use super::super::story_window::StoryWindowState;
use super::super::subsidy_list::SubsidyListState;
use super::super::tile_inspector_window::TileInspectorWindowState;
use super::super::timetable_window::TimetableWindowState;
use super::super::toolbar::EditorTownMenuState;
use super::super::toolbar::{
    BridgeBuildState, DepotPanelState, DragBuildState, NewGrfRoadTypePreviewCache,
    NewGrfStationPreviewCache, OrderEditState, RailSignalGhostState, StationBuildState,
    StationCargoPanelState, ToolbarState, UiToolState,
};
use super::super::town_directory::TownDirectoryState;
use super::super::town_window::TownWindowState;
use super::super::vehicle_chain::VehicleChainRegistry;
use super::super::vehicle_details_window::VehicleDetailsWindowState;
use super::super::vehicle_list::VehicleListState;
use super::super::vehicle_window::VehicleWindowState;

/// Entrada inventariable del registro de teardown de recursos.
pub(super) struct ResourceTeardown {
    /// Inventario / tests (`registry_tests`); no se lee en runtime.
    #[allow(dead_code)]
    pub name: &'static str,
    pub apply: fn(&mut World),
}

fn reset_default<T: Resource<Mutability = Mutable> + Default>(world: &mut World) {
    if let Some(mut resource) = world.get_resource_mut::<T>() {
        *resource = T::default();
    }
}

fn remove_res<T: Resource>(world: &mut World) {
    world.remove_resource::<T>();
}

fn unpause_virtual_time(world: &mut World) {
    if let Some(mut virtual_time) = world.get_resource_mut::<Time<Virtual>>() {
        virtual_time.unpause();
    }
}

fn clear_pending_sim_events(world: &mut World) {
    if let Some(mut pending) = world.get_resource_mut::<PendingSimEvents>() {
        pending.0.clear();
    }
}

fn clear_order_edit(world: &mut World) {
    if let Some(mut order_edit) = world.get_resource_mut::<OrderEditState>() {
        order_edit.clear();
    }
}

fn idle_order_pick(world: &mut World) {
    if let Some(mut next_pick) = world.get_resource_mut::<NextState<OrderPickState>>() {
        next_pick.set(OrderPickState::Idle);
    }
}

fn close_save_window(world: &mut World) {
    if let Some(mut save_window) = world.get_resource_mut::<SaveWindowState>() {
        save_window.close();
    }
}

fn clear_train_preview_cache(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfTrainPreviewCache>() {
        *cache = NewGrfTrainPreviewCache::default();
    }
}

fn clear_road_preview_cache(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfRoadTypePreviewCache>() {
        cache.clear();
    }
}

fn clear_station_preview_cache(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfStationPreviewCache>() {
        cache.clear();
    }
}

fn clear_newgrf_train_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfTrainSpriteCache>() {
        cache.clear();
    }
}

fn clear_newgrf_road_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfRoadSpriteCache>() {
        cache.clear();
    }
}

fn clear_newgrf_station_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfStationSpriteCache>() {
        cache.clear();
    }
}

fn clear_newgrf_shore_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfShoreSpriteCache>() {
        cache.clear();
    }
}

fn clear_newgrf_catenary_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfCatenarySpriteCache>() {
        cache.clear();
    }
}

fn clear_newgrf_action5_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfAction5SpriteCache>() {
        cache.clear();
    }
}

fn clear_newgrf_object_sprites(world: &mut World) {
    if let Some(mut cache) = world.get_resource_mut::<NewGrfObjectSpriteCache>() {
        cache.clear();
    }
}

/// Preserva `SuspendedGameSession.editor` si hay suspensión activa; luego inactiva el editor.
fn preserve_suspended_and_reset_editor(world: &mut World) {
    let suspending = world
        .get_resource::<crate::state::SuspendedGameSession>()
        .is_some_and(|s| s.active);
    let editor_active = world
        .get_resource::<EditorSession>()
        .is_some_and(|e| e.active);
    if suspending
        && let Some(mut suspended) = world.get_resource_mut::<crate::state::SuspendedGameSession>()
    {
        suspended.editor = editor_active;
    }
    if let Some(mut editor) = world.get_resource_mut::<EditorSession>() {
        *editor = EditorSession::inactive();
    }
}

/// Resets / clear / especiales (no remove). Orden = teardown histórico.
pub(super) static RESOURCE_RESETS: &[ResourceTeardown] = &[
    ResourceTeardown {
        name: "Time<Virtual>",
        apply: unpause_virtual_time,
    },
    ResourceTeardown {
        name: "VehicleIndex",
        apply: reset_default::<VehicleIndex>,
    },
    ResourceTeardown {
        name: "RemapMapVisualsPending",
        apply: reset_default::<RemapMapVisualsPending>,
    },
    ResourceTeardown {
        name: "SimClock",
        apply: reset_default::<SimClock>,
    },
    ResourceTeardown {
        name: "PendingSimEvents",
        apply: clear_pending_sim_events,
    },
    ResourceTeardown {
        name: "FxSpawnQueue",
        apply: reset_default::<FxSpawnQueue>,
    },
    ResourceTeardown {
        name: "UiToolState",
        apply: reset_default::<UiToolState>,
    },
    ResourceTeardown {
        name: "StationBuildState",
        apply: reset_default::<StationBuildState>,
    },
    ResourceTeardown {
        name: "DragBuildState",
        apply: reset_default::<DragBuildState>,
    },
    ResourceTeardown {
        name: "BridgeBuildState",
        apply: reset_default::<BridgeBuildState>,
    },
    ResourceTeardown {
        name: "OrderEditState",
        apply: clear_order_edit,
    },
    ResourceTeardown {
        name: "NextState<OrderPickState>",
        apply: idle_order_pick,
    },
    ResourceTeardown {
        name: "DepotPanelState",
        apply: reset_default::<DepotPanelState>,
    },
    ResourceTeardown {
        name: "StationCargoPanelState",
        apply: reset_default::<StationCargoPanelState>,
    },
    ResourceTeardown {
        name: "RailSignalGhostState",
        apply: reset_default::<RailSignalGhostState>,
    },
    ResourceTeardown {
        name: "ToolbarState",
        apply: reset_default::<ToolbarState>,
    },
    ResourceTeardown {
        name: "ToolbarMenuState",
        apply: reset_default::<ToolbarMenuState>,
    },
    ResourceTeardown {
        name: "IndustryPanelState",
        apply: reset_default::<IndustryPanelState>,
    },
    ResourceTeardown {
        name: "IndustryDirectoryState",
        apply: reset_default::<IndustryDirectoryState>,
    },
    ResourceTeardown {
        name: "SaveWindowState",
        apply: close_save_window,
    },
    ResourceTeardown {
        name: "TownWindowState",
        apply: reset_default::<TownWindowState>,
    },
    ResourceTeardown {
        name: "TownDirectoryState",
        apply: reset_default::<TownDirectoryState>,
    },
    ResourceTeardown {
        name: "StationDirectoryState",
        apply: reset_default::<StationDirectoryState>,
    },
    ResourceTeardown {
        name: "VehicleListState",
        apply: reset_default::<VehicleListState>,
    },
    ResourceTeardown {
        name: "SubsidyListState",
        apply: reset_default::<SubsidyListState>,
    },
    ResourceTeardown {
        name: "BuyVehicleWindowState",
        apply: reset_default::<BuyVehicleWindowState>,
    },
    ResourceTeardown {
        name: "NewGrfTrainPreviewCache",
        apply: clear_train_preview_cache,
    },
    ResourceTeardown {
        name: "NewGrfRoadTypePreviewCache",
        apply: clear_road_preview_cache,
    },
    ResourceTeardown {
        name: "NewGrfStationPreviewCache",
        apply: clear_station_preview_cache,
    },
    ResourceTeardown {
        name: "NewGrfTrainSpriteCache",
        apply: clear_newgrf_train_sprites,
    },
    ResourceTeardown {
        name: "NewGrfRoadSpriteCache",
        apply: clear_newgrf_road_sprites,
    },
    ResourceTeardown {
        name: "NewGrfStationSpriteCache",
        apply: clear_newgrf_station_sprites,
    },
    ResourceTeardown {
        name: "NewGrfShoreSpriteCache",
        apply: clear_newgrf_shore_sprites,
    },
    ResourceTeardown {
        name: "NewGrfCatenarySpriteCache",
        apply: clear_newgrf_catenary_sprites,
    },
    ResourceTeardown {
        name: "NewGrfAction5SpriteCache",
        apply: clear_newgrf_action5_sprites,
    },
    ResourceTeardown {
        name: "NewGrfObjectSpriteCache",
        apply: clear_newgrf_object_sprites,
    },
    ResourceTeardown {
        name: "DestinationPickerState",
        apply: reset_default::<DestinationPickerState>,
    },
    ResourceTeardown {
        name: "VehicleWindowState",
        apply: reset_default::<VehicleWindowState>,
    },
    ResourceTeardown {
        name: "VehicleChainRegistry",
        apply: reset_default::<VehicleChainRegistry>,
    },
    ResourceTeardown {
        name: "VehicleDetailsWindowState",
        apply: reset_default::<VehicleDetailsWindowState>,
    },
    ResourceTeardown {
        name: "RefitWindowState",
        apply: reset_default::<RefitWindowState>,
    },
    ResourceTeardown {
        name: "SharedOrdersWindowState",
        apply: reset_default::<SharedOrdersWindowState>,
    },
    ResourceTeardown {
        name: "AutoreplaceWindowState",
        apply: reset_default::<AutoreplaceWindowState>,
    },
    ResourceTeardown {
        name: "TimetableWindowState",
        apply: reset_default::<TimetableWindowState>,
    },
    ResourceTeardown {
        name: "FinancesWindowState",
        apply: reset_default::<FinancesWindowState>,
    },
    ResourceTeardown {
        name: "GraphWindowState",
        apply: reset_default::<GraphWindowState>,
    },
    ResourceTeardown {
        name: "CargoPaymentWindowState",
        apply: reset_default::<CargoPaymentWindowState>,
    },
    ResourceTeardown {
        name: "NewsSettingsWindowState",
        apply: reset_default::<NewsSettingsWindowState>,
    },
    ResourceTeardown {
        name: "PathfindingSettingsWindowState",
        apply: reset_default::<PathfindingSettingsWindowState>,
    },
    ResourceTeardown {
        name: "CargoDistSettingsWindowState",
        apply: reset_default::<CargoDistSettingsWindowState>,
    },
    ResourceTeardown {
        name: "AiSettingsWindowState",
        apply: reset_default::<AiSettingsWindowState>,
    },
    ResourceTeardown {
        name: "NewGrfWindowState",
        apply: reset_default::<NewGrfWindowState>,
    },
    ResourceTeardown {
        name: "HelpWindowState",
        apply: reset_default::<HelpWindowState>,
    },
    ResourceTeardown {
        name: "DevConsoleState",
        apply: reset_default::<DevConsoleState>,
    },
    ResourceTeardown {
        name: "TileInspectorWindowState",
        apply: reset_default::<TileInspectorWindowState>,
    },
    ResourceTeardown {
        name: "CheatWindowState",
        apply: reset_default::<CheatWindowState>,
    },
    ResourceTeardown {
        name: "GenLandWindowState",
        apply: reset_default::<GenLandWindowState>,
    },
    ResourceTeardown {
        name: "EditorTownMenuState",
        apply: reset_default::<EditorTownMenuState>,
    },
    ResourceTeardown {
        name: "GoalListWindowState",
        apply: reset_default::<GoalListWindowState>,
    },
    ResourceTeardown {
        name: "StoryWindowState",
        apply: reset_default::<StoryWindowState>,
    },
    ResourceTeardown {
        name: "LeagueWindowState",
        apply: reset_default::<LeagueWindowState>,
    },
    ResourceTeardown {
        name: "SuspendedGameSession+EditorSession",
        apply: preserve_suspended_and_reset_editor,
    },
    ResourceTeardown {
        name: "EndScreenState",
        apply: reset_default::<EndScreenState>,
    },
    ResourceTeardown {
        name: "RetireGameRequested",
        apply: reset_default::<RetireGameRequested>,
    },
    ResourceTeardown {
        name: "SoundMusicWindowState",
        apply: reset_default::<SoundMusicWindowState>,
    },
    ResourceTeardown {
        name: "NewsUiState",
        apply: reset_default::<NewsUiState>,
    },
    ResourceTeardown {
        name: "NewsHistoryState",
        apply: reset_default::<NewsHistoryState>,
    },
];

/// Caches / assets de mapa que se eliminan del world al salir.
pub(super) static RESOURCE_REMOVES: &[ResourceTeardown] = &[
    ResourceTeardown {
        name: "MapTileSpawnViewport",
        apply: remove_res::<MapTileSpawnViewport>,
    },
    ResourceTeardown {
        name: "WorldAssets",
        apply: remove_res::<WorldAssets>,
    },
    ResourceTeardown {
        name: "WaterAnimFrames",
        apply: remove_res::<WaterAnimFrames>,
    },
    ResourceTeardown {
        name: "RefineryFireAnimFrames",
        apply: remove_res::<RefineryFireAnimFrames>,
    },
    ResourceTeardown {
        name: "FizzyDrinkAnimFrames",
        apply: remove_res::<FizzyDrinkAnimFrames>,
    },
    ResourceTeardown {
        name: "LighthouseAnimFrames",
        apply: remove_res::<LighthouseAnimFrames>,
    },
    ResourceTeardown {
        name: "ChimneySmokeFrames",
        apply: remove_res::<ChimneySmokeFrames>,
    },
    ResourceTeardown {
        name: "CopperMineSmokeFrames",
        apply: remove_res::<CopperMineSmokeFrames>,
    },
    ResourceTeardown {
        name: "EffectVehicleFrames",
        apply: remove_res::<EffectVehicleFrames>,
    },
    ResourceTeardown {
        name: "CompanyColoredSprites",
        apply: remove_res::<CompanyColoredSprites>,
    },
    ResourceTeardown {
        name: "TileAtlas",
        apply: remove_res::<TileAtlas>,
    },
    ResourceTeardown {
        name: "LoadedMapTileChunks",
        apply: remove_res::<LoadedMapTileChunks>,
    },
    ResourceTeardown {
        name: "TruckHandles",
        apply: remove_res::<TruckHandles>,
    },
    ResourceTeardown {
        name: "NewGrfTrainSpriteCache(remove)",
        apply: remove_res::<NewGrfTrainSpriteCache>,
    },
    ResourceTeardown {
        name: "NewGrfRoadSpriteCache(remove)",
        apply: remove_res::<NewGrfRoadSpriteCache>,
    },
    ResourceTeardown {
        name: "NewGrfStationSpriteCache(remove)",
        apply: remove_res::<NewGrfStationSpriteCache>,
    },
    ResourceTeardown {
        name: "NewGrfShoreSpriteCache(remove)",
        apply: remove_res::<NewGrfShoreSpriteCache>,
    },
    ResourceTeardown {
        name: "NewGrfCatenarySpriteCache(remove)",
        apply: remove_res::<NewGrfCatenarySpriteCache>,
    },
    ResourceTeardown {
        name: "NewGrfAction5SpriteCache(remove)",
        apply: remove_res::<NewGrfAction5SpriteCache>,
    },
    ResourceTeardown {
        name: "NewGrfObjectSpriteCache(remove)",
        apply: remove_res::<NewGrfObjectSpriteCache>,
    },
];

pub(super) fn apply_session_resource_teardown(world: &mut World) {
    for entry in RESOURCE_RESETS {
        (entry.apply)(world);
    }
    for entry in RESOURCE_REMOVES {
        (entry.apply)(world);
    }
}

#[cfg(test)]
pub(super) fn resource_reset_names() -> Vec<&'static str> {
    RESOURCE_RESETS.iter().map(|e| e.name).collect()
}

#[cfg(test)]
pub(super) fn resource_remove_names() -> Vec<&'static str> {
    RESOURCE_REMOVES.iter().map(|e| e.name).collect()
}
