//! Inventario del registro de teardown (#145).

#![allow(clippy::unwrap_used)]

use super::super::SaveWindowState;
use super::super::ai_settings_window::AiSettingsWindowState;
use super::super::audio_settings_window::SoundMusicWindowState;
use super::super::autoreplace_window::AutoreplaceWindowState;
use super::super::buy_window::{BuyVehicleWindowState, NewGrfTrainPreviewCache};
use super::super::cargo_dist_settings_window::CargoDistSettingsWindowState;
use super::super::cargo_payment_window::CargoPaymentWindowState;
use super::super::destination_window::DestinationPickerState;
use super::super::finances_window::FinancesWindowState;
use super::super::graph_window::GraphWindowState;
use super::super::industry_panel::IndustryPanelState;
use super::super::newgrf_window::NewGrfWindowState;
use super::super::news_settings_window::NewsSettingsWindowState;
use super::super::pathfinding_settings_window::PathfindingSettingsWindowState;
use super::super::refit_window::RefitWindowState;
use super::super::shared_orders_window::SharedOrdersWindowState;
use super::super::statusbar::{NewsHistoryState, NewsUiState};
use super::super::timetable_window::TimetableWindowState;
use super::super::toolbar::{
    BridgeBuildState, DepotPanelState, DragBuildState, OrderEditState, RailSignalGhostState,
    StationBuildState, StationCargoPanelState, ToolbarState, UiToolState,
};
use super::super::town_window::TownWindowState;
use super::super::vehicle_chain::VehicleChainRegistry;
use super::super::vehicle_details_window::VehicleDetailsWindowState;
use super::super::vehicle_window::VehicleWindowState;
use super::entity_cleanup::{ENTITY_TEARDOWNS, entity_teardown_names};
use super::plugin::leave_ingame;
use super::resource_reset::{
    RESOURCE_REMOVES, RESOURCE_RESETS, resource_remove_names, resource_reset_names,
};
use crate::audio::PendingSimEvents;
use crate::render::effect_fx::FxSpawnQueue;
use crate::render::{MapVisualLayer, PrimaryGameCamera, RemapMapVisualsPending, VehicleIndex};
use crate::simulation::SimClock;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

/// Recursos de sesión que **deben** tener política en el registro (reset o remove).
///
/// Al añadir un resource de sesión InGame, agregarlo aquí **y** a
/// `RESOURCE_RESETS` / `RESOURCE_REMOVES` (o el test falla).
const REQUIRED_SESSION_POLICIES: &[&str] = &[
    "Time<Virtual>",
    "VehicleIndex",
    "RemapMapVisualsPending",
    "SimClock",
    "PendingSimEvents",
    "FxSpawnQueue",
    "UiToolState",
    "StationBuildState",
    "DragBuildState",
    "BridgeBuildState",
    "OrderEditState",
    "NextState<OrderPickState>",
    "DepotPanelState",
    "StationCargoPanelState",
    "RailSignalGhostState",
    "ToolbarState",
    "ToolbarMenuState",
    "IndustryPanelState",
    "IndustryDirectoryState",
    "SaveWindowState",
    "TownWindowState",
    "TownDirectoryState",
    "StationDirectoryState",
    "VehicleListState",
    "SubsidyListState",
    "BuyVehicleWindowState",
    "NewGrfTrainPreviewCache",
    "NewGrfRoadTypePreviewCache",
    "NewGrfStationPreviewCache",
    "NewGrfTrainSpriteCache",
    "NewGrfRoadSpriteCache",
    "NewGrfStationSpriteCache",
    "NewGrfShoreSpriteCache",
    "NewGrfAction5SpriteCache",
    "NewGrfObjectSpriteCache",
    "NewGrfCatenarySpriteCache",
    "DestinationPickerState",
    "VehicleWindowState",
    "VehicleChainRegistry",
    "VehicleDetailsWindowState",
    "RefitWindowState",
    "SharedOrdersWindowState",
    "AutoreplaceWindowState",
    "TimetableWindowState",
    "FinancesWindowState",
    "GraphWindowState",
    "CargoPaymentWindowState",
    "NewsSettingsWindowState",
    "PathfindingSettingsWindowState",
    "CargoDistSettingsWindowState",
    "AiSettingsWindowState",
    "NewGrfWindowState",
    "HelpWindowState",
    "DevConsoleState",
    "TileInspectorWindowState",
    "CheatWindowState",
    "ModalStack",
    "QueryStringWindowState",
    "ErrorDialogWindowState",
    "OskWindowState",
    "GenLandWindowState",
    "EditorTownMenuState",
    "GoalListWindowState",
    "StoryWindowState",
    "LeagueWindowState",
    "SuspendedGameSession+EditorSession",
    "EndScreenState",
    "RetireGameRequested",
    "SoundMusicWindowState",
    "NewsUiState",
    "NewsHistoryState",
    "MapTileSpawnViewport",
    "WorldAssets",
    "WaterAnimFrames",
    "RefineryFireAnimFrames",
    "FizzyDrinkAnimFrames",
    "LighthouseAnimFrames",
    "ChimneySmokeFrames",
    "CopperMineSmokeFrames",
    "EffectVehicleFrames",
    "CompanyColoredSprites",
    "TileAtlas",
    "LoadedMapTileChunks",
    "TruckHandles",
    "NewGrfTrainSpriteCache(remove)",
    "NewGrfRoadSpriteCache(remove)",
    "NewGrfStationSpriteCache(remove)",
    "NewGrfShoreSpriteCache(remove)",
    "NewGrfAction5SpriteCache(remove)",
    "NewGrfObjectSpriteCache(remove)",
    "NewGrfCatenarySpriteCache(remove)",
];

#[test]
fn session_resource_policies_cover_required_inventory() {
    let mut covered: Vec<&str> = resource_reset_names();
    covered.extend(resource_remove_names());
    covered.sort_unstable();
    for required in REQUIRED_SESSION_POLICIES {
        assert!(
            covered.binary_search(required).is_ok(),
            "recurso de sesión sin política de teardown: {required} — \
             añadilo a RESOURCE_RESETS o RESOURCE_REMOVES en resource_reset.rs"
        );
    }
}

#[test]
fn teardown_registry_has_no_duplicate_names() {
    let mut names = entity_teardown_names();
    names.extend(resource_reset_names());
    names.extend(resource_remove_names());
    let before = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        before,
        names.len(),
        "nombres duplicados en el registro de teardown"
    );
}

#[test]
fn entity_and_resource_registries_non_empty() {
    assert!(
        ENTITY_TEARDOWNS.len() >= 17,
        "faltan markers de despawn (históricamente ~17)"
    );
    assert!(
        RESOURCE_RESETS.len() >= 40,
        "faltan resets de recursos de sesión"
    );
    assert!(
        RESOURCE_REMOVES.len() >= 14,
        "faltan removes de caches/assets"
    );
}

#[test]
fn leave_ingame_despawns_world_entities() {
    let mut world = World::new();
    world.init_resource::<VehicleIndex>();
    world.init_resource::<RemapMapVisualsPending>();
    world.init_resource::<SimClock>();
    world.init_resource::<PendingSimEvents>();
    world.init_resource::<FxSpawnQueue>();
    world.init_resource::<UiToolState>();
    world.init_resource::<StationBuildState>();
    world.init_resource::<DragBuildState>();
    world.init_resource::<BridgeBuildState>();
    world.init_resource::<OrderEditState>();
    world.init_resource::<DepotPanelState>();
    world.init_resource::<StationCargoPanelState>();
    world.init_resource::<RailSignalGhostState>();
    world.init_resource::<ToolbarState>();
    world.init_resource::<IndustryPanelState>();
    world.init_resource::<SaveWindowState>();
    world.init_resource::<TownWindowState>();
    world.init_resource::<BuyVehicleWindowState>();
    world.init_resource::<DestinationPickerState>();
    world.init_resource::<VehicleWindowState>();
    world.init_resource::<VehicleChainRegistry>();
    world.init_resource::<VehicleDetailsWindowState>();
    world.init_resource::<RefitWindowState>();
    world.init_resource::<SharedOrdersWindowState>();
    world.init_resource::<AutoreplaceWindowState>();
    world.init_resource::<TimetableWindowState>();
    world.init_resource::<FinancesWindowState>();
    world.init_resource::<GraphWindowState>();
    world.init_resource::<CargoPaymentWindowState>();
    world.init_resource::<NewsSettingsWindowState>();
    world.init_resource::<PathfindingSettingsWindowState>();
    world.init_resource::<CargoDistSettingsWindowState>();
    world.init_resource::<AiSettingsWindowState>();
    world.init_resource::<NewGrfWindowState>();
    world.init_resource::<SoundMusicWindowState>();
    world.init_resource::<NewsUiState>();
    world.init_resource::<NewsHistoryState>();
    world.init_resource::<NewGrfTrainPreviewCache>();

    let cam = world.spawn((PrimaryGameCamera, Camera2d)).id();
    world.spawn(MapVisualLayer);
    world.run_system_once(leave_ingame).unwrap();
    assert!(world.get_entity(cam).is_err());
    assert_eq!(world.query::<&MapVisualLayer>().iter(&world).count(), 0);
}
