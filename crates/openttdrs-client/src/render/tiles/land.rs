use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
    CLEAR_GROUND_SNOW, Climate, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND,
    OBJECT_TYPE_STATUE_COMPANY, OBJECT_TYPE_TRANSMITTER, ObjectSpecDef, effective_clear_ground,
    industry_uses_water_ground, is_newgrf_object_type_id,
};

use super::{
    helpers::{
        FLAT_WATER_LAYER_FRAC, foundation_surface_overlay_pos,
        spawn_forced_leveled_foundation_with_child_parent, spawn_foundation_child_sprite_at,
    },
    sloped_or_flat_image, spawn_ground_sprite,
};
use crate::iso::{
    RoadStopSeqGfx, full_tile_sprite_pos, ground_draw_z, overlay_pos, remap_tile_offset,
    road_stop_build_sprite_center, slope_sprite_offset, wang_hash,
};
use crate::render::atlas::AtlasSprite;
use crate::render::newgrf_cache::{runtime_fingerprint, vars};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    CompanyColoredSprites, MapVisualLayer, TileRenderContext, ViewportSortableChild,
    ViewportSortableParent, WaterTile, WorldAssets, sprite_from_atlas_or_company_colour,
    sprite_from_atlas_or_industry_palette, viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{
    CompanyColour, FENCE_MOD_BY_TILEH_NE, FENCE_MOD_BY_TILEH_NW, FENCE_MOD_BY_TILEH_SE,
    FENCE_MOD_BY_TILEH_SW, FENCE_SPRITE_META, FIELD_STATES, HOUSE_DRAW_DATA, HouseDrawSpec,
    TILEH_TO_SHORE_SPRITE, TREE_LAYOUT_SPRITE, TREE_LAYOUT_XY, TREE_SPRITE_META,
    house_building_stage_from_tile, industry_anim_layer_used_in_any_frame,
    industry_building_needs_client_anim, industry_effective_m4_for_draw,
    industry_gfx_entry_for_tile, industry_gfx_uses_fizzy_drink_anim,
    industry_gfx_uses_random_colour, industry_gfx_uses_refinery_fire_anim,
    industry_palette_colour_for_instance,
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
const fn openttd_tile_hash(tx: u32, ty: u32) -> u32 {
    (tx ^ (tx >> 2) ^ ty).wrapping_sub(ty >> 2)
}

/// Índice de `_landscape_clear_sprites_rough` para suelo áspero plano.
const fn rough_flat_variant(tx: u32, ty: u32) -> usize {
    const ROUGH_BY_HASH: [usize; 8] = [0, 1, 2, 3, 4, 0, 1, 2];
    ROUGH_BY_HASH[(openttd_tile_hash(tx, ty) & 0x07) as usize]
}

const TREE_SNOW_DESERT_BASE: [u32; 4] = [4493, 4512, 4531, 4550];
const SPR_FLAT_BARE_LAND: u32 = 3924;
const SPR_FLAT_ROUGH_LAND: u32 = 4000;
const SPR_FLAT_ROCKY_LAND_1: u32 = 4023;
const SPR_FLAT_WATER_TILE: u32 = 4061;
/// `PaletteID::PALETTE_ALL_BLACK` en el namespace de sprites vanilla.
const PALETTE_ALL_BLACK: u32 = 6140;
/// `PALETTE_RECOLOUR_START` de OpenTTD. Las industrias usan
/// `GetColourPalette(ind->random_colour)` al transformar la capa de edificio
/// de `industry_land.h`.
const PALETTE_RECOLOUR_START: u32 = 775;

/// Paleta lógica de la capa sortable de una industria vanilla.
///
/// `DrawTile_Industry` aplica `SpriteLayoutPaletteTransform` con
/// `GetColourPalette(ind->random_colour)`. La tabla local ya concentra qué
/// GFX llevan la rampa de color; conservar también esa decisión en la traza
/// evita que dos edificios geométricamente idénticos de industrias distintas
/// se fusionen en el comparador de orden.
fn industry_building_trace_palette(gfx: u16, palette_colour: CompanyColour) -> u32 {
    if industry_gfx_uses_random_colour(gfx) {
        PALETTE_RECOLOUR_START + palette_colour.as_u8() as u32
    } else {
        0
    }
}

/// Sprite de suelo que selecciona `DrawTile_Clear`, salvo campos (que además
/// llevan cercas). Mantener esta decisión pura evita que el nombre del PNG,
/// la traza `world-draw` y el selector visual diverjan.
const fn clear_ground_sprite_id(ground: u8, density: usize, tileh: u8, tx: u32, ty: u32) -> u32 {
    let slope = slope_sprite_offset(tileh) as u32;
    let density = if density > 3 { 3 } else { density } as u32;
    match ground {
        // DrawClearLandTile(ti, GetClearDensity(tile)).
        CLEAR_GROUND_GRASS => SPR_FLAT_BARE_LAND + density * 19 + slope,
        // DrawHillyLandTile(ti): sólo el caso plano usa TileHash.
        CLEAR_GROUND_ROUGH if slope != 0 => SPR_FLAT_ROUGH_LAND + slope,
        CLEAR_GROUND_ROUGH => {
            const ROUGH_IDS: [u32; 5] = [4000, 4019, 4020, 4021, 4022];
            ROUGH_IDS[rough_flat_variant(tx, ty)]
        }
        // OpenGFX 8bpp y OpenGFX2 High Def no anuncian
        // `SecondRockyTileSet` (misc bit 6), por lo que `DrawHillyLandTile`
        // usa siempre la primera serie 4023..4041. El segundo set queda
        // disponible en el atlas para un baseset que sí lo habilite.
        CLEAR_GROUND_ROCKY => SPR_FLAT_ROCKY_LAND_1 + slope,
        // La tabla `_clear_land_sprites_snow_desert` se comparte entre nieve
        // y desierto; la densidad está en los dos bits bajos de MAP5.
        CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => TREE_SNOW_DESERT_BASE[density as usize] + slope,
        // El caller separa Fields para dibujar sus cercas. Mantener una salida
        // visible sólo protege saves corruptos con un valor de suelo ajeno.
        _ => SPR_FLAT_ROUGH_LAND + slope,
    }
}

/// Decisión exacta de `DrawTile_Void`.
///
/// Con bordes libres, OpenTTD conserva la silueta de la tesela con el suelo
/// desnudo, pero aplica `PALETTE_ALL_BLACK`. Con la opción desactivada usa el
/// conjunto completo de agua, incluida la pendiente.
const fn void_ground_sprite_and_palette(tileh: u8, freeform_edges: bool) -> (u32, u32) {
    let slope = slope_sprite_offset(tileh) as u32;
    if freeform_edges {
        (SPR_FLAT_BARE_LAND + slope, PALETTE_ALL_BLACK)
    } else {
        (SPR_FLAT_WATER_TILE + slope, 0)
    }
}

/// Dibuja la tesela de borde que OpenTTD llama `Void`.
///
/// No se puede omitir aunque no sea construible: cada borde de un mapa
/// freeform sigue siendo un `DrawGroundSprite`, y su ausencia dejaba 1.020
/// comandos sin contrapartida en la traza de Kale.
pub(crate) fn spawn_void_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    freeform_edges: bool,
) {
    let slope = usize::from(slope_sprite_offset(ctx.info.tileh));
    let (sprite_id, palette) = void_ground_sprite_and_palette(ctx.info.tileh, freeform_edges);
    WorldDrawTrace::record_sprite_with_palette("ground", "ground", sprite_id, palette, false);

    let (image, color) = if freeform_edges {
        (&assets.grass_density[0][slope], Color::BLACK)
    } else {
        (&assets.water_slopes[slope], Color::WHITE)
    };
    spawn_ground_sprite(commands, image, color, ctx, slope_half_ground);
}

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
            ROUGH_IDS[rough_flat_variant(tx, ty)]
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
        &assets.rough_flat[rough_flat_variant(tx, ty)],
        &assets.rough_slopes,
    )
}

fn snow_desert_image(assets: &WorldAssets, density: usize, tileh: u8) -> AtlasSprite {
    assets.snow_desert[density.min(3)][usize::from(slope_sprite_offset(tileh))].clone()
}

fn rocky_image(assets: &WorldAssets, tileh: u8) -> AtlasSprite {
    assets.rocky[0][usize::from(slope_sprite_offset(tileh))].clone()
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

/// Offset de `TownDrawHouseLift`: child de pantalla, no coordenada TILE_SEQ.
const fn house_lift_screen_offset(position: u8) -> (i32, i32, i32) {
    let clamped = if position > openttdrs_core::map::LIFT_MAX_POSITION {
        openttdrs_core::map::LIFT_MAX_POSITION
    } else {
        position
    };
    (14, 60 - clamped as i32, 0)
}

/// Geometría que `DrawTile_Town` entrega a `AddSortableSpriteToDraw`.
///
/// El `M(...)` de `town_land.h` contiene el prisma del edificio, no el de la
/// capa de suelo. `DrawFoundation(Leveled)` puede modificar `ti->z` antes de
/// esa llamada, por lo que el origen Z debe tomar la superficie resultante y
/// no la altura cruda de la tesela.
fn house_building_trace_geometry(
    spec: &HouseDrawSpec,
    base_z: u8,
    foundation_surface_base_z: u8,
) -> (i32, TraceSpriteBounds) {
    let world_z_delta = (i32::from(foundation_surface_base_z) - i32::from(base_z)) * 8;
    (
        world_z_delta,
        TraceSpriteBounds::new(
            spec.sort_ox,
            spec.sort_oy,
            spec.sort_oz,
            spec.sort_ex,
            spec.sort_ey,
            spec.sort_ez,
        ),
    )
}

/// Bounds inclusivos del parent que OpenTTD construye a partir de `M(...)`.
fn house_building_parent_bounds(
    ctx: &TileRenderContext,
    spec: &HouseDrawSpec,
    base_z: u8,
    foundation_surface_base_z: u8,
) -> ParentSpriteBounds {
    let (world_z_delta, bounds) =
        house_building_trace_geometry(spec, base_z, foundation_surface_base_z);
    let x = ctx.tx_i32() * 16 + bounds.ox;
    let y = ctx.ty_i32() * 16 + bounds.oy;
    let z = i32::from(base_z) * 8 + world_z_delta + bounds.oz;
    ParentSpriteBounds::new(
        x,
        y,
        z,
        x + bounds.ex - 1,
        y + bounds.ey - 1,
        z + bounds.ez - 1,
    )
}

/// Caja conservadora para un sprite de casa proporcionado por NewGRF.
///
/// `HouseSpecDef` todavía no modela `TileLayoutSpriteGroup`/`M(...)`; el
/// decoder sí conserva offsets y dimensiones de Action1. Usar esa extensión
/// como prisma mantiene el sprite dentro del ordenador de viewport y evita
/// que una casa custom atraviese edificios vecinos aunque su layout avanzado
/// siga pendiente.
fn newgrf_house_parent_bounds(
    ctx: &TileRenderContext,
    view: &openttdrs_core::DecodedSprite,
    surface_base_z: u8,
) -> ParentSpriteBounds {
    let x = ctx.tx_i32() * 16 + i32::from(view.x_offs);
    let y = ctx.ty_i32() * 16 + i32::from(view.y_offs);
    let z = i32::from(surface_base_z) * 8;
    let width = i32::from(view.width).max(1);
    let height = i32::from(view.height).max(1);
    ParentSpriteBounds::new(x, y, z, x + width - 1, y + height - 1, z + height - 1)
}

/// Bounds inclusivos del parent vanilla de `DrawTile_Industry`.
///
/// La tabla generada preserva el `M(dx, dy, sx, sy, sz)` de
/// `industry_land.h`. Sólo se usa en la ruta plana y estática: una industria
/// inclinada todavía tiene un cimiento legacy sin parent común y los layouts
/// NewGRF/animados necesitan actualizar su propia caja antes de participar.
fn industry_building_parent_bounds(
    ctx: &TileRenderContext,
    spec: &crate::sprites::IndustryGfxSprite,
) -> ParentSpriteBounds {
    let x = ctx.tx_i32() * 16 + spec.sort_ox;
    let y = ctx.ty_i32() * 16 + spec.sort_oy;
    let z = i32::from(ctx.info.base_z) * 8 + spec.sort_oz;
    ParentSpriteBounds::new(
        x,
        y,
        z,
        x + spec.sort_ex - 1,
        y + spec.sort_ey - 1,
        z + spec.sort_ez - 1,
    )
}

/// Recursos prestados que sólo necesita la ruta de casas.
///
/// Las fundaciones Action5 requieren el mapa y sus vecinos, mientras que los
/// sprites de casas siguen viniendo de `WorldAssets`. Agrupar esta parte evita
/// que el contrato del spawner crezca cada vez que se acerca más a
/// `DrawTile_Town` de OpenTTD.
pub(crate) struct HouseSpawnResources<'a> {
    pub(crate) map: &'a Map,
    pub(crate) map_dims: (u32, u32),
    pub(crate) house_catalog: &'a [openttdrs_core::HouseSpecDef],
    pub(crate) house_counts: Option<&'a openttdrs_core::HouseScopeCounts>,
    pub(crate) towns: &'a [openttdrs_core::Town],
    pub(crate) climate: Climate,
    pub(crate) newgrf_stack: &'a [openttdrs_core::NewGrfEntry],
    pub(crate) foundation_newgrf: &'a [Option<openttdrs_core::DecodedSprite>],
    pub(crate) house_sprites: Option<&'a mut crate::render::NewGrfHouseSpriteCache>,
    pub(crate) action5_sprites: Option<&'a mut crate::render::NewGrfAction5SpriteCache>,
    pub(crate) images: Option<&'a mut Assets<Image>>,
}

pub(crate) fn spawn_house_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    mut resources: HouseSpawnResources<'_>,
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
        resources.house_catalog,
    );
    let spec = &HOUSE_DRAW_DATA[spec_idx];
    // Resolver el layout antes de dibujar `s1`: un grupo completo puede
    // declarar `DODRAW=0` y debe reemplazar suelo y edificio vanilla juntos.
    let house_layout = ctx.tile.and_then(|tile| {
        crate::render::house_newgrf::newgrf_house_def_for_id(
            resources.house_catalog,
            clean_house_id,
        )
        .and_then(|def| {
            resolve_newgrf_house_layout(
                resources.map,
                def,
                building_stage,
                tile,
                ctx.tx_i32(),
                ctx.ty_i32(),
                resources.climate,
                resources.towns,
                resources.house_catalog,
                resources.house_counts,
                resources.newgrf_stack,
            )
        })
    });
    let custom_house_layout = house_layout
        .as_ref()
        .is_some_and(|(_, layout, _)| layout.complete)
        && resources.house_sprites.is_some()
        && resources.images.is_some();

    // `DrawTile_Town`: si la tesela no es plana, `DrawFoundation(Leveled)`
    // muta la superficie antes de dibujar *ambas* capas de la casa. El suelo
    // `s1` no es el césped natural que había debajo: es exactamente el
    // `ground.sprite` de `town_land.h`.
    let leveled = tileh != 0;
    // No alcanza con `foundation_{tileh}`: `DrawFoundation` escoge el bloque
    // 0..3 según las dos paredes visibles frente a sus vecinos. Para casas en
    // pendientes, el atajo histórico usaba siempre el bloque original y
    // producía muros equivocados (o faltantes) en Kale. Reutilizamos el mismo
    // plan genérico que ya valida vías, puentes y estaciones.
    let forced_foundation = leveled.then(|| {
        spawn_forced_leveled_foundation_with_child_parent(
            commands,
            resources.map,
            resources.map_dims,
            assets,
            ctx,
            tileh,
            "house",
            "house-foundation",
            resources.foundation_newgrf,
            resources.action5_sprites.as_deref_mut(),
            resources.images.as_deref_mut(),
        )
    });
    let foundation_surface_base_z =
        forced_foundation.map_or(base_z, |foundation| foundation.surface_base_z);
    let house_pos = |xrel: f32, yrel: f32, w: f32, h: f32, layer: f32| {
        if leveled {
            foundation_surface_overlay_pos(
                ctx.iso_pos,
                xrel,
                yrel,
                w,
                h,
                foundation_surface_base_z,
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
    if custom_house_layout
        && let Some((def, layout, runtime_fp)) = house_layout.as_ref()
        && let (Some(cache), Some(images)) =
            (resources.house_sprites.as_mut(), resources.images.as_mut())
    {
        let _ = spawn_newgrf_house_layout_ground(
            commands,
            ctx,
            resources.map_dims.0,
            foundation_surface_base_z,
            forced_foundation.and_then(|foundation| foundation.child_parent),
            def,
            *runtime_fp,
            layout,
            cache,
            images,
        );
    } else if spec.s1 != 0 {
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
            let mut position = house_pos(spec.s1_xrel, spec.s1_yrel, spec.s1_w, spec.s1_h, 0.4);
            if let Some(parent) = forced_foundation.and_then(|foundation| foundation.child_parent) {
                // `DrawFoundation` deja el último parent activo; el ground
                // posterior usa `AddChildSpriteScreen`; los offsets NFO de
                // `s1` siguen formando parte del sprite, pero su altura ya
                // es la superficie efectiva de la fundación.
                let source_depth = viewport_source_depth(position.z, ctx.tx, resources.map_dims.0);
                position.z = source_depth;
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    ground_sprite,
                    Transform::from_translation(position),
                    ViewportSortableChild {
                        parent,
                        source_depth,
                    },
                ));
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    ground_sprite,
                    Transform::from_translation(position),
                ));
            }
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
            // En una tesela plana `DrawGroundSprite(s1)` sigue perteneciendo
            // al pase de suelo completo de OpenTTD. Dejarlo en la banda
            // sortable permitía que la reasignación global de `s2` lo pusiera
            // por debajo de su propio patio transparente (los huecos negros
            // visibles al ampliar Kale). Conservamos la posición NFO, pero
            // reservamos la profundidad exclusiva del pase ground.
            let mut position = house_pos(spec.s1_xrel, spec.s1_yrel, spec.s1_w, spec.s1_h, 0.4);
            position.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), 0.4);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                ground_sprite,
                Transform::from_translation(position),
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
    if custom_house_layout
        && let Some((def, layout, runtime_fp)) = house_layout.as_ref()
        && let (Some(cache), Some(images)) =
            (resources.house_sprites.as_mut(), resources.images.as_mut())
    {
        if spawn_newgrf_house_layout_sequence(
            commands,
            ctx,
            resources.map_dims.0,
            foundation_surface_base_z,
            def,
            *runtime_fp,
            layout,
            cache,
            images,
            tint,
        ) {
            return;
        }
        // Layout completo sin secuencia: el ground ya fue emitido arriba.
        return;
    }
    // `DrawNewHouseTile` resuelve el grupo Action2 después de la fundación.
    // Antes el catálogo sólo se consultaba para elegir el sustituto vanilla,
    // por lo que cualquier casa con random/variational terminaba mostrando
    // una fila de `HOUSE_DRAW_DATA` ajena. Repetimos la resolución con las
    // variables persistidas de la tesela y mantenemos el ground vanilla como
    // fallback mientras no exista un layout de suelo NewGRF completo.
    if let Some(tile) = ctx.tile
        && let Some(def) = crate::render::house_newgrf::newgrf_house_def_for_id(
            resources.house_catalog,
            clean_house_id,
        )
        && let (Some(cache), Some(images)) =
            (resources.house_sprites.as_mut(), resources.images.as_mut())
    {
        let mut a2 = house_action2_context(
            resources.map,
            tile,
            ctx.tx_i32(),
            ctx.ty_i32(),
            resources.climate,
            resources.towns,
            resources.house_catalog,
            resources.house_counts,
            def,
        );
        a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
            resources.newgrf_stack,
            def.grfid,
        ));
        if let Some(view) = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(building_stage, &mut a2)
        } else {
            def.newgrf_view(building_stage).cloned()
        } && let Some(handle) = cache.handle_for_runtime(def, building_stage, &mut a2, images)
        {
            let pos3 = house_pos(
                f32::from(view.x_offs),
                f32::from(view.y_offs),
                f32::from(view.width),
                f32::from(view.height),
                0.5,
            );
            let source_depth = viewport_source_depth(pos3.z, ctx.tx, resources.map_dims.0);
            let sprite = Sprite {
                image: handle,
                color: tint,
                ..default()
            };
            WorldDrawTrace::record_sprite_with_geometry(
                "house-building-newgrf",
                "sortable",
                u32::from(def.id),
                false,
                (i32::from(view.x_offs), i32::from(view.y_offs), 0),
                (i32::from(foundation_surface_base_z) - i32::from(base_z)) * 8,
                Some(TraceSpriteBounds::new(
                    i32::from(view.x_offs),
                    i32::from(view.y_offs),
                    0,
                    i32::from(view.width).max(1),
                    i32::from(view.height).max(1),
                    i32::from(view.height).max(1),
                )),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(pos3.x, pos3.y, source_depth)),
                ViewportSortableParent {
                    sprite_id: u32::from(def.id),
                    bounds: newgrf_house_parent_bounds(ctx, &view, foundation_surface_base_z),
                    insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, 2),
                    source_depth,
                },
            ));
            return;
        }
    }

    let mut building_entity = None;
    if spec.s2 != 0 {
        let (building_world_z_delta, building_bounds) =
            house_building_trace_geometry(spec, base_z, foundation_surface_base_z);
        let Some(img) = assets.houses.get(&spec.s2) else {
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "house-building",
                "sortable",
                spec.s2,
                spec.s2_palette,
                true,
                (0, 0, 0),
                building_world_z_delta,
                Some(building_bounds),
            );
            return;
        };
        let palette_image = (spec.s2_palette != 0)
            .then(|| assets.house_palettes.handle(spec.s2, spec.s2_palette))
            .flatten();
        let fallback = spec.s2_palette != 0 && palette_image.is_none();
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "house-building",
            "sortable",
            spec.s2,
            spec.s2_palette,
            fallback,
            (0, 0, 0),
            building_world_z_delta,
            Some(building_bounds),
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
        let mut pos3 = house_pos(spec.s2_xrel, spec.s2_yrel, spec.s2_w, spec.s2_h, 0.5);
        let source_depth = viewport_source_depth(pos3.z, ctx.tx, resources.map_dims.0);
        pos3.z = source_depth;
        let entity = commands
            .spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(pos3),
                ViewportSortableParent {
                    sprite_id: spec.s2,
                    bounds: house_building_parent_bounds(
                        ctx,
                        spec,
                        base_z,
                        foundation_surface_base_z,
                    ),
                    insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, 2),
                    source_depth,
                },
            ))
            .id();
        if anim {
            commands
                .entity(entity)
                .insert(crate::render::LighthouseAnim { sprite_id: spec.s2 });
        }
        building_entity = Some(entity);
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
        let lift_position = ctx
            .tile
            .map(openttdrs_core::lift_position)
            .unwrap_or(0)
            .min(openttdrs_core::map::LIFT_MAX_POSITION);
        WorldDrawTrace::record_child_sprite_screen(
            "house-lift",
            1443,
            0,
            false,
            house_lift_screen_offset(lift_position),
        );
        let lift_base = house_pos(
            spec.s2_xrel + crate::render::HOUSE_LIFT_SCREEN_X,
            spec.s2_yrel + crate::render::HOUSE_LIFT_SCREEN_Y,
            lift_w,
            lift_h,
            0.55,
        );
        let Some(parent) = building_entity else {
            return;
        };
        let lift_source_depth = viewport_source_depth(lift_base.z, ctx.tx, resources.map_dims.0);
        // Dejar la entidad ya en la posición de la partida evita un frame en
        // el que el ascensor aparece en el piso cero antes del primer update.
        let pos3 = Vec3::new(
            lift_base.x,
            lift_base.y + f32::from(lift_position),
            lift_source_depth,
        );
        let mut sprite = assets.house_lift.sprite();
        sprite.color = tint;
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(pos3),
            crate::render::HouseLiftAnim {
                base: lift_base,
                coord: ctx.coord,
            },
            ViewportSortableChild {
                parent,
                source_depth: lift_source_depth,
            },
        ));
    }
}

/// Resuelve el layout `TileSeq` de una casa con el contexto de
/// `HouseScopeResolver` disponible en el mapa y conserva su huella runtime.
#[allow(clippy::too_many_arguments)]
fn resolve_newgrf_house_layout<'a>(
    map: &Map,
    def: &'a openttdrs_core::HouseSpecDef,
    building_stage: usize,
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
    towns: &[openttdrs_core::Town],
    house_catalog: &[openttdrs_core::HouseSpecDef],
    house_counts: Option<&openttdrs_core::HouseScopeCounts>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) -> Option<(
    &'a openttdrs_core::HouseSpecDef,
    openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    u32,
)> {
    let mut action2 = house_action2_context(
        map,
        tile,
        tx,
        ty,
        climate,
        towns,
        house_catalog,
        house_counts,
        def,
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        def.grfid,
    ));
    let layout = def.newgrf_tile_layout_runtime(building_stage, &mut action2)?;
    let runtime_fp = def
        .newgrf_runtime
        .as_ref()
        .map_or(0, |_| runtime_fingerprint(&action2, vars::HOUSE, false));
    Some((def, layout, runtime_fp))
}

#[allow(clippy::too_many_arguments)]
fn house_action2_context(
    map: &Map,
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
    towns: &[openttdrs_core::Town],
    house_catalog: &[openttdrs_core::HouseSpecDef],
    house_counts: Option<&openttdrs_core::HouseScopeCounts>,
    def: &openttdrs_core::HouseSpecDef,
) -> openttdrs_core::newgrf_sprites::Action2EvalCtx {
    let neighbor_params = requested_house_scope_vars(def.newgrf_runtime.as_deref());
    house_counts.map_or_else(
        || {
            openttdrs_core::action2_eval_ctx_for_house_tile_with_map(
                map,
                tile,
                tx,
                ty,
                climate,
                towns,
                house_catalog,
                &neighbor_params,
            )
        },
        |counts| {
            openttdrs_core::action2_eval_ctx_for_house_tile_with_counts(
                map,
                tile,
                tx,
                ty,
                climate,
                towns,
                house_catalog,
                counts,
                &neighbor_params,
            )
        },
    )
}

fn requested_house_scope_vars(
    runtime: Option<&openttdrs_core::newgrf_sprites::TrainSpriteGraphics>,
) -> Vec<(u8, u8)> {
    let Some(runtime) = runtime else {
        return Vec::new();
    };
    let mut requested = Vec::new();
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if matches!(term.variable, 0x60..=0x63)
                && let Some(parameter) = term.param
                && !requested.contains(&(term.variable, parameter))
            {
                requested.push((term.variable, parameter));
            }
        }
    }
    requested.sort_unstable();
    requested
}

/// Emite el suelo de un layout de casa. Un layout completo sin ground es
/// `DODRAW=0` y por eso devuelve `true` sin recuperar `s1` vanilla.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_house_layout_ground(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    map_width: u32,
    surface_base_z: u8,
    foundation_child_parent: Option<Entity>,
    def: &openttdrs_core::HouseSpecDef,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfHouseSpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !layout.complete {
        return false;
    }
    let Some(ground) = layout.ground.as_ref() else {
        return true;
    };
    let handle = cache.handle_for_layout(def, 0, runtime_fp, &ground.sprite, images);
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(ground.sprite.x_offs),
        f32::from(ground.sprite.y_offs),
        f32::from(ground.sprite.width),
        f32::from(ground.sprite.height),
        surface_base_z,
        0.4,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let sprite = Sprite {
        image: handle,
        color: Color::WHITE,
        ..default()
    };
    if let Some(parent) = foundation_child_parent {
        spawn_foundation_child_sprite_at(commands, sprite, ctx, position, map_width, parent);
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
    true
}

/// Emite parents y children `BUILD` de un layout de casa NewGRF.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_house_layout_sequence(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    map_width: u32,
    surface_base_z: u8,
    def: &openttdrs_core::HouseSpecDef,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfHouseSpriteCache,
    images: &mut Assets<Image>,
    tint: Color,
) -> bool {
    if !layout.complete || layout.sequence.is_empty() {
        return false;
    }
    let mut last_parent: Option<(Entity, Vec2)> = None;
    for (index, layer) in layout.sequence.iter().enumerate() {
        let slot = u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX);
        let handle = cache.handle_for_layout(def, slot, runtime_fp, &layer.sprite, images);
        let width = f32::from(layer.sprite.width);
        let height = f32::from(layer.sprite.height);
        let seq = RoadStopSeqGfx {
            dx: f32::from(layer.origin[0]),
            dy: f32::from(layer.origin[1]),
            dz: if layer.is_parent() {
                f32::from(layer.origin[2])
            } else {
                0.0
            },
            x_offs: f32::from(layer.sprite.x_offs),
            y_offs: f32::from(layer.sprite.y_offs),
            remap_x_adj: 0.0,
        };
        let layer_z = 0.5 + index as f32 * 0.0003;
        let sprite = Sprite {
            image: handle,
            color: tint,
            ..default()
        };
        if layer.is_parent() {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer_z,
                seq,
                width,
                height,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            let sprite_id = u32::MAX.saturating_sub(u32::try_from(index).unwrap_or(u32::MAX));
            let bounds = object_tile_seq_bounds(
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer.origin,
                layer.extent,
            );
            let entity = commands
                .spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                    ViewportSortableParent {
                        sprite_id,
                        bounds,
                        insertion_key: viewport_insertion_key(
                            ctx.tx,
                            ctx.ty,
                            u8::try_from(index.saturating_add(2)).unwrap_or(u8::MAX),
                        ),
                        source_depth,
                    },
                ))
                .id();
            last_parent = Some((
                entity,
                Vec2::new(position.x - width / 2.0, position.y + height / 2.0),
            ));
        } else if let Some((parent, parent_top_left)) = last_parent {
            let position = object_tile_seq_child_center(
                parent_top_left,
                layer.origin,
                width,
                height,
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer_z,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                ViewportSortableChild {
                    parent,
                    source_depth,
                },
            ));
        } else {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer_z,
                seq,
                width,
                height,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
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
    industry_sprites: Option<&mut crate::render::NewGrfIndustrySpriteCache>,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    spawn_industry_tile_with_world(
        commands,
        assets,
        map,
        ctx,
        slope_half_ground,
        industries,
        Climate::Temperate,
        &[],
        &[],
        company,
        images,
        industry_catalog,
        industry_overrides,
        industry_sprites,
        foundation_newgrf,
        action5_sprites,
        newgrf_stack,
    );
}

/// Variante del renderer que recibe los pools de pueblos y tipos de industria
/// para evaluar el scope padre de `IndustryTile` como hace OpenTTD.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_industry_tile_with_world(
    commands: &mut Commands,
    assets: &WorldAssets,
    map: &Map,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    industries: &[openttdrs_core::Industry],
    climate: Climate,
    towns: &[openttdrs_core::Town],
    industry_specs: &[openttdrs_core::IndustrySpecDef],
    company: &mut CompanyColoredSprites,
    images: &mut Assets<Image>,
    industry_catalog: &[openttdrs_core::IndustryTileSpecDef],
    industry_overrides: &[u16],
    mut industry_sprites: Option<&mut crate::render::NewGrfIndustrySpriteCache>,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    let map_dims = map.dimensions();
    let map_width = map_dims.0;
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    // gfx limpio + traducción NewGRF (`GetIndustryGfx`).
    let clean = ctx
        .tile
        .map_or(0u16, |t| openttdrs_core::get_clean_industry_gfx(t.m5, t.m6));
    let translated = openttdrs_core::get_translated_industry_tile_id(clean, industry_overrides);
    let m1 = ctx.tile.map_or(0, |t| t.m1);
    let m2 = ctx
        .tile
        .map_or(0, |tile| openttdrs_core::industry_instance_id(&tile));
    let m3hi = ctx.tile.map_or(0, |t| t.m3hi);
    let stage = usize::from(openttdrs_core::industry_construction_stage(m1));
    let newgrf_def = if translated >= openttdrs_core::NEW_INDUSTRY_TILE_OFFSET {
        crate::render::industry_newgrf::newgrf_industry_tile_def(industry_catalog, translated)
    } else {
        None
    };
    // Resolver el layout antes de dibujar el suelo: un `TileLayout` completo
    // puede declarar `DODRAW=0` y debe suprimir también el agua/rough vanilla.
    // Si no hay caché disponible, se conserva todo el fallback atómico.
    let industry_layout = newgrf_def.and_then(|def| {
        ctx.tile.and_then(|_| {
            resolve_newgrf_industry_layout(
                map,
                ctx.coord,
                industries,
                towns,
                industry_specs,
                climate,
                industry_catalog,
                def,
                stage,
                newgrf_stack,
            )
        })
    });
    let custom_industry_layout = industry_layout
        .as_ref()
        .is_some_and(|(_, layout, _)| layout.complete)
        && industry_sprites.is_some();
    // Tabla vanilla: NewGRF usa subst_id si no hay sprites / como fallback.
    let gfx = if translated >= openttdrs_core::NEW_INDUSTRY_TILE_OFFSET {
        openttdrs_core::industry_tile_spec_def(industry_catalog, translated)
            .map(|d| d.subst_id)
            .unwrap_or(0)
    } else {
        translated
    };
    let palette_colour = industry_palette_colour_for_instance(m2, industries);
    let building_trace_palette = industry_building_trace_palette(gfx, palette_colour);
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
    if !custom_industry_layout && use_water {
        commands.spawn((
            MapVisualLayer,
            chunk,
            WaterTile::ANIMATED,
            assets.water.sprite(),
            Transform::from_translation(full_tile_sprite_pos(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                FLAT_WATER_LAYER_FRAC,
            )),
        ));
    } else if !custom_industry_layout
        && (ground_sid == 0 || !assets.industries.contains_key(&ground_sid))
    {
        // La tabla vanilla trae el suelo exacto (`s1`) y lo pinta más abajo.
        // Sólo conservar el terreno áspero como red de seguridad para una
        // fila realmente vacía o un asset que todavía no está disponible;
        // de otro modo se filtraba bajo `SPR_FLAT_BARE_LAND` (3924).
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
    let foundation = if leveled {
        spawn_forced_leveled_foundation_with_child_parent(
            commands,
            map,
            map_dims,
            assets,
            ctx,
            tileh,
            "industry",
            "industry-foundation",
            foundation_newgrf,
            action5_sprites,
            Some(&mut *images),
        )
    } else {
        super::helpers::ForcedLeveledFoundation {
            surface_base_z: base_z,
            child_parent: None,
        }
    };
    let overlay_z = foundation.surface_base_z;
    if custom_industry_layout
        && let Some((def, layout, runtime_fp)) = industry_layout.as_ref()
        && let Some(cache) = industry_sprites.as_mut()
    {
        let _ = spawn_newgrf_industry_layout_ground(
            commands,
            ctx,
            map_width,
            foundation.surface_base_z,
            foundation.child_parent,
            def,
            palette_colour,
            *runtime_fp,
            layout,
            cache,
            images,
        );
    }
    let overlay_at = |xrel, yrel, w, h, layer| {
        if leveled {
            foundation_surface_overlay_pos(
                ctx.iso_pos,
                xrel,
                yrel,
                w,
                h,
                foundation.surface_base_z,
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
    // Un layout completo reemplaza la secuencia vanilla completa, aunque no
    // publique un suelo. Los parents se ordenan como bloque y los children
    // conservan la relación relativa del formato `TileSeq`.
    if custom_industry_layout
        && let Some((def, layout, runtime_fp)) = industry_layout.as_ref()
        && let Some(cache) = industry_sprites.as_mut()
    {
        if spawn_newgrf_industry_layout_sequence(
            commands,
            ctx,
            map_width,
            foundation.surface_base_z,
            def,
            palette_colour,
            *runtime_fp,
            layout,
            cache,
            images,
        ) {
            return;
        }
        return;
    }
    if let Some(def) = newgrf_def
        && let Some(cache) = industry_sprites.as_mut()
    {
        let colour = Some(palette_colour);
        let neighbor_params = requested_industry_scope_vars(def.newgrf_runtime.as_deref());
        let mut a2 = openttdrs_core::action2_eval_ctx_for_industry_tile_with_world(
            map,
            ctx.coord,
            industries,
            towns,
            industry_catalog,
            industry_specs,
            climate,
            Some(def),
            &neighbor_params,
        );
        a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
            newgrf_stack,
            def.newgrf_grfid,
        ));
        if let Some(handle) = cache.handle_for_runtime(def, stage, colour, &mut a2, images) {
            let view = if def.newgrf_runtime.is_some() {
                def.newgrf_view_runtime(stage, &mut a2)
            } else {
                def.newgrf_view(stage).cloned()
            };
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
                if let Some(parent) = foundation.child_parent {
                    spawn_foundation_child_sprite_at(
                        commands, sprite, ctx, pos3, map_width, parent,
                    );
                } else {
                    commands.spawn((
                        MapVisualLayer,
                        chunk,
                        sprite,
                        Transform::from_translation(pos3),
                    ));
                }
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
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "industry-building",
                "sortable",
                s.sprite_id,
                building_trace_palette,
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
                let mut pos3 = overlay_at(s.xrel, s.yrel, s.w, s.h, 0.5);
                // Sólo los buildings vanilla planos que no cambian de frame
                // conservan una caja C++ inmutable. Los demás siguen en su
                // ruta local hasta que tengan parent/children completos.
                let sortable_parent = if !leveled && !client_anim && !refinery_fire && !fizzy_drink
                {
                    let source_depth = viewport_source_depth(pos3.z, ctx.tx, map_width);
                    pos3.z = source_depth;
                    Some(ViewportSortableParent {
                        sprite_id: s.sprite_id,
                        bounds: industry_building_parent_bounds(ctx, s),
                        insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, 2),
                        source_depth,
                    })
                } else {
                    None
                };
                let mut entity = commands.spawn((
                    MapVisualLayer,
                    chunk,
                    sprite,
                    Transform::from_translation(pos3),
                ));
                if let Some(parent) = sortable_parent {
                    entity.insert(parent);
                }
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

/// Resuelve el layout `TileSeq` de una tesela de industria con el mismo
/// contexto Action2 que la vista plana (`stage`, random y parámetros GRF).
#[allow(clippy::too_many_arguments)]
fn resolve_newgrf_industry_layout<'a>(
    map: &Map,
    coord: openttdrs_core::TileCoord,
    industries: &[openttdrs_core::Industry],
    towns: &[openttdrs_core::Town],
    industry_specs: &[openttdrs_core::IndustrySpecDef],
    climate: Climate,
    tile_catalog: &[openttdrs_core::IndustryTileSpecDef],
    def: &'a openttdrs_core::IndustryTileSpecDef,
    stage: usize,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) -> Option<(
    &'a openttdrs_core::IndustryTileSpecDef,
    openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    u32,
)> {
    let neighbor_params = requested_industry_scope_vars(def.newgrf_runtime.as_deref());
    let mut action2 = openttdrs_core::action2_eval_ctx_for_industry_tile_with_world(
        map,
        coord,
        industries,
        towns,
        tile_catalog,
        industry_specs,
        climate,
        Some(def),
        &neighbor_params,
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        def.newgrf_grfid,
    ));
    let layout = def.newgrf_tile_layout_runtime(stage, &mut action2)?;
    let runtime_fp = def
        .newgrf_runtime
        .as_ref()
        .map_or(0, |_| runtime_fingerprint(&action2, vars::INDUSTRY, false));
    Some((def, layout, runtime_fp))
}

fn requested_industry_scope_vars(
    runtime: Option<&openttdrs_core::newgrf_sprites::TrainSpriteGraphics>,
) -> Vec<(u8, u8)> {
    let Some(runtime) = runtime else {
        return Vec::new();
    };
    let mut requested = Vec::new();
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if ((0x60..=0x71).contains(&term.variable) || term.variable == 0x7A)
                && let Some(parameter) = term.param
                && !requested.contains(&(term.variable, parameter))
            {
                requested.push((term.variable, parameter));
            }
        }
    }
    requested.sort_unstable();
    requested
}

/// Emite el suelo de un layout de industria. Un layout completo sin ground
/// es `DODRAW=0` y por eso devuelve `true` sin crear sprite vanilla.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_industry_layout_ground(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    map_width: u32,
    surface_base_z: u8,
    foundation_child_parent: Option<Entity>,
    def: &openttdrs_core::IndustryTileSpecDef,
    palette_colour: CompanyColour,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfIndustrySpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !layout.complete {
        return false;
    }
    let Some(ground) = layout.ground.as_ref() else {
        return true;
    };
    let handle = cache.handle_for_layout(
        def,
        0,
        Some(palette_colour),
        runtime_fp,
        &ground.sprite,
        images,
    );
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(ground.sprite.x_offs),
        f32::from(ground.sprite.y_offs),
        f32::from(ground.sprite.width),
        f32::from(ground.sprite.height),
        surface_base_z,
        0.45,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let sprite = Sprite {
        image: handle,
        color: crate::sprites::with_to_alpha(
            Color::WHITE,
            crate::sprites::TransparencyOption::Industries,
        ),
        ..default()
    };
    if let Some(parent) = foundation_child_parent {
        spawn_foundation_child_sprite_at(commands, sprite, ctx, position, map_width, parent);
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
    true
}

/// Emite parents y children `BUILD` de un layout de industria NewGRF.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_industry_layout_sequence(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    map_width: u32,
    surface_base_z: u8,
    def: &openttdrs_core::IndustryTileSpecDef,
    palette_colour: CompanyColour,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfIndustrySpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !layout.complete || layout.sequence.is_empty() {
        return false;
    }
    let tint =
        crate::sprites::with_to_alpha(Color::WHITE, crate::sprites::TransparencyOption::Industries);
    let mut last_parent: Option<(Entity, Vec2)> = None;
    for (index, layer) in layout.sequence.iter().enumerate() {
        let slot = u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX);
        let handle = cache.handle_for_layout(
            def,
            slot,
            Some(palette_colour),
            runtime_fp,
            &layer.sprite,
            images,
        );
        let width = f32::from(layer.sprite.width);
        let height = f32::from(layer.sprite.height);
        let seq = RoadStopSeqGfx {
            dx: f32::from(layer.origin[0]),
            dy: f32::from(layer.origin[1]),
            dz: if layer.is_parent() {
                f32::from(layer.origin[2])
            } else {
                0.0
            },
            x_offs: f32::from(layer.sprite.x_offs),
            y_offs: f32::from(layer.sprite.y_offs),
            remap_x_adj: 0.0,
        };
        let layer_z = 0.5 + index as f32 * 0.0003;
        let sprite = Sprite {
            image: handle,
            color: tint,
            ..default()
        };
        if layer.is_parent() {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer_z,
                seq,
                width,
                height,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            let sprite_id = u32::MAX.saturating_sub(u32::try_from(index).unwrap_or(u32::MAX));
            let bounds = object_tile_seq_bounds(
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer.origin,
                layer.extent,
            );
            let entity = commands
                .spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                    ViewportSortableParent {
                        sprite_id,
                        bounds,
                        insertion_key: viewport_insertion_key(
                            ctx.tx,
                            ctx.ty,
                            u8::try_from(index.saturating_add(2)).unwrap_or(u8::MAX),
                        ),
                        source_depth,
                    },
                ))
                .id();
            last_parent = Some((
                entity,
                Vec2::new(position.x - width / 2.0, position.y + height / 2.0),
            ));
        } else if let Some((parent, parent_top_left)) = last_parent {
            let position = object_tile_seq_child_center(
                parent_top_left,
                layer.origin,
                width,
                height,
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer_z,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                ViewportSortableChild {
                    parent,
                    source_depth,
                },
            ));
        } else {
            // Un child huérfano es inválido según el contrato, pero conservar
            // el sprite en el ancla de la tesela evita perderlo por completo.
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                layer_z,
                seq,
                width,
                height,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
    }
    true
}

/// Bounds inclusivos de una entrada parent `TileSeq` de objeto.
#[allow(clippy::too_many_arguments)]
fn object_tile_seq_bounds(
    tx: i32,
    ty: i32,
    base_z: u8,
    origin: [i8; 3],
    extent: [u8; 3],
) -> ParentSpriteBounds {
    let x = tx * 16 + i32::from(origin[0]);
    let y = ty * 16 + i32::from(origin[1]);
    let z = i32::from(base_z) * 8 + i32::from(origin[2]);
    ParentSpriteBounds::new(
        x,
        y,
        z,
        x + i32::from(extent[0]) - 1,
        y + i32::from(extent[1]) - 1,
        z + i32::from(extent[2]) - 1,
    )
}

/// Centro de un child `TileSeq`: offsets de pantalla desde la esquina
/// superior izquierda del parent, con el eje Y invertido al entrar en Bevy.
#[allow(clippy::too_many_arguments)]
fn object_tile_seq_child_center(
    parent_top_left: Vec2,
    origin: [i8; 3],
    width: f32,
    height: f32,
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
) -> Vec3 {
    let top_left = parent_top_left + Vec2::new(f32::from(origin[0]), -f32::from(origin[1]));
    Vec3::new(
        top_left.x + width / 2.0,
        top_left.y - height / 2.0,
        crate::iso::sortable_draw_z(tx, ty, base_z, layer_z),
    )
}

/// Resuelve el `TileSeq` de un objeto para la tesela concreta del footprint.
#[allow(clippy::too_many_arguments)]
fn resolve_newgrf_object_layout<'a>(
    map: &Map,
    object_type: u16,
    tile: Tile,
    tileh: u8,
    coord: openttdrs_core::TileCoord,
    climate: Climate,
    object_catalog: &'a [ObjectSpecDef],
    towns: &[openttdrs_core::Town],
    objects: &[openttdrs_core::sav::SavObject],
    object_counts: Option<&openttdrs_core::ObjectScopeCounts>,
) -> Option<(
    &'a ObjectSpecDef,
    openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    u32,
    usize,
)> {
    let def =
        crate::render::object_newgrf::newgrf_object_def_for_type(object_catalog, object_type)?;
    let view_idx =
        openttdrs_core::object_view_index_for_type(&tile, object_type, object_catalog).unwrap_or(0);
    let object_origin = openttdrs_core::object_origin_from_tile_with_objects(&tile, coord, objects);
    let neighbor_params = requested_object_neighbor_vars(def.newgrf_runtime.as_deref());
    let mut action2 = object_counts.map_or_else(
        || {
            openttdrs_core::action2_eval_ctx_for_object_tile_with_world(
                map,
                tile,
                tileh,
                climate,
                coord,
                towns,
                objects,
                object_catalog,
                object_type,
                object_origin,
                &neighbor_params,
            )
        },
        |counts| {
            openttdrs_core::action2_eval_ctx_for_object_tile_with_counts(
                map,
                tile,
                tileh,
                climate,
                coord,
                towns,
                objects,
                object_catalog,
                object_type,
                object_origin,
                counts,
                &neighbor_params,
            )
        },
    );
    let layout = def.newgrf_tile_layout_runtime(view_idx, &mut action2)?;
    let runtime_fp = def
        .newgrf_runtime
        .as_ref()
        .map_or(0, |_| runtime_fingerprint(&action2, vars::OBJECT, false));
    Some((def, layout, runtime_fp, view_idx))
}

fn requested_object_neighbor_vars(
    runtime: Option<&openttdrs_core::newgrf_sprites::TrainSpriteGraphics>,
) -> Vec<(u8, u8)> {
    let Some(runtime) = runtime else {
        return Vec::new();
    };
    let mut requested = Vec::new();
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if matches!(term.variable, 0x60..=0x64)
                && let Some(parameter) = term.param
                && !requested.contains(&(term.variable, parameter))
            {
                requested.push((term.variable, parameter));
            }
        }
    }
    requested.sort_unstable();
    requested
}

/// Emite el suelo de un layout de objeto. Un resultado completo sin `ground`
/// representa `DODRAW=0` y suprime el suelo genérico de la tesela.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_object_layout_ground(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    def: &ObjectSpecDef,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfObjectSpriteCache,
    images: &mut Assets<Image>,
    tint: Color,
) -> bool {
    if !layout.complete {
        return false;
    }
    let Some(ground) = layout.ground.as_ref() else {
        return true;
    };
    let handle = cache.handle_for_layout(def, 0, runtime_fp, &ground.sprite, images);
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(ground.sprite.x_offs),
        f32::from(ground.sprite.y_offs),
        f32::from(ground.sprite.width),
        f32::from(ground.sprite.height),
        ctx.info.base_z,
        0.55,
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
        Transform::from_translation(position),
    ));
    true
}

/// Emite parents y children `BUILD` de un layout de objeto NewGRF.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_object_layout_sequence(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    map_width: u32,
    def: &ObjectSpecDef,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfObjectSpriteCache,
    images: &mut Assets<Image>,
    tint: Color,
) -> bool {
    if !layout.complete || layout.sequence.is_empty() {
        return false;
    }
    let mut last_parent: Option<(Entity, Vec2)> = None;
    for (index, layer) in layout.sequence.iter().enumerate() {
        let slot = u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX);
        let handle = cache.handle_for_layout(def, slot, runtime_fp, &layer.sprite, images);
        let width = f32::from(layer.sprite.width);
        let height = f32::from(layer.sprite.height);
        let seq = RoadStopSeqGfx {
            dx: f32::from(layer.origin[0]),
            dy: f32::from(layer.origin[1]),
            dz: if layer.is_parent() {
                f32::from(layer.origin[2])
            } else {
                0.0
            },
            x_offs: f32::from(layer.sprite.x_offs),
            y_offs: f32::from(layer.sprite.y_offs),
            remap_x_adj: 0.0,
        };
        let layer_z = 0.6 + index as f32 * 0.0003;
        let sprite = Sprite {
            image: handle,
            color: tint,
            ..default()
        };
        if layer.is_parent() {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                layer_z,
                seq,
                width,
                height,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            let sprite_id = u32::MAX.saturating_sub(u32::try_from(index).unwrap_or(u32::MAX));
            let bounds = object_tile_seq_bounds(
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                layer.origin,
                layer.extent,
            );
            let entity = commands
                .spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                    ViewportSortableParent {
                        sprite_id,
                        bounds,
                        insertion_key: viewport_insertion_key(
                            ctx.tx,
                            ctx.ty,
                            u8::try_from(index.saturating_add(2)).unwrap_or(u8::MAX),
                        ),
                        source_depth,
                    },
                ))
                .id();
            last_parent = Some((
                entity,
                Vec2::new(position.x - width / 2.0, position.y + height / 2.0),
            ));
        } else if let Some((parent, parent_top_left)) = last_parent {
            let position = object_tile_seq_child_center(
                parent_top_left,
                layer.origin,
                width,
                height,
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                layer_z,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                ViewportSortableChild {
                    parent,
                    source_depth,
                },
            ));
        } else {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                layer_z,
                seq,
                width,
                height,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn spawn_generic_land_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    map: &Map,
    slope_half_ground: f32,
    climate: Climate,
    world_seed: u64,
    map_width: u32,
    object_catalog: &[ObjectSpecDef],
    towns: &[openttdrs_core::Town],
    object_sprites: Option<&mut crate::render::NewGrfObjectSpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    spawn_generic_land_tile_with_objects(
        commands,
        assets,
        company,
        owner_colour,
        ctx,
        map,
        slope_half_ground,
        climate,
        world_seed,
        map_width,
        object_catalog,
        towns,
        &[],
        None,
        object_sprites,
        images,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_generic_land_tile_with_objects(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    map: &Map,
    slope_half_ground: f32,
    climate: Climate,
    world_seed: u64,
    map_width: u32,
    object_catalog: &[ObjectSpecDef],
    towns: &[openttdrs_core::Town],
    objects: &[openttdrs_core::sav::SavObject],
    object_counts: Option<&openttdrs_core::ObjectScopeCounts>,
    mut object_sprites: Option<&mut crate::render::NewGrfObjectSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let tileh = ctx.info.tileh;
    let ottd_type = ctx.tile.map_or(0u8, |t| (t.mapt >> 4) & 0xF);
    let tile_m5 = ctx.tile.map_or(0u8, |t| t.m5);
    let object_type = ctx.object_type.unwrap_or(u16::from(tile_m5));
    let object_layout = if ottd_type == 10 && is_newgrf_object_type_id(object_type) {
        ctx.tile.and_then(|tile| {
            resolve_newgrf_object_layout(
                map,
                object_type,
                tile,
                tileh,
                ctx.coord,
                climate,
                object_catalog,
                towns,
                objects,
                object_counts,
            )
        })
    } else {
        None
    };

    // MP_CLEAR (0): distinguir subtipo de suelo vía m5 bits 2-4.
    // MP_OBJECT (10): el suelo depende del ObjectType resuelto desde OBJS;
    // m5 sólo es el byte alto crudo de ObjectID en una partida importada.
    let grass_img = || grass_density_image(assets, clear_grass_density(tile_m5), tileh);
    let full_grass_img = || sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    let rough_img = || rough_tree_image(assets, tileh, ctx.tx, ctx.ty);
    let rocky_img = || rocky_image(assets, tileh);
    let snow_desert_img = || snow_desert_image(assets, usize::from(tile_m5 & 0x03), tileh);

    // `IsSnowTile` de OpenTTD vive en MAP3 bit 4 y no cambia el tipo de suelo
    // de MAP5. Mantener el fallback de `effective_clear_ground` permite seguir
    // dibujando mapas JSON legados que codificaban `CLEAR_SNOW` en MAP5.
    let clear_ground = if ctx.tile.is_some_and(|tile| tile.m3 & 0x10 != 0) {
        CLEAR_GROUND_SNOW
    } else {
        effective_clear_ground(climate, tile_m5, ctx.tx_i32(), ctx.ty_i32(), world_seed)
    };

    let (image, color) = match ctx.kind {
        TileKind::Grass if ottd_type == 0 => match clear_ground {
            CLEAR_GROUND_GRASS => {
                WorldDrawTrace::record_sprite(
                    "clear-ground",
                    "ground",
                    clear_ground_sprite_id(
                        clear_ground,
                        clear_grass_density(tile_m5),
                        tileh,
                        ctx.tx,
                        ctx.ty,
                    ),
                    false,
                );
                (grass_img(), Color::WHITE)
            }
            CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT | CLEAR_GROUND_ROUGH | CLEAR_GROUND_ROCKY => {
                WorldDrawTrace::record_sprite(
                    "clear-ground",
                    "ground",
                    clear_ground_sprite_id(
                        clear_ground,
                        usize::from(tile_m5 & 0x03),
                        tileh,
                        ctx.tx,
                        ctx.ty,
                    ),
                    false,
                );
                let image = match clear_ground {
                    CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => snow_desert_img(),
                    CLEAR_GROUND_ROUGH => rough_img(),
                    CLEAR_GROUND_ROCKY => rocky_img(),
                    _ => unreachable!(),
                };
                (image, Color::WHITE)
            }
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
            _ => {
                WorldDrawTrace::record_sprite(
                    "clear-ground-fallback",
                    "ground",
                    clear_ground_sprite_id(
                        clear_ground,
                        usize::from(tile_m5 & 0x03),
                        tileh,
                        ctx.tx,
                        ctx.ty,
                    ),
                    true,
                );
                (rough_img(), Color::WHITE)
            }
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
            CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => (snow_desert_img(), Color::WHITE),
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
    let mut used_newgrf_layout_ground = false;
    if let Some((def, layout, runtime_fp, _view_idx)) = object_layout.as_ref()
        && let (Some(cache), Some(image_store)) = (object_sprites.as_mut(), images.as_mut())
    {
        used_newgrf_layout_ground = spawn_newgrf_object_layout_ground(
            commands,
            ctx,
            def,
            *runtime_fp,
            layout,
            cache,
            image_store,
            color,
        );
    }
    if !used_newgrf_layout_ground {
        spawn_ground_sprite(commands, &image, color, ctx, slope_half_ground);
    }

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
            && let Some(tile) = ctx.tile
            && let (Some(cache), Some(images)) = (object_sprites.as_mut(), images.as_mut())
        {
            // `DrawNewObjectTile` usa el mismo ObjectScopeResolver para cada
            // tesela. Resolver aquí permite que Action2 observe el random,
            // offset de footprint, pendiente, frame y owner en vez de usar
            // siempre el preview estático del GRF.
            let neighbor_params = requested_object_neighbor_vars(def.newgrf_runtime.as_deref());
            let object_origin =
                openttdrs_core::object_origin_from_tile_with_objects(&tile, ctx.coord, objects);
            let mut a2 = object_counts.map_or_else(
                || {
                    openttdrs_core::action2_eval_ctx_for_object_tile_with_world(
                        map,
                        tile,
                        ctx.info.tileh,
                        climate,
                        ctx.coord,
                        towns,
                        objects,
                        object_catalog,
                        object_type,
                        object_origin,
                        &neighbor_params,
                    )
                },
                |counts| {
                    openttdrs_core::action2_eval_ctx_for_object_tile_with_counts(
                        map,
                        tile,
                        ctx.info.tileh,
                        climate,
                        ctx.coord,
                        towns,
                        objects,
                        object_catalog,
                        object_type,
                        object_origin,
                        counts,
                        &neighbor_params,
                    )
                },
            );
            if let Some((layout_def, layout, runtime_fp, _layout_view_idx)) = object_layout.as_ref()
                && layout.complete
            {
                if spawn_newgrf_object_layout_sequence(
                    commands,
                    ctx,
                    map_width,
                    layout_def,
                    *runtime_fp,
                    layout,
                    cache,
                    images,
                    tint,
                ) {
                    return;
                }
                // Un layout completo sin secuencia sólo dibuja su ground (ya
                // emitido arriba), por lo que no debe duplicarse con la vista
                // plana Action1/3.
                return;
            }
            let view = if def.newgrf_runtime.is_some() {
                def.newgrf_view_runtime(view_idx, &mut a2)
            } else {
                def.view(view_idx).cloned()
            };
            if let Some(view) = view
                && let Some(handle) = cache.handle_for_runtime(def, view_idx, &mut a2, images)
            {
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
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
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

    let mut parent_entity = None;
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
        let sprite = assets.trees[sprite_idx].sprite_colored(tint);
        if draw_order == 0 {
            // Sólo la primera capa es un parent de `DrawTile_Trees`; las
            // restantes llegan con `AddCombinedSprite` y deben seguirlo si
            // el sorter global intercambia el árbol con una estructura
            // vecina. La caja no incorpora dx/dy: OpenTTD usa esos valores
            // como offset de pantalla, pero el prisma 16×16×48 empieza en la
            // tesela base (tal como lo expone world-sort).
            let source_depth = viewport_source_depth(pos3.z, ctx.tx, map_w);
            pos3.z = source_depth;
            let entity = commands
                .spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(pos3),
                    ViewportSortableParent {
                        sprite_id: 1576 + sprite_idx as u32,
                        bounds: tree_parent_bounds(ctx, slope_z_offset),
                        insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, 1),
                        source_depth,
                    },
                ))
                .id();
            parent_entity = Some(entity);
        } else {
            let mut entity = commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(pos3),
            ));
            if let Some(parent) = parent_entity {
                entity.insert(ViewportSortableChild {
                    parent,
                    source_depth: pos3.z,
                });
            }
        }
    }
}

/// Prisma que OpenTTD entrega para el primer árbol de `DrawTile_Trees`.
///
/// Las posiciones de layout desplazan el píxel de la copa, no la caja del
/// parent. `AddSortableSpriteToDraw` conserva la tesela base 16×16 y la mitad
/// de pendiente calculada por `GetSlopeMaxPixelZ(tileh) / 2`.
fn tree_parent_bounds(ctx: &TileRenderContext, slope_z_offset: i32) -> ParentSpriteBounds {
    let xmin = ctx.tx_i32() * 16;
    let ymin = ctx.ty_i32() * 16;
    let zmin = i32::from(ctx.info.base_z) * 8 + slope_z_offset;
    ParentSpriteBounds::new(xmin, ymin, zmin, xmin + 15, ymin + 15, zmin + 47)
}

#[cfg(test)]
mod tests {
    use crate::render::TileRenderContext;
    use crate::render::grid::TileRenderInfo;
    use crate::render::viewport_sort::ParentSpriteBounds;
    use crate::sprites::{CompanyColour, HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA};
    use bevy::prelude::Vec2;
    use openttdrs_core::{
        CLEAR_GROUND_DESERT, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_ROUGH,
        CLEAR_GROUND_SNOW, Map, TileCoord, TileKind,
    };

    use super::{
        TreeGround, clear_ground_sprite_id, field_fence_draws, field_ground_sprite_id,
        field_slope_max_pixel_z, field_slope_pixel_z_in_corner, house_building_trace_geometry,
        house_lift_screen_offset, industry_building_parent_bounds, industry_building_trace_palette,
        openttd_tile_hash, rough_flat_variant, sort_tree_layers_like_openttd,
        tree_density_from_tile, tree_ground_from_tile, tree_ground_sprite_id, tree_parent_bounds,
        tree_shore_sprite_id, tree_slope_z_offset, void_ground_sprite_and_palette,
    };

    fn industry_ctx_at(tx: u32, ty: u32, base_z: u8) -> TileRenderContext {
        TileRenderContext {
            tx,
            ty,
            coord: TileCoord::new(tx as i32, ty as i32),
            tile: None,
            object_type: None,
            kind: TileKind::Industry,
            info: TileRenderInfo {
                tileh: 0,
                base_z,
                use_shore: false,
            },
            iso_pos: Vec2::ZERO,
        }
    }

    #[test]
    fn clear_ground_selector_matches_openttd_drawtile_clear() {
        // DrawClearLandTile: base + density * 19 + SlopeToSpriteOffset.
        assert_eq!(clear_ground_sprite_id(CLEAR_GROUND_GRASS, 3, 0, 0, 0), 3981);
        assert_eq!(
            clear_ground_sprite_id(CLEAR_GROUND_GRASS, 3, 29, 0, 0),
            3996
        );

        // DrawHillyLandTile: TileHash sólo en plano; 1,0 produce hash 1.
        assert_eq!(openttd_tile_hash(1, 0), 1);
        assert_eq!(rough_flat_variant(1, 0), 1);
        assert_eq!(clear_ground_sprite_id(CLEAR_GROUND_ROUGH, 0, 0, 1, 0), 4019);
        assert_eq!(
            clear_ground_sprite_id(CLEAR_GROUND_ROUGH, 0, 29, 1, 0),
            4015
        );

        // OpenGFX y OpenGFX2 no activan `SecondRockyTileSet`: incluso donde
        // TileHash es impar se conserva la primera serie. Las pendientes
        // 4024..4041 no se pueden sustituir por rough.
        assert_eq!(clear_ground_sprite_id(CLEAR_GROUND_ROCKY, 0, 0, 0, 0), 4023);
        assert_eq!(clear_ground_sprite_id(CLEAR_GROUND_ROCKY, 0, 0, 1, 0), 4023);
        assert_eq!(
            clear_ground_sprite_id(CLEAR_GROUND_ROCKY, 0, 29, 0, 0),
            4038
        );

        // Nieve y desierto comparten `_clear_land_sprites_snow_desert`.
        assert_eq!(clear_ground_sprite_id(CLEAR_GROUND_SNOW, 2, 29, 0, 0), 4546);
        assert_eq!(
            clear_ground_sprite_id(CLEAR_GROUND_DESERT, 2, 29, 0, 0),
            4546
        );
    }

    #[test]
    fn void_ground_selector_matches_openttd_drawtile_void() {
        // `void_cmd.cpp`: bare land + PALETTE_ALL_BLACK with freeform edges.
        assert_eq!(void_ground_sprite_and_palette(0, true), (3924, 6140));
        assert_eq!(void_ground_sprite_and_palette(29, true), (3939, 6140));

        // Otherwise OpenTTD draws the matching water slope with PAL_NONE.
        assert_eq!(void_ground_sprite_and_palette(0, false), (4061, 0));
        assert_eq!(void_ground_sprite_and_palette(29, false), (4076, 0));
    }

    #[test]
    fn forest_layers_follow_openttd_subtile_order_and_ties() {
        let mut layers = [(1593, 9, 3), (1611, 1, 8), (1700, 1, 8)];

        sort_tree_layers_like_openttd(&mut layers);

        assert_eq!(layers, [(1611, 1, 8), (1700, 1, 8), (1593, 9, 3)]);
    }

    #[test]
    fn tree_parent_bounds_keep_base_tile_and_half_slope_height() {
        // Kale `(138,7)`: el offset de layout de la copa es `(4,4)`, pero
        // `ViewportSortParentSprites` recibe el prisma base 2208..2223,
        // 112..127 y la media pendiente z=4.
        let ctx = industry_ctx_at(138, 7, 0);
        let bounds = tree_parent_bounds(&ctx, tree_slope_z_offset(0x04));
        assert_eq!(
            (
                bounds.xmin,
                bounds.ymin,
                bounds.zmin,
                bounds.xmax,
                bounds.ymax,
                bounds.zmax,
            ),
            (2208, 112, 4, 2223, 127, 51)
        );
        assert_eq!(tree_slope_z_offset(0), 0);
        assert_eq!(tree_slope_z_offset(0x1b), 8);
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
    fn house_foundation_and_lift_match_kale_drawfoundation_examples() {
        // Kale (9,4): SLOPE_SW, fundación leveled con sólo pared NW visible.
        // `DrawFoundation` debe usar el bloque Action5 1, no `992` del
        // bloque clásico que usaba la ruta histórica de casas.
        let normal = openttdrs_core::foundation_draw_plan(0x03, 1, 1);
        assert_eq!(normal.sprites[0].map(|draw| draw.sprite_id), Some(5423));
        assert_eq!(normal.surface_z_delta, 1);

        // Kale (164,12): pendiente doble normal, bloque 3 y una elevación.
        let block_three = openttdrs_core::foundation_draw_plan(0x0C, 1, 3);
        assert_eq!(
            block_three.sprites[0].map(|draw| draw.sprite_id),
            Some(5476)
        );
        assert_eq!(block_three.surface_z_delta, 1);

        // Una pendiente STEEP sí emite el muro inferior y el medio tile
        // superior; la superficie efectiva queda dos niveles arriba. Así se
        // protege el dato que usa `foundation_surface_overlay_pos` para el
        // suelo y edificio de cualquier casa empinada.
        let steep = openttdrs_core::foundation_draw_plan(0x1B, 1, 3);
        assert_eq!(steep.sprites[0].map(|draw| draw.sprite_id), Some(5475));
        assert_eq!(steep.sprites[1].map(|draw| draw.sprite_id), Some(5465));
        assert_eq!(steep.surface_z_delta, 2);

        // TownDrawHouseLift: (14, 60 - GetLiftPosition()). El valor real de
        // Kale en la primera muestra era 12, por eso OpenTTD emitió y=48.
        assert_eq!(house_lift_screen_offset(0), (14, 60, 0));
        assert_eq!(house_lift_screen_offset(12), (14, 48, 0));
        assert_eq!(house_lift_screen_offset(63), (14, 24, 0));
    }

    #[test]
    fn house_building_trace_keeps_macro_bounds_and_effective_foundation_z() {
        // Primera entrada de `town_land.h`: M(..., 0, 0, 14, 14, 8, 0).
        let (flat_delta, flat_bounds) = house_building_trace_geometry(&HOUSE_DRAW_DATA[0], 2, 2);
        assert_eq!(flat_delta, 0);
        assert_eq!(
            (
                flat_bounds.ox,
                flat_bounds.oy,
                flat_bounds.oz,
                flat_bounds.ex,
                flat_bounds.ey,
                flat_bounds.ez,
            ),
            (0, 0, 0, 14, 14, 8)
        );

        // En una pendiente `DrawFoundation(Leveled)` cambia `ti->z` antes
        // del edificio. La segunda entrada tiene una caja alta (60 px), y
        // una superficie +1 debe reflejarse como +8 en mundo OpenTTD.
        let (sloped_delta, sloped_bounds) =
            house_building_trace_geometry(&HOUSE_DRAW_DATA[1], 2, 3);
        assert_eq!(sloped_delta, 8);
        assert_eq!(
            (
                sloped_bounds.ox,
                sloped_bounds.oy,
                sloped_bounds.oz,
                sloped_bounds.ex,
                sloped_bounds.ey,
                sloped_bounds.ez,
            ),
            (0, 0, 0, 14, 14, 60)
        );
    }

    #[test]
    fn industry_building_parent_bounds_match_kale_sortable_prism() {
        // Kale `(186,1)`: OpenTTD emite `2119` con
        // world=(2976,16,8), bounds=(0,0,0;16,16,20).
        let spec = match INDUSTRY_GFX_DATA.iter().find(|spec| spec.sprite_id == 2119) {
            Some(spec) => spec,
            None => panic!("la tabla vanilla debe contener la fábrica 2119"),
        };
        assert_eq!(
            industry_building_parent_bounds(&industry_ctx_at(186, 1, 1), spec),
            ParentSpriteBounds::new(2976, 16, 8, 2991, 31, 27)
        );
    }

    #[test]
    fn industry_building_trace_palette_keeps_the_instance_random_colour() {
        // `DrawTile_Industry` pasa GetColourPalette(ind->random_colour) a
        // SpriteLayoutPaletteTransform. Mauve = colour 10 = palette 785.
        assert_eq!(
            industry_building_trace_palette(18, CompanyColour::Mauve),
            785
        );
        // Un GFX PAL_NONE no debe adquirir el color de la industria sólo por
        // compartir la misma instancia.
        assert_eq!(industry_building_trace_palette(10, CompanyColour::Mauve), 0);
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
