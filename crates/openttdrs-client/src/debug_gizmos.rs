//! Overlays de debug opcionales dibujados con gizmos de Bevy.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::math::Isometry2d;
use bevy::prelude::*;
use openttdrs_core::IndustryKind;

use crate::bevy_app::UpdateSet;
use crate::config;
use crate::iso::{gizmo_diamond, iso};
use crate::render::MapVisualLayer;
use crate::settings::ClientPreferences;
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct DebugGizmosPlugin;

impl Plugin for DebugGizmosPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_diagnostics_overlay, update_diagnostics_overlay_text).in_set(UpdateSet::Status),
        );
        app.add_systems(
            Update,
            (draw_industries, draw_stations)
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
        app.add_systems(
            OnEnter(ClientScreen::InGame),
            spawn_diagnostics_overlay.in_set(crate::bevy_app::StartupSet::Ui),
        );
    }
}

#[derive(Component)]
pub(crate) struct DiagnosticsOverlayRoot;

#[derive(Component)]
struct DiagnosticsOverlayText;

fn show_debug_gizmos(prefs: &ClientPreferences) -> bool {
    prefs.show_debug_gizmos || config::env_flag("OPENTTDRS_GIZMOS")
}

fn show_diagnostics_overlay(prefs: &ClientPreferences) -> bool {
    prefs.show_diagnostics_overlay || config::env_flag("OPENTTDRS_DEBUG")
}

fn spawn_diagnostics_overlay(mut commands: Commands) {
    commands
        .spawn((
            DiagnosticsOverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Visibility::Hidden,
            ZIndex(4000),
        ))
        .with_children(|parent| {
            parent.spawn((
                DiagnosticsOverlayText,
                Text::new("FPS —"),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 1.0, 0.85)),
            ));
        });
}

fn sync_diagnostics_overlay(
    prefs: Res<ClientPreferences>,
    mut q: Query<&mut Visibility, With<DiagnosticsOverlayRoot>>,
) {
    let show = show_diagnostics_overlay(&prefs);
    for mut vis in &mut q {
        *vis = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_diagnostics_overlay_text(
    prefs: Res<ClientPreferences>,
    diagnostics: Res<DiagnosticsStore>,
    map_q: Query<(), With<MapVisualLayer>>,
    mut text_q: Query<&mut Text, With<DiagnosticsOverlayText>>,
) {
    if !show_diagnostics_overlay(&prefs) {
        return;
    }
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(bevy::diagnostic::Diagnostic::smoothed)
        .map(|f| format!("{f:.0}"))
        .unwrap_or_else(|| "—".into());
    let entities = map_q.iter().count();
    let line = format!("FPS {fps} | visuales {entities}");
    for mut text in &mut text_q {
        **text = line.clone();
    }
}

pub(crate) fn draw_industries(
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
    mut gizmos: Gizmos,
) {
    if !show_debug_gizmos(&prefs) {
        return;
    }
    for industry in &sim.state.industries {
        let center = iso(industry.pos.x, industry.pos.y);
        let color = industry_color(industry.kind);
        gizmo_diamond(&mut gizmos, center, 30.0, 14.0, color);

        let label = format!(
            "{:?} ({},{})",
            industry.kind, industry.pos.x, industry.pos.y
        );
        gizmos.text_2d(
            Isometry2d::from_translation(center + Vec2::new(0.0, 18.0)),
            &label,
            10.0,
            Vec2::ZERO,
            Color::WHITE,
        );

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

pub(crate) fn draw_stations(sim: Res<SimWorld>, prefs: Res<ClientPreferences>, mut gizmos: Gizmos) {
    if !show_debug_gizmos(&prefs) {
        return;
    }
    for station in &sim.state.stations {
        let center = iso(station.pos.x, station.pos.y);
        gizmo_diamond(&mut gizmos, center, 26.0, 12.0, Color::srgb(0.0, 0.9, 0.9));

        let label = format!(
            "{:?} ({},{})",
            station.stop_kind, station.pos.x, station.pos.y
        );
        gizmos.text_2d(
            Isometry2d::from_translation(center + Vec2::new(0.0, 16.0)),
            &label,
            10.0,
            Vec2::ZERO,
            Color::WHITE,
        );

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
        app.init_resource::<ClientPreferences>();
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

    #[test]
    fn overlay_flags_respect_prefs() {
        let mut prefs = ClientPreferences::default();
        assert!(!show_debug_gizmos(&prefs));
        prefs.show_debug_gizmos = true;
        assert!(show_debug_gizmos(&prefs));
    }
}
