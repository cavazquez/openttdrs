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
        let color = industry_color(industry.kind);
        gizmo_diamond(&mut gizmos, center, 30.0, 14.0, color);

        if let Some(bar_w) = industry_bar_width(industry.stock, industry.capacity) {
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

        if let Some(bar_w) = station_bar_width(station.income) {
            let bar_y = center.y - 10.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
}

fn industry_color(kind: IndustryKind) -> Color {
    match kind {
        IndustryKind::CoalMine => Color::srgb(1.0, 0.9, 0.1),
        IndustryKind::Forest => Color::srgb(1.0, 0.5, 0.05),
        IndustryKind::OilWell => Color::srgb(0.35, 0.55, 1.0),
        IndustryKind::Factory => Color::srgb(0.85, 0.45, 0.95),
    }
}

fn industry_bar_width(stock: u32, capacity: u32) -> Option<f32> {
    if stock == 0 || capacity == 0 {
        return None;
    }
    let fill = stock as f32 / capacity as f32;
    Some(56.0 * fill)
}

fn station_bar_width(income: u64) -> Option<f32> {
    if income == 0 {
        return None;
    }
    let fill = ((income as f32).log2() / 10.0).min(1.0);
    Some(48.0 * fill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_build_registers_systems() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(DebugGizmosPlugin);
    }

    #[test]
    fn helper_functions_cover_branches() {
        let _ = industry_color(IndustryKind::CoalMine);
        let _ = industry_color(IndustryKind::Forest);
        let _ = industry_color(IndustryKind::OilWell);
        let _ = industry_color(IndustryKind::Factory);

        assert_eq!(industry_bar_width(0, 100), None);
        assert_eq!(industry_bar_width(10, 0), None);
        assert!(industry_bar_width(25, 100).is_some());

        assert_eq!(station_bar_width(0), None);
        assert!(station_bar_width(1).is_some());
        assert!(station_bar_width(10_000).is_some());
    }
}
