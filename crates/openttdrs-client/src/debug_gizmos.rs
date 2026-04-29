//! Overlays de debug opcionales dibujados con gizmos de Bevy.

use bevy::prelude::*;
use openttdrs_core::IndustryKind;

use crate::bevy_app::UpdateSet;
use crate::config::env_flag;
use crate::iso::{gizmo_diamond, iso};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct DebugGizmosPlugin;

impl Plugin for DebugGizmosPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_industries, draw_stations)
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

pub(crate) fn draw_industries(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    if !env_flag("OPENTTDRS_GIZMOS") {
        return;
    }
    for industry in &sim.state.industries {
        let center = iso(industry.pos.x, industry.pos.y);
        let color = match industry.kind {
            IndustryKind::CoalMine => Color::srgb(1.0, 0.9, 0.1),
            IndustryKind::Forest => Color::srgb(1.0, 0.5, 0.05),
            IndustryKind::OilWell => Color::srgb(0.35, 0.55, 1.0),
            IndustryKind::Factory => Color::srgb(0.85, 0.45, 0.95),
        };
        gizmo_diamond(&mut gizmos, center, 30.0, 14.0, color);

        if industry.stock > 0 {
            let fill = industry.stock as f32 / industry.capacity as f32;
            let bar_w = 56.0 * fill;
            let bar_y = center.y - 12.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::WHITE,
            );
        }
    }
}

pub(crate) fn draw_stations(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    if !env_flag("OPENTTDRS_GIZMOS") {
        return;
    }
    for station in &sim.state.stations {
        let center = iso(station.pos.x, station.pos.y);
        gizmo_diamond(&mut gizmos, center, 26.0, 12.0, Color::srgb(0.0, 0.9, 0.9));

        if station.income > 0 {
            let fill = ((station.income as f32).log2() / 10.0).min(1.0);
            let bar_w = 48.0 * fill;
            let bar_y = center.y - 10.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
}
