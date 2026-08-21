//! Etiquetas de ciudades en el viewport: «Nombre (población)» flotando sobre
//! la tesela central de cada ciudad, como los town signs de `OpenTTD`.

use bevy::prelude::*;

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::viewport::TileViewportBounds;
use crate::render::{MapLabelLod, MapLabelText, MapVisualLayer};
use crate::state::SimWorld;

/// Z fija por encima de todos los sprites del mapa (cámara en ~1000).
const LABEL_Z: f32 = 900.0;
const FONT_SIZE: f32 = 10.0;
const SMALL_FONT_SIZE: f32 = 7.0;
/// Avance horizontal aproximado por carácter de `DejaVuSansMono` (0.602 em).
const CHAR_ADVANCE: f32 = FONT_SIZE * 0.602;
/// Altura del cartel sobre el suelo de la tesela, en píxeles de pantalla.
const LABEL_RAISE: f32 = 18.0;

/// Marcador de las entidades del cartel (texto y fondo).
#[derive(Component)]
pub(crate) struct TownLabel;

/// Centro y tamaño (mundo) del cartel de una ciudad; mismo cálculo que el spawn.
pub(crate) fn town_label_rect(
    map: &openttdrs_core::Map,
    town: &openttdrs_core::Town,
) -> (Vec2, Vec2) {
    let (tx, ty) = (town.pos.x, town.pos.y);
    let (tileh, base_z) = tile_slope_and_min_z(map, tx as u32, ty as u32);
    let ground = tile_pos(tx, ty, base_z, 0.0);
    let center = Vec2::new(
        ground.x,
        ground.y + LABEL_RAISE + f32::from(tileh & 0xF) * 2.0,
    );
    let normal_label = format!("{} ({})", town.name, town.population);
    let size = Vec2::new(
        normal_label.chars().count() as f32 * CHAR_ADVANCE + 6.0,
        FONT_SIZE + 4.0,
    );
    let small_size = Vec2::new(
        town.name.chars().count() as f32 * (SMALL_FONT_SIZE * 0.602) + 5.0,
        SMALL_FONT_SIZE + 4.0,
    );
    (center, size.max(small_size))
}

/// `true` si la tesela de la ciudad cae dentro del rectángulo de spawn del mapa.
#[must_use]
pub(crate) fn town_label_in_bounds(
    town: &openttdrs_core::Town,
    bounds: TileViewportBounds,
) -> bool {
    let tx = town.pos.x;
    let ty = town.pos.y;
    tx >= 0
        && ty >= 0
        && (tx as u32) >= bounds.tx0
        && (ty as u32) >= bounds.ty0
        && (tx as u32) < bounds.tx1
        && (ty as u32) < bounds.ty1
}

/// Ciudad cuyo cartel contiene `world_pos`, si hay alguna.
pub(crate) fn town_id_at_label_pos(sim: &SimWorld, world_pos: Vec2) -> Option<u32> {
    sim.state.towns.iter().find_map(|town| {
        let (center, size) = town_label_rect(&sim.state.map, town);
        let half = size * 0.5;
        ((world_pos.x - center.x).abs() <= half.x && (world_pos.y - center.y).abs() <= half.y)
            .then_some(town.id)
    })
}

/// Crea los carteles de las ciudades dentro de `bounds`.
/// Se llama al construir la capa de mundo y al panear en mapas con culling.
pub(crate) fn spawn_town_labels(
    commands: &mut Commands,
    sim: &SimWorld,
    font: &Handle<Font>,
    bounds: TileViewportBounds,
    show_town_labels: bool,
) {
    if !show_town_labels {
        return;
    }
    let map = &sim.state.map;
    for town in &sim.state.towns {
        if !town_label_in_bounds(town, bounds) {
            continue;
        }
        let (center, bg_size) = town_label_rect(map, town);
        let normal_label = format!("{} ({})", town.name, town.population);
        let small_label = town.name.clone();
        let small_size = Vec2::new(
            small_label.chars().count() as f32 * (SMALL_FONT_SIZE * 0.602) + 5.0,
            SMALL_FONT_SIZE + 4.0,
        );

        // Fondo translúcido oscuro (sign con fondo, como el cliente oficial).
        commands.spawn((
            MapVisualLayer,
            TownLabel,
            MapLabelLod {
                size: bg_size,
                small_size,
            },
            Sprite {
                color: Color::srgba(0.08, 0.10, 0.14, 0.65),
                custom_size: Some(bg_size),
                ..default()
            },
            Transform::from_translation(center.extend(LABEL_Z)),
        ));
        commands.spawn((
            MapVisualLayer,
            TownLabel,
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
            TextColor(Color::WHITE),
            Transform::from_translation(center.extend(LABEL_Z + 0.1)),
        ));
    }
}

/// Despawn de carteles previos y spawn según el viewport actual.
pub(crate) fn resync_town_labels(
    commands: &mut Commands,
    label_entities: impl IntoIterator<Item = Entity>,
    sim: &SimWorld,
    font: &Handle<Font>,
    bounds: TileViewportBounds,
    show_town_labels: bool,
) {
    for entity in label_entities {
        commands.entity(entity).despawn();
    }
    spawn_town_labels(commands, sim, font, bounds, show_town_labels);
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::prelude::*;

    use super::*;

    #[test]
    fn town_label_in_bounds_filters_by_rect() {
        let town = openttdrs_core::Town {
            id: 1,
            pos: TileCoord::new(5, 8),
            name: "A".into(),
            population: 1,
            ..Default::default()
        };
        let inside = TileViewportBounds {
            tx0: 0,
            ty0: 0,
            tx1: 10,
            ty1: 10,
        };
        let outside = TileViewportBounds {
            tx0: 0,
            ty0: 0,
            tx1: 4,
            ty1: 4,
        };
        assert!(town_label_in_bounds(&town, inside));
        assert!(!town_label_in_bounds(&town, outside));
    }

    #[test]
    fn spawns_text_and_background_per_town_in_bounds() {
        let mut state = GameState::new(8, 8);
        state.towns.push(openttdrs_core::Town {
            id: 1,
            pos: TileCoord::new(3, 3),
            name: "Nuntburg".to_string(),
            population: 738,
            ..Default::default()
        });
        state.towns.push(openttdrs_core::Town {
            id: 2,
            pos: TileCoord::new(7, 7),
            name: "Farville".to_string(),
            population: 100,
            ..Default::default()
        });
        let sim = SimWorld {
            state,
            ..Default::default()
        };
        let bounds = TileViewportBounds {
            tx0: 0,
            ty0: 0,
            tx1: 5,
            ty1: 5,
        };

        let mut world = World::new();
        world.insert_resource(sim);
        world
            .run_system_once(move |mut commands: Commands, sim: Res<SimWorld>| {
                spawn_town_labels(&mut commands, &sim, &Handle::default(), bounds, true);
            })
            .expect("spawn labels");

        let texts: Vec<String> = world
            .query::<&Text2d>()
            .iter(&world)
            .map(|t| t.0.clone())
            .collect();
        assert_eq!(texts, vec!["Nuntburg (738)".to_string()]);
        let bg = world
            .query_filtered::<&Sprite, With<TownLabel>>()
            .iter(&world)
            .count();
        assert_eq!(bg, 1, "un fondo por ciudad visible");
    }
}
