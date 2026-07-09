use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::preview::economy_industry_tool_visible;
use crate::ui::toolbar::{
    BuildMenuAction, DragBuildState, ToolButtonGroup, ToolSelectButton, ToolbarGroup,
    ToolbarGroupButton, ToolbarState, UiToolState,
};

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
            Some(ToolbarGroup::Rail) => -196.0,
            Some(ToolbarGroup::Road) => -140.0,
            Some(ToolbarGroup::Water) => -84.0,
            Some(ToolbarGroup::Air) => -28.0,
            Some(ToolbarGroup::Economy) => 28.0,
            Some(ToolbarGroup::Landscape) => 84.0,
            Some(ToolbarGroup::Info) => 140.0,
            Some(ToolbarGroup::Settings) => 196.0,
            None => 0.0,
        };
        node.margin.left = Val::Px(offset);
    }
}

pub(crate) fn toolbar_group_for_action(action: BuildMenuAction) -> ToolbarGroup {
    match action {
        BuildMenuAction::Rail
        | BuildMenuAction::RailX
        | BuildMenuAction::RailY
        | BuildMenuAction::RailHorz
        | BuildMenuAction::RailVert
        | BuildMenuAction::RailDepot
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::RailStation
        | BuildMenuAction::RailWaypoint
        | BuildMenuAction::RailSignals
        | BuildMenuAction::RailRemove
        | BuildMenuAction::RailConvert => ToolbarGroup::Rail,
        BuildMenuAction::Road
        | BuildMenuAction::RoadX
        | BuildMenuAction::RoadY
        | BuildMenuAction::RoadDepot
        | BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::BusStop
        | BuildMenuAction::Station
        | BuildMenuAction::Clear => ToolbarGroup::Road,
        BuildMenuAction::ShipDepot
        | BuildMenuAction::Dock
        | BuildMenuAction::Canal
        | BuildMenuAction::Lock => ToolbarGroup::Water,
        BuildMenuAction::Airport | BuildMenuAction::AirportSmall => ToolbarGroup::Air,
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
        | BuildMenuAction::BuildFarm
        | BuildMenuAction::BuildCottonCandy
        | BuildMenuAction::BuildCandyFactory
        | BuildMenuAction::BuildBatteryFarm
        | BuildMenuAction::BuildColaWells
        | BuildMenuAction::BuildToyFactory
        | BuildMenuAction::BuildPlasticFountain
        | BuildMenuAction::BuildFizzyDrinkFactory
        | BuildMenuAction::BuildBubbleGenerator
        | BuildMenuAction::BuildToffeeQuarry
        | BuildMenuAction::BuildSugarMine => ToolbarGroup::Economy,
        BuildMenuAction::RaiseLand
        | BuildMenuAction::LowerLand
        | BuildMenuAction::LevelLand
        | BuildMenuAction::BuyLand => ToolbarGroup::Landscape,
    }
}

/// Demoler (`Clear`) comparte botón en paneles carretera y ferrocarril.
fn tool_compatible_with_panel(action: BuildMenuAction, active: Option<ToolbarGroup>) -> bool {
    let Some(active) = active else {
        return false;
    };
    if action == BuildMenuAction::Clear {
        return matches!(active, ToolbarGroup::Road | ToolbarGroup::Rail);
    }
    toolbar_group_for_action(action) == active
}

pub(crate) fn hide_tool_when_panel_closed(
    toolbar_state: Res<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
) {
    let Some(action) = tool_state.active_tool else {
        return;
    };
    if !tool_compatible_with_panel(action, toolbar_state.active_group) {
        tool_state.active_tool = None;
        cancel_placement(&mut drag_state);
    }
}

pub(crate) fn sync_climate_industry_tools(
    sim: Res<SimWorld>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    mut q: Query<(&BuildMenuAction, &mut Node), With<ToolSelectButton>>,
) {
    let climate = sim.state.climate;
    for (action, mut node) in &mut q {
        if toolbar_group_for_action(*action) != ToolbarGroup::Economy {
            continue;
        }
        let visible = economy_industry_tool_visible(*action, climate);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if !visible
            && tool_state
                .active_tool
                .is_some_and(|active| active == *action)
        {
            tool_state.active_tool = None;
            cancel_placement(&mut drag_state);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;

    #[test]
    fn clear_tool_stays_active_on_rail_panel() {
        let mut world = World::new();
        world.insert_resource(ToolbarState {
            active_group: Some(ToolbarGroup::Rail),
            ..Default::default()
        });
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::Clear),
        });
        world.insert_resource(DragBuildState::default());
        world.run_system_once(hide_tool_when_panel_closed).unwrap();
        assert_eq!(
            world.resource::<UiToolState>().active_tool,
            Some(BuildMenuAction::Clear)
        );
    }

    #[test]
    fn rail_tool_cleared_when_road_panel_open() {
        let mut world = World::new();
        world.insert_resource(ToolbarState {
            active_group: Some(ToolbarGroup::Road),
            ..Default::default()
        });
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::RailSignals),
        });
        world.insert_resource(DragBuildState::default());
        world.run_system_once(hide_tool_when_panel_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }
}
