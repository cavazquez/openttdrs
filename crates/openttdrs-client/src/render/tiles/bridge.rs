//! Tramos intermedios de puente sobre teselas ajenas (`IsBridgeAbove`).
//!
//! En los saves de OpenTTD solo las dos rampas son `MP_TUNNELBRIDGE`; las
//! teselas del vano (agua, césped, etc.) marcan el puente en bits 2–3 del
//! byte `type` (`mapt`): 0 = sin puente, 1 = eje X, 2 = eje Y.
//!
//! Réplica de `DrawBridgeMiddle` (`tunnelbridge_cmd.cpp`) para el puente de
//! madera: rear (suelo + barandilla trasera) y front (barandilla frontal,
//! desplazada +12 unidades de mundo hacia la cámara) se dibujan con sus
//! offsets NFO a `z = altura_tablero − BRIDGE_Z_START`; los pilares bajan en
//! columnas de `TILE_HEIGHT` px desde el tablero hasta el suelo.

use bevy::prelude::*;
use openttdrs_core::{Map, Tile, TileCoord, TileKind};

use crate::iso::{HEIGHT_PX, remap_tile_offset, tile_slope_and_min_z};
use crate::render::{AtlasSprite, MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    BRIDGE_WOOD_FRONT_META, BRIDGE_WOOD_PILLAR_META, BRIDGE_WOOD_REAR_RAIL_META,
    BRIDGE_WOOD_REAR_ROAD_META,
};

/// Capas relativas dentro de la tesela: pilares por debajo del tablero,
/// barandilla frontal por encima.
const DECK_LAYER_FRAC: f32 = 0.08;
const FRONT_LAYER_FRAC: f32 = 0.085;
const PILLAR_BACK_LAYER_FRAC: f32 = 0.074;
const PILLAR_LAYER_FRAC: f32 = 0.075;

/// `BRIDGE_Z_START` (`tunnelbridge_cmd.cpp`): el tablero se dibuja 3 px de
/// mundo por debajo de la altura lógica del puente.
const BRIDGE_Z_START: f32 = 3.0;
/// `TILE_HEIGHT` de OpenTTD en px de mundo (= [`HEIGHT_PX`] en pantalla).
const TILE_HEIGHT_PX: f32 = 8.0;

/// Eje del puente que pasa por encima (`GetBridgeAxis`): 0 = X, 1 = Y.
fn bridge_above_axis(tile: Tile) -> Option<usize> {
    match (tile.mapt >> 2) & 0x3 {
        1 => Some(0),
        2 => Some(1),
        _ => None,
    }
}

/// `GetBridgeHeight` (`bridge_map.cpp`): z del tablero = z de la rampa + 1
/// nivel, aplicando la fundación del cabezal (`GetBridgeFoundation`):
/// inclinada según el eje o plana → sin fundación; una esquina elevada →
/// fundación inclinada (sin Δz); resto → fundación niveladora (+1).
fn bridge_deck_z(ramp_tileh: u8, ramp_min_z: u8, axis: usize) -> u8 {
    let aligned = if axis == 0 {
        ramp_tileh == 12 || ramp_tileh == 3 // SLOPE_NE / SLOPE_SW
    } else {
        ramp_tileh == 9 || ramp_tileh == 6 // SLOPE_NW / SLOPE_SE
    };
    if ramp_tileh == 0 || aligned {
        return ramp_min_z.saturating_add(1);
    }
    let one_corner = matches!(ramp_tileh, 1 | 2 | 4 | 8);
    ramp_min_z.saturating_add(if one_corner { 1 } else { 2 })
}

/// Busca la rampa del puente caminando por el eje y devuelve `(deck_z, rail)`.
///
/// `deck_z` = altura del tablero en niveles (`GetBridgeHeight`); `rail` según
/// los bits de transporte de la rampa (`TransportType`: 0 = rail).
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
                let (tileh, min_z) = tile_slope_and_min_z(map, x as u32, y as u32);
                let rail = (t.m5 >> 2) & 0x3 == 0;
                return Some((bridge_deck_z(tileh, min_z, axis), rail));
            }
            // Solo el vano marcado puede seguir; otra cosa corta la búsqueda.
            if bridge_above_axis(t).is_none() {
                break;
            }
        }
    }
    None
}

/// Alturas en px de mundo del suelo bajo los bordes frontal (SE para eje X,
/// SW para eje Y) y trasero de la tesela, desde `tileh`/`base_z` (análogo a
/// `GetSlopePixelZOnEdge` con el máximo de las dos esquinas del borde).
fn pillar_ground_px(tileh: u8, base_z: u8, axis: usize) -> (f32, f32) {
    let corner = |bit: u8| f32::from(base_z) + f32::from((tileh >> bit) & 1);
    let (w, s, e, n) = (corner(0), corner(1), corner(2), corner(3));
    let (front, back) = if axis == 0 {
        (e.max(s), n.max(w)) // eje X: frente = borde SE
    } else {
        (s.max(w), n.max(e)) // eje Y: frente = borde SW
    };
    (front * TILE_HEIGHT_PX, back * TILE_HEIGHT_PX)
}

/// Dibuja el tablero (rear + front + pilares) si la tesela tiene un puente
/// por encima. La tesela subyacente ya fue dibujada.
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

    let deck_image = match (rail, axis) {
        (false, 0) => &assets.road_bridge,
        (false, 1) => &assets.road_bridge_y,
        (true, 0) => &assets.rail_bridge,
        (true, 1) => &assets.rail_bridge_y,
        _ => unreachable!(),
    };

    // Posición tipo `AddSortableSpriteToDraw`: esquina norte de la tesela +
    // desplazamiento de mundo + offsets NFO; `z_px` sube la pantalla 1:1.
    let spawn = |commands: &mut Commands,
                 image: &AtlasSprite,
                 meta: (f32, f32, f32, f32),
                 shift: Vec2,
                 z_px: f32,
                 layer: f32| {
        let (w, h, xrel, yrel) = meta;
        let pos = Vec3::new(
            ctx.iso_pos.x + shift.x + xrel + w / 2.0,
            ctx.iso_pos.y + shift.y - yrel - h / 2.0 + z_px,
            (ctx.tx_i32() + ctx.ty_i32()) as f32 * 0.01 + f32::from(deck_z) * 0.001 + layer,
        );
        commands.spawn((
            MapVisualLayer,
            image.sprite(),
            Transform::from_translation(pos),
        ));
    };

    let z_draw_px = f32::from(deck_z) * HEIGHT_PX - BRIDGE_Z_START;

    // Barandilla frontal: +12 unidades de mundo perpendiculares al eje.
    let front_shift = if axis == 0 {
        remap_tile_offset(0.0, 12.0, 0.0) * 0.5
    } else {
        remap_tile_offset(12.0, 0.0, 0.0) * 0.5
    };
    // Pilar trasero: 9 unidades hacia atrás desde el frontal (`back_pillar_offset`).
    let back_shift = if axis == 0 {
        remap_tile_offset(0.0, 3.0, 0.0) * 0.5
    } else {
        remap_tile_offset(3.0, 0.0, 0.0) * 0.5
    };

    let rear_meta = if rail {
        BRIDGE_WOOD_REAR_RAIL_META[axis]
    } else {
        BRIDGE_WOOD_REAR_ROAD_META[axis]
    };
    spawn(
        commands,
        deck_image,
        rear_meta,
        Vec2::ZERO,
        z_draw_px,
        DECK_LAYER_FRAC,
    );
    spawn(
        commands,
        &assets.bridge_front[axis],
        BRIDGE_WOOD_FRONT_META[axis],
        front_shift,
        z_draw_px,
        FRONT_LAYER_FRAC,
    );

    // Pilares (`DrawBridgePillars`): columna frontal desde el tablero hasta
    // el suelo y columna trasera (saltando los dos tramos tapados por el
    // tablero).
    if tile.kind != TileKind::Void {
        let (front_ground_px, back_ground_px) =
            pillar_ground_px(ctx.info.tileh, ctx.info.base_z, axis);
        let pillar_meta = BRIDGE_WOOD_PILLAR_META[axis];
        let mut cur_z = z_draw_px;
        while cur_z >= front_ground_px {
            spawn(
                commands,
                &assets.bridge_pillar[axis],
                pillar_meta,
                front_shift,
                cur_z,
                PILLAR_LAYER_FRAC,
            );
            cur_z -= TILE_HEIGHT_PX;
        }
        let back_top_px = z_draw_px - 2.0 * TILE_HEIGHT_PX;
        if back_ground_px <= back_top_px {
            let mut cur_z = back_top_px;
            while cur_z >= back_ground_px {
                spawn(
                    commands,
                    &assets.bridge_pillar[axis],
                    pillar_meta,
                    back_shift,
                    cur_z,
                    PILLAR_BACK_LAYER_FRAC,
                );
                cur_z -= TILE_HEIGHT_PX;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_z_flat_and_axis_aligned_ramp_is_min_z_plus_one() {
        assert_eq!(bridge_deck_z(0, 2, 0), 3);
        assert_eq!(
            bridge_deck_z(12, 1, 0),
            2,
            "SLOPE_NE en eje X: sin fundación"
        );
        assert_eq!(
            bridge_deck_z(3, 1, 0),
            2,
            "SLOPE_SW en eje X: sin fundación"
        );
        assert_eq!(
            bridge_deck_z(9, 1, 1),
            2,
            "SLOPE_NW en eje Y: sin fundación"
        );
        assert_eq!(
            bridge_deck_z(6, 1, 1),
            2,
            "SLOPE_SE en eje Y: sin fundación"
        );
    }

    #[test]
    fn deck_z_one_corner_uses_inclined_foundation() {
        // Una esquina elevada → fundación inclinada, sin Δz extra.
        for tileh in [1u8, 2, 4, 8] {
            assert_eq!(bridge_deck_z(tileh, 0, 0), 1);
        }
    }

    #[test]
    fn deck_z_other_slopes_use_leveling_foundation() {
        // Pendiente perpendicular / 2-3 esquinas → fundación niveladora (+1).
        assert_eq!(bridge_deck_z(9, 0, 0), 2, "SLOPE_NW en eje X se nivela");
        assert_eq!(bridge_deck_z(6, 0, 0), 2, "SLOPE_SE en eje X se nivela");
        assert_eq!(bridge_deck_z(12, 0, 1), 2, "SLOPE_NE en eje Y se nivela");
        for tileh in [7u8, 11, 13, 14] {
            assert_eq!(bridge_deck_z(tileh, 0, 0), 2);
        }
    }

    #[test]
    fn pillar_ground_uses_max_corner_of_each_edge() {
        // Plano a z=1: ambos bordes a 8 px.
        assert_eq!(pillar_ground_px(0, 1, 0), (8.0, 8.0));
        // SLOPE_E (4) en eje X: frente (borde SE) sube, atrás no.
        assert_eq!(pillar_ground_px(4, 0, 0), (8.0, 0.0));
        // SLOPE_N (8) en eje X: frente al ras, atrás sube.
        assert_eq!(pillar_ground_px(8, 0, 0), (0.0, 8.0));
    }
}
