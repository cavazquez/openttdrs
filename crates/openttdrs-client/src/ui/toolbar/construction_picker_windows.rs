//! Pickers greenfield de construcción (#270): dock, buoy, waypoints, landscaping, depósitos.

use bevy::prelude::*;

use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;

use super::{BuildMenuAction, BuildMenuUi, StationBuildState, UiToolState};

const BTN_BG: Color = Color::srgb(0.22, 0.20, 0.16);
const BTN_BORDER: Color = Color::srgb(0.45, 0.40, 0.30);
const BTN_SELECTED: Color = Color::srgb(0.40, 0.32, 0.18);

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DockOrientButton {
    North,
    East,
    South,
    West,
}

impl DockOrientButton {
    const ALL: [(Self, &'static str); 4] = [
        (Self::North, "N"),
        (Self::East, "E"),
        (Self::South, "S"),
        (Self::West, "O"),
    ];

    fn as_orientation(self) -> u8 {
        match self {
            Self::North => 0,
            Self::East => 1,
            Self::South => 2,
            Self::West => 3,
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DepotKindButton {
    Road,
    Rail,
    Ship,
}

impl DepotKindButton {
    const ALL: [(Self, &'static str); 3] = [
        (Self::Road, "Carretera"),
        (Self::Rail, "Tren"),
        (Self::Ship, "Barco"),
    ];

    fn as_tool(self) -> BuildMenuAction {
        match self {
            Self::Road => BuildMenuAction::RoadDepot,
            Self::Rail => BuildMenuAction::RailDepot,
            Self::Ship => BuildMenuAction::ShipDepot,
        }
    }
}

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

fn spawn_chip<B: Component + Clone>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    button: B,
    label: &str,
) {
    parent.spawn((
        Button,
        button,
        Node {
            min_width: Val::Px(52.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
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
    let (_root, content) = spawn_floating_window(
        &mut commands,
        &asset_server,
        FloatingWindowId::DockPicker,
        "Muelle",
        TITLE_BROWN,
        Vec2::new(258.0, 120.0),
        320.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            Text::new("Orientación del muelle"),
            window_text_font(&asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                for (btn, label) in DockOrientButton::ALL {
                    spawn_chip(row, &asset_server, btn, label);
                }
            });
    });
}

pub(crate) fn sync_dock_picker(
    tool_state: Res<UiToolState>,
    station_state: Res<StationBuildState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut buttons: Query<(&DockOrientButton, &mut BackgroundColor), With<Button>>,
) {
    let open = tool_state.active_tool == Some(BuildMenuAction::Dock);
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::DockPicker)
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
        .find(|(title_text, _)| title_text.0 == FloatingWindowId::DockPicker)
    {
        **title_text = "Muelle".to_string();
    }
    for (btn, mut bg) in &mut buttons {
        *bg = BackgroundColor(if btn.as_orientation() == station_state.orientation % 4 {
            BTN_SELECTED
        } else {
            BTN_BG
        });
    }
}

pub(crate) fn handle_dock_picker_buttons(
    buttons: Query<(&Interaction, &DockOrientButton), (Changed<Interaction>, With<Button>)>,
    mut station_state: ResMut<StationBuildState>,
    tool_state: Res<UiToolState>,
) {
    if tool_state.active_tool != Some(BuildMenuAction::Dock) {
        return;
    }
    for (interaction, btn) in &buttons {
        if *interaction == Interaction::Pressed {
            station_state.orientation = btn.as_orientation();
        }
    }
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
        "Coloca la boya en agua navegable para abrir rutas.",
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
        "Coloca el waypoint sobre vía férrea recta.",
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
        "Coloca el waypoint sobre carretera recta.",
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
        "Planta o hace crecer árboles en hierba / bosque.",
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
        "Elevar, bajar, nivelar o comprar terreno con la herramienta activa.",
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
        "Coloca un cartel de texto en el mapa.",
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
    let (_root, content) = spawn_floating_window(
        &mut commands,
        &asset_server,
        FloatingWindowId::DepotBuildPicker,
        "Depósito",
        TITLE_BROWN,
        Vec2::new(280.0, 120.0),
        320.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            Text::new("Tipo de depósito a construir"),
            window_text_font(&asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                for (btn, label) in DepotKindButton::ALL {
                    spawn_chip(row, &asset_server, btn, label);
                }
            });
    });
}

pub(crate) fn sync_depot_build_picker(
    tool_state: Res<UiToolState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut buttons: Query<(&DepotKindButton, &mut BackgroundColor), With<Button>>,
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
    for (btn, mut bg) in &mut buttons {
        *bg = BackgroundColor(if Some(btn.as_tool()) == tool_state.active_tool {
            BTN_SELECTED
        } else {
            BTN_BG
        });
    }
}

pub(crate) fn handle_depot_build_picker_buttons(
    buttons: Query<(&Interaction, &DepotKindButton), (Changed<Interaction>, With<Button>)>,
    mut tool_state: ResMut<UiToolState>,
) {
    let open = matches!(
        tool_state.active_tool,
        Some(BuildMenuAction::RoadDepot)
            | Some(BuildMenuAction::RailDepot)
            | Some(BuildMenuAction::ShipDepot)
    );
    if !open {
        return;
    }
    for (interaction, btn) in &buttons {
        if *interaction == Interaction::Pressed {
            tool_state.active_tool = Some(btn.as_tool());
        }
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

    #[test]
    fn depot_build_picker_on_closed_clears_ship_depot_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::ShipDepot),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::DepotBuildPicker),
        ));
        world.run_system_once(depot_build_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }

    #[test]
    fn rail_waypoint_picker_on_closed_clears_tool() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::RailWaypoint),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::RailWaypointPicker),
        ));
        world
            .run_system_once(rail_waypoint_picker_on_closed)
            .unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }

    #[test]
    fn terraform_picker_on_closed_clears_raise_land() {
        let mut world = World::new();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::RaiseLand),
            ..Default::default()
        });
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::TerraformPicker),
        ));
        world.run_system_once(terraform_picker_on_closed).unwrap();
        assert!(world.resource::<UiToolState>().active_tool.is_none());
    }

    #[test]
    fn dock_orient_button_maps_orientation() {
        assert_eq!(DockOrientButton::North.as_orientation(), 0);
        assert_eq!(DockOrientButton::West.as_orientation(), 3);
    }

    #[test]
    fn depot_kind_button_maps_tools() {
        assert_eq!(DepotKindButton::Ship.as_tool(), BuildMenuAction::ShipDepot);
        assert_eq!(DepotKindButton::Rail.as_tool(), BuildMenuAction::RailDepot);
    }
}
