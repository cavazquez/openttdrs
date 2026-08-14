use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND,
    OBJECT_TYPE_STATUE_COMPANY, OBJECT_TYPE_TRANSMITTER, ObjectSpecDef, effective_clear_ground,
    industry_uses_water_ground, is_newgrf_object_type_id,
};

use super::{
    helpers::FLAT_WATER_LAYER_FRAC, leveled_foundation_overlay_pos, sloped_or_flat_image,
    spawn_ground_sprite, spawn_leveled_foundation,
};
use crate::iso::{overlay_pos, remap_tile_offset, slope_sprite_offset, tile_pos, wang_hash};
use crate::render::atlas::AtlasSprite;
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    CompanyColoredSprites, MapSpriteBatches, MapVisualLayer, TileRenderContext, WaterTile,
    WorldAssets, sprite_from_atlas_or_company_colour, sprite_from_atlas_or_industry_palette,
};
use crate::sprites::{
    CompanyColour, FENCE_MOD_BY_TILEH_NE, FENCE_MOD_BY_TILEH_NW, FENCE_MOD_BY_TILEH_SE,
    FENCE_MOD_BY_TILEH_SW, FENCE_SPRITE_META, FIELD_STATES, HOUSE_DRAW_DATA, TILEH_TO_SHORE_SPRITE,
    TREE_LAYOUT_SPRITE, TREE_LAYOUT_XY, TREE_SPRITE_META, house_building_stage_from_tile,
    industry_anim_layer_used_in_any_frame, industry_building_needs_client_anim,
    industry_effective_m4_for_draw, industry_gfx_entry_for_tile,
    industry_gfx_uses_fizzy_drink_anim, industry_gfx_uses_random_colour,
    industry_gfx_uses_refinery_fire_anim, industry_palette_colour_for_instance,
};

/// `GetTreeGround`: bits 6–8 de MAP2 (MAP2 es una palabra, no sólo `m2`).
///
/// Los bosques no heredan el suelo de `MP_CLEAR`: pueden conservar costa,
/// terreno áspero o nieve aunque la tesela vecina sea césped. Reducir esto a
/// `TileKind::Forest` era la causa de costas reemplazadas por hierba debajo de
/// los árboles cargados desde `.sav`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TreeGround {
    Grass,
    Rough,
    SnowDesert,
    Shore,
    RoughSnow,
    /// Valor reservado/corrupto: se dibuja un fallback explícito en la traza.
    Other(u8),
}

const fn tree_map2(tile: Tile) -> u16 {
    tile.m2 as u16 | ((tile.m2_hi as u16) << 8)
}

const fn tree_ground_from_tile(tile: Tile) -> TreeGround {
    let ground = ((tree_map2(tile) >> 6) & 0x07) as u8;
    match ground {
        0 => TreeGround::Grass,
        1 => TreeGround::Rough,
        2 => TreeGround::SnowDesert,
        3 => TreeGround::Shore,
        4 => TreeGround::RoughSnow,
        _ => TreeGround::Other(ground),
    }
}

/// `GetTreeDensity`: bits 4–5 de MAP2. En `TreeGround::Grass` es la
/// densidad de hierba; para nieve/desierto selecciona el grado de cobertura.
const fn tree_density_from_tile(tile: Tile) -> usize {
    ((tree_map2(tile) >> 4) & 0x03) as usize
}

/// `TileHash(x, y)` de OpenTTD aplicado a coordenadas de tesela. OpenTTD
/// recibe coordenadas de mundo (`16 * tx`, `16 * ty`), por lo que los shifts
/// 4 y 6 quedan como `tx`/`ty` y `tx >> 2`/`ty >> 2` respectivamente.
const fn tree_rough_flat_variant(tx: u32, ty: u32) -> usize {
    const ROUGH_BY_HASH: [usize; 8] = [0, 1, 2, 3, 4, 0, 1, 2];
    let hash = (tx ^ (tx >> 2) ^ ty).wrapping_sub(ty >> 2);
    ROUGH_BY_HASH[(hash & 0x07) as usize]
}

const TREE_SNOW_DESERT_BASE: [u32; 4] = [4493, 4512, 4531, 4550];

/// Sprite que `DrawTile_Trees` entrega a `DrawGroundSprite` antes de
/// componer los árboles. Mantenerlo en una función permite probar todos los
/// `TreeGround` sin depender del atlas ni de una ventana Bevy.
const fn tree_ground_sprite_id(
    ground: TreeGround,
    density: usize,
    tileh: u8,
    tx: u32,
    ty: u32,
) -> u32 {
    let slope = slope_sprite_offset(tileh) as u32;
    let density = if density > 3 { 3 } else { density };
    match ground {
        // DrawClearLandTile(ti, GetTreeDensity(tile)).
        TreeGround::Grass => 3924 + slope + density as u32 * 19,
        // DrawHillyLandTile(ti): las pendientes no se aleatorizan.
        TreeGround::Rough if slope != 0 => 4000 + slope,
        TreeGround::Rough => {
            const ROUGH_IDS: [u32; 5] = [4000, 4019, 4020, 4021, 4022];
            ROUGH_IDS[tree_rough_flat_variant(tx, ty)]
        }
        // DrawTile_Trees usa la misma tabla para SnowDesert y RoughSnow.
        TreeGround::SnowDesert | TreeGround::RoughSnow => TREE_SNOW_DESERT_BASE[density] + slope,
        TreeGround::Shore => tree_shore_sprite_id(tileh),
        // Conserva una salida visible y trazable para MAP2 inválido.
        TreeGround::Other(_) => 3981 + slope,
    }
}

fn grass_density_image(assets: &WorldAssets, density: usize, tileh: u8) -> AtlasSprite {
    assets.grass_density[density.min(3)][usize::from(slope_sprite_offset(tileh))].clone()
}

fn rough_tree_image(assets: &WorldAssets, tileh: u8, tx: u32, ty: u32) -> AtlasSprite {
    sloped_or_flat_image(
        tileh,
        &assets.rough_flat[tree_rough_flat_variant(tx, ty)],
        &assets.rough_slopes,
    )
}

fn snow_desert_image(assets: &WorldAssets, density: usize, tileh: u8) -> AtlasSprite {
    assets.snow_desert[density.min(3)][usize::from(slope_sprite_offset(tileh))].clone()
}

fn clear_grass_density(tile_m5: u8) -> usize {
    // Los mapas procedurales históricos usan `m5 == 0` como su valor de
    // inicio; conservar su césped pleno. Los MP_TREES usan
    // `tree_density_from_tile` y nunca pasan por esta compatibilidad.
    if tile_m5 == 0 {
        3
    } else {
        usize::from(tile_m5 & 0x03)
    }
}

fn record_tree_ground(sprite_id: u32, fallback: bool) {
    WorldDrawTrace::record_sprite(
        if fallback {
            "tree-ground-fallback"
        } else {
            "tree-ground"
        },
        "ground",
        sprite_id,
        fallback,
    );
}

const fn tree_shore_sprite_id(tileh: u8) -> u32 {
    5936 + TILEH_TO_SHORE_SPRITE[tileh as usize] as u32
}

/// Primer sprite de los nueve estados de cultivo de OpenTTD.
///
/// `table/sprites.h`: `SPR_FARMLAND_BARE = 4126`; los estados siguientes
/// ocupan bloques consecutivos de 19 pendientes. Centralizar el cálculo
/// impide que la selección del atlas y la traza de paridad diverjan.
const SPR_FARMLAND_BARE: u32 = 4126;

/// Primer sprite de los seis tipos de cerca de un campo.
///
/// `table/sprites.h`: `SPR_HEDGE_BUSHES = 4090`; cada tipo tiene seis
/// variantes de pendiente.
const SPR_HEDGE_BUSHES: u32 = 4090;

const FIELD_SLOPE_SPRITE_COUNT: usize = 19;

/// ID que `DrawTile_Clear` entrega a `DrawGroundSprite` para un campo.
const fn field_ground_sprite_id(state: usize, tileh: u8) -> u32 {
    let state = if state >= FIELD_STATES {
        FIELD_STATES - 1
    } else {
        state
    };
    SPR_FARMLAND_BARE
        + (state * FIELD_SLOPE_SPRITE_COUNT + slope_sprite_offset(tileh) as usize) as u32
}

/// `GetSlopeMaxPixelZ(tileh)` de OpenTTD para una pendiente natural.
///
/// Las pendientes normales alcanzan una altura de tesela (8 px); las cuatro
/// pendientes empinadas (`bit STEEP`) alcanzan dos (16 px).
const fn field_slope_max_pixel_z(tileh: u8) -> i32 {
    let tileh = tileh & 0x1F;
    if tileh & 0x10 != 0 {
        16
    } else if tileh & 0x0F != 0 {
        8
    } else {
        0
    }
}

/// `GetSlopePixelZInCorner(tileh, corner)` de `landscape.cpp`.
///
/// En una pendiente empinada la esquina alta queda a dos niveles. El antiguo
/// renderer sólo comprobaba el bit de la esquina y la dejaba un nivel (8 px)
/// por debajo de OpenTTD; eso desanclaba cercas en las parcelas inclinadas de
/// Kale.
const fn field_slope_pixel_z_in_corner(tileh: u8, corner_bit: u8) -> i32 {
    let tileh = tileh & 0x1F;
    let raised = if tileh & corner_bit != 0 { 8 } else { 0 };
    let steep_for_corner = match corner_bit {
        // SLOPE_STEEP_W/S/E/N en `slope_type.h`.
        0x1 => 27,
        0x2 => 23,
        0x4 => 30,
        0x8 => 29,
        _ => 0,
    };
    raised + if tileh == steep_for_corner { 8 } else { 0 }
}

/// Una cerca ya resuelta desde los bytes de la tesela, antes de crear el
/// sprite de Bevy. Esta forma intermedia se usa por el render y por la traza
/// para que ambos mantengan el contrato de `DrawClearLandFence`.
#[derive(Clone, Copy, Debug)]
struct FieldFenceDraw {
    fence_type: usize,
    variant: usize,
    sprite_id: u32,
    /// Offset y altura en unidades de mundo de OpenTTD (no píxeles Bevy).
    offset: (i32, i32, i32),
    layer: f32,
}

/// Resuelve NW, NE, SW y SE con el mismo orden que `DrawClearLandFence`.
///
/// `Tile::m3hi` es el byte serializado como `m4` de OpenTTD; por eso SW/SE
/// salen de este campo y no de `m3`.
fn field_fence_draws(t: &Tile, tileh: u8) -> [Option<FieldFenceDraw>; 4] {
    let tileh = usize::from(tileh & 0x1F);
    // (tipo codificado, tabla de variantes, dx, dy, esquina de referencia,
    // capa visual local). NW usa W, NE E, SW/SE S como el C++.
    let sides: [(u8, &[u8; 32], i32, i32, u8, f32); 4] = [
        ((t.m6 >> 2) & 0x7, &FENCE_MOD_BY_TILEH_NW, 0, -16, 0x1, 0.06),
        ((t.m3 >> 5) & 0x7, &FENCE_MOD_BY_TILEH_NE, -16, 0, 0x4, 0.06),
        ((t.m3hi >> 5) & 0x7, &FENCE_MOD_BY_TILEH_SW, 0, 0, 0x2, 0.26),
        ((t.m3hi >> 2) & 0x7, &FENCE_MOD_BY_TILEH_SE, 0, 0, 0x2, 0.26),
    ];

    let mut draws = [None; 4];
    for (index, (fence, mods, dx, dy, corner_bit, layer)) in sides.into_iter().enumerate() {
        if fence == 0 {
            continue;
        }
        let fence_type = usize::from(fence - 1).min(5);
        let variant = usize::from(mods[tileh]).min(5);
        draws[index] = Some(FieldFenceDraw {
            fence_type,
            variant,
            sprite_id: SPR_HEDGE_BUSHES + (fence_type * 6 + variant) as u32,
            offset: (
                dx,
                dy,
                field_slope_pixel_z_in_corner(tileh as u8, corner_bit),
            ),
            layer,
        });
    }
    draws
}

pub(crate) fn spawn_house_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    _slope_half_ground: f32,
    house_catalog: &[openttdrs_core::HouseSpecDef],
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    // GetCleanHouseType: GB(m8, 0, 12) — el resto es datos NewGRF
    let clean_house_id = ctx.tile.map_or(0u16, |t| t.m8 & 0xFFF);
    let (m5, m3) = ctx.tile.map_or((0u8, 0x80u8), |t| (t.m5, t.m3));
    let building_stage = house_building_stage_from_tile(m5, m3);
    let spec_idx = crate::sprites::house_draw_data_index_for_tile_with_catalog(
        clean_house_id,
        ctx.tx_i32(),
        ctx.ty_i32(),
        building_stage,
        house_catalog,
    );
    let spec = &HOUSE_DRAW_DATA[spec_idx];

    // `DrawTile_Town`: si la tesela no es plana, `DrawFoundation(Leveled)`
    // muta la superficie antes de dibujar *ambas* capas de la casa. El suelo
    // `s1` no es el césped natural que había debajo: es exactamente el
    // `ground.sprite` de `town_land.h`.
    let leveled = tileh != 0;
    if leveled {
        spawn_leveled_foundation(commands, assets, ctx, tileh, &[], None, None);
    }
    let house_pos = |xrel: f32, yrel: f32, w: f32, h: f32, layer: f32| {
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
                base_z,
                layer,
                ctx.tx_i32(),
                ctx.ty_i32(),
            )
        }
    };
    if spec.s1 != 0 {
        let ground = assets.houses.get(&spec.s1);
        let palette_image = (spec.s1_palette != 0)
            .then(|| assets.house_palettes.handle(spec.s1, spec.s1_palette))
            .flatten();
        let fallback = ground.is_none() || (spec.s1_palette != 0 && palette_image.is_none());
        let ground_sprite = palette_image.map_or_else(
            || ground.unwrap_or(&assets.grass_density[0][0]).sprite(),
            |image| Sprite {
                image: image.clone(),
                ..default()
            },
        );
        if leveled {
            // `DrawGroundSprite` queda colgado de la fundación mediante
            // `OffsetGroundSprite(0, -TILE_HEIGHT)`. La posición Bevy
            // equivalente usa la superficie plana efectiva, pero la traza
            // conserva la semántica child del oráculo.
            WorldDrawTrace::record_foundation_child_sprite_with_palette(
                "house-foundation-ground",
                spec.s1,
                spec.s1_palette,
                fallback,
                (0, -32, 0),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                ground_sprite,
                Transform::from_translation(house_pos(
                    spec.s1_xrel,
                    spec.s1_yrel,
                    spec.s1_w,
                    spec.s1_h,
                    0.4,
                )),
            ));
        } else {
            WorldDrawTrace::record_sprite_with_palette(
                "house-ground",
                "ground",
                spec.s1,
                spec.s1_palette,
                fallback,
            );
            // Los sprites de ground de `town_land.h` no siempre miden 64×31
            // (los patios de oficinas llegan a 64×37): se anclan con sus
            // propios offsets NFO, no con el centro del rombo natural.
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                ground_sprite,
                Transform::from_translation(house_pos(
                    spec.s1_xrel,
                    spec.s1_yrel,
                    spec.s1_w,
                    spec.s1_h,
                    0.4,
                )),
            ));
        }
    }

    // La invisibilidad de casas sólo afecta la parte superior. OpenTTD ya
    // dibujó `s1` y la fundación al llegar a este punto.
    use crate::sprites::{TransparencyOption, is_hidden, sprite_color};
    if is_hidden(TransparencyOption::Houses) {
        return;
    }
    let tint = sprite_color(TransparencyOption::Houses);
    if spec.s2 != 0 {
        let Some(img) = assets.houses.get(&spec.s2) else {
            WorldDrawTrace::record_sprite_with_palette(
                "house-building",
                "sortable",
                spec.s2,
                spec.s2_palette,
                true,
            );
            return;
        };
        let palette_image = (spec.s2_palette != 0)
            .then(|| assets.house_palettes.handle(spec.s2, spec.s2_palette))
            .flatten();
        let fallback = spec.s2_palette != 0 && palette_image.is_none();
        WorldDrawTrace::record_sprite_with_palette(
            "house-building",
            "sortable",
            spec.s2,
            spec.s2_palette,
            fallback,
        );
        let anim = spec.s2_palette == 0
            && (1483..=1486).contains(&spec.s2)
            && assets.lighthouse_anim_frames.contains_key(&spec.s2);
        let mut sprite = if let Some(image) = palette_image {
            Sprite {
                image: image.clone(),
                ..default()
            }
        } else if anim {
            assets.lighthouse_anim_frames[&spec.s2][0].sprite()
        } else {
            img.sprite()
        };
        sprite.color = tint;
        let pos3 = house_pos(spec.s2_xrel, spec.s2_yrel, spec.s2_w, spec.s2_h, 0.5);
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
        let pos3 = house_pos(
            spec.s2_xrel + crate::render::HOUSE_LIFT_SCREEN_X,
            spec.s2_yrel + crate::render::HOUSE_LIFT_SCREEN_Y,
            lift_w,
            lift_h,
            0.55,
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
        // La tabla copia tanto las capas como la caja de `M()` de
        // `industry_land.h`. Registrar el contrato antes de convertirlo a
        // sprites Bevy permite confrontar las industrias vanilla tile por
        // tile contra `DrawTile_Industry` de OpenTTD. El agua sigue siendo
        // `SPR_FLAT_WATER_TILE` (4061) aunque se dibuje desde WaterTile.
        if s.ground_sprite_id != 0 {
            WorldDrawTrace::record_sprite_with_geometry(
                "industry-ground",
                "ground",
                s.ground_sprite_id,
                !use_water && !assets.industries.contains_key(&s.ground_sprite_id),
                (0, 0, 0),
                0,
                None,
            );
        }
        if s.sprite_id != 0 {
            WorldDrawTrace::record_sprite_with_geometry(
                "industry-building",
                "sortable",
                s.sprite_id,
                !assets.industries.contains_key(&s.sprite_id),
                (0, 0, 0),
                0,
                Some(TraceSpriteBounds::new(
                    s.sort_ox, s.sort_oy, s.sort_oz, s.sort_ex, s.sort_ey, s.sort_ez,
                )),
            );
        }
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_generic_land_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    climate: Climate,
    world_seed: u64,
    object_catalog: &[ObjectSpecDef],
    mut object_sprites: Option<&mut crate::render::NewGrfObjectSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let tileh = ctx.info.tileh;
    let ottd_type = ctx.tile.map_or(0u8, |t| (t.mapt >> 4) & 0xF);
    let tile_m5 = ctx.tile.map_or(0u8, |t| t.m5);
    let object_type = ctx.object_type.unwrap_or(u16::from(tile_m5));

    // MP_CLEAR (0): distinguir subtipo de suelo vía m5 bits 2-4.
    // MP_OBJECT (10): el suelo depende del ObjectType resuelto desde OBJS;
    // m5 sólo es el byte alto crudo de ObjectID en una partida importada.
    let grass_img = || grass_density_image(assets, clear_grass_density(tile_m5), tileh);
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
                WorldDrawTrace::record_sprite_with_geometry(
                    "field-ground",
                    "ground",
                    field_ground_sprite_id(state, tileh),
                    false,
                    (0, 0, 0),
                    0,
                    None,
                );
                let img =
                    assets.fields[state * 19 + usize::from(slope_sprite_offset(tileh))].clone();
                spawn_field_fences(commands, assets, ctx);
                (img, Color::WHITE)
            }
            CLEAR_GROUND_ROUGH => (rough_img(), Color::srgb(0.78, 0.73, 0.58)),
            CLEAR_GROUND_ROCKY => (rocky_img(), Color::WHITE),
            _ => (rough_img(), Color::srgb(0.78, 0.73, 0.58)),
        },
        // `table/object_land.h`: transmisor/faro usan 2/3 de césped, la
        // estatua concreto y el terreno comprado suelo desnudo. Antes esta
        // rama caía en césped pleno y escondía el error de interpretar MAP5
        // como ObjectType.
        TileKind::Grass if ottd_type == 10 => match object_type {
            t if t == u16::from(OBJECT_TYPE_TRANSMITTER)
                || t == u16::from(OBJECT_TYPE_LIGHTHOUSE) =>
            {
                (grass_density_image(assets, 2, tileh), Color::WHITE)
            }
            t if t == u16::from(OBJECT_TYPE_STATUE_COMPANY) => {
                (assets.object_concrete.clone(), Color::WHITE)
            }
            t if t == u16::from(OBJECT_TYPE_OWNED_LAND) => {
                (grass_density_image(assets, 0, tileh), Color::WHITE)
            }
            // `DrawNewObjectTile` decide su propio ground Action3. Mientras
            // no exista una vista NewGRF decodificable, mantener una base
            // explícita y no usar el ObjectID almacenado en MAP5 como tipo.
            _ => (full_grass_img(), Color::WHITE),
        },
        TileKind::Grass => match clear_ground {
            CLEAR_GROUND_SNOW => (snow_img(), snow_color),
            CLEAR_GROUND_DESERT => (rough_img(), desert_color),
            _ => (full_grass_img(), Color::WHITE),
        },
        TileKind::Forest => {
            let (ground, density) = ctx
                .tile
                .map(|tile| (tree_ground_from_tile(tile), tree_density_from_tile(tile)))
                .unwrap_or((TreeGround::Grass, 3));
            let sprite_id = tree_ground_sprite_id(ground, density, tileh, ctx.tx, ctx.ty);
            let fallback = matches!(ground, TreeGround::Other(_));
            record_tree_ground(sprite_id, fallback);
            match ground {
                // DrawClearLandTile(ti, GetTreeDensity(tile)).
                TreeGround::Grass => (grass_density_image(assets, density, tileh), Color::WHITE),
                // DrawHillyLandTile(ti), incluidas sus cinco variantes planas.
                TreeGround::Rough => (
                    rough_tree_image(assets, tileh, ctx.tx, ctx.ty),
                    Color::WHITE,
                ),
                // `_clear_land_sprites_snow_desert[density] + SlopeToSpriteOffset(tileh)`.
                TreeGround::SnowDesert | TreeGround::RoughSnow => {
                    (snow_desert_image(assets, density, tileh), Color::WHITE)
                }
                TreeGround::Shore => {
                    let shore = usize::from(TILEH_TO_SHORE_SPRITE[usize::from(tileh)]);
                    (assets.shore[shore].clone(), Color::WHITE)
                }
                TreeGround::Other(_) => (full_grass_img(), Color::WHITE),
            }
        }
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

    // Los dos hitos originales requieren terreno plano y `DrawTile_Object`
    // siempre les asigna `SPR_FLAT_2_THIRD_GRASS_TILE`. Registrar ambas
    // capas permite que `world-draw` detecte una regresión en la resolución
    // ObjectID -> ObjectType, aun cuando MAP5 sea el byte alto del ID y no el
    // tipo (como en Kale).
    if ottd_type == 10
        && matches!(
            object_type,
            t if t == u16::from(OBJECT_TYPE_TRANSMITTER)
                || t == u16::from(OBJECT_TYPE_LIGHTHOUSE)
        )
    {
        WorldDrawTrace::record_sprite("object-landmark-ground", "ground", 3962, false);
    }

    // MP_OBJECT: faro/transmisor vanilla o Action3 NewGRF `views[i % len]` por tesela.
    // ObjectType de OpenTTD: 0=Transmisor, 1=Faro; ≥5 = NewGRF.
    if ottd_type == 10 {
        use crate::sprites::{TransparencyOption, is_hidden, sprite_color};
        if is_hidden(TransparencyOption::Structures) {
            return;
        }
        let tint = sprite_color(TransparencyOption::Structures);
        let landmark_trace = match object_type {
            t if t == u16::from(OBJECT_TYPE_TRANSMITTER) => Some((
                2601,
                TraceSpriteBounds {
                    ox: 7,
                    oy: 7,
                    oz: 0,
                    ex: 2,
                    ey: 2,
                    ez: 70,
                },
            )),
            t if t == u16::from(OBJECT_TYPE_LIGHTHOUSE) => Some((
                2602,
                TraceSpriteBounds {
                    ox: 4,
                    oy: 4,
                    oz: 0,
                    ex: 7,
                    ey: 7,
                    ez: 61,
                },
            )),
            _ => None,
        };
        if let Some((sprite_id, bounds)) = landmark_trace {
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "object-landmark",
                "sortable",
                sprite_id,
                0,
                false,
                (0, 0, 0),
                0,
                Some(bounds),
            );
        }
        let view_idx = ctx
            .tile
            .and_then(|t| {
                openttdrs_core::object_view_index_for_type(&t, object_type, object_catalog)
            })
            .unwrap_or(0);
        if is_newgrf_object_type_id(object_type)
            && let Some(def) = crate::render::object_newgrf::newgrf_object_def_for_type(
                object_catalog,
                object_type,
            )
            && let Some(view) = def.view(view_idx)
            && let (Some(cache), Some(images)) = (object_sprites.as_mut(), images.as_mut())
        {
            let handle = cache.handle_for(def, view_idx, view, images);
            let pos3 = overlay_pos(
                ctx.iso_pos,
                f32::from(view.x_offs),
                f32::from(view.y_offs),
                f32::from(view.width),
                f32::from(view.height),
                ctx.info.base_z,
                0.6,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                Sprite {
                    image: handle,
                    color: tint,
                    ..default()
                },
                Transform::from_translation(pos3),
            ));
            return;
        }
        // Offsets NFO OpenGFX2 32ez (`ogfx21_base_32ez.nfo` sprites 2601/2602).
        let (obj_img, obj_xrel, obj_yrel, obj_w, obj_h) = match object_type {
            t if t == u16::from(OBJECT_TYPE_TRANSMITTER) => {
                (Some(assets.transmitter.clone()), -26.0, -80.0, 54.0, 94.0)
            }
            t if t == u16::from(OBJECT_TYPE_LIGHTHOUSE) => {
                (Some(assets.lighthouse.clone()), -9.0, -52.0, 21.0, 64.0)
            }
            t if t == u16::from(OBJECT_TYPE_OWNED_LAND) => {
                (Some(assets.bought_land.clone()), -16.0, -40.0, 32.0, 48.0)
            }
            t if t == u16::from(OBJECT_TYPE_STATUE_COMPANY) => (
                Some(assets.company_statue.clone()),
                -30.0,
                -42.0,
                60.0,
                45.0,
            ),
            _ => (None, 0.0, 0.0, 0.0, 0.0),
        };
        if let Some(img) = obj_img {
            let anim = object_type == u16::from(OBJECT_TYPE_LIGHTHOUSE)
                && assets.lighthouse_anim_frames.contains_key(&2602);
            let mut sprite = if object_type == u16::from(OBJECT_TYPE_STATUE_COMPANY) {
                sprite_from_atlas_or_company_colour(
                    company,
                    owner_colour,
                    &img,
                    "assets/opengfx/tiles/object_statue_company.png",
                    tint,
                )
            } else if anim {
                assets.lighthouse_anim_frames[&2602][0].sprite()
            } else {
                img.sprite()
            };
            // Owned land no es "structure" de faro/antena; no tintar si es bought land.
            if object_type != u16::from(OBJECT_TYPE_OWNED_LAND) {
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
    let bounds =
        TraceSpriteBounds::new(0, 0, 0, 16, 16, 4 + field_slope_max_pixel_z(ctx.info.tileh));
    let mut combined = false;
    for draw in field_fence_draws(&t, ctx.info.tileh).into_iter().flatten() {
        // `StartSpriteCombine`: la primera cerca es sortable y las demás se
        // agregan al mismo objeto. La traza mantiene este orden y geometría
        // exactos para poder contrastarlos contra `DrawClearLandFence`.
        WorldDrawTrace::record_sprite_with_geometry(
            "field-fence",
            if combined { "combined" } else { "sortable" },
            draw.sprite_id,
            false,
            draw.offset,
            0,
            Some(bounds),
        );
        combined = true;

        let meta = FENCE_SPRITE_META[draw.fence_type][draw.variant];
        let off = remap_tile_offset(
            draw.offset.0 as f32,
            draw.offset.1 as f32,
            draw.offset.2 as f32,
        ) * 0.5;
        let pos3 = overlay_pos(
            Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
            meta.xrel,
            meta.yrel,
            meta.w,
            meta.h,
            ctx.info.base_z,
            draw.layer,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.fences[draw.fence_type * 6 + draw.variant].sprite(),
            Transform::from_translation(pos3),
        ));
    }
}

/// Árboles de una tesela `MP_TREES`, fiel a `DrawTile_Trees` (`tree_cmd.cpp`):
/// 1–4 árboles según bits 6–7 de m5, posiciones de `_tree_layout_xy`, especie
/// por árbol de `_tree_layout_sprite[tipo×4 + variante]` y etapa de
/// crecimiento (bits 0–2 de m5) solo en el último árbol (el resto adulto +3).
///
/// OpenTTD selecciona repetidamente la entrada con menor `x + y` de la lista
/// original. La ordenación estable conserva el mismo desempate por índice.
fn sort_tree_layers_like_openttd(layers: &mut [(usize, u8, u8)]) {
    layers.sort_by_key(|(_, dx, dy)| u16::from(*dx) + u16::from(*dy));
}

/// `DrawTile_Trees` eleva el objeto a media altura de la pendiente. La
/// posición de terreno (`base_z`) sola deja árboles y sus bounds cuatro píxeles
/// demasiado abajo en una pendiente normal (u ocho en una empinada).
const fn tree_slope_z_offset(tileh: u8) -> i32 {
    if tileh == 0 {
        0
    } else if tileh & 0x10 != 0 {
        8
    } else {
        4
    }
}

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
    let mut layers = Vec::with_capacity(count);
    for i in 0..count {
        let stage = if i == count - 1 { growth } else { 3 };
        let (dx, dy) = TREE_LAYOUT_XY[layout][i];
        layers.push((row[i] as usize + stage, dx, dy));
    }

    // `DrawTile_Trees` no dibuja en el orden de la tabla: eso determina cuál
    // árbol queda delante dentro de la misma tesela y evita invertir capas de
    // copa como en el artefacto azul observado.
    sort_tree_layers_like_openttd(&mut layers);

    for (draw_order, (sprite_idx, dx, dy)) in layers.into_iter().enumerate() {
        // `TREE_LAYOUT_SPRITE` contiene índices relativos a SPR_TREE_BASE
        // (1576). Registrar el ID original antes de resolver el atlas permite
        // contrastar el árbol azul/corrupto contra el draw proc de OpenTTD.
        let slope_z_offset = tree_slope_z_offset(ctx.info.tileh);
        WorldDrawTrace::record_sprite_with_geometry(
            "tree",
            if draw_order == 0 {
                "sortable"
            } else {
                "combined"
            },
            1576 + sprite_idx as u32,
            false,
            (i32::from(dx), i32::from(dy), 0),
            slope_z_offset,
            Some(TraceSpriteBounds::new(0, 0, 0, 16, 16, 48)),
        );
        let meta = &TREE_SPRITE_META[sprite_idx];
        // Offset sub-tesela en pantalla (misma escala que iso(): remap × 0.5).
        let off = remap_tile_offset(f32::from(dx), f32::from(dy), 0.0) * 0.5;
        let mut pos3 = overlay_pos(
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
        // `overlay_pos` recibe alturas enteras de TileZ; completar la media
        // unidad de `GetSlopeMaxPixelZ(tileh) / 2` en píxeles y en la capa.
        pos3.y += slope_z_offset as f32;
        pos3.z += slope_z_offset as f32 * 0.000_125;
        batches.trees.push((
            ctx.map_tile_chunk(),
            assets.trees[sprite_idx].sprite_colored(tint),
            Transform::from_translation(pos3),
        ));
    }
}

#[cfg(test)]
mod tests {
    use openttdrs_core::{Map, TileCoord};

    use super::{
        TreeGround, field_fence_draws, field_ground_sprite_id, field_slope_max_pixel_z,
        field_slope_pixel_z_in_corner, sort_tree_layers_like_openttd, tree_density_from_tile,
        tree_ground_from_tile, tree_ground_sprite_id, tree_shore_sprite_id,
    };

    #[test]
    fn forest_layers_follow_openttd_subtile_order_and_ties() {
        let mut layers = [(1593, 9, 3), (1611, 1, 8), (1700, 1, 8)];

        sort_tree_layers_like_openttd(&mut layers);

        assert_eq!(layers, [(1611, 1, 8), (1700, 1, 8), (1593, 9, 3)]);
    }

    #[test]
    #[allow(clippy::expect_used)] // Fixture 1×1: el acceso es parte del precondition del test.
    fn tree_ground_keeps_the_high_map2_bit_and_uses_the_shore_table() {
        let mut tile = Map::new_flat(1, 1, 0)
            .get(TileCoord::new(0, 0))
            .expect("tile");
        tile.m2 = 0xC0; // TreeGround::Shore = 3.
        assert_eq!(tree_ground_from_tile(tile), TreeGround::Shore);
        assert_eq!(tree_shore_sprite_id(29), 5946);

        // TreeGround::RoughSnow = 4 necesita MAP2 bit 8.
        tile.m2 = 0;
        tile.m2_hi = 1;
        assert_eq!(tree_ground_from_tile(tile), TreeGround::RoughSnow);
    }

    #[test]
    fn tree_ground_selector_ports_all_drawtile_trees_branches() {
        // DrawClearLandTile: base 3924 + densidad * 19 + pendiente.
        assert_eq!(tree_ground_sprite_id(TreeGround::Grass, 0, 0, 0, 0), 3924);
        assert_eq!(tree_ground_sprite_id(TreeGround::Grass, 3, 29, 0, 0), 3996);

        // DrawHillyLandTile: plano hash-aleatorio, pendiente fija.
        assert_eq!(tree_ground_sprite_id(TreeGround::Rough, 3, 0, 1, 0), 4019);
        assert_eq!(tree_ground_sprite_id(TreeGround::Rough, 3, 29, 1, 0), 4015);

        // SnowDesert y RoughSnow usan la misma tabla de cuatro densidades.
        assert_eq!(
            tree_ground_sprite_id(TreeGround::SnowDesert, 2, 29, 0, 0),
            4546
        );
        assert_eq!(
            tree_ground_sprite_id(TreeGround::RoughSnow, 2, 29, 0, 0),
            4546
        );
        assert_eq!(tree_ground_sprite_id(TreeGround::Shore, 3, 29, 0, 0), 5946);
    }

    #[test]
    #[allow(clippy::expect_used)] // Fixture y cuatro entradas contractuales del test.
    fn field_ground_and_fences_match_openttd_clear_land_sprite_contract() {
        // Valores de `table/sprites.h` y `DrawClearLandFence`:
        // field state 4 + SLOPE_STEEP_E (tileh 30) = 4202 + 18 = 4220.
        assert_eq!(field_ground_sprite_id(4, 30), 4220);

        let mut field = Map::new_flat(1, 1, 0)
            .get(TileCoord::new(0, 0))
            .expect("tile");
        // NW=fence (type 3), NE=bushes (type 1), SW=stone (type 6),
        // SE=gate (type 2). El almacenamiento m3hi es MAP4 de OpenTTD.
        field.m6 = 3 << 2;
        field.m3 = 1 << 5;
        field.m3hi = (6 << 5) | (2 << 2);
        let draws = field_fence_draws(&field, 30);

        let nw = draws[0].expect("NW");
        assert_eq!((nw.sprite_id, nw.offset), (4105, (0, -16, 0)));
        let ne = draws[1].expect("NE");
        assert_eq!((ne.sprite_id, ne.offset), (4094, (-16, 0, 16)));
        let sw = draws[2].expect("SW");
        assert_eq!((sw.sprite_id, sw.offset), (4124, (0, 0, 8)));
        let se = draws[3].expect("SE");
        assert_eq!((se.sprite_id, se.offset), (4099, (0, 0, 8)));
    }

    #[test]
    fn field_fence_steep_corner_height_matches_get_slope_pixel_z_in_corner() {
        // `landscape.cpp`: la esquina máxima de una pendiente STEEP está a
        // dos TILE_HEIGHT (16 px), no a una. Cubrir las cuatro constantes de
        // `slope_type.h` evita que una futura simplificación vuelva a dejar
        // las cercas flotando un nivel por debajo.
        assert_eq!(field_slope_max_pixel_z(0), 0);
        assert_eq!(field_slope_max_pixel_z(0x04), 8);
        assert_eq!(field_slope_max_pixel_z(23), 16); // SLOPE_STEEP_S
        assert_eq!(field_slope_pixel_z_in_corner(27, 0x01), 16); // W
        assert_eq!(field_slope_pixel_z_in_corner(23, 0x02), 16); // S
        assert_eq!(field_slope_pixel_z_in_corner(30, 0x04), 16); // E
        assert_eq!(field_slope_pixel_z_in_corner(29, 0x08), 16); // N
        assert_eq!(field_slope_pixel_z_in_corner(30, 0x01), 0);
    }

    #[test]
    #[allow(clippy::expect_used)] // Fixture mínima, acceso intencional.
    fn tree_density_comes_from_map2_not_tree_count() {
        let mut tile = Map::new_flat(1, 1, 0)
            .get(TileCoord::new(0, 0))
            .expect("tile");
        tile.m2 = 0x30; // density=3; ground=Grass.
        tile.m5 = 0; // no debe alterar GetTreeDensity.
        assert_eq!(tree_density_from_tile(tile), 3);
    }
}
