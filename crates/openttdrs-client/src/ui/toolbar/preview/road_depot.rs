//! Fantasma de depósito de carretera: losa + carretera + capas BUILD (como el mapa).

use bevy::prelude::*;
use openttdrs_core::{
    Action2EvalCtx, NewGrfEntry, RoadTypeDef, TramwayDepotReplacement, stack_params_for_grfid,
};

use crate::iso::{iso, road_depot_build_sprite_center, tile_pos_half};
use crate::render::{CompanyColoredSprites, NewGrfRoadSpriteCache, sprite_from_company_or_asset};
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
const ROTSG_DEPOT: u8 = 8;

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
    pub custom_depot_def: Option<&'a RoadTypeDef>,
    pub newgrf_stack: &'a [NewGrfEntry],
    pub road_sprites: &'a mut NewGrfRoadSpriteCache,
    pub images: &'a mut Assets<Image>,
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
        custom_depot_def,
        newgrf_stack,
        road_sprites,
        images,
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

    for (layer_index, spec) in road_depot_build_layers(dir).iter().enumerate() {
        let custom_layer = custom_depot_def.and_then(|def| {
            custom_depot_preview_layer(
                def,
                layer_index,
                spec,
                newgrf_stack,
                road_sprites,
                images,
                tint,
            )
        });
        let (sprite, seq, width, height) = custom_layer.unwrap_or_else(|| {
            let (path, seq, width, height) = preview_layer_asset(action5_replacement, spec);
            (
                sprite_from_company_or_asset(company, asset_server, &path, tint),
                seq,
                width,
                height,
            )
        });
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
            sprite,
            Transform::from_translation(center).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}

/// Resuelve una capa `ROTSG_DEPOT` con el contexto de GUI de OpenTTD
/// (`INVALID_TILE`): parámetros del GRF sí se conservan, pero no se inventan
/// variables de una tesela que todavía no existe. La caché compartida evita
/// crear una textura nueva en cada frame mientras el cursor se mueve.
fn custom_depot_preview_layer(
    def: &RoadTypeDef,
    layer_index: usize,
    spec: &RoadDepotLayerGfx,
    newgrf_stack: &[NewGrfEntry],
    road_sprites: &mut NewGrfRoadSpriteCache,
    images: &mut Assets<Image>,
    tint: Color,
) -> Option<(Sprite, crate::iso::RoadStopSeqGfx, f32, f32)> {
    let mut action2 = Action2EvalCtx::default();
    action2.set_grf_params(stack_params_for_grfid(newgrf_stack, def.newgrf_grfid));
    let view = def.newgrf_specific_view_runtime(ROTSG_DEPOT, layer_index, &mut action2)?;
    let image = road_sprites.handle_for_resolved_specific_view(
        def,
        ROTSG_DEPOT,
        layer_index,
        None,
        &action2,
        &view,
        images,
    );
    let mut seq = road_depot_seq_gfx(spec);
    seq.x_offs = f32::from(view.x_offs);
    seq.y_offs = f32::from(view.y_offs);
    Some((
        Sprite {
            image,
            color: tint,
            ..default()
        },
        seq,
        f32::from(view.width),
        f32::from(view.height),
    ))
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

    #[test]
    fn custom_depot_layer_uses_specific_view_geometry() {
        use std::collections::HashMap;

        let view = openttdrs_core::DecodedSprite {
            width: 7,
            height: 9,
            x_offs: -3,
            y_offs: -5,
            rgba: vec![255, 0, 0, 255].repeat(7 * 9),
            mask: Vec::new(),
        };
        let graphics = openttdrs_core::TrainSpriteGraphics {
            sets: vec![vec![view.clone()]],
            assigns: vec![openttdrs_core::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            }],
            specific_assigns: HashMap::from([((0, ROTSG_DEPOT), 0)]),
            ..default()
        };
        let def = RoadTypeDef {
            id: openttdrs_core::RoadType::from_u8(2),
            class: openttdrs_core::RoadTramType::Road,
            label: "Preview depot".into(),
            short_label: "PDEP".into(),
            intro_year: 0,
            max_speed: 0,
            cost_multiplier: 0,
            maintenance_multiplier: 0,
            flags: 0,
            powered_mask: 0,
            badges: Vec::new(),
            from_tramtypes_feature: false,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(graphics)),
            newgrf_grfid: 0,
            newgrf_type_tables: None,
        };
        let spec = road_depot_build_layers(0)[0];
        let mut images = Assets::<Image>::default();
        let mut road_sprites = NewGrfRoadSpriteCache::default();

        let (_, seq, width, height) = custom_depot_preview_layer(
            &def,
            0,
            &spec,
            &[],
            &mut road_sprites,
            &mut images,
            Color::WHITE,
        )
        .expect("ROTSG_DEPOT preview view");

        assert_eq!((width, height), (7.0, 9.0));
        assert_eq!((seq.x_offs, seq.y_offs), (-3.0, -5.0));
        assert_eq!(images.len(), 1);
    }
}
