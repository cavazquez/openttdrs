use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND,
    OBJECT_TYPE_TRANSMITTER, effective_clear_ground, industry_uses_water_ground,
};

use super::{
    helpers::FLAT_WATER_LAYER_FRAC, leveled_foundation_overlay_pos, sloped_or_flat_image,
    spawn_ground_sprite, spawn_leveled_foundation,
};
use crate::iso::{overlay_pos, remap_tile_offset, tile_pos, wang_hash};
use crate::render::atlas::AtlasSprite;
use crate::render::{
    CompanyColoredSprites, MapSpriteBatches, MapVisualLayer, TileRenderContext, WaterTile,
    WorldAssets, sprite_from_atlas_or_industry_palette,
};
use crate::sprites::{
    FENCE_MOD_BY_TILEH_NE, FENCE_MOD_BY_TILEH_NW, FENCE_MOD_BY_TILEH_SE, FENCE_MOD_BY_TILEH_SW,
    FENCE_SPRITE_META, FIELD_STATES, HOUSE_DRAW_DATA, TREE_LAYOUT_SPRITE, TREE_LAYOUT_XY,
    TREE_SPRITE_META, house_building_stage_from_tile, house_draw_data_index_for_tile,
    industry_anim_layer_used_in_any_frame, industry_building_needs_client_anim,
    industry_effective_m4_for_draw, industry_gfx_entry_for_tile,
    industry_gfx_uses_fizzy_drink_anim, industry_gfx_uses_random_colour,
    industry_gfx_uses_refinery_fire_anim, industry_palette_colour_for_instance,
};

/// Sprite plano de hierba según densidad (`m5 & 0x3`) en teselas `MP_CLEAR`.
/// `m5 == 0` se trata como hierba completa (valor por defecto histórico de `new_flat`).
fn grass_flat_for_clear(assets: &WorldAssets, tile_m5: u8) -> &AtlasSprite {
    let density = if tile_m5 == 0 { 3 } else { tile_m5 & 0x03 };
    match density {
        0 => &assets.bare,
        1 => &assets.grass_one_third,
        2 => &assets.grass_two_third,
        _ => &assets.grass,
    }
}

pub(crate) fn spawn_house_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    // GetCleanHouseType: GB(m8, 0, 12) — el resto es datos NewGRF
    let clean_house_id = ctx.tile.map_or(0u16, |t| t.m8 & 0xFFF);
    let house_base = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    spawn_ground_sprite(commands, &house_base, Color::WHITE, ctx, slope_half_ground);
    let (m5, m3) = ctx.tile.map_or((0u8, 0x80u8), |t| (t.m5, t.m3));
    let building_stage = house_building_stage_from_tile(m5, m3);
    let spec_idx =
        house_draw_data_index_for_tile(clean_house_id, ctx.tx_i32(), ctx.ty_i32(), building_stage);
    let spec = &HOUSE_DRAW_DATA[spec_idx];
    use crate::sprites::{TransparencyOption, is_hidden, sprite_color};
    if is_hidden(TransparencyOption::Houses) {
        return;
    }
    let tint = sprite_color(TransparencyOption::Houses);
    if spec.s1 != 0
        && let Some(img) = assets.houses.get(&spec.s1)
    {
        let pos3 = overlay_pos(
            ctx.iso_pos,
            spec.s1_xrel,
            spec.s1_yrel,
            spec.s1_w,
            spec.s1_h,
            base_z,
            0.4,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            img.sprite_colored(tint),
            Transform::from_translation(pos3),
        ));
    }
    if spec.s2 != 0
        && let Some(img) = assets.houses.get(&spec.s2)
    {
        let anim = (1483..=1486).contains(&spec.s2)
            && assets.lighthouse_anim_frames.contains_key(&spec.s2);
        let mut sprite = if anim {
            assets.lighthouse_anim_frames[&spec.s2][0].sprite()
        } else {
            img.sprite()
        };
        sprite.color = tint;
        let pos3 = overlay_pos(
            ctx.iso_pos,
            spec.s2_xrel,
            spec.s2_yrel,
            spec.s2_w,
            spec.s2_h,
            base_z,
            0.5,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        let mut entity = commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(pos3),
        ));
        if anim {
            entity.insert(crate::render::LighthouseAnim { sprite_id: spec.s2 });
        }
    }
    // Ascensor Large Office: solo stage final (`draw_proc == 1` en `town_land.h`).
    // Stage 2 reusa el mismo s2 (1442/4569) con `draw_proc == 0` (obra sin lift).
    if spec.draw_proc == 1 {
        debug_assert!(
            crate::render::house_sprite_has_lift(spec.s2),
            "draw_proc==1 esperaba s2 con lift, got {}",
            spec.s2
        );
        let lift_w = 4.0;
        let lift_h = 13.0;
        // OpenTTD: `AddChildSpriteScreen(SPR_LIFT, …, 14, 60 - pos)` — offsets de
        // **pantalla** relativos al edificio, no unidades TILE_SEQ.
        // `remap_tile_offset(14, 60)` los trataba como tesela y desplazaba ~3 tiles.
        let pos3 = overlay_pos(
            ctx.iso_pos,
            spec.s2_xrel + crate::render::HOUSE_LIFT_SCREEN_X,
            spec.s2_yrel + crate::render::HOUSE_LIFT_SCREEN_Y,
            lift_w,
            lift_h,
            base_z,
            0.55,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        let mut sprite = assets.house_lift.sprite();
        sprite.color = tint;
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(pos3),
            crate::render::HouseLiftAnim {
                base: pos3,
                coord: ctx.coord,
            },
        ));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_industry_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    map: &Map,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    industries: &[openttdrs_core::Industry],
    company: &mut CompanyColoredSprites,
    images: &mut Assets<Image>,
    industry_catalog: &[openttdrs_core::IndustryTileSpecDef],
    industry_overrides: &[u16],
    mut industry_sprites: Option<&mut crate::render::NewGrfIndustrySpriteCache>,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    // gfx limpio + traducción NewGRF (`GetIndustryGfx`).
    let clean = ctx
        .tile
        .map_or(0u16, |t| openttdrs_core::get_clean_industry_gfx(t.m5, t.m6));
    let translated = openttdrs_core::get_translated_industry_tile_id(clean, industry_overrides);
    let m1 = ctx.tile.map_or(0, |t| t.m1);
    let m2 = ctx.tile.map_or(0, |t| t.m2);
    let m3hi = ctx.tile.map_or(0, |t| t.m3hi);
    let stage = usize::from(openttdrs_core::industry_construction_stage(m1));
    let newgrf_def = if translated >= openttdrs_core::NEW_INDUSTRY_TILE_OFFSET {
        crate::render::industry_newgrf::newgrf_industry_tile_def(industry_catalog, translated)
    } else {
        None
    };
    // Tabla vanilla: NewGRF usa subst_id si no hay sprites / como fallback.
    let gfx = if translated >= openttdrs_core::NEW_INDUSTRY_TILE_OFFSET {
        openttdrs_core::industry_tile_spec_def(industry_catalog, translated)
            .map(|d| d.subst_id)
            .unwrap_or(0)
    } else {
        translated
    };
    let palette_colour = industry_palette_colour_for_instance(m2, industries);
    let client_anim = industry_building_needs_client_anim(gfx, m1);
    let phase = crate::render::industry_anim_phase(ctx.tx_i32(), ctx.ty_i32(), m3hi);
    let m4 = industry_effective_m4_for_draw(gfx, m1, m3hi, 0.0, phase);
    let entry = industry_gfx_entry_for_tile(gfx, m1, m4);
    crate::sprites::log_industry_gfx_once(translated, m1, m3hi, entry);
    use crate::sprites::{TransparencyOption, is_hidden, with_to_alpha};
    let industries_hidden = is_hidden(TransparencyOption::Industries);
    // Chimenea de la central terminada: penacho de humo animado encima.
    if !industries_hidden && gfx == crate::render::GFX_POWERPLANT_CHIMNEY && m1 & 0x80 != 0 {
        crate::render::spawn_chimney_smoke(commands, assets, ctx);
    }
    // Chimenea mina de cobre terminada: humo `EV_COPPER_MINE_SMOKE`.
    if !industries_hidden && gfx == crate::render::GFX_COPPER_MINE_CHIMNEY && m1 & 0x80 != 0 {
        crate::render::spawn_copper_mine_smoke(commands, assets, ctx);
    }
    let ground_sid = entry.map(|e| e.ground_sprite_id).unwrap_or(0);
    let use_water = industry_uses_water_ground(map, ctx.coord, gfx, ground_sid);
    let chunk = ctx.map_tile_chunk();
    if use_water {
        commands.spawn((
            MapVisualLayer,
            chunk,
            WaterTile::ANIMATED,
            assets.water.sprite(),
            Transform::from_translation(tile_pos(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                FLAT_WATER_LAYER_FRAC,
            )),
        ));
    } else {
        // Terreno natural bajo la industria; en pendiente se añade cimiento nivelado (P4).
        let terrain_img = sloped_or_flat_image(tileh, &assets.rough, &assets.rough_slopes);
        let terrain_color = Color::srgb(0.55, 0.50, 0.45);
        spawn_ground_sprite(
            commands,
            &terrain_img,
            terrain_color,
            ctx,
            slope_half_ground,
        );
    }
    let leveled = tileh != 0;
    if leveled {
        spawn_leveled_foundation(
            commands,
            assets,
            ctx,
            tileh,
            foundation_newgrf,
            action5_sprites,
            Some(&mut *images),
        );
    }
    let overlay_z = if leveled {
        base_z.saturating_add(crate::sprites::leveled_foundation_z_delta(tileh))
    } else {
        base_z
    };
    let overlay_at = |xrel, yrel, w, h, layer| {
        if leveled {
            leveled_foundation_overlay_pos(
                ctx.iso_pos,
                xrel,
                yrel,
                w,
                h,
                base_z,
                layer,
                ctx.tx_i32(),
                ctx.ty_i32(),
            )
        } else {
            overlay_pos(
                ctx.iso_pos,
                xrel,
                yrel,
                w,
                h,
                overlay_z,
                layer,
                ctx.tx_i32(),
                ctx.ty_i32(),
            )
        }
    };
    let overlay_ctx =
        crate::render::IndustryOverlayContext::from_tile_ctx(ctx, base_z, overlay_z, leveled);
    if industries_hidden {
        return;
    }
    if let Some(def) = newgrf_def
        && let Some(cache) = industry_sprites.as_mut()
    {
        let colour = Some(palette_colour);
        let mut a2 = openttdrs_core::Action2EvalCtx {
            random_bits: u32::from(m3hi),
            ..Default::default()
        };
        a2.vars.insert(0x5F, u32::from(m3hi) << 8);
        a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
            newgrf_stack,
            def.newgrf_grfid,
        ));
        if let Some(handle) = cache.handle_for_runtime(def, stage, colour, &mut a2, images) {
            let view = def.newgrf_view(stage).or(def.newgrf_preview.as_ref());
            if let Some(view) = view {
                let pos3 = overlay_at(
                    f32::from(view.x_offs),
                    f32::from(view.y_offs),
                    f32::from(view.width),
                    f32::from(view.height),
                    0.5,
                );
                let mut sprite = Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                };
                sprite.color = with_to_alpha(sprite.color, TransparencyOption::Industries);
                commands.spawn((
                    MapVisualLayer,
                    chunk,
                    sprite,
                    Transform::from_translation(pos3),
                ));
                return;
            }
        }
    }
    if let Some(s) = entry {
        let anim_base = |ground: bool| {
            crate::render::IndustryBuildingAnim::new(gfx, m1, phase, ground, overlay_ctx)
        };
        if s.ground_sprite_id != 0 && s.ground_w > 0.0 && s.ground_h > 0.0 {
            if client_anim && industry_anim_layer_used_in_any_frame(gfx, true) {
                crate::render::spawn_industry_anim_layer(
                    commands,
                    assets,
                    chunk,
                    anim_base(true),
                    s.ground_sprite_id,
                    s.ground_xrel,
                    s.ground_yrel,
                    s.ground_w,
                    s.ground_h,
                    0.45,
                );
            } else if let Some(img) = assets.industries.get(&s.ground_sprite_id) {
                // Acería: metal fundido está en la capa ground (ciclo `oil_refinery`).
                let ground_fire = industry_gfx_uses_refinery_fire_anim(gfx, m1)
                    && assets
                        .refinery_fire_frames
                        .contains_key(&s.ground_sprite_id);
                let mut sprite = if ground_fire {
                    assets.refinery_fire_frames[&s.ground_sprite_id][0].sprite()
                } else if industry_gfx_uses_random_colour(gfx) {
                    sprite_from_atlas_or_industry_palette(
                        company,
                        images,
                        img,
                        s.ground_sprite_id,
                        palette_colour,
                    )
                } else {
                    img.sprite()
                };
                sprite.color = with_to_alpha(sprite.color, TransparencyOption::Industries);
                let pos_g = overlay_at(s.ground_xrel, s.ground_yrel, s.ground_w, s.ground_h, 0.45);
                let mut entity = commands.spawn((
                    MapVisualLayer,
                    chunk,
                    sprite,
                    Transform::from_translation(pos_g),
                ));
                if ground_fire {
                    entity.insert(crate::render::RefineryFireAnim {
                        sprite_id: s.ground_sprite_id,
                    });
                }
            }
        }
        if s.sprite_id != 0 && s.w > 0.0 && s.h > 0.0 {
            if client_anim && industry_anim_layer_used_in_any_frame(gfx, false) {
                crate::render::spawn_industry_anim_layer(
                    commands,
                    assets,
                    chunk,
                    anim_base(false),
                    s.sprite_id,
                    s.xrel,
                    s.yrel,
                    s.w,
                    s.h,
                    0.5,
                );
            } else if let Some(img) = assets.industries.get(&s.sprite_id) {
                let refinery_fire = industry_gfx_uses_refinery_fire_anim(gfx, m1)
                    && assets.refinery_fire_frames.contains_key(&s.sprite_id);
                let fizzy_drink = industry_gfx_uses_fizzy_drink_anim(gfx, m1)
                    && assets.fizzy_drink_frames.contains_key(&s.sprite_id);
                // Fuego: nunca recolorear con paleta de compañía el PNG base
                // (congela la llama); usar frames `oil_refinery` o el atlas.
                let mut sprite = if refinery_fire {
                    assets.refinery_fire_frames[&s.sprite_id][0].sprite()
                } else if fizzy_drink {
                    assets.fizzy_drink_frames[&s.sprite_id][0].sprite()
                } else if industry_gfx_uses_random_colour(gfx)
                    && !industry_gfx_uses_refinery_fire_anim(gfx, m1)
                {
                    sprite_from_atlas_or_industry_palette(
                        company,
                        images,
                        img,
                        s.sprite_id,
                        palette_colour,
                    )
                } else {
                    img.sprite()
                };
                sprite.color = with_to_alpha(sprite.color, TransparencyOption::Industries);
                let pos3 = overlay_at(s.xrel, s.yrel, s.w, s.h, 0.5);
                let mut entity = commands.spawn((
                    MapVisualLayer,
                    chunk,
                    sprite,
                    Transform::from_translation(pos3),
                ));
                if refinery_fire {
                    entity.insert(crate::render::RefineryFireAnim {
                        sprite_id: s.sprite_id,
                    });
                } else if fizzy_drink {
                    entity.insert(crate::render::FizzyDrinkAnim {
                        sprite_id: s.sprite_id,
                    });
                }
            }
        }
    }
    crate::render::spawn_industry_draw_proc_overlays(
        commands,
        assets,
        ctx,
        gfx,
        m1,
        m3hi,
        overlay_ctx,
        chunk,
    );
}

pub(crate) fn spawn_generic_land_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    climate: Climate,
    world_seed: u64,
) {
    let tileh = ctx.info.tileh;
    let ottd_type = ctx.tile.map_or(0u8, |t| (t.mapt >> 4) & 0xF);
    let tile_m5 = ctx.tile.map_or(0u8, |t| t.m5);

    // MP_CLEAR (0): distinguir subtipo de suelo via m5 bits 2-4
    // MP_OBJECT (10): grass de base + overlay de objeto
    let grass_img = || {
        sloped_or_flat_image(
            tileh,
            grass_flat_for_clear(assets, tile_m5),
            &assets.grass_slopes,
        )
    };
    let full_grass_img = || sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    let rough_img = || sloped_or_flat_image(tileh, &assets.rough, &assets.rough_slopes);
    let rocky_variant = wang_hash(ctx.tx, ctx.ty, world_seed.wrapping_add(0xB0C0_5EED) as u32)
        as usize
        % assets.rocky.len();
    let rocky_img =
        || sloped_or_flat_image(tileh, &assets.rocky[rocky_variant], &assets.rough_slopes);
    let snow_img = || {
        if tileh == 0 {
            assets.snow.clone()
        } else {
            sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes)
        }
    };
    let snow_color = Color::srgb(0.94, 0.97, 1.0);
    let desert_color = Color::srgb(0.92, 0.82, 0.62);

    let clear_ground =
        effective_clear_ground(climate, tile_m5, ctx.tx_i32(), ctx.ty_i32(), world_seed);

    let (image, color) = match ctx.kind {
        TileKind::Grass if ottd_type == 0 => match clear_ground {
            CLEAR_GROUND_GRASS => (grass_img(), Color::WHITE),
            CLEAR_GROUND_SNOW => (snow_img(), snow_color),
            CLEAR_GROUND_DESERT => (rough_img(), desert_color),
            3 => {
                // `DrawTile_Clear` Fields: estado de cultivo en bits 0–3 de
                // m3 + offset de pendiente; cercas como overlay.
                let state = usize::from(ctx.tile.map_or(0, |t| t.m3 & 0x0F)).min(FIELD_STATES - 1);
                let img = assets.fields[state * 15 + usize::from(tileh.min(14))].clone();
                spawn_field_fences(commands, assets, ctx);
                (img, Color::WHITE)
            }
            CLEAR_GROUND_ROUGH => (rough_img(), Color::srgb(0.78, 0.73, 0.58)),
            CLEAR_GROUND_ROCKY => (rocky_img(), Color::WHITE),
            _ => (rough_img(), Color::srgb(0.78, 0.73, 0.58)),
        },
        TileKind::Grass => match clear_ground {
            CLEAR_GROUND_SNOW => (snow_img(), snow_color),
            CLEAR_GROUND_DESERT => (rough_img(), desert_color),
            _ => (full_grass_img(), Color::WHITE),
        },
        TileKind::Forest => match clear_ground {
            CLEAR_GROUND_SNOW => (snow_img(), snow_color),
            CLEAR_GROUND_DESERT => (rough_img(), desert_color),
            _ => (full_grass_img(), Color::WHITE),
        },
        TileKind::CoalField => (rough_img(), Color::srgb(0.55, 0.50, 0.45)),
        TileKind::Unknown(_) => (grass_img(), Color::srgb(1.0, 0.0, 1.0)),
        TileKind::House
        | TileKind::Station
        | TileKind::Road
        | TileKind::Rail
        | TileKind::RoadDepot
        | TileKind::RailDepot
        | TileKind::RoadTunnel
        | TileKind::RailTunnel
        | TileKind::RoadBridge
        | TileKind::RailBridge
        | TileKind::ShipDepot
        | TileKind::Airport
        | TileKind::Industry
        | TileKind::Water
        | TileKind::Void => unreachable!(),
    };
    spawn_ground_sprite(commands, &image, color, ctx, slope_half_ground);

    // MP_OBJECT: renderizar faro o transmisor como overlay.
    // ObjectType de OpenTTD: 0=Transmisor, 1=Faro.
    if ottd_type == 10 {
        use crate::sprites::{TransparencyOption, is_hidden, sprite_color};
        if is_hidden(TransparencyOption::Structures) {
            return;
        }
        let tint = sprite_color(TransparencyOption::Structures);
        // Offsets NFO OpenGFX2 32ez (`ogfx21_base_32ez.nfo` sprites 2601/2602).
        let (obj_img, obj_xrel, obj_yrel, obj_w, obj_h) = match tile_m5 {
            OBJECT_TYPE_TRANSMITTER => (Some(assets.transmitter.clone()), -26.0, -80.0, 54.0, 94.0),
            OBJECT_TYPE_LIGHTHOUSE => (Some(assets.lighthouse.clone()), -9.0, -52.0, 21.0, 64.0),
            OBJECT_TYPE_OWNED_LAND => (Some(assets.bought_land.clone()), -16.0, -40.0, 32.0, 48.0),
            _ => (None, 0.0, 0.0, 0.0, 0.0),
        };
        if let Some(img) = obj_img {
            let anim = tile_m5 == OBJECT_TYPE_LIGHTHOUSE
                && assets.lighthouse_anim_frames.contains_key(&2602);
            let mut sprite = if anim {
                assets.lighthouse_anim_frames[&2602][0].sprite()
            } else {
                img.sprite()
            };
            // Owned land no es "structure" de faro/antena; no tintar si es bought land.
            if tile_m5 != OBJECT_TYPE_OWNED_LAND {
                sprite.color = tint;
            }
            let pos3 = overlay_pos(
                ctx.iso_pos,
                obj_xrel,
                obj_yrel,
                obj_w,
                obj_h,
                ctx.info.base_z,
                0.6,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            let mut entity = commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(pos3),
            ));
            if anim {
                entity.insert(crate::render::LighthouseAnim { sprite_id: 2602 });
            }
        }
    }
}

/// Cercas de campos de cultivo, fiel a `DrawClearLandFence` (`clear_cmd.cpp`):
/// tipo por lado en m4 (SE bits 2–4, SW 5–7), m3 (NE bits 5–7) y m6 (NW bits
/// 2–4); variante de sprite por pendiente (`_fence_mod_by_tileh_*`).
fn spawn_field_fences(commands: &mut Commands, assets: &WorldAssets, ctx: &TileRenderContext) {
    let Some(t) = ctx.tile else {
        return;
    };
    let tileh = usize::from(ctx.info.tileh & 0x1F);
    // (tipo, tabla de variantes, dx, dy, bit de esquina para z, capa)
    // NW usa CORNER_W (bit 1), NE CORNER_E (bit 4), SW/SE CORNER_S (bit 2).
    let sides: [(u8, &[u8; 32], f32, f32, u8, f32); 4] = [
        (
            (t.m6 >> 2) & 0x7,
            &FENCE_MOD_BY_TILEH_NW,
            0.0,
            -16.0,
            0x1,
            0.06,
        ),
        (
            (t.m3 >> 5) & 0x7,
            &FENCE_MOD_BY_TILEH_NE,
            -16.0,
            0.0,
            0x4,
            0.06,
        ),
        (
            (t.m3hi >> 5) & 0x7,
            &FENCE_MOD_BY_TILEH_SW,
            0.0,
            0.0,
            0x2,
            0.26,
        ),
        (
            (t.m3hi >> 2) & 0x7,
            &FENCE_MOD_BY_TILEH_SE,
            0.0,
            0.0,
            0x2,
            0.26,
        ),
    ];
    for (fence, mods, dx, dy, corner_bit, layer) in sides {
        if fence == 0 {
            continue;
        }
        let ftype = usize::from(fence - 1).min(5);
        let variant = usize::from(mods[tileh]).min(5);
        let meta = FENCE_SPRITE_META[ftype][variant];
        // `GetSlopePixelZInCorner`: esquina elevada = TILE_HEIGHT (8 unidades).
        let dz = if ctx.info.tileh & corner_bit != 0 {
            8.0
        } else {
            0.0
        };
        let off = remap_tile_offset(dx, dy, dz) * 0.5;
        let pos3 = overlay_pos(
            Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
            meta.xrel,
            meta.yrel,
            meta.w,
            meta.h,
            ctx.info.base_z,
            layer,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.fences[ftype * 6 + variant].sprite(),
            Transform::from_translation(pos3),
        ));
    }
}

/// Árboles de una tesela `MP_TREES`, fiel a `DrawTile_Trees` (`tree_cmd.cpp`):
/// 1–4 árboles según bits 6–7 de m5, posiciones de `_tree_layout_xy`, especie
/// por árbol de `_tree_layout_sprite[tipo×4 + variante]` y etapa de
/// crecimiento (bits 0–2 de m5) solo en el último árbol (el resto adulto +3).
pub(crate) fn push_forest_tree(
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    batches: &mut MapSpriteBatches,
    map_w: u32,
) {
    use crate::sprites::{TransparencyOption, is_hidden, sprite_color};
    if is_hidden(TransparencyOption::Trees) {
        return;
    }
    let tint = sprite_color(TransparencyOption::Trees);
    let (tree_type, count, growth) = match ctx.tile {
        // MP_TREES real (nibble alto de mapt = 4): datos del save.
        Some(t) if (t.mapt >> 4) & 0xF == 4 => (
            u32::from(t.m3) % 12,
            usize::from((t.m5 >> 6) & 0x3) + 1,
            usize::from(t.m5 & 0x7).min(6),
        ),
        // Mapas generados sin datos: variedad determinista equivalente.
        _ => {
            let h = wang_hash(ctx.tx, ctx.ty, 0xCAFE);
            (h % 12, (h >> 8) as usize % 2 + 1, 3)
        }
    };

    // `tmp = CountBits(tile + x + y)` con x/y en unidades de mundo (×16).
    let tile_index = ctx.ty * map_w + ctx.tx;
    let tmp = (tile_index + ctx.tx * 16 + ctx.ty * 16).count_ones();
    let variant = (tmp & 0x3) as usize;
    let layout = ((tmp >> 2) & 0x3) as usize;

    let row = &TREE_LAYOUT_SPRITE[tree_type as usize * 4 + variant];
    for i in 0..count {
        let stage = if i == count - 1 { growth } else { 3 };
        let sprite_idx = row[i] as usize + stage;
        let meta = &TREE_SPRITE_META[sprite_idx];
        let (dx, dy) = TREE_LAYOUT_XY[layout][i];
        // Offset sub-tesela en pantalla (misma escala que iso(): remap × 0.5).
        let off = remap_tile_offset(f32::from(dx), f32::from(dy), 0.0) * 0.5;
        let pos3 = overlay_pos(
            Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
            meta.xrel,
            meta.yrel,
            meta.w,
            meta.h,
            ctx.info.base_z,
            // Orden dentro de la tesela: los árboles más al sur tapan.
            0.3 + (f32::from(dx) + f32::from(dy)) * 1e-4,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        batches.trees.push((
            ctx.map_tile_chunk(),
            assets.trees[sprite_idx].sprite_colored(tint),
            Transform::from_translation(pos3),
        ));
    }
}
