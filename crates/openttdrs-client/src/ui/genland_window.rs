//! Ventana GenLand del scenario editor (#42): regenerar paisaje in-place.

use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::render::RemapMapVisualsPending;
use crate::state::bootstrap::TerrainRoughness;
use crate::state::{EditorSession, SimWorld, regenerate_landscape_in_place};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::HudBuildFeedback;
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ON: Color = Color::srgb(0.28, 0.42, 0.28);
const BTN_GO: Color = Color::srgb(0.32, 0.48, 0.28);

#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct GenLandWindowState {
    pub(crate) open: bool,
    pub(crate) seed: u64,
    pub(crate) roughness: TerrainRoughness,
    pub(crate) island: bool,
    pub(crate) climate: Climate,
}

impl Default for GenLandWindowState {
    fn default() -> Self {
        Self {
            open: false,
            seed: 42,
            roughness: TerrainRoughness::Normal,
            island: false,
            climate: Climate::Temperate,
        }
    }
}

#[derive(Component)]
pub(crate) struct GenLandStatusText;

#[derive(Component)]
pub(crate) struct GenLandSeedText;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenLandAction {
    SeedMinus,
    SeedPlus,
    CycleRoughness,
    ToggleIsland,
    CycleClimate,
    Generate,
}

pub(crate) fn setup_genland_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::GenLand,
        "Generar paisaje",
        TITLE_BROWN,
        Vec2::new(260.0, 160.0),
        340.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Editor · regenera el mapa (borra pueblos/industrias/infra)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            GenLandSeedText,
            Text::new("Semilla —"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            GenLandStatusText,
            Text::new("—"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            spawn_btn(row, asset_server, GenLandAction::SeedMinus, "Semilla−");
            spawn_btn(row, asset_server, GenLandAction::SeedPlus, "Semilla+");
            spawn_btn(row, asset_server, GenLandAction::CycleRoughness, "Relieve");
            spawn_btn(row, asset_server, GenLandAction::ToggleIsland, "Isla");
            spawn_btn(row, asset_server, GenLandAction::CycleClimate, "Clima");
            spawn_btn(row, asset_server, GenLandAction::Generate, "Generar");
        });
    });
}

fn spawn_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: GenLandAction,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                min_width: Val::Px(72.0),
                height: Val::Px(26.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(if matches!(action, GenLandAction::Generate) {
                BTN_GO
            } else {
                BTN_BG
            }),
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

pub(crate) fn sync_genland_window(
    state: Res<GenLandWindowState>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut seed_q: Query<&mut Text, (With<GenLandSeedText>, Without<GenLandStatusText>)>,
    mut status_q: Query<&mut Text, (With<GenLandStatusText>, Without<GenLandSeedText>)>,
    mut buttons: Query<(&GenLandAction, &mut BackgroundColor), With<Button>>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::GenLand {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        return;
    }
    let seed_line = format!("Semilla {}", state.seed);
    for mut text in &mut seed_q {
        **text = seed_line.clone();
    }
    let status = format!(
        "Relieve {} · {} · clima {:?}",
        state.roughness.menu_label(),
        if state.island { "isla" } else { "continente" },
        state.climate
    );
    for mut text in &mut status_q {
        **text = status.clone();
    }
    for (action, mut bg) in &mut buttons {
        *bg = BackgroundColor(match *action {
            GenLandAction::ToggleIsland if state.island => BTN_ON,
            GenLandAction::Generate => BTN_GO,
            _ => BTN_BG,
        });
    }
}

pub(crate) fn handle_genland_buttons(
    editor: Res<EditorSession>,
    mut state: ResMut<GenLandWindowState>,
    mut sim: Option<ResMut<SimWorld>>,
    mut remap: Option<ResMut<RemapMapVisualsPending>>,
    mut feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
    buttons: Query<
        (&Interaction, &GenLandAction),
        (Changed<Interaction>, With<Button>, With<GenLandAction>),
    >,
) {
    if !state.open {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            GenLandAction::SeedMinus => {
                state.seed = state.seed.saturating_sub(1);
            }
            GenLandAction::SeedPlus => {
                state.seed = state.seed.saturating_add(1);
            }
            GenLandAction::CycleRoughness => {
                state.roughness = match state.roughness {
                    TerrainRoughness::Flat => TerrainRoughness::Normal,
                    TerrainRoughness::Normal => TerrainRoughness::Hilly,
                    TerrainRoughness::Hilly => TerrainRoughness::Flat,
                };
            }
            GenLandAction::ToggleIsland => {
                state.island = !state.island;
            }
            GenLandAction::CycleClimate => {
                state.climate = match state.climate {
                    Climate::Temperate => Climate::SubArctic,
                    Climate::SubArctic => Climate::SubTropical,
                    Climate::SubTropical => Climate::Toyland,
                    Climate::Toyland => Climate::Temperate,
                };
            }
            GenLandAction::Generate => {
                if !editor.active {
                    feedback.message = Some("GenLand solo en el editor de escenarios".into());
                    feedback.expires_at_secs = time.elapsed_secs() + 3.0;
                    continue;
                }
                let Some(sim) = sim.as_deref_mut() else {
                    continue;
                };
                match regenerate_landscape_in_place(
                    &mut sim.state,
                    state.climate,
                    state.seed,
                    state.island,
                    state.roughness,
                ) {
                    Ok(()) => {
                        if let Some(remap) = remap.as_deref_mut() {
                            remap.pending = true;
                            remap.full = true;
                        }
                        feedback.message = Some(format!(
                            "Paisaje regenerado (semilla {}, {:?})",
                            state.seed, state.climate
                        ));
                        feedback.expires_at_secs = time.elapsed_secs() + 4.0;
                    }
                    Err(e) => {
                        feedback.message = Some(format!("GenLand falló: {e}"));
                        feedback.expires_at_secs = time.elapsed_secs() + 5.0;
                    }
                }
            }
        }
    }
}

pub(crate) fn genland_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<GenLandWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::GenLand {
            state.open = false;
        }
    }
}
