//! Pickers de construcción pendientes por integrar (greenfield).

use bevy::prelude::*;

use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;

use super::{BuildMenuAction, BuildMenuUi, UiToolState};

#[derive(Component)]
struct PickerDescription;

fn setup_text_picker(
    commands: &mut Commands,
    asset_server: &AssetServer,
    id: FloatingWindowId,
    title: &str,
    body: &str,
    size: Vec2,
) {
    let (_root, content) =
        spawn_floating_window(commands, asset_server, id, title, TITLE_BROWN, size, 320.0);
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            PickerDescription,
            Text::new(body),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            BuildMenuUi,
        ));
    });
}

fn sync_picker(
    tool_state: Res<UiToolState>,
    expected: BuildMenuAction,
    id: FloatingWindowId,
    title: &str,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    let Some((_, mut visibility)) = root_q.iter_mut().find(|(window, _)| window.id == id) else {
        return;
    };
    let open = tool_state.active_tool == Some(expected);
    *visibility = if open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !open {
        return;
    }
    if let Some((_, mut title_text)) = title_q
        .iter_mut()
        .find(|(title_text, _)| title_text.0 == id)
    {
        **title_text = title.to_string();
    }
}

fn picker_closed(
    id: FloatingWindowId,
    expected: BuildMenuAction,
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0.class == id && tool_state.active_tool == Some(expected) {
            tool_state.active_tool = None;
        }
    }
}

pub(crate) fn setup_dock_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::DockPicker,
        "Muelle",
        "Selecciona el tipo de muelle y orientación desde el mapa al colocar.",
        Vec2::new(258.0, 120.0),
    );
}

pub(crate) fn sync_dock_picker(
    tool_state: Res<UiToolState>,
    root_q: Query<(&FloatingWindow, &mut Visibility)>,
    title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    sync_picker(
        tool_state,
        BuildMenuAction::Dock,
        FloatingWindowId::DockPicker,
        "Muelle",
        root_q,
        title_q,
    );
}

pub(crate) fn dock_picker_on_closed(
    closed: MessageReader<FloatingWindowClosed>,
    tool_state: ResMut<UiToolState>,
) {
    picker_closed(
        FloatingWindowId::DockPicker,
        BuildMenuAction::Dock,
        closed,
        tool_state,
    );
}

pub(crate) fn setup_buoy_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::BuoyPicker,
        "Boya",
        "Selecciona la boya para abrir rutas de navegación.",
        Vec2::new(258.0, 96.0),
    );
}

pub(crate) fn sync_buoy_picker(
    tool_state: Res<UiToolState>,
    root_q: Query<(&FloatingWindow, &mut Visibility)>,
    title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    sync_picker(
        tool_state,
        BuildMenuAction::Buoy,
        FloatingWindowId::BuoyPicker,
        "Boya",
        root_q,
        title_q,
    );
}

pub(crate) fn buoy_picker_on_closed(
    closed: MessageReader<FloatingWindowClosed>,
    tool_state: ResMut<UiToolState>,
) {
    picker_closed(
        FloatingWindowId::BuoyPicker,
        BuildMenuAction::Buoy,
        closed,
        tool_state,
    );
}

pub(crate) fn setup_rail_waypoint_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::RailWaypointPicker,
        "Waypoint ferroviario",
        "Selecciona opciones de waypoint de tren antes de colocar.",
        Vec2::new(284.0, 96.0),
    );
}

pub(crate) fn sync_rail_waypoint_picker(
    tool_state: Res<UiToolState>,
    root_q: Query<(&FloatingWindow, &mut Visibility)>,
    title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    sync_picker(
        tool_state,
        BuildMenuAction::RailWaypoint,
        FloatingWindowId::RailWaypointPicker,
        "Waypoint ferroviario",
        root_q,
        title_q,
    );
}

pub(crate) fn rail_waypoint_picker_on_closed(
    closed: MessageReader<FloatingWindowClosed>,
    tool_state: ResMut<UiToolState>,
) {
    picker_closed(
        FloatingWindowId::RailWaypointPicker,
        BuildMenuAction::RailWaypoint,
        closed,
        tool_state,
    );
}

pub(crate) fn setup_road_waypoint_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::RoadWaypointPicker,
        "Waypoint de carretera",
        "Selecciona opciones de waypoint de carretera antes de colocar.",
        Vec2::new(284.0, 96.0),
    );
}

pub(crate) fn sync_road_waypoint_picker(
    tool_state: Res<UiToolState>,
    root_q: Query<(&FloatingWindow, &mut Visibility)>,
    title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    sync_picker(
        tool_state,
        BuildMenuAction::RoadWaypoint,
        FloatingWindowId::RoadWaypointPicker,
        "Waypoint de carretera",
        root_q,
        title_q,
    );
}

pub(crate) fn road_waypoint_picker_on_closed(
    closed: MessageReader<FloatingWindowClosed>,
    tool_state: ResMut<UiToolState>,
) {
    picker_closed(
        FloatingWindowId::RoadWaypointPicker,
        BuildMenuAction::RoadWaypoint,
        closed,
        tool_state,
    );
}

pub(crate) fn setup_tree_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::TreePicker,
        "Arbolado",
        "Ajusta el tipo o estado del árbol en paisaje si aplica en tu versión.",
        Vec2::new(260.0, 96.0),
    );
}

pub(crate) fn sync_tree_picker(
    tool_state: Res<UiToolState>,
    root_q: Query<(&FloatingWindow, &mut Visibility)>,
    title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    sync_picker(
        tool_state,
        BuildMenuAction::PlantTree,
        FloatingWindowId::TreePicker,
        "Arbolado",
        root_q,
        title_q,
    );
}

pub(crate) fn tree_picker_on_closed(
    closed: MessageReader<FloatingWindowClosed>,
    tool_state: ResMut<UiToolState>,
) {
    picker_closed(
        FloatingWindowId::TreePicker,
        BuildMenuAction::PlantTree,
        closed,
        tool_state,
    );
}

pub(crate) fn setup_terraform_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::TerraformPicker,
        "Terraform",
        "Ajusta opción de elevación para el comando de terreno activo.",
        Vec2::new(260.0, 96.0),
    );
}

pub(crate) fn sync_terraform_picker(
    tool_state: Res<UiToolState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    let open = matches!(
        tool_state.active_tool,
        Some(BuildMenuAction::RaiseLand)
            | Some(BuildMenuAction::LowerLand)
            | Some(BuildMenuAction::LevelLand)
            | Some(BuildMenuAction::BuyLand)
    );
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::TerraformPicker)
    else {
        return;
    };
    *visibility = if open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !open {
        return;
    }
    if let Some((_, mut title_text)) = title_q
        .iter_mut()
        .find(|(title_text, _)| title_text.0 == FloatingWindowId::TerraformPicker)
    {
        **title_text = "Terraform".to_string();
    }
}

pub(crate) fn terraform_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::TerraformPicker {
            continue;
        }
        if matches!(
            tool_state.active_tool,
            Some(BuildMenuAction::RaiseLand)
                | Some(BuildMenuAction::LowerLand)
                | Some(BuildMenuAction::LevelLand)
                | Some(BuildMenuAction::BuyLand)
        ) {
            tool_state.active_tool = None;
            return;
        }
    }
}

pub(crate) fn setup_sign_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::SignPicker,
        "Cartel",
        "Configura texto y estilo de cartel desde la interfaz de edición.",
        Vec2::new(300.0, 96.0),
    );
}

pub(crate) fn sync_sign_picker(
    tool_state: Res<UiToolState>,
    root_q: Query<(&FloatingWindow, &mut Visibility)>,
    title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    sync_picker(
        tool_state,
        BuildMenuAction::PlaceSign,
        FloatingWindowId::SignPicker,
        "Cartel",
        root_q,
        title_q,
    );
}

pub(crate) fn sign_picker_on_closed(
    closed: MessageReader<FloatingWindowClosed>,
    tool_state: ResMut<UiToolState>,
) {
    picker_closed(
        FloatingWindowId::SignPicker,
        BuildMenuAction::PlaceSign,
        closed,
        tool_state,
    );
}

pub(crate) fn setup_depot_build_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    setup_text_picker(
        &mut commands,
        &asset_server,
        FloatingWindowId::DepotBuildPicker,
        "Depósito",
        "Selecciona el tipo de depósito en construcción.",
        Vec2::new(260.0, 96.0),
    );
}

pub(crate) fn sync_depot_build_picker(
    tool_state: Res<UiToolState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
) {
    let open = matches!(
        tool_state.active_tool,
        Some(BuildMenuAction::RoadDepot)
            | Some(BuildMenuAction::RailDepot)
            | Some(BuildMenuAction::ShipDepot)
    );
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::DepotBuildPicker)
    else {
        return;
    };
    *visibility = if open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !open {
        return;
    }
    if let Some((_, mut title_text)) = title_q
        .iter_mut()
        .find(|(title_text, _)| title_text.0 == FloatingWindowId::DepotBuildPicker)
    {
        **title_text = "Depósito".to_string();
    }
}

pub(crate) fn depot_build_picker_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tool_state: ResMut<UiToolState>,
) {
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::DepotBuildPicker {
            continue;
        }
        if matches!(
            tool_state.active_tool,
            Some(BuildMenuAction::RoadDepot)
                | Some(BuildMenuAction::RailDepot)
                | Some(BuildMenuAction::ShipDepot)
        ) {
            tool_state.active_tool = None;
            return;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn dock_picker_on_closed_clears_active_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::Dock),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::DockPicker),
        ));
        world.run_system_once(dock_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }
}
