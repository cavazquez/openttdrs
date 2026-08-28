//! Colocación de pueblos con calle y casas 1×1 (MVP de `GenerateTowns`).

use crate::house_spec::{
    BUILDING_FLAG_SIZE_1X2, BUILDING_FLAG_SIZE_2X1, BUILDING_FLAG_SIZE_2X2, HouseSpec,
    climate_zone_mask, get_town_radius_group,
};
use crate::map::tree_tile_loop::clear_ground_type;
use crate::map::{
    SLOPE_NE, SLOPE_NW, SLOPE_STEEP, TOWN_HOUSE_COMPLETED, TileCoord, TileKind, TownHouseFootprint,
    TownHouseSpec, complement_slope, tile_slope_and_z,
};
use crate::sav::house_spec_population;
use crate::town::{Town, TownLayout, update_town_radius};
use crate::town_expand::{
    can_build_house, resolve_town_house_footprint, town_house_tile_max_z,
    town_layout_allows_house_here,
};
use crate::townname::generate_town_name;
use crate::world_gen::CLEAR_GROUND_ROUGH;

use super::{
    PROCEDURAL_HOUSE_STYLE_SPREAD, PopCtx, in_preserve, procedural_house_choices,
    tile_is_flat_grass, tile_ok_for_house,
};

/// Bits de carretera recta (eje X / eje Y).
const ROAD_BITS_AXIS_X: u8 = 0x0A;
const ROAD_BITS_AXIS_Y: u8 = 0x05;
const ROAD_NW: u8 = 0x01;
const ROAD_BITS_N: u8 = 0x09;
const ROAD_BITS_E: u8 = 0x0C;
const ROAD_BITS_S: u8 = 0x06;
const ROAD_BITS_W: u8 = 0x03;
/// `m3` de una calle sin tranvía: owner de tram = `OWNER_TOWN` (none).
const TOWN_ROAD_NO_TRAM_OWNER: u8 = 0xF0;
/// `m8` conserva `INVALID_ROADTYPE` (63) para la capa tram ausente.
const TOWN_ROAD_INVALID_TRAM_TYPE: u16 = 0x0FC0;

/// `CreateRandomTown` prueba veinte ubicaciones antes de abandonar un nombre.
const RANDOM_TOWN_ATTEMPTS: usize = 20;
/// `TownCanBePlacedHere`: distancia mínima a un borde de mapa.
const TOWN_EDGE_DISTANCE: i32 = 12;
/// `IsCloseToTown(tile, 20)` usa distancia Manhattan estrictamente menor.
const TOWN_MIN_DISTANCE: i32 = 20;
/// Doce de las 25 teselas del cuadro 5×5 deben ser construibles.
const TOWN_SURROUNDING_GOAL: usize = 12;
/// El ajuste vanilla por defecto crea una ciudad grande por cada cuatro pueblos.
const DEFAULT_LARGER_TOWNS_INTERVAL: u32 = 4;
/// Último intento de `GenerateTowns` cuando no se pudo crear ninguno.
const RANDOM_TOWN_FALLBACK_ATTEMPTS: usize = 10_000;
const SPIRAL_DIRS: [(i32, i32); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
/// Secuencia acumulativa de `_town_coord_mod` usada por `GrowTown`.
const TOWN_GROWTH_COORD_MOD: [(i32, i32); 13] = [
    (-1, 0),
    (1, 1),
    (1, -1),
    (-1, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -2),
    (-1, -1),
    (-2, 2),
    (2, 2),
    (2, -2),
    (0, 0),
];
/// Valor vanilla de `economy.initial_city_size` en una partida nueva.
const DEFAULT_INITIAL_CITY_SIZE: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreetAxis {
    EastWest,
    NorthSouth,
}

struct StreetTownPlan {
    axis: StreetAxis,
    roads: Vec<TileCoord>,
    houses: Vec<TileCoord>,
    town_pos: TileCoord,
    bootstrap_road: Option<BootstrapRoad>,
}

/// El primer bloque vial que `GrowTown` crea cuando el pueblo aún no tiene red.
#[derive(Clone, Copy)]
struct BootstrapRoad {
    pos: TileCoord,
    bits: u8,
}

/// Estado inicial que `BuildTownHouse` codifica en `MAP3`/`MAP5`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TownHouseConstruction {
    counter: u8,
    stage: u8,
}

/// Resultado de una extracción aceptada de `TryBuildTownHouse` durante la
/// generación inicial.
///
/// La selección conserva el estado que permite auditar reintentos: el pool
/// original, cuántas entradas se probaron y cuál fue la tesela base después de
/// validar una huella multitile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TownHouseCandidate {
    id: u16,
    base: TileCoord,
    random_bits: u8,
    probability_max: u32,
    candidate_count: usize,
    attempts: usize,
}

/// Intenta colocar hasta `target` pueblos; devuelve cuántos se crearon.
pub(super) fn place_towns(
    ctx: &mut PopCtx<'_>,
    target: usize,
    town_centers: &mut Vec<TileCoord>,
) -> usize {
    let before = town_centers.len();
    let map_w = i32::try_from(ctx.mw).unwrap_or(i32::MAX);
    let map_h = i32::try_from(ctx.mh).unwrap_or(i32::MAX);
    // `GenerateTowns` consume este sorteo aunque no se consiga fundar ningún
    // pueblo. La construcción/growth posterior sigue siendo RMAP-024, pero
    // la selección de la semilla de cada intento ya debe arrancar aquí.
    let city_random_offset = ctx.rng.next() % DEFAULT_LARGER_TOWNS_INTERVAL;

    for _ in 0..target {
        let is_city = (city_random_offset
            .saturating_add(u32::try_from(town_centers.len().saturating_sub(before)).unwrap_or(0)))
            % DEFAULT_LARGER_TOWNS_INTERVAL
            == 0;
        // En el set vanilla de nombres `GenerateTownName` obtiene su parte
        // determinista de un único `Random()`. La comprobación de nombres
        // únicos queda pendiente junto con el modelo de crecimiento.
        let name_seed = ctx.rng.next();
        let _ = try_build_random_town_with_mvp_plan(
            ctx,
            town_centers,
            name_seed,
            is_city,
            map_w,
            map_h,
            RANDOM_TOWN_ATTEMPTS,
        );
    }

    // `GenerateTowns` hace un último intento agresivo si no consiguió crear
    // ninguno. Es importante para mapas pequeños e islas: el total inicial es
    // una sugerencia, no una garantía de que los 20 intentos alcancen tierra.
    if town_centers.len() == before {
        let name_seed = ctx.rng.next();
        let _ = try_build_random_town_with_mvp_plan(
            ctx,
            town_centers,
            name_seed,
            true,
            map_w,
            map_h,
            RANDOM_TOWN_FALLBACK_ATTEMPTS,
        );
    }
    town_centers.len().saturating_sub(before)
}

/// Recorre los intentos de `CreateRandomTown` y deja que el constructor MVP
/// rechace una trama que todavía no sabe materializar. `DoCreateTown` nativo
/// también vuelve a intentar cuando una fundación no llega a tener población.
fn try_build_random_town_with_mvp_plan(
    ctx: &mut PopCtx<'_>,
    town_centers: &mut Vec<TileCoord>,
    name_seed: u32,
    is_city: bool,
    map_w: i32,
    map_h: i32,
    attempts: usize,
) -> bool {
    for _ in 0..attempts {
        let Some(center) = next_random_town_site(ctx, town_centers) else {
            continue;
        };
        if build_selected_town_with_mvp_plan(
            ctx,
            town_centers,
            center,
            name_seed,
            is_city,
            map_w,
            map_h,
        ) {
            return true;
        }
    }
    false
}

/// Materializa provisionalmente un sitio que ya superó `CreateRandomTown`.
///
/// La topología/crecimiento de `DoCreateTown` sigue pendiente, pero el plan
/// local no consume sorteos antes de construir las casas y retiene el centro,
/// ID y layout nativos de la fundación.
fn build_selected_town_with_mvp_plan(
    ctx: &mut PopCtx<'_>,
    town_centers: &mut Vec<TileCoord>,
    center: TileCoord,
    name_seed: u32,
    is_city: bool,
    map_w: i32,
    map_h: i32,
) -> bool {
    // `DoCreateTown` toma este sorteo inmediatamente después de seleccionar
    // la fundación, incluso si su crecimiento posterior no llega a poblarla.
    // El contador temporal influye en su radio durante `GrowTown`; la trama
    // MVP aún no modela ese radio, pero debe conservar la frontera RNG.
    let _temporary_house_budget = initial_town_house_budget(ctx.rng, is_city);
    // El plan MVP todavía no es `DoCreateTown`; se deriva de la parte de
    // nombre ya consumida y conserva el centro exacto seleccionado.
    let axis = if name_seed & 1 == 0 {
        StreetAxis::EastWest
    } else {
        StreetAxis::NorthSouth
    };
    let half_len = i32::try_from(2 + ((name_seed >> 1) % 3)).unwrap_or(2);
    let south_row = (name_seed >> 3) % 3 != 0;
    let mut plan = plan_street_town(center, axis, half_len, south_row, map_w, map_h)
        .filter(|candidate| plan_fits_terrain(ctx, candidate))
        .or_else(|| compact_town_plan(ctx, center));
    let Some(mut plan) = plan.take() else {
        return false;
    };

    // Primera iteración de `GrowTown`: todavía no hay calles, por lo que C++
    // busca `_town_coord_mod`, limpia la primera tesela plana y recién ahí
    // toma `GenRandomRoadBits`. Se ejecuta sólo después de aceptar el plan
    // MVP; si éste aún no puede materializar un pueblo, no fingimos haber
    // alcanzado la frontera nativa.
    plan.bootstrap_road = initial_town_growth_bootstrap(&ctx.state.map, center, ctx.rng);
    if !plan_fits_terrain(ctx, &plan) {
        return false;
    }

    let choices = procedural_house_choices();
    if choices.is_empty() {
        return false;
    }
    let town_house_base = (name_seed >> 8) % u32::try_from(choices.len()).unwrap_or(1);
    let town_id = u32::try_from(ctx.state.towns.len()).unwrap_or(u32::MAX);
    let (placed_houses, population) = build_street_town(ctx, &plan, town_house_base, town_id);
    if placed_houses < 3 {
        return false;
    }

    let name = generate_town_name(4, name_seed)
        .unwrap_or_else(|| format!("Pueblo {},{}", center.x, center.y));
    let mut town = Town {
        id: town_id,
        pos: plan.town_pos,
        name,
        population,
        passengers_served: 0,
        mail_served: 0,
        growth_funded: 0,
        num_houses: u16::try_from(placed_houses).unwrap_or(0),
        ..Default::default()
    };
    town.initialize_layout(Some(TownLayout::Original));
    town.init_growth_goals(ctx.state.climate);
    town.init_grow_counter();
    update_town_radius(&mut town);
    ctx.state.towns.push(town);
    town_centers.push(plan.town_pos);
    true
}

/// `TSZ_RANDOM` de `DoCreateTown`, incluido el multiplicador de ciudad.
fn initial_town_house_budget(rng: &mut crate::cargodist::parity::Randomizer, city: bool) -> u32 {
    let houses = (rng.next() & 0x0F).saturating_add(8);
    if city {
        houses.saturating_mul(DEFAULT_INITIAL_CITY_SIZE)
    } else {
        houses
    }
}

/// Primer camino de `GrowTown` cuando aún no hay una calle municipal.
///
/// La suma de offsets es intencional: `_town_coord_mod` se aplica después de
/// cada comprobación, por lo que no es una lista de posiciones absolutas.
fn initial_town_growth_bootstrap(
    map: &crate::map::Map,
    center: TileCoord,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> Option<BootstrapRoad> {
    let mut tile = center;
    for &(dx, dy) in &TOWN_GROWTH_COORD_MOD {
        if can_seed_initial_town_road(map, tile) {
            return Some(BootstrapRoad {
                pos: tile,
                bits: random_town_road_bits(rng),
            });
        }
        tile = TileCoord::new(tile.x + dx, tile.y + dy);
    }
    None
}

/// Predicado de la generación nueva: `CMD_LANDSCAPE_CLEAR` puede despejar
/// clear/trees planos; agua y casas no llegan a `CMD_BUILD_ROAD`.
fn can_seed_initial_town_road(map: &crate::map::Map, tile: TileCoord) -> bool {
    matches!(map.get_kind(tile), Some(TileKind::Grass | TileKind::Forest))
        && tile_slope_and_z(map, tile).is_some_and(|(slope, _)| slope == 0)
}

/// `GenRandomRoadBits`: dos direcciones distintas de la misma palabra RNG.
fn random_town_road_bits(rng: &mut crate::cargodist::parity::Randomizer) -> u8 {
    let random = rng.next();
    let a = u8::try_from(random & 3).unwrap_or(0);
    let mut b = u8::try_from((random >> 8) & 3).unwrap_or(0);
    if a == b {
        b ^= 2;
    }
    (ROAD_NW << a) | (ROAD_NW << b)
}

/// `RandomDiagDir`: a diferencia de [`random_town_road_bits`], el resultado
/// se interpreta como un `DiagDirection` (`NE=0`, `SE=1`, `SW=2`, `NW=3`).
/// Por eso no puede usarse directamente como desplazamiento de `ROAD_NW`.
fn random_town_diag_dir(rng: &mut crate::cargodist::parity::Randomizer) -> u8 {
    u8::try_from(rng.random_range(4)).unwrap_or(0)
}

const fn reverse_town_diag_dir(dir: u8) -> u8 {
    dir.wrapping_add(2) & 3
}

/// `DiagDirToRoadBits` con el orden de `DiagDirection`, inverso al orden de
/// los bits `NW,SW,SE,NE` guardados en una carretera.
const fn town_diag_dir_to_road_bits(dir: u8) -> u8 {
    ROAD_NW << ((3_u8.wrapping_sub(dir & 3)) & 3)
}

fn add_town_diag(tile: TileCoord, dir: u8) -> TileCoord {
    let (dx, dy) = crate::map::diag_dir_offset(dir);
    TileCoord::new(tile.x + dx, tile.y + dy)
}

/// `GrowTownInTile` puede colocar una casa en la esquina interior de una
/// curva. Los desplazamientos son `TileAddByDir`, no `DiagDirection`: una
/// esquina usa ambos ejes del mapa isométrico.
fn generated_town_corner_house_tile(tile: TileCoord, road_bits: u8) -> Option<TileCoord> {
    let (dx, dy) = match road_bits {
        // ROAD_N -> DIR_S, ROAD_S -> DIR_N.
        ROAD_BITS_N => (1, 1),
        ROAD_BITS_S => (-1, -1),
        // ROAD_E -> DIR_W, ROAD_W -> DIR_E.
        ROAD_BITS_E => (1, -1),
        ROAD_BITS_W => (-1, 1),
        _ => return None,
    };
    Some(TileCoord::new(tile.x + dx, tile.y + dy))
}

fn generated_town_road_bits(map: &crate::map::Map, tile: TileCoord) -> u8 {
    map.get(tile)
        .filter(|candidate| candidate.kind == TileKind::Road)
        .map_or(0, |candidate| candidate.m5 & 0x0F)
}

/// Subconjunto seguro de `CanFollowRoad` para el mapa recién generado.
///
/// Estaciones, puentes, túneles y vías todavía no existen durante la primera
/// fundación de la fixture. Se dejan explícitamente para la conexión completa
/// de RMAP-030, pero esta rama sí conserva el orden de selección/reintento de
/// las carreteras municipales sobre terreno y carreteras existentes.
fn generated_can_follow_town_road(map: &crate::map::Map, tile: TileCoord, dir: u8) -> bool {
    let target = add_town_diag(tile, dir);
    match map.get_kind(target) {
        Some(TileKind::Road) => generated_town_road_bits(map, target) != 0,
        Some(TileKind::Grass | TileKind::Forest) => true,
        _ => false,
    }
}

/// `IsNeighbourRoadTile`: evita que `TL_ORIGINAL` dibuje una calle paralela
/// demasiado cerca de la que está intentando extender. La tabla nativa visita
/// ambos laterales desde la tesela candidata y desde una tesela hacia atrás;
/// también exige el bit que apunta hacia el eje central, no cualquier calle.
fn generated_is_neighbour_road_tile(
    map: &crate::map::Map,
    tile: TileCoord,
    dir: u8,
    distance_multiplier: u8,
) -> bool {
    let upper_bound = u32::from(distance_multiplier.saturating_add(1)).saturating_mul(4);
    for pos in 4..upper_bound {
        let steps = i32::try_from(pos / 4).unwrap_or(i32::MAX);
        let side_dir = if pos & 1 != 0 {
            dir.wrapping_add(1) & 3
        } else {
            dir.wrapping_add(3) & 3
        };
        let (dx, dy) = crate::map::diag_dir_offset(side_dir);
        let mut candidate = TileCoord::new(
            tile.x.saturating_add(dx.saturating_mul(steps)),
            tile.y.saturating_add(dy.saturating_mul(steps)),
        );
        if pos & 2 != 0 {
            candidate = add_town_diag(candidate, reverse_town_diag_dir(dir));
        }
        let facing = if pos & 2 != 0 {
            dir
        } else {
            reverse_town_diag_dir(dir)
        };
        if generated_town_road_bits(map, candidate) & town_diag_dir_to_road_bits(facing) != 0 {
            return true;
        }
    }
    false
}

/// Núcleo de `IsRoadAllowedHere` necesario antes de decidir casa o carretera.
///
/// La comprobación de pendiente no es puramente geométrica: si el sentido no
/// coincide, `OpenTTD` consume `Chance16(1, 8)` y, durante generación mundial,
/// puede consumir otro `Chance16(1, 3)` antes de rechazar. Mantener esas
/// palabras es indispensable incluso cuando finalmente se elige una casa.
fn generated_town_road_allowed_here(
    map: &crate::map::Map,
    town: &Town,
    tile: TileCoord,
    dir: u8,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> bool {
    let (width, height) = map.dimensions();
    let max_x = i32::try_from(width).unwrap_or(i32::MAX).saturating_sub(1);
    let max_y = i32::try_from(height).unwrap_or(i32::MAX).saturating_sub(1);
    if tile.x <= 0 || tile.y <= 0 || tile.x >= max_x || tile.y >= max_y {
        return false;
    }
    if !matches!(
        map.get_kind(tile),
        Some(TileKind::Grass | TileKind::Forest | TileKind::Road)
    ) {
        return false;
    }

    let neighbour_distance = if town.layout == TownLayout::Original {
        1
    } else {
        2
    };
    let ret = !generated_is_neighbour_road_tile(map, tile, dir, neighbour_distance);

    let slope = tile_slope_and_z(map, tile).map_or(SLOPE_STEEP, |(slope, _)| slope);
    if slope == 0 {
        return ret;
    }
    let desired_slope = if matches!(dir & 3, 1 | 3) {
        SLOPE_NW
    } else {
        SLOPE_NE
    };
    if slope == desired_slope || slope == complement_slope(desired_slope) {
        return ret;
    }

    // En `GenerateWorld`, la rama de terraformación interna no se ejecuta.
    // Si ambos sorteos aceptan la pendiente, el resultado sigue siendo `ret`:
    // un camino paralelo cercano no se vuelve válido por el azar de pendiente.
    if chance16(rng, 1, 8) && chance16(rng, 1, 3) {
        return ret;
    }
    false
}

/// Subconjunto ejecutable de `LevelTownLand` para la fundación de un pueblo.
///
/// La ruta nativa intenta primero elevar las esquinas bajas de la tesela para
/// dejarla plana (`TerraformTownTile(..., true)`). En una tesela de terreno
/// recién generado sin agua ni casas esto equivale a llevar sus cuatro
/// vértices a la altura mayor. Mantener la mutación, y no sólo el sorteo que
/// la decide, es importante: la comprobación vial posterior consulta la nueva
/// pendiente de la misma tesela.
fn level_generated_town_land(map: &mut crate::map::Map, tile: TileCoord) -> bool {
    let corners = [
        tile,
        TileCoord::new(tile.x + 1, tile.y),
        TileCoord::new(tile.x, tile.y + 1),
        TileCoord::new(tile.x + 1, tile.y + 1),
    ];
    let mut highest = 0_u8;
    for corner in corners {
        let Some(current) = map.get(corner) else {
            return false;
        };
        if matches!(
            current.kind,
            TileKind::House | TileKind::Water | TileKind::Void
        ) {
            return false;
        }
        highest = highest.max(current.height);
    }

    for corner in corners {
        if map.set_height(corner, highest).is_err() {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GeneratedRoadGrowthResult {
    Road(TileCoord),
    House(TileCoord),
    Continue,
    SearchStopped,
}

/// Datos de partida que `GrowTown` consulta al filtrar el catálogo de casas.
/// Agruparlos conserva una frontera explícita entre el walker y el contexto de
/// generación, sin convertir la fixture en un supuesto de clima o fecha.
#[derive(Clone, Copy)]
struct GeneratedTownGrowthContext {
    climate: crate::world_gen::Climate,
    calendar_year: u32,
}

/// Parte vial de `GrowTownInTile` usada por la fundación procedural.
///
/// Esta pieza cubre carretera y la primera bifurcación de casa: una carretera
/// ya existente se recorre con `RandomDiagDir`; al llegar a clear se aplican
/// los dos `Chance16` y se construye el siguiente bloque con la máscara nativa.
/// Para una casa usa el pool mutable de `TryBuildTownHouse`, sin sustituir sus
/// sorteos por una heurística local.
fn grow_generated_town_road_in_tile(
    map: &mut crate::map::Map,
    town: &mut Town,
    tile: TileCoord,
    cur_rb: u8,
    target_dir: Option<u8>,
    context: GeneratedTownGrowthContext,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> GeneratedRoadGrowthResult {
    if cur_rb == 0 {
        let Some(mut target_dir) = target_dir else {
            return GeneratedRoadGrowthResult::SearchStopped;
        };
        if !matches!(map.get_kind(tile), Some(TileKind::Grass | TileKind::Forest)) {
            return GeneratedRoadGrowthResult::SearchStopped;
        }

        // A diferencia del bootstrap, `GrowTownInTile` puede nivelar una
        // pendiente antes de poner carretera. En tierra recién generada la
        // primera alternativa nativa (elevar las esquinas bajas) es
        // suficiente y deja la misma pendiente que verá `IsRoadAllowedHere`.
        if chance16(rng, 1, 6) {
            let _ = level_generated_town_land(map, tile);
        }
        // OpenTTD valida primero la tesela de entrada. Si no admite la vía,
        // no sortea ni curva ni continuación: ese retorno temprano es una
        // frontera RNG observable aun cuando no se escribe ninguna tesela.
        if !generated_town_road_allowed_here(map, town, tile, target_dir, rng) {
            return GeneratedRoadGrowthResult::SearchStopped;
        }

        let source_dir = reverse_town_diag_dir(target_dir);
        if chance16(rng, 1, 4) {
            loop {
                target_dir = random_town_diag_dir(rng);
                if target_dir != source_dir {
                    break;
                }
            }
        }

        // La segunda consulta es sobre la continuación; puede consumir el
        // azar de pendiente y permite un tramo recto junto a una casa aunque
        // no pueda continuar más lejos.
        let rcmd = town_diag_dir_to_road_bits(target_dir) | town_diag_dir_to_road_bits(source_dir);
        let continuation = add_town_diag(tile, target_dir);
        if !generated_town_road_allowed_here(map, town, continuation, target_dir, rng) {
            if target_dir != reverse_town_diag_dir(source_dir) {
                return GeneratedRoadGrowthResult::SearchStopped;
            }
            let right_side = add_town_diag(tile, target_dir.wrapping_add(1) & 3);
            let left_side = add_town_diag(tile, target_dir.wrapping_add(3) & 3);
            if map.get_kind(right_side) != Some(TileKind::House)
                && map.get_kind(left_side) != Some(TileKind::House)
            {
                return GeneratedRoadGrowthResult::SearchStopped;
            }
        }
        if write_generated_town_road_to_map(map, tile, rcmd, town.id) {
            return GeneratedRoadGrowthResult::Road(tile);
        }
        return GeneratedRoadGrowthResult::SearchStopped;
    }

    // `GrowTownInTile` puede llegar por una arista que la carretera destino
    // aún no tiene. Para TL_ORIGINAL añade sólo el bit inverso a esa tesela;
    // es una extensión parcial, no un bloque vial nuevo y no toma otro RNG.
    if let Some(dir) = target_dir
        && cur_rb & town_diag_dir_to_road_bits(reverse_town_diag_dir(dir)) == 0
    {
        let rcmd = town_diag_dir_to_road_bits(reverse_town_diag_dir(dir));
        if add_generated_town_road_bits_to_map(map, tile, rcmd, town.id) {
            return GeneratedRoadGrowthResult::Road(tile);
        }
        return GeneratedRoadGrowthResult::SearchStopped;
    }

    let target_dir = random_town_diag_dir(rng);
    let target_bits = town_diag_dir_to_road_bits(target_dir);
    let (house_tile, road_target_dir) = if cur_rb & target_bits != 0 {
        // En una curva, sólo el bit de `ROAD_X` que apunta a la esquina
        // habilita la casa interior. Cualquier recta, cruce u otro lado de
        // la curva conserva el `Continue` nativo sin sortear más RNG.
        if cur_rb & ROAD_BITS_AXIS_X != target_bits {
            return GeneratedRoadGrowthResult::Continue;
        }
        let Some(house_tile) = generated_town_corner_house_tile(tile, cur_rb) else {
            return GeneratedRoadGrowthResult::Continue;
        };
        (house_tile, None)
    } else {
        (add_town_diag(tile, target_dir), Some(target_dir))
    };

    // `HasTileWaterGround` e `IsValidTile` preceden a la lotería de la casa.
    // En particular, una esquina que cae en agua no consume el `Chance16`
    // de `LevelTownLand`.
    if map.get(house_tile).is_none() || map.get_kind(house_tile) == Some(TileKind::Water) {
        return GeneratedRoadGrowthResult::Continue;
    }

    // TL_ORIGINAL reserva la casa con probabilidad 6/10 cuando la carretera
    // puede seguir, o siempre si `IsRoadAllowedHere` la rechaza. La esquina
    // de una curva no tiene target vial y mantiene `allow_house = true`.
    let allow_house = road_target_dir.is_none_or(|road_target_dir| {
        !generated_town_road_allowed_here(map, town, house_tile, road_target_dir, rng)
            || chance16(rng, 6, 10)
    });
    if allow_house {
        // La rama C++ no nivela ni llama `TryBuildTownHouse` sobre una casa
        // ya presente. En ese caso vuelve al caminador para elegir otro arco.
        if map.get_kind(house_tile) == Some(TileKind::House) {
            return GeneratedRoadGrowthResult::Continue;
        }

        // `LevelTownLand` se decide antes del pool. La mutación debe ocurrir
        // antes de filtrar la casa porque puede cambiar tanto su pendiente
        // como la de los vecinos que recorran las llamadas siguientes.
        if chance16(rng, 1, 6) {
            let _ = level_generated_town_land(map, house_tile);
        }
        if let Some(candidate) = choose_generated_town_house_candidate(
            town,
            map,
            house_tile,
            context.climate,
            context.calendar_year,
            rng,
        ) && materialize_generated_town_house(map, town, candidate, rng)
        {
            return GeneratedRoadGrowthResult::House(candidate.base);
        }
        return GeneratedRoadGrowthResult::Continue;
    }
    if road_target_dir.is_none() {
        return GeneratedRoadGrowthResult::Continue;
    }
    if add_generated_town_road_bits_to_map(map, tile, target_bits, town.id) {
        GeneratedRoadGrowthResult::Road(tile)
    } else {
        GeneratedRoadGrowthResult::SearchStopped
    }
}

/// Recorre una sola llamada a `GrowTown` de la fundación procedural.
///
/// Devuelve la tesela de la carretera o casa que consiguió crear. Todavía es
/// un walker aislado del constructor MVP: su integración completa queda en
/// RMAP-030/RMAP-032, pero tanto las bifurcaciones viales como la primera casa
/// comparten ya el stream global de `GenerateTowns`.
#[allow(dead_code)]
fn grow_generated_town_road_once(
    map: &mut crate::map::Map,
    town: &mut Town,
    context: GeneratedTownGrowthContext,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> Option<TileCoord> {
    let mut tile = town.pos;
    for &(dx, dy) in &TOWN_GROWTH_COORD_MOD {
        if generated_town_road_bits(map, tile) != 0 {
            return grow_generated_town_at_road(map, town, tile, context, rng);
        }
        tile = TileCoord::new(tile.x + dx, tile.y + dy);
    }
    None
}

fn grow_generated_town_at_road(
    map: &mut crate::map::Map,
    town: &mut Town,
    mut tile: TileCoord,
    context: GeneratedTownGrowthContext,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> Option<TileCoord> {
    let mut target_dir = None;
    let houses = i32::from(town.num_houses);
    let mut iterations = match town.layout {
        TownLayout::BetterRoads => 10 + houses * 2 / 9,
        TownLayout::Grid2x2 | TownLayout::Grid3x3 => 10 + houses / 9,
        TownLayout::Original | TownLayout::Random => 10 + houses * 4 / 9,
    };

    loop {
        let cur_rb = generated_town_road_bits(map, tile);
        match grow_generated_town_road_in_tile(map, town, tile, cur_rb, target_dir, context, rng) {
            GeneratedRoadGrowthResult::Road(pos) | GeneratedRoadGrowthResult::House(pos) => {
                return Some(pos);
            }
            GeneratedRoadGrowthResult::SearchStopped => iterations = 0,
            GeneratedRoadGrowthResult::Continue => {}
        }

        let mut candidate_bits = cur_rb;
        if let Some(dir) = target_dir {
            candidate_bits &= !town_diag_dir_to_road_bits(reverse_town_diag_dir(dir));
        }
        if candidate_bits == 0 {
            return None;
        }

        loop {
            if candidate_bits == 0 {
                return None;
            }
            let dir = loop {
                let dir = random_town_diag_dir(rng);
                if candidate_bits & town_diag_dir_to_road_bits(dir) != 0 {
                    break dir;
                }
            };
            candidate_bits &= !town_diag_dir_to_road_bits(dir);
            if generated_can_follow_town_road(map, tile, dir) {
                target_dir = Some(dir);
                break;
            }
        }

        let dir = target_dir?;
        tile = add_town_diag(tile, dir);
        iterations -= 1;
        if iterations < 0 {
            return None;
        }
    }
}

/// `BuildTownHouse` durante `GenerateWorld`: sortea el aspecto de obra y
/// decide si nace terminada. Las casas vanilla no son históricas, por lo que
/// `Chance16(1, 7)` siempre consume su segunda palabra RNG.
fn generated_town_house_construction(
    rng: &mut crate::cargodist::parity::Randomizer,
) -> TownHouseConstruction {
    let construction_random = rng.next();
    let stage = if chance16(rng, 1, 7) {
        u8::try_from(construction_random & 0x03).unwrap_or(0)
    } else {
        TOWN_HOUSE_COMPLETED
    };
    let counter = if stage == TOWN_HOUSE_COMPLETED {
        0
    } else {
        u8::try_from((construction_random >> 2) & 0x03).unwrap_or(0)
    };
    TownHouseConstruction { counter, stage }
}

/// Convierte los flags de `HouseSpec` a la geometría que `MakeTownHouse`
/// escribe en los cuatro `MAP*` consecutivos. La prioridad coincide con las
/// ramas de `TryBuildTownHouse` / `MakeTownHouse`.
const fn generated_town_house_footprint(building_flags: u8) -> TownHouseFootprint {
    if building_flags & BUILDING_FLAG_SIZE_2X2 != 0 {
        TownHouseFootprint::TwoByTwo
    } else if building_flags & BUILDING_FLAG_SIZE_2X1 != 0 {
        TownHouseFootprint::TwoByOne
    } else if building_flags & BUILDING_FLAG_SIZE_1X2 != 0 {
        TownHouseFootprint::OneByTwo
    } else {
        TownHouseFootprint::OneByOne
    }
}

/// Materializa el tramo final de `BuildTownHouse` después de que el pool de
/// `TryBuildTownHouse` aceptó una entrada. `cache.num_houses` de `OpenTTD`
/// cuenta edificios, no subteselas, por eso se incrementa una única vez aun
/// cuando `MakeTownHouse` escriba una huella 2×2.
fn materialize_generated_town_house(
    map: &mut crate::map::Map,
    town: &mut Town,
    candidate: TownHouseCandidate,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> bool {
    let Some(house) = HouseSpec::get(candidate.id) else {
        return false;
    };
    let construction = generated_town_house_construction(rng);
    let spec = TownHouseSpec {
        house_id: candidate.id,
        town_id: town.id,
        random_bits: candidate.random_bits,
        construction_counter: construction.counter,
        construction_stage: construction.stage,
        // Los specs vanilla de esta ruta no exponen `extra_flags`; el primer
        // fixture no es histórico ni protegido. NewGRF se conecta aparte.
        is_protected: false,
        processing_time: 0,
    };
    if map
        .make_town_house_footprint(
            candidate.base,
            spec,
            generated_town_house_footprint(house.building_flags),
        )
        .is_err()
    {
        return false;
    }

    town.num_houses = town.num_houses.saturating_add(1);
    if construction.stage == TOWN_HOUSE_COMPLETED {
        town.population = town
            .population
            .saturating_add(u32::from(house_spec_population(candidate.id)));
    }
    if house.is_church() {
        town.has_church = true;
    }
    if house.is_stadium() {
        town.has_stadium = true;
    }
    update_town_radius(town);
    true
}

/// `Chance16I(a, b, Random())`, que usa los 16 bits bajos y redondeo al
/// entero más cercano; `RandomRange(b) < a` no es equivalente.
fn chance16(rng: &mut crate::cargodist::parity::Randomizer, a: u32, b: u32) -> bool {
    if b == 0 {
        return false;
    }
    let random_low = u64::from(rng.next() & 0xFFFF);
    let divisor = u64::from(b);
    ((random_low
        .saturating_mul(divisor)
        .saturating_add(divisor / 2))
        >> 16)
        < u64::from(a)
}

/// Selecciona una casa vanilla como `TryBuildTownHouse` durante la generación.
///
/// El pool se forma sólo por zona/clima, se extrae con `RandomRange` y se quita
/// con el mismo `swap-with-last` de `OpenTTD`. Los filtros tardíos no devuelven la
/// entrada al pool: año, edificios únicos, pendiente y huella consumen otro
/// sorteo si rechazan el candidato. Esto es importante porque filtrar antes de
/// `RandomRange` adelanta o atrasa todo el stream global de `GenerateTowns`.
///
/// El catálogo `NewGRF` y CB 0x17 siguen fuera de este núcleo vanilla; se
/// incorporarán al conectar la caminata completa de `GrowTown`.
#[allow(dead_code)]
fn choose_generated_town_house_candidate(
    town: &Town,
    map: &crate::map::Map,
    tile: TileCoord,
    climate: crate::world_gen::Climate,
    calendar_year: u32,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> Option<TownHouseCandidate> {
    // Estas dos salidas son previas al pool en C++; por tanto no pueden
    // consumir una palabra RNG aunque no haya una casa posible.
    if !town_layout_allows_house_here(town, tile) || !can_build_house(map, tile, false) {
        return None;
    }

    let (slope, _) = tile_slope_and_z(map, tile)?;
    let max_z = town_house_tile_max_z(map, tile)?;
    let zone = get_town_radius_group(town, tile);
    let required_zones = (1_u16 << (zone as u8)) | climate_zone_mask(climate, max_z);
    let mut probability_max = 0_u32;
    let mut candidates = Vec::new();

    for id in 0..crate::house_spec::NUM_HOUSES_VANILLA {
        let house_id = u16::try_from(id).unwrap_or(u16::MAX);
        let Some(house) = HouseSpec::get(house_id) else {
            continue;
        };
        if !house.matches_zones(required_zones) {
            continue;
        }
        probability_max = probability_max.saturating_add(u32::from(house.probability));
        candidates.push(house);
    }

    if probability_max == 0 {
        return None;
    }
    let initial_probability_max = probability_max;
    let candidate_count = candidates.len();
    let mut attempts = 0_usize;

    while probability_max > 0 {
        let mut roll = rng.random_range(probability_max);
        let mut index = 0_usize;
        while let Some(candidate) = candidates.get(index) {
            let weight = u32::from(candidate.probability);
            if weight > roll {
                break;
            }
            roll = roll.saturating_sub(weight);
            index += 1;
        }

        let house = candidates.get(index).copied()?;
        probability_max = probability_max.saturating_sub(u32::from(house.probability));
        // `probs[i] = probs.back(); probs.pop_back();` del original. El orden
        // que queda para el siguiente `RandomRange` es parte del contrato.
        let _ = candidates.swap_remove(index);
        attempts = attempts.saturating_add(1);

        if calendar_year < house.min_year || calendar_year > house.max_year {
            continue;
        }
        if (house.is_church() && town.has_church) || (house.is_stadium() && town.has_stadium) {
            continue;
        }
        if house.requires_flat() && slope != 0 {
            continue;
        }
        let Some(base) = resolve_town_house_footprint(map, town, tile, house.building_flags) else {
            continue;
        };

        return Some(TownHouseCandidate {
            id: house.id,
            base,
            random_bits: u8::try_from(rng.next() & 0xFF).unwrap_or(0),
            probability_max: initial_probability_max,
            candidate_count,
            attempts,
        });
    }
    None
}

/// Fallback provisional para un sitio que `OpenTTD` aceptó pero cuya trama MVP
/// recta no cabe en altura. El `DoCreateTown` real probará expansiones locales;
/// mientras se porta, un tramo de una tesela mantiene el contrato de fundar al
/// menos tres casas sin desplazar la coordenada validada.
fn compact_town_plan(ctx: &PopCtx<'_>, center: TileCoord) -> Option<StreetTownPlan> {
    // El sitio nativo sólo exige terreno construible en 5×5, no una recta
    // totalmente plana. Mientras `DoCreateTown` sigue pendiente, buscamos un
    // cruce plano cercano sin cambiar `Town::xy`, para no descartar la
    // fundación ya validada por OpenTTD.
    for radius in 0..=8_i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let road = TileCoord::new(center.x + dx, center.y + dy);
                if !tile_is_flat_grass(&ctx.state.map, road) {
                    continue;
                }
                let mut houses = Vec::new();
                for (hx, hy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let candidate = TileCoord::new(road.x + hx, road.y + hy);
                    if tile_ok_for_house(ctx.state, candidate, ctx.preserve) {
                        houses.push(candidate);
                    }
                }
                if houses.len() >= 3 {
                    return Some(StreetTownPlan {
                        axis: StreetAxis::EastWest,
                        roads: vec![road],
                        houses,
                        town_pos: center,
                        bootstrap_road: None,
                    });
                }
            }
        }
    }
    None
}

/// Selecciona una fundación como `CreateRandomTown`, sin construir todavía sus
/// calles/casas. Así la regla de ubicación queda verificable aparte del
/// crecimiento de `DoCreateTown`.
#[cfg(test)]
fn select_random_town_site(
    ctx: &mut PopCtx<'_>,
    town_centers: &[TileCoord],
    attempts: usize,
) -> Option<TileCoord> {
    for _ in 0..attempts {
        if let Some(site) = next_random_town_site(ctx, town_centers) {
            return Some(site);
        }
    }
    None
}

/// Un único `RandomTile` de `CreateRandomTown`, separado para que un
/// constructor provisional pueda seguir al intento siguiente al fallar.
fn next_random_town_site(ctx: &mut PopCtx<'_>, town_centers: &[TileCoord]) -> Option<TileCoord> {
    let candidate = random_tile(ctx.rng.next(), ctx.mw, ctx.mh);
    if ctx.state.map.get_kind(candidate) == Some(TileKind::Water) {
        find_nearest_good_coastal_town_spot(ctx, candidate, town_centers)
    } else if town_can_be_placed_here(ctx, candidate, town_centers) {
        Some(candidate)
    } else {
        None
    }
}

/// `TownCanBePlacedHere(tile, true)` para las teselas que existen en el mapa
/// generado. Las teselas del borde ya se descartan antes del cuadro 5×5.
fn town_can_be_placed_here(
    ctx: &PopCtx<'_>,
    center: TileCoord,
    town_centers: &[TileCoord],
) -> bool {
    if in_preserve(ctx.preserve, center.x, center.y)
        || distance_from_edge(ctx, center) < TOWN_EDGE_DISTANCE
        || town_centers.iter().any(|&other| {
            (other.x - center.x)
                .abs()
                .saturating_add((other.y - center.y).abs())
                < TOWN_MIN_DISTANCE
        })
        || !is_flat_clear_or_tree(&ctx.state.map, center)
    {
        return false;
    }

    let Some((_, town_height)) = tile_slope_and_z(&ctx.state.map, center) else {
        return false;
    };
    let mut valid = 0_usize;
    for y in center.y - 2..=center.y + 2 {
        for x in center.x - 2..=center.x + 2 {
            let candidate = TileCoord::new(x, y);
            if !is_clear_or_tree_not_rough(&ctx.state.map, candidate) {
                continue;
            }
            let Some((_, z)) = tile_slope_and_z(&ctx.state.map, candidate) else {
                continue;
            };
            let Some(max_z) = tile_max_z(&ctx.state.map, candidate) else {
                continue;
            };
            if max_z <= town_height.saturating_add(1) && z.saturating_add(1) >= town_height {
                valid += 1;
                if valid == TOWN_SURROUNDING_GOAL {
                    return true;
                }
            }
        }
    }
    false
}

/// `FindNearestGoodCoastalTownSpot`: toma la primera isla/costa de clear que
/// toca la espiral 40×40 y, dentro de ella, el punto válido más alejado del
/// agua. Ninguna de estas búsquedas consume RNG.
fn find_nearest_good_coastal_town_spot(
    ctx: &PopCtx<'_>,
    start: TileCoord,
    town_centers: &[TileCoord],
) -> Option<TileCoord> {
    for coast in spiral_tiles(start, 40, ctx.mw, ctx.mh) {
        if ctx.state.map.get_kind(coast) != Some(TileKind::Grass) {
            continue;
        }
        let mut furthest = None;
        let mut max_distance = 0_u32;
        for test in spiral_tiles(coast, 10, ctx.mw, ctx.mh) {
            if !is_flat_clear_or_tree(&ctx.state.map, test)
                || ctx.state.map.get_kind(test) != Some(TileKind::Grass)
                || !town_can_be_placed_here(ctx, test, town_centers)
            {
                continue;
            }
            let distance = closest_water_distance(&ctx.state.map, test);
            if distance > max_distance {
                furthest = Some(test);
                max_distance = distance;
            }
        }
        return furthest;
    }
    None
}

fn distance_from_edge(ctx: &PopCtx<'_>, tile: TileCoord) -> i32 {
    let max_x = i32::try_from(ctx.mw).unwrap_or(i32::MAX).saturating_sub(1);
    let max_y = i32::try_from(ctx.mh).unwrap_or(i32::MAX).saturating_sub(1);
    tile.x
        .min(tile.y)
        .min(max_x.saturating_sub(tile.x))
        .min(max_y.saturating_sub(tile.y))
}

fn is_flat_clear_or_tree(map: &crate::map::Map, tile: TileCoord) -> bool {
    matches!(map.get_kind(tile), Some(TileKind::Grass | TileKind::Forest))
        && tile_slope_and_z(map, tile).is_some_and(|(slope, _)| slope == 0)
}

fn is_clear_or_tree_not_rough(map: &crate::map::Map, tile: TileCoord) -> bool {
    map.get(tile).is_some_and(|candidate| match candidate.kind {
        TileKind::Grass => clear_ground_type(candidate.m5) != CLEAR_GROUND_ROUGH,
        // Los árboles se crean después de pueblos en genworld. El subtipo de
        // suelo de árbol aún no se necesita para las fixtures, pero permitir
        // bosque aquí conserva la misma clase de terreno del chequeo nativo.
        TileKind::Forest => true,
        _ => false,
    })
}

fn tile_max_z(map: &crate::map::Map, tile: TileCoord) -> Option<u8> {
    tile_slope_and_z(map, tile).map(|(slope, z)| {
        z.saturating_add(if slope == 0 {
            0
        } else if slope & SLOPE_STEEP != 0 {
            2
        } else {
            1
        })
    })
}

/// `RandomTile()` con el mask de mapas de potencia de dos de `OpenTTD`.
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

/// `SpiralTileSequence` para los diámetros pares de búsqueda costera. El
/// orden afecta qué primera franja clear toma el algoritmo original.
fn spiral_tiles(center: TileCoord, diameter: u32, map_w: u32, map_h: u32) -> Vec<TileCoord> {
    if diameter == 0 || map_w == 0 || map_h == 0 {
        return Vec::new();
    }
    if diameter % 2 == 1 {
        let radius = i32::try_from(diameter / 2).unwrap_or(0);
        let mut tiles =
            Vec::with_capacity(usize::try_from(diameter.saturating_mul(diameter)).unwrap_or(0));
        for y in center.y - radius..=center.y + radius {
            for x in center.x - radius..=center.x + radius {
                if x >= 0 && y >= 0 && x < map_w as i32 && y < map_h as i32 {
                    tiles.push(TileCoord::new(x, y));
                }
            }
        }
        return tiles;
    }

    let max_radius = i32::try_from(diameter / 2).unwrap_or(0);
    let mut radius = 0_i32;
    let mut direction = 0_usize;
    let mut position = 1_i32;
    let mut x = center.x + 1;
    let mut y = center.y;
    let mut tiles =
        Vec::with_capacity(usize::try_from(diameter.saturating_mul(diameter)).unwrap_or(0));

    while radius < max_radius {
        if x >= 0 && y >= 0 && x < map_w as i32 && y < map_h as i32 {
            tiles.push(TileCoord::new(x, y));
        }
        let (dx, dy) = SPIRAL_DIRS[direction];
        x += dx;
        y += dy;
        position -= 1;
        if position > 0 {
            continue;
        }
        direction += 1;
        if direction == SPIRAL_DIRS.len() {
            x += 1;
            y -= 1;
            radius += 1;
            direction = 0;
            if radius == max_radius {
                break;
            }
        }
        position = radius * 2 + 1;
    }
    tiles
}

/// `GetClosestWaterDistance(test, true)`: las costas no son agua a nivel de
/// suelo, igual que `HasTileWaterGround` de `OpenTTD`.
fn closest_water_distance(map: &crate::map::Map, center: TileCoord) -> u32 {
    if is_water_ground(map, center) {
        return 0;
    }
    let (map_w, map_h) = map.dimensions();
    for distance in 1..0x7F_u32 {
        let d = i32::try_from(distance).unwrap_or(i32::MAX);
        for y in center.y - d..=center.y + d {
            for x in center.x - d..=center.x + d {
                if (x - center.x).abs().max((y - center.y).abs()) != d {
                    continue;
                }
                if x >= 0
                    && y >= 0
                    && x < map_w as i32
                    && y < map_h as i32
                    && is_water_ground(map, TileCoord::new(x, y))
                {
                    return distance;
                }
            }
        }
    }
    0x7F
}

fn is_water_ground(map: &crate::map::Map, tile: TileCoord) -> bool {
    map.get(tile)
        .is_some_and(|candidate| candidate.kind == TileKind::Water && (candidate.m5 >> 4) == 0)
}

fn plan_street_town(
    center: TileCoord,
    axis: StreetAxis,
    half_len: i32,
    south_row: bool,
    map_w: i32,
    map_h: i32,
) -> Option<StreetTownPlan> {
    let mut roads = Vec::new();
    let mut houses = Vec::new();
    let town_pos;

    match axis {
        StreetAxis::EastWest => {
            let road_y = center.y;
            town_pos = center;
            for dx in -half_len..=half_len {
                let x = center.x + dx;
                if !coord_in_map(x, road_y, map_w, map_h) {
                    return None;
                }
                roads.push(TileCoord::new(x, road_y));
                for row in house_rows_beside_road(south_row) {
                    let hy = road_y + row;
                    if coord_in_map(x, hy, map_w, map_h) {
                        houses.push(TileCoord::new(x, hy));
                    }
                }
            }
        }
        StreetAxis::NorthSouth => {
            let road_x = center.x;
            town_pos = center;
            for dy in -half_len..=half_len {
                let y = center.y + dy;
                if !coord_in_map(road_x, y, map_w, map_h) {
                    return None;
                }
                roads.push(TileCoord::new(road_x, y));
                for col in house_cols_beside_road(south_row) {
                    let hx = road_x + col;
                    if coord_in_map(hx, y, map_w, map_h) {
                        houses.push(TileCoord::new(hx, y));
                    }
                }
            }
        }
    }

    if roads.is_empty() || houses.len() < 3 {
        return None;
    }
    Some(StreetTownPlan {
        axis,
        roads,
        houses,
        town_pos,
        bootstrap_road: None,
    })
}

fn house_rows_beside_road(second_side: bool) -> Vec<i32> {
    let mut rows = vec![-1];
    if second_side {
        rows.push(1);
    }
    rows
}

fn house_cols_beside_road(east_side: bool) -> Vec<i32> {
    let mut cols = vec![-1];
    if east_side {
        cols.push(1);
    }
    cols
}

fn coord_in_map(x: i32, y: i32, map_w: i32, map_h: i32) -> bool {
    x >= 0 && y >= 0 && x < map_w && y < map_h
}

fn plan_fits_terrain(ctx: &PopCtx<'_>, plan: &StreetTownPlan) -> bool {
    if plan
        .roads
        .iter()
        .any(|&c| in_preserve(ctx.preserve, c.x, c.y))
    {
        return false;
    }
    if !street_roads_are_flat_and_level(ctx.state, &plan.roads) {
        return false;
    }
    if let Some(bootstrap) = plan.bootstrap_road
        && (in_preserve(ctx.preserve, bootstrap.pos.x, bootstrap.pos.y)
            || !can_seed_initial_town_road(&ctx.state.map, bootstrap.pos))
    {
        return false;
    }
    plan.houses
        .iter()
        .filter(|&&c| tile_ok_for_house(ctx.state, c, ctx.preserve))
        .count()
        >= 3
}

fn street_roads_are_flat_and_level(
    state: &crate::game_state::GameState,
    roads: &[TileCoord],
) -> bool {
    let mut base_z = None;
    for &c in roads {
        if !tile_is_flat_grass(&state.map, c) {
            return false;
        }
        let Some((tileh, z)) = tile_slope_and_z(&state.map, c) else {
            return false;
        };
        if tileh != 0 {
            return false;
        }
        match base_z {
            None => base_z = Some(z),
            Some(b) if b != z => return false,
            Some(_) => {}
        }
    }
    true
}

fn build_street_town(
    ctx: &mut PopCtx<'_>,
    plan: &StreetTownPlan,
    town_house_base: u32,
    town_id: u32,
) -> (usize, u32) {
    let road_bits = match plan.axis {
        StreetAxis::EastWest => ROAD_BITS_AXIS_X,
        StreetAxis::NorthSouth => ROAD_BITS_AXIS_Y,
    };

    if !plan_fits_terrain(ctx, plan) {
        return (0, 0);
    }

    for &c in &plan.roads {
        if !write_generated_town_road(ctx.state, c, road_bits, town_id) {
            rollback_road_tiles(ctx.state, &plan.roads);
            return (0, 0);
        }
    }
    if let Some(bootstrap) = plan.bootstrap_road
        && !write_generated_town_road(ctx.state, bootstrap.pos, bootstrap.bits, town_id)
    {
        rollback_road_tiles(ctx.state, &plan.roads);
        return (0, 0);
    }

    let choices = procedural_house_choices();
    let n_choices = u32::try_from(choices.len()).unwrap_or(0);
    if n_choices == 0 {
        return (0, 0);
    }

    let mut placed = 0_usize;
    let mut population = 0_u32;
    for &c in &plan.houses {
        if !tile_ok_for_house(ctx.state, c, ctx.preserve) {
            continue;
        }
        let idx =
            (town_house_base + ctx.rng.random_range(PROCEDURAL_HOUSE_STYLE_SPREAD)) % n_choices;
        let house_id = choices[usize::try_from(idx).unwrap_or(0)];
        let random_bits = u8::try_from(ctx.rng.next() & 0xFF).unwrap_or(0);
        let construction = generated_town_house_construction(ctx.rng);
        if ctx
            .state
            .map
            .make_town_house(
                c,
                TownHouseSpec {
                    house_id,
                    town_id,
                    random_bits,
                    construction_counter: construction.counter,
                    construction_stage: construction.stage,
                    is_protected: false,
                    processing_time: 0,
                },
            )
            .is_ok()
        {
            placed += 1;
            if construction.stage == TOWN_HOUSE_COMPLETED {
                population = population.saturating_add(u32::from(house_spec_population(house_id)));
            }
        }
    }
    (placed, population)
}

fn rollback_road_tiles(state: &mut crate::game_state::GameState, roads: &[TileCoord]) {
    for &c in roads {
        if state.map.get_kind(c) == Some(TileKind::Road) {
            let _ = state.map.set_kind(c, TileKind::Grass);
        }
    }
}

/// Escribe `MakeRoadNormal` para una calle creada durante `GenerateTowns`.
///
/// Las rutas de comando interactivas usan la compañía activa. Durante la
/// generación C++ cambia a `OWNER_TOWN`, fija el índice del pueblo y conserva
/// el sentinel de tram ausente; usar esos bytes evita que el constructor MVP
/// introduzca una compañía humana o una capa de tranvía falsa.
fn write_generated_town_road(
    state: &mut crate::game_state::GameState,
    coord: TileCoord,
    road_bits: u8,
    town_id: u32,
) -> bool {
    write_generated_town_road_to_map(&mut state.map, coord, road_bits, town_id)
}

/// Variante que usa el caminador de `GrowTown` sin requerir el `GameState`
/// entero. Ambos caminos comparten los bytes de `MakeRoadNormal`.
fn write_generated_town_road_to_map(
    map: &mut crate::map::Map,
    coord: TileCoord,
    road_bits: u8,
    town_id: u32,
) -> bool {
    let Some(mut tile) = map.get(coord) else {
        return false;
    };
    let town = u16::try_from(town_id).unwrap_or(u16::MAX).to_le_bytes();
    tile.kind = TileKind::Road;
    tile.mapt = 0x20;
    tile.m1 = crate::company::OWNER_TOWN_M1;
    tile.m2 = town[0];
    tile.m2_hi = town[1];
    tile.m3 = TOWN_ROAD_NO_TRAM_OWNER;
    tile.m3hi = 0;
    tile.m5 = road_bits & 0x0F;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = TOWN_ROAD_INVALID_TRAM_TYPE;
    map.set_tile(coord, tile).is_ok()
}

/// `CMD_BUILD_ROAD` sobre una carretera municipal existente añade sus bits sin
/// reiniciar los campos que ya dejó `MakeRoadNormal`.
fn add_generated_town_road_bits_to_map(
    map: &mut crate::map::Map,
    coord: TileCoord,
    road_bits: u8,
    town_id: u32,
) -> bool {
    let Some(mut tile) = map.get(coord) else {
        return false;
    };
    if tile.kind != TileKind::Road {
        return write_generated_town_road_to_map(map, coord, road_bits, town_id);
    }
    tile.m5 |= road_bits & 0x0F;
    map.set_tile(coord, tile).is_ok()
}

#[cfg(test)]
mod tests {
    use super::super::{TownDensity, town_generation_target_count};
    use super::*;
    use crate::cargodist::parity::Randomizer;
    use crate::game_state::GameState;
    use crate::map::Map;
    use crate::world_gen::{Climate, TerrainType, WorldGenConfig, apply_world_gen_with_rng};

    fn clear_phase_state(seed: u64) -> GameState {
        // La fixture debe coincidir con `world_raw_dumper` y el config
        // headless de OpenTTD, no con el mapa-isla usado por tests genéricos.
        let mut state = GameState::from_map(Map::new_flat(64, 64, 0));
        state.climate = Climate::Temperate;
        apply_world_gen_with_rng(
            &mut state.map,
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
        .expect("64×64 landscape and clear");
        state
    }

    /// La fixture GDB usa una partida Temperate de 1950; mantener esos dos
    /// parámetros en un único sitio evita que los chequeos de la caminata
    /// oculten una fecha o clima distintos del oráculo.
    fn grow_first_fixture_town(
        state: &mut GameState,
        town: &mut Town,
        rng: &mut Randomizer,
    ) -> Option<TileCoord> {
        grow_generated_town_road_once(
            &mut state.map,
            town,
            GeneratedTownGrowthContext {
                climate: Climate::Temperate,
                calendar_year: 1950,
            },
            rng,
        )
    }

    fn assert_first_generated_house(state: &GameState, town: &Town, rng: Randomizer) {
        let house = state
            .map
            .get(TileCoord::new(46, 24))
            .expect("first generated house");
        assert_eq!(house.kind, TileKind::House);
        assert_eq!(house.m1, 157);
        assert_eq!([house.m2, house.m2_hi], 0_u16.to_le_bytes());
        assert_eq!(house.m3, 0x80);
        assert_eq!(house.m5, 0);
        assert_eq!(house.m8 & 0x0FFF, 26);
        assert_eq!(town.num_houses, 23);
        assert_eq!(town.population, 13);
        assert_eq!(rng.state, [3_931_740_615, 3_932_304_260]);
    }

    #[derive(Clone, Copy)]
    struct ExpectedGeneratedHouse {
        pos: TileCoord,
        house_id: u16,
        random_bits: u8,
        num_houses: u16,
        population: u32,
        rng_state: [u32; 2],
    }

    /// Variante de la aserción de casa que conserva los cinco bits de obra
    /// (`MAP5`) y el bit de terminado (`MAP3`). La generación vanilla puede
    /// dejar edificios inconclusos aun cuando se esté creando el mundo.
    #[derive(Clone, Copy)]
    struct ExpectedGeneratedHouseUnderConstruction {
        pos: TileCoord,
        house_id: u16,
        random_bits: u8,
        construction_counter: u8,
        construction_stage: u8,
        num_houses: u16,
        population: u32,
        rng_state: [u32; 2],
    }

    fn assert_generated_house_at(
        state: &GameState,
        town: &Town,
        rng: Randomizer,
        expected: ExpectedGeneratedHouse,
    ) {
        let house = state.map.get(expected.pos).expect("generated house");
        assert_eq!(house.kind, TileKind::House);
        assert_eq!(house.m1, expected.random_bits);
        assert_eq!(house.m3, 0x80);
        assert_eq!(house.m8 & 0x0FFF, expected.house_id);
        assert_eq!(town.num_houses, expected.num_houses);
        assert_eq!(town.population, expected.population);
        assert_eq!(rng.state, expected.rng_state);
    }

    fn assert_generated_house_under_construction_at(
        state: &GameState,
        town: &Town,
        rng: Randomizer,
        expected: ExpectedGeneratedHouseUnderConstruction,
    ) {
        let house = state.map.get(expected.pos).expect("generated house");
        assert_eq!(house.kind, TileKind::House);
        assert_eq!(house.m1, expected.random_bits);
        assert_eq!(house.m3 & 0x80, 0);
        assert_eq!(house.m5 & 0x07, expected.construction_counter);
        assert_eq!((house.m5 >> 3) & 0x03, expected.construction_stage);
        assert_eq!(house.m8 & 0x0FFF, expected.house_id);
        assert_eq!(town.num_houses, expected.num_houses);
        assert_eq!(town.population, expected.population);
        assert_eq!(rng.state, expected.rng_state);
    }

    #[derive(Clone, Copy)]
    struct ExpectedGeneratedRoad {
        pos: TileCoord,
        bits: u8,
        rng_state: [u32; 2],
    }

    fn grow_and_assert_generated_house(
        state: &mut GameState,
        town: &mut Town,
        rng: &mut Randomizer,
        expected: ExpectedGeneratedHouse,
    ) {
        assert_eq!(
            grow_first_fixture_town(state, town, rng),
            Some(expected.pos)
        );
        assert_generated_house_at(state, town, *rng, expected);
    }

    fn grow_and_assert_generated_house_under_construction(
        state: &mut GameState,
        town: &mut Town,
        rng: &mut Randomizer,
        expected: ExpectedGeneratedHouseUnderConstruction,
    ) {
        assert_eq!(
            grow_first_fixture_town(state, town, rng),
            Some(expected.pos)
        );
        assert_generated_house_under_construction_at(state, town, *rng, expected);
    }

    fn grow_and_assert_generated_road(
        state: &mut GameState,
        town: &mut Town,
        rng: &mut Randomizer,
        expected: ExpectedGeneratedRoad,
    ) {
        assert_eq!(
            grow_first_fixture_town(state, town, rng),
            Some(expected.pos)
        );
        assert_eq!(
            state.map.get(expected.pos).expect("generated road").m5,
            expected.bits
        );
        assert_eq!(rng.state, expected.rng_state);
    }

    fn grow_and_assert_no_construction(
        state: &mut GameState,
        town: &mut Town,
        rng: &mut Randomizer,
        rng_state: [u32; 2],
    ) {
        assert_eq!(grow_first_fixture_town(state, town, rng), None);
        assert_eq!(rng.state, rng_state);
    }

    #[test]
    fn coastal_selector_replays_first_seed_foundation_and_rng_boundary() {
        let mut state = clear_phase_state(1_330_935_378);
        // Estado a la entrada de `CreateRandomTown`, después del sorteo de
        // ciudad y de `GenerateTownName` en el oráculo C++.
        let mut rng = Randomizer {
            state: [2_177_730_081, 1_749_743_298],
        };
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
        };

        assert_eq!(
            select_random_town_site(&mut ctx, &[], RANDOM_TOWN_ATTEMPTS),
            Some(TileCoord::new(47, 23))
        );
        assert_eq!(ctx.rng.state, [2_945_732_258, 1_049_486_831]);
    }

    #[test]
    fn land_selector_replays_second_seed_foundation_and_rng_boundary() {
        let mut state = clear_phase_state(1_330_935_379);
        let mut rng = Randomizer {
            state: [3_486_424_933, 3_307_154_652],
        };
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
        };

        assert_eq!(
            select_random_town_site(&mut ctx, &[], RANDOM_TOWN_ATTEMPTS),
            Some(TileCoord::new(43, 15))
        );
        assert_eq!(ctx.rng.state, [394_065_499, 3_120_157_675]);
    }

    #[test]
    fn selector_rejects_sites_at_manhattan_distance_twenty_minus_one() {
        let mut state = GameState::new(64, 64);
        let mut rng = Randomizer::new(1);
        let ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
        };
        assert!(!town_can_be_placed_here(
            &ctx,
            TileCoord::new(32, 32),
            &[TileCoord::new(51, 32)]
        ));
        assert!(town_can_be_placed_here(
            &ctx,
            TileCoord::new(32, 32),
            &[TileCoord::new(52, 32)]
        ));
    }

    #[test]
    fn random_town_size_replays_city_multiplier_and_rng_boundary() {
        // Primera entrada a `DoCreateTown` de la seed 1330935378: C++ toma
        // 11 y, al ser ciudad, aplica `initial_city_size = 2`.
        let mut city_rng = Randomizer {
            state: [2_945_732_258, 1_049_486_831],
        };
        assert_eq!(initial_town_house_budget(&mut city_rng, true), 22);
        assert_eq!(city_rng.state, [3_488_465_418, 1_441_958_355]);

        let mut town_rng = Randomizer {
            state: [2_346_534_627, 3_574_143_874],
        };
        assert_eq!(initial_town_house_budget(&mut town_rng, false), 19);
        assert_eq!(town_rng.state, [2_271_986_047, 1_903_929_563]);
    }

    #[test]
    fn first_growtown_bootstrap_replays_road_bits_and_rng_boundary() {
        let state = clear_phase_state(1_330_935_378);
        // Estado C++ al entrar al primer `GrowTown`, justo después de
        // `TSZ_RANDOM` y el multiplicador de ciudad.
        let mut rng = Randomizer {
            state: [3_488_465_418, 1_441_958_355],
        };
        let road = initial_town_growth_bootstrap(&state.map, TileCoord::new(47, 23), &mut rng)
            .expect("primer bloque vial");

        // GDB en `GenRandomRoadBits`: Random() = 1509800000, a=b=0 y b^=2.
        assert_eq!(road.pos, TileCoord::new(47, 23));
        assert_eq!(road.bits, ROAD_BITS_AXIS_Y);
        assert_eq!(rng.state, [679_301_066, 1_509_800_000]);
    }

    #[test]
    fn corner_house_offsets_follow_native_cardinal_directions() {
        let tile = TileCoord::new(10, 10);
        assert_eq!(
            generated_town_corner_house_tile(tile, ROAD_BITS_N),
            Some(TileCoord::new(11, 11))
        );
        assert_eq!(
            generated_town_corner_house_tile(tile, ROAD_BITS_E),
            Some(TileCoord::new(11, 9))
        );
        assert_eq!(
            generated_town_corner_house_tile(tile, ROAD_BITS_S),
            Some(TileCoord::new(9, 9))
        );
        assert_eq!(
            generated_town_corner_house_tile(tile, ROAD_BITS_W),
            Some(TileCoord::new(9, 11))
        );
        assert_eq!(
            generated_town_corner_house_tile(tile, ROAD_BITS_AXIS_X),
            None
        );
    }

    #[test]
    fn second_growtown_replays_global_random_road_walk() {
        let mut state = clear_phase_state(1_330_935_378);
        let mut rng = Randomizer {
            state: [3_488_465_418, 1_441_958_355],
        };
        let bootstrap = initial_town_growth_bootstrap(&state.map, TileCoord::new(47, 23), &mut rng)
            .expect("bootstrap road");
        assert!(write_generated_town_road(
            &mut state,
            bootstrap.pos,
            bootstrap.bits,
            0,
        ));
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(47, 23),
            num_houses: 22,
            layout: TownLayout::Original,
            ..Town::default()
        };
        // El oráculo entra a `GrowTown` después de `BuildTownHouse`, que ya
        // ejecutó `UpdateTownRadius`; la selección ponderada depende de esta
        // zona aunque la caminata vial anterior no la necesite.
        update_town_radius(&mut town);

        // Segunda llamada a GrowTown de la primera ciudad. La primera
        // `RandomDiagDir` cae en el bloque existente; la caminata toma NW,
        // ejecuta los dos Chance16 y vuelve a NW antes de construir la calle.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 22))
        );
        let road = state.map.get(TileCoord::new(47, 22)).expect("new road");
        assert_eq!(road.kind, TileKind::Road);
        assert_eq!(road.m5, ROAD_BITS_AXIS_Y);
        assert_eq!(road.m1, crate::company::OWNER_TOWN_M1);
        assert_eq!(rng.state, [2_624_695_974, 4_247_471_157]);

        // Las tres llamadas siguientes alternan una calle nueva, una
        // conexión parcial sobre el centro y otra calle nueva. GDB fija estas
        // fronteras antes de la primera petición de casa.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 24))
        );
        assert_eq!(rng.state, [4_152_555_872, 1_800_899_484]);
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 23))
        );
        assert_eq!(
            state
                .map
                .get(TileCoord::new(47, 23))
                .expect("centre road")
                .m5,
            0x0D
        );
        assert_eq!(rng.state, [1_720_666_415, 2_546_907_170]);
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 25))
        );
        assert_eq!(rng.state, [2_141_609_185, 1_465_150_535]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 21))
        );
        assert_eq!(rng.state, [622_992_501, 4_241_598_493]);
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 23))
        );
        assert_eq!(
            state
                .map
                .get(TileCoord::new(47, 23))
                .expect("centre junction")
                .m5,
            0x0F
        );
        assert_eq!(rng.state, [3_496_806_558, 1_566_571_789]);
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(48, 23))
        );
        assert_eq!(rng.state, [2_665_860_601, 314_355_655]);
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(49, 23))
        );
        assert_eq!(rng.state, [499_559_121, 3_620_043_346]);
        assert_eq!(
            tile_slope_and_z(&state.map, TileCoord::new(46, 24)).map(|(slope, _)| slope),
            Some(SLOPE_NW)
        );

        // Décima llamada: por primera vez el camino llega a la rama de casa.
        // La misma caminata consume pool, bits aleatorios y obra, y deja los
        // bytes de `MakeTownHouse` sin sustituir ningún sorteo por el MVP.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(46, 24))
        );
        assert_first_generated_house(&state, &town, rng);
    }

    // El oráculo es una secuencia única: dividirla ocultaría una frontera RNG
    // equivocada entre llamadas consecutivas de `GrowTown`.
    #[allow(clippy::too_many_lines)]
    #[test]
    fn global_walk_replays_first_post_house_branches() {
        let mut state = clear_phase_state(1_330_935_378);
        let mut rng = Randomizer {
            state: [3_488_465_418, 1_441_958_355],
        };
        let bootstrap = initial_town_growth_bootstrap(&state.map, TileCoord::new(47, 23), &mut rng)
            .expect("bootstrap road");
        assert!(write_generated_town_road(
            &mut state,
            bootstrap.pos,
            bootstrap.bits,
            0,
        ));
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(47, 23),
            num_houses: 22,
            layout: TownLayout::Original,
            ..Town::default()
        };
        update_town_radius(&mut town);

        // Las llamadas 2–10 se comprueban en detalle en la fixture anterior;
        // aquí se reconstruyen para fijar el punto de partida del tramo nuevo.
        for expected in [
            TileCoord::new(47, 22),
            TileCoord::new(47, 24),
            TileCoord::new(47, 23),
            TileCoord::new(47, 25),
            TileCoord::new(47, 21),
            TileCoord::new(47, 23),
            TileCoord::new(48, 23),
            TileCoord::new(49, 23),
            TileCoord::new(46, 24),
        ] {
            assert_eq!(
                grow_first_fixture_town(&mut state, &mut town, &mut rng),
                Some(expected)
            );
        }
        assert_eq!(rng.state, [3_931_740_615, 3_932_304_260]);

        // GDB: llamadas 11–14. Cubren prolongación de calle, segunda casa,
        // otra calle perpendicular y una prolongación posterior.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 26))
        );
        let road = state.map.get(TileCoord::new(47, 26)).expect("road n=11");
        assert_eq!(road.m5, ROAD_BITS_AXIS_Y);
        assert_eq!(rng.state, [2_945_147_987, 1_759_293_208]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(48, 22))
        );
        let house = state.map.get(TileCoord::new(48, 22)).expect("house n=12");
        assert_eq!(house.kind, TileKind::House);
        assert_eq!(rng.state, [996_756_625, 699_117_934]);
        assert_eq!(house.m1, 56);
        assert_eq!(house.m3, 0x80);
        assert_eq!(house.m8 & 0x0FFF, 16);
        assert_eq!(town.num_houses, 24);
        assert_eq!(town.population, 108);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(46, 23))
        );
        assert_eq!(
            state.map.get(TileCoord::new(46, 23)).expect("road n=13").m5,
            ROAD_BITS_AXIS_X
        );
        assert_eq!(rng.state, [1_322_664_340, 2_304_059_721]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 27))
        );
        assert_eq!(
            state.map.get(TileCoord::new(47, 27)).expect("road n=14").m5,
            ROAD_BITS_AXIS_Y
        );
        assert_eq!(rng.state, [3_624_132_389, 4_014_754_631]);

        // La llamada 15 termina la búsqueda sin construir, pero su frontera
        // RNG sigue siendo observable y debe quedar antes de la casa n=16.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            None
        );
        assert_eq!(rng.state, [15_256_948, 1_831_888_115]);

        // GDB: llamada 16. Tras el retorno sin escritura vuelve a recorrer
        // la rama vial y materializa otra casa vanilla 1×1.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(46, 26))
        );
        let house = state.map.get(TileCoord::new(46, 26)).expect("house n=16");
        assert_eq!(house.kind, TileKind::House);
        assert_eq!(house.m1, 56);
        assert_eq!(house.m3, 0x80);
        assert_eq!(house.m8 & 0x0FFF, 24);
        assert_eq!(town.num_houses, 25);
        assert_eq!(town.population, 123);
        assert_eq!(rng.state, [315_420_011, 1_693_018_963]);

        // GDB: llamadas 17–29. Este tramo alterna casas, dos retornos sin
        // escritura y extensiones viales; conserva el stream tras RMAP-042.
        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(48, 21))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(48, 21),
                house_id: 39,
                random_bits: 178,
                num_houses: 26,
                population: 158,
                rng_state: [1_927_507_994, 3_926_082_081],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(48, 24))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(48, 24),
                house_id: 39,
                random_bits: 153,
                num_houses: 27,
                population: 193,
                rng_state: [2_792_462_561, 650_015_413],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(46, 27))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(46, 27),
                house_id: 6,
                random_bits: 151,
                num_houses: 28,
                population: 223,
                rng_state: [1_301_758_313, 3_503_407_212],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 20))
        );
        assert_eq!(
            state.map.get(TileCoord::new(47, 20)).expect("road n=20").m5,
            ROAD_BITS_AXIS_Y
        );
        assert_eq!(rng.state, [473_131_335, 1_291_890_479]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(48, 25))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(48, 25),
                house_id: 27,
                random_bits: 77,
                num_houses: 29,
                population: 323,
                rng_state: [3_685_405_240, 1_435_929_572],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(49, 22))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(49, 22),
                house_id: 29,
                random_bits: 22,
                num_houses: 30,
                population: 423,
                rng_state: [2_826_272_915, 420_711_512],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            None
        );
        assert_eq!(rng.state, [1_638_729_553, 2_609_795_962]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(50, 23))
        );
        assert_eq!(
            state.map.get(TileCoord::new(50, 23)).expect("road n=24").m5,
            0x0C
        );
        assert_eq!(rng.state, [1_041_915_711, 1_649_600_337]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(48, 27))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(48, 27),
                house_id: 3,
                random_bits: 55,
                num_houses: 31,
                population: 428,
                rng_state: [3_882_883_480, 2_419_258_248],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(51, 22))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(51, 22),
                house_id: 25,
                random_bits: 201,
                num_houses: 32,
                population: 440,
                rng_state: [1_839_330_471, 4_174_771_340],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            None
        );
        assert_eq!(rng.state, [4_265_538_174, 2_120_585_280]);

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(46, 25))
        );
        assert_generated_house_at(
            &state,
            &town,
            rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(46, 25),
                house_id: 6,
                random_bits: 167,
                num_houses: 33,
                population: 470,
                rng_state: [459_825_490, 3_339_473_443],
            },
        );

        assert_eq!(
            grow_first_fixture_town(&mut state, &mut town, &mut rng),
            Some(TileCoord::new(47, 28))
        );
        assert_eq!(
            state.map.get(TileCoord::new(47, 28)).expect("road n=29").m5,
            0x09
        );
        assert_eq!(rng.state, [2_375_691_898, 265_796_979]);

        // GDB: llamadas 30–44. Después del corner house, alternan ramas que
        // no escriben, conexiones parciales y la misma lotería de casas.
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [605_125_561, 1_729_065_499],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(47, 26),
                bits: 0x07,
                rng_state: [3_636_450_204, 1_400_287_687],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_650_280_168, 798_358_475],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(50, 22),
                house_id: 39,
                random_bits: 53,
                num_houses: 34,
                population: 505,
                rng_state: [1_066_572_928, 1_328_983_146],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(49, 24),
                house_id: 16,
                random_bits: 235,
                num_houses: 35,
                population: 600,
                rng_state: [3_089_153_253, 800_452_775],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(48, 26),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [1_001_774_706, 2_782_416_329],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [29_783_464, 2_538_923_709],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(46, 28),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [3_611_264_031, 3_222_008_102],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(47, 19),
                bits: ROAD_BITS_AXIS_Y,
                rng_state: [3_302_331_109, 3_363_916_080],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(50, 24),
                bits: ROAD_BITS_AXIS_Y,
                rng_state: [4_048_198_352, 2_791_402_603],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(51, 23),
                house_id: 24,
                random_bits: 131,
                num_houses: 36,
                population: 615,
                rng_state: [2_845_500_094, 274_890_803],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [4_168_349_962, 3_434_340_341],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(46, 22),
                house_id: 29,
                random_bits: 12,
                num_houses: 37,
                population: 715,
                rng_state: [3_908_574_912, 3_120_255_309],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(46, 21),
                house_id: 15,
                random_bits: 174,
                num_houses: 38,
                population: 810,
                rng_state: [182_380_726, 4_028_724_846],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(48, 19),
                house_id: 20,
                random_bits: 111,
                num_houses: 39,
                population: 875,
                rng_state: [2_310_750_700, 2_493_436_935],
            },
        );

        // GDB: llamadas 45–60. Este tramo incluye tanto calles que se
        // superponen a una vía existente como las dos obras no terminadas que
        // `BuildTownHouse` sortea durante GenerateWorld. Mantenerlas en la
        // misma secuencia protege cada frontera RNG entre caminos y casas.
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [826_450_367, 3_724_833_213],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(47, 18),
                bits: ROAD_BITS_AXIS_Y,
                rng_state: [2_261_710_564, 3_092_690_480],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [965_030_029, 3_111_770_699],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(46, 18),
                house_id: 25,
                random_bits: 77,
                num_houses: 40,
                population: 887,
                rng_state: [4_073_696_506, 3_390_972_652],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_863_027_217, 1_439_928_571],
        );
        grow_and_assert_generated_house_under_construction(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouseUnderConstruction {
                pos: TileCoord::new(48, 18),
                house_id: 25,
                random_bits: 212,
                construction_counter: 2,
                construction_stage: 0,
                num_houses: 41,
                population: 887,
                rng_state: [823_602_116, 1_211_042_935],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_350_853_069, 2_167_207_462],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(47, 20),
                bits: 0x0D,
                rng_state: [200_532_372, 477_415_756],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(48, 29),
                house_id: 24,
                random_bits: 42,
                num_houses: 42,
                population: 902,
                rng_state: [3_324_607_089, 3_545_840_365],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_270_905_269, 1_060_294_968],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(51, 24),
                house_id: 25,
                random_bits: 174,
                num_houses: 43,
                population: 914,
                rng_state: [3_800_550_565, 4_003_912_885],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(49, 26),
                bits: 0x0A,
                rng_state: [2_039_029_954, 3_340_657_089],
            },
        );
        grow_and_assert_generated_house_under_construction(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouseUnderConstruction {
                pos: TileCoord::new(47, 29),
                house_id: 6,
                random_bits: 248,
                construction_counter: 2,
                construction_stage: 2,
                num_houses: 44,
                population: 914,
                rng_state: [1_430_491_786, 1_296_178_683],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(49, 25),
                house_id: 16,
                random_bits: 111,
                num_houses: 45,
                population: 1_009,
                rng_state: [1_495_887_549, 2_084_743_055],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_814_238_036, 2_373_812_800],
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_638_330_032, 3_483_907_467],
        );

        // GDB: llamadas 61–72. El tramo vuelve a recorrer la frontera que
        // acaba de nivelar RMAP-046 y confirma que las calles siguientes no
        // desplazan ni los retornos ni las casas con población cero.
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [735_079_868, 1_125_417_823],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(46, 20),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [216_521_646, 2_107_826_408],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [3_022_712_418, 3_115_729_456],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(49, 27),
                house_id: 11,
                random_bits: 171,
                num_houses: 46,
                population: 1_009,
                rng_state: [3_940_414_989, 2_405_020_400],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(45, 20),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [3_723_900_061, 2_979_391_363],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [4_071_879_203, 4_265_401_531],
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_511_774_654, 2_533_439_832],
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [3_054_545_718, 2_413_590_698],
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_541_626_736, 1_471_201_578],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(45, 19),
                house_id: 39,
                random_bits: 108,
                num_houses: 47,
                population: 1_044,
                rng_state: [1_152_231_306, 2_805_052_611],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(45, 23),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [663_350_535, 3_962_587_022],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(50, 26),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [4_159_791_342, 1_546_280_518],
            },
        );

        // GDB: llamadas 73–88, último tramo de las 87 iteraciones
        // posteriores al bootstrap. Incluye una obra que conserva etapa 2,
        // dos extensiones parciales sobre la misma calle y la frontera que
        // precede el siguiente `GenerateTownName`.
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_287_643_450, 1_189_114_218],
        );
        grow_and_assert_generated_house_under_construction(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouseUnderConstruction {
                pos: TileCoord::new(45, 24),
                house_id: 10,
                random_bits: 107,
                construction_counter: 2,
                construction_stage: 2,
                num_houses: 48,
                population: 1_044,
                rng_state: [298_008_851, 3_234_727_606],
            },
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(50, 25),
                bits: ROAD_BITS_AXIS_Y,
                rng_state: [1_458_821_164, 2_992_086_678],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [2_521_287_847, 2_313_659_761],
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(46, 19),
                house_id: 6,
                random_bits: 249,
                num_houses: 49,
                population: 1_074,
                rng_state: [964_005_529, 959_794_496],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(45, 22),
                house_id: 14,
                random_bits: 120,
                num_houses: 50,
                population: 1_169,
                rng_state: [1_035_852_168, 3_180_680_691],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [3_925_973_630, 3_952_716_027],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(50, 26),
                bits: 0x0B,
                rng_state: [898_802_329, 2_075_442_547],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [2_070_317_468, 1_545_569_637],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(50, 26),
                bits: 0x0F,
                rng_state: [2_331_701_476, 2_944_451_531],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [265_616_072, 915_649_993],
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [1_920_472_990, 2_070_328_968],
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [2_461_875_466, 2_625_919_482],
        );
        grow_and_assert_generated_road(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedRoad {
                pos: TileCoord::new(45, 28),
                bits: ROAD_BITS_AXIS_X,
                rng_state: [1_229_641_668, 532_314_275],
            },
        );
        grow_and_assert_generated_house(
            &mut state,
            &mut town,
            &mut rng,
            ExpectedGeneratedHouse {
                pos: TileCoord::new(45, 29),
                house_id: 25,
                random_bits: 174,
                num_houses: 51,
                population: 1_181,
                rng_state: [1_949_720_220, 2_452_805_513],
            },
        );
        grow_and_assert_no_construction(
            &mut state,
            &mut town,
            &mut rng,
            [3_221_856_382, 229_218_699],
        );
    }

    #[test]
    fn first_town_house_lottery_replays_candidate_and_random_bits() {
        let state = clear_phase_state(1_330_935_378);
        let mut town = Town {
            pos: TileCoord::new(47, 23),
            num_houses: 22,
            ..Town::default()
        };
        update_town_radius(&mut town);
        let tile = TileCoord::new(46, 24);
        // Entrada al primer `TryBuildTownHouse` de C++; el primer candidato
        // es válido 1×1, por lo que no activa todavía el descarte de huellas.
        let mut rng = Randomizer {
            state: [1_631_607_644, 4_263_025_919],
        };
        let house = choose_generated_town_house_candidate(
            &town,
            &state.map,
            tile,
            Climate::Temperate,
            1950,
            &mut rng,
        )
        .expect("candidate house");

        assert_eq!(house.id, 26);
        assert_eq!(house.base, tile);
        assert_eq!(house.random_bits, 157);
        assert_eq!(house.probability_max, 272);
        assert_eq!(house.candidate_count, 17);
        assert_eq!(house.attempts, 1);
        // RandomRange(probability_max) seguido de Random() para MAP5.
        assert_eq!(rng.state, [2_387_930_541, 1_281_562_269]);
    }

    #[test]
    fn generated_town_house_lottery_removes_late_year_rejection_before_retrying() {
        let state = clear_phase_state(1_330_935_378);
        let mut town = Town {
            pos: TileCoord::new(47, 23),
            num_houses: 22,
            ..Town::default()
        };
        update_town_radius(&mut town);

        // Sexta tentativa de casa de la primera ciudad. El oráculo C++ extrae
        // primero la 32 (aún no disponible en 1950), la elimina y recién
        // después extrae la 6. Si la fecha se filtra al crear el pool, ambos
        // `RandomRange` y toda la frontera posterior quedan mal.
        let tile = TileCoord::new(46, 27);
        let mut rng = Randomizer {
            state: [3_417_675_983, 2_894_021_268],
        };
        let house = choose_generated_town_house_candidate(
            &town,
            &state.map,
            tile,
            Climate::Temperate,
            1950,
            &mut rng,
        )
        .expect("candidate after late-year rejection");

        assert_eq!(house.id, 6);
        assert_eq!(house.base, tile);
        assert_eq!(house.probability_max, 112);
        assert_eq!(house.candidate_count, 7);
        assert_eq!(house.attempts, 2);
        assert_eq!(house.random_bits, 151);
        // Dos RandomRange (32 descartada, 6 aceptada) y Random() de MAP1.
        assert_eq!(rng.state, [3_042_269_420, 2_388_727_447]);
    }

    #[test]
    fn first_town_house_construction_replays_rng_and_completed_state() {
        // Estado justo después de `random_bits` de la primera casa: 26 en
        // (46,24). GDB tras `BuildTownHouse` da población 13 y esta frontera.
        let mut rng = Randomizer {
            state: [2_387_930_541, 1_281_562_269],
        };
        let construction = generated_town_house_construction(&mut rng);

        assert_eq!(construction.stage, TOWN_HOUSE_COMPLETED);
        assert_eq!(construction.counter, 0);
        assert_eq!(house_spec_population(26), 13);
        assert_eq!(rng.state, [3_931_740_615, 3_932_304_260]);
    }

    #[test]
    fn generated_town_road_keeps_native_owner_and_absent_tram_bytes() {
        let mut state = GameState::new(8, 8);
        let coord = TileCoord::new(3, 4);
        assert!(write_generated_town_road(
            &mut state,
            coord,
            ROAD_BITS_AXIS_Y,
            0x1234
        ));
        let road = state.map.get(coord).expect("road tile");
        assert_eq!(road.kind, TileKind::Road);
        assert_eq!(road.mapt, 0x20);
        assert_eq!(road.m1, crate::company::OWNER_TOWN_M1);
        assert_eq!([road.m2, road.m2_hi], 0x1234_u16.to_le_bytes());
        assert_eq!(road.m3, TOWN_ROAD_NO_TRAM_OWNER);
        assert_eq!(road.m3hi, 0);
        assert_eq!(road.m5, ROAD_BITS_AXIS_Y);
        assert_eq!(road.m8, TOWN_ROAD_INVALID_TRAM_TYPE);
    }

    #[test]
    fn integrated_generation_keeps_the_reference_first_foundation() {
        for (seed, begin, expected) in [
            (
                1_330_935_378,
                [1_168_016_413, 2_955_223_551],
                TileCoord::new(47, 23),
            ),
            (
                1_330_935_379,
                [1_179_957_886, 1_700_995_136],
                TileCoord::new(43, 15),
            ),
        ] {
            let mut state = clear_phase_state(seed);
            let mut rng = Randomizer { state: begin };
            let target = town_generation_target_count(TownDensity::Normal, &state.map, &mut rng);
            let mut centers = Vec::new();
            let mut ctx = PopCtx {
                state: &mut state,
                preserve: &[],
                rng: &mut rng,
                mw: 64,
                mh: 64,
            };

            assert!(
                place_towns(&mut ctx, target, &mut centers) > 0,
                "seed {seed}"
            );
            assert_eq!(ctx.state.towns[0].pos, expected, "seed {seed}");
        }
    }
}
