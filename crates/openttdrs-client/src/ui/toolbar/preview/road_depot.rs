//! Fantasma de depósito de carretera: losa + carretera + capas BUILD (como el mapa).

use bevy::prelude::*;
use openttdrs_core::TramwayDepotReplacement;

use crate::iso::{iso, road_depot_build_sprite_center, tile_pos_half};
use crate::render::{CompanyColoredSprites, sprite_from_company_or_asset};
use crate::sprites::{
    ROAD_DEPOT_GROUND_PATH, RoadDepotLayerGfx, road_depot_build_layers,
    road_depot_entrance_road_bits, road_depot_seq_gfx, road_flat_sprite_index,
    tramway_sprite_atlas_key, tramway_sprite_gfx,
};
use crate::ui::toolbar::preview::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;
const ROAD_DEPOT_SEQUENCE_SPRITE_BASE: u32 = 1408;
const TRAM_DEPOT_WITH_TRACK_SPRITE_BASE: u32 = crate::sprites::TRAMWAY_SPRITE_BASE + 49;
const TRAM_DEPOT_NO_TRACK_SPRITE_BASE: u32 = crate::sprites::TRAMWAY_SPRITE_BASE + 113;

pub(crate) struct RoadDepotPreviewSpawn<'a> {
    pub px: i32,
    pub py: i32,
    pub base_z: u8,
    pub half_h: f32,
    pub dir: usize,
    pub tint: Color,
    pub asset_server: &'a AssetServer,
    pub company: Option<&'a CompanyColoredSprites>,
    pub action5_replacement: Option<TramwayDepotReplacement>,
    pub show_tram_overlay: bool,
}

pub(crate) fn spawn_road_depot_preview(commands: &mut Commands, spawn: RoadDepotPreviewSpawn<'_>) {
    let RoadDepotPreviewSpawn {
        px,
        py,
        base_z,
        half_h,
        dir,
        tint,
        asset_server,
        company,
        action5_replacement,
        show_tram_overlay,
    } = spawn;
    let dir = dir.min(3);

    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: asset_server.load::<Image>(ROAD_DEPOT_GROUND_PATH),
            color: tint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(px, py, base_z, PREVIEW_Z_BASE, half_h))
            .with_scale(Vec3::splat(PREVIEW_SCALE)),
    ));

    if show_tram_overlay {
        let road_bits = road_depot_entrance_road_bits(dir as u8);
        let fi = road_flat_sprite_index(0, road_bits);
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: asset_server
                    .load::<Image>(format!("assets/opengfx/tiles/tram_flat_{fi:02}.png")),
                color: tint,
                ..default()
            },
            Transform::from_translation(tile_pos_half(
                px,
                py,
                base_z,
                PREVIEW_Z_BASE + 0.025,
                half_h,
            ))
            .with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }

    for spec in road_depot_build_layers(dir) {
        let (path, seq, width, height) = preview_layer_asset(action5_replacement, spec);
        let layer_z = PREVIEW_Z_BASE + spec.z;
        let center = road_depot_build_sprite_center(
            iso(px, py),
            px,
            py,
            base_z,
            layer_z,
            seq,
            width,
            height,
        );
        commands.spawn((
            BuildGhostPreview,
            sprite_from_company_or_asset(company, asset_server, &path, tint),
            Transform::from_translation(center).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}

/// Resuelve el PNG y la caja NFO de una capa BUILD relocalizada por Action5.
/// Si el bloque no tiene un sprite extraíble, conserva el fallback OpenGFX de
/// la capa original, igual que hace el renderer del mapa.
fn preview_layer_asset(
    replacement: Option<TramwayDepotReplacement>,
    spec: &RoadDepotLayerGfx,
) -> (String, crate::iso::RoadStopSeqGfx, f32, f32) {
    let Some(replacement) = replacement else {
        return (
            spec.path.to_owned(),
            road_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
    };

    let base = match replacement {
        TramwayDepotReplacement::WithTrack => TRAM_DEPOT_WITH_TRACK_SPRITE_BASE,
        TramwayDepotReplacement::NoTrack => TRAM_DEPOT_NO_TRACK_SPRITE_BASE,
    };
    let Some(offset) = spec.sprite_id.checked_sub(ROAD_DEPOT_SEQUENCE_SPRITE_BASE) else {
        return (
            spec.path.to_owned(),
            road_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
    };
    let Some(sprite_id) = base.checked_add(offset) else {
        return (
            spec.path.to_owned(),
            road_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
    };
    let (Some(key), Some(gfx)) = (
        tramway_sprite_atlas_key(sprite_id),
        tramway_sprite_gfx(sprite_id),
    ) else {
        return (
            spec.path.to_owned(),
            road_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
    };
    let mut seq = road_depot_seq_gfx(spec);
    seq.x_offs = gfx.x_offs;
    seq.y_offs = gfx.y_offs;
    (
        format!("assets/opengfx/tiles/{key}"),
        seq,
        gfx.width,
        gfx.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action5_layers_use_relocated_asset_and_nfo_geometry() {
        let spec = road_depot_build_layers(1)[1];
        let (path, seq, width, height) =
            preview_layer_asset(Some(TramwayDepotReplacement::WithTrack), &spec);
        let sprite_id = crate::sprites::TRAMWAY_SPRITE_BASE + 50;
        let gfx = tramway_sprite_gfx(sprite_id).expect("vanilla tram depot layer metadata");

        assert_eq!(path, "assets/opengfx/tiles/tramway_050.png");
        assert_eq!(seq.x_offs, gfx.x_offs);
        assert_eq!(seq.y_offs, gfx.y_offs);
        assert_eq!((width, height), (gfx.width, gfx.height));
    }

    #[test]
    fn road_layers_keep_opengfx_asset_without_action5() {
        let spec = road_depot_build_layers(0)[0];
        let (path, seq, width, height) = preview_layer_asset(None, &spec);
        let expected_seq = road_depot_seq_gfx(&spec);

        assert_eq!(path, spec.path);
        assert_eq!(seq.dx, expected_seq.dx);
        assert_eq!(seq.dy, expected_seq.dy);
        assert_eq!(seq.dz, expected_seq.dz);
        assert_eq!(seq.x_offs, expected_seq.x_offs);
        assert_eq!(seq.y_offs, expected_seq.y_offs);
        assert_eq!(seq.remap_x_adj, expected_seq.remap_x_adj);
        assert_eq!((width, height), (spec.w, spec.h));
    }
}
