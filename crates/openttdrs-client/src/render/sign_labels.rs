//! Etiquetas de carteles del jugador en el viewport.

use bevy::prelude::*;

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::MapVisualLayer;
use crate::render::viewport::TileViewportBounds;
use crate::state::SimWorld;

const LABEL_Z: f32 = 901.0;
const FONT_SIZE: f32 = 10.0;
const CHAR_ADVANCE: f32 = FONT_SIZE * 0.602;
const LABEL_RAISE: f32 = 22.0;

#[derive(Component)]
pub(crate) struct SignLabel;

fn sign_in_bounds(pos: openttdrs_core::TileCoord, bounds: TileViewportBounds) -> bool {
    pos.x >= 0
        && pos.y >= 0
        && (pos.x as u32) >= bounds.tx0
        && (pos.y as u32) >= bounds.ty0
        && (pos.x as u32) < bounds.tx1
        && (pos.y as u32) < bounds.ty1
}

pub(crate) fn spawn_sign_labels(
    commands: &mut Commands,
    sim: &SimWorld,
    font: &Handle<Font>,
    bounds: TileViewportBounds,
) {
    use crate::sprites::{TransparencyOption, is_hidden, text_color, with_to_alpha};
    if is_hidden(TransparencyOption::Signs) {
        return;
    }
    for sign in &sim.state.signs {
        if !sign_in_bounds(sign.pos, bounds) {
            continue;
        }
        let (tx, ty) = (sign.pos.x, sign.pos.y);
        let (tileh, base_z) = tile_slope_and_min_z(&sim.state.map, tx as u32, ty as u32);
        let ground = tile_pos(tx, ty, base_z, 0.0);
        let center = Vec2::new(
            ground.x,
            ground.y + LABEL_RAISE + f32::from(tileh & 0xF) * 2.0,
        );
        let width = sign.name.chars().count() as f32 * CHAR_ADVANCE + 6.0;
        commands.spawn((
            MapVisualLayer,
            SignLabel,
            Sprite {
                color: with_to_alpha(
                    Color::srgba(0.12, 0.1, 0.06, 0.72),
                    TransparencyOption::Signs,
                ),
                custom_size: Some(Vec2::new(width, FONT_SIZE + 4.0)),
                ..default()
            },
            Transform::from_translation(center.extend(LABEL_Z)),
        ));
        commands.spawn((
            MapVisualLayer,
            SignLabel,
            Text2d::new(sign.name.clone()),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(text_color(
                TransparencyOption::Signs,
                Color::srgb(1.0, 0.92, 0.55),
            )),
            Transform::from_translation(center.extend(LABEL_Z + 0.1)),
        ));
    }
}

pub(crate) fn resync_sign_labels(
    commands: &mut Commands,
    label_entities: impl IntoIterator<Item = Entity>,
    sim: &SimWorld,
    font: &Handle<Font>,
    bounds: TileViewportBounds,
) {
    for entity in label_entities {
        commands.entity(entity).despawn();
    }
    spawn_sign_labels(commands, sim, font, bounds);
}
