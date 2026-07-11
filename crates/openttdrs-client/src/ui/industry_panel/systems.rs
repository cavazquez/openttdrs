use bevy::prelude::*;
use openttdrs_core::industry::{INDUSTRY_PRODUCE_AMOUNT, industry_produce_period_ticks};

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::{IndustryPreviewCamera, MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;

use super::logic::{
    dominant_gfx_for_component, flood_industry_tiles, format_panel_title, industry_gfx,
    industry_stats_for_component, kind_label, spec_label,
};
use super::{
    IndustryPanelCenterButton, IndustryPanelCloseButton, IndustryPanelDetails, IndustryPanelRoot,
    IndustryPanelState, IndustryPanelTitle,
};
use crate::ui::industry_directory::industry_chain_label;
use crate::ui::sparkline::sparkline_u32;

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

pub(crate) fn industry_panel_center_interaction(
    q: Query<&Interaction, (Changed<Interaction>, With<IndustryPanelCenterButton>)>,
    panel: Res<IndustryPanelState>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    let Some(focus) = panel.focus_tile else {
        return;
    };
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let height = sim.state.map.get(focus).map_or(0, |tile| tile.height);
        let center = tile_pos(focus.x, focus.y, height, 0.0);
        if let Ok(mut transform) = cam_q.single_mut() {
            transform.translation.x = center.x;
            transform.translation.y = center.y;
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
    primary_proj: Query<&Projection, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
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
            let type_label = spec.map_or_else(|| kind_label(kind), spec_label);
            let chain = sim
                .state
                .industries
                .iter()
                .find(|i| i.pos == origin)
                .map(industry_chain_label)
                .unwrap_or_else(|| "—".to_string());
            let period = industry_produce_period_ticks(kind);
            let industry = sim.state.industries.iter().find(|i| i.pos == origin);
            let hist_block = industry.map_or_else(
                || "Historial: —".to_string(),
                |ind| {
                    if ind.history.samples.is_empty() {
                        "Historial mensual: (avanza el tiempo)".to_string()
                    } else {
                        let produced: Vec<u32> =
                            ind.history.samples.iter().map(|s| s.produced).collect();
                        let transported: Vec<u32> =
                            ind.history.samples.iter().map(|s| s.transported).collect();
                        let stock: Vec<u32> =
                            ind.history.samples.iter().map(|s| s.stock).collect();
                        format!(
                            "Historial ({} m):\n  Stock       {}\n  Producido   {}\n  Transport.  {}",
                            ind.history.samples.len(),
                            sparkline_u32(&stock, 24),
                            sparkline_u32(&produced, 24),
                            sparkline_u32(&transported, 24),
                        )
                    }
                },
            );
            **details = format!(
                "{type_label}\nPosición: ({}, {}) · tiles: {tile_count}\nStock: {stock}/{capacity}\nProducción: +{INDUSTRY_PRODUCE_AMOUNT} cada {period} ticks\nCadena: {chain}\n\n{hist_block}",
                origin.x, origin.y,
            );
        } else {
            let gfx9 = sim
                .state
                .map
                .get(focus)
                .map_or(0, |tile| industry_gfx(&tile));
            **details = format!(
                "Industria sin datos de simulación\nTiles conectadas: {tile_count}\n(gfx {gfx9})"
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
