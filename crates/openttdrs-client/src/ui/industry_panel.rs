//! Panel flotante al clicar una industria (sin herramienta activa), con vista previa renderizada a textura.

use std::collections::{HashSet, VecDeque};

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::widget::ImageNode;
use bevy::ui::{FocusPolicy, GlobalZIndex};
use openttdrs_core::{IndustryKind, Map, TileCoord, TileKind};

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::state::bootstrap::industry_group_from_gfx;
use crate::ui::toolbar::BuildMenuUi;

const PREVIEW_TEX_W: u32 = 320;
const PREVIEW_TEX_H: u32 = 180;
const PREVIEW_SCALE_MUL: f32 = 0.38;

#[derive(Resource, Default)]
pub(crate) struct IndustryPanelState {
    pub(crate) open: bool,
    pub(crate) focus_tile: Option<TileCoord>,
}

#[derive(Component)]
pub(crate) struct IndustryPanelRoot;

#[derive(Component)]
pub(crate) struct IndustryPanelTitle;

#[derive(Component)]
pub(crate) struct IndustryPanelCloseButton;

fn industry_gfx(tile: &openttdrs_core::Tile) -> u16 {
    u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8)
}

fn flood_industry_tiles(map: &Map, start: TileCoord) -> Vec<TileCoord> {
    if map.get_kind(start) != Some(TileKind::Industry) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut q = VecDeque::new();
    q.push_back(start);
    while let Some(c) = q.pop_front() {
        if !seen.insert(c) {
            continue;
        }
        out.push(c);
        for (dx, dy) in [(0i32, 1), (0, -1), (1, 0), (-1, 0)] {
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if map.get_kind(n) == Some(TileKind::Industry) {
                q.push_back(n);
            }
        }
    }
    out
}

fn industry_kind_for_component(
    map: &Map,
    sim: &SimWorld,
    anchor: TileCoord,
) -> Option<IndustryKind> {
    let tiles = flood_industry_tiles(map, anchor);
    let set: HashSet<TileCoord> = tiles.into_iter().collect();
    sim.state
        .industries
        .iter()
        .find(|i| set.contains(&i.pos))
        .map(|i| i.kind)
}

fn kind_label(k: IndustryKind) -> &'static str {
    match k {
        IndustryKind::CoalMine => "Carbon",
        IndustryKind::Forest => "Bosque",
        IndustryKind::OilWell => "Petróleo",
        IndustryKind::Factory => "Fábrica",
    }
}

fn format_panel_title(map: &Map, sim: &SimWorld, focus: TileCoord) -> String {
    let gfx_label = map.get(focus).map_or("Sin datos de tesela", |t| {
        industry_group_from_gfx(industry_gfx(&t))
    });
    let sim_part = industry_kind_for_component(map, sim, focus)
        .map(|k| format!(" · Sim: {}", kind_label(k)))
        .unwrap_or_default();
    format!("Industria — {gfx_label}{sim_part}")
}

pub(crate) fn setup_industry_panel(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = Image::new_target_texture(
        PREVIEW_TEX_W,
        PREVIEW_TEX_H,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    let rt_handle = images.add(image);

    commands.spawn((
        Camera2d,
        IndustryPreviewCamera,
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.22, 0.38, 0.52)),
            ..default()
        },
        RenderTarget::from(rt_handle.clone()),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: 2.0,
            ..OrthographicProjection::default_2d()
        }),
    ));

    commands
        .spawn((
            IndustryPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(72.0),
                width: Val::Px(340.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.14, 0.11, 0.08, 0.97)),
            BorderColor::all(Color::srgb(0.78, 0.7, 0.48)),
            GlobalZIndex(2200),
            Visibility::Hidden,
            BuildMenuUi,
            FocusPolicy::Block,
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    IndustryPanelTitle,
                    Text::new("Industria"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.8)),
                ));
                row.spawn((
                    IndustryPanelCloseButton,
                    Button,
                    Node {
                        width: Val::Px(28.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.42, 0.36, 0.24)),
                    BorderColor::all(Color::srgb(0.7, 0.62, 0.42)),
                    Interaction::default(),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("✕"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.92, 0.88, 0.78)),
                    ));
                });
            });
            p.spawn((
                Node {
                    width: Val::Px(PREVIEW_TEX_W as f32),
                    height: Val::Px(PREVIEW_TEX_H as f32),
                    ..default()
                },
                ImageNode::new(rt_handle),
            ));
        });
}

pub(crate) fn industry_panel_close_interaction(
    q: Query<&Interaction, (Changed<Interaction>, With<IndustryPanelCloseButton>)>,
    mut panel: ResMut<IndustryPanelState>,
    mut preview_cam: Query<&mut Camera, (With<IndustryPreviewCamera>, Without<PrimaryGameCamera>)>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        panel.open = false;
        panel.focus_tile = None;
        if let Ok(mut cam) = preview_cam.single_mut() {
            cam.is_active = false;
        }
    }
}

pub(crate) fn sync_industry_panel(
    panel: Res<IndustryPanelState>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<IndustryPanelRoot>>,
    mut title_q: Query<&mut Text, With<IndustryPanelTitle>>,
    mut preview: Query<
        (&mut Transform, &mut Projection, &mut Camera),
        (With<IndustryPreviewCamera>, Without<PrimaryGameCamera>),
    >,
    primary_proj: Query<&Projection, (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
) {
    let Ok(mut root_vis) = root_q.single_mut() else {
        return;
    };

    if !panel.open {
        *root_vis = Visibility::Hidden;
        if let Ok((_tf, _proj, mut cam)) = preview.single_mut() {
            cam.is_active = false;
        }
        return;
    }

    let Some(focus) = panel.focus_tile else {
        *root_vis = Visibility::Hidden;
        if let Ok((_tf, _proj, mut cam)) = preview.single_mut() {
            cam.is_active = false;
        }
        return;
    };

    *root_vis = Visibility::Visible;

    if let Ok(mut text) = title_q.single_mut() {
        **text = format_panel_title(&sim.state.map, &sim, focus);
    }

    let (_tileh, base_z) =
        tile_slope_and_min_z(&sim.state.map, focus.x.max(0) as u32, focus.y.max(0) as u32);
    let pos = tile_pos(focus.x, focus.y, base_z, 0.0);

    let primary_scale = primary_proj
        .single()
        .ok()
        .and_then(|p| match p {
            Projection::Orthographic(o) => Some(o.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    let preview_scale = (primary_scale * PREVIEW_SCALE_MUL).max(0.35);

    if let Ok((mut tf, mut proj, mut cam)) = preview.single_mut() {
        cam.is_active = true;
        tf.translation = Vec3::new(pos.x, pos.y, 999.0);
        if let Projection::Orthographic(ref mut o) = *proj {
            o.scale = preview_scale;
        }
    }
}
