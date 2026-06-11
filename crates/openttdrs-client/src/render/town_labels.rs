//! Etiquetas de ciudades en el viewport: «Nombre (población)» flotando sobre
//! la tesela central de cada ciudad, como los town signs de `OpenTTD`.

use bevy::prelude::*;

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::MapVisualLayer;
use crate::state::SimWorld;

/// Z fija por encima de todos los sprites del mapa (cámara en ~1000).
const LABEL_Z: f32 = 900.0;
const FONT_SIZE: f32 = 10.0;
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
    let label = format!("{} ({})", town.name, town.population);
    let size = Vec2::new(
        label.chars().count() as f32 * CHAR_ADVANCE + 6.0,
        FONT_SIZE + 4.0,
    );
    (center, size)
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

/// Crea los carteles de todas las ciudades. Se llama al construir la capa de
/// mundo, así se regeneran junto al resto de `MapVisualLayer` en los remaps.
pub(crate) fn spawn_town_labels(commands: &mut Commands, sim: &SimWorld, font: &Handle<Font>) {
    let map = &sim.state.map;
    for town in &sim.state.towns {
        let (center, bg_size) = town_label_rect(map, town);
        let label = format!("{} ({})", town.name, town.population);

        // Fondo translúcido oscuro (sign con fondo, como el cliente oficial).
        commands.spawn((
            MapVisualLayer,
            TownLabel,
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
            Text2d::new(label),
            TextFont {
                font: font.clone(),
                font_size: FONT_SIZE,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_translation(center.extend(LABEL_Z + 0.1)),
        ));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{GameState, TileCoord};

    use super::*;

    #[test]
    fn spawns_text_and_background_per_town() {
        let mut state = GameState::new(8, 8);
        state.towns.push(openttdrs_core::Town {
            id: 1,
            pos: TileCoord::new(3, 3),
            name: "Nuntburg".to_string(),
            population: 738,
        });
        let sim = SimWorld {
            state,
            ..Default::default()
        };

        let mut world = World::new();
        world.insert_resource(sim);
        world
            .run_system_once(|mut commands: Commands, sim: Res<SimWorld>| {
                spawn_town_labels(&mut commands, &sim, &Handle::default());
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
        assert_eq!(bg, 1, "un fondo por ciudad");
    }
}
