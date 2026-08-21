use bevy::prelude::*;

use super::super::{
    BuildMenuAction, BuildMenuUi, CompanyColourSwatch, SaveMenuAction, ToolSelectButton,
    ToolbarCloseButton, ToolbarTooltipTarget, ZoomButton,
};
use crate::sprites::{company_colour_swatch_color, company_colour_tooltip};

pub(super) fn spawn_panel_title(
    parent: &mut ChildSpawnerCommands,
    title: &'static str,
    width: f32,
) {
    parent
        .spawn((
            Node {
                height: Val::Px(16.0),
                width: Val::Px(width),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.36, 0.52, 0.28)),
            BorderColor::all(Color::srgb(0.73, 0.84, 0.55)),
            BuildMenuUi,
        ))
        .with_children(|title_bar| {
            title_bar.spawn((
                Node {
                    width: Val::Px(15.0),
                    height: Val::Px(15.0),
                    ..default()
                },
                BuildMenuUi,
            ));
            title_bar.spawn((
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BuildMenuUi,
                children![(
                    Text::new(title),
                    TextFont {
                        font_size: FontSize::Rem(0.7),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.96, 0.82)),
                )],
            ));
            title_bar.spawn((
                Button,
                ToolbarCloseButton,
                ToolbarTooltipTarget { text: "Cerrar" },
                BuildMenuUi,
                Node {
                    width: Val::Px(15.0),
                    height: Val::Px(15.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.31, 0.14)),
                BorderColor::all(Color::srgb(0.12, 0.16, 0.09)),
                Interaction::default(),
                children![(
                    Text::new("X"),
                    TextFont {
                        font_size: FontSize::Rem(0.7),
                        ..default()
                    },
                    TextColor(Color::srgb(0.02, 0.03, 0.02)),
                )],
            ));
        });
}

pub(super) fn spawn_icon_tool_buttons(
    buttons: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    defs: &[(&'static str, &'static str, BuildMenuAction)],
) {
    for (tip, icon_path, action) in defs {
        buttons
            .spawn((
                Button,
                *action,
                ToolSelectButton,
                ToolbarTooltipTarget { text: tip },
                BuildMenuUi,
                Node {
                    width: Val::Px(53.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.63, 0.35)),
                BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
                Interaction::default(),
            ))
            .with_children(|p| {
                spawn_button_icon(p, asset_server, icon_path, 42.0, 34.0, false);
            });
    }
}

pub(super) fn spawn_settings_buttons(buttons: &mut ChildSpawnerCommands) {
    // Tres filas para que el panel no quede aplastado en horizontal.
    const ROW1: &[(&str, &str, SaveMenuAction)] = &[
        (
            "Pausa/Reanudar",
            "Alternar pausa de simulacion",
            SaveMenuAction::PauseResume,
        ),
        (
            "Más lento",
            "Reduce la simulación: 4x / 2x / 1x / 0.5x / 0.25x",
            SaveMenuAction::SlowDown,
        ),
        (
            "Acelerar",
            "Aumenta la simulación: 0.25x / 0.5x / 1x / 2x / 4x",
            SaveMenuAction::SpeedUp,
        ),
        (
            "Normalizar",
            "Vuelve a velocidad 1x y zoom 1.0x",
            SaveMenuAction::Normalize,
        ),
        ("Zoom +", "Acercar camara", SaveMenuAction::ZoomIn),
        ("Zoom -", "Alejar camara", SaveMenuAction::ZoomOut),
        (
            "Guardar...",
            "Elegir archivo y guardar simulacion JSON",
            SaveMenuAction::SaveAs,
        ),
        (
            "Cargar...",
            "Elegir archivo y cargar simulacion JSON",
            SaveMenuAction::LoadFrom,
        ),
        (
            "Noticias...",
            "Off / Summary / Full por tipo de noticia",
            SaveMenuAction::NewsSettings,
        ),
    ];
    const ROW2: &[(&str, &str, SaveMenuAction)] = &[
        (
            "Averías: normales",
            "Cicla averías: normales / reducidas / desactivadas",
            SaveMenuAction::CycleVehicleBreakdowns,
        ),
        (
            "Pathfinding / PBS...",
            "Espera path, giro en señales y look-ahead (pf.*)",
            SaveMenuAction::PathfindingSettings,
        ),
        (
            "Carretera: izquierda",
            "Cicla circulación vial izquierda / derecha (tablas bay)",
            SaveMenuAction::ToggleRoadDrivingSide,
        ),
        (
            "Distribución de carga...",
            "CargoDist: Manual / Asimétrica / Simétrica",
            SaveMenuAction::CargoDistSettings,
        ),
        (
            "IA / TransCargo...",
            "Activar rival, umbral de dinero, máx. rutas y debug",
            SaveMenuAction::AiSettings,
        ),
        (
            "NewGRF...",
            "Stack NewGRF (ON/OFF, orden, añadir; sin Action0–14)",
            SaveMenuAction::NewGrf,
        ),
        (
            "Display...",
            "Minimapa, PBS, catenaria, nombres de pueblos",
            SaveMenuAction::DisplayOptions,
        ),
        (
            "Vista extra",
            "Segunda camara (sigue a la principal)",
            SaveMenuAction::ExtraViewport,
        ),
        (
            "Ayuda...",
            "About y mapa de hotkeys (F1)",
            SaveMenuAction::Help,
        ),
        (
            "Consola...",
            "FPS, gizmos, comandos (F3 / `)",
            SaveMenuAction::DevConsole,
        ),
    ];
    const ROW3: &[(&str, &str, SaveMenuAction)] = &[
        (
            "Inspector tile",
            "Dump del tile seleccionado (F2)",
            SaveMenuAction::TileInspector,
        ),
        (
            "Cheats...",
            "Dinero, año, bulldozer, compañía (Ctrl+Alt+C)",
            SaveMenuAction::Cheats,
        ),
        (
            "Guardar escenario...",
            "JSON en save/scenarios/ (editor #42)",
            SaveMenuAction::SaveScenario,
        ),
        (
            "Finalizar partida",
            "Retiro voluntario → puntuación y menú",
            SaveMenuAction::EndGame,
        ),
        (
            "Catenaria",
            "Cicla visible / transparente / oculta (TO_CATENARY)",
            SaveMenuAction::CycleCatenaryDisplay,
        ),
        (
            "Menu principal",
            "Volver al menu de inicio",
            SaveMenuAction::ReturnToMainMenu,
        ),
    ];

    buttons
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|col| {
            spawn_settings_button_row(col, ROW1);
            spawn_settings_button_row(col, ROW2);
            col.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(1.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                for &(label, tip, action) in ROW3 {
                    spawn_settings_text_button(row, label, tip, action);
                }
                spawn_company_colour_picker(row);
                crate::ui::toolbar::company_selector::spawn_company_selector(row);
            });
        });
}

fn spawn_settings_button_row(
    parent: &mut ChildSpawnerCommands,
    defs: &[(&'static str, &'static str, SaveMenuAction)],
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(1.0),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            for &(label, tip, action) in defs {
                spawn_settings_text_button(row, label, tip, action);
            }
        });
}

fn spawn_settings_text_button(
    parent: &mut ChildSpawnerCommands,
    label: &'static str,
    tip: &'static str,
    action: SaveMenuAction,
) {
    let mut entity = parent.spawn((
        Button,
        action,
        ToolbarTooltipTarget { text: tip },
        BuildMenuUi,
        Node {
            width: Val::Px(118.0),
            height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.5, 0.63, 0.35)),
        BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(0.65),
                ..default()
            },
            TextColor(Color::srgb(0.08, 0.07, 0.05)),
        )],
    ));
    if matches!(action, SaveMenuAction::ZoomIn | SaveMenuAction::ZoomOut) {
        entity.insert(ZoomButton);
    }
}

fn spawn_company_colour_picker(buttons: &mut ChildSpawnerCommands) {
    buttons
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                align_items: AlignItems::FlexStart,
                align_self: AlignSelf::Center,
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|col| {
            col.spawn((
                Text::new("Color compañía"),
                TextFont {
                    font_size: FontSize::Rem(0.65),
                    ..default()
                },
                TextColor(Color::srgb(0.08, 0.07, 0.05)),
                BuildMenuUi,
            ));
            for row in 0..2u8 {
                col.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .with_children(|row_node| {
                    for i in (row * 8)..(row * 8 + 8) {
                        row_node.spawn((
                            Button,
                            CompanyColourSwatch(i),
                            ToolbarTooltipTarget {
                                text: company_colour_tooltip(i),
                            },
                            BuildMenuUi,
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(company_colour_swatch_color(i)),
                            BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
                            Interaction::default(),
                        ));
                    }
                });
            }
        });
}

fn spawn_button_icon(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    icon_path: &str,
    width: f32,
    height: f32,
    with_margin: bool,
) {
    let margin = if with_margin {
        UiRect::right(Val::Px(4.0))
    } else {
        UiRect::default()
    };
    if let Some(label) = icon_path.strip_prefix("text:") {
        parent.spawn((
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(height + 4.0),
                ..default()
            },
            TextColor(Color::srgb(0.08, 0.07, 0.05)),
            Node {
                width: Val::Px(width),
                margin,
                ..default()
            },
        ));
    } else {
        parent.spawn((
            ImageNode::new(asset_server.load::<Image>(icon_path.to_string())),
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                margin,
                ..default()
            },
        ));
    }
}
