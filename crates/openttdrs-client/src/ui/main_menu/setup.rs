use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::state::bootstrap::{
    MapAxisSize, PopulationDensity, START_YEARS, STARTING_MONEY_OPTIONS, TerrainRoughness,
};
use crate::state::new_game::NewGameSettingsResource;
use crate::ui::font::UiFontRole;

use super::labels::{dev_mode, option_section_label, panel_hints, panel_title, summary_text};
use super::widgets::{
    climate_button, density_button, map_size_button, primary_button, roughness_button,
    secondary_button, seed_adjust_button, start_year_button, starting_money_button, toggle_button,
};
use super::{
    MainMenuBackButton, MainMenuContinueButton, MainMenuContinueWrap, MainMenuDemoButton,
    MainMenuDensityTarget, MainMenuHeightmapSlot, MainMenuHighscoresButton, MainMenuHighscoresText,
    MainMenuHintsText, MainMenuLanguageLabel, MainMenuLoadButton, MainMenuMapSizeButton,
    MainMenuNewGameButton, MainMenuOpenHeightmapsDirButton, MainMenuOpenScenariosDirButton,
    MainMenuPanel, MainMenuPreferencesButton, MainMenuQuitButton, MainMenuQuitConfirmNo,
    MainMenuQuitConfirmYes, MainMenuResolutionButton, MainMenuScenariosButton,
    MainMenuSeedDecButton, MainMenuSeedIncButton, MainMenuSoundButton, MainMenuStartButton,
    MainMenuSubPanel, MainMenuSummaryText, MainMenuTitleText, MainMenuToggle, MainMenuUi,
};

pub(crate) fn setup_main_menu(mut commands: Commands) {
    commands.insert_resource(MainMenuPanel::default());
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.42)),
            GlobalZIndex(3000),
            MainMenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(520.0),
                    height: Val::Percent(90.0),
                    max_height: Val::Percent(90.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::new(
                        Val::Px(18.0),
                        Val::Px(18.0),
                        Val::Px(18.0),
                        Val::Px(14.0),
                    ),
                    border: UiRect::all(Val::Px(3.0)),
                    row_gap: Val::Px(10.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.18, 0.17, 0.12, 0.96)),
                BorderColor::all(Color::srgb(0.74, 0.68, 0.5)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    MainMenuTitleText,
                    Text::new(panel_title(MainMenuPanel::Root)),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Title.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.91, 0.72)),
                ));

                spawn_root_panel(panel);
                spawn_new_game_panel(panel);
                spawn_highscores_panel(panel);
                spawn_scenarios_panel(panel);
                spawn_preferences_panel(panel);
                spawn_quit_confirm_panel(panel);

                panel.spawn((
                    MainMenuHintsText,
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Text::new(panel_hints(MainMenuPanel::Root)),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.76, 0.72, 0.58)),
                ));
            });
        });
}

fn hidden_subpanel_node(extra: Node) -> Node {
    Node {
        display: Display::None,
        ..extra
    }
}

fn spawn_root_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::Root),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|menu| {
            menu.spawn((
                MainMenuContinueWrap,
                hidden_subpanel_node(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                }),
                Visibility::Hidden,
            ))
            .with_children(|wrap| {
                wrap.spawn(primary_button(
                    MainMenuContinueButton,
                    "Continuar partida",
                    50.0,
                ));
            });
            menu.spawn(primary_button(MainMenuNewGameButton, "Nueva partida", 50.0));
            menu.spawn(primary_button(MainMenuLoadButton, "Cargar partida", 50.0));
            menu.spawn(secondary_button(
                MainMenuScenariosButton,
                "Escenarios / heightmap",
                42.0,
            ));
            menu.spawn(secondary_button(
                MainMenuDemoButton,
                "Demo clasica (mapa plano)",
                42.0,
            ));
            menu.spawn(secondary_button(
                MainMenuHighscoresButton,
                "Mejores puntuaciones",
                42.0,
            ));
            menu.spawn(secondary_button(
                MainMenuPreferencesButton,
                "Preferencias",
                42.0,
            ));
            menu.spawn(secondary_button(
                MainMenuSoundButton,
                "Sonido / musica",
                42.0,
            ));
            menu.spawn(secondary_button(MainMenuQuitButton, "Salir", 42.0));
        });
}

fn spawn_highscores_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::Highscores),
            hidden_subpanel_node(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            }),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                MainMenuHighscoresText,
                Text::new("(sin puntuaciones)"),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.86, 0.74)),
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            panel.spawn(secondary_button(MainMenuBackButton, "Volver", 42.0));
        });
}

fn spawn_new_game_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::NewGame),
            hidden_subpanel_node(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            }),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    min_height: Val::Px(0.0),
                    overflow: Overflow::scroll_y(),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|scroll| {
                    spawn_new_game_options(scroll);
                });

            panel.spawn((
                MainMenuSummaryText,
                Text::new(summary_text(NewGameSettingsResource::default().settings())),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.74, 0.58)),
            ));

            panel.spawn(primary_button(MainMenuStartButton, "Iniciar partida", 50.0));
            panel.spawn(secondary_button(MainMenuBackButton, "Volver", 42.0));
        });
}

fn spawn_new_game_options(panel: &mut ChildSpawnerCommands) {
    panel.spawn(option_section_label("Clima"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .with_children(|row| {
            for climate in [
                Climate::Temperate,
                Climate::SubArctic,
                Climate::SubTropical,
                Climate::Toyland,
            ] {
                row.spawn(climate_button(climate));
            }
        });

    panel.spawn(option_section_label("Tamano del mapa (demo)"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|row| {
            row.spawn(map_size_button(MainMenuMapSizeButton::Compact));
        });

    panel.spawn(option_section_label("Ancho (teselas)"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Px(420.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|row| {
            for axis in MapAxisSize::all() {
                row.spawn(map_size_button(MainMenuMapSizeButton::Width(axis)));
            }
        });

    panel.spawn(option_section_label("Alto (teselas)"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Px(420.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|row| {
            for axis in MapAxisSize::all() {
                row.spawn(map_size_button(MainMenuMapSizeButton::Height(axis)));
            }
        });

    panel.spawn(option_section_label("Ano de inicio"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Px(400.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|row| {
            for year in START_YEARS {
                row.spawn(start_year_button(year));
            }
        });

    panel.spawn(option_section_label("Densidad de pueblos"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .with_children(|row| {
            for density in PopulationDensity::all() {
                row.spawn(density_button(density, MainMenuDensityTarget::Town));
            }
        });

    panel.spawn(option_section_label("Densidad de industrias"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .with_children(|row| {
            for density in PopulationDensity::all() {
                row.spawn(density_button(density, MainMenuDensityTarget::Industry));
            }
        });

    panel.spawn(option_section_label("Dinero inicial"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Px(400.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .with_children(|row| {
            for amount in STARTING_MONEY_OPTIONS {
                row.spawn(starting_money_button(amount));
            }
        });

    panel.spawn(option_section_label("Relieve"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .with_children(|row| {
            for roughness in TerrainRoughness::all() {
                row.spawn(roughness_button(roughness));
            }
        });

    panel.spawn(option_section_label("Terreno"));
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .with_children(|toggles| {
            toggles.spawn(toggle_button(
                MainMenuToggle::WorldGen,
                "Terreno procedural",
            ));
            toggles.spawn(toggle_button(MainMenuToggle::Island, "Modo isla (costas)"));
            toggles.spawn(toggle_button(
                MainMenuToggle::RivalAi,
                "Rival IA (TransCargo)",
            ));
            toggles.spawn(toggle_button(
                MainMenuToggle::Disasters,
                "Desastres ambientales",
            ));
            if dev_mode() {
                toggles.spawn(toggle_button(
                    MainMenuToggle::PreserveDemo,
                    "Incluir zona demo/tutorial (24×18)",
                ));
            }
        });

    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_children(|row| {
            row.spawn(option_section_label("Semilla"));
            row.spawn(seed_adjust_button(MainMenuSeedDecButton, "−"));
            row.spawn(seed_adjust_button(MainMenuSeedIncButton, "+"));
        });
}

fn spawn_scenarios_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::Scenarios),
            hidden_subpanel_node(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            }),
        ))
        .with_children(|panel| {
            panel.spawn(option_section_label(
                "Escenarios: save/scenarios/ · Heightmaps: save/heightmaps/*.hmap",
            ));
            panel.spawn(primary_button(
                MainMenuOpenScenariosDirButton,
                "Abrir escenarios (.json/.sav)",
                44.0,
            ));
            panel.spawn(secondary_button(
                MainMenuOpenHeightmapsDirButton,
                "Abrir carpeta heightmaps",
                40.0,
            ));
            panel.spawn(option_section_label(
                "Heightmaps detectados (clic para jugar)",
            ));
            for slot in 0..6 {
                panel.spawn((
                    Button,
                    MainMenuHeightmapSlot(slot),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
                    BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
                    Interaction::default(),
                    children![(
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.86, 0.72)),
                    )],
                ));
            }
            panel.spawn(secondary_button(MainMenuBackButton, "Volver", 42.0));
        });
}

fn spawn_preferences_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::Preferences),
            hidden_subpanel_node(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            }),
        ))
        .with_children(|panel| {
            panel.spawn(option_section_label("Resolucion (reinicio al cambiar)"));
            panel
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(6.0),
                    justify_content: JustifyContent::Center,
                    width: Val::Px(420.0),
                    ..default()
                },))
                .with_children(|row| {
                    for (w, h) in [(1280_u32, 720_u32), (1600, 900), (1920, 1080)] {
                        row.spawn((
                            Button,
                            crate::ui::hud::UiClickBeep,
                            MainMenuResolutionButton {
                                width: w,
                                height: h,
                            },
                            Node {
                                width: Val::Px(120.0),
                                height: Val::Px(32.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
                            BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
                            Interaction::default(),
                            children![(
                                Text::new(format!("{w}×{h}")),
                                TextFont {
                                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.86, 0.72)),
                            )],
                        ));
                    }
                });
            panel.spawn((
                MainMenuLanguageLabel,
                Text::new("Idioma: Espanol (unico por ahora)"),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.74, 0.58)),
            ));
            panel.spawn(secondary_button(MainMenuBackButton, "Volver", 42.0));
        });
}

fn spawn_quit_confirm_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::QuitConfirm),
            hidden_subpanel_node(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            }),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("¿Salir de OpenTTDRS?"),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.84, 0.7)),
            ));
            panel.spawn(primary_button(MainMenuQuitConfirmYes, "Si, salir", 44.0));
            panel.spawn(secondary_button(MainMenuQuitConfirmNo, "Cancelar", 42.0));
        });
}
