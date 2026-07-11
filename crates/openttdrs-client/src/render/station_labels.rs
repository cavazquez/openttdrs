//! Etiquetas de estaciones en el viewport (nombres sobre `station.pos`).

use bevy::prelude::*;
use openttdrs_core::{Station, StopKind};

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::MapVisualLayer;
use crate::render::viewport::TileViewportBounds;
use crate::state::SimWorld;

const LABEL_Z: f32 = 901.0;
const FONT_SIZE: f32 = 9.0;
const CHAR_ADVANCE: f32 = FONT_SIZE * 0.602;
const LABEL_RAISE: f32 = 14.0;

#[derive(Component)]
pub(crate) struct StationLabel;

#[must_use]
pub(crate) fn station_display_name(station: &Station) -> String {
    station
        .name
        .clone()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            format!(
                "{} ({}, {})",
                station_kind_short(station.stop_kind),
                station.pos.x,
                station.pos.y
            )
        })
}

const fn station_kind_short(kind: StopKind) -> &'static str {
    match kind {
        StopKind::BusStop => "Bus",
        StopKind::TruckStop => "Camión",
        StopKind::RailStation => "Tren",
        StopKind::Dock => "Muelle",
        StopKind::Buoy => "Boya",
        StopKind::Airport => "Aero",
        StopKind::RailWaypoint => "WP",
    }
}

fn station_label_rect(map: &openttdrs_core::Map, station: &Station) -> (Vec2, Vec2) {
    let (tx, ty) = (station.pos.x, station.pos.y);
    let (tileh, base_z) = tile_slope_and_min_z(map, tx as u32, ty as u32);
    let ground = tile_pos(tx, ty, base_z, 0.0);
    let center = Vec2::new(
        ground.x,
        ground.y + LABEL_RAISE + f32::from(tileh & 0xF) * 2.0,
    );
    let label = station_display_name(station);
    let size = Vec2::new(
        label.chars().count() as f32 * CHAR_ADVANCE + 6.0,
        FONT_SIZE + 4.0,
    );
    (center, size)
}

fn station_label_in_bounds(station: &Station, bounds: TileViewportBounds) -> bool {
    let tx = station.pos.x;
    let ty = station.pos.y;
    tx >= 0
        && ty >= 0
        && (tx as u32) >= bounds.tx0
        && (ty as u32) >= bounds.ty0
        && (tx as u32) < bounds.tx1
        && (ty as u32) < bounds.ty1
}

pub(crate) fn spawn_station_labels(
    commands: &mut Commands,
    sim: &SimWorld,
    font: &Handle<Font>,
    bounds: TileViewportBounds,
    show: bool,
) {
    if !show {
        return;
    }
    let map = &sim.state.map;
    for station in &sim.state.stations {
        if station.stop_kind == StopKind::RailWaypoint {
            continue;
        }
        if !station_label_in_bounds(station, bounds) {
            continue;
        }
        let (center, bg_size) = station_label_rect(map, station);
        let label = station_display_name(station);
        commands.spawn((
            MapVisualLayer,
            StationLabel,
            Sprite {
                color: Color::srgba(0.12, 0.18, 0.10, 0.70),
                custom_size: Some(bg_size),
                ..default()
            },
            Transform::from_translation(center.extend(LABEL_Z)),
        ));
        commands.spawn((
            MapVisualLayer,
            StationLabel,
            Text2d::new(label),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.96, 0.82)),
            Transform::from_translation(center.extend(LABEL_Z + 0.1)),
        ));
    }
}

pub(crate) fn resync_station_labels(
    commands: &mut Commands,
    label_entities: impl IntoIterator<Item = Entity>,
    sim: &SimWorld,
    font: &Handle<Font>,
    bounds: TileViewportBounds,
    show: bool,
) {
    for entity in label_entities {
        commands.entity(entity).despawn();
    }
    spawn_station_labels(commands, sim, font, bounds, show);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{Station, StopKind, TileCoord};

    #[test]
    fn display_name_uses_custom_or_fallback() {
        let mut st = Station::new_with_kind(TileCoord::new(2, 3), StopKind::BusStop);
        assert!(station_display_name(&st).contains("Bus"));
        st.name = Some("Central".into());
        assert_eq!(station_display_name(&st), "Central");
    }

    #[test]
    fn waypoint_skipped_from_kind_short() {
        assert_eq!(station_kind_short(StopKind::RailWaypoint), "WP");
        assert_eq!(station_kind_short(StopKind::RailStation), "Tren");
    }
}
