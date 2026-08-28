//! Colocación de industrias (MVP de `GenerateIndustries`).

use crate::command::{
    Command, apply_command, check_place_industry_spec_layout, industry_template_layout_count,
    industry_template_with_layout, simulate_generated_terraform_north_corner,
};
use crate::company::OWNER_NONE_M1;
use crate::industry::IndustrySpec;
use crate::map::tree_tile_loop::{clear_ground_type, with_clear_counter};
use crate::map::{
    Map, TileCoord, TileKind, WaterClass, clear_neighbour_non_flooding_states, set_water_class_m1,
};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_SNOW,
    clear_ground_m5,
};

use super::{PopCtx, in_preserve};

/// `PlaceIndustry` prueba hasta este número de teselas para una especie ya
/// seleccionada. La selección ponderada ocurre fuera de este bucle; no se
/// vuelve a sortear el tipo al rechazar una ubicación.
const INDUSTRY_PLACEMENT_ATTEMPTS: usize = 2_000;

/// Límite `try_hard` de `PlaceIndustry` al satisfacer `force_one` durante la
/// creación de un mapa nuevo.
const FORCED_INDUSTRY_PLACEMENT_ATTEMPTS: usize = 10_000;

/// Intenta colocar hasta `target` industrias; devuelve cuántas se crearon.
pub(super) fn place_industries(
    ctx: &mut PopCtx<'_>,
    target: usize,
    town_centers: &[TileCoord],
) -> usize {
    if target == 0 {
        return 0;
    }
    let specs = IndustrySpec::specs_for_climate(ctx.state.climate);
    if specs.is_empty() {
        return 0;
    }
    let mut industry_origins: Vec<TileCoord> = Vec::with_capacity(target);

    // `GenerateIndustries` atiende primero `force_one`, en orden ascendente
    // de `IndustryType`, con `PlaceIndustry(..., true)`. No consume un sorteo
    // de especie antes de esos intentos. Temperate tiene las diez especies
    // vanilla; los demás climas continúan temporalmente por la ruta MVP hasta
    // que RMAP-056 porte sus probabilidades y listas force-one.
    let forced_specs = map_creation_force_one_specs(ctx.state.climate);
    for &spec in forced_specs {
        let _ = try_place_industry(
            ctx,
            spec,
            FORCED_INDUSTRY_PLACEMENT_ATTEMPTS,
            town_centers,
            &mut industry_origins,
        );
    }

    // En OpenTTD, `total_amount` se reduce por cada especie force-one aunque
    // su búsqueda falle. A falta del reparto tierra/agua de RMAP-056, `target`
    // conserva el equivalente base y evita volver a agregar colocaciones en
    // mapas donde las forzadas ya cubren el total (por ejemplo 64² normal).
    for _ in 0..target.saturating_sub(forced_specs.len()) {
        // `GenerateIndustries` elige una especie y `PlaceIndustry` conserva
        // esa elección mientras explora hasta 2000 `RandomTile()`. El peso
        // native se integra en RMAP-056; conservar ya esta frontera evita
        // volver a sortear tras cada fallo de sitio.
        let spec = specs[usize::try_from(
            ctx.rng
                .random_range(u32::try_from(specs.len()).unwrap_or(1)),
        )
        .unwrap_or(0)];
        let _ = try_place_industry(
            ctx,
            spec,
            INDUSTRY_PLACEMENT_ATTEMPTS,
            town_centers,
            &mut industry_origins,
        );
    }
    industry_origins.len()
}

fn map_creation_force_one_specs(climate: crate::Climate) -> &'static [IndustrySpec] {
    match climate {
        crate::Climate::Temperate => IndustrySpec::temperate_map_creation_force_one(),
        crate::Climate::SubArctic | crate::Climate::SubTropical | crate::Climate::Toyland => &[],
    }
}

/// Área que `CheckIfCanLevelIndustryPlatform` recorre alrededor del layout.
///
/// `TileHeight` es la esquina norte guardada de cada tesela. El ancho/alto de
/// C++ incluye el borde de la huella y `construction.industry_platform` a
/// ambos lados; las coordenadas se conservan exclusivas al final para que el
/// recorrido row-major coincida con `TileArea`.
fn generated_industry_platform_area(
    map: &Map,
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
    platform: u8,
) -> Option<(i32, i32, i32, i32)> {
    let template = industry_template_with_layout(origin, spec, layout_index)?;
    let max_layout_x = template
        .iter()
        .map(|(tile, _)| tile.x.saturating_sub(origin.x))
        .max()
        .unwrap_or(0);
    let max_layout_y = template
        .iter()
        .map(|(tile, _)| tile.y.saturating_sub(origin.y))
        .max()
        .unwrap_or(0);
    let margin = i32::from(platform);
    if origin.x <= margin.saturating_add(1) || origin.y <= margin.saturating_add(1) {
        return None;
    }
    let start_x = origin.x.saturating_sub(margin);
    let start_y = origin.y.saturating_sub(margin);
    let end_x = start_x
        .saturating_add(max_layout_x)
        .saturating_add(2 + 2 * margin);
    let end_y = start_y
        .saturating_add(max_layout_y)
        .saturating_add(2 + 2 * margin);
    let (map_w, map_h) = map.dimensions();
    let max_map_x = i32::try_from(map_w).ok()?.saturating_sub(1);
    let max_map_y = i32::try_from(map_h).ok()?.saturating_sub(1);
    // El original aborta si `start + width >= Map::MaxX/Y`, dejando el último
    // borde como `MP_VOID` fuera de una plataforma generada.
    if end_x >= max_map_x || end_y >= max_map_y {
        return None;
    }
    Some((start_x, start_y, end_x, end_y))
}

/// `CheckCanTerraformSurroundingTiles` de `industry_cmd.cpp`.
///
/// La primera llamada valida un cuadrado 2×2 de suelo/árboles alrededor de la
/// esquina; si una altura difiere, la llamada interna exige que cada vecino se
/// mantenga a una unidad de la altura objetivo. Sólo hay una recursión porque
/// `internal != 0` ya no abre vecinos adicionales.
fn generated_industry_can_terraform_surroundings(
    map: &Map,
    tile: TileCoord,
    height: u8,
    internal: u8,
) -> bool {
    if tile.x == 0 || tile.y == 0 || map.get_kind(tile) == Some(TileKind::Void) {
        return false;
    }
    for y in tile.y.saturating_sub(1)..=tile.y {
        for x in tile.x.saturating_sub(1)..=tile.x {
            let current = TileCoord::new(x, y);
            let Some(current_tile) = map.get(current) else {
                return false;
            };
            if !matches!(current_tile.kind, TileKind::Grass | TileKind::Forest) {
                return false;
            }
            if internal != 0 && current_tile.height.abs_diff(height) > 1 {
                return false;
            }
            if internal == 0
                && current_tile.height != height
                && (current.x == 0
                    || current.y == 0
                    || !generated_industry_can_terraform_surroundings(
                        map,
                        TileCoord::new(current.x - 1, current.y - 1),
                        height,
                        internal.saturating_add(1),
                    ))
            {
                return false;
            }
        }
    }
    true
}

/// Paso de prueba de `CheckIfCanLevelIndustryPlatform`.
///
/// Igual que el original, cada `CmdTerraformLand` se prueba contra el mapa
/// intacto: las mutaciones sólo ocurren después de que toda la plataforma haya
/// pasado esta primera ronda.
fn generated_industry_platform_is_valid(
    map: &Map,
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
    platform: u8,
) -> bool {
    let Some((start_x, start_y, end_x, end_y)) =
        generated_industry_platform_area(map, origin, spec, layout_index, platform)
    else {
        return false;
    };
    let Some(target_height) = map.get(origin).map(|tile| tile.height) else {
        return false;
    };
    for y in start_y..end_y {
        for x in start_x..end_x {
            let c = TileCoord::new(x, y);
            let Some(current) = map.get(c) else {
                return false;
            };
            if current.height == target_height {
                continue;
            }
            if !generated_industry_can_terraform_surroundings(map, c, target_height, 0)
                || simulate_generated_terraform_north_corner(
                    map,
                    c,
                    current.height <= target_height,
                )
                .is_none()
            {
                return false;
            }
        }
    }
    true
}

/// `DoClearSquare` mínimo de una tesela tocada por la terraformación gratuita
/// de una plataforma durante `GenerateWorld`.
fn clear_generated_industry_platform_tile(map: &mut Map, c: TileCoord) -> bool {
    let Some(mut tile) = map.get(c) else {
        return false;
    };
    if !matches!(tile.kind, TileKind::Grass | TileKind::Forest) {
        return false;
    }
    clear_neighbour_non_flooding_states(map, c);
    tile.kind = TileKind::Grass;
    tile.mapt &= 0x0F;
    tile.m1 = OWNER_NONE_M1;
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m5 = clear_ground_m5(0, 3);
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    map.set_tile(c, tile).is_ok()
}

/// Segundo pase `Execute` de `CheckIfCanLevelIndustryPlatform`.
///
/// Cada esquina se mueve de a un nivel, usando el mismo modelo que el pase de
/// prueba y limpiando las teselas incidentes antes de escribir las alturas.
/// Si el mapa cambió entre ambos pases se aborta sin intentar colocar la
/// industria; el flujo normal siempre lo llama inmediatamente tras validar.
fn level_generated_industry_platform(
    map: &mut Map,
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
    platform: u8,
) -> bool {
    if !generated_industry_platform_is_valid(map, origin, spec, layout_index, platform) {
        return false;
    }
    let Some((start_x, start_y, end_x, end_y)) =
        generated_industry_platform_area(map, origin, spec, layout_index, platform)
    else {
        return false;
    };
    let Some(target_height) = map.get(origin).map(|tile| tile.height) else {
        return false;
    };
    for y in start_y..end_y {
        for x in start_x..end_x {
            let c = TileCoord::new(x, y);
            loop {
                let Some(current_height) = map.get(c).map(|tile| tile.height) else {
                    return false;
                };
                if current_height == target_height {
                    break;
                }
                let Some(step) = simulate_generated_terraform_north_corner(
                    map,
                    c,
                    current_height <= target_height,
                ) else {
                    return false;
                };
                if !step
                    .dirty_tiles
                    .iter()
                    .copied()
                    .all(|dirty| clear_generated_industry_platform_tile(map, dirty))
                {
                    return false;
                }
                for (height_x, height_y, height) in step.heights {
                    if map
                        .set_height(TileCoord::new(height_x, height_y), height)
                        .is_err()
                    {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Ejecuta una llamada completa a `PlaceIndustry` para una especie fija.
///
/// El límite pertenece a la llamada, no a una selección global. Incluso los
/// rechazos consumen el prefijo de `CreateNewIndustry`, tal como RMAP-058.
fn try_place_industry(
    ctx: &mut PopCtx<'_>,
    spec: IndustrySpec,
    attempts: usize,
    _town_centers: &[TileCoord],
    industry_origins: &mut Vec<TileCoord>,
) -> bool {
    for _ in 0..attempts {
        let attempt = generated_industry_attempt(
            ctx.rng,
            ctx.mw,
            ctx.mh,
            industry_template_layout_count(spec),
        );
        let origin = attempt.origin;
        if in_preserve(ctx.preserve, origin.x, origin.y) {
            continue;
        }
        // `CreateNewIndustryHelper` no tiene una distancia euclídea genérica
        // a pueblos ni a cualquier industria. Asocia el pueblo más cercano y
        // sólo rechaza restricciones declaradas por el spec (o conflictos
        // explícitos por tipo). La antigua heurística 10²/8² descartaba la
        // posición nativa `(21,41)` de la primera coal mine de RMAP-061.
        // Los conflictos y behaviours por spec se portan en RMAP-062; no se
        // sustituyen aquí por una distancia inventada que cambie el stream.
        if check_place_industry_spec_layout(&ctx.state.map, origin, spec, attempt.layout_index)
            .is_err()
        {
            continue;
        }
        if !generated_industry_platform_is_valid(
            &ctx.state.map,
            origin,
            spec,
            attempt.layout_index,
            ctx.industry_platform,
        ) {
            continue;
        }
        let Ok(layout_index) = u8::try_from(attempt.layout_index) else {
            continue;
        };
        if !level_generated_industry_platform(
            &mut ctx.state.map,
            origin,
            spec,
            attempt.layout_index,
            ctx.industry_platform,
        ) {
            continue;
        }
        if apply_command(
            ctx.state,
            &Command::PlaceIndustrySpecLayout(origin, spec, layout_index),
        )
        .is_err()
        {
            continue;
        }
        // La cola de `DoCreateNewIndustry` consume producción smooth,
        // color/counter, `MakeIndustry` y triggers de construcción. No
        // pertenece al intento RMAP-058: ocurre sólo después de que el sitio
        // fue aceptado y determina el primer `RandomTile` de la especie
        // force-one siguiente.
        consume_successful_industry_constructor_rng(ctx.rng, spec, attempt.layout_index);
        // `DoCreateNewIndustry` planta 50 campos alrededor de una granja con
        // `PlantRandomFarmField`. El mapa generado debe conservar el contrato
        // MP_CLEAR/CLEAR_FIELDS (no un TileKind inventado): el renderer y el
        // save de OpenTTD leen m5=0x0f, m2=IndustryID y m3=estado.
        if matches!(spec, IndustrySpec::Farm | IndustrySpec::FarmTropic) {
            let industry_id = ctx
                .state
                .industries
                .last()
                .map_or(0, |industry| industry.instance_id);
            plant_farm_fields(ctx, origin, industry_id);
        }
        industry_origins.push(origin);
        return true;
    }
    false
}

/// Consume el prefijo RNG de un intento de `CreateNewIndustry`.
///
/// `PlaceIndustry` obtiene primero `RandomTile()`. Cada intento consume luego
/// tres `Random()` adicionales —seed de callback, bits iniciales y selección
/// de layout— incluso si el chequeo de sitio falla. El escritor MVP aún no
/// acepta esos tres valores, pero debe consumirlos ahora para no desplazar las
/// fases posteriores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GeneratedIndustryAttempt {
    origin: TileCoord,
    layout_index: usize,
}

fn generated_industry_attempt(
    rng: &mut crate::cargodist::parity::Randomizer,
    map_w: u32,
    map_h: u32,
    layout_count: usize,
) -> GeneratedIndustryAttempt {
    let origin = random_tile(rng.next(), map_w, map_h);
    let _random_var8f = rng.next();
    let _initial_random_bits = rng.next();
    let layout_limit = u32::try_from(layout_count.max(1)).unwrap_or(1);
    let layout_index = usize::try_from(rng.random_range(layout_limit)).unwrap_or(0);
    GeneratedIndustryAttempt {
        origin,
        layout_index,
    }
}

/// Consumo RNG de la parte vanilla de `DoCreateNewIndustry` posterior a una
/// ubicación aceptada.
///
/// El callback/producción `NewGRF` y los campos de granja se tratan en issues
/// separados. Con la economía *smooth* que usa la generación por defecto,
/// `OpenTTD` toma un `RandomRange(256)` por carga producida antes del color,
/// luego un random por tesela materializada y otro por cada trigger
/// `ConstructionStageChanged`; este último se consume incluso si el tile
/// vanilla no declara un callback de animación.
fn consume_successful_industry_constructor_rng(
    rng: &mut crate::cargodist::parity::Randomizer,
    spec: IndustrySpec,
    layout_index: usize,
) {
    for _ in spec.produced_cargos() {
        let _initial_smooth_economy_rate = rng.next();
    }
    let _random_colour_and_counter = rng.next();
    let tile_count = industry_template_with_layout(TileCoord::new(0, 0), spec, layout_index)
        .map_or(0, |layout| layout.len());
    for _ in 0..tile_count {
        let _industry_tile_random = rng.next();
    }
    for _ in 0..tile_count {
        let _construction_stage_changed_random = rng.next();
    }
}

/// `RandomTileSeed` sobre mapas de potencia de dos.
fn random_tile(random: u32, map_w: u32, map_h: u32) -> TileCoord {
    let count = map_w.saturating_mul(map_h).max(1);
    let index = if map_w.is_power_of_two() && map_h.is_power_of_two() {
        random & count.saturating_sub(1)
    } else {
        random % count
    };
    TileCoord::new(
        i32::try_from(index % map_w.max(1)).unwrap_or(0),
        i32::try_from(index / map_w.max(1)).unwrap_or(0),
    )
}

const FARM_FIELD_ATTEMPTS: usize = 50;

fn farm_field_suitable(tile: crate::map::Tile, allow_fields: bool, allow_rough: bool) -> bool {
    match tile.kind {
        TileKind::Grass => match clear_ground_type(tile.m5) {
            CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => false,
            CLEAR_GROUND_ROCKY => allow_rough,
            CLEAR_GROUND_FIELDS => allow_fields,
            _ => true,
        },
        // OpenTTD permits ordinary trees as a field substrate, but not shore
        // trees. `tree_ground` is stored in m2 bits 6..8 for MP_TREES.
        TileKind::Forest => ((tile.m2 >> 6) & 0x07) != 3 && allow_rough,
        _ => false,
    }
}

fn plant_farm_fields(ctx: &mut PopCtx<'_>, origin: TileCoord, industry_id: u8) {
    let map_w = i32::try_from(ctx.mw).unwrap_or(i32::MAX);
    let map_h = i32::try_from(ctx.mh).unwrap_or(i32::MAX);
    if map_w == 0 || map_h == 0 {
        return;
    }
    for _ in 0..FARM_FIELD_ATTEMPTS {
        // `PlantFarmField`: width/height are 4..7 in temperate and are
        // derived from the same 0x303 random mask as upstream.
        let size_random = (ctx.rng.next() & 0x303).wrapping_add(0x404);
        let size_x = i32::try_from(size_random & 0xFF).unwrap_or(4).max(1);
        let size_y = i32::try_from((size_random >> 8) & 0xFF).unwrap_or(4).max(1);
        let center_x = origin.x + i32::try_from(ctx.rng.random_range(31)).unwrap_or(0) - 16;
        let center_y = origin.y + i32::try_from(ctx.rng.random_range(31)).unwrap_or(0) - 16;
        let min_x = (center_x - size_x / 2).clamp(0, map_w.saturating_sub(1));
        let min_y = (center_y - size_y / 2).clamp(0, map_h.saturating_sub(1));
        let max_x = (min_x + size_x).min(map_w);
        let max_y = (min_y + size_y).min(map_h);
        if max_x <= min_x || max_y <= min_y {
            continue;
        }

        let mut suitable = 0usize;
        let mut total = 0usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                total += 1;
                if ctx
                    .state
                    .map
                    .get(TileCoord::new(x, y))
                    .is_some_and(|tile| farm_field_suitable(tile, false, false))
                {
                    suitable += 1;
                }
            }
        }
        if suitable * 2 < total {
            continue;
        }

        let field_random = ctx.rng.next();
        let counter = u8::try_from((field_random >> 5) & 7).unwrap_or(0);
        let field_type = u8::try_from((((field_random >> 8) & 0xFF) * 9) >> 8).unwrap_or(0);
        for y in min_y..max_y {
            for x in min_x..max_x {
                let c = TileCoord::new(x, y);
                let Some(tile) = ctx.state.map.get(c) else {
                    continue;
                };
                if !farm_field_suitable(tile, true, true) || in_preserve(ctx.preserve, x, y) {
                    continue;
                }
                let mut field = tile;
                field.kind = TileKind::Grass;
                field.mapt = 0;
                field.m1 = set_water_class_m1(OWNER_NONE_M1, WaterClass::Invalid);
                field.m2 = industry_id;
                field.m3 = field_type;
                field.m5 = with_clear_counter(clear_ground_m5(CLEAR_GROUND_FIELDS, 3), counter);
                field.m6 = 0;
                field.m7 = 0;
                field.m3hi = 0;
                let _ = ctx.state.map.set_tile(c, field);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargodist::parity::Randomizer;
    use crate::game_state::GameState;
    use crate::map::{Map, tree_tile_loop::clear_density};
    use crate::world_gen::{
        Climate, PopulationGenConfig, TerrainType, WorldGenConfig, apply_world_gen_with_rng,
        generate_towns_with_rng,
    };

    fn generated_towns_state_for_platform(seed: u64) -> GameState {
        let mut map = Map::new_flat(64, 64, 0);
        let mut rng = apply_world_gen_with_rng(
            &mut map,
            &WorldGenConfig {
                climate: Climate::Temperate,
                seed,
                sea_level: 1,
                island: false,
                water_borders: Some(0x10),
                amount_of_rivers: 2,
                startup_rng_draws: 1,
                ..WorldGenConfig::default().with_terrain_type(TerrainType::Flat)
            },
            &[],
        )
        .unwrap_or_else(|error| panic!("terrain fixture failed: {error:?}"));
        let mut state = GameState::from_map(map);
        state.world_seed = seed;
        state.climate = Climate::Temperate;
        let population = PopulationGenConfig {
            seed,
            ..PopulationGenConfig::default()
        };
        let _ = generate_towns_with_rng(&mut state, &population, &[], &mut rng);
        state
    }

    #[test]
    fn farm_fields_use_openttd_clear_tile_contract() {
        let mut state = GameState::new(64, 64);
        state.world_seed = 0xCAFE;
        for y in 0..64_i32 {
            for x in 0..64_i32 {
                let c = TileCoord::new(x, y);
                let mut tile = state.map.get(c).expect("flat map tile");
                tile.mapt = 0;
                tile.m5 = clear_ground_m5(0, 3);
                tile.m1 = OWNER_NONE_M1;
                state.map.set_tile(c, tile).expect("set flat tile");
            }
        }
        let mut rng = crate::cargodist::parity::Randomizer::new(7);
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
        };
        plant_farm_fields(&mut ctx, TileCoord::new(32, 32), 7);

        let fields: Vec<_> = ctx
            .state
            .map
            .tiles()
            .iter()
            .filter(|tile| clear_ground_type(tile.m5) == CLEAR_GROUND_FIELDS)
            .collect();
        assert!(
            !fields.is_empty(),
            "a flat map must accept at least one field"
        );
        assert!(fields.iter().all(|tile| {
            tile.kind == TileKind::Grass
                && tile.mapt == 0
                && tile.m2 == 7
                && clear_density(tile.m5) == 3
                && (tile.m3 & 0x0F) <= 8
                && tile.m1 == set_water_class_m1(OWNER_NONE_M1, WaterClass::Invalid)
        }));
    }

    #[test]
    fn industry_attempt_consumes_random_tile_and_three_constructor_draws() {
        // Primera llamada de `CreateNewIndustry` de la coal mine force-one
        // para la seed 1330935378, tomada justo después de RMAP-055.
        let mut rng = Randomizer {
            state: [11_204_508, 1_784_072_412],
        };

        assert_eq!(
            generated_industry_attempt(&mut rng, 64, 64, 4),
            GeneratedIndustryAttempt {
                origin: TileCoord::new(50, 59),
                layout_index: 0,
            }
        );
        assert_eq!(rng.state, [1_957_844_100, 95_334_821]);
    }

    #[test]
    fn successful_constructor_preserves_the_next_force_one_site() {
        // Tras doce rechazos, la coal mine de 1330935378 acepta el intento
        // trece con layout 3 (diez teselas). `DoCreateNewIndustry` consume
        // su rate smooth, color/counter, diez randoms de `MakeIndustry` y
        // diez de `ConstructionStageChanged`, antes de que PowerStation saque
        // su siguiente `RandomTile`, que GDB fija en (44,5).
        let mut rng = Randomizer {
            state: [11_204_508, 1_784_072_412],
        };
        let mut accepted = GeneratedIndustryAttempt {
            origin: TileCoord::new(0, 0),
            layout_index: 0,
        };
        for _ in 0..13 {
            accepted = generated_industry_attempt(&mut rng, 64, 64, 4);
        }
        assert_eq!(accepted.origin, TileCoord::new(21, 41));
        assert_eq!(accepted.layout_index, 3);
        consume_successful_industry_constructor_rng(
            &mut rng,
            IndustrySpec::CoalMine,
            accepted.layout_index,
        );
        assert_eq!(rng.state, [2_354_350_958, 520_419_394]);
        assert_eq!(
            generated_industry_attempt(&mut rng, 64, 64, 3).origin,
            TileCoord::new(44, 5)
        );
    }

    #[test]
    fn platform_rejects_the_first_native_coal_attempt_and_accepts_its_later_site() {
        // Traza de `CreateNewIndustry` de OpenTTD para 1330935378: la primera
        // coal mine sortea layout 0, rechaza (50,59) por plataforma y acepta
        // (21,41). Ambas pendientes simples pasan antes por
        // `CheckIfCanLevelIndustryPlatform`, por eso este caso protege la
        // regla que no cubría el chequeo de slope aislado.
        let state = generated_towns_state_for_platform(1_330_935_378);
        assert!(!generated_industry_platform_is_valid(
            &state.map,
            TileCoord::new(50, 59),
            IndustrySpec::CoalMine,
            0,
            1,
        ));
        assert!(generated_industry_platform_is_valid(
            &state.map,
            TileCoord::new(21, 41),
            IndustrySpec::CoalMine,
            3,
            1,
        ));
    }
}
