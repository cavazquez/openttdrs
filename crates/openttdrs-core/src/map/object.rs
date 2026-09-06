//! Objetos de mapa (`MP_OBJECT` / `TileType::Object` en `OpenTTD`).

use super::{Map, Tile, TileCoord, TileKind, tile_slope_and_z, water_class};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_sprites::{
    CALLBACK_FAILED, CBID_OBJECT_ANIMATION_NEXT_FRAME, CBID_OBJECT_ANIMATION_SPEED,
};
use crate::object_spec::{
    NEW_OBJECT_OFFSET, ObjectSpecDef, decode_object_tile_offset, encode_object_tile_offset,
    object_footprint_tile_index, object_spec_def,
};
use crate::sav::SavObject;
use crate::world_gen::Climate;
use std::collections::HashSet;
use std::hash::BuildHasher;

/// Nibble alto de `mapt` para teselas objeto.
pub const OTTD_MP_OBJECT: u8 = 10;

/// `mapt` con tipo objeto (bits 4–7 = 10).
pub const MP_OBJECT_MAPT: u8 = OTTD_MP_OBJECT << 4;

/// `ObjectType` en saves vanilla (`object_type.h`).
pub const OBJECT_TYPE_TRANSMITTER: u8 = 0;
pub const OBJECT_TYPE_LIGHTHOUSE: u8 = 1;
/// Estatua de compañía construida por la autoridad local (`SPR_STATUE_COMPANY`).
pub const OBJECT_TYPE_STATUE_COMPANY: u8 = 2;
pub const OBJECT_TYPE_OWNED_LAND: u8 = 3;

/// Conteos de instancias `Object` para `ObjectScopeResolver::0x64`.
///
/// El contador nativo es por objeto (no por tesela del footprint). Se
/// precalcula una vez por pase de render y se consulta por `ObjectType`.
#[derive(Debug, Clone, Default)]
pub struct ObjectScopeCounts {
    by_type: std::collections::HashMap<u16, u32>,
}

impl ObjectScopeCounts {
    /// Construye una instantánea de las instancias persistidas en `OBJS`.
    #[must_use]
    pub fn from_objects(objects: &[SavObject]) -> Self {
        let mut counts = Self::default();
        for object in objects {
            let entry = counts.by_type.entry(object.object_type).or_default();
            *entry = entry.saturating_add(1);
        }
        counts
    }

    /// Número de objetos de un tipo global.
    #[must_use]
    pub fn count(&self, object_type: u16) -> u32 {
        self.by_type.get(&object_type).copied().unwrap_or(0)
    }
}

#[must_use]
pub const fn is_map_object_tile(mapt: u8) -> bool {
    (mapt >> 4) & 0xF == OTTD_MP_OBJECT
}

/// `true` si un tipo de objeto es un id `NewGRF` (`≥` [`NEW_OBJECT_OFFSET`]).
#[must_use]
pub const fn is_newgrf_object_type_id(object_type: u16) -> bool {
    object_type >= NEW_OBJECT_OFFSET
}

/// Compatibilidad para mapas locales históricos que guardan el tipo en `m5`.
#[must_use]
pub const fn is_newgrf_object_type(m5: u8) -> bool {
    is_newgrf_object_type_id(m5 as u16)
}

/// `ObjectID` crudo de una tesela `MP_OBJECT`.
///
/// OpenTTD lo forma como `m2() | (m5() << 16)`. El tipo visual se consulta en
/// el pool `OBJS`, no en `m5`.
#[must_use]
pub const fn object_id_from_tile(tile: &Tile) -> Option<u32> {
    if is_map_object_tile(tile.mapt) {
        Some((tile.m2 as u32) | ((tile.m2_hi as u32) << 8) | ((tile.m5 as u32) << 16))
    } else {
        None
    }
}

/// Tipo codificado localmente en `m5`.
///
/// Los mapas creados por este proyecto conservaron este layout antes de que el
/// importador recibiera el pool `OBJS`. Para una partida OpenTTD importada usar
/// [`Map::object_type_at`], que consulta el `ObjectID` y el footer `OBTY`.
#[must_use]
pub const fn object_type_from_tile(tile: &Tile) -> Option<u8> {
    if is_map_object_tile(tile.mapt) {
        Some(tile.m5)
    } else {
        None
    }
}

impl Map {
    /// Instala el pool `ObjectID → ObjectType` leído del footer `OBTY`.
    ///
    /// Incluso un pool vacío es significativo: desactiva el fallback histórico
    /// que interpreta `m5` como tipo y por tanto preserva la semántica cruda de
    /// un save OpenTTD moderno.
    pub(crate) fn set_imported_object_types_from_footer(&mut self, types: &[(u32, u16)]) {
        self.imported_object_types = Some(types.iter().copied().collect());
    }

    /// Tipo visual efectivo de una tesela `MP_OBJECT`.
    ///
    /// En imports modernos usa el pool `OBJS` transportado por `OBTY`; para
    /// mapas locales o exports históricos sin ese footer mantiene el formato
    /// previo donde el tipo estaba en `m5`.
    #[must_use]
    pub fn object_type_at(&self, at: TileCoord) -> Option<u16> {
        let tile = self.get(at)?;
        let object_id = object_id_from_tile(&tile)?;
        match &self.imported_object_types {
            Some(types) => types.get(&object_id).copied(),
            None => Some(u16::from(tile.m5)),
        }
    }
}

/// Id de spec `NewGRF` persistido en `m5`, si aplica.
#[must_use]
pub const fn object_spec_id_from_tile(tile: &Tile) -> Option<u16> {
    match object_type_from_tile(tile) {
        Some(m5) if is_newgrf_object_type(m5) => Some(m5 as u16),
        _ => None,
    }
}

#[must_use]
pub const fn is_owned_land_tile(tile: &Tile) -> bool {
    is_map_object_tile(tile.mapt) && tile.m5 == OBJECT_TYPE_OWNED_LAND
}

/// Dimensiones W×H del objeto (vanilla = 1×1).
#[must_use]
pub fn object_type_dims(object_type: u8, catalog: &[ObjectSpecDef]) -> (u8, u8) {
    object_type_dims_id(u16::from(object_type), catalog)
}

/// Dimensiones W×H de un `ObjectType` completo (incluye IDs NewGRF > 255).
#[must_use]
pub fn object_type_dims_id(object_type: u16, catalog: &[ObjectSpecDef]) -> (u8, u8) {
    if is_newgrf_object_type_id(object_type) {
        object_spec_def(catalog, object_type).map_or((1, 1), |d| (d.size_width(), d.size_height()))
    } else {
        (1, 1)
    }
}

/// Teselas del footprint con origen en `origin` y tamaño `w`×`h`.
#[must_use]
pub fn object_footprint_tiles(origin: TileCoord, w: u8, h: u8) -> Vec<TileCoord> {
    let mut out = Vec::with_capacity(usize::from(w).saturating_mul(usize::from(h)));
    for dy in 0..h {
        for dx in 0..w {
            out.push(TileCoord::new(
                origin.x.saturating_add(i32::from(dx)),
                origin.y.saturating_add(i32::from(dy)),
            ));
        }
    }
    out
}

/// Origen del objeto a partir de una tesela del footprint (`m2` = offset).
#[must_use]
pub fn object_origin_from_tile(tile: &Tile, at: TileCoord) -> Option<TileCoord> {
    let _ = object_type_from_tile(tile)?;
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    Some(TileCoord::new(
        at.x.saturating_sub(i32::from(dx)),
        at.y.saturating_sub(i32::from(dy)),
    ))
}

/// Origen de un objeto usando el pool nativo `OBJS` cuando está disponible.
///
/// En un save de `OpenTTD`, `MAP2/MAP5` guardan el `ObjectID`, no el offset de
/// la tesela dentro del footprint; el origen vive en `Object::location.tile`.
/// El fallback conserva el formato histórico del proyecto, que sí codificaba
/// `(dx, dy)` en `MAP2`.
#[must_use]
pub fn object_origin_from_tile_with_objects(
    tile: &Tile,
    at: TileCoord,
    objects: &[SavObject],
) -> Option<TileCoord> {
    let object_id = object_id_from_tile(tile)?;
    objects
        .iter()
        .find(|object| object.object_id == object_id)
        .map(|object| object.tile)
        .or_else(|| object_origin_from_tile(tile, at))
}

/// Todas las teselas del footprint del objeto que contiene `at`.
#[must_use]
pub fn object_footprint_at(
    map: &Map,
    at: TileCoord,
    catalog: &[ObjectSpecDef],
) -> Option<Vec<TileCoord>> {
    let tile = map.get(at)?;
    let object_type = map.object_type_at(at)?;
    let origin = object_origin_from_tile(&tile, at)?;
    let (w, h) = object_type_dims_id(object_type, catalog);
    Some(object_footprint_tiles(origin, w, h))
}

/// Índice de vista Action3 para la tesela (`dy * width + dx`).
#[must_use]
pub fn object_view_index_for_tile(tile: &Tile, catalog: &[ObjectSpecDef]) -> Option<usize> {
    let object_type = object_type_from_tile(tile)?;
    object_view_index_for_type(tile, u16::from(object_type), catalog)
}

/// Índice de vista Action3 para una tesela y su `ObjectType` ya resuelto.
#[must_use]
pub fn object_view_index_for_type(
    tile: &Tile,
    object_type: u16,
    catalog: &[ObjectSpecDef],
) -> Option<usize> {
    object_id_from_tile(tile)?;
    let (w, _) = object_type_dims_id(object_type, catalog);
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    Some(object_footprint_tile_index(dx, dy, w))
}

/// Codifica offset de footprint en `m2` (reexport útil para build).
#[must_use]
pub const fn object_tile_offset_byte(dx: u8, dy: u8) -> u8 {
    encode_object_tile_offset(dx, dy)
}

/// Construye el contexto de variables Action2 para una tesela de objeto.
///
/// Este wrapper conserva el contrato histórico para callers que sólo tienen
/// la tesela: las variables dependientes del pueblo siguen ausentes. El
/// renderer usa [`action2_eval_ctx_for_object_tile_with_towns`] para resolver
/// el scope completo disponible en el mapa.
#[must_use]
pub fn action2_eval_ctx_for_object_tile(tile: Tile, tileh: u8, climate: Climate) -> Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_object_tile_with_towns(
        tile,
        tileh,
        climate,
        TileCoord::new(0, 0),
        &[],
    );
    ctx.vars.remove(&0x45);
    ctx.vars.remove(&0x46);
    ctx
}

/// Variante que materializa las variables de pueblo de `ObjectScopeResolver`.
///
/// Los objetos importados todavía no conservan un puntero nativo a su pueblo,
/// así que se consulta el pueblo más cercano, igual que en los scopes de casas
/// y aeropuertos. `0x45` empaqueta zona urbana en los 16 bits altos y distancia
/// Manhattan acotada en los bajos; `0x46` devuelve la distancia euclídea al
/// cuadrado.
#[must_use]
pub fn action2_eval_ctx_for_object_tile_with_towns(
    tile: Tile,
    tileh: u8,
    climate: Climate,
    coord: TileCoord,
    towns: &[crate::town::Town],
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let random = u32::from(tile.m3);
    ctx.random_bits = random;
    // Var 5F expone los random bits en el byte alto, igual que los demás
    // scopes de tesela (`GetRandomBits` + `GetVariable(0x5F)`).
    ctx.vars.insert(0x5F, random << 8);

    // `ObjectScopeResolver::GetVariable(0x40)`: yyxx repetido en las dos
    // mitades de la palabra. El formato local guarda dx/dy en MAP2 como lo
    // hace la ruta de construcción de objetos de este proyecto.
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    let relative = u32::from(dy) << 20 | u32::from(dx) << 16 | u32::from(dy) << 8 | u32::from(dx);
    ctx.vars.insert(0x40, relative);

    // `GetTileSlope(tile) << 8 | GetTerrainType(tile)`. La marca MAP7 0x20
    // es la misma que usan las rutas road/rail para nieve/desierto importados.
    let terrain = if climate.uses_desert_patches() && tile.m7 & 0x20 != 0 {
        1
    } else if climate.uses_snow_ground() || tile.m7 & 0x20 != 0 {
        4
    } else {
        0
    };
    ctx.vars.insert(0x41, u32::from(tileh) << 8 | terrain);
    // MAP3HI es MAP4 en el save OpenTTD y contiene el frame de animación de
    // objetos cuando existe.
    ctx.vars.insert(0x43, u32::from(tile.m3hi));
    // `GetTileOwner(tile).base()`: los mapas del proyecto conservan el owner
    // directamente en m1 para MP_OBJECT.
    ctx.vars.insert(0x44, u32::from(tile.m1));
    if let Some(town) = towns
        .iter()
        .min_by_key(|town| crate::house_spec::distance_square(town.pos, coord))
    {
        let zone = crate::house_spec::get_town_radius_group(town, coord) as u32;
        let manhattan = town
            .pos
            .x
            .abs_diff(coord.x)
            .saturating_add(town.pos.y.abs_diff(coord.y))
            .min(u32::from(u16::MAX));
        ctx.vars
            .insert(0x45, (zone << 16) | manhattan.min(u32::from(u16::MAX)));
        ctx.vars
            .insert(0x46, crate::house_spec::distance_square(town.pos, coord));
    }
    ctx
}

/// Construye el contexto de un objeto con información de teselas vecinas.
///
/// `neighbor_params` contiene los pares `(variable, parámetro)` que el grafo
/// Action2 realmente solicita. Sólo se calculan esos offsets para evitar que
/// cada tesela de un mapa grande tenga que recorrer las 256 combinaciones
/// posibles de `GetNearbyTile`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_object_tile_with_map(
    map: &Map,
    tile: Tile,
    tileh: u8,
    climate: Climate,
    coord: TileCoord,
    towns: &[crate::town::Town],
    object_type: u16,
    object_origin: Option<TileCoord>,
    neighbor_params: &[(u8, u8)],
) -> Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_object_tile_with_towns(tile, tileh, climate, coord, towns);
    for &(variable, parameter) in neighbor_params {
        if !matches!(variable, 0x62 | 0x63) {
            continue;
        }
        let nearby = nearby_object_coord(map, coord, parameter);
        let same_object = object_origin.is_some_and(|origin| {
            map.object_type_at(nearby) == Some(object_type)
                && map
                    .get(nearby)
                    .and_then(|candidate| object_origin_from_tile(&candidate, nearby))
                    == Some(origin)
        });
        let value = match variable {
            0x62 => {
                nearby_object_tile_information(map, nearby, climate) | (u32::from(same_object) << 8)
            }
            0x63 if same_object => map
                .get(nearby)
                .map_or(0, |candidate| u32::from(candidate.m3hi)),
            _ => 0,
        };
        ctx.parameterized_vars.insert((variable, parameter), value);
    }
    ctx
}

/// Contexto Action2 de objeto con la instancia `OBJS` y los conteos globales.
///
/// Completa las variables que sólo existen para un objeto construido:
/// fecha/color/vista, asociación al pueblo, ids/random de vecinos y
/// `0x64` (cantidad y distancia de la instancia hermana más cercana).
#[must_use]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub fn action2_eval_ctx_for_object_tile_with_counts(
    map: &Map,
    tile: Tile,
    tileh: u8,
    climate: Climate,
    coord: TileCoord,
    towns: &[crate::town::Town],
    objects: &[SavObject],
    object_catalog: &[ObjectSpecDef],
    object_type: u16,
    object_origin: Option<TileCoord>,
    counts: &ObjectScopeCounts,
    neighbor_params: &[(u8, u8)],
) -> Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_object_tile_with_map(
        map,
        tile,
        tileh,
        climate,
        coord,
        towns,
        object_type,
        object_origin,
        &[],
    );
    let current_id = object_id_from_tile(&tile);
    let current = objects
        .iter()
        .find(|object| current_id == Some(object.object_id))
        .or_else(|| {
            object_origin.and_then(|origin| {
                objects
                    .iter()
                    .find(|object| object.tile == origin && object.object_type == object_type)
            })
        });
    let current_origin = current.map(|object| object.tile).or(object_origin);
    let town = current
        .and_then(|object| {
            (object.town != 0).then_some(object.town).and_then(|id| {
                towns
                    .iter()
                    .find(|town| town.id == id)
                    .map(|town| (town, true))
            })
        })
        .or_else(|| {
            towns
                .iter()
                .min_by_key(|town| crate::house_spec::distance_square(town.pos, coord))
                .map(|town| (town, false))
        });
    if let Some((town, _native)) = town {
        let zone = crate::house_spec::get_town_radius_group(town, coord) as u32;
        let manhattan = town
            .pos
            .x
            .abs_diff(coord.x)
            .saturating_add(town.pos.y.abs_diff(coord.y))
            .min(u32::from(u16::MAX));
        ctx.vars
            .insert(0x45, (zone << 16) | manhattan.min(u32::from(u16::MAX)));
        ctx.vars
            .insert(0x46, crate::house_spec::distance_square(town.pos, coord));
        // ObjectResolverObject's parent scope is the associated town. Select
        // its persistent storage by the object GRFID so `7C` sees the same
        // values as OpenTTD after a SAV round-trip.
        town.copy_newgrf_parent_scope(current_grfid(object_type, object_catalog), &mut ctx);
    }
    ctx.vars
        .insert(0x42, current.map_or(0, |object| object.build_date));
    ctx.vars
        .insert(0x47, current.map_or(0, |object| u32::from(object.colour)));
    ctx.vars
        .insert(0x48, current.map_or(0, |object| u32::from(object.view)));
    if let Some(origin) = current_origin {
        let dx = u32::try_from(coord.x.saturating_sub(origin.x)).unwrap_or(0);
        let dy = u32::try_from(coord.y.saturating_sub(origin.y)).unwrap_or(0);
        ctx.vars.insert(
            0x40,
            (dy & 0x0F) << 20 | (dx & 0x0F) << 16 | (dy & 0x0F) << 8 | (dx & 0x0F),
        );
    }

    for &(variable, parameter) in neighbor_params {
        let nearby = nearby_object_coord(map, coord, parameter);
        let nearby_tile = map.get(nearby);
        let nearby_id = nearby_tile.as_ref().and_then(object_id_from_tile);
        let nearby_instance =
            nearby_id.and_then(|id| objects.iter().find(|object| object.object_id == id));
        let same_object = match (current_id, nearby_id) {
            (Some(current), Some(nearby)) if current == nearby => true,
            _ => current_origin
                .is_some_and(|origin| nearby_instance.is_some_and(|object| object.tile == origin)),
        };
        let nearby_type = nearby_instance
            .map(|object| object.object_type)
            .or_else(|| map.object_type_at(nearby))
            .unwrap_or(u16::MAX);
        let value = match variable {
            0x60 => object_id_at_offset(
                nearby_tile,
                nearby_instance,
                nearby_type,
                object_catalog,
                current_grfid(object_type, object_catalog),
            ),
            0x61 if same_object => nearby_tile.map_or(0, |candidate| u32::from(candidate.m3)),
            0x62 => {
                nearby_object_tile_information(map, nearby, climate) | (u32::from(same_object) << 8)
            }
            0x63 if same_object => nearby_tile.map_or(0, |candidate| u32::from(candidate.m3hi)),
            0x64 => object_count_and_distance(
                parameter,
                &ctx,
                object_type,
                object_catalog,
                counts,
                objects,
                coord,
                current_id,
            ),
            _ => 0,
        };
        ctx.parameterized_vars.insert((variable, parameter), value);
    }
    ctx
}

/// Variante que construye la instantánea de `OBJS` para callers pequeños.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_object_tile_with_world(
    map: &Map,
    tile: Tile,
    tileh: u8,
    climate: Climate,
    coord: TileCoord,
    towns: &[crate::town::Town],
    objects: &[SavObject],
    object_catalog: &[ObjectSpecDef],
    object_type: u16,
    object_origin: Option<TileCoord>,
    neighbor_params: &[(u8, u8)],
) -> Action2EvalCtx {
    let counts = ObjectScopeCounts::from_objects(objects);
    action2_eval_ctx_for_object_tile_with_counts(
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
        &counts,
        neighbor_params,
    )
}

/// Ejecuta el scheduler `AnimateTile_Object` para objetos `NewGRF`.
///
/// El frame vive en `MAP3HI` (`m3hi`) y las teselas activas se conservan en
/// `active_tiles`, el equivalente persistido de `AnimatedTileList`. La primera
/// pasada siembra los objetos ya importados; los objetos construidos después
/// deben registrar sus teselas desde la ruta de construcción. Esto evita que un
/// callback `0xFF` vuelva a activar una animación detenida en el siguiente tick.
#[must_use]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub fn step_newgrf_object_tiles<S: BuildHasher>(
    map: &mut Map,
    tick: u64,
    objects: &[SavObject],
    towns: &mut [crate::town::Town],
    catalog: &[ObjectSpecDef],
    climate: Climate,
    world_seed: u64,
    active_tiles: &mut HashSet<TileCoord, S>,
    initialized: &mut bool,
) -> Vec<TileCoord> {
    let mut candidates = Vec::new();
    for object in objects {
        let Some(def) = object_spec_def(catalog, object.object_type) else {
            continue;
        };
        if !def.from_newgrf || !def.has_animation() || def.newgrf_runtime.is_none() {
            continue;
        }
        let width = u8::try_from(object.width)
            .ok()
            .filter(|width| *width != 0)
            .unwrap_or_else(|| def.size_width().max(1));
        let height = u8::try_from(object.height)
            .ok()
            .filter(|height| *height != 0)
            .unwrap_or_else(|| def.size_height().max(1));
        for coord in object_footprint_tiles(object.tile, width, height) {
            let Some(tile) = map.get(coord) else {
                continue;
            };
            if !is_map_object_tile(tile.mapt)
                || object_origin_from_tile_with_objects(&tile, coord, objects) != Some(object.tile)
            {
                continue;
            }
            candidates.push((coord, object.object_type, object.tile));
        }
    }
    candidates.sort_by_key(|(coord, object_type, origin)| {
        (coord.x, coord.y, *object_type, origin.x, origin.y)
    });
    candidates.dedup_by(|left, right| left.0 == right.0);

    if !*initialized {
        for &(coord, object_type, _) in &candidates {
            let Some(def) = object_spec_def(catalog, object_type) else {
                continue;
            };
            let frame = map.get(coord).map_or(0, |tile| tile.m3hi);
            if !def.animation_loops() && frame >= def.animation_frames && tick > 0 {
                continue;
            }
            active_tiles.insert(coord);
        }
        *initialized = true;
    }

    let mut dirty = Vec::new();
    for (coord, object_type, origin) in candidates {
        if !active_tiles.contains(&coord) {
            continue;
        }
        let Some(def) = object_spec_def(catalog, object_type) else {
            active_tiles.remove(&coord);
            continue;
        };
        let Some(mut tile) = map.get(coord) else {
            active_tiles.remove(&coord);
            continue;
        };
        let before = tile.m3hi;
        let mut speed = def.animation_speed.min(16);
        if def.has_animation_speed_callback() {
            let result = resolve_object_animation_callback(
                map,
                &tile,
                coord,
                origin,
                object_type,
                objects,
                towns,
                catalog,
                climate,
                CBID_OBJECT_ANIMATION_SPEED,
                0,
                0,
            );
            if result != CALLBACK_FAILED {
                speed = u8::try_from(result & 0xFF).unwrap_or(16).min(16);
            }
        }
        if !tick.is_multiple_of(1_u64 << u32::from(speed)) {
            continue;
        }

        let result = if def.has_animation_next_frame_callback() {
            let random = if def.animation_next_frame_uses_random_bits() {
                object_animation_random_bits(world_seed, tick, coord)
            } else {
                0
            };
            resolve_object_animation_callback(
                map,
                &tile,
                coord,
                origin,
                object_type,
                objects,
                towns,
                catalog,
                climate,
                CBID_OBJECT_ANIMATION_NEXT_FRAME,
                random,
                0,
            )
        } else {
            CALLBACK_FAILED
        };
        match (result & 0xFF) as u8 {
            0xFF if result != CALLBACK_FAILED => {
                active_tiles.remove(&coord);
            }
            0xFE if result != CALLBACK_FAILED => {
                if !advance_object_animation_frame(&mut tile, def) {
                    active_tiles.remove(&coord);
                }
            }
            frame if result != CALLBACK_FAILED => {
                tile.m3hi = frame;
            }
            _ => {
                if !advance_object_animation_frame(&mut tile, def) {
                    active_tiles.remove(&coord);
                }
            }
        }
        if tile.m3hi != before && map.set_tile(coord, tile).is_ok() {
            dirty.push(coord);
        }
    }
    dirty
}

#[allow(clippy::too_many_arguments)]
fn resolve_object_animation_callback(
    map: &Map,
    tile: &Tile,
    coord: TileCoord,
    origin: TileCoord,
    object_type: u16,
    objects: &[SavObject],
    towns: &mut [crate::town::Town],
    catalog: &[ObjectSpecDef],
    climate: Climate,
    callback: u16,
    param1: u32,
    param2: u32,
) -> u16 {
    let Some(def) = object_spec_def(catalog, object_type) else {
        return CALLBACK_FAILED;
    };
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };
    let tileh = tile_slope_and_z(map, coord).map_or(0, |(slope, _)| slope);
    let mut ctx = action2_eval_ctx_for_object_tile_with_world(
        map,
        *tile,
        tileh,
        climate,
        coord,
        &*towns,
        objects,
        catalog,
        object_type,
        Some(origin),
        &requested_object_scope_vars(runtime),
    );
    ctx.random_bits = param1;
    ctx.vars.insert(0x5F, param1 << 8);
    let result = runtime.resolve_callback_ctx(def.local_id, callback, param1, param2, &mut ctx);

    if !ctx.parent_persistent_registers.is_empty() {
        let current = objects
            .iter()
            .find(|object| object.tile == origin && object.object_type == object_type);
        let town_index = current
            .and_then(|object| {
                (object.town != 0)
                    .then_some(object.town)
                    .and_then(|id| towns.iter().position(|town| town.id == id))
            })
            .or_else(|| {
                towns
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, town)| crate::house_spec::distance_square(town.pos, coord))
                    .map(|(index, _)| index)
            });
        if let Some(index) = town_index {
            towns[index]
                .newgrf_persistent_regs
                .insert(def.grfid, ctx.parent_persistent_registers);
        }
    }
    result
}

fn requested_object_scope_vars(
    runtime: &crate::newgrf_sprites::TrainSpriteGraphics,
) -> Vec<(u8, u8)> {
    let mut params = HashSet::new();
    for entry in runtime.action2_var.values() {
        let terms = std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs));
        for term in terms {
            if (0x60..=0x64).contains(&term.variable)
                && let Some(parameter) = term.param
            {
                params.insert((term.variable, parameter));
            }
        }
    }
    let mut params: Vec<_> = params.into_iter().collect();
    params.sort_unstable();
    params
}

fn object_animation_random_bits(world_seed: u64, tick: u64, coord: TileCoord) -> u32 {
    let low = crate::map::industry_tile_rng(world_seed, tick, coord, 0x4F42_4A41);
    let high = crate::map::industry_tile_rng(world_seed, tick, coord, 0x4F42_4A42);
    u32::from(low) | (u32::from(high) << 8)
}

fn advance_object_animation_frame(tile: &mut Tile, def: &ObjectSpecDef) -> bool {
    if tile.m3hi < def.animation_frames {
        tile.m3hi = tile.m3hi.saturating_add(1);
        true
    } else if def.animation_loops() {
        tile.m3hi = 0;
        true
    } else {
        false
    }
}

fn current_grfid(object_type: u16, object_catalog: &[ObjectSpecDef]) -> u32 {
    object_spec_def(object_catalog, object_type).map_or(0, |def| def.grfid)
}

fn object_id_at_offset(
    _tile: Option<Tile>,
    instance: Option<&SavObject>,
    object_type: u16,
    catalog: &[ObjectSpecDef],
    current_grfid: u32,
) -> u32 {
    let Some(instance) = instance else {
        return 0xFFFF;
    };
    let Some(def) = object_spec_def(catalog, object_type) else {
        return 0xFFFE;
    };
    if !def.from_newgrf {
        return 0xFFFE;
    }
    if def.grfid != current_grfid {
        return 0xFFFE;
    }
    u32::from(def.local_id) | (u32::from(instance.view) << 16)
}

#[allow(clippy::too_many_arguments)]
fn object_count_and_distance(
    parameter: u8,
    ctx: &Action2EvalCtx,
    current_type: u16,
    catalog: &[ObjectSpecDef],
    counts: &ObjectScopeCounts,
    objects: &[SavObject],
    coord: TileCoord,
    current_id: Option<u32>,
) -> u32 {
    let requested_grfid = ctx.registers_100.get(&0x100).copied().unwrap_or(0);
    let target = if requested_grfid == 0 {
        Some(u16::from(parameter))
    } else if requested_grfid == u32::MAX {
        object_spec_def(catalog, current_type)
            .map(|current| current.grfid)
            .and_then(|grfid| {
                catalog
                    .iter()
                    .find(|def| def.from_newgrf && def.grfid == grfid && def.local_id == parameter)
                    .map(|def| def.id)
            })
    } else {
        catalog
            .iter()
            .find(|def| {
                def.from_newgrf && def.grfid == requested_grfid && def.local_id == parameter
            })
            .map(|def| def.id)
    };
    let Some(target) = target else {
        return 0xFFFF;
    };
    let count = counts.count(target).min(u32::from(u16::MAX));
    let distance = objects
        .iter()
        .filter(|object| object.object_type == target)
        .filter(|object| current_id != Some(object.object_id))
        .map(|object| {
            object
                .tile
                .x
                .abs_diff(coord.x)
                .saturating_add(object.tile.y.abs_diff(coord.y))
        })
        .min()
        .unwrap_or(u32::from(u16::MAX))
        .min(u32::from(u16::MAX));
    (count << 16) | distance
}

fn nearby_object_coord(map: &Map, base: TileCoord, parameter: u8) -> TileCoord {
    let (width, height) = map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return base;
    };
    if width == 0 || height == 0 {
        return base;
    }
    let signed_nibble = |value: u8| {
        let value = i32::from(value & 0x0F);
        if value >= 8 { value - 16 } else { value }
    };
    TileCoord::new(
        base.x
            .saturating_add(signed_nibble(parameter))
            .rem_euclid(width),
        base.y
            .saturating_add(signed_nibble(parameter >> 4))
            .rem_euclid(height),
    )
}

fn nearby_object_tile_information(map: &Map, coord: TileCoord, climate: Climate) -> u32 {
    let Some(tile) = map.get(coord) else {
        return 0;
    };
    let (tileh, z) = tile_slope_and_z(map, coord).unwrap_or((0, tile.height));
    let terrain = if climate.uses_desert_patches() && tile.m7 & 0x20 != 0 {
        1
    } else if climate.uses_snow_ground() || tile.m7 & 0x20 != 0 {
        4
    } else {
        0
    };
    let water_info = water_class(tile).map_or(0, |water| (water as u8 + 1) & 3);
    let is_water = u8::from(tile.kind == TileKind::Water);
    let terrain_info = (water_info << 5) | (terrain << 2) | (is_water << 1);
    let tile_type = if tile.ottd_type_nibble() != 0 || tile.kind == TileKind::Grass {
        tile.ottd_type_nibble()
    } else {
        match tile.kind {
            TileKind::Water => 6,
            TileKind::Forest => 4,
            TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
            TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
            TileKind::House => 3,
            TileKind::Station => 5,
            TileKind::Industry => 8,
            TileKind::Void => 7,
            TileKind::ShipDepot
            | TileKind::Airport
            | TileKind::CoalField
            | TileKind::Unknown(_) => tile.ottd_type_nibble(),
            TileKind::Grass => 0,
        }
    };
    (u32::from(tile_type) << 24)
        | (u32::from(z) << 16)
        | (u32::from(terrain_info) << 8)
        | u32::from(tileh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::map::TileKind;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::sav;

    fn object_animation_callback_runtime(next_frame: u8, speed: u8) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFFFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(3, 0x158, 0x158), (4, 0x15A, 0x15A)],
                default: 0,
            },
        );
        for (set_id, value) in [(3, next_frame), (4, speed)] {
            gfx.action2_var.insert(
                set_id,
                Action2VarEntry {
                    first: Action2VarTerm {
                        variable: 0x1A,
                        param: None,
                        adjust: Action2VarAdjust {
                            shift: 0,
                            and_mask: u32::from(value),
                            ..Action2VarAdjust::default()
                        },
                    },
                    ops: Vec::new(),
                    ranges: Vec::new(),
                    default: 0,
                },
            );
        }
        gfx
    }

    fn animated_object_spec(runtime: TrainSpriteGraphics) -> ObjectSpecDef {
        ObjectSpecDef {
            id: 5,
            class_label: "TEST".into(),
            name: "Animated object".into(),
            size: 0x11,
            from_newgrf: true,
            local_id: 0,
            grfid: 0xABCD_0001,
            newgrf_grf_version: 0,
            climate_mask: 0x0F,
            build_cost_factor: 1,
            flags: crate::object_spec::OBJECT_FLAG_ANIMATION,
            animation_frames: 2,
            animation_status: 1,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: crate::object_spec::OBJECT_CALLBACK_ANIMATION_NEXT_FRAME_MASK
                | crate::object_spec::OBJECT_CALLBACK_ANIMATION_SPEED_MASK,
            views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
            associated_badges: Vec::new(),
        }
    }

    fn animated_object_fixture() -> (Map, Vec<SavObject>, Vec<ObjectSpecDef>) {
        let mut map = Map::new_flat(2, 1, 0);
        let tile = Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: MP_OBJECT_MAPT,
            m5: 5,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0x12,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        map.set_tile(TileCoord::new(0, 0), tile)
            .expect("object tile");
        let object_id = object_id_from_tile(&tile).expect("object id");
        let objects = vec![SavObject {
            object_id,
            tile: TileCoord::new(0, 0),
            width: 1,
            height: 1,
            town: 0,
            build_date: 0,
            colour: 0,
            view: 0,
            object_type: 5,
        }];
        (
            map,
            objects,
            vec![animated_object_spec(object_animation_callback_runtime(
                1, 1,
            ))],
        )
    }

    #[test]
    fn object_animation_scheduler_honours_callback_speed_and_next_frame() {
        let (mut map, objects, catalog) = animated_object_fixture();
        let mut towns = Vec::new();
        let mut active = HashSet::new();
        let mut initialized = false;

        assert!(
            step_newgrf_object_tiles(
                &mut map,
                1,
                &objects,
                &mut towns,
                &catalog,
                Climate::Temperate,
                7,
                &mut active,
                &mut initialized,
            )
            .is_empty()
        );
        assert!(active.contains(&TileCoord::new(0, 0)));
        assert_eq!(map.get(TileCoord::new(0, 0)).expect("tile").m3hi, 0);

        let dirty = step_newgrf_object_tiles(
            &mut map,
            2,
            &objects,
            &mut towns,
            &catalog,
            Climate::Temperate,
            7,
            &mut active,
            &mut initialized,
        );
        assert_eq!(dirty, vec![TileCoord::new(0, 0)]);
        assert_eq!(map.get(TileCoord::new(0, 0)).expect("tile").m3hi, 1);
        assert!(active.contains(&TileCoord::new(0, 0)));
    }

    #[test]
    fn object_animation_callback_ff_removes_tile_without_restarting() {
        let (mut map, objects, mut catalog) = animated_object_fixture();
        catalog[0].newgrf_runtime = Some(Box::new(object_animation_callback_runtime(0xFF, 1)));
        let mut towns = Vec::new();
        let mut active = HashSet::new();
        let mut initialized = false;

        let _ = step_newgrf_object_tiles(
            &mut map,
            1,
            &objects,
            &mut towns,
            &catalog,
            Climate::Temperate,
            7,
            &mut active,
            &mut initialized,
        );
        let _ = step_newgrf_object_tiles(
            &mut map,
            2,
            &objects,
            &mut towns,
            &catalog,
            Climate::Temperate,
            7,
            &mut active,
            &mut initialized,
        );
        assert!(!active.contains(&TileCoord::new(0, 0)));
        let _ = step_newgrf_object_tiles(
            &mut map,
            4,
            &objects,
            &mut towns,
            &catalog,
            Climate::Temperate,
            7,
            &mut active,
            &mut initialized,
        );
        assert!(!active.contains(&TileCoord::new(0, 0)));
        assert_eq!(map.get(TileCoord::new(0, 0)).expect("tile").m3hi, 0);
    }

    #[test]
    fn object_animation_scheduler_state_roundtrips_as_json() {
        let mut state = GameState::new(2, 1);
        state
            .newgrf_animated_object_tiles
            .insert(TileCoord::new(1, 0));
        state.newgrf_object_animation_initialized = true;

        let encoded = serde_json::to_string(&state).expect("state json");
        let decoded: GameState = serde_json::from_str(&encoded).expect("state json load");
        assert!(
            decoded
                .newgrf_animated_object_tiles
                .contains(&TileCoord::new(1, 0))
        );
        assert!(decoded.newgrf_object_animation_initialized);
    }

    #[test]
    fn dual_fixture_resolves_transmitter_and_lighthouse_from_object_pool() {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))
        .expect("fixture");
        let state = GameState::from_sav_game(sav::load(&raw).expect("load"));
        let tx = state
            .map
            .get(TileCoord::new(47, 33))
            .expect("transmitter tile");
        let lh = state
            .map
            .get(TileCoord::new(60, 55))
            .expect("lighthouse tile");
        assert_eq!(
            state.map.object_type_at(TileCoord::new(47, 33)),
            Some(u16::from(OBJECT_TYPE_TRANSMITTER))
        );
        assert_eq!(
            state.map.object_type_at(TileCoord::new(60, 55)),
            Some(u16::from(OBJECT_TYPE_LIGHTHOUSE))
        );
        assert!(object_id_from_tile(&tx).is_some());
        assert!(object_id_from_tile(&lh).is_some());
    }

    #[test]
    fn vanilla_object_ids_match_openttd_object_type_h() {
        assert_eq!(OBJECT_TYPE_TRANSMITTER, 0);
        assert_eq!(OBJECT_TYPE_LIGHTHOUSE, 1);
        assert_eq!(OBJECT_TYPE_STATUE_COMPANY, 2);
        assert_eq!(OBJECT_TYPE_OWNED_LAND, 3);
    }

    #[test]
    fn object_action2_context_matches_scope_tile_fields() {
        let tile = Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: MP_OBJECT_MAPT,
            m5: 5,
            m1: 7,
            m6: 0,
            m8: 0,
            m3: 0x2A,
            m2: object_tile_offset_byte(2, 3),
            m2_hi: 0,
            m7: 0x20,
            m3hi: 9,
        };
        let ctx = action2_eval_ctx_for_object_tile(tile, 5, Climate::SubTropical);
        assert_eq!(ctx.random_bits, 0x2A);
        assert_eq!(ctx.vars.get(&0x5F), Some(&0x2A00));
        assert_eq!(ctx.vars.get(&0x40), Some(&0x0032_0302));
        assert_eq!(ctx.vars.get(&0x41), Some(&0x0501));
        assert_eq!(ctx.vars.get(&0x43), Some(&9));
        assert_eq!(ctx.vars.get(&0x44), Some(&7));
    }

    #[test]
    fn object_action2_context_exposes_nearest_town_zone_and_distances() {
        let tile = Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: MP_OBJECT_MAPT,
            m5: 0,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: object_tile_offset_byte(0, 0),
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        let mut town = crate::town::Town {
            pos: TileCoord::new(5, 2),
            num_houses: 48,
            ..Default::default()
        };
        crate::town::update_town_radius(&mut town);

        let ctx = action2_eval_ctx_for_object_tile_with_towns(
            tile,
            0,
            Climate::Temperate,
            TileCoord::new(6, 4),
            std::slice::from_ref(&town),
        );

        assert_eq!(ctx.vars.get(&0x45), Some(&0x0004_0003));
        assert_eq!(ctx.vars.get(&0x46), Some(&5));
    }

    #[test]
    fn object_action2_context_resolves_neighbor_tile_info_and_animation() {
        let mut map = Map::new_flat(4, 4, 0);
        let mut origin = Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: MP_OBJECT_MAPT,
            m5: 110,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0x11,
            m2: object_tile_offset_byte(0, 0),
            m2_hi: 0,
            m7: 0,
            m3hi: 2,
        };
        map.set_tile(TileCoord::new(1, 1), origin)
            .expect("object origin");
        origin.m2 = object_tile_offset_byte(1, 0);
        origin.m3hi = 7;
        map.set_tile(TileCoord::new(2, 1), origin)
            .expect("object neighbour");

        let ctx = action2_eval_ctx_for_object_tile_with_map(
            &map,
            map.get(TileCoord::new(1, 1)).expect("object tile"),
            0,
            Climate::Temperate,
            TileCoord::new(1, 1),
            &[],
            110,
            Some(TileCoord::new(1, 1)),
            &[(0x62, 0x01), (0x63, 0x01)],
        );

        assert_eq!(
            ctx.parameterized_vars.get(&(0x62, 0x01)),
            Some(&0x0A00_0100)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x63, 0x01)), Some(&7));
    }

    #[test]
    fn object_action2_context_uses_pool_metadata_and_scope_counts() {
        let mut map = Map::new_flat(4, 1, 0);
        let tile = Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: MP_OBJECT_MAPT,
            // Native saves store ObjectID in MAP2/MAP5. Both footprint tiles
            // therefore carry the same id instead of the legacy offset.
            m5: 0,
            m1: 3,
            m6: 0,
            m8: 0,
            m3: 0x12,
            m2: 7,
            m2_hi: 0,
            m7: 0,
            m3hi: 9,
        };
        map.set_tile(TileCoord::new(0, 0), tile)
            .expect("object origin");
        map.set_tile(TileCoord::new(1, 0), tile)
            .expect("object footprint");

        let objects = vec![SavObject {
            object_id: 7,
            tile: TileCoord::new(0, 0),
            width: 2,
            height: 1,
            town: 1,
            build_date: 1234,
            colour: 6,
            view: 2,
            object_type: 5,
        }];
        let catalog = vec![ObjectSpecDef {
            id: 5,
            class_label: "TEST".into(),
            name: "Test object".into(),
            size: 0x12,
            from_newgrf: true,
            local_id: 4,
            grfid: 0xABCD_0001,
            newgrf_grf_version: 0,
            climate_mask: 0x0F,
            build_cost_factor: 1,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        }];
        let mut town = crate::town::Town {
            id: 1,
            pos: TileCoord::new(1, 0),
            ..Default::default()
        };
        town.newgrf_persistent_regs
            .entry(0xABCD_0001)
            .or_default()
            .insert(4, 0x1020_3040);
        let counts = ObjectScopeCounts::from_objects(&objects);
        let ctx = action2_eval_ctx_for_object_tile_with_counts(
            &map,
            map.get(TileCoord::new(1, 0)).expect("object tile"),
            0,
            Climate::Temperate,
            TileCoord::new(1, 0),
            std::slice::from_ref(&town),
            &objects,
            &catalog,
            5,
            Some(TileCoord::new(0, 0)),
            &counts,
            &[
                (0x60, 0x0F),
                (0x61, 0x0F),
                (0x62, 0x0F),
                (0x63, 0x0F),
                (0x64, 5),
            ],
        );

        assert_eq!(ctx.vars.get(&0x40), Some(&0x0001_0001));
        assert_eq!(ctx.vars.get(&0x42), Some(&1234));
        assert_eq!(ctx.vars.get(&0x45), Some(&0));
        assert_eq!(ctx.vars.get(&0x46), Some(&0));
        assert_eq!(ctx.vars.get(&0x47), Some(&6));
        assert_eq!(ctx.vars.get(&0x48), Some(&2));
        assert_eq!(ctx.parent_persistent_registers.get(&4), Some(&0x1020_3040));
        assert_eq!(ctx.parent_vars.get(&0x41), Some(&1));
        assert_eq!(
            ctx.parameterized_vars.get(&(0x60, 0x0F)),
            Some(&0x0002_0004)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x61, 0x0F)), Some(&0x12));
        assert_eq!(
            ctx.parameterized_vars.get(&(0x62, 0x0F)),
            Some(&0x0A00_0100)
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x63, 0x0F)), Some(&9));
        assert_eq!(ctx.parameterized_vars.get(&(0x64, 5)), Some(&0x0001_FFFF));
    }
}
