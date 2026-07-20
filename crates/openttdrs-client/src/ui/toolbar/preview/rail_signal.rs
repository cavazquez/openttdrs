//! Fantasma de colocación de señal: vía existente + sprite de señal encima (con tween).

use bevy::prelude::*;
use openttdrs_core::{
    Map, TileCoord, TileKind,
    rail_signals::{
        calendar_year_at_tick, default_signal_variant, resolve_signal_track,
        signal_facing_for_orientation, signal_placement_for_track,
    },
    tick::GameTick,
};

use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::render::TileAtlas;
use crate::sprites::{
    RAIL_TILE_SIGNALS, collect_rail_ghost_sprites, collect_signal_sprite_draws,
    rail_ghost_overlay_offset, signal_draw_pos, signal_screen_position,
};

use super::BuildGhostPreview;
use super::ghost_lerp::{GHOST_LERP_SPEED, GhostLerp};

/// Marcador de fantasma de señal (no se despawn cada frame; se actualiza con tween).
#[derive(Component)]
pub(crate) struct RailSignalGhost;

/// Sprite eléctrico verde (OpenGFX) usado si el atlas no resuelve la entrada.
const FALLBACK_SIGNAL_SPRITE_ID: u32 = 1278;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RailSignalGhostKey {
    tx: i32,
    ty: i32,
    fract_x: u8,
    fract_y: u8,
    orientation: u8,
    signal_type: u8,
    signal_variant: u8,
}

#[derive(Resource, Default)]
pub(crate) struct RailSignalGhostState {
    pub(crate) key: Option<RailSignalGhostKey>,
}

#[derive(Clone)]
struct GhostLayer {
    translation: Vec3,
    color: Color,
    image: Option<Handle<Image>>,
    atlas_sprite: Option<crate::render::AtlasSprite>,
}

struct RailSignalGhostPlan {
    key: RailSignalGhostKey,
    valid: bool,
    layers: Vec<GhostLayer>,
}

#[allow(clippy::too_many_arguments)]
fn build_rail_signal_ghost_plan(
    map: &Map,
    atlas: Option<&TileAtlas>,
    asset_server: &AssetServer,
    coord: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
    signal_type: u8,
    signal_variant: u8,
    valid: bool,
) -> Option<RailSignalGhostPlan> {
    let tile = map.get(coord).filter(|t| t.kind == TileKind::Rail)?;
    let tb = tile.m5 & 0x3F;
    let track = resolve_signal_track(tb, fract_x, fract_y)?;
    let face = signal_facing_for_orientation(track, orientation);
    let placement = signal_placement_for_track(track, face, signal_variant & 1, signal_type)?;
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let m5 = track.track_bit() | (RAIL_TILE_SIGNALS << 6);
    let sig_draws = collect_signal_sprite_draws(placement.m2, placement.m3, placement.m3hi, m5);
    let preview_tex = sig_draws
        .first()
        .map(|d| d.sprite_id)
        .unwrap_or(FALLBACK_SIGNAL_SPRITE_ID);
    let preview_pos = signal_draw_pos(track as u8, placement.sig_bit);
    let signal_xy =
        signal_screen_position(coord.x, coord.y, preview_pos, preview_tex, half_h, base_z);
    let signal_tint = ghost_signal_tint(valid, 0.0);
    let rail_tint = ghost_rail_tint(valid);
    let rail_center = tile_pos_half(coord.x, coord.y, base_z, 0.02, half_h);
    let signal_base = Vec3::new(
        signal_xy.x,
        signal_xy.y,
        tile_pos_half(coord.x, coord.y, base_z, 0.04, half_h).z,
    );

    let mut layers = Vec::new();
    if let Some(atlas) = atlas {
        let mut rail_ids = Vec::new();
        collect_rail_ghost_sprites(track.track_bit(), tileh, &mut rail_ids);
        for (i, sid) in rail_ids.iter().copied().enumerate() {
            let offset = rail_ghost_overlay_offset(sid);
            let img = atlas.get(&format!("rail_{sid}.png"));
            layers.push(GhostLayer {
                translation: rail_center + Vec3::new(offset.x, offset.y, i as f32 * 0.001),
                color: rail_tint,
                image: None,
                atlas_sprite: Some(img),
            });
        }
    }

    let mut signal_spawned = 0usize;
    if let Some(atlas) = atlas {
        for (i, draw) in sig_draws.iter().copied().enumerate() {
            let Some(img) = atlas.try_get(&format!("rail_{}.png", draw.sprite_id)) else {
                continue;
            };
            layers.push(GhostLayer {
                translation: signal_base + Vec3::new(0.0, 0.0, 0.01 + i as f32 * 0.001),
                color: signal_tint,
                image: None,
                atlas_sprite: Some(img),
            });
            signal_spawned += 1;
        }
    }

    if signal_spawned == 0 {
        let sprite_id = sig_draws
            .first()
            .map(|d| d.sprite_id)
            .unwrap_or(FALLBACK_SIGNAL_SPRITE_ID);
        layers.push(GhostLayer {
            translation: signal_base,
            color: signal_tint,
            image: Some(asset_server.load(format!("assets/opengfx/tiles/rail_{sprite_id}.png"))),
            atlas_sprite: None,
        });
    }

    Some(RailSignalGhostPlan {
        key: RailSignalGhostKey {
            tx: coord.x,
            ty: coord.y,
            fract_x,
            fract_y,
            orientation,
            signal_type,
            signal_variant: signal_variant & 1,
        },
        valid,
        layers,
    })
}

fn ghost_rail_tint(valid: bool) -> Color {
    if valid {
        Color::srgba(1.0, 1.02, 1.05, 0.38)
    } else {
        Color::srgba(1.0, 0.4, 0.35, 0.45)
    }
}

fn ghost_signal_tint(valid: bool, pulse: f32) -> Color {
    if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.75)
    } else {
        let a = 0.55 + 0.2 * pulse;
        Color::srgba(1.0, 0.35, 0.3, a)
    }
}

fn spawn_ghost_layer(commands: &mut Commands, layer: &GhostLayer) {
    let lerp = GhostLerp {
        target: layer.translation,
        speed: GHOST_LERP_SPEED,
    };
    if let Some(atlas) = &layer.atlas_sprite {
        commands.spawn((
            BuildGhostPreview,
            RailSignalGhost,
            lerp,
            atlas.sprite_colored(layer.color),
            Transform::from_translation(layer.translation),
        ));
    } else if let Some(image) = &layer.image {
        commands.spawn((
            BuildGhostPreview,
            RailSignalGhost,
            lerp,
            Sprite {
                image: image.clone(),
                color: layer.color,
                ..default()
            },
            Transform::from_translation(layer.translation),
        ));
    }
}

/// Posición de destello al colocar una señal (centro del sprite de señal).
#[must_use]
pub(crate) fn rail_signal_flash_position(
    map: &Map,
    coord: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
    tick: GameTick,
) -> Option<Vec3> {
    let tile = map.get(coord).filter(|t| t.kind == TileKind::Rail)?;
    let tb = tile.m5 & 0x3F;
    let track = resolve_signal_track(tb, fract_x, fract_y)?;
    let face = signal_facing_for_orientation(track, orientation);
    let variant = default_signal_variant(calendar_year_at_tick(tick));
    let placement =
        signal_placement_for_track(track, face, variant, openttdrs_core::SIGTYPE_BLOCK)?;
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let preview_tex = collect_signal_sprite_draws(
        placement.m2,
        placement.m3,
        placement.m3hi,
        track.track_bit() | (RAIL_TILE_SIGNALS << 6),
    )
    .first()
    .map(|d| d.sprite_id)
    .unwrap_or(FALLBACK_SIGNAL_SPRITE_ID);
    let preview_pos = signal_draw_pos(track as u8, placement.sig_bit);
    let signal_xy =
        signal_screen_position(coord.x, coord.y, preview_pos, preview_tex, half_h, base_z);
    Some(Vec3::new(
        signal_xy.x,
        signal_xy.y,
        tile_pos_half(coord.x, coord.y, base_z, 0.04, half_h).z,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_rail_signal_ghost_preview(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    atlas: Option<Res<TileAtlas>>,
    mut state: ResMut<RailSignalGhostState>,
    map: &Map,
    coord: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
    signal_type: u8,
    signal_variant: u8,
    valid: bool,
    mut existing: Query<(Entity, &mut GhostLerp, &mut Sprite), With<RailSignalGhost>>,
) {
    let Some(plan) = build_rail_signal_ghost_plan(
        map,
        atlas.as_deref(),
        &asset_server,
        coord,
        orientation,
        fract_x,
        fract_y,
        signal_type,
        signal_variant,
        valid,
    ) else {
        state.key = None;
        return;
    };

    let pulse = (time.elapsed_secs() * 7.0).sin().abs();
    let key_match = state.key == Some(plan.key);
    if !key_match || existing.is_empty() || existing.iter().len() != plan.layers.len() {
        for (entity, _, _) in &existing {
            commands.entity(entity).despawn();
        }
        state.key = Some(plan.key);
        for layer in &plan.layers {
            let mut tinted = layer.clone();
            if tinted.image.is_some() || tinted.translation.z > 0.01 {
                tinted.color = ghost_signal_tint(plan.valid, pulse);
            }
            spawn_ghost_layer(&mut commands, &tinted);
        }
        return;
    }

    for ((_, mut lerp, mut sprite), layer) in existing.iter_mut().zip(plan.layers.iter()) {
        lerp.target = layer.translation;
        if layer.image.is_some() || layer.translation.z > 0.01 {
            sprite.color = ghost_signal_tint(plan.valid, pulse);
        } else {
            sprite.color = ghost_rail_tint(plan.valid);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use openttdrs_core::{Map, rail_signals::RAIL_TILE_NORMAL};

    const RAIL_TB_X: u8 = 0x01;

    fn flat_rail_x_map() -> Map {
        let mut map = Map::new_flat(5, 5, 0);
        let c = TileCoord::new(2, 2);
        map.set_kind(c, TileKind::Rail).expect("kind");
        let mut t = map.get(c).expect("tile");
        t.m5 = RAIL_TB_X | (RAIL_TILE_NORMAL << 6);
        map.set_tile(c, t).expect("tile");
        map
    }

    fn signal_draws_for_face(map: &Map, face: u8) -> Vec<crate::sprites::SignalSpriteDraw> {
        let tb = map.get(TileCoord::new(2, 2)).expect("tile").m5 & 0x3F;
        let track = resolve_signal_track(tb, 128, 128).expect("track");
        let variant = default_signal_variant(1950);
        let placement =
            signal_placement_for_track(track, face, variant, openttdrs_core::SIGTYPE_BLOCK)
                .expect("placement");
        let m5 = track.track_bit() | (RAIL_TILE_SIGNALS << 6);
        collect_signal_sprite_draws(placement.m2, placement.m3, placement.m3hi, m5)
    }

    #[test]
    fn preview_resolves_signal_sprite_for_diagonal_track() {
        let map = flat_rail_x_map();
        let draws = signal_draws_for_face(&map, 0);
        assert!(
            !draws.is_empty(),
            "preview debe resolver al menos un sprite de señal"
        );
        assert_ne!(
            draws[0].sprite_id, 704,
            "sprite de señal no debe ser el cursor de demolición (704)"
        );
        assert_ne!(
            draws[0].sprite_id, 1565,
            "sprite de señal no debe coincidir con ui_demolish (1565)"
        );
        assert_eq!(
            draws[0].sprite_id, 1278,
            "NE eléctrica (1950+) → textura 1278, no topadora 1419"
        );
    }

    #[test]
    fn preview_sw_facing_maps_electric_texture_to_classic_block() {
        let map = flat_rail_x_map();
        let draws = signal_draws_for_face(&map, 2);
        assert!(!draws.is_empty());
        assert_eq!(
            draws[0].sprite_id, 1276,
            "SW eléctrica (1950+) → textura 1276, no topadora 1417"
        );
    }
}
