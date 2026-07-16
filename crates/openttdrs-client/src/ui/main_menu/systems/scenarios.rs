use bevy::prelude::*;

use crate::render::{MapVisualLayer, ShoreTile, WaterTile};
use crate::state::bootstrap::{MapSizePreset, NewGameSettings};
use crate::state::{
    ClientScreen, SimWorld, SuspendedGameSession, new_game::NewGameSettingsResource,
};
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};

use super::super::widgets::{hover_primary, hover_secondary};
use super::super::{
    MainMenuCamera, MainMenuHeightmapSlot, MainMenuOpenHeightmapsDirButton,
    MainMenuOpenScenariosDirButton, MainMenuPanel, MainMenuScenariosButton, MainMenuUi,
};
use super::session::enter_new_game;

fn scenarios_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("save/scenarios")
}

fn heightmaps_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("save/heightmaps")
}

fn list_heightmap_files() -> Vec<std::path::PathBuf> {
    let dir = heightmaps_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("hmap"))
        })
        .collect();
    paths.sort();
    paths
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_scenarios_interaction(
    mut panel: ResMut<MainMenuPanel>,
    mut save_window: ResMut<SaveWindowState>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut root_btn: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MainMenuScenariosButton>,
            Without<MainMenuOpenScenariosDirButton>,
            Without<MainMenuOpenHeightmapsDirButton>,
            Without<MainMenuHeightmapSlot>,
        ),
    >,
    mut open_scn: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MainMenuOpenScenariosDirButton>,
            Without<MainMenuScenariosButton>,
            Without<MainMenuOpenHeightmapsDirButton>,
            Without<MainMenuHeightmapSlot>,
        ),
    >,
    mut open_hmap: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MainMenuOpenHeightmapsDirButton>,
            Without<MainMenuScenariosButton>,
            Without<MainMenuOpenScenariosDirButton>,
            Without<MainMenuHeightmapSlot>,
        ),
    >,
    mut slots: Query<
        (&Interaction, &MainMenuHeightmapSlot, &mut BackgroundColor),
        (
            Without<MainMenuScenariosButton>,
            Without<MainMenuOpenScenariosDirButton>,
            Without<MainMenuOpenHeightmapsDirButton>,
        ),
    >,
    mut commands: Commands,
) {
    if *panel == MainMenuPanel::Root {
        for (interaction, mut bg) in &mut root_btn {
            if *interaction == Interaction::Pressed {
                let _ = std::fs::create_dir_all(scenarios_dir());
                let _ = std::fs::create_dir_all(heightmaps_dir());
                *panel = MainMenuPanel::Scenarios;
                return;
            }
            hover_secondary(interaction, &mut bg);
        }
        return;
    }
    if *panel != MainMenuPanel::Scenarios {
        return;
    }
    for (interaction, mut bg) in &mut open_scn {
        if *interaction == Interaction::Pressed {
            let dir = scenarios_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("_menu.json");
            save_window.open_in_mode(
                SaveWindowMode::Load,
                &save_dir_from(&path.to_string_lossy()),
            );
            return;
        }
        hover_primary(interaction, &mut bg);
    }
    for (interaction, mut bg) in &mut open_hmap {
        if *interaction == Interaction::Pressed {
            let dir = heightmaps_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("README.txt");
            let _ = std::fs::write(
                &path,
                "Coloca archivos .hmap (OTDRHMAP1) aquí y elígelos en la lista del menú.\n",
            );
            return;
        }
        hover_secondary(interaction, &mut bg);
    }
    let files = list_heightmap_files();
    for (interaction, slot, mut bg) in &mut slots {
        if *interaction != Interaction::Pressed {
            hover_secondary(interaction, &mut bg);
            continue;
        }
        let Some(path) = files.get(slot.0) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(data) = openttdrs_core::parse_hmap(&text) else {
            continue;
        };
        settings.0 = NewGameSettings {
            world_gen: false,
            island: false,
            preserve_demo: false,
            map_size: MapSizePreset::SMALL,
            ..NewGameSettings::default()
        };
        commands.insert_resource(PendingHeightmap(data));
        enter_new_game(
            &mut commands,
            &q_menu,
            &q_menu_cam,
            &intro_layers,
            settings.settings(),
            &mut next_screen,
            &mut suspended,
        );
        return;
    }
}

#[derive(Resource)]
pub(crate) struct PendingHeightmap(pub openttdrs_core::HeightmapData);

pub(crate) fn apply_pending_heightmap_on_enter(
    mut commands: Commands,
    pending: Option<Res<PendingHeightmap>>,
    mut sim: ResMut<SimWorld>,
) {
    let Some(pending) = pending else {
        return;
    };
    let data = pending.0.clone();
    let climate = sim.state.climate;
    let seed = sim.state.world_seed;
    if openttdrs_core::apply_heightmap(&mut sim.state.map, &data, 1, climate, seed).is_ok() {
        sim.state.towns.clear();
        sim.state.industries.clear();
        sim.state.stations.clear();
    }
    commands.remove_resource::<PendingHeightmap>();
}

pub(crate) fn sync_main_menu_heightmap_slots(
    panel: Res<MainMenuPanel>,
    mut q: Query<(&MainMenuHeightmapSlot, &mut Node, &Children)>,
    mut texts: Query<&mut Text>,
) {
    if *panel != MainMenuPanel::Scenarios {
        return;
    }
    let files = list_heightmap_files();
    for (slot, mut node, children) in &mut q {
        if let Some(path) = files.get(slot.0) {
            node.display = Display::Flex;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("heightmap.hmap");
            for child in children.iter() {
                if let Ok(mut text) = texts.get_mut(child) {
                    **text = name.to_string();
                }
            }
        } else {
            node.display = Display::None;
        }
    }
}
