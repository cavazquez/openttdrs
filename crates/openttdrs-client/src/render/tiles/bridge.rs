//! Tramos intermedios de puente sobre teselas ajenas (`IsBridgeAbove`).
//!
//! En los saves de OpenTTD solo las dos rampas son `MP_TUNNELBRIDGE`; las
//! teselas del vano (agua, césped, etc.) marcan el puente en bits 2–3 del
//! byte `type` (`mapt`): 0 = sin puente, 1 = eje X, 2 = eje Y. El tablero se
//! dibuja a la altura de la rampa (`GetBridgeHeight` = z de rampa + 1).

use bevy::prelude::*;
use openttdrs_core::{Map, Tile, TileCoord, TileKind};

use super::TILE_OVERLAP_SCALE;
use crate::iso::{TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};

/// Capas relativas dentro de la tesela: tablero sobre el agua/terreno,
/// barandilla frontal por encima del tablero, pilar por debajo.
const DECK_LAYER_FRAC: f32 = 0.08;
const FRONT_LAYER_FRAC: f32 = 0.085;
const PILLAR_LAYER_FRAC: f32 = 0.075;

/// Eje del puente que pasa por encima (`GetBridgeAxis`): 0 = X, 1 = Y.
fn bridge_above_axis(tile: Tile) -> Option<usize> {
    match (tile.mapt >> 2) & 0x3 {
        1 => Some(0),
        2 => Some(1),
        _ => None,
    }
}

/// Busca la rampa del puente caminando por el eje y devuelve `(deck_z, rail)`.
///
/// `deck_z` = z mínima de la rampa + 1 (`GetBridgeHeight`); `rail` según los
/// bits de transporte de la rampa (`TransportType`: 0 = rail).
fn bridge_deck_info(
    map: &Map,
    tx: u32,
    ty: u32,
    axis: usize,
    dims: (u32, u32),
) -> Option<(u8, bool)> {
    let (dx, dy) = if axis == 0 { (1i32, 0i32) } else { (0, 1) };
    for dir in [-1i32, 1] {
        let mut x = tx as i32;
        let mut y = ty as i32;
        loop {
            x += dx * dir;
            y += dy * dir;
            if x < 0 || y < 0 || x >= dims.0 as i32 || y >= dims.1 as i32 {
                break;
            }
            let Some(t) = map.get(TileCoord::new(x, y)) else {
                break;
            };
            if t.is_tunnel_bridge_tile() && t.m5 & 0x80 != 0 {
                let (_, min_z) = tile_slope_and_min_z(map, x as u32, y as u32);
                let rail = (t.m5 >> 2) & 0x3 == 0;
                return Some((min_z.saturating_add(1), rail));
            }
            // Solo el vano marcado puede seguir; otra cosa corta la búsqueda.
            if bridge_above_axis(t).is_none() {
                break;
            }
        }
    }
    None
}

/// Dibuja el tablero (suelo + barandilla frontal + pilares) si la tesela
/// tiene un puente por encima. La tesela subyacente ya fue dibujada.
pub(crate) fn spawn_bridge_middle(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
) {
    let Some(tile) = ctx.tile else {
        return;
    };
    let Some(axis) = bridge_above_axis(tile) else {
        return;
    };
    let Some((deck_z, rail)) = bridge_deck_info(map, ctx.tx, ctx.ty, axis, dims) else {
        return;
    };

    let deck = match (rail, axis) {
        (false, 0) => assets.road_bridge.clone(),
        (false, 1) => assets.road_bridge_y.clone(),
        (true, 0) => assets.rail_bridge.clone(),
        (true, 1) => assets.rail_bridge_y.clone(),
        _ => unreachable!(),
    };

    let spawn = |commands: &mut Commands, image: Handle<Image>, z: u8, layer: f32| {
        commands.spawn((
            MapVisualLayer,
            Sprite {
                image,
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                z,
                layer,
                TILE_HALF_H,
            ))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
        ));
    };

    // Pilares desde el suelo (o el agua) hasta el tablero.
    if tile.kind != TileKind::Void {
        for z in ctx.info.base_z..deck_z {
            spawn(
                commands,
                assets.bridge_pillar[axis].clone(),
                z,
                PILLAR_LAYER_FRAC,
            );
        }
    }
    spawn(commands, deck, deck_z, DECK_LAYER_FRAC);
    spawn(
        commands,
        assets.bridge_front[axis].clone(),
        deck_z,
        FRONT_LAYER_FRAC,
    );
}
