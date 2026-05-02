use bevy::prelude::*;

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;

use super::logic::{
    dominant_gfx_for_component, flood_industry_tiles, format_panel_title, industry_gfx,
    industry_stats_for_component, kind_label, spec_label,
};
use super::{
    IndustryPanelCloseButton, IndustryPanelDetails, IndustryPanelRoot, IndustryPanelState,
    IndustryPanelTitle,
};

const PREVIEW_SCALE_MUL: f32 = 0.62;

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
