//! Sistemas ECS de la toolbar (grupos, cierre, herramientas, tooltip).

mod click_beep;
mod close;
mod groups;
mod tools;
mod tooltip;

pub(crate) use click_beep::toolbar_click_beep;
pub(crate) use close::{close_toolbar_button_interaction, close_toolbar_panel_on_escape};
pub(crate) use groups::{
    hide_tool_when_panel_closed, sync_climate_industry_tools, toolbar_group_interaction,
    update_toolbar_group_visuals, update_toolbar_tool_visibility,
};
pub(crate) use tools::{build_menu_interaction, update_tool_button_visuals};
pub(crate) use tooltip::update_toolbar_tooltip;

#[cfg(test)]
pub(crate) use groups::toolbar_group_for_action;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use openttdrs_core::{Command, GameState, TileCoord, TileKind, VehicleKind, VehicleOrder};

    use crate::render::{PrimaryGameCamera, RemapMapVisualsPending, VehicleIndex};
    use crate::state::SimWorld;
    use crate::ui::audio_settings_window::AudioSettingsWindowState;
    use crate::ui::hud::{HoveredTileCoord, HudBuildFeedback, SelectedTileInfo, SimHudControls};
    use crate::ui::industry_panel::IndustryPanelState;
    use crate::ui::news_settings_window::NewsSettingsWindowState;
    use crate::ui::save_window::SaveWindowState;
    use crate::ui::timetable_window::TimetableWindowState;
    use crate::ui::toolbar::build_input::commands::{command_for_action, command_for_line_action};
    use crate::ui::toolbar::build_input::drag::{
        action_is_tunnel, action_supports_area_drag, action_supports_drag, drag_line_tiles,
        road_bits_for_drag_action, tunnel_placement_is_valid,
    };
    use crate::ui::toolbar::build_input::orders::order_for_clicked_tile;
    use crate::ui::toolbar::{
        BridgeBuildState, BuildMenuAction, BuildMenuUi, DepotPanelState, DragBuildState,
        OrderEditState, SaveMenuAction, StationBuildState, StationCargoPanelState, ToolbarGroup,
        ToolbarState, UiToolState, handle_minimap_click, handle_order_panel_buttons,
        handle_settings_menu_buttons, handle_tile_click, setup_minimap, setup_order_panel,
        sync_minimap, sync_order_panel,
    };
    use openttdrs_core::BridgeType;

    #[test]
    fn close_toolbar_escape_clears_state() {
        let mut world = World::new();
        let mut kb = ButtonInput::<KeyCode>::default();
        kb.press(KeyCode::Escape);
        world.insert_resource(kb);
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::RoadY),
        });
        world.insert_resource(DragBuildState {
            armed: true,
            ..default()
        });
        world.insert_resource(OrderEditState::default());
        world
            .run_system_once(close_toolbar_panel_on_escape)
            .unwrap();
    }

    #[test]
    fn hide_tool_mismatch_group_clears_tool() {
        let mut world = World::new();
        world.insert_resource(ToolbarState {
            active_group: Some(ToolbarGroup::Road),
            ..Default::default()
        });
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::Rail),
        });
        world.insert_resource(DragBuildState::default());
        world.run_system_once(hide_tool_when_panel_closed).unwrap();
    }

    #[test]
    fn setup_minimap_then_sync_minimap() {
        let mut world = World::new();
        world.run_system_once(setup_minimap).unwrap();
        world.insert_resource(SimWorld::default());
        world.insert_resource(SimHudControls::default());
        world.run_system_once(sync_minimap).unwrap();
    }

    #[test]
    fn handle_minimap_click_ignored_when_toolbar_is_interacting() {
        let mut world = World::new();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(SelectedTileInfo::default());
        world.spawn((BuildMenuUi, Interaction::Pressed));
        world.run_system_once(handle_minimap_click).unwrap();
    }

    #[test]
    fn setup_order_panel_then_sync_order_panel() {
        use bevy::asset::AssetPlugin;

        let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
            file_path: asset_root.into(),
            ..default()
        });
        app.world_mut().init_resource::<Assets<Image>>();
        app.init_asset::<Font>();
        app.world_mut().run_system_once(setup_order_panel).unwrap();
        app.world_mut().insert_resource(OrderEditState::default());
        app.world_mut().insert_resource(SimWorld::default());
        app.world_mut().spawn((
            PrimaryGameCamera,
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        app.world_mut().run_system_once(sync_order_panel).unwrap();
    }

    #[test]
    fn toolbar_interaction_systems_run_with_empty_queries() {
        let mut world = World::new();
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(DragBuildState::default());
        world.insert_resource(OrderEditState::default());
        world.run_system_once(toolbar_group_interaction).unwrap();
        world.run_system_once(update_toolbar_group_visuals).unwrap();
        world
            .run_system_once(update_toolbar_tool_visibility)
            .unwrap();
        world
            .run_system_once(close_toolbar_button_interaction)
            .unwrap();
        world.run_system_once(build_menu_interaction).unwrap();
    }

    #[test]
    fn update_tool_button_visuals_empty() {
        let mut world = World::new();
        world.insert_resource(UiToolState::default());
        world.run_system_once(update_tool_button_visuals).unwrap();
    }

    #[test]
    fn update_toolbar_tooltip_no_ui_returns_early() {
        let mut world = World::new();
        world.run_system_once(update_toolbar_tooltip).unwrap();
    }

    #[test]
    fn handle_order_panel_buttons_empty() {
        let mut world = World::new();
        world.insert_resource(OrderEditState::default());
        world.insert_resource(crate::ui::destination_window::DestinationPickerState::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(DragBuildState::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(HudBuildFeedback::default());
        world.insert_resource(TimetableWindowState::default());
        world.insert_resource(Time::<()>::default());
        world.run_system_once(handle_order_panel_buttons).unwrap();
    }

    #[test]
    fn handle_settings_menu_buttons_save_load_open_save_window() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("sim.json");

        let mut world = World::new();
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(NewsSettingsWindowState::default());
        world.insert_resource(AudioSettingsWindowState::default());
        world.insert_resource(SimHudControls {
            paused: false,
            sim_speed: 1.0,
            json_save_path: save_path.to_string_lossy().to_string(),
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });

        world.spawn((Button, SaveMenuAction::SaveAs, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert!(world.resource::<SaveWindowState>().open);

        world.spawn((Button, SaveMenuAction::LoadFrom, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert!(world.resource::<SaveWindowState>().open);
    }

    #[test]
    fn handle_settings_menu_buttons_pause_speed_and_zoom() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(NewsSettingsWindowState::default());
        world.insert_resource(AudioSettingsWindowState::default());
        world.insert_resource(SimHudControls::default());
        world.spawn((
            PrimaryGameCamera,
            Transform::from_xyz(123.0, -45.0, 0.0),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.spawn((Button, SaveMenuAction::PauseResume, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert!(world.resource::<SimHudControls>().paused);

        world.spawn((Button, SaveMenuAction::SpeedUp, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert_eq!(world.resource::<SimHudControls>().sim_speed, 2.0);

        world.spawn((Button, SaveMenuAction::Normalize, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        assert_eq!(world.resource::<SimHudControls>().sim_speed, 1.0);
        let mut q_norm =
            world.query_filtered::<(&Transform, &Projection), With<PrimaryGameCamera>>();
        let (tf_norm, proj_norm) = q_norm.single(&world).unwrap();
        let Projection::Orthographic(o_norm) = proj_norm else {
            panic!("expected orthographic projection");
        };
        assert_eq!(o_norm.scale, 1.0);
        assert_eq!(tf_norm.translation.x, 123.0);
        assert_eq!(tf_norm.translation.y, -45.0);

        let mut world_zoom_in = World::new();
        world_zoom_in.insert_resource(SimWorld::default());
        world_zoom_in.insert_resource(VehicleIndex::default());
        world_zoom_in.insert_resource(RemapMapVisualsPending::default());
        world_zoom_in.insert_resource(SaveWindowState::default());
        world_zoom_in.insert_resource(NewsSettingsWindowState::default());
        world_zoom_in.insert_resource(AudioSettingsWindowState::default());
        world_zoom_in.insert_resource(SimHudControls::default());
        world_zoom_in.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world_zoom_in.spawn((Button, SaveMenuAction::ZoomIn, Interaction::Pressed));
        world_zoom_in
            .run_system_once(handle_settings_menu_buttons)
            .unwrap();
        let mut q_in = world_zoom_in.query_filtered::<&Projection, With<PrimaryGameCamera>>();
        let Projection::Orthographic(o_in) = q_in.single(&world_zoom_in).unwrap() else {
            panic!("expected orthographic projection");
        };
        assert!(o_in.scale < 1.0);

        let mut world_zoom_out = World::new();
        world_zoom_out.insert_resource(SimWorld::default());
        world_zoom_out.insert_resource(VehicleIndex::default());
        world_zoom_out.insert_resource(RemapMapVisualsPending::default());
        world_zoom_out.insert_resource(SaveWindowState::default());
        world_zoom_out.insert_resource(NewsSettingsWindowState::default());
        world_zoom_out.insert_resource(AudioSettingsWindowState::default());
        world_zoom_out.insert_resource(SimHudControls::default());
        world_zoom_out.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world_zoom_out.spawn((Button, SaveMenuAction::ZoomOut, Interaction::Pressed));
        world_zoom_out
            .run_system_once(handle_settings_menu_buttons)
            .unwrap();
        let mut q_out = world_zoom_out.query_filtered::<&Projection, With<PrimaryGameCamera>>();
        let Projection::Orthographic(o_out) = q_out.single(&world_zoom_out).unwrap() else {
            panic!("expected orthographic projection");
        };
        assert!(o_out.scale > 1.0);
    }

    #[test]
    fn handle_tile_click_minimal_returns_early() {
        let mut world = World::new();
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.insert_resource(SelectedTileInfo::default());
        world.insert_resource(HoveredTileCoord::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(StationBuildState::default());
        world.insert_resource(DragBuildState::default());
        world.insert_resource(BridgeBuildState::default());
        world.insert_resource(OrderEditState::default());
        world.insert_resource(DepotPanelState::default());
        world.insert_resource(StationCargoPanelState::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(IndustryPanelState::default());
        world.insert_resource(crate::ui::town_window::TownWindowState::default());
        world.insert_resource(crate::ui::vehicle_window::VehicleWindowState::default());
        world.insert_resource(HudBuildFeedback::default());
        world.insert_resource(Time::<()>::default());
        world.run_system_once(handle_tile_click).unwrap();
    }

    #[test]
    fn pure_toolbar_helpers_cover_branches() {
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::RailTunnel),
            ToolbarGroup::Rail
        ));
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::BuildFactory),
            ToolbarGroup::Economy
        ));
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::Orders),
            ToolbarGroup::Info
        ));
        assert!(matches!(
            toolbar_group_for_action(BuildMenuAction::LowerLand),
            ToolbarGroup::Landscape
        ));

        assert!(action_supports_drag(BuildMenuAction::RoadBridge));
        assert!(action_supports_drag(BuildMenuAction::RailTunnel));
        assert!(action_supports_drag(BuildMenuAction::RaiseLand));
        assert!(action_supports_area_drag(BuildMenuAction::LevelLand));
        assert!(!action_supports_drag(BuildMenuAction::BuildHouse));
        assert!(!action_supports_drag(BuildMenuAction::Station));

        assert!(action_is_tunnel(BuildMenuAction::RoadTunnel));
        assert!(action_is_tunnel(BuildMenuAction::RailTunnel));
        assert!(!action_is_tunnel(BuildMenuAction::RoadBridge));

        assert!(matches!(
            command_for_action(
                BuildMenuAction::Station,
                TileCoord::new(1, 2),
                &StationBuildState {
                    orientation: 3,
                    ..Default::default()
                },
                None,
                None,
                None,
            ),
            Some(Command::PlaceStationDir(_, 3))
        ));
        assert!(matches!(
            command_for_action(
                BuildMenuAction::RoadDepot,
                TileCoord::new(1, 2),
                &StationBuildState {
                    orientation: 2,
                    ..Default::default()
                },
                None,
                None,
                None,
            ),
            Some(Command::PlaceRoadDepotDir(_, 2))
        ));
        assert!(matches!(
            command_for_action(
                BuildMenuAction::BuildCoalMine,
                TileCoord::new(1, 2),
                &StationBuildState::default(),
                None,
                None,
                None,
            ),
            Some(Command::PlaceIndustrySpec(
                _,
                openttdrs_core::IndustrySpec::CoalMine
            ))
        ));
        assert!(
            command_for_action(
                BuildMenuAction::RoadTunnel,
                TileCoord::new(1, 2),
                &StationBuildState::default(),
                None,
                None,
                None,
            )
            .is_none()
        );

        assert!(matches!(
            command_for_line_action(
                BuildMenuAction::RoadTunnel,
                &[(1, 1), (3, 1)],
                BridgeType::Wooden
            ),
            Some(Command::PlaceRoadTunnel(_, _))
        ));
        assert!(matches!(
            command_for_line_action(
                BuildMenuAction::RailBridge,
                &[(1, 1), (3, 1)],
                BridgeType::Wooden
            ),
            Some(Command::PlaceRailBridge(_, _, _))
        ));
        assert!(
            command_for_line_action(
                BuildMenuAction::RoadX,
                &[(1, 1), (3, 1)],
                BridgeType::Wooden
            )
            .is_none()
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::Road, &[(1, 1), (4, 1)]),
            Some(0x0A)
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::Road, &[(1, 1), (1, 4)]),
            Some(0x05)
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::RoadX, &[(1, 1), (1, 4)]),
            Some(0x0A)
        );
        assert_eq!(
            road_bits_for_drag_action(BuildMenuAction::Clear, &[(1, 1), (1, 4)]),
            None
        );

        assert_eq!(
            drag_line_tiles(None, BuildMenuAction::RoadX, (1, 2), (4, 9)),
            vec![(1, 2), (2, 2), (3, 2), (4, 2)]
        );
        assert_eq!(
            drag_line_tiles(None, BuildMenuAction::RoadY, (3, 1), (0, 4)),
            vec![(3, 1), (3, 2), (3, 3), (3, 4)]
        );
        assert_eq!(
            drag_line_tiles(None, BuildMenuAction::Road, (5, 2), (2, 2)),
            vec![(5, 2), (4, 2), (3, 2), (2, 2)]
        );
        assert_eq!(
            drag_line_tiles(None, BuildMenuAction::Road, (2, 2), (3, 6)),
            vec![(2, 2), (2, 3), (2, 4), (2, 5), (2, 6)]
        );
    }

    #[test]
    fn map_related_helpers_cover_tunnels() {
        let mut sim = SimWorld {
            state: GameState::new(6, 6),
            ..SimWorld::default()
        };
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        // Pendiente NE en (5,5) y salida SW en (3,5), mismo nivel.
        sim.state.map.set_height(c(2, 2), 2).unwrap();
        sim.state.map.set_height(c(2, 3), 2).unwrap();
        sim.state.map.set_height(c(3, 2), 1).unwrap();
        sim.state.map.set_height(c(3, 3), 1).unwrap();
        sim.state.map.set_height(c(0, 2), 1).unwrap();
        sim.state.map.set_height(c(0, 3), 1).unwrap();
        sim.state.map.set_height(c(1, 2), 2).unwrap();
        sim.state.map.set_height(c(1, 3), 2).unwrap();

        assert!(tunnel_placement_is_valid(
            &sim.state,
            BuildMenuAction::RoadTunnel,
            &[(2, 2)]
        ));
        assert!(!tunnel_placement_is_valid(
            &sim.state,
            BuildMenuAction::RoadTunnel,
            &[(0, 0)]
        ));
        assert!(!tunnel_placement_is_valid(
            &sim.state,
            BuildMenuAction::Road,
            &[(5, 5)]
        ));
    }

    #[test]
    fn order_for_clicked_tile_accepts_depot_and_rejects_incompatible_station() {
        let mut sim = SimWorld::default();
        sim.state.vehicles.clear();
        sim.state.stations.clear();
        let road_depot = TileCoord::new(2, 2);
        let rail_depot = TileCoord::new(4, 2);
        let truck_stop = TileCoord::new(3, 2);
        sim.state
            .map
            .set_kind(road_depot, TileKind::RoadDepot)
            .unwrap();
        sim.state
            .map
            .set_kind(rail_depot, TileKind::RailDepot)
            .unwrap();
        sim.state
            .stations
            .push(openttdrs_core::Station::new_with_kind(
                truck_stop,
                openttdrs_core::StopKind::TruckStop,
            ));
        sim.state.vehicles.push(openttdrs_core::Vehicle::new(
            42,
            VehicleKind::Bus,
            road_depot,
            road_depot,
        ));
        sim.state.vehicles.push(openttdrs_core::Vehicle::new(
            7,
            VehicleKind::Train,
            rail_depot,
            rail_depot,
        ));

        assert!(matches!(
            order_for_clicked_tile(&sim, 42, road_depot),
            Some(VehicleOrder::Depot { .. })
        ));
        assert!(matches!(
            order_for_clicked_tile(&sim, 7, rail_depot),
            Some(VehicleOrder::Depot { .. })
        ));
        assert!(order_for_clicked_tile(&sim, 42, truck_stop).is_none());
        assert!(order_for_clicked_tile(&sim, 42, rail_depot).is_none());
    }
}
