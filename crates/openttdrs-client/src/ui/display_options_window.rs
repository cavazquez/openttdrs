//! Opciones de visualización (Display Options) persistidas en `ClientPreferences`.

use bevy::prelude::*;

use crate::render::RemapMapVisualsPending;
use crate::settings::{ClientPreferences, ClientSettingsPreset};
use crate::sprites::{TransparencyMode, TransparencyOption};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::SimHudControls;
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);

#[derive(Resource, Default)]
pub(crate) struct DisplayOptionsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayOptionsToggle {
    Minimap,
    TownLabels,
    StationLabels,
    FullAnimation,
    FullDetail,
    PbsOverlay,
    DebugGizmos,
    Diagnostics,
    /// Fila TO_*: categoría + modo del botón.
    Transparency {
        option: TransparencyOption,
        mode: TransparencyMode,
    },
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayOptionsPreset {
    Classic,
    Performance,
    Dev,
}

pub(crate) fn setup_display_options_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::DisplayOptions,
        "Opciones de visualización",
        TITLE_BROWN,
        Vec2::new(280.0, 64.0),
        460.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Preferencias de cliente (se guardan al salir)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
        ));
        for (label, toggle) in [
            ("Minimapa", DisplayOptionsToggle::Minimap),
            ("Nombres de pueblos", DisplayOptionsToggle::TownLabels),
            ("Nombres de estaciones", DisplayOptionsToggle::StationLabels),
            ("Animación completa", DisplayOptionsToggle::FullAnimation),
            ("Detalle completo", DisplayOptionsToggle::FullDetail),
            ("Reservas PBS", DisplayOptionsToggle::PbsOverlay),
            ("Gizmos de depuración", DisplayOptionsToggle::DebugGizmos),
            ("Overlay de diagnóstico", DisplayOptionsToggle::Diagnostics),
        ] {
            spawn_toggle_row(body, asset_server, label, toggle);
        }
        body.spawn((
            Text::new("Presets de cliente"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
        ));
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            spawn_preset_btn(row, asset_server, DisplayOptionsPreset::Classic, "Clásico");
            spawn_preset_btn(
                row,
                asset_server,
                DisplayOptionsPreset::Performance,
                "Rendimiento",
            );
            spawn_preset_btn(row, asset_server, DisplayOptionsPreset::Dev, "Dev");
        });
        body.spawn((
            Text::new("Transparencia / invisibilidad (TO_*)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
        ));
        for option in TransparencyOption::ALL {
            spawn_transparency_row(body, asset_server, option);
        }
    });
}

fn spawn_toggle_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    toggle: DisplayOptionsToggle,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            row.spawn((
                Button,
                toggle,
                Node {
                    min_width: Val::Px(48.0),
                    height: Val::Px(22.0),
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
                    Text::new("ON"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
        });
}

fn spawn_preset_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    preset: DisplayOptionsPreset,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            preset,
            Node {
                flex_grow: 1.0,
                height: Val::Px(22.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
        });
}

fn spawn_transparency_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    option: TransparencyOption,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            margin: UiRect::top(Val::Px(3.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(option.label_es()),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                Node {
                    min_width: Val::Px(88.0),
                    ..default()
                },
            ));
            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|btns| {
                for mode in [
                    TransparencyMode::Visible,
                    TransparencyMode::Transparent,
                    TransparencyMode::Hidden,
                ] {
                    let short = match mode {
                        TransparencyMode::Visible => "V",
                        TransparencyMode::Transparent => "T",
                        TransparencyMode::Hidden => "O",
                    };
                    btns.spawn((
                        Button,
                        DisplayOptionsToggle::Transparency { option, mode },
                        Node {
                            min_width: Val::Px(28.0),
                            height: Val::Px(22.0),
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
                            Text::new(short),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
        });
}

pub(crate) fn handle_display_options_buttons(
    buttons: Query<
        (&Interaction, &DisplayOptionsToggle),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisplayOptionsPreset>,
        ),
    >,
    presets: Query<
        (&Interaction, &DisplayOptionsPreset),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisplayOptionsToggle>,
        ),
    >,
    mut prefs: ResMut<ClientPreferences>,
    mut hud: ResMut<SimHudControls>,
    mut pending_remap: Option<ResMut<RemapMapVisualsPending>>,
) {
    for (interaction, preset) in &presets {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let mapped = match *preset {
            DisplayOptionsPreset::Classic => ClientSettingsPreset::Classic,
            DisplayOptionsPreset::Performance => ClientSettingsPreset::Performance,
            DisplayOptionsPreset::Dev => ClientSettingsPreset::Dev,
        };
        prefs.apply_preset(mapped);
        hud.minimap_visible = prefs.minimap_visible;
        hud.sim_speed = prefs.default_sim_speed.clamp(0.25, 8.0);
        crate::sprites::set_transparency_preferences(
            prefs.transparency_opt,
            prefs.invisibility_opt,
        );
        request_full_remap(pending_remap.as_deref_mut());
    }
    for (interaction, toggle) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *toggle {
            DisplayOptionsToggle::Minimap => {
                prefs.minimap_visible = !prefs.minimap_visible;
                hud.minimap_visible = prefs.minimap_visible;
            }
            DisplayOptionsToggle::TownLabels => {
                prefs.show_town_labels = !prefs.show_town_labels;
                request_full_remap(pending_remap.as_deref_mut());
            }
            DisplayOptionsToggle::StationLabels => {
                prefs.show_station_labels = !prefs.show_station_labels;
                request_full_remap(pending_remap.as_deref_mut());
            }
            DisplayOptionsToggle::FullAnimation => {
                prefs.full_animation = !prefs.full_animation;
            }
            DisplayOptionsToggle::FullDetail => {
                prefs.full_detail = !prefs.full_detail;
                request_full_remap(pending_remap.as_deref_mut());
            }
            DisplayOptionsToggle::PbsOverlay => {
                prefs.show_pbs_reservations = !prefs.show_pbs_reservations;
                request_full_remap(pending_remap.as_deref_mut());
            }
            DisplayOptionsToggle::DebugGizmos => {
                prefs.show_debug_gizmos = !prefs.show_debug_gizmos;
            }
            DisplayOptionsToggle::Diagnostics => {
                prefs.show_diagnostics_overlay = !prefs.show_diagnostics_overlay;
            }
            DisplayOptionsToggle::Transparency { option, mode } => {
                prefs.set_transparency_mode(option, mode);
                crate::sprites::set_transparency_preferences(
                    prefs.transparency_opt,
                    prefs.invisibility_opt,
                );
                request_full_remap(pending_remap.as_deref_mut());
            }
        }
    }
}

fn request_full_remap(pending: Option<&mut RemapMapVisualsPending>) {
    if let Some(pending) = pending {
        pending.pending = true;
        pending.full = true;
    }
}

pub(crate) fn sync_display_options_window(
    state: Res<DisplayOptionsWindowState>,
    prefs: Res<ClientPreferences>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility), Without<DisplayOptionsToggle>>,
    mut buttons: Query<(&DisplayOptionsToggle, &mut BackgroundColor, &Children), With<Button>>,
    mut texts: Query<&mut Text>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::DisplayOptions)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    for (toggle, mut bg, children) in &mut buttons {
        let (on, label) = match *toggle {
            DisplayOptionsToggle::Minimap => {
                (prefs.minimap_visible, bool_label(prefs.minimap_visible))
            }
            DisplayOptionsToggle::TownLabels => {
                (prefs.show_town_labels, bool_label(prefs.show_town_labels))
            }
            DisplayOptionsToggle::StationLabels => (
                prefs.show_station_labels,
                bool_label(prefs.show_station_labels),
            ),
            DisplayOptionsToggle::FullAnimation => {
                (prefs.full_animation, bool_label(prefs.full_animation))
            }
            DisplayOptionsToggle::FullDetail => (prefs.full_detail, bool_label(prefs.full_detail)),
            DisplayOptionsToggle::PbsOverlay => (
                prefs.show_pbs_reservations,
                bool_label(prefs.show_pbs_reservations),
            ),
            DisplayOptionsToggle::DebugGizmos => {
                (prefs.show_debug_gizmos, bool_label(prefs.show_debug_gizmos))
            }
            DisplayOptionsToggle::Diagnostics => (
                prefs.show_diagnostics_overlay,
                bool_label(prefs.show_diagnostics_overlay),
            ),
            DisplayOptionsToggle::Transparency { option, mode } => {
                (prefs.transparency_mode(option) == mode, "")
            }
        };
        *bg = BackgroundColor(if on { BTN_ACTIVE } else { BTN_BG });
        if matches!(
            *toggle,
            DisplayOptionsToggle::Minimap
                | DisplayOptionsToggle::TownLabels
                | DisplayOptionsToggle::StationLabels
                | DisplayOptionsToggle::FullAnimation
                | DisplayOptionsToggle::FullDetail
                | DisplayOptionsToggle::PbsOverlay
                | DisplayOptionsToggle::DebugGizmos
                | DisplayOptionsToggle::Diagnostics
        ) {
            for child in children.iter() {
                if let Ok(mut text) = texts.get_mut(child) {
                    **text = label.to_string();
                }
            }
        }
    }
}

fn bool_label(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

pub(crate) fn display_options_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<DisplayOptionsWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::DisplayOptions {
            state.open = false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn toggling_minimap_updates_prefs_and_hud() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences {
            minimap_visible: true,
            ..Default::default()
        });
        world.insert_resource(SimHudControls {
            minimap_visible: true,
            ..Default::default()
        });
        world.spawn((Button, DisplayOptionsToggle::Minimap, Interaction::Pressed));
        world
            .run_system_once(handle_display_options_buttons)
            .unwrap();
        assert!(!world.resource::<ClientPreferences>().minimap_visible);
        assert!(!world.resource::<SimHudControls>().minimap_visible);
    }

    #[test]
    fn toggling_full_animation_and_detail() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(RemapMapVisualsPending::default());

        world.spawn((
            Button,
            DisplayOptionsToggle::FullAnimation,
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_display_options_buttons)
            .unwrap();
        assert!(!world.resource::<ClientPreferences>().full_animation);

        let entities: Vec<_> = world
            .query_filtered::<Entity, With<DisplayOptionsToggle>>()
            .iter(&world)
            .collect();
        for e in entities {
            world.entity_mut(e).despawn();
        }
        world.spawn((
            Button,
            DisplayOptionsToggle::FullDetail,
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_display_options_buttons)
            .unwrap();
        assert!(!world.resource::<ClientPreferences>().full_detail);
        assert!(world.resource::<RemapMapVisualsPending>().pending);
    }

    #[test]
    fn transparency_row_sets_trees_hidden() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.spawn((
            Button,
            DisplayOptionsToggle::Transparency {
                option: TransparencyOption::Trees,
                mode: TransparencyMode::Hidden,
            },
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_display_options_buttons)
            .unwrap();
        let prefs = world.resource::<ClientPreferences>();
        assert_eq!(
            prefs.transparency_mode(TransparencyOption::Trees),
            TransparencyMode::Hidden
        );
        assert!(world.resource::<RemapMapVisualsPending>().pending);
    }
}
