use bevy::app::AppExit;
use bevy::prelude::*;
use openttdrs_core::{Climate, format_money};

use crate::render::{MapVisualLayer, ShoreTile, WaterTile};
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, START_YEARS, STARTING_MONEY_OPTIONS,
};
use crate::state::{ClientScreen, SimWorld, new_game::NewGameSettingsResource};
use crate::ui::SimHudControls;
use crate::ui::font::UiFontRole;
use crate::ui::main_menu_intro::cleanup_main_menu_intro;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct MainMenuCamera;

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainMenuPanel {
    #[default]
    Root,
    NewGame,
    QuitConfirm,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuSubPanel(MainMenuPanel);

#[derive(Component)]
pub(crate) struct MainMenuTitleText;

#[derive(Component)]
pub(crate) struct MainMenuHintsText;

#[derive(Component)]
pub(crate) struct MainMenuNewGameButton;

#[derive(Component)]
pub(crate) struct MainMenuLoadButton;

#[derive(Component)]
pub(crate) struct MainMenuDemoButton;

#[derive(Component)]
pub(crate) struct MainMenuQuitButton;

#[derive(Component)]
pub(crate) struct MainMenuBackButton;

#[derive(Component)]
pub(crate) struct MainMenuStartButton;

#[derive(Component)]
pub(crate) struct MainMenuQuitConfirmYes;

#[derive(Component)]
pub(crate) struct MainMenuQuitConfirmNo;

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuClimateButton(Climate);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuMapSizeButton(pub MapSizePreset);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuStartYearButton(pub u32);

#[derive(Component)]
pub(crate) struct MainMenuSeedDecButton;

#[derive(Component)]
pub(crate) struct MainMenuSeedIncButton;

#[derive(Component, Clone, Copy)]
pub(crate) enum MainMenuToggle {
    WorldGen,
    Island,
    PreserveDemo,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum MainMenuDensityTarget {
    Town,
    Industry,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuDensityButton(pub PopulationDensity, pub MainMenuDensityTarget);

#[derive(Component, Clone, Copy)]
pub(crate) struct MainMenuStartingMoneyButton(pub i64);

#[derive(Component)]
pub(crate) struct MainMenuSummaryText;

fn dev_mode() -> bool {
    std::env::var_os("OPENTTDRS_DEV").is_some()
}

fn climate_label(climate: Climate) -> &'static str {
    match climate {
        Climate::Temperate => "Templado",
        Climate::SubArctic => "Artico",
        Climate::SubTropical => "Tropical",
        Climate::Toyland => "Toyland",
    }
}

fn map_size_label(size: MapSizePreset) -> &'static str {
    match size {
        MapSizePreset::Compact => "24×18",
        MapSizePreset::Small => "64×64",
        MapSizePreset::Medium => "128×128",
        MapSizePreset::Large => "256×256",
    }
}

fn summary_text(settings: NewGameSettings) -> String {
    let settings = settings.sanitized();
    let mode = if settings.world_gen {
        if settings.island {
            "isla procedural + lagos"
        } else {
            "colinas procedural + lagos"
        }
    } else if settings.preserve_demo {
        "demo clasica (plana)"
    } else {
        "mapa plano"
    };
    format!(
        "Mapa {} · clima {} · inicio {} · {} · semilla={}\nPueblos {} · industrias {} · capital {}",
        map_size_label(settings.map_size),
        climate_label(settings.climate),
        settings.start_year,
        mode,
        if settings.seed == 0 {
            "auto".to_string()
        } else {
            settings.seed.to_string()
        },
        settings.town_density.menu_label(),
        settings.industry_density.menu_label(),
        format_money(settings.starting_money),
    )
}

fn option_section_label(text: &str) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.76, 0.62)),
    )
}

fn panel_title(panel: MainMenuPanel) -> &'static str {
    match panel {
        MainMenuPanel::Root => "OpenTTDRS",
        MainMenuPanel::NewGame => "Nueva partida",
        MainMenuPanel::QuitConfirm => "Salir del juego",
    }
}

fn panel_hints(panel: MainMenuPanel) -> &'static str {
    match panel {
        MainMenuPanel::Root => "Esc salir · raton para elegir",
        MainMenuPanel::NewGame => {
            "Enter iniciar · Esc volver · 1-4 clima · [ ] semilla · z/x densidad"
        }
        MainMenuPanel::QuitConfirm => "Esc cancelar",
    }
}

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
                spawn_quit_confirm_panel(panel);

                panel.spawn((
                    MainMenuHintsText,
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
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|menu| {
            menu.spawn(primary_button(MainMenuNewGameButton, "Nueva partida", 50.0));
            menu.spawn(primary_button(MainMenuLoadButton, "Cargar partida", 50.0));
            menu.spawn(secondary_button(
                MainMenuDemoButton,
                "Demo clasica (mapa plano)",
                42.0,
            ));
            menu.spawn(secondary_button(MainMenuQuitButton, "Salir", 42.0));
        });
}

fn spawn_new_game_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            MainMenuSubPanel(MainMenuPanel::NewGame),
            hidden_subpanel_node(Node {
                width: Val::Percent(100.0),
                max_height: Val::Px(520.0),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            }),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
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

            panel.spawn(option_section_label("Tamano del mapa"));
            panel
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                },))
                .with_children(|row| {
                    for size in MapSizePreset::all() {
                        row.spawn(map_size_button(size));
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

fn density_button(density: PopulationDensity, target: MainMenuDensityTarget) -> impl Bundle {
    (
        Button,
        MainMenuDensityButton(density, target),
        Node {
            width: Val::Px(88.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        Interaction::default(),
        children![(
            Text::new(density.menu_label()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

fn starting_money_button(amount: i64) -> impl Bundle {
    let label = if amount >= 1_000_000 {
        format!("{}M", amount / 1_000_000)
    } else if amount >= 1_000 {
        format!("{}k", amount / 1_000)
    } else {
        amount.to_string()
    };
    (
        Button,
        MainMenuStartingMoneyButton(amount),
        Node {
            width: Val::Px(72.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

fn map_size_button(size: MapSizePreset) -> impl Bundle {
    (
        Button,
        MainMenuMapSizeButton(size),
        Node {
            width: Val::Px(92.0),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
        Interaction::default(),
        children![(
            Text::new(size.menu_label()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.74)),
        )],
    )
}

fn start_year_button(year: u32) -> impl Bundle {
    (
        Button,
        MainMenuStartYearButton(year),
        Node {
            width: Val::Px(46.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.26, 0.24, 0.19)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.42)),
        Interaction::default(),
        children![(
            Text::new(year.to_string()),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

fn seed_adjust_button(marker: impl Component, label: &str) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(36.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
        BorderColor::all(Color::srgb(0.55, 0.5, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
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

fn climate_button(climate: Climate) -> impl Bundle {
    (
        Button,
        MainMenuClimateButton(climate),
        Node {
            width: Val::Px(100.0),
            height: Val::Px(32.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2)),
        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
        Interaction::default(),
        children![(
            Text::new(climate_label(climate)),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.74)),
        )],
    )
}

fn toggle_button(toggle: MainMenuToggle, label: &'static str) -> impl Bundle {
    (
        Button,
        toggle,
        Node {
            width: Val::Px(360.0),
            height: Val::Px(30.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
        BorderColor::all(Color::srgb(0.55, 0.5, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.86, 0.72)),
        )],
    )
}

fn primary_button(marker: impl Component, label: &str, height: f32) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        BorderColor::all(Color::srgb(0.7, 0.66, 0.5)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Hud.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.8)),
        )],
    )
}

fn secondary_button(marker: impl Component, label: &str, height: f32) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        BorderColor::all(Color::srgb(0.58, 0.54, 0.4)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(UiFontRole::Body.rem_size()),
                ..default()
            },
            TextColor(Color::srgb(0.91, 0.88, 0.76)),
        )],
    )
}

pub(crate) fn sync_main_menu_panel_visibility(
    panel: Res<MainMenuPanel>,
    mut subpanels: Query<(&MainMenuSubPanel, &mut Node, &mut Visibility)>,
    mut title_q: Query<&mut Text, (With<MainMenuTitleText>, Without<MainMenuHintsText>)>,
    mut hints_q: Query<
        &mut Text,
        (
            With<MainMenuHintsText>,
            Without<MainMenuTitleText>,
            Without<MainMenuSummaryText>,
        ),
    >,
) {
    let panel_changed = panel.is_changed();
    for (sub, mut node, mut vis) in &mut subpanels {
        let active = sub.0 == *panel;
        node.display = if active {
            Display::DEFAULT
        } else {
            Display::None
        };
        *vis = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !panel_changed {
        return;
    }
    if let Ok(mut title) = title_q.single_mut() {
        title.0 = panel_title(*panel).to_string();
    }
    if let Ok(mut hints) = hints_q.single_mut() {
        hints.0 = panel_hints(*panel).to_string();
    }
}

pub(crate) fn sync_main_menu_summary(
    settings: Res<NewGameSettingsResource>,
    panel: Res<MainMenuPanel>,
    mut q: Query<&mut Text, With<MainMenuSummaryText>>,
) {
    if !settings.is_changed() && !panel.is_changed() {
        return;
    }
    if *panel != MainMenuPanel::NewGame {
        return;
    }
    for mut text in &mut q {
        text.0 = summary_text(settings.settings());
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_options_interaction(
    panel: Res<MainMenuPanel>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut button_sets: ParamSet<(
        Query<(&Interaction, &MainMenuClimateButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuMapSizeButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuStartYearButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuToggle, &mut BackgroundColor)>,
        Query<(&Interaction, &mut BackgroundColor), With<MainMenuSeedDecButton>>,
        Query<(&Interaction, &mut BackgroundColor), With<MainMenuSeedIncButton>>,
        Query<(&Interaction, &MainMenuDensityButton, &mut BackgroundColor)>,
        Query<(
            &Interaction,
            &MainMenuStartingMoneyButton,
            &mut BackgroundColor,
        )>,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if *panel != MainMenuPanel::NewGame {
        return;
    }

    if keys.just_pressed(KeyCode::Digit1) {
        settings.0.climate = Climate::Temperate;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        settings.0.climate = Climate::SubArctic;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        settings.0.climate = Climate::SubTropical;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        settings.0.climate = Climate::Toyland;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        adjust_seed(&mut settings.0.seed, -1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        adjust_seed(&mut settings.0.seed, 1);
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        cycle_density(&mut settings.0.town_density);
    }
    if keys.just_pressed(KeyCode::KeyX) {
        cycle_density(&mut settings.0.industry_density);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p0() {
        if *interaction == Interaction::Pressed {
            settings.0.climate = btn.0;
        }
        *bg = option_button_bg(settings.0.climate == btn.0, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p1() {
        if *interaction == Interaction::Pressed {
            settings.0.map_size = btn.0;
            if settings.0.map_size != MapSizePreset::Compact {
                settings.0.preserve_demo = false;
            }
        }
        *bg = option_button_bg(settings.0.map_size == btn.0, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p2() {
        if *interaction == Interaction::Pressed {
            settings.0.start_year = btn.0;
        }
        *bg = option_button_bg(settings.0.start_year == btn.0, *interaction);
    }

    for (interaction, toggle, mut bg) in &mut button_sets.p3() {
        if *interaction == Interaction::Pressed {
            match toggle {
                MainMenuToggle::WorldGen => settings.0.world_gen = !settings.0.world_gen,
                MainMenuToggle::Island => settings.0.island = !settings.0.island,
                MainMenuToggle::PreserveDemo => {
                    if settings.0.map_size == MapSizePreset::Compact {
                        settings.0.preserve_demo = !settings.0.preserve_demo;
                    }
                }
            }
        }
        let on = match toggle {
            MainMenuToggle::WorldGen => settings.0.world_gen,
            MainMenuToggle::Island => settings.0.island,
            MainMenuToggle::PreserveDemo => {
                settings.0.preserve_demo && settings.0.map_size == MapSizePreset::Compact
            }
        };
        *bg = toggle_button_bg(on, *interaction);
    }

    for (interaction, mut bg) in &mut button_sets.p4() {
        if *interaction == Interaction::Pressed {
            adjust_seed(&mut settings.0.seed, -1);
        }
        *bg = seed_button_bg(*interaction);
    }
    for (interaction, mut bg) in &mut button_sets.p5() {
        if *interaction == Interaction::Pressed {
            adjust_seed(&mut settings.0.seed, 1);
        }
        *bg = seed_button_bg(*interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p6() {
        if *interaction == Interaction::Pressed {
            match btn.1 {
                MainMenuDensityTarget::Town => settings.0.town_density = btn.0,
                MainMenuDensityTarget::Industry => settings.0.industry_density = btn.0,
            }
        }
        let selected = match btn.1 {
            MainMenuDensityTarget::Town => settings.0.town_density == btn.0,
            MainMenuDensityTarget::Industry => settings.0.industry_density == btn.0,
        };
        *bg = option_button_bg(selected, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p7() {
        if *interaction == Interaction::Pressed {
            settings.0.starting_money = btn.0;
        }
        *bg = option_button_bg(settings.0.starting_money == btn.0, *interaction);
    }
}

fn cycle_density(density: &mut PopulationDensity) {
    *density = match density {
        PopulationDensity::Sparse => PopulationDensity::Normal,
        PopulationDensity::Normal => PopulationDensity::Dense,
        PopulationDensity::Dense => PopulationDensity::Sparse,
    };
}

fn adjust_seed(seed: &mut u64, delta: i32) {
    if delta < 0 {
        *seed = seed.saturating_sub(1);
    } else {
        *seed = seed.saturating_add(1);
    }
}

fn option_button_bg(selected: bool, interaction: Interaction) -> BackgroundColor {
    if selected {
        BackgroundColor(Color::srgb(0.48, 0.42, 0.28))
    } else if interaction == Interaction::Hovered {
        BackgroundColor(Color::srgb(0.36, 0.32, 0.22))
    } else {
        BackgroundColor(Color::srgb(0.28, 0.26, 0.2))
    }
}

fn toggle_button_bg(on: bool, interaction: Interaction) -> BackgroundColor {
    if on {
        BackgroundColor(Color::srgb(0.38, 0.44, 0.32))
    } else if interaction == Interaction::Hovered {
        BackgroundColor(Color::srgb(0.3, 0.28, 0.2))
    } else {
        BackgroundColor(Color::srgb(0.24, 0.22, 0.17))
    }
}

fn seed_button_bg(interaction: Interaction) -> BackgroundColor {
    match interaction {
        Interaction::Hovered => BackgroundColor(Color::srgb(0.32, 0.3, 0.22)),
        Interaction::Pressed => BackgroundColor(Color::srgb(0.38, 0.34, 0.24)),
        Interaction::None => BackgroundColor(Color::srgb(0.24, 0.22, 0.17)),
    }
}

pub(crate) fn leave_main_menu(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    next_screen: &mut NextState<ClientScreen>,
) {
    cleanup_main_menu_intro(commands, intro_layers);
    for e in q_menu {
        commands.entity(e).despawn();
    }
    for cam in q_menu_cam {
        commands.entity(cam).despawn();
    }
    commands.remove_resource::<MainMenuPanel>();
    next_screen.set(ClientScreen::InGame);
}

fn enter_new_game(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    settings: NewGameSettings,
    next_screen: &mut NextState<ClientScreen>,
) {
    commands.insert_resource(SimWorld::from_new_game(&settings.sanitized()));
    leave_main_menu(commands, q_menu, q_menu_cam, intro_layers, next_screen);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_interaction(
    mut panel: ResMut<MainMenuPanel>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut save_window: ResMut<SaveWindowState>,
    hud: Res<SimHudControls>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut button_sets: ParamSet<(
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuNewGameButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuLoadButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuDemoButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuBackButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuStartButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitConfirmYes>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitConfirmNo>),
        >,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if save_window.open {
        return;
    }

    let esc = keys.just_pressed(KeyCode::Escape);
    match *panel {
        MainMenuPanel::Root if esc => {
            *panel = MainMenuPanel::QuitConfirm;
            return;
        }
        MainMenuPanel::NewGame if esc => {
            *panel = MainMenuPanel::Root;
            return;
        }
        MainMenuPanel::QuitConfirm if esc => {
            *panel = MainMenuPanel::Root;
            return;
        }
        _ => {}
    }

    match *panel {
        MainMenuPanel::Root => {
            for (interaction, mut bg) in &mut button_sets.p0() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::NewGame;
                    return;
                }
                hover_primary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p1() {
                if *interaction == Interaction::Pressed {
                    save_window
                        .open_in_mode(SaveWindowMode::Load, &save_dir_from(&hud.json_save_path));
                    return;
                }
                hover_primary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p2() {
                if *interaction == Interaction::Pressed {
                    settings.0 = NewGameSettings {
                        climate: Climate::Temperate,
                        map_size: MapSizePreset::Compact,
                        start_year: START_YEARS[0],
                        world_gen: false,
                        island: false,
                        preserve_demo: true,
                        seed: 0,
                        town_density: PopulationDensity::Normal,
                        industry_density: PopulationDensity::Normal,
                        starting_money: STARTING_MONEY_OPTIONS[1],
                    };
                    enter_new_game(
                        &mut commands,
                        &q_menu,
                        &q_menu_cam,
                        &intro_layers,
                        settings.settings(),
                        &mut next_screen,
                    );
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p3() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::QuitConfirm;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
        MainMenuPanel::NewGame => {
            let start_via_key =
                keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
            let mut start_requested = start_via_key;
            for (interaction, mut bg) in &mut button_sets.p5() {
                if *interaction == Interaction::Pressed {
                    start_requested = true;
                }
                hover_primary(interaction, &mut bg);
            }
            if start_requested {
                enter_new_game(
                    &mut commands,
                    &q_menu,
                    &q_menu_cam,
                    &intro_layers,
                    settings.settings(),
                    &mut next_screen,
                );
                return;
            }
            for (interaction, mut bg) in &mut button_sets.p4() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::Root;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
        MainMenuPanel::QuitConfirm => {
            for (interaction, mut bg) in &mut button_sets.p6() {
                if *interaction == Interaction::Pressed {
                    exit.write(AppExit::Success);
                    return;
                }
                hover_primary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p7() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::Root;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
    }
}

fn hover_primary(interaction: &Interaction, bg: &mut BackgroundColor) {
    match *interaction {
        Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.46, 0.42, 0.3)),
        Interaction::None => *bg = BackgroundColor(Color::srgb(0.35, 0.33, 0.24)),
        Interaction::Pressed => {}
    }
}

fn hover_secondary(interaction: &Interaction, bg: &mut BackgroundColor) {
    match *interaction {
        Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.34, 0.3, 0.22)),
        Interaction::None => *bg = BackgroundColor(Color::srgb(0.24, 0.22, 0.16)),
        Interaction::Pressed => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::World;

    #[test]
    fn setup_main_menu_and_camera_run() {
        let mut world = World::new();
        world.init_resource::<NewGameSettingsResource>();
        world.run_system_once(setup_main_menu).unwrap();
        assert_eq!(world.resource::<MainMenuPanel>(), &MainMenuPanel::Root);
    }
}
