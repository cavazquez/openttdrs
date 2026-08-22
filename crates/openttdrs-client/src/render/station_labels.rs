//! Etiquetas de estaciones en el viewport (nombres sobre `station.pos`).

use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::{MapLabelCandidates, MapLabelLod, MapLabelText, MapVisualLayer};
use crate::state::SimWorld;

// OpenTTD compone town → sign → station labels en ese orden. Mantener la
// estación por encima de los carteles evita que el orden de spawn ECS decida
// cuál de los dos queda visible cuando se superponen en Out4x/Out8x.
const LABEL_Z: f32 = 902.0;
const FONT_SIZE: f32 = 9.0;
const SMALL_FONT_SIZE: f32 = 7.0;
const CHAR_ADVANCE: f32 = FONT_SIZE * 0.602;
const LABEL_RAISE: f32 = 14.0;
const LABEL_BACKGROUND_ALPHA: f32 = 1.0;
const UNOWNED_LABEL_COLOUR: Color = Color::srgb(0.42, 0.42, 0.42);

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
        StopKind::RoadWaypoint => "WP-R",
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

/// `true` si el nombre de estación/waypoint pasa los filtros del viewport.
#[must_use]
pub(crate) fn station_label_visible(
    station: &Station,
    local_company: CompanyId,
    show_waypoints: bool,
    show_competitors: bool,
) -> bool {
    let waypoint = matches!(
        station.stop_kind,
        StopKind::RailWaypoint | StopKind::RoadWaypoint
    );
    // `OWNER_NONE` no pertenece a ningún rival: el viewport oficial siempre
    // conserva estos carteles (boyas, estaciones fantasma o tras una quiebra)
    // aun cuando se ocultan las compañías competidoras.
    let owner_is_none = station.owner.0 == openttdrs_core::company::OWNER_NONE_M1;
    (!waypoint || show_waypoints)
        && (show_competitors || station.owner == local_company || owner_is_none)
}

fn station_background_colour(sim: &SimWorld, station: &Station) -> Color {
    let base = sim
        .state
        .companies
        .iter()
        .find(|company| company.id == station.owner)
        .map(|company| crate::sprites::company_colour_swatch_color(company.colour))
        .unwrap_or(UNOWNED_LABEL_COLOUR);
    let colour = base.to_srgba();
    Color::srgba(
        colour.red,
        colour.green,
        colour.blue,
        LABEL_BACKGROUND_ALPHA,
    )
}

pub(crate) fn spawn_station_labels(
    commands: &mut Commands,
    sim: &SimWorld,
    font: &Handle<Font>,
    candidates: &MapLabelCandidates,
    show: bool,
    show_waypoints: bool,
    show_competitors: bool,
) {
    if !show {
        return;
    }
    let map = &sim.state.map;
    for &index in &candidates.stations {
        let Some(station) = sim.state.stations.get(index) else {
            continue;
        };
        if !station_label_visible(
            station,
            sim.state.active_company,
            show_waypoints,
            show_competitors,
        ) {
            continue;
        }
        let (center, bg_size) = station_label_rect(map, station);
        let normal_label = station_display_name(station);
        let small_label = normal_label.clone();
        let small_size = Vec2::new(
            small_label.chars().count() as f32 * (SMALL_FONT_SIZE * 0.602) + 5.0,
            SMALL_FONT_SIZE + 4.0,
        );
        commands.spawn((
            MapVisualLayer,
            StationLabel,
            MapLabelLod {
                size: bg_size,
                small_size,
            },
            Sprite {
                color: station_background_colour(sim, station),
                custom_size: Some(bg_size),
                ..default()
            },
            Transform::from_translation(center.extend(LABEL_Z)),
        ));
        commands.spawn((
            MapVisualLayer,
            StationLabel,
            MapLabelLod {
                size: bg_size,
                small_size,
            },
            MapLabelText {
                normal: normal_label.clone(),
                small: small_label,
            },
            Text2d::new(normal_label),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(Color::srgb(0.05, 0.05, 0.05)),
            Transform::from_translation(center.extend(LABEL_Z + 0.1)),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resync_station_labels(
    commands: &mut Commands,
    label_entities: impl IntoIterator<Item = Entity>,
    sim: &SimWorld,
    font: &Handle<Font>,
    candidates: &MapLabelCandidates,
    show: bool,
    show_waypoints: bool,
    show_competitors: bool,
) {
    for entity in label_entities {
        commands.entity(entity).despawn();
    }
    spawn_station_labels(
        commands,
        sim,
        font,
        candidates,
        show,
        show_waypoints,
        show_competitors,
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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

    #[test]
    fn competitor_and_waypoint_filters_match_viewport_policy() {
        let local = CompanyId::PLAYER;
        let mut rival = Station::new_with_kind(TileCoord::new(2, 3), StopKind::RailStation);
        rival.owner = CompanyId(1);
        assert!(!station_label_visible(&rival, local, true, false));
        assert!(station_label_visible(&rival, local, true, true));

        let waypoint = Station::new_with_kind(TileCoord::new(2, 3), StopKind::RailWaypoint);
        assert!(!station_label_visible(&waypoint, local, false, true));
        assert!(station_label_visible(&waypoint, local, true, true));

        let mut unowned = Station::new_with_kind(TileCoord::new(2, 3), StopKind::Buoy);
        unowned.owner = CompanyId(openttdrs_core::company::OWNER_NONE_M1);
        assert!(station_label_visible(&unowned, local, true, false));
    }
}
