//! Panel flotante al clicar una industria (sin herramienta activa), con vista previa renderizada a textura.

use std::collections::{HashSet, VecDeque};

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::ui::widget::ImageNode;
use bevy::ui::{FocusPolicy, GlobalZIndex};
use openttdrs_core::{IndustryKind, IndustrySpec, Map, TileCoord, TileKind};

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::state::bootstrap::industry_group_from_gfx;
use crate::ui::toolbar::BuildMenuUi;

const PREVIEW_TEX_W: u32 = 320;
const PREVIEW_TEX_H: u32 = 180;
const PREVIEW_SCALE_MUL: f32 = 0.62;
const UI_FONT: &str = "static/fonts/DejaVuSansMono.ttf";

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
pub(crate) struct IndustryPanelDetails;

#[derive(Component)]
pub(crate) struct IndustryPanelCloseButton;

fn industry_gfx(tile: &openttdrs_core::Tile) -> u16 {
    u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8)
}

fn flood_industry_tiles(map: &Map, start: TileCoord) -> Vec<TileCoord> {
    let Some(start_tile) = map.get(start) else {
        return Vec::new();
    };
    if start_tile.kind != TileKind::Industry {
        return Vec::new();
    }
    let start_industry_id = start_tile.m1;
    let require_same_industry_id = start_industry_id != 0;
    let start_gfx_group = industry_group_from_gfx(industry_gfx(&start_tile));
    let require_same_gfx_group = start_gfx_group != "Unknown gfx";
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
            if let Some(tile) = map.get(n)
                && tile.kind == TileKind::Industry
                && (!require_same_industry_id || tile.m1 == start_industry_id)
                && (!require_same_gfx_group
                    || industry_group_from_gfx(industry_gfx(&tile)) == start_gfx_group)
            {
                q.push_back(n);
            }
        }
    }
    out
}

fn dominant_gfx_for_component(
    map: &Map,
    anchor: TileCoord,
) -> Option<(&'static str, TileCoord, u16)> {
    let tiles = flood_industry_tiles(map, anchor);
    if tiles.is_empty() {
        return None;
    }
    let mut best_label = "Unknown gfx";
    let mut best_count = 0usize;
    let mut best_coord = anchor;
    let mut best_gfx = 0u16;
    for c in &tiles {
        let Some(tile) = map.get(*c) else {
            continue;
        };
        let gfx = industry_gfx(&tile);
        let label = industry_group_from_gfx(gfx);
        let count = tiles
            .iter()
            .filter_map(|coord| map.get(*coord))
            .map(|t| industry_group_from_gfx(industry_gfx(&t)))
            .filter(|l| *l == label)
            .count();
        if count > best_count {
            best_count = count;
            best_label = label;
            best_coord = *c;
            best_gfx = gfx;
        }
    }
    Some((best_label, best_coord, best_gfx))
}

fn industry_stats_for_component(
    map: &Map,
    sim: &SimWorld,
    anchor: TileCoord,
) -> Option<(IndustryKind, Option<IndustrySpec>, u32, u32, TileCoord)> {
    let tiles = flood_industry_tiles(map, anchor);
    let set: HashSet<TileCoord> = tiles.into_iter().collect();
    sim.state
        .industries
        .iter()
        .find(|i| i.tiles.iter().any(|tile| set.contains(tile)) || set.contains(&i.pos))
        .map(|i| (i.kind, i.spec, i.stock, i.capacity, i.pos))
}

fn kind_label(k: IndustryKind) -> &'static str {
    match k {
        IndustryKind::CoalMine => "Carbon",
        IndustryKind::Forest => "Bosque",
        IndustryKind::OilWell => "Petróleo",
        IndustryKind::Factory => "Fábrica",
    }
}

fn spec_label(spec: IndustrySpec) -> &'static str {
    match spec {
        IndustrySpec::CoalMine => "Mina de carbón",
        IndustrySpec::IronOreMine => "Mina de hierro",
        IndustrySpec::CopperOreMine => "Mina de cobre",
        IndustrySpec::GoldMine => "Mina de oro",
        IndustrySpec::Forest => "Bosque",
        IndustrySpec::Farm => "Granja",
        IndustrySpec::OilWells => "Pozos petroleros",
        IndustrySpec::OilRefinery => "Refinería",
        IndustrySpec::Factory => "Fábrica",
        IndustrySpec::Sawmill => "Aserradero",
    }
}

fn format_panel_title(map: &Map, sim: &SimWorld, focus: TileCoord) -> String {
    if let Some((gfx_label, _coord, _gfx)) = dominant_gfx_for_component(map, focus)
        && gfx_label != "Unknown gfx"
    {
        return format!("Industria - {gfx_label} - GFX");
    }
    if let Some((kind, spec, _, _, origin)) = industry_stats_for_component(map, sim, focus) {
        return if let Some(spec) = spec {
            format!("Industria - {} - Sim", spec_label(spec))
        } else if let Some(tile) = map.get(origin) {
            let gfx = industry_gfx(&tile);
            let gfx_label = industry_group_from_gfx(gfx);
            if gfx_label != "Unknown gfx" {
                format!("Industria - {gfx_label} - GFX")
            } else {
                format!("Industria - {} - Sim", kind_label(kind))
            }
        } else {
            format!("Industria - {} - Sim", kind_label(kind))
        };
    }
    "Industria - Sin datos de simulacion".to_string()
}

pub(crate) fn setup_industry_panel(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let image = Image::new_target_texture(
        PREVIEW_TEX_W,
        PREVIEW_TEX_H,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    let rt_handle = images.add(image);
    let ui_font = asset_server.load::<Font>(UI_FONT);

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
                        font: ui_font.clone(),
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
                            font: ui_font.clone(),
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
            p.spawn((
                IndustryPanelDetails,
                Text::new("Stock: --"),
                TextFont {
                    font: ui_font,
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.88, 0.76)),
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
    mut title_q: Query<&mut Text, (With<IndustryPanelTitle>, Without<IndustryPanelDetails>)>,
    mut details_q: Query<&mut Text, (With<IndustryPanelDetails>, Without<IndustryPanelTitle>)>,
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
    if let Ok(mut details) = details_q.single_mut() {
        let tile_count = flood_industry_tiles(&sim.state.map, focus).len();
        if let Some((kind, spec, stock, capacity, origin)) =
            industry_stats_for_component(&sim.state.map, &sim, focus)
        {
            let (focus_gfx_label, _preview_anchor, focus_gfx) =
                dominant_gfx_for_component(&sim.state.map, focus).unwrap_or(("n/d", focus, 0));
            let industry_id = sim.state.map.get(origin).map_or(0, |tile| tile.m1);
            **details = format!(
                "Tipo Sim: {} | Tipo GFX: {} | Stock: {stock}/{capacity}\nOrigen sim: ({}, {}) | Industry ID(m1): {} | Tiles conectadas: {tile_count} | gfx9(raw): {focus_gfx}",
                spec.map_or_else(|| kind_label(kind), spec_label),
                focus_gfx_label,
                origin.x,
                origin.y,
                industry_id
            );
        } else {
            let gfx9 = sim
                .state
                .map
                .get(focus)
                .map_or(0, |tile| industry_gfx(&tile));
            **details = format!(
                "Tipo: desconocido en sim | GFX: n/d | Stock: n/d\nTiles conectadas: {tile_count} | gfx9(raw): {gfx9}"
            );
        }
    }

    let preview_anchor = dominant_gfx_for_component(&sim.state.map, focus)
        .map(|(_, coord, _)| coord)
        .unwrap_or(focus);
    let (_tileh, base_z) = tile_slope_and_min_z(
        &sim.state.map,
        preview_anchor.x.max(0) as u32,
        preview_anchor.y.max(0) as u32,
    );
    let pos = tile_pos(preview_anchor.x, preview_anchor.y, base_z, 0.0);

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::{App, MinimalPlugins, World};
    use openttdrs_core::IndustryKind;

    #[test]
    fn setup_industry_panel_runs() {
        let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
            file_path: asset_root.into(),
            ..default()
        });
        app.world_mut().init_resource::<Assets<Image>>();
        app.init_asset::<Font>();
        app.world_mut()
            .run_system_once(setup_industry_panel)
            .unwrap();
    }

    #[test]
    fn industry_panel_close_no_entities_is_noop() {
        let mut world = World::new();
        world.insert_resource(IndustryPanelState::default());
        world
            .run_system_once(industry_panel_close_interaction)
            .unwrap();
    }

    #[test]
    fn sync_industry_panel_closed_is_noop() {
        let mut world = World::new();
        world.insert_resource(IndustryPanelState::default());
        world.insert_resource(SimWorld::default());
        world.run_system_once(sync_industry_panel).unwrap();
    }

    #[test]
    fn industry_helper_functions_cover_paths() {
        let mut map = Map::new_flat(5, 5, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        map.set_kind(c(2, 2), TileKind::Industry).unwrap();
        map.set_kind(c(2, 3), TileKind::Industry).unwrap();
        map.set_kind(c(3, 3), TileKind::Industry).unwrap();

        let mut sim = SimWorld::default();
        sim.state.industries.clear();
        sim.state.industries.push(openttdrs_core::Industry {
            pos: c(2, 2),
            tiles: vec![c(2, 2), c(2, 3), c(3, 3)],
            spec: Some(openttdrs_core::IndustrySpec::Forest),
            kind: IndustryKind::Forest,
            stock: 0,
            capacity: 100,
        });

        let tiles = flood_industry_tiles(&map, c(2, 2));
        assert!(tiles.len() >= 3);
        assert!(flood_industry_tiles(&map, c(0, 0)).is_empty());
        let stats = industry_stats_for_component(&map, &sim, c(2, 2)).unwrap();
        assert_eq!(stats.0, IndustryKind::Forest);
        assert_eq!(stats.1, Some(openttdrs_core::IndustrySpec::Forest));
        assert_eq!(stats.2, 0);
        assert_eq!(stats.3, 100);
        assert!(spec_label(openttdrs_core::IndustrySpec::OilRefinery).contains("Refinería"));
        assert_eq!(kind_label(IndustryKind::CoalMine), "Carbon");
        assert!(format_panel_title(&map, &sim, c(2, 2)).contains("Industria"));
    }

    #[test]
    fn flood_industry_tiles_respects_m1_components_when_present() {
        let mut map = Map::new_flat(3, 1, 0);
        let c = |x: i32| TileCoord::new(x, 0);
        let mut t0 = map.get(c(0)).expect("tile 0");
        t0.kind = TileKind::Industry;
        t0.m1 = 5;
        let mut t1 = map.get(c(1)).expect("tile 1");
        t1.kind = TileKind::Industry;
        t1.m1 = 6;
        let _ = map.set_tile(c(0), t0);
        let _ = map.set_tile(c(1), t1);

        let from_left = flood_industry_tiles(&map, c(0));
        let from_right = flood_industry_tiles(&map, c(1));
        assert_eq!(from_left.len(), 1);
        assert_eq!(from_right.len(), 1);
    }

    #[test]
    fn flood_industry_tiles_respects_gfx_group_when_m1_matches() {
        let mut map = Map::new_flat(2, 1, 0);
        let c0 = TileCoord::new(0, 0);
        let c1 = TileCoord::new(1, 0);
        let mut t0 = map.get(c0).expect("tile 0");
        t0.kind = TileKind::Industry;
        t0.m1 = 7;
        t0.m5 = 18; // Oil Refinery
        let mut t1 = map.get(c1).expect("tile 1");
        t1.kind = TileKind::Industry;
        t1.m1 = 7;
        t1.m5 = 16; // Forest
        let _ = map.set_tile(c0, t0);
        let _ = map.set_tile(c1, t1);

        let from_left = flood_industry_tiles(&map, c0);
        let from_right = flood_industry_tiles(&map, c1);
        assert_eq!(from_left.len(), 1);
        assert_eq!(from_right.len(), 1);
    }
}
