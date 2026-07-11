//! Desmontaje de sesión al salir de `InGame`.

use bevy::prelude::*;

use crate::audio::{MusicPlayer, PendingSimEvents};
use crate::debug_gizmos::DiagnosticsOverlayRoot;
use crate::render::effect_fx::FxSpawnQueue;
use crate::render::{
    ChimneySmokeFrames, CompanyColoredSprites, CopperMineSmokeFrames, EffectVehicleFrames,
    FizzyDrinkAnimFrames, IndustryPreviewCamera, LighthouseAnimFrames, LoadedMapTileChunks,
    MapPreviewCamera, MapTileSpawnViewport, MapVisualLayer, PrimaryGameCamera,
    RefineryFireAnimFrames, RemapMapVisualsPending, ShoreTile, TileAtlas, TruckHandles,
    VehicleIndex, WaterAnimFrames, WaterTile, WorldAssets,
};
use crate::simulation::SimClock;
use crate::state::ingame_lifecycle::InGameUi;
use crate::state::{ClientScreen, OrderPickState};

use super::SaveWindowState;
use super::audio_settings_window::SoundMusicWindowState;
use super::autoreplace_window::AutoreplaceWindowState;
use super::buy_window::BuyVehicleWindowState;
use super::cargo_payment_window::CargoPaymentWindowState;
use super::destination_window::DestinationPickerState;
use super::finances_window::FinancesWindowState;
use super::floating_window::FloatingWindow;
use super::graph_window::GraphWindowState;
use super::hud::TileInfoText;
use super::industry_directory::IndustryDirectoryState;
use super::industry_panel::{IndustryPanelRoot, IndustryPanelState};
use super::navigation::ToolbarMenuState;
use super::newgrf_window::NewGrfWindowState;
use super::news_settings_window::NewsSettingsWindowState;
use super::pathfinding_settings_window::PathfindingSettingsWindowState;
use super::refit_window::RefitWindowState;
use super::shared_orders_window::SharedOrdersWindowState;
use super::station_directory::StationDirectoryState;
use super::statusbar::{NewsHistoryState, NewsPopupRoot, NewsUiState, StatusBarRoot};
use super::subsidy_list::SubsidyListState;
use super::timetable_window::TimetableWindowState;
use super::toolbar::{
    BridgeBuildState, BuildGhostPreview, DepotPanelState, DragBuildState, MinimapRoot,
    OrderEditState, OrderPanelRoot, RailSignalGhost, RailSignalGhostState, StationBuildState,
    StationCargoPanelState, ToolbarState, UiToolState,
};
use super::town_directory::TownDirectoryState;
use super::town_window::TownWindowState;
use super::vehicle_list::VehicleListState;
use super::vehicle_window::VehicleWindowState;

pub(crate) struct InGameLifecyclePlugin;

impl Plugin for InGameLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(ClientScreen::InGame), leave_ingame);
    }
}

/// Desmonta mundo, HUD y recursos de sesión al volver al menú u otra pantalla.
pub(crate) fn leave_ingame(world: &mut World) {
    let mut to_despawn: Vec<Entity> = Vec::new();
    collect_matching::<PrimaryGameCamera>(world, &mut to_despawn);
    collect_matching::<MapVisualLayer>(world, &mut to_despawn);
    collect_matching::<WaterTile>(world, &mut to_despawn);
    collect_matching::<ShoreTile>(world, &mut to_despawn);
    collect_matching::<StatusBarRoot>(world, &mut to_despawn);
    collect_matching::<MinimapRoot>(world, &mut to_despawn);
    collect_matching::<OrderPanelRoot>(world, &mut to_despawn);
    collect_matching::<IndustryPanelRoot>(world, &mut to_despawn);
    collect_matching::<FloatingWindow>(world, &mut to_despawn);
    collect_matching::<TileInfoText>(world, &mut to_despawn);
    collect_matching::<DiagnosticsOverlayRoot>(world, &mut to_despawn);
    collect_matching::<NewsPopupRoot>(world, &mut to_despawn);
    collect_matching::<BuildGhostPreview>(world, &mut to_despawn);
    collect_matching::<RailSignalGhost>(world, &mut to_despawn);
    collect_matching::<MapPreviewCamera>(world, &mut to_despawn);
    collect_matching::<IndustryPreviewCamera>(world, &mut to_despawn);
    collect_matching::<InGameUi>(world, &mut to_despawn);
    to_despawn.sort_unstable();
    to_despawn.dedup();

    let mut music_entities: Vec<Entity> = Vec::new();
    collect_matching::<MusicPlayer>(world, &mut music_entities);

    let mut commands = world.commands();
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
    for entity in music_entities {
        commands.entity(entity).despawn();
    }

    if let Some(mut virtual_time) = world.get_resource_mut::<Time<Virtual>>() {
        virtual_time.unpause();
    }

    if let Some(mut vehicle_index) = world.get_resource_mut::<VehicleIndex>() {
        *vehicle_index = VehicleIndex::default();
    }
    if let Some(mut remap) = world.get_resource_mut::<RemapMapVisualsPending>() {
        *remap = RemapMapVisualsPending::default();
    }
    if let Some(mut sim_clock) = world.get_resource_mut::<SimClock>() {
        *sim_clock = SimClock::default();
    }
    if let Some(mut pending_events) = world.get_resource_mut::<PendingSimEvents>() {
        pending_events.0.clear();
    }
    if let Some(mut fx_queue) = world.get_resource_mut::<FxSpawnQueue>() {
        *fx_queue = FxSpawnQueue::default();
    }

    if let Some(mut tool_state) = world.get_resource_mut::<UiToolState>() {
        *tool_state = UiToolState::default();
    }
    if let Some(mut station_build) = world.get_resource_mut::<StationBuildState>() {
        *station_build = StationBuildState::default();
    }
    if let Some(mut drag_build) = world.get_resource_mut::<DragBuildState>() {
        *drag_build = DragBuildState::default();
    }
    if let Some(mut bridge_build) = world.get_resource_mut::<BridgeBuildState>() {
        *bridge_build = BridgeBuildState::default();
    }
    if let Some(mut order_edit) = world.get_resource_mut::<OrderEditState>() {
        order_edit.clear();
    }
    if let Some(mut next_pick) = world.get_resource_mut::<NextState<OrderPickState>>() {
        next_pick.set(OrderPickState::Idle);
    }
    if let Some(mut depot_panel) = world.get_resource_mut::<DepotPanelState>() {
        *depot_panel = DepotPanelState::default();
    }
    if let Some(mut station_cargo) = world.get_resource_mut::<StationCargoPanelState>() {
        *station_cargo = StationCargoPanelState::default();
    }
    if let Some(mut signal_ghost) = world.get_resource_mut::<RailSignalGhostState>() {
        *signal_ghost = RailSignalGhostState::default();
    }
    if let Some(mut toolbar) = world.get_resource_mut::<ToolbarState>() {
        *toolbar = ToolbarState::default();
    }
    if let Some(mut menu) = world.get_resource_mut::<ToolbarMenuState>() {
        *menu = ToolbarMenuState::default();
    }
    if let Some(mut industry_panel) = world.get_resource_mut::<IndustryPanelState>() {
        *industry_panel = IndustryPanelState::default();
    }
    if let Some(mut industry_directory) = world.get_resource_mut::<IndustryDirectoryState>() {
        *industry_directory = IndustryDirectoryState::default();
    }
    if let Some(mut save_window) = world.get_resource_mut::<SaveWindowState>() {
        save_window.close();
    }
    if let Some(mut town_window) = world.get_resource_mut::<TownWindowState>() {
        *town_window = TownWindowState::default();
    }
    if let Some(mut town_directory) = world.get_resource_mut::<TownDirectoryState>() {
        *town_directory = TownDirectoryState::default();
    }
    if let Some(mut station_directory) = world.get_resource_mut::<StationDirectoryState>() {
        *station_directory = StationDirectoryState::default();
    }
    if let Some(mut vehicle_list) = world.get_resource_mut::<VehicleListState>() {
        *vehicle_list = VehicleListState::default();
    }
    if let Some(mut subsidy_list) = world.get_resource_mut::<SubsidyListState>() {
        *subsidy_list = SubsidyListState::default();
    }
    if let Some(mut buy_window) = world.get_resource_mut::<BuyVehicleWindowState>() {
        *buy_window = BuyVehicleWindowState::default();
    }
    if let Some(mut destination_picker) = world.get_resource_mut::<DestinationPickerState>() {
        *destination_picker = DestinationPickerState::default();
    }
    if let Some(mut vehicle_window) = world.get_resource_mut::<VehicleWindowState>() {
        *vehicle_window = VehicleWindowState::default();
    }
    if let Some(mut refit_window) = world.get_resource_mut::<RefitWindowState>() {
        *refit_window = RefitWindowState::default();
    }
    if let Some(mut shared_orders) = world.get_resource_mut::<SharedOrdersWindowState>() {
        *shared_orders = SharedOrdersWindowState::default();
    }
    if let Some(mut autoreplace) = world.get_resource_mut::<AutoreplaceWindowState>() {
        *autoreplace = AutoreplaceWindowState::default();
    }
    if let Some(mut timetable_window) = world.get_resource_mut::<TimetableWindowState>() {
        *timetable_window = TimetableWindowState::default();
    }
    if let Some(mut finances_window) = world.get_resource_mut::<FinancesWindowState>() {
        *finances_window = FinancesWindowState::default();
    }
    if let Some(mut graph_window) = world.get_resource_mut::<GraphWindowState>() {
        *graph_window = GraphWindowState::default();
    }
    if let Some(mut cargo_payment) = world.get_resource_mut::<CargoPaymentWindowState>() {
        *cargo_payment = CargoPaymentWindowState::default();
    }
    if let Some(mut news_settings) = world.get_resource_mut::<NewsSettingsWindowState>() {
        *news_settings = NewsSettingsWindowState::default();
    }
    if let Some(mut pathfinding_settings) =
        world.get_resource_mut::<PathfindingSettingsWindowState>()
    {
        *pathfinding_settings = PathfindingSettingsWindowState::default();
    }
    if let Some(mut newgrf) = world.get_resource_mut::<NewGrfWindowState>() {
        *newgrf = NewGrfWindowState::default();
    }
    if let Some(mut sound_music) = world.get_resource_mut::<SoundMusicWindowState>() {
        *sound_music = SoundMusicWindowState::default();
    }
    if let Some(mut news_ui) = world.get_resource_mut::<NewsUiState>() {
        *news_ui = NewsUiState::default();
    }
    if let Some(mut news_history) = world.get_resource_mut::<NewsHistoryState>() {
        *news_history = NewsHistoryState::default();
    }

    world.remove_resource::<MapTileSpawnViewport>();
    world.remove_resource::<WorldAssets>();
    world.remove_resource::<WaterAnimFrames>();
    world.remove_resource::<RefineryFireAnimFrames>();
    world.remove_resource::<FizzyDrinkAnimFrames>();
    world.remove_resource::<LighthouseAnimFrames>();
    world.remove_resource::<ChimneySmokeFrames>();
    world.remove_resource::<CopperMineSmokeFrames>();
    world.remove_resource::<EffectVehicleFrames>();
    world.remove_resource::<CompanyColoredSprites>();
    world.remove_resource::<TileAtlas>();
    world.remove_resource::<LoadedMapTileChunks>();
    world.remove_resource::<TruckHandles>();
}

fn collect_matching<M: Component>(world: &mut World, out: &mut Vec<Entity>) {
    let mut query = world.query_filtered::<Entity, With<M>>();
    for entity in query.iter(world) {
        out.push(entity);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

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
        world.init_resource::<RefitWindowState>();
        world.init_resource::<SharedOrdersWindowState>();
        world.init_resource::<AutoreplaceWindowState>();
        world.init_resource::<TimetableWindowState>();
        world.init_resource::<FinancesWindowState>();
        world.init_resource::<GraphWindowState>();
        world.init_resource::<CargoPaymentWindowState>();
        world.init_resource::<NewsSettingsWindowState>();
        world.init_resource::<PathfindingSettingsWindowState>();
        world.init_resource::<NewGrfWindowState>();
        world.init_resource::<SoundMusicWindowState>();
        world.init_resource::<NewsUiState>();
        world.init_resource::<NewsHistoryState>();

        let cam = world.spawn((PrimaryGameCamera, Camera2d)).id();
        world.spawn(MapVisualLayer);
        world.run_system_once(leave_ingame).unwrap();
        assert!(world.get_entity(cam).is_err());
        assert_eq!(world.query::<&MapVisualLayer>().iter(&world).count(), 0);
    }
}
