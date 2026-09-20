use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui::FocusPolicy;
use openttdrs_core::Climate;

use crate::network::{NetCli, NetworkStatus};
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
    MainMenuDensityTarget, MainMenuEditorButton, MainMenuHeightmapSlot, MainMenuHighscoresButton,
    MainMenuHighscoresText, MainMenuHintsText, MainMenuLanguageButton, MainMenuLanguageLabel,
    MainMenuLoadButton, MainMenuLocalizedText, MainMenuMapSizeButton, MainMenuNewGameButton,
    MainMenuNewGameOptionsColumn, MainMenuOpenHeightmapsDirButton, MainMenuOpenScenariosDirButton,
    MainMenuPanel, MainMenuPreferencesButton, MainMenuQuitButton, MainMenuQuitConfirmNo,
    MainMenuQuitConfirmYes, MainMenuResolutionButton, MainMenuScenariosButton,
    MainMenuSeedDecButton, MainMenuSeedIncButton, MainMenuSeedInput, MainMenuSeedInputState,
    MainMenuSeedRandomButton, MainMenuSoundButton, MainMenuStartButton, MainMenuSubPanel,
    MainMenuSummaryText, MainMenuTitleText, MainMenuToggle, MainMenuUi,
};

const MAIN_MENU_BACKDROP_ALPHA: f32 = 0.28;
const MAIN_MENU_PANEL_ALPHA: f32 = 0.86;
const MAIN_MENU_PANEL_MAX_WIDTH: f32 = 900.0;
const NEW_GAME_OPTIONS_COLUMN_WIDTH: f32 = 420.0;

pub(crate) fn setup_main_menu(
    mut commands: Commands,
    net_cli: Res<NetCli>,
    status: Res<NetworkStatus>,
) {
    commands.insert_resource(MainMenuPanel::default());
    if let NetCli::Client { addr } = &*net_cli {
        spawn_client_connecting_menu(&mut commands, addr, &status.label);
        return;
    }
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
            BackgroundColor(Color::srgba(0.04, 0.06, 0.09, MAIN_MENU_BACKDROP_ALPHA)),
            GlobalZIndex(3000),
            MainMenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Percent(90.0),
                    max_width: Val::Px(MAIN_MENU_PANEL_MAX_WIDTH),
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
                BackgroundColor(Color::srgba(0.18, 0.17, 0.12, MAIN_MENU_PANEL_ALPHA)),
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

fn spawn_client_connecting_menu(commands: &mut Commands, addr: &str, status_label: &str) {
    let subtitle = if status_label.is_empty() {
        format!("Conectando a {addr}…")
    } else {
        format!("{status_label} — esperando Welcome…")
    };
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
            BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.72)),
            GlobalZIndex(3000),
            MainMenuUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(420.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.18, 0.17, 0.12, 0.96)),
                BorderColor::all(Color::srgb(0.74, 0.68, 0.5)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    MainMenuTitleText,
                    MainMenuLocalizedText("Multijugador"),
                    Text::new("Multijugador"),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Title.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.91, 0.72)),
                ));
                panel.spawn((
                    Text::new(subtitle),
                    TextFont {
                        font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.84, 0.7)),
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
                MainMenuEditorButton,
                "Editor de escenarios",
                42.0,
            ));
            menu.spawn(secondary_button(
                MainMenuDemoButton,
                "Demo completa (mapa plano)",
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
    panel
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(16.0),
            row_gap: Val::Px(12.0),
            ..default()
        },))
        .with_children(|columns| {
            columns
                .spawn((MainMenuNewGameOptionsColumn, new_game_options_column()))
                .with_children(spawn_new_game_map_options);
            columns
                .spawn((MainMenuNewGameOptionsColumn, new_game_options_column()))
                .with_children(spawn_new_game_world_options);
        });
}

fn new_game_options_column() -> Node {
    Node {
        width: Val::Px(NEW_GAME_OPTIONS_COLUMN_WIDTH),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(6.0),
        ..default()
    }
}

fn spawn_new_game_map_options(panel: &mut ChildSpawnerCommands) {
    panel.spawn(option_section_label("Clima"));
    panel
        .spawn((wide_option_row(Val::Px(6.0)),))
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
        .spawn((wide_option_row(Val::Px(4.0)),))
        .with_children(|row| {
            row.spawn(map_size_button(MainMenuMapSizeButton::Compact));
        });

    panel.spawn(option_section_label("Ancho (teselas)"));
    panel
        .spawn((wide_option_row(Val::Px(4.0)),))
        .with_children(|row| {
            for axis in MapAxisSize::all() {
                row.spawn(map_size_button(MainMenuMapSizeButton::Width(axis)));
            }
        });

    panel.spawn(option_section_label("Alto (teselas)"));
    panel
        .spawn((wide_option_row(Val::Px(4.0)),))
        .with_children(|row| {
            for axis in MapAxisSize::all() {
                row.spawn(map_size_button(MainMenuMapSizeButton::Height(axis)));
            }
        });

    panel.spawn(option_section_label("Ano de inicio"));
    panel
        .spawn((wide_option_row(Val::Px(4.0)),))
        .with_children(|row| {
            for year in START_YEARS {
                row.spawn(start_year_button(year));
            }
        });

    panel.spawn(option_section_label("Dinero inicial"));
    panel
        .spawn((wide_option_row(Val::Px(4.0)),))
        .with_children(|row| {
            for amount in STARTING_MONEY_OPTIONS {
                row.spawn(starting_money_button(amount));
            }
        });
}

fn spawn_new_game_world_options(panel: &mut ChildSpawnerCommands) {
    spawn_seed_options(panel);

    panel.spawn(option_section_label("Densidad de pueblos"));
    panel
        .spawn((wide_option_row(Val::Px(6.0)),))
        .with_children(|row| {
            for density in PopulationDensity::all() {
                row.spawn(density_button(density, MainMenuDensityTarget::Town));
            }
        });

    panel.spawn(option_section_label("Densidad de industrias"));
    panel
        .spawn((wide_option_row(Val::Px(6.0)),))
        .with_children(|row| {
            for density in PopulationDensity::all() {
                row.spawn(density_button(density, MainMenuDensityTarget::Industry));
            }
        });

    panel.spawn(option_section_label("Relieve"));
    panel
        .spawn((wide_option_row(Val::Px(6.0)),))
        .with_children(|row| {
            for roughness in TerrainRoughness::all() {
                row.spawn(roughness_button(roughness));
            }
        });

    panel.spawn(option_section_label("Terreno"));
    panel
        .spawn((wide_option_row(Val::Px(6.0)),))
        .with_children(|toggles| {
            toggles.spawn(toggle_button(
                MainMenuToggle::WorldGen,
                "Terreno procedural",
                206.0,
            ));
            toggles.spawn(toggle_button(
                MainMenuToggle::Island,
                "Modo isla (costas)",
                206.0,
            ));
            toggles.spawn(toggle_button(
                MainMenuToggle::RivalAi,
                "Rival IA (TransCargo)",
                206.0,
            ));
            toggles.spawn(toggle_button(
                MainMenuToggle::Disasters,
                "Desastres ambientales",
                206.0,
            ));
            if dev_mode() {
                toggles.spawn(toggle_button(
                    MainMenuToggle::PreserveDemo,
                    "Incluir showcase completo (64×64)",
                    206.0,
                ));
            }
        });
}

fn wide_option_row(gap: Val) -> Node {
    Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::Center,
        column_gap: gap,
        row_gap: gap,
        ..default()
    }
}

fn spawn_seed_options(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_children(|row| {
            row.spawn(option_section_label("Semilla"));
            row.spawn((
                MainMenuSeedInput,
                MainMenuSeedInputState::default(),
                EditableText::new("0"),
                Node {
                    width: Val::Px(142.0),
                    height: Val::Px(28.0),
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.08, 0.06)),
                BorderColor::all(Color::srgb(0.6, 0.53, 0.36)),
                Interaction::default(),
                FocusPolicy::Block,
                TextCursorStyle::default(),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.93, 0.8)),
            ));
            row.spawn(seed_adjust_button(MainMenuSeedDecButton, "−"));
            row.spawn(seed_adjust_button(MainMenuSeedIncButton, "+"));
            row.spawn(seed_adjust_button(MainMenuSeedRandomButton, "↻"));
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
                Text::new("Idioma: Español"),
                TextFont {
                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.74, 0.58)),
            ));
            panel
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    justify_content: JustifyContent::Center,
                    width: Val::Px(260.0),
                    ..default()
                },))
                .with_children(|row| {
                    for locale in crate::i18n::Locale::ALL {
                        row.spawn((
                            Button,
                            crate::ui::hud::UiClickBeep,
                            MainMenuLanguageButton(locale),
                            Node {
                                width: Val::Px(112.0),
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
                                Text::new(locale.label()),
                                TextFont {
                                    font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.9, 0.86, 0.72)),
                            )],
                        ));
                    }
                });
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
