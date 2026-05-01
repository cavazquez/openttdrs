use super::build_input::cancel_placement;
use super::{
    BuildMenuAction, DragBuildState, ToolButtonGroup, ToolSelectButton, ToolbarCloseButton,
    ToolbarGroup, ToolbarGroupButton, ToolbarState, ToolbarTooltipTarget, TooltipBox, TooltipText,
    UiToolState,
};
use bevy::prelude::*;

pub(crate) fn toolbar_group_interaction(
    mut q: Query<(&Interaction, &ToolbarGroup), (Changed<Interaction>, With<ToolbarGroupButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for (interaction, group) in &mut q {
        if *interaction == Interaction::Pressed {
            if toolbar_state.active_group == Some(*group) {
                toolbar_state.active_group = None;
                tool_state.active_tool = None;
                cancel_placement(&mut drag_state);
            } else {
                toolbar_state.active_group = Some(*group);
            }
        }
    }
}

pub(crate) fn update_toolbar_group_visuals(
    toolbar_state: Res<ToolbarState>,
    mut q: Query<
        (
            &ToolbarGroup,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<ToolbarGroupButton>,
    >,
) {
    for (group, interaction, mut bg, mut border) in &mut q {
        let is_active = Some(*group) == toolbar_state.active_group;
        *bg = if is_active && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.78, 0.68, 0.43))
        } else if is_active && *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.7, 0.61, 0.38))
        } else if Some(*group) == toolbar_state.active_group {
            BackgroundColor(Color::srgb(0.62, 0.54, 0.34))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.42, 0.36, 0.24))
        } else {
            BackgroundColor(Color::srgb(0.33, 0.28, 0.19))
        };
        *border = if is_active {
            BorderColor::all(Color::srgb(0.86, 0.76, 0.5))
        } else {
            BorderColor::all(Color::srgb(0.64, 0.57, 0.39))
        };
    }
}

pub(crate) fn update_toolbar_tool_visibility(
    toolbar_state: Res<ToolbarState>,
    mut q: Query<(&ToolButtonGroup, &mut Node)>,
) {
    if !toolbar_state.is_changed() {
        return;
    }
    for (tool_group, mut node) in &mut q {
        node.display = if Some(tool_group.0) == toolbar_state.active_group {
            Display::Flex
        } else {
            Display::None
        };
        let offset = match toolbar_state.active_group {
            Some(ToolbarGroup::Rail) => -112.0,
            Some(ToolbarGroup::Road) => -56.0,
            Some(ToolbarGroup::Economy) => 0.0,
            Some(ToolbarGroup::Info) => 56.0,
            Some(ToolbarGroup::Settings) => 112.0,
            None => 0.0,
        };
        node.margin.left = Val::Px(offset);
    }
}

pub(crate) fn close_toolbar_panel_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

pub(crate) fn close_toolbar_button_interaction(
    q: Query<&Interaction, (Changed<Interaction>, With<ToolbarCloseButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        toolbar_state.active_group = None;
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

fn toolbar_group_for_action(action: BuildMenuAction) -> ToolbarGroup {
    match action {
        BuildMenuAction::Rail
        | BuildMenuAction::RailDepot
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel => ToolbarGroup::Rail,
        BuildMenuAction::Road
        | BuildMenuAction::RoadX
        | BuildMenuAction::RoadY
        | BuildMenuAction::RoadDepot
        | BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::BusStop
        | BuildMenuAction::Station
        | BuildMenuAction::Clear => ToolbarGroup::Road,
        BuildMenuAction::Orders => ToolbarGroup::Info,
        BuildMenuAction::BuildHouse
        | BuildMenuAction::BuildCoalMine
        | BuildMenuAction::BuildIronOreMine
        | BuildMenuAction::BuildGoldMine
        | BuildMenuAction::BuildOilWell
        | BuildMenuAction::BuildOilRefinery
        | BuildMenuAction::BuildFactory
        | BuildMenuAction::BuildSawmill
        | BuildMenuAction::BuildForest
        | BuildMenuAction::BuildFarm => ToolbarGroup::Economy,
    }
}

pub(crate) fn hide_tool_when_panel_closed(
    toolbar_state: Res<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    let Some(action) = tool_state.active_tool else {
        return;
    };
    if toolbar_state.active_group != Some(toolbar_group_for_action(action)) {
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

/// El boton del menu selecciona la herramienta activa para aplicar en el mapa.
#[allow(clippy::type_complexity)]
pub(crate) fn build_menu_interaction(
    mut q: Query<(&Interaction, &BuildMenuAction), (Changed<Interaction>, With<Button>)>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        tool_state.active_tool = Some(*action);
        cancel_placement(&mut drag_state);
    }
}

/// Resalta el boton de herramienta actualmente activo.
pub(crate) fn update_tool_button_visuals(
    tool_state: Res<UiToolState>,
    mut q: Query<
        (
            &BuildMenuAction,
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<ToolSelectButton>,
    >,
) {
    for (action, interaction, mut bg, mut border) in &mut q {
        let is_active = tool_state
            .active_tool
            .is_some_and(|active| active == *action);
        *bg = if is_active && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.76, 0.67, 0.42))
        } else if is_active && *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.68, 0.59, 0.37))
        } else if is_active {
            BackgroundColor(Color::srgb(0.6, 0.52, 0.33))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.4, 0.34, 0.23))
        } else {
            BackgroundColor(Color::srgb(0.28, 0.24, 0.16))
        };
        *border = if is_active {
            BorderColor::all(Color::srgb(0.84, 0.74, 0.5))
        } else {
            BorderColor::all(Color::srgb(0.64, 0.57, 0.39))
        };
    }
}

pub(crate) fn update_toolbar_tooltip(
    mut tooltip_q: Query<&mut Node, With<TooltipBox>>,
    mut text_q: Query<&mut Text, With<TooltipText>>,
    target_q: Query<(&Interaction, &ToolbarTooltipTarget)>,
) {
    let mut hovered: Option<&'static str> = None;
    for (interaction, tip) in &target_q {
        if *interaction == Interaction::Hovered {
            hovered = Some(tip.text);
            break;
        }
    }

    let Ok(mut tooltip_text) = text_q.single_mut() else {
        return;
    };
    let Ok(mut node) = tooltip_q.single_mut() else {
        return;
    };

    if let Some(text) = hovered {
        **tooltip_text = text.to_string();
        node.display = Display::Flex;
    } else {
        node.display = Display::None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;
    use openttdrs_core::{Command, Map, TileCoord, TileKind, VehicleKind, VehicleOrder};

    use crate::render::{PrimaryGameCamera, RemapMapVisualsPending, VehicleIndex};
    use crate::state::SimWorld;
    use crate::ui::hud::{SelectedTileInfo, SimHudControls};
    use crate::ui::industry_panel::IndustryPanelState;
    use crate::ui::toolbar::build_input::{
        action_is_tunnel, action_supports_drag, command_for_action, command_for_line_action,
        drag_line_tiles, order_for_clicked_tile, road_bits_for_drag_action,
        tunnel_placement_is_valid,
    };
    use crate::ui::toolbar::{
        BuildMenuUi, DepotPanelState, OrderEditState, SaveMenuAction, StationBuildState,
        StationCargoPanelState, handle_minimap_click, handle_order_panel_buttons,
        handle_settings_menu_buttons, handle_tile_click, setup_minimap, setup_order_panel,
        sync_minimap, sync_order_panel,
    };

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
        world
            .run_system_once(close_toolbar_panel_on_escape)
            .unwrap();
    }

    #[test]
    fn hide_tool_mismatch_group_clears_tool() {
        let mut world = World::new();
        world.insert_resource(ToolbarState {
            active_group: Some(ToolbarGroup::Road),
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
    fn handle_minimap_click_ignored_when_ui_is_interacting() {
        let mut world = World::new();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SimWorld::default());
        world.spawn((BuildMenuUi, Interaction::Pressed));
        world.run_system_once(handle_minimap_click).unwrap();
    }

    #[test]
    fn setup_order_panel_then_sync_order_panel() {
        let mut world = World::new();
        world.run_system_once(setup_order_panel).unwrap();
        world.insert_resource(OrderEditState::default());
        world.insert_resource(SimWorld::default());
        world.run_system_once(sync_order_panel).unwrap();
    }

    #[test]
    fn toolbar_interaction_systems_run_with_empty_queries() {
        let mut world = World::new();
        world.insert_resource(ToolbarState::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(DragBuildState::default());
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
        world.insert_resource(SimWorld::default());
        world.run_system_once(handle_order_panel_buttons).unwrap();
    }

    #[test]
    fn handle_settings_menu_buttons_save_load_with_file_dialog_abstraction() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("sim.json");

        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SimHudControls {
            paused: false,
            sim_speed: 1.0,
            json_save_path: save_path.to_string_lossy().to_string(),
            minimap_visible: true,
        });

        world.spawn((Button, SaveMenuAction::SaveAs, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();

        world.spawn((Button, SaveMenuAction::LoadFrom, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        let remap = world.resource::<RemapMapVisualsPending>();
        assert!(remap.pending);
        assert!(remap.sync_camera);
    }

    #[test]
    fn handle_settings_menu_buttons_pause_speed_and_zoom() {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
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
        world.insert_resource(SimWorld::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(StationBuildState::default());
        world.insert_resource(DragBuildState::default());
        world.insert_resource(OrderEditState::default());
        world.insert_resource(DepotPanelState::default());
        world.insert_resource(StationCargoPanelState::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(IndustryPanelState::default());
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

        assert!(action_supports_drag(BuildMenuAction::RoadBridge));
        assert!(action_supports_drag(BuildMenuAction::RailTunnel));
        assert!(action_supports_drag(BuildMenuAction::Clear));
        assert!(!action_supports_drag(BuildMenuAction::BuildHouse));
        assert!(!action_supports_drag(BuildMenuAction::Station));

        assert!(action_is_tunnel(BuildMenuAction::RoadTunnel));
        assert!(action_is_tunnel(BuildMenuAction::RailTunnel));
        assert!(!action_is_tunnel(BuildMenuAction::RoadBridge));

        assert!(matches!(
            command_for_action(
                BuildMenuAction::Station,
                TileCoord::new(1, 2),
                &StationBuildState { orientation: 3 }
            ),
            Some(Command::PlaceStationDir(_, 3))
        ));
        assert!(matches!(
            command_for_action(
                BuildMenuAction::RoadDepot,
                TileCoord::new(1, 2),
                &StationBuildState { orientation: 2 }
            ),
            Some(Command::PlaceRoadDepotDir(_, 2))
        ));
        assert!(matches!(
            command_for_action(
                BuildMenuAction::BuildCoalMine,
                TileCoord::new(1, 2),
                &StationBuildState::default()
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
                &StationBuildState::default()
            )
            .is_none()
        );

        assert!(matches!(
            command_for_line_action(BuildMenuAction::RoadTunnel, &[(1, 1), (3, 1)]),
            Some(Command::PlaceRoadTunnel(_, _))
        ));
        assert!(matches!(
            command_for_line_action(BuildMenuAction::RailBridge, &[(1, 1), (3, 1)]),
            Some(Command::PlaceRailBridge(_, _))
        ));
        assert!(command_for_line_action(BuildMenuAction::RoadX, &[(1, 1), (3, 1)]).is_none());
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
            drag_line_tiles(BuildMenuAction::RoadX, (1, 2), (4, 9)),
            vec![(1, 2), (2, 2), (3, 2), (4, 2)]
        );
        assert_eq!(
            drag_line_tiles(BuildMenuAction::RoadY, (3, 1), (0, 4)),
            vec![(3, 1), (3, 2), (3, 3), (3, 4)]
        );
        assert_eq!(
            drag_line_tiles(BuildMenuAction::Road, (5, 2), (2, 2)),
            vec![(5, 2), (4, 2), (3, 2), (2, 2)]
        );
        assert_eq!(
            drag_line_tiles(BuildMenuAction::Road, (2, 2), (3, 6)),
            vec![(2, 2), (2, 3), (2, 4), (2, 5), (2, 6)]
        );
    }

    #[test]
    fn map_related_helpers_cover_tunnels() {
        let mut map = Map::new_flat(6, 6, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        map.set_height(c(1, 1), 2).unwrap();
        map.set_height(c(3, 1), 2).unwrap();
        map.set_kind(c(1, 1), TileKind::Road).unwrap();
        map.set_kind(c(2, 1), TileKind::Road).unwrap();
        map.set_kind(c(3, 1), TileKind::Road).unwrap();
        map.set_kind(c(4, 1), TileKind::Water).unwrap();

        assert!(!tunnel_placement_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1)]
        ));
        assert!(tunnel_placement_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (3, 1)]
        ));
        assert!(!tunnel_placement_is_valid(
            &map,
            BuildMenuAction::RoadTunnel,
            &[(1, 1), (2, 1), (4, 1)]
        ));
        assert!(!tunnel_placement_is_valid(
            &map,
            BuildMenuAction::Road,
            &[(1, 1), (2, 1)]
        ));
    }

    #[test]
    fn order_for_clicked_tile_accepts_depot_and_rejects_incompatible_station() {
        let mut sim = SimWorld::default();
        sim.state.vehicles.clear();
        sim.state.stations.clear();
        let depot = TileCoord::new(2, 2);
        let truck_stop = TileCoord::new(3, 2);
        sim.state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        sim.state
            .stations
            .push(openttdrs_core::Station::new_with_kind(
                truck_stop,
                openttdrs_core::StopKind::TruckStop,
            ));
        sim.state.vehicles.push(openttdrs_core::Vehicle::new(
            42,
            VehicleKind::Bus,
            depot,
            depot,
        ));

        assert!(matches!(
            order_for_clicked_tile(&sim, 42, depot),
            Some(VehicleOrder::Tile(_))
        ));
        assert!(order_for_clicked_tile(&sim, 42, truck_stop).is_none());
    }
}
