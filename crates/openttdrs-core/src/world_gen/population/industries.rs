//! Colocación de industrias (MVP de `GenerateIndustries`).

use crate::command::{
    Command, apply_command, check_place_industry_spec_layout, industry_template_layout_count,
    industry_template_with_layout, simulate_generated_terraform_north_corner,
};
use crate::company::OWNER_NONE_M1;
use crate::game_state::GameState;
use crate::industry::IndustrySpec;
use crate::map::tree_tile_loop::{clear_ground_type, with_clear_counter};
use crate::map::{Map, Tile, TileCoord, TileKind, clear_neighbour_non_flooding_states};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_ROUGH, CLEAR_GROUND_SNOW,
    clear_ground_m5,
};

use super::{PopCtx, in_preserve, scale_by_land_proportion, scale_by_size};

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
    density: super::IndustryDensity,
) -> usize {
    if density == super::IndustryDensity::FundedOnly {
        return 0;
    }
    let specs = IndustrySpec::specs_for_climate(ctx.state.climate);
    if specs.is_empty() {
        return 0;
    }
    let (map_w, map_h) = ctx.state.map.dimensions();
    // `GenerateIndustries` mantiene dos distribuciones independientes: una
    // para industrias en tierra y otra para `IndustryBehaviour::BuiltOnWater`.
    // El catálogo vanilla representado hoy sólo contiene especies terrestres,
    // pero conservar la pasada vacía es importante: el total y el stream RNG
    // deben quedar listos para añadir `IT_OIL_RIG` sin reescribir esta fase.
    let land_probabilities =
        generation_probabilities(ctx.state.climate, map_w, map_h, specs, false);
    let water_probabilities =
        generation_probabilities(ctx.state.climate, map_w, map_h, specs, true);
    let total_probability = land_probabilities
        .iter()
        .chain(water_probabilities.iter())
        .map(|(_, probability)| u64::from(*probability))
        .sum::<u64>();
    if total_probability == 0 {
        return 0;
    }
    let mut industry_origins: Vec<TileCoord> = Vec::with_capacity(target);

    // OpenTTD calcula la cantidad de cada pasada a partir de la suma global
    // de probabilidades. La pasada terrestre se reduce por `CountLandTiles`;
    // la acuática no se reduce y puede usar sus propios force-one.
    let scaled_target = u32::try_from(target).unwrap_or(u32::MAX);
    for (water, probabilities) in [
        (false, land_probabilities.as_slice()),
        (true, water_probabilities.as_slice()),
    ] {
        let pass_probability: u64 = probabilities
            .iter()
            .map(|(_, probability)| u64::from(*probability))
            .sum();
        if pass_probability == 0 {
            continue;
        }
        let mut total_amount = u32::try_from(
            u64::from(scaled_target).saturating_mul(pass_probability) / total_probability,
        )
        .unwrap_or(u32::MAX);
        if !water {
            total_amount = scale_by_land_proportion(total_amount, &ctx.state.map);
        }
        let forced_specs: Vec<IndustrySpec> =
            probabilities.iter().map(|(spec, _)| **spec).collect();
        let forced_count = u32::try_from(forced_specs.len()).unwrap_or(u32::MAX);
        if total_amount < forced_count {
            total_amount = forced_count;
        }
        for spec in forced_specs {
            let _ = try_place_industry(
                ctx,
                spec,
                FORCED_INDUSTRY_PLACEMENT_ATTEMPTS,
                town_centers,
                &mut industry_origins,
            );
            total_amount = total_amount.saturating_sub(1);
        }

        for _ in 0..total_amount {
            let spec = weighted_spec(ctx.rng, probabilities, pass_probability);
            let _ = try_place_industry(
                ctx,
                spec,
                INDUSTRY_PLACEMENT_ATTEMPTS,
                town_centers,
                &mut industry_origins,
            );
        }
    }
    industry_origins.len()
}

/// Probabilidades escaladas por tamaño de mapa (`GetScaledProbabilities`).
fn generation_probabilities(
    climate: crate::Climate,
    map_w: u32,
    map_h: u32,
    specs: &[IndustrySpec],
    water: bool,
) -> Vec<(&IndustrySpec, u32)> {
    specs
        .iter()
        .filter_map(|spec| {
            // Ningún `IndustrySpec` vanilla del catálogo es `BuiltOnWater`.
            // La condición queda en la función común para que el día que se
            // modele `IT_OIL_RIG` la distribución acuática no contamine la
            // terrestre ni cambie el orden de consumo de RNG.
            if water {
                return None;
            }
            let base = u32::from(spec.map_creation_probability(climate));
            if base == 0 {
                return None;
            }
            let scaled_base = base.saturating_mul(16);
            let scaled = if matches!(spec, IndustrySpec::OilRefinery) {
                scale_by_size_1d(scaled_base, map_w, map_h)
            } else {
                scale_by_size(scaled_base, map_w, map_h)
            };
            (scaled > 0).then_some((spec, scaled))
        })
        .collect()
}

fn weighted_spec(
    rng: &mut crate::cargodist::parity::Randomizer,
    probabilities: &[(&IndustrySpec, u32)],
    total: u64,
) -> IndustrySpec {
    let total = u32::try_from(total).unwrap_or(u32::MAX).max(1);
    let mut roll = rng.random_range(total);
    for (spec, probability) in probabilities {
        if roll < *probability {
            return **spec;
        }
        roll = roll.saturating_sub(*probability);
    }
    probabilities
        .last()
        .map_or(IndustrySpec::CoalMine, |(spec, _)| **spec)
}

fn scale_by_size_1d(value: u32, map_w: u32, map_h: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    let log_x = map_w.max(1).ilog2();
    let log_y = map_h.max(1).ilog2();
    super::ceil_div((value << log_x).saturating_add(value << log_y), 1 << 9)
}

/// Distancia máxima de `CheckIfFarEnoughFromConflictingIndustry`.
const INDUSTRY_CONFLICT_DISTANCE: u32 = 14;

/// `CheckIfFarEnoughFromConflictingIndustry` de `industry_cmd.cpp`.
///
/// La distancia nativa es `DistanceMax`, no la distancia Manhattan usada para
/// asociar el pueblo. Sólo se inspeccionan las especies enumeradas por el
/// `IndustrySpec` nuevo; dos tipos que no son conflicto explícito pueden
/// compartir ubicación cercana si sus huellas no se superponen.
fn generated_industry_has_conflict(
    state: &GameState,
    origin: TileCoord,
    spec: IndustrySpec,
) -> bool {
    state.industries.iter().any(|industry| {
        industry
            .spec
            .is_some_and(|existing| spec.conflicting_specs().contains(&existing))
            && origin
                .x
                .abs_diff(industry.pos.x)
                .max(origin.y.abs_diff(industry.pos.y))
                <= INDUSTRY_CONFLICT_DISTANCE
    })
}

/// `CalcClosestTownFromTile` para una tesela clear durante `GenerateWorld`.
///
/// En esta fase la tesela candidata aún no es una casa ni una carretera, por
/// lo que `ClosestTownFromTile` delega en `CalcClosestTownFromTile`. Éste usa
/// distancia Manhattan; conservar el primer pueblo ante un empate mantiene el
/// orden del pool que construyó `GenerateTowns`.
fn generated_industry_closest_town_id(state: &GameState, origin: TileCoord) -> Option<u32> {
    state
        .towns
        .iter()
        .min_by_key(|town| crate::economy::manhattan_distance(origin, town.pos))
        .map(|town| town.id)
}

/// `FindTownForIndustry` de `industry_cmd.cpp` para la creación procedural.
///
/// `OpenTTD` persiste el puntero al pueblo en la industria. El modelo Rust aún
/// no conserva esa columna, pero durante `GenerateIndustries` los pueblos no
/// cambian: volver a obtener el pueblo más cercano de cada industria existente
/// es equivalente y preserva el ajuste `multiple_industry_per_town`.
fn generated_industry_can_use_closest_town(
    state: &GameState,
    origin: TileCoord,
    spec: IndustrySpec,
    multiple_industry_per_town: bool,
) -> bool {
    let Some(town_id) = generated_industry_closest_town_id(state, origin) else {
        // El original asume que `GenerateTowns` ya dejó al menos un pueblo.
        // Rechazar mantiene esta API total para fixtures y mapas inválidos.
        return false;
    };
    if multiple_industry_per_town {
        return true;
    }
    !state.industries.iter().any(|industry| {
        industry.spec == Some(spec)
            && generated_industry_closest_town_id(state, industry.pos) == Some(town_id)
    })
}

fn generated_industry_has_vehicle(state: &GameState, layout: &[(TileCoord, u8)]) -> bool {
    layout
        .iter()
        .any(|(tile, _)| state.vehicles.iter().any(|vehicle| vehicle.pos == *tile))
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
    map: &Map,
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
    platform: u8,
) -> Option<Map> {
    if !generated_industry_platform_is_valid(map, origin, spec, layout_index, platform) {
        return None;
    }
    // `CheckIfCanLevelIndustryPlatform` has a test pass and an execute pass.
    // Keep the execute pass isolated until every terraform/clear operation has
    // succeeded; a failed command must not leave a half-levelled map behind.
    let mut candidate = map.clone();
    let (start_x, start_y, end_x, end_y) =
        generated_industry_platform_area(&candidate, origin, spec, layout_index, platform)?;
    let target_height = candidate.get(origin).map(|tile| tile.height)?;
    for y in start_y..end_y {
        for x in start_x..end_x {
            let c = TileCoord::new(x, y);
            loop {
                let current_height = candidate.get(c).map(|tile| tile.height)?;
                if current_height == target_height {
                    break;
                }
                let step = simulate_generated_terraform_north_corner(
                    &candidate,
                    c,
                    current_height <= target_height,
                )?;
                if !step
                    .dirty_tiles
                    .iter()
                    .copied()
                    .all(|dirty| clear_generated_industry_platform_tile(&mut candidate, dirty))
                {
                    return None;
                }
                for (height_x, height_y, height) in step.heights {
                    if candidate
                        .set_height(TileCoord::new(height_x, height_y), height)
                        .is_err()
                    {
                        return None;
                    }
                }
            }
        }
    }
    Some(candidate)
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
        // `CreateNewIndustryHelper` ejecuta primero conflictos explícitos y
        // `FindTownForIndustry`; no sustituirlos por una distancia genérica
        // evita descartar la mina nativa `(21,41)` de RMAP-061 y, a la vez,
        // rechaza la central demasiado cerca de ella en RMAP-062.
        if generated_industry_has_conflict(ctx.state, origin, spec)
            || !generated_industry_can_use_closest_town(
                ctx.state,
                origin,
                spec,
                ctx.multiple_industry_per_town,
            )
        {
            continue;
        }
        // `EnsureNoVehicleOnGround` runs before the clear pass in
        // `CreateNewIndustryHelper`. The procedural model stores one tile
        // position per vehicle, which is sufficient to reject a footprint
        // occupied by a moving or stopped vehicle without consuming another
        // random draw.
        let Some(layout) = industry_template_with_layout(origin, spec, attempt.layout_index) else {
            continue;
        };
        if generated_industry_has_vehicle(ctx.state, &layout) {
            continue;
        }
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
        let Some(leveled_map) = level_generated_industry_platform(
            &ctx.state.map,
            origin,
            spec,
            attempt.layout_index,
            ctx.industry_platform,
        ) else {
            continue;
        };
        let original_map = std::mem::replace(&mut ctx.state.map, leveled_map);
        if apply_command(
            ctx.state,
            &Command::PlaceIndustrySpecLayout(origin, spec, layout_index),
        )
        .is_err()
        {
            // The command layer is intentionally allowed to reject a stale
            // candidate (for example when a pool limit is reached). Restore
            // the pre-platform map just like OpenTTD's command transaction.
            ctx.state.map = original_map;
            continue;
        }
        // La cola de `DoCreateNewIndustry` consume producción smooth,
        // color/counter, `MakeIndustry` y triggers de construcción. No
        // pertenece al intento RMAP-058: ocurre sólo después de que el sitio
        // fue aceptado y determina el primer `RandomTile` de la especie
        // force-one siguiente. Los valores que escribe `MakeIndustry` se
        // aplican después de la orden semántica, manteniendo la ruta de
        // construcción manual separada de la que se ejecuta durante
        // `GenerateIndustries`.
        let constructor_random =
            consume_successful_industry_constructor_rng(ctx.rng, spec, attempt.layout_index);
        apply_generated_industry_bytes(
            ctx.state,
            origin,
            spec,
            attempt.layout_index,
            &constructor_random,
        );
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
            plant_farm_fields(ctx, origin, spec, attempt.layout_index, industry_id);
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
#[derive(Debug, Clone, PartialEq, Eq)]
struct IndustryConstructorRandom {
    random_colour: u8,
    counter: u16,
    tile_random: Vec<u8>,
}

fn consume_successful_industry_constructor_rng(
    rng: &mut crate::cargodist::parity::Randomizer,
    spec: IndustrySpec,
    layout_index: usize,
) -> IndustryConstructorRandom {
    for _ in spec.produced_cargos() {
        let _initial_smooth_economy_rate = rng.next();
    }
    let random_colour_and_counter = rng.next();
    let random_colour = u8::try_from(random_colour_and_counter & 0x0F).unwrap_or(0);
    let counter = u16::try_from((random_colour_and_counter >> 4) & 0x0FFF).unwrap_or(0);
    let tile_count = industry_template_with_layout(TileCoord::new(0, 0), spec, layout_index)
        .map_or(0, |layout| layout.len());
    let tile_random = (0..tile_count)
        .map(|_| u8::try_from(rng.next() & 0xFF).unwrap_or(0))
        .collect();
    for _ in 0..tile_count {
        let _construction_stage_changed_random = rng.next();
    }
    IndustryConstructorRandom {
        random_colour,
        counter,
        tile_random,
    }
}

/// Completa la escritura cruda de `DoCreateNewIndustry` durante la generación.
///
/// `MakeIndustry` deja `m1=WaterClass` y, bajo `_generating_world`, el caller
/// escribe inmediatamente `counter=3`/`stage=2`. Cada tesela recibe además el
/// byte bajo de su propio `Random()`. La orden pública conserva sus bytes
/// deterministas para no cambiar el comportamiento de fundación manual.
fn apply_generated_industry_bytes(
    state: &mut GameState,
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
    random: &IndustryConstructorRandom,
) {
    let Some(layout) = industry_template_with_layout(origin, spec, layout_index) else {
        return;
    };
    for ((coord, _), tile_random) in layout.iter().zip(random.tile_random.iter().copied()) {
        let Some(mut tile) = state.map.get(*coord) else {
            continue;
        };
        // Preserve bits 5..6 (`WaterClass`) while writing stage 2 and counter
        // 3, yielding 0x6E for a land industry (`MakeIndustry` + generation).
        tile.m1 = (tile.m1 & !0x0F) | 0x0E;
        // `MakeIndustry` resets m4 (MAP4, represented by `m3hi`) even when
        // the platform clear left a fence/animation byte on the source tile.
        tile.m3hi = 0;
        tile.m3 = tile_random;
        let _ = state.map.set_tile(*coord, tile);
    }
    if let Some(industry) = state
        .industries
        .iter_mut()
        .find(|industry| industry.pos == origin && industry.spec == Some(spec))
    {
        industry.random_colour = random.random_colour;
        industry.counter = random.counter;
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
const TREE_GROUND_ROUGH: u8 = 1;
const TREE_GROUND_SHORE: u8 = 3;
const FARM_FIELD_FENCE_TYPES: [u8; 16] = [1, 1, 1, 1, 1, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6];

/// Lados diagonales de una tesela, con la codificación de `GetFence` / `SetFence`.
///
/// La estructura cruda de `MP_CLEAR` reparte las cuatro cercas entre `m3`,
/// `m4` (nuestro `m3hi`) y `m6`. Mantener la asociación acá evita compensar
/// el RNG sin reproducir la geometría que decide los `Chance16(1, 7)`.
#[derive(Debug, Clone, Copy)]
enum FarmFenceSide {
    Ne,
    Se,
    Sw,
    Nw,
}

impl FarmFenceSide {
    const fn reverse(self) -> Self {
        match self {
            Self::Ne => Self::Sw,
            Self::Se => Self::Nw,
            Self::Sw => Self::Ne,
            Self::Nw => Self::Se,
        }
    }

    const fn neighbour_delta(self) -> (i32, i32) {
        match self {
            Self::Ne => (-1, 0),
            Self::Se => (0, 1),
            Self::Sw => (1, 0),
            Self::Nw => (0, -1),
        }
    }

    /// `TileOffsByAxis(OtherAxis(DiagDirToAxis(side)))`.
    const fn sweep_delta(self) -> (i32, i32) {
        match self {
            Self::Ne | Self::Sw => (0, 1),
            Self::Se | Self::Nw => (1, 0),
        }
    }

    const fn fence(self, tile: Tile) -> u8 {
        match self {
            Self::Se => (tile.m3hi >> 2) & 0x07,
            Self::Sw => (tile.m3hi >> 5) & 0x07,
            Self::Ne => (tile.m3 >> 5) & 0x07,
            Self::Nw => (tile.m6 >> 2) & 0x07,
        }
    }

    fn set_fence(self, tile: &mut Tile, value: u8) {
        let value = value & 0x07;
        match self {
            Self::Se => tile.m3hi = (tile.m3hi & !(0x07 << 2)) | (value << 2),
            Self::Sw => tile.m3hi = (tile.m3hi & !(0x07 << 5)) | (value << 5),
            Self::Ne => tile.m3 = (tile.m3 & !(0x07 << 5)) | (value << 5),
            Self::Nw => tile.m6 = (tile.m6 & !(0x07 << 2)) | (value << 2),
        }
    }
}

fn is_farm_field(tile: Tile) -> bool {
    tile.kind == TileKind::Grass && clear_ground_type(tile.m5) == CLEAR_GROUND_FIELDS
}

fn farm_field_suitable(tile: crate::map::Tile, allow_fields: bool, allow_rough: bool) -> bool {
    match tile.kind {
        TileKind::Grass => {
            // `IsSnowTile` usa MAP3 bit 4. `CLEAR_GROUND_SNOW` se conserva
            // también porque la representación procedural heredada lo usa.
            if tile.m3 & 0x10 != 0 {
                return false;
            }
            match clear_ground_type(tile.m5) {
                CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => false,
                CLEAR_GROUND_ROUGH => allow_rough,
                CLEAR_GROUND_FIELDS => allow_fields,
                _ => true,
            }
        }
        // OpenTTD permits ordinary trees as a field substrate, but not shore
        // trees. `tree_ground` is stored in m2 bits 6..8 for MP_TREES.
        TileKind::Forest => {
            let ground = (tile.m2 >> 6) & 0x07;
            ground != TREE_GROUND_SHORE && (allow_rough || ground != TREE_GROUND_ROUGH)
        }
        _ => false,
    }
}

/// `TileAddWrap` de los campos de granja con `freeform_edges` habilitado.
///
/// El generador procedural siempre materializa el borde void. Por eso las
/// coordenadas 0 y `size - 1` no son centros válidos, aunque el área posterior
/// sí puede tocarlas y simplemente las descarta por no ser `MP_CLEAR`.
fn farm_field_tile_add_wrap(
    origin: TileCoord,
    dx: i32,
    dy: i32,
    map_w: i32,
    map_h: i32,
) -> Option<TileCoord> {
    let x = origin.x.saturating_add(dx);
    let y = origin.y.saturating_add(dy);
    let max_x = map_w.saturating_sub(1);
    let max_y = map_h.saturating_sub(1);
    if x <= 0 || y <= 0 || x >= max_x || y >= max_y {
        None
    } else {
        Some(TileCoord::new(x, y))
    }
}

fn farm_industry_location_size(
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
) -> (i32, i32) {
    let Some(layout) = industry_template_with_layout(origin, spec, layout_index) else {
        return (1, 1);
    };
    let width = layout
        .iter()
        .map(|(tile, _)| tile.x.saturating_sub(origin.x))
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let height = layout
        .iter()
        .map(|(tile, _)| tile.y.saturating_sub(origin.y))
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    (width, height)
}

fn make_farm_field(mut field: Tile, field_type: u8, counter: u8, industry_id: u8) -> Tile {
    // `MakeField`: `SetTileType` conserva el nibble bajo de MAPT, mientras
    // que los bytes de clear se reinicializan exactamente como la rutina C++.
    field.kind = TileKind::Grass;
    field.mapt &= 0x0F;
    field.m1 = OWNER_NONE_M1;
    field.m2 = industry_id;
    field.m2_hi = 0;
    field.m3 = field_type;
    field.m3hi = 0;
    field.m5 = with_clear_counter(clear_ground_m5(CLEAR_GROUND_FIELDS, 3), counter);
    field.m6 &= 0x03;
    field.m7 = 0;
    field.m8 = 0;
    field
}

/// `Chance16(1, 7)`: toma la palabra RNG sólo cuando se puede instalar cerca.
fn farm_fence_becomes_type_two(rng: &mut crate::cargodist::parity::Randomizer) -> bool {
    let low = u32::from(rng.next() as u16);
    ((low * 7 + 3) >> 16) < 1
}

/// `SetupFarmFieldFence` de `industry_cmd.cpp`.
fn setup_farm_field_fence(
    ctx: &mut PopCtx<'_>,
    start: TileCoord,
    span: i32,
    fence_type: u8,
    side: FarmFenceSide,
) {
    let mut current = start;
    let (sweep_x, sweep_y) = side.sweep_delta();
    let (neighbour_x, neighbour_y) = side.neighbour_delta();
    for _ in 0..span.max(0) {
        let Some(tile) = ctx.state.map.get(current) else {
            current = TileCoord::new(
                current.x.saturating_add(sweep_x),
                current.y.saturating_add(sweep_y),
            );
            continue;
        };
        if is_farm_field(tile) {
            let neighbour = TileCoord::new(
                current.x.saturating_add(neighbour_x),
                current.y.saturating_add(neighbour_y),
            );
            let neighbour_has_matching_fence = ctx
                .state
                .map
                .get(neighbour)
                .is_some_and(|tile| is_farm_field(tile) && side.reverse().fence(tile) != 0);
            if !neighbour_has_matching_fence {
                let actual_type = if fence_type == 1 && farm_fence_becomes_type_two(ctx.rng) {
                    2
                } else {
                    fence_type
                };
                let mut field = tile;
                side.set_fence(&mut field, actual_type);
                let _ = ctx.state.map.set_tile(current, field);
            }
        }
        current = TileCoord::new(
            current.x.saturating_add(sweep_x),
            current.y.saturating_add(sweep_y),
        );
    }
}

/// `PlantFarmField`: una vez elegido un centro válido, el tamaño se consume
/// antes de saber si el rectángulo tiene suficientes teselas aptas.
fn plant_farm_field(ctx: &mut PopCtx<'_>, center: TileCoord, industry_id: u8) {
    let map_w = i32::try_from(ctx.mw).unwrap_or(i32::MAX);
    let map_h = i32::try_from(ctx.mh).unwrap_or(i32::MAX);
    if map_w == 0 || map_h == 0 {
        return;
    }
    let mut size_random = (ctx.rng.next() & 0x303).wrapping_add(0x404);
    if matches!(ctx.state.climate, crate::Climate::SubArctic) {
        size_random = size_random.wrapping_add(0x404);
    }
    let size_x = i32::try_from(size_random & 0xFF).unwrap_or(4).max(1);
    let size_y = i32::try_from((size_random >> 8) & 0xFF).unwrap_or(4).max(1);
    let min_x = center.x.saturating_sub(center.x.min(size_x / 2));
    let min_y = center.y.saturating_sub(center.y.min(size_y / 2));
    let max_x = min_x.saturating_add(size_x).min(map_w);
    let max_y = min_y.saturating_add(size_y).min(map_h);
    if max_x <= min_x || max_y <= min_y {
        return;
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
        return;
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
            let _ = ctx
                .state
                .map
                .set_tile(c, make_farm_field(tile, field_type, counter, industry_id));
        }
    }

    let fence_type = if matches!(
        ctx.state.climate,
        crate::Climate::SubArctic | crate::Climate::SubTropical
    ) {
        3
    } else {
        FARM_FIELD_FENCE_TYPES[usize::try_from(ctx.rng.next() & 0x0F).unwrap_or(0)]
    };
    setup_farm_field_fence(
        ctx,
        TileCoord::new(min_x, min_y),
        max_y.saturating_sub(min_y),
        fence_type,
        FarmFenceSide::Ne,
    );
    setup_farm_field_fence(
        ctx,
        TileCoord::new(min_x, min_y),
        max_x.saturating_sub(min_x),
        fence_type,
        FarmFenceSide::Nw,
    );
    setup_farm_field_fence(
        ctx,
        TileCoord::new(max_x.saturating_sub(1), min_y),
        max_y.saturating_sub(min_y),
        fence_type,
        FarmFenceSide::Sw,
    );
    setup_farm_field_fence(
        ctx,
        TileCoord::new(min_x, max_y.saturating_sub(1)),
        max_x.saturating_sub(min_x),
        fence_type,
        FarmFenceSide::Se,
    );
}

/// `PlantRandomFarmField`: las coordenadas se sortean antes del tamaño.
fn plant_farm_fields(
    ctx: &mut PopCtx<'_>,
    origin: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
    industry_id: u8,
) {
    let map_w = i32::try_from(ctx.mw).unwrap_or(i32::MAX);
    let map_h = i32::try_from(ctx.mh).unwrap_or(i32::MAX);
    let (location_w, location_h) = farm_industry_location_size(origin, spec, layout_index);
    for _ in 0..FARM_FIELD_ATTEMPTS {
        let dx = location_w / 2 + i32::try_from(ctx.rng.next() % 31).unwrap_or(0) - 16;
        let dy = location_h / 2 + i32::try_from(ctx.rng.next() % 31).unwrap_or(0) - 16;
        let Some(center) = farm_field_tile_add_wrap(origin, dx, dy, map_w, map_h) else {
            continue;
        };
        plant_farm_field(ctx, center, industry_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargodist::parity::Randomizer;
    use crate::game_state::GameState;
    use crate::industry::{Industry, IndustryKind};
    use crate::map::{Map, tree_tile_loop::clear_density};
    use crate::town::Town;
    use crate::vehicle::{Vehicle, VehicleKind};
    use crate::world_gen::{
        Climate, PopulationGenConfig, TerrainType, WorldGenConfig, apply_world_gen_with_rng,
        generate_towns_with_rng,
    };

    fn generated_towns_state_and_rng(seed: u64) -> (GameState, Randomizer) {
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
        (state, rng)
    }

    fn generated_towns_state_for_platform(seed: u64) -> GameState {
        generated_towns_state_and_rng(seed).0
    }

    #[test]
    fn generation_probabilities_scale_like_openttd() {
        let specs = IndustrySpec::specs_for_climate(Climate::Temperate);
        let small = generation_probabilities(Climate::Temperate, 64, 64, specs, false);
        assert_eq!(
            small,
            vec![
                (&IndustrySpec::CoalMine, 8),
                (&IndustrySpec::PowerStation, 5),
                (&IndustrySpec::Sawmill, 5),
                (&IndustrySpec::Forest, 5),
                (&IndustrySpec::OilRefinery, 16),
                (&IndustrySpec::Factory, 5),
                (&IndustrySpec::SteelMill, 5),
                (&IndustrySpec::Farm, 9),
                (&IndustrySpec::OilWells, 4),
                (&IndustrySpec::IronOreMine, 5),
            ]
        );

        let native_size = generation_probabilities(Climate::Temperate, 256, 256, specs, false);
        assert_eq!(
            native_size,
            vec![
                (&IndustrySpec::CoalMine, 128),
                (&IndustrySpec::PowerStation, 80),
                (&IndustrySpec::Sawmill, 80),
                (&IndustrySpec::Forest, 80),
                (&IndustrySpec::OilRefinery, 64),
                (&IndustrySpec::Factory, 80),
                (&IndustrySpec::SteelMill, 80),
                (&IndustrySpec::Farm, 144),
                (&IndustrySpec::OilWells, 64),
                (&IndustrySpec::IronOreMine, 80),
            ]
        );

        // El roster vanilla actual no tiene una industria `BuiltOnWater`; la
        // pasada acuática debe quedar vacía y no consumir RNG por accidente.
        assert!(generation_probabilities(Climate::Temperate, 64, 64, specs, true).is_empty());
    }

    #[test]
    fn weighted_industry_selection_uses_relative_probability() {
        let probabilities = vec![
            (&IndustrySpec::CoalMine, 8),
            (&IndustrySpec::PowerStation, 2),
        ];
        let mut rng = Randomizer { state: [1, 1] };
        assert_eq!(
            weighted_spec(&mut rng, &probabilities, 10),
            IndustrySpec::CoalMine
        );

        let mut rng = Randomizer {
            state: [u32::MAX, u32::MAX],
        };
        assert_eq!(
            weighted_spec(&mut rng, &probabilities, 10),
            IndustrySpec::PowerStation
        );
    }

    #[test]
    fn industry_admission_rejects_a_vehicle_on_the_footprint() {
        let origin = TileCoord::new(4, 4);
        let mut state = GameState::new(16, 16);
        let layout = industry_template_with_layout(origin, IndustrySpec::CoalMine, 0)
            .expect("native layout");
        state
            .vehicles
            .push(Vehicle::new(1, VehicleKind::Train, layout[0].0, origin));
        assert!(generated_industry_has_vehicle(&state, &layout));

        state.vehicles[0].pos = TileCoord::new(15, 15);
        assert!(!generated_industry_has_vehicle(&state, &layout));
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
            multiple_industry_per_town: false,
        };
        plant_farm_fields(&mut ctx, TileCoord::new(32, 32), IndustrySpec::Farm, 0, 7);

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
                && tile.m2_hi == 0
                && clear_density(tile.m5) == 3
                && (tile.m3 & 0x0F) <= 8
                && tile.m1 == OWNER_NONE_M1
        }));
    }

    #[test]
    fn farm_field_suitability_matches_clear_and_tree_rules() {
        let mut tile = GameState::new(2, 2)
            .map
            .get(TileCoord::new(0, 0))
            .expect("flat tile");
        tile.m5 = clear_ground_m5(CLEAR_GROUND_ROUGH, 3);
        assert!(!farm_field_suitable(tile, false, false));
        assert!(farm_field_suitable(tile, false, true));

        tile.m5 = clear_ground_m5(crate::world_gen::CLEAR_GROUND_ROCKY, 3);
        assert!(farm_field_suitable(tile, false, false));

        tile.kind = TileKind::Forest;
        tile.m2 = 0;
        assert!(farm_field_suitable(tile, false, false));
        tile.m2 = TREE_GROUND_ROUGH << 6;
        assert!(!farm_field_suitable(tile, false, false));
        assert!(farm_field_suitable(tile, false, true));
        tile.m2 = TREE_GROUND_SHORE << 6;
        assert!(!farm_field_suitable(tile, true, true));
    }

    #[test]
    fn farm_fence_bytes_follow_the_four_native_slots() {
        let mut tile = GameState::new(2, 2)
            .map
            .get(TileCoord::new(0, 0))
            .expect("flat tile");
        tile.m5 = clear_ground_m5(CLEAR_GROUND_FIELDS, 3);
        FarmFenceSide::Ne.set_fence(&mut tile, 1);
        FarmFenceSide::Se.set_fence(&mut tile, 2);
        FarmFenceSide::Sw.set_fence(&mut tile, 3);
        FarmFenceSide::Nw.set_fence(&mut tile, 4);
        assert_eq!(FarmFenceSide::Ne.fence(tile), 1);
        assert_eq!(FarmFenceSide::Se.fence(tile), 2);
        assert_eq!(FarmFenceSide::Sw.fence(tile), 3);
        assert_eq!(FarmFenceSide::Nw.fence(tile), 4);
    }

    fn assert_force_one_origins_match_reference(seed: u64, expected: &[TileCoord]) {
        let (mut state, mut rng) = generated_towns_state_and_rng(seed);
        let town_centers: Vec<_> = state.towns.iter().map(|town| town.pos).collect();
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
            multiple_industry_per_town: false,
        };
        let mut origins = Vec::new();
        let specs = IndustrySpec::temperate_map_creation_force_one();
        assert_eq!(specs.len(), expected.len());

        for (&spec, &expected_origin) in specs.iter().zip(expected) {
            assert!(try_place_industry(
                &mut ctx,
                spec,
                FORCED_INDUSTRY_PLACEMENT_ATTEMPTS,
                &town_centers,
                &mut origins,
            ));
            assert_eq!(origins.last(), Some(&expected_origin), "{spec:?}");
        }
    }

    #[test]
    fn farm_rng_keeps_force_one_origins_for_seed_1330935378() {
        // Traza GDB de `DoCreateNewIndustry`: tras los 50
        // `PlantRandomFarmField`, OilWells vuelve a aceptar `(40,38)`, y la
        // mina de hierro posterior queda en `(17,18)`.
        assert_force_one_origins_match_reference(
            1_330_935_378,
            &[
                TileCoord::new(21, 41),
                TileCoord::new(42, 39),
                TileCoord::new(32, 16),
                TileCoord::new(38, 54),
                TileCoord::new(27, 19),
                TileCoord::new(48, 47),
                TileCoord::new(30, 56),
                TileCoord::new(14, 14),
                TileCoord::new(40, 38),
                TileCoord::new(17, 18),
            ],
        );
    }

    #[test]
    fn farm_rng_keeps_force_one_origins_for_seed_1330935379() {
        // Misma traza independiente: no sólo se conserva la cuenta de RNG,
        // sino las ramas condicionales de cerca sobre un segundo relieve.
        assert_force_one_origins_match_reference(
            1_330_935_379,
            &[
                TileCoord::new(22, 20),
                TileCoord::new(39, 26),
                TileCoord::new(9, 54),
                TileCoord::new(6, 31),
                TileCoord::new(5, 36),
                TileCoord::new(37, 48),
                TileCoord::new(47, 11),
                TileCoord::new(21, 12),
                TileCoord::new(27, 4),
                TileCoord::new(23, 36),
            ],
        );
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
    fn generated_constructor_writes_native_industry_tile_bytes() {
        let (mut state, mut rng) = generated_towns_state_and_rng(1_330_935_378);
        let town_centers: Vec<_> = state.towns.iter().map(|town| town.pos).collect();
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
            multiple_industry_per_town: false,
        };
        let mut origins = Vec::new();
        assert!(try_place_industry(
            &mut ctx,
            IndustrySpec::CoalMine,
            FORCED_INDUSTRY_PLACEMENT_ATTEMPTS,
            &town_centers,
            &mut origins,
        ));
        assert_eq!(origins, [TileCoord::new(21, 41)]);

        let industry = ctx.state.industries.last().expect("created industry");
        assert_eq!(industry.instance_id, 0);
        assert_eq!(industry.random_colour, 13);
        assert_eq!(industry.counter, 1_347);
        let expected_random = [0x61, 0xE1, 0xBD, 0x41, 0x1E, 0x2E, 0xDD, 0xD7, 0x09, 0x57];
        let layout = industry_template_with_layout(industry.pos, IndustrySpec::CoalMine, 3)
            .expect("native coal layout");
        assert_eq!(layout.len(), expected_random.len());
        for ((coord, _), expected) in layout.iter().zip(expected_random) {
            let tile = ctx.state.map.get(*coord).expect("industry tile");
            assert_eq!(tile.m1, 0x6E, "native stage/counter at {coord:?}");
            assert_eq!(tile.m3, expected, "native MakeIndustry random at {coord:?}");
            assert_eq!(tile.m3hi, 0);
            assert_eq!(tile.m7, 0);
            assert_eq!(tile.m8, 0);
        }
        // The ten ConstructionStageChanged callbacks are consumed after the
        // tile writes, preserving the next force-one RandomTile boundary.
        assert_eq!(ctx.rng.state, [2_354_350_958, 520_419_394]);
    }

    #[test]
    fn generated_constructor_resets_map4_before_make_industry() {
        let mut state = GameState::new(64, 64);
        let origin = TileCoord::new(20, 20);
        let layout = industry_template_with_layout(origin, IndustrySpec::CoalMine, 0)
            .expect("native coal layout");
        for (coord, _) in &layout {
            let mut tile = state.map.get(*coord).expect("layout tile");
            tile.m3hi = 0xFF;
            state.map.set_tile(*coord, tile).expect("seed map4");
        }
        let random = IndustryConstructorRandom {
            random_colour: 0,
            counter: 0,
            tile_random: vec![0x2A; layout.len()],
        };

        apply_generated_industry_bytes(&mut state, origin, IndustrySpec::CoalMine, 0, &random);

        for (coord, _) in layout {
            assert_eq!(state.map.get(coord).expect("industry tile").m3hi, 0);
        }
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

    #[test]
    fn industry_platform_execute_is_transactional() {
        let state = generated_towns_state_for_platform(1_330_935_378);
        let before = state.map.tiles().to_vec();
        let candidate = level_generated_industry_platform(
            &state.map,
            TileCoord::new(21, 41),
            IndustrySpec::CoalMine,
            3,
            1,
        )
        .expect("accepted platform");

        // The test pass accepts an immutable map; execute writes only to the
        // returned candidate, so a later command failure can restore the
        // original without leaving partial terraform behind.
        assert_eq!(state.map.tiles(), before.as_slice());
        assert_eq!(candidate.dimensions(), state.map.dimensions());
    }

    #[test]
    fn conflicting_industries_use_native_distance_max_boundary() {
        let mut state = GameState::new(64, 64);
        state.industries.push(Industry::with_tiles_spec(
            TileCoord::new(20, 20),
            IndustryKind::CoalMine,
            IndustrySpec::CoalMine,
            vec![TileCoord::new(20, 20)],
            0,
        ));

        assert!(generated_industry_has_conflict(
            &state,
            TileCoord::new(34, 34),
            IndustrySpec::PowerStation,
        ));
        assert!(!generated_industry_has_conflict(
            &state,
            TileCoord::new(35, 34),
            IndustrySpec::PowerStation,
        ));
        assert!(!generated_industry_has_conflict(
            &state,
            TileCoord::new(20, 20),
            IndustrySpec::Sawmill,
        ));
    }

    #[test]
    fn same_type_is_limited_to_the_closest_town_by_default() {
        let mut state = GameState::new(64, 64);
        state.towns.push(Town {
            id: 7,
            pos: TileCoord::new(10, 10),
            ..Town::default()
        });
        state.industries.push(Industry::with_tiles_spec(
            TileCoord::new(12, 10),
            IndustryKind::CoalMine,
            IndustrySpec::CoalMine,
            vec![TileCoord::new(12, 10)],
            0,
        ));

        assert!(!generated_industry_can_use_closest_town(
            &state,
            TileCoord::new(8, 10),
            IndustrySpec::CoalMine,
            false,
        ));
        assert!(generated_industry_can_use_closest_town(
            &state,
            TileCoord::new(8, 10),
            IndustrySpec::CoalMine,
            true,
        ));
        assert!(generated_industry_can_use_closest_town(
            &state,
            TileCoord::new(8, 10),
            IndustrySpec::PowerStation,
            false,
        ));
    }

    #[test]
    fn power_station_skips_the_conflicting_reference_site_after_coal() {
        // La frontera de industrias para 1330935378 se toma después de towns.
        // OpenTTD descarta seis intentos de PowerStation y el séptimo sitio
        // relevante `(23,49)` por estar a DistMax=8 de la CoalMine creada en
        // `(21,41)`; el siguiente sitio admitido es `(42,39)`.
        let mut state = generated_towns_state_for_platform(1_330_935_378);
        let town_centers: Vec<_> = state.towns.iter().map(|town| town.pos).collect();
        let mut rng = Randomizer {
            state: [11_204_508, 1_784_072_412],
        };
        let mut origins = Vec::new();
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
            multiple_industry_per_town: false,
        };

        assert!(try_place_industry(
            &mut ctx,
            IndustrySpec::CoalMine,
            FORCED_INDUSTRY_PLACEMENT_ATTEMPTS,
            &town_centers,
            &mut origins,
        ));
        assert_eq!(origins, [TileCoord::new(21, 41)]);

        assert!(try_place_industry(
            &mut ctx,
            IndustrySpec::PowerStation,
            FORCED_INDUSTRY_PLACEMENT_ATTEMPTS,
            &town_centers,
            &mut origins,
        ));
        assert_eq!(origins, [TileCoord::new(21, 41), TileCoord::new(42, 39)]);
        assert_eq!(
            ctx.state
                .industries
                .iter()
                .map(|industry| (industry.spec, industry.pos))
                .collect::<Vec<_>>(),
            vec![
                (Some(IndustrySpec::CoalMine), TileCoord::new(21, 41)),
                (Some(IndustrySpec::PowerStation), TileCoord::new(42, 39)),
            ],
        );
    }
}
