//! Colocación y crecimiento inicial de pueblos (`GenerateTowns` / `DoCreateTown`).

use std::collections::HashSet;

use super::{PopCtx, in_preserve};
use crate::bridge_spec::{BridgeSpecDef, BridgeType, bridge_available_in, set_bridge_middle_mapt};
use crate::company::OWNER_NONE_M1;
use crate::house_spec::{
    BUILDING_FLAG_SIZE_1X2, BUILDING_FLAG_SIZE_2X1, BUILDING_FLAG_SIZE_2X2, HouseSpec,
    climate_zone_mask_at_snow_line, get_town_radius_group,
};
use crate::map::tree_tile_loop::{clear_density, clear_ground_type, tree_count};
use crate::map::{
    SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_STEEP, SLOPE_SW, TOWN_HOUSE_COMPLETED, TileCoord, TileKind,
    TownHouseFootprint, TownHouseSpec, WaterClass, bridge_surface_slope_and_z,
    clear_neighbour_non_flooding_states, complement_slope, has_tile_water_ground, is_coast_tile,
    tile_slope_and_z, water_class_from_m1,
};
use crate::sav::house_spec_population;
use crate::town::{
    Town, TownLayout, town_ticks_to_game_ticks, update_town_growth_rate, update_town_radius,
};
use crate::town_expand::{
    can_build_house, resolve_town_house_footprint, town_house_tile_max_z,
    town_layout_allows_house_here,
};
use crate::townname::generate_town_name;
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_GRASS, CLEAR_GROUND_ROCKY,
    CLEAR_GROUND_ROUGH, CLEAR_GROUND_SNOW, clear_ground_m5,
};

/// Bits de carretera recta (eje X / eje Y).
const ROAD_BITS_AXIS_X: u8 = 0x0A;
const ROAD_BITS_AXIS_Y: u8 = 0x05;
const ROAD_NW: u8 = 0x01;
const ROAD_SW: u8 = 0x02;
const ROAD_SE: u8 = 0x04;
const ROAD_NE: u8 = 0x08;
const ROAD_BITS_N: u8 = 0x09;
const ROAD_BITS_E: u8 = 0x0C;
const ROAD_BITS_S: u8 = 0x06;
const ROAD_BITS_W: u8 = 0x03;
/// `m3` de una calle sin tranvía: owner de tram = `OWNER_TOWN` (none).
const TOWN_ROAD_NO_TRAM_OWNER: u8 = 0xF0;
/// `m8` conserva `INVALID_ROADTYPE` (63) para la capa tram ausente.
const TOWN_ROAD_INVALID_TRAM_TYPE: u16 = 0x0FC0;

/// `CheckRoadSlope` usa estas máscaras antes de materializar una carretera.
/// El primer grupo expresa qué bits no se pueden mezclar en una fundación
/// nivelada; el segundo deja únicamente los ejes rectos que caben al completar
/// una semicarretera cuesta arriba. Se mantienen como bytes porque `RoadBits`
/// también se guarda así en `m5`.
const GENERATED_INVALID_ROAD_BITS_ON_LEVELLED_SLOPE: [u8; 15] = [
    0,
    ROAD_NE | ROAD_SE,
    ROAD_NE | ROAD_NW,
    ROAD_NE,
    ROAD_NW | ROAD_SW,
    0,
    ROAD_NW,
    0,
    ROAD_SE | ROAD_SW,
    ROAD_SE,
    0,
    0,
    ROAD_SW,
    0,
    0,
];

const GENERATED_INVALID_ROAD_BITS_ON_STRAIGHT_SLOPE: [u8; 15] = [
    0,
    0,
    0,
    ROAD_BITS_AXIS_Y,
    0,
    0x0F,
    ROAD_BITS_AXIS_X,
    0x0F,
    0,
    ROAD_BITS_AXIS_X,
    0x0F,
    0x0F,
    ROAD_BITS_AXIS_Y,
    0x0F,
    0x0F,
];

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
/// `GenerateTownName` limita cada intento a mil palabras del generador.
const GENERATED_TOWN_NAME_ATTEMPTS: usize = 1_000;
/// `MAX_LENGTH_TOWN_NAME_CHARS` rechaza nombres con 32 o más caracteres.
const MAX_GENERATED_TOWN_NAME_CHARS: usize = 32;
/// Valor vanilla de `game_creation.town_name` en una partida nueva (inglés).
const DEFAULT_GENERATED_TOWN_NAME_LANGUAGE: u16 = 0;
const SPIRAL_DIRS: [(i32, i32); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
/// Direcciones de `GetClosestWaterDistance`, que recorre rombos Manhattan.
const WATER_DISTANCE_DIAMOND_DIRS: [(i32, i32); 4] = [(-1, 1), (1, 1), (1, -1), (-1, -1)];
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

/// Precios vanilla que intervienen en el límite de `TerraformTownTile`.
///
/// La generación de mundo arranca con la tabla de precios por defecto. El
/// comando nativo descarta la fundación cuando el coste alcanza
/// `(_price[PR_TERRAFORM] + 2) * 8`; no es un presupuesto económico del
/// pueblo, sino una frontera observable de la secuencia urbana.
const GENERATED_TOWN_TERRAFORM_PRICE: u32 = 250;
const GENERATED_TOWN_CLEAR_GRASS_PRICE: u32 = 20;
const GENERATED_TOWN_CLEAR_ROUGH_PRICE: u32 = 40;
const GENERATED_TOWN_CLEAR_ROCKS_PRICE: u32 = 200;
const GENERATED_TOWN_CLEAR_FIELDS_PRICE: u32 = 500;
const GENERATED_TOWN_CLEAR_WATER_PRICE: u32 = 10_000;
const GENERATED_TOWN_TERRAFORM_COST_LIMIT: u32 = (GENERATED_TOWN_TERRAFORM_PRICE + 2) * 8;
/// Valor vanilla de `economy.initial_city_size` en una partida nueva.
const DEFAULT_INITIAL_CITY_SIZE: u32 = 2;

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
    // `GenerateTowns` consume este sorteo aunque no se consiga fundar ningún
    // pueblo. El constructor que sigue conserva el mismo stream durante
    // `DoCreateTown`, incluidos sus intentos de crecimiento inicial.
    let city_random_offset = ctx.rng.next() % DEFAULT_LARGER_TOWNS_INTERVAL;
    // El conjunto nativo se rellena antes del primer intento y reserva el
    // nombre aunque `CreateRandomTown` no consiga una fundación. Sin esta
    // frontera, un nombre repetido consume una sola palabra en Rust mientras
    // que OpenTTD sigue llamando `Random()` hasta encontrar uno válido.
    let mut town_names: HashSet<String> = ctx
        .state
        .towns
        .iter()
        .map(|town| town.name.clone())
        .collect();
    for _ in 0..target {
        let is_city = (city_random_offset
            .saturating_add(u32::try_from(town_centers.len().saturating_sub(before)).unwrap_or(0)))
            % DEFAULT_LARGER_TOWNS_INTERVAL
            == 0;
        // `GenerateTownName` puede consumir hasta mil palabras si el nombre
        // es demasiado largo o ya fue reservado por otro pueblo.
        let Some(name_seed) = next_unique_town_name_seed(ctx.rng, &mut town_names) else {
            continue;
        };
        let _ = try_build_random_town_with_generated_growth(
            ctx,
            town_centers,
            name_seed,
            is_city,
            RANDOM_TOWN_ATTEMPTS,
        );
    }

    // `GenerateTowns` hace un último intento agresivo si no consiguió crear
    // ninguno. Es importante para mapas pequeños e islas: el total inicial es
    // una sugerencia, no una garantía de que los 20 intentos alcancen tierra.
    if town_centers.len() == before {
        // Vanilla descarta el conjunto temporal antes del intento agresivo:
        // aquí sólo deben bloquearse nombres de pueblos que sí sobrevivieron
        // a `CreateRandomTown`.
        town_names.clear();
        town_names.extend(ctx.state.towns.iter().map(|town| town.name.clone()));
        if let Some(name_seed) = next_unique_town_name_seed(ctx.rng, &mut town_names) {
            let _ = try_build_random_town_with_generated_growth(
                ctx,
                town_centers,
                name_seed,
                true,
                RANDOM_TOWN_FALLBACK_ATTEMPTS,
            );
        }
    }
    town_centers.len().saturating_sub(before)
}

/// Devuelve la próxima palabra que `GenerateTownName` aceptaría.
///
/// La reserva se hace antes de intentar `CreateRandomTown`, igual que el
/// `TownNames` temporal de `GenerateTowns`: una fundación que luego se borra
/// no libera el nombre dentro de la primera pasada. `HashSet<String>` modela
/// la comparación del nombre renderizado, ya que varios seeds pueden generar
/// la misma ciudad.
fn next_unique_town_name_seed(
    rng: &mut crate::cargodist::parity::Randomizer,
    town_names: &mut HashSet<String>,
) -> Option<u32> {
    for _ in 0..GENERATED_TOWN_NAME_ATTEMPTS {
        let seed = rng.next();
        let Some(name) = generate_town_name(DEFAULT_GENERATED_TOWN_NAME_LANGUAGE, seed) else {
            continue;
        };
        if name.chars().count() >= MAX_GENERATED_TOWN_NAME_CHARS {
            continue;
        }
        if town_names.insert(name) {
            return Some(seed);
        }
    }
    None
}

/// Recorre los intentos de `CreateRandomTown`.
///
/// `DoCreateTown` puede dejar una fundación sin población tras sus iteraciones
/// de `GrowTown`; en ese caso se restaura su mapa temporal y el intento siguiente
/// conserva la frontera RNG ya consumida, igual que el borrado del pueblo nativo.
fn try_build_random_town_with_generated_growth(
    ctx: &mut PopCtx<'_>,
    town_centers: &mut Vec<TileCoord>,
    name_seed: u32,
    is_city: bool,
    attempts: usize,
) -> bool {
    for _ in 0..attempts {
        let Some(center) = next_random_town_site(ctx, town_centers) else {
            continue;
        };
        if build_selected_town_with_generated_growth(ctx, town_centers, center, name_seed, is_city)
        {
            return true;
        }
    }
    false
}

/// Materializa una fundación ya aceptada por `CreateRandomTown` mediante el
/// bucle inicial de `DoCreateTown`.
///
/// El contador de casas temporal existe sólo durante las `x * 4` llamadas a
/// `GrowTown`: determina las zonas de construcción y se retira antes de
/// publicar el pueblo, exactamente como `town_cmd.cpp`. La ciudad no entra en
/// `GameState::towns` hasta que tiene población, pero recibe desde el comienzo
/// el ID que le correspondería, por lo que las teselas municipales conservan el
/// `TownID` nativo.
fn build_selected_town_with_generated_growth(
    ctx: &mut PopCtx<'_>,
    town_centers: &mut Vec<TileCoord>,
    center: TileCoord,
    name_seed: u32,
    is_city: bool,
) -> bool {
    let name = generate_town_name(DEFAULT_GENERATED_TOWN_NAME_LANGUAGE, name_seed)
        .unwrap_or_else(|| format!("Pueblo {},{}", center.x, center.y));
    let mut town = Town {
        id: u32::try_from(ctx.state.towns.len()).unwrap_or(u32::MAX),
        pos: center,
        name,
        ..Default::default()
    };
    town.initialize_layout(Some(TownLayout::Original));
    town.init_growth_goals(ctx.state.climate);
    town.init_grow_counter();
    town.growth_rate = town_ticks_to_game_ticks(250);

    // `DoCreateTown` aumenta `num_houses` antes de actualizar los radios y
    // recorrer `GrowTown` cuatro veces por casa temporal. Esa elevación es
    // observable: cambia el pool ponderado de `TryBuildTownHouse`.
    let temporary_house_budget = initial_town_house_budget(ctx.rng, is_city);
    town.num_houses = u16::try_from(temporary_house_budget).unwrap_or(u16::MAX);
    update_town_radius(&mut town);
    // `CMD_DELETE_TOWN` revierte una fundación sin población. El generador no
    // publica el pueblo hasta el final, así que basta conservar una copia del
    // mapa; deliberadamente no se revierte el RNG porque OpenTTD ya consumió
    // el intento completo antes de borrarlo.
    let map_before = ctx.state.map.clone();
    let Some(bootstrap) = initial_town_growth_bootstrap(&ctx.state.map, center, ctx.rng) else {
        return false;
    };
    if !write_generated_town_road(ctx.state, bootstrap.pos, bootstrap.bits, town.id) {
        return false;
    }

    let growth_context = GeneratedTownGrowthContext {
        climate: ctx.state.climate,
        snow_line_height: ctx.state.snow_line_height,
        calendar_year: ctx.state.calendar.year,
        bridge_spec_catalog: ctx.state.bridge_spec_catalog.clone(),
    };
    let initial_growth_calls = temporary_house_budget.saturating_mul(4);
    // La primera llamada de `GrowTown` ya está representada por el bootstrap
    // que crea la carretera inicial; continuar en la frontera siguiente evita
    // ejecutar una iteración extra respecto de `DoCreateTown`.
    for _ in 1..initial_growth_calls {
        let _ =
            grow_generated_town_road_once(&mut ctx.state.map, &mut town, &growth_context, ctx.rng);
    }

    if town.population == 0
        || map_changes_preserved_tiles(&map_before, &ctx.state.map, ctx.preserve)
    {
        ctx.state.map = map_before;
        return false;
    }

    town.num_houses = town
        .num_houses
        .saturating_sub(u16::try_from(temporary_house_budget).unwrap_or(u16::MAX));
    update_town_radius(&mut town);
    update_town_growth_rate(
        &mut town,
        &ctx.state.stations,
        &ctx.state.map,
        &ctx.state.industries,
    );
    ctx.state.towns.push(town);
    town_centers.push(center);
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

/// Comprueba que la fundación temporal no haya escrito ni nivelado dentro de
/// una zona reservada. `GrowTown` puede modificar las cuatro esquinas de una
/// tesela, de modo que validar únicamente su resultado no es suficiente.
fn map_changes_preserved_tiles(
    before: &crate::map::Map,
    after: &crate::map::Map,
    preserve: &[crate::world_gen::PreserveRect],
) -> bool {
    preserve.iter().any(|rect| {
        (rect.y0..=rect.y1).any(|y| {
            (rect.x0..=rect.x1).any(|x| {
                let tile = TileCoord::new(x, y);
                before.get(tile) != after.get(tile)
            })
        })
    })
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
    let Some(candidate) = map.get(tile) else {
        return 0;
    };
    match candidate.kind {
        TileKind::Road => candidate.m5 & 0x0F,
        // `GetTownRoadBits` pide `GetAnyRoadBits(..., true)`: una rampa de
        // carretera se presenta al walker como el eje recto completo, no
        // sólo como su boca exterior almacenada en `m5`.
        TileKind::RoadBridge | TileKind::RoadTunnel if candidate.m5 & 0x0C == 0x04 => {
            let direction = candidate.m5 & 0x03;
            town_diag_dir_to_road_bits(direction)
                | town_diag_dir_to_road_bits(reverse_town_diag_dir(direction))
        }
        _ => 0,
    }
}

/// Pendiente efectiva de una carretera con cimiento (`GetFoundationSlope`).
///
/// `IsRoadAllowedHere` no compara siempre contra la pendiente cruda del mapa:
/// cuando `build_on_slopes` está activo, `GetRoadFoundation` puede nivelar una
/// combinación de `m5` y altura (por ejemplo, `SLOPE_WSE` + `ROAD_X` pasa a
/// `SLOPE_FLAT`). Omitir esta reducción hace que el walker rechace una calle
/// existente y consuma un camino RNG distinto al de `OpenTTD`.
fn generated_town_road_surface_slope(tileh: u8, road_bits: u8) -> u8 {
    // `_invalid_tileh_slopes_road[0]` de `road_cmd.cpp`: los bits prohibidos
    // para una base nivelada. Cero significa que la fundación nivela la vía.
    const INVALID_WITHOUT_FOUNDATION: [u8; 15] = [
        0x00, 0x0C, 0x09, 0x08, 0x03, 0x00, 0x01, 0x00, 0x06, 0x04, 0x00, 0x00, 0x02, 0x00, 0x00,
    ];
    // `_invalid_tileh_slopes_road[1]`: una carretera recta puede conservar
    // la pendiente sin cimiento. Las curvas/cruces caen en el cimiento
    // inclinado que `ApplyFoundationToSlope` transforma en una pendiente
    // diagonal de superficie.
    const INVALID_STRAIGHT: [u8; 15] = [
        0x00, 0x00, 0x00, 0x05, 0x00, 0x0F, 0x0A, 0x0F, 0x00, 0x0A, 0x0F, 0x0F, 0x05, 0x0F, 0x0F,
    ];
    if tileh == 0 || road_bits == 0 {
        return tileh;
    }

    // `GetRoadFoundation` trata las pendientes empinadas como la esquina más
    // alta antes de consultar las dos tablas de combinaciones válidas.
    let normalized = if tileh & SLOPE_STEEP != 0 {
        match tileh & 0x0F {
            1 => 1,
            2 => 2,
            4 => 4,
            _ => 8,
        }
    } else {
        tileh & 0x0F
    };

    if INVALID_WITHOUT_FOUNDATION[usize::from(normalized)] & road_bits == 0 {
        return 0;
    }

    let one_corner = matches!(normalized, 1 | 2 | 4 | 8);
    if !one_corner && INVALID_STRAIGHT[usize::from(normalized)] & road_bits == 0 {
        return normalized;
    }

    let highest_corner = match normalized {
        1 => 1,
        2 => 2,
        4 => 4,
        _ => 8,
    };
    if road_bits == ROAD_BITS_AXIS_X {
        if matches!(highest_corner, 1 | 2) {
            SLOPE_SW
        } else {
            SLOPE_NE
        }
    } else if matches!(highest_corner, 2 | 4) {
        SLOPE_SE
    } else {
        SLOPE_NW
    }
}

/// `CleanUpRoadBits` elimina una salida vial que apunta a una tesela con la
/// que no se puede conectar. `GrowTownInTile` compone primero sus dos bits y
/// recién aquí ve si una rama termina contra una casa, agua real o borde.
///
/// Durante `GenerateTowns` aún no existen estaciones ni cruces útiles; las
/// variantes de puente/túnel/estación se conservan explícitamente fuera del
/// subconjunto de RMAP-030. Las carreteras municipales ya son normales, por
/// lo que resultan conectivas como en `road.cpp`.
fn clean_up_generated_town_road_bits(
    map: &crate::map::Map,
    tile: TileCoord,
    mut road_bits: u8,
) -> u8 {
    for dir in 0..4 {
        let bit = town_diag_dir_to_road_bits(dir);
        if road_bits & bit == 0 {
            continue;
        }
        let neighbour = add_town_diag(tile, dir);
        let connective = match map.get_kind(neighbour) {
            Some(TileKind::Grass | TileKind::Forest | TileKind::Road) => true,
            Some(TileKind::RoadBridge | TileKind::RoadTunnel) => {
                // `CleanUpRoadBits` no acepta una rampa de puente/túnel por
                // el mero hecho de ser vial: la boca sólo conecta si su
                // único bit exterior coincide con el bit espejado de la
                // conexión que se está limpiando. `GetAnyRoadBits` del
                // oráculo usa la dirección opuesta a la almacenada en M5.
                generated_town_road_tunnel_bridge_direction(map, neighbour).is_some_and(
                    |bridge_dir| {
                        town_diag_dir_to_road_bits(reverse_town_diag_dir(bridge_dir))
                            & town_diag_dir_to_road_bits(reverse_town_diag_dir(dir))
                            != 0
                    },
                )
            }
            Some(TileKind::Water) => !is_water_ground(map, neighbour),
            Some(
                TileKind::Rail
                | TileKind::RailBridge
                | TileKind::RailTunnel
                | TileKind::RoadDepot
                | TileKind::RailDepot
                | TileKind::ShipDepot
                | TileKind::Airport
                | TileKind::House
                | TileKind::Station
                | TileKind::Industry
                | TileKind::CoalField
                | TileKind::Void
                | TileKind::Unknown(_),
            )
            | None => false,
        };
        if !connective {
            road_bits ^= bit;
        }
    }
    road_bits
}

/// Subconjunto seguro de `CanFollowRoad` para el mapa recién generado.
///
/// Estaciones, puentes, túneles y vías todavía no existen durante la primera
/// fundación de la fixture. Se dejan explícitamente para la conexión completa
/// de RMAP-030, pero esta rama sí conserva el orden de selección/reintento de
/// las carreteras municipales sobre terreno y carreteras existentes.
fn generated_can_follow_town_road(map: &crate::map::Map, tile: TileCoord, dir: u8) -> bool {
    let target = add_town_diag(tile, dir);
    let Some(target_tile) = map.get(target) else {
        return false;
    };
    if has_tile_water_ground(target_tile) {
        return false;
    }
    match target_tile.kind {
        TileKind::Road => generated_town_road_bits(map, target) != 0,
        TileKind::RoadBridge | TileKind::RoadTunnel => target_tile.m5 & 0x0C == 0x04,
        // `CanFollowRoad` sólo rechaza agua de suelo antes de llegar a su
        // `default`. Una costa queda por tanto disponible para seguir o
        // construir la calle igual que clear/trees.
        TileKind::Grass | TileKind::Forest | TileKind::Water => true,
        _ => false,
    }
}

/// `GrowTownAtRoad` abandona la caminata al entrar en una carretera municipal
/// de otra ciudad. `CanFollowRoad` sí acepta la tesela para poder avanzar, pero
/// el chequeo de propietario posterior a `TileAddByDiagDir` termina la llamada
/// sin probar otra dirección ni consumir más RNG. Mantener esta frontera fuera
/// de `generated_can_follow_town_road` conserva ese orden observable.
fn generated_town_road_is_foreign(map: &crate::map::Map, tile: TileCoord, town_id: u32) -> bool {
    let Some(candidate) = map.get(tile) else {
        return false;
    };
    if candidate.kind != TileKind::Road || candidate.m1 != crate::company::OWNER_TOWN_M1 {
        return false;
    }
    let owner = u16::from(candidate.m2) | (u16::from(candidate.m2_hi) << 8);
    owner != u16::try_from(town_id).unwrap_or(u16::MAX)
}

/// Dirección persistida de una rampa vial `MP_TUNNELBRIDGE`; `None` para
/// ferrocarril, una rampa corrupta o una tesela común.
fn generated_town_road_tunnel_bridge_direction(
    map: &crate::map::Map,
    tile: TileCoord,
) -> Option<u8> {
    let candidate = map.get(tile)?;
    matches!(candidate.kind, TileKind::RoadBridge | TileKind::RoadTunnel)
        .then_some(candidate)
        .filter(|candidate| candidate.m5 & 0x0C == 0x04)
        .map(|candidate| candidate.m5 & 0x03)
}

/// Equivalente seguro de `GetOtherTunnelBridgeEnd` para la caminata urbana.
/// Los puentes tienen un vano persistente; los túneles usan sus dos bocas y
/// por ello comparten el resolver de mapa ya validado.
fn generated_town_road_tunnel_bridge_other_end(
    map: &crate::map::Map,
    tile: TileCoord,
) -> Option<TileCoord> {
    match map.get_kind(tile)? {
        TileKind::RoadBridge => crate::road_bridge_other_end(map, tile),
        TileKind::RoadTunnel => crate::map::resolve_existing_tunnel_end(map, tile),
        _ => None,
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
    let clearable = map.get(tile).is_some_and(|candidate| {
        // `IsRoadAllowedHere` first accepts any existing town road bits. A
        // bridge/tunnel mouth exposes those bits through `GetAnyRoadBits`,
        // even though its tile kind is not `MP_ROAD`; rejecting it here would
        // stop the walker before `CleanUpRoadBits` and shift the RNG stream.
        generated_town_road_bits(map, tile) != 0
            || matches!(
                candidate.kind,
                TileKind::Grass | TileKind::Forest | TileKind::Road
            )
            || (candidate.kind == TileKind::Water && !has_tile_water_ground(candidate))
    });
    if !clearable {
        return false;
    }

    let neighbour_distance = if town.layout == TownLayout::Original {
        1
    } else {
        2
    };
    let has_neighbour = generated_is_neighbour_road_tile(map, tile, dir, neighbour_distance);
    let ret = !has_neighbour;

    let raw_slope = tile_slope_and_z(map, tile).map_or(SLOPE_STEEP, |(slope, _)| slope);
    let slope = if map.get_kind(tile) == Some(TileKind::Road) {
        generated_town_road_surface_slope(raw_slope, generated_town_road_bits(map, tile))
    } else {
        raw_slope
    };
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

const SLOPE_CORNER_W: u8 = 0x01;
const SLOPE_CORNER_S: u8 = 0x02;
const SLOPE_CORNER_E: u8 = 0x04;
const SLOPE_CORNER_N: u8 = 0x08;
const SLOPE_CORNER_MASK: u8 = SLOPE_CORNER_W | SLOPE_CORNER_S | SLOPE_CORNER_E | SLOPE_CORNER_N;

/// `CmdTerraformLand` guarda cada vértice como la esquina norte de una
/// tesela. Esta tabla es el mapeo de `SLOPE_W/S/E/N` a ese almacenamiento.
const fn generated_town_terraform_corner(tile: TileCoord, corner: u8) -> Option<TileCoord> {
    match corner {
        SLOPE_CORNER_W => Some(TileCoord::new(tile.x + 1, tile.y)),
        SLOPE_CORNER_S => Some(TileCoord::new(tile.x + 1, tile.y + 1)),
        SLOPE_CORNER_E => Some(TileCoord::new(tile.x, tile.y + 1)),
        SLOPE_CORNER_N => Some(tile),
        _ => None,
    }
}

/// Modelo local de `TerraformerState` para `LevelTownLand`.
///
/// La altura se almacena en la esquina norte de una tesela. Al modificar una
/// esquina, `CmdTerraformLand` puede tener que ajustar recursivamente las
/// cuatro esquinas ortogonales vecinas para que ninguna diferencia supere un
/// nivel. Mantener esos cambios fuera del mapa hasta el final es esencial:
/// una carretera o una costa alcanzada por la cascada rechaza todo el comando,
/// igual que la prueba sin `Execute` de `OpenTTD`.
#[derive(Default)]
struct GeneratedTownTerraformState {
    heights: Vec<(TileCoord, u8)>,
    dirty_tiles: Vec<TileCoord>,
    terraform_cost: u32,
}

impl GeneratedTownTerraformState {
    fn height_at(&self, map: &crate::map::Map, vertex: TileCoord) -> Option<u8> {
        self.heights
            .iter()
            .rev()
            .find_map(|(candidate, height)| (*candidate == vertex).then_some(*height))
            .or_else(|| map.get(vertex).map(|tile| tile.height))
    }

    fn set_height(&mut self, vertex: TileCoord, height: u8) {
        if let Some((_, current)) = self
            .heights
            .iter_mut()
            .find(|(candidate, _)| *candidate == vertex)
        {
            *current = height;
        } else {
            self.heights.push((vertex, height));
        }
    }

    fn add_dirty_tile(&mut self, map: &crate::map::Map, tile: TileCoord) {
        if map.get(tile).is_some() && !self.dirty_tiles.contains(&tile) {
            self.dirty_tiles.push(tile);
        }
    }

    fn add_dirty_tiles_around(&mut self, map: &crate::map::Map, vertex: TileCoord) {
        // Mismo orden y geometría que `TerraformAddDirtyTileAround`: las cuatro
        // teselas que comparten la esquina norte almacenada.
        self.add_dirty_tile(map, TileCoord::new(vertex.x, vertex.y - 1));
        self.add_dirty_tile(map, TileCoord::new(vertex.x - 1, vertex.y - 1));
        self.add_dirty_tile(map, TileCoord::new(vertex.x - 1, vertex.y));
        self.add_dirty_tile(map, vertex);
    }
}

/// Replica la recursión de `TerraformTileHeight` para el subconjunto de
/// terreno que puede tocar `GenerateTowns`. El orden de vecinos es
/// NE, SE, SW, NW (`-X, +Y, +X, -Y`), el mismo del enum nativo.
fn generated_town_terraform_height(
    map: &crate::map::Map,
    state: &mut GeneratedTownTerraformState,
    vertex: TileCoord,
    height: u8,
) -> bool {
    let Some(current) = state.height_at(map, vertex) else {
        return false;
    };
    // `TerraformTileHeight` falla si una segunda esquina del mismo comando ya
    // dejó este vértice en la altura pedida.
    if height == current {
        return false;
    }

    state.add_dirty_tiles_around(map, vertex);
    state.set_height(vertex, height);
    state.terraform_cost += GENERATED_TOWN_TERRAFORM_PRICE;

    for (dx, dy) in [(-1, 0), (0, 1), (1, 0), (0, -1)] {
        let neighbour = TileCoord::new(vertex.x + dx, vertex.y + dy);
        let Some(neighbour_height) = state.height_at(map, neighbour) else {
            continue;
        };
        let height_diff = i16::from(height) - i16::from(neighbour_height);
        if height_diff.unsigned_abs() <= 1 {
            continue;
        }

        // OpenTTD acerca el vecino exactamente a un nivel de la nueva altura.
        let adjusted_height = if height_diff < 0 {
            neighbour_height.checked_sub(1)
        } else {
            neighbour_height.checked_add(1)
        };
        let Some(adjusted_height) = adjusted_height else {
            return false;
        };
        if !generated_town_terraform_height(map, state, neighbour, adjusted_height) {
            return false;
        }
    }
    true
}

/// Coste de `CMD_LANDSCAPE_CLEAR` que invoca `TerraformTile_*` para un tile
/// sucio de `CmdTerraformLand`.
///
/// El pase de prueba nativo usa `NoWater`: agua real rechaza la fundación,
/// pero una costa se puede convertir a clear. Las ramas de ciudad que llegan
/// a costa son precisamente las que hacen visible este detalle en los bytes
/// `m5` y `m3` de las teselas vecinas.
fn generated_town_terraform_clear_cost(map: &crate::map::Map, tile: TileCoord) -> Option<u32> {
    let entry = map.get(tile)?;
    match entry.kind {
        // `CmdTerraformLand` no invoca el procedimiento del tile para
        // `MP_VOID`; la altura puede propagarse hasta el borde sin clear ni
        // coste adicional.
        TileKind::Void => Some(0),
        TileKind::Grass => {
            let ground = clear_ground_type(entry.m5);
            let clear_price = match ground {
                CLEAR_GROUND_GRASS => GENERATED_TOWN_CLEAR_GRASS_PRICE,
                CLEAR_GROUND_ROUGH | CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => {
                    GENERATED_TOWN_CLEAR_ROUGH_PRICE
                }
                CLEAR_GROUND_ROCKY => GENERATED_TOWN_CLEAR_ROCKS_PRICE,
                CLEAR_GROUND_FIELDS => GENERATED_TOWN_CLEAR_FIELDS_PRICE,
                _ => return None,
            };
            if entry.m3 & 0x10 != 0 {
                // `IsSnowTile`: precio del suelo bajo la nieve y la diferencia
                // absoluta entre rough y grass.
                Some(
                    clear_price + GENERATED_TOWN_CLEAR_ROUGH_PRICE
                        - GENERATED_TOWN_CLEAR_GRASS_PRICE,
                )
            } else if ground != CLEAR_GROUND_GRASS || clear_density(entry.m5) != 0 {
                Some(clear_price)
            } else {
                Some(0)
            }
        }
        TileKind::Forest => {
            // `ClearTile_Trees` cobra por árbol; los tipos rainforest/cactus
            // ocupan el intervalo 20..=27 y multiplican por cuatro.
            let multiplier = u32::from((20..=27).contains(&entry.m3)) * 3 + 1;
            Some(u32::from(tree_count(entry.m5)) * GENERATED_TOWN_CLEAR_GRASS_PRICE * multiplier)
        }
        TileKind::Water if is_coast_tile(entry) => {
            // `ClearTile_Water` consulta la pendiente previa a aplicar las
            // alturas nuevas. Una costa con una única esquina elevada vale
            // `PR_CLEAR_WATER`; de otro modo vale rough.
            let (slope, _) = tile_slope_and_z(map, tile)?;
            Some(if matches!(slope, 1 | 2 | 4 | 8) {
                GENERATED_TOWN_CLEAR_WATER_PRICE
            } else {
                GENERATED_TOWN_CLEAR_ROUGH_PRICE
            })
        }
        // Infraestructura, agua no-costera, casas y tipos desconocidos hacen
        // fallar el comando de prueba, como `TerraformTile_*` con NoWater.
        _ => None,
    }
}

/// Coste total que inspecciona `TerraformTownTile` antes del pase `Execute`.
fn generated_town_terraform_cost(
    map: &crate::map::Map,
    state: &GeneratedTownTerraformState,
) -> Option<u32> {
    let clear_cost = state.dirty_tiles.iter().try_fold(0_u32, |total, dirty| {
        generated_town_terraform_clear_cost(map, *dirty).and_then(|cost| total.checked_add(cost))
    })?;
    state.terraform_cost.checked_add(clear_cost)
}

/// Materializa el `DoClearSquare` de una tesela ya aprobada por el pase de
/// prueba. Todos los cambios se hacen después de evaluar coste y validez de la
/// operación completa, preservando la atomicidad de `CmdTerraformLand`.
fn clear_generated_town_terraform_tile(map: &mut crate::map::Map, tile: TileCoord) -> bool {
    let Some(mut entry) = map.get(tile) else {
        return false;
    };
    if entry.kind == TileKind::Void {
        return true;
    }
    if !(matches!(entry.kind, TileKind::Grass | TileKind::Forest)
        || entry.kind == TileKind::Water && is_coast_tile(entry))
    {
        return false;
    }

    clear_neighbour_non_flooding_states(map, tile);
    entry.kind = TileKind::Grass;
    entry.mapt &= 0x0F;
    entry.m1 = OWNER_NONE_M1;
    entry.m2 = 0;
    entry.m2_hi = 0;
    entry.m3 = 0;
    entry.m3hi = 0;
    entry.m5 = clear_ground_m5(CLEAR_GROUND_GRASS, 3);
    entry.m6 = 0;
    entry.m7 = 0;
    entry.m8 = 0;
    map.set_tile(tile, entry).is_ok()
}

/// Despeje de una boca de túnel municipal.
///
/// `CmdBuildTunnel` puede arrancar sobre una carretera de `OWNER_TOWN` que el
/// caminador acaba de crear. `ClearTile_Road` retira esa calle bajo `Auto`,
/// mientras que el despeje usado por `TerraformTownTile` debe seguir
/// rechazando infraestructura. Mantener esta variante separada conserva
/// ambas semánticas y los efectos laterales sobre el agua vecina.
fn clear_generated_town_tunnel_endpoint(map: &mut crate::map::Map, tile: TileCoord) -> bool {
    let Some(mut entry) = map.get(tile) else {
        return false;
    };
    if entry.kind == TileKind::Void {
        return true;
    }
    let municipal_road = entry.kind == TileKind::Road
        && entry.m1 == crate::company::OWNER_TOWN_M1
        && entry.m3 == TOWN_ROAD_NO_TRAM_OWNER
        && entry.m8 == TOWN_ROAD_INVALID_TRAM_TYPE
        && crate::road_type::tram_track_bits(&entry) == 0;
    if !(matches!(entry.kind, TileKind::Grass | TileKind::Forest)
        || entry.kind == TileKind::Water && is_coast_tile(entry)
        || municipal_road)
    {
        return false;
    }

    clear_neighbour_non_flooding_states(map, tile);
    entry.kind = TileKind::Grass;
    entry.mapt &= 0x0F;
    entry.m1 = OWNER_NONE_M1;
    entry.m2 = 0;
    entry.m2_hi = 0;
    entry.m3 = 0;
    entry.m3hi = 0;
    entry.m5 = clear_ground_m5(CLEAR_GROUND_GRASS, 3);
    entry.m6 = 0;
    entry.m7 = 0;
    entry.m8 = 0;
    map.set_tile(tile, entry).is_ok()
}

/// Intenta una alternativa de `TerraformTownTile` en un modelo atómico antes
/// de materializarla. Además de propagar las alturas, reproduce el límite de
/// coste y el `DoClearSquare` de `CmdTerraformLand`: sin ambos, una fundación
/// urbana puede aceptarse de más o dejar residuos de clear/costa que cambian
/// los bytes del mapa y la secuencia posterior.
fn try_generated_town_terraform(
    map: &mut crate::map::Map,
    tile: TileCoord,
    corners: u8,
    raise: bool,
    enforce_cost_limit: bool,
) -> bool {
    let mut state = GeneratedTownTerraformState::default();
    for corner in [
        SLOPE_CORNER_W,
        SLOPE_CORNER_S,
        SLOPE_CORNER_E,
        SLOPE_CORNER_N,
    ] {
        if corners & corner == 0 {
            continue;
        }
        let Some(vertex) = generated_town_terraform_corner(tile, corner) else {
            return false;
        };
        let Some(current) = map.get(vertex) else {
            return false;
        };
        let Some(height) = (if raise {
            current.height.checked_add(1)
        } else {
            current.height.checked_sub(1)
        }) else {
            return false;
        };
        if !generated_town_terraform_height(map, &mut state, vertex, height) {
            return false;
        }
    }
    if state.heights.is_empty() {
        return false;
    }
    let Some(total_cost) = generated_town_terraform_cost(map, &state) else {
        return false;
    };
    if enforce_cost_limit && total_cost >= GENERATED_TOWN_TERRAFORM_COST_LIMIT {
        return false;
    }
    if !state
        .dirty_tiles
        .iter()
        .copied()
        .all(|dirty| clear_generated_town_terraform_tile(map, dirty))
    {
        return false;
    }
    for (vertex, height) in state.heights {
        if map.set_height(vertex, height).is_err() {
            return false;
        }
    }
    true
}

fn try_level_generated_town_land(
    map: &mut crate::map::Map,
    tile: TileCoord,
    corners: u8,
    raise: bool,
) -> bool {
    try_generated_town_terraform(map, tile, corners, raise, true)
}

/// `CmdBuildTunnel` invoca `CMD_TERRAFORM_LAND` directamente y no pasa por el
/// límite reducido de `TerraformTownTile`. Una boca en una ladera pronunciada
/// puede propagar el ajuste a más de ocho vértices; el coste se valida para
/// que no falle el comando, pero no se usa como umbral de rechazo.
fn try_level_generated_tunnel_land(
    map: &mut crate::map::Map,
    tile: TileCoord,
    corners: u8,
    raise: bool,
) -> bool {
    try_generated_town_terraform(map, tile, corners, raise, false)
}

/// Subconjunto ejecutable de `LevelTownLand` para la fundación de un pueblo.
///
/// `OpenTTD` intenta primero elevar las esquinas bajas y, si ese `CmdTerraformLand`
/// no es viable, baja las altas. No basta con fijar los cuatro vértices al
/// máximo: una carretera que comparte una esquina puede rechazar el primer
/// intento y cambia la pendiente que verá la siguiente llamada a `GrowTown`.
fn level_generated_town_land(map: &mut crate::map::Map, tile: TileCoord) -> bool {
    if map.get_kind(tile) == Some(TileKind::House) {
        return false;
    }
    let Some((slope, _)) = tile_slope_and_z(map, tile) else {
        return false;
    };
    if slope == 0 {
        return false;
    }
    let low_corners = (!slope) & SLOPE_CORNER_MASK;
    if try_level_generated_town_land(map, tile, low_corners, true) {
        return true;
    }
    try_level_generated_town_land(map, tile, slope & SLOPE_CORNER_MASK, false)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GeneratedRoadGrowthResult {
    Road(TileCoord),
    House(TileCoord),
    /// `GrowTownInTile` saltó desde una boca vial al extremo opuesto. El
    /// caller debe continuar fuera de la boca, sin sortear una dirección del
    /// bloque anterior.
    TunnelBridge(TileCoord),
    Continue,
    SearchStopped,
}

/// Límite plano de `GrowTownWithBridge`: las ciudades pequeñas sólo pueden
/// cruzar ríos cortos. La longitud cuenta el salto desde la primera rampa
/// hasta la rampa opuesta, igual que `bridge_length` de `OpenTTD`.
const GENERATED_TOWN_FLAT_BRIDGE_LENGTH_CAP: usize = 5;
/// El arranque inclinado escala con población, pero la rutina nativa lo limita
/// a once teselas incluso para ciudades grandes.
const GENERATED_TOWN_SLOPED_BRIDGE_LENGTH_CAP: usize = 11;
/// `GrowTownWithTunnel` permite cubrir una montaña continua con un mínimo de
/// siete teselas, ampliable con la población temporal del pueblo.
const GENERATED_TOWN_MOUNTAIN_TUNNEL_LENGTH_BASE: usize = 7;
/// `MAX_BRIDGES - 1`: la selección urbana toma los slots 0..=11, no el
/// último puente tubular. Mantener el límite es una frontera RNG observable.
const GENERATED_TOWN_BRIDGE_RANDOM_TYPE_LIMIT: u32 = 12;
/// La referencia abandona después de 23 tipos sorteados, incluso si la
/// geometría resulta inviable para todos ellos.
const GENERATED_TOWN_BRIDGE_TYPE_ATTEMPTS: usize = 23;

/// `IsWaterTile(tile) && !IsSea(tile)` dentro de la rama plana de
/// `GrowTownWithBridge`. Un canal tiene la misma semántica que un río aquí;
/// una costa/agua marina no es un candidato para un puente urbano plano.
fn generated_town_flat_bridge_crosses_water(tile: crate::map::Tile) -> bool {
    tile.kind == TileKind::Water
        && !is_coast_tile(tile)
        && water_class_from_m1(tile.m1) != WaterClass::Sea
}

/// `GrowTownWithBridge` sólo recorre agua plana (`IsWaterTile`); la rama plana
/// además limita el cruce a río/canal para no tender puentes sobre el mar.
fn generated_town_bridge_crosses_water(tile: crate::map::Tile, start_is_flat: bool) -> bool {
    tile.kind == TileKind::Water
        && !is_coast_tile(tile)
        && (!start_is_flat || generated_town_flat_bridge_crosses_water(tile))
}

/// `InclinedSlope(DiagDirection)` de `OpenTTD`.
const fn generated_town_inclined_slope(direction: u8) -> u8 {
    match direction & 3 {
        0 => SLOPE_NE,
        1 => SLOPE_SE,
        2 => SLOPE_SW,
        _ => SLOPE_NW,
    }
}

/// Tope de `GrowTownWithBridge`: plano fijo a cinco, inclinado según la
/// población temporal que `DoCreateTown` mantiene durante su bootstrap.
fn generated_town_bridge_length_cap(start_slope: u8, population: u32) -> usize {
    if start_slope == 0 {
        GENERATED_TOWN_FLAT_BRIDGE_LENGTH_CAP
    } else {
        usize::try_from(population / 1_000)
            .unwrap_or(usize::MAX)
            .saturating_add(GENERATED_TOWN_FLAT_BRIDGE_LENGTH_CAP)
            .min(GENERATED_TOWN_SLOPED_BRIDGE_LENGTH_CAP)
    }
}

/// Límite de `GrowTownWithTunnel` cuando la pendiente continua se clasifica
/// como montaña. `OpenTTD` parte de siete teselas y sólo amplía ese recorrido
/// con la población temporal del pueblo.
fn generated_town_mountain_tunnel_length_cap(population: u32) -> usize {
    usize::try_from(population / 1_000)
        .unwrap_or(usize::MAX)
        .saturating_add(GENERATED_TOWN_MOUNTAIN_TUNNEL_LENGTH_BASE)
}

/// `SpiralTileSequence(tile, bridge_length, 0, 0)` usado por
/// `GrowTownWithBridge` para descartar puentes paralelos redundantes.
///
/// La secuencia nativa recorre una espiral de coronas alrededor de la rampa
/// inicial. Para esta consulta sólo importa pertenecer a la secuencia (el
/// orden no consume RNG), pero no se puede sustituir por una ventana cuadrada:
/// sus esquinas no son inspeccionadas por `SpiralTileSequence`.
fn generated_town_has_parallel_road_bridge(
    map: &crate::map::Map,
    start: TileCoord,
    radius: usize,
    direction: u8,
) -> bool {
    let opposite_slope = generated_town_inclined_slope(reverse_town_diag_dir(direction));
    // `SpiralTileSequence(start, radius, 0, 0)` no es un cuadrado: sus
    // cascarones avanzan con `TileIndexDiffCByDiagDir` y saltan cada corona
    // con `DIR_W = {+1,-1}`. El filtro cuadrado anterior podía ver una rampa
    // diagonal a dos teselas que el oráculo nunca inspecciona (RMAP-134).
    let mut candidate = TileCoord::new(start.x.saturating_add(1), start.y);
    for ring in 0..radius {
        let side_length = ring.saturating_mul(2).saturating_add(1);
        for spiral_direction in 0..4_u8 {
            for _ in 0..side_length {
                if map.get_kind(candidate) == Some(TileKind::RoadBridge)
                    && tile_slope_and_z(map, candidate)
                        .is_some_and(|(slope, _)| slope & opposite_slope != 0)
                {
                    return true;
                }
                let (dx, dy) = crate::map::diag_dir_offset(spiral_direction);
                candidate = TileCoord::new(
                    candidate.x.saturating_add(dx),
                    candidate.y.saturating_add(dy),
                );
            }
        }
        // `TileIndexDiffCByDir(DIR_W)` in OpenTTD.
        candidate = TileCoord::new(candidate.x.saturating_add(1), candidate.y.saturating_sub(1));
    }
    false
}

/// Comprueba la continuación que exige `CanRoadContinueIntoNextTile` una vez
/// que ya se encontró la otra orilla. Durante la generación inicial no hay
/// estaciones, pasos a nivel ni carreteras unidireccionales creadas por el
/// jugador; conservar las rutas habituales evita que un río corto consuma la
/// selección de puente cuando no hay suelo al que conectar.
fn generated_town_road_can_continue_after_bridge(
    map: &crate::map::Map,
    end: TileCoord,
    direction: u8,
) -> bool {
    let next = add_town_diag(end, direction);
    let Some(tile) = map.get(next) else {
        return false;
    };
    match tile.kind {
        TileKind::Grass | TileKind::Forest => true,
        TileKind::RoadBridge | TileKind::RoadTunnel => {
            tile.m5 & 0x0C == 0x04 && tile.m5 & 0x03 == direction
        }
        // `CMD_BUILD_ROAD(NoWater)` puede proseguir por una costa, pero no
        // sobre agua real. Es la misma distinción que `IsRoadAllowedHere`.
        TileKind::Water => !has_tile_water_ground(tile),
        // `CanRoadContinueIntoNextTile` accepts an `MP_ROAD` tile only when it
        // is a depot facing the bridge direction. Procedural town growth has
        // no depots at this point, so normal roads and all other tile kinds
        // stop the preflight instead of consuming bridge/tunnel draws.
        _ => false,
    }
}

/// Preflight de `GrowTownWithTunnel` para la carretera municipal.
///
/// `OpenTTD` no consume RNG al decidir un túnel: primero comprueba la pendiente
/// y la continuación, busca la primera tesela al mismo nivel y recién después
/// ejecuta `CMD_BUILD_TUNNEL`. Mantener esa frontera sin sorteos es importante
/// porque un túnel que no se puede materializar debe dejar que la llamada siga
/// por `GrowTownWithRoad` con exactamente el mismo estado aleatorio.
fn generated_town_road_tunnel_end(
    map: &crate::map::Map,
    start: TileCoord,
    direction: u8,
    population: u32,
) -> Option<TileCoord> {
    let (start_slope, start_z) = tile_slope_and_z(map, start)?;
    if start_slope != generated_town_inclined_slope(direction) {
        return None;
    }

    let source = add_town_diag(start, reverse_town_diag_dir(direction));
    if generated_town_road_bits(map, source) & town_diag_dir_to_road_bits(direction) == 0 {
        return None;
    }

    // La primera rama de la referencia distingue entre un túnel bajo una
    // montaña continua y un túnel corto bajo una obstrucción. En ambos casos
    // `CanRoadContinueIntoNextTile` es una consulta sin RNG.
    let continues_at_start = generated_town_road_can_continue_after_bridge(map, start, direction);
    let max_length = if continues_at_start {
        for offset in 0..4_i32 {
            let tile = TileCoord::new(
                start.x + crate::map::diag_dir_offset(direction).0 * offset,
                start.y + crate::map::diag_dir_offset(direction).1 * offset,
            );
            let slope = tile_slope_and_z(map, tile)?.0;
            let one_corner_raised = matches!(
                slope & 0x0F,
                SLOPE_CORNER_W | SLOPE_CORNER_S | SLOPE_CORNER_E | SLOPE_CORNER_N
            ) && slope & SLOPE_STEEP == 0;
            if slope != generated_town_inclined_slope(direction)
                && slope & SLOPE_STEEP == 0
                && !one_corner_raised
            {
                return None;
            }
        }
        generated_town_mountain_tunnel_length_cap(population)
    } else {
        5
    };

    let (dx, dy) = crate::map::diag_dir_offset(direction);
    let mut tunnel_length = 0_usize;
    let mut end = start;
    loop {
        if tunnel_length >= max_length {
            return None;
        }
        tunnel_length = tunnel_length.saturating_add(1);
        end = TileCoord::new(end.x + dx, end.y + dy);
        let (_, end_z) = tile_slope_and_z(map, end)?;
        if end_z == start_z {
            break;
        }
    }

    if tunnel_length == 1 || !generated_town_road_can_continue_after_bridge(map, end, direction) {
        return None;
    }

    // `CMD_BUILD_TUNNEL` limpia sólo las dos bocas. En la generación de
    // pueblos esas teselas deben ser clear/trees (agua real y objetos se
    // rechazan con `NoWater`); el interior queda intacto bajo la montaña.
    let endpoint_clearable = |tile: TileCoord, start_endpoint: bool| {
        map.get(tile).is_some_and(|tile| {
            (matches!(tile.kind, TileKind::Grass | TileKind::Forest)
                && !has_tile_water_ground(tile))
                || (tile.kind == TileKind::Road
                    && tile.m1 == crate::company::OWNER_TOWN_M1
                    && tile.m3 == TOWN_ROAD_NO_TRAM_OWNER
                    && tile.m8 == TOWN_ROAD_INVALID_TRAM_TYPE
                    && crate::road_type::tram_track_bits(&tile) == 0
                    // `CmdBuildTunnel` passes `Auto` to `ClearTile_Road` for
                    // the exit. A two-bit block must first be removed
                    // explicitly and therefore rejects the preflight. The
                    // source mouth is already the road selected by the town
                    // walker and remains eligible for the native command.
                    && (start_endpoint || (tile.m5 & 0x0F).is_power_of_two()))
        })
    };
    endpoint_clearable(start, true)
        .then_some(end)
        .filter(|_| endpoint_clearable(end, false))
}

/// Escribe `MakeRoadTunnel` para las dos bocas de un túnel municipal.
///
/// `MakeRoadTunnel` usa `MAPT=0x90`, el dueño de carretera en `MAP7` (la
/// representación no-normal de `SetRoadOwner`) y `INVALID_ROADTYPE` en la
/// capa de tranvía. Las teselas interiores no se modifican: `OpenTTD` sólo
/// persiste las dos entradas y reconstruye el tramo a partir de ellas.
fn materialize_generated_town_road_tunnel(
    map: &mut crate::map::Map,
    start: TileCoord,
    end: TileCoord,
    direction: u8,
) -> bool {
    let Some((start_slope, _)) = tile_slope_and_z(map, start) else {
        return false;
    };
    let Some((end_slope, _)) = tile_slope_and_z(map, end) else {
        return false;
    };

    // `CMD_BUILD_TUNNEL` despeja las dos bocas antes de escribirlas. Esto no
    // cambia la geometría, pero sí reinicia los bytes de clear y sus vecinos
    // de agua; hacerlo antes de la terraformación conserva esos efectos.
    if !clear_generated_town_tunnel_endpoint(map, start)
        || !clear_generated_town_tunnel_endpoint(map, end)
    {
        return false;
    }

    // Si la pendiente opuesta no es complementaria, OpenTTD excava la boca
    // final con `CMD_TERRAFORM_LAND(end_tileh & start_tileh, false)`. La
    // rutina de terraformación ya implementa la recursión, los tiles sucios,
    // el coste y la limpieza atómica usados por LevelTownLand.
    if complement_slope(start_slope) != end_slope {
        let edges = end_slope & start_slope & SLOPE_CORNER_MASK;
        if edges == 0 || !try_level_generated_tunnel_land(map, end, edges, false) {
            return false;
        }
    }

    for (coord, portal_direction) in [
        (start, direction & 3),
        (end, reverse_town_diag_dir(direction)),
    ] {
        let Some(mut tile) = map.get(coord) else {
            return false;
        };
        tile.kind = TileKind::RoadTunnel;
        tile.mapt = (tile.mapt & 0x0F) | 0x90;
        tile.m1 = crate::company::OWNER_TOWN_M1;
        tile.m2 = 0;
        tile.m2_hi = 0;
        tile.m3 = 0;
        tile.m3hi = 0;
        tile.m5 = 0x04 | portal_direction;
        tile.m6 = 0;
        tile.m7 = crate::company::OWNER_TOWN_M1;
        tile.m8 = TOWN_ROAD_INVALID_TRAM_TYPE;
        if map.set_tile(coord, tile).is_err() {
            return false;
        }
    }
    true
}

/// Genera todas las teselas de un puente recto sin depender del índice lineal
/// del mapa. La llamada llega siempre por un eje diagonal; el `Option` protege
/// los bordes antes de que se materialice cualquier byte.
fn generated_town_bridge_line(
    map: &crate::map::Map,
    start: TileCoord,
    end: TileCoord,
    direction: u8,
) -> Option<Vec<TileCoord>> {
    let mut line = vec![start];
    let mut current = start;
    while current != end {
        current = add_town_diag(current, direction);
        map.get(current)?;
        line.push(current);
    }
    Some(line)
}

/// Parte sin RNG de `CMD_BUILD_BRIDGE` que necesita el generador de pueblos.
///
/// Cubre una rampa inicial plana sobre río/canal y la rampa inclinada nativa
/// compatible con la dirección de salida. Vías, calles unidireccionales y el
/// resto de geometrías/foundations siguen en RMAP-030 para no inventar
/// resultados de `CheckBridgeSlope`.
/// Si esta comprobación falla después de que `GrowTownWithBridge` ya pasó sus
/// gates, el caller conserva los 23 sorteos de tipo que hace el comando C++.
fn generated_town_road_bridge_command_supported(
    map: &crate::map::Map,
    line: &[TileCoord],
    direction: u8,
) -> bool {
    let Some((&start, rest)) = line.split_first() else {
        return false;
    };
    let Some((&end, middle)) = rest.split_last() else {
        return false;
    };
    if middle.is_empty() {
        return false;
    }

    let (Some(start_tile), Some(end_tile)) = (map.get(start), map.get(end)) else {
        return false;
    };
    // `CMD_BUILD_BRIDGE` puede usar una costa como rampa: el comando limpia
    // ese `MP_WATER` durante el pase Execute y luego escribe
    // `MakeRoadBridgeRamp`. Excluirla aquí hacía que el walker consumiera los
    // 23 sorteos de tipo y volviera a una calle normal, a diferencia del
    // oráculo que acepta el primer puente válido.
    let ramp_clearable = |tile: crate::map::Tile| {
        matches!(tile.kind, TileKind::Grass | TileKind::Forest)
            || (tile.kind == TileKind::Water && is_coast_tile(tile))
    };
    if !ramp_clearable(start_tile) || !ramp_clearable(end_tile) {
        return false;
    }
    let Some((start_slope, start_z)) = tile_slope_and_z(map, start) else {
        return false;
    };
    let Some((end_slope, end_z)) = tile_slope_and_z(map, end) else {
        return false;
    };

    // `CmdBuildBridge` canoniza los extremos por índice lineal antes de
    // aplicar `CheckBridgeSlope`: el extremo menor es la pieza NORTH y el
    // mayor la SOUTH, aun cuando `GrowTownWithBridge` esté caminando en el
    // sentido opuesto. Cada cimiento puede cambiar tanto la pendiente de la
    // rampa como su nivel efectivo; omitirlo acepta puentes que el comando
    // nativo rechaza cuando las cabezas no quedan a la misma altura (por
    // ejemplo, la semilla ártica 1330935380).
    let (width, _) = map.dimensions();
    let linear_index = |tile: TileCoord| {
        u64::try_from(tile.y)
            .unwrap_or(0)
            .saturating_mul(u64::from(width))
            .saturating_add(u64::try_from(tile.x).unwrap_or(0))
    };
    let axis_x = matches!(direction & 3, 0 | 2);
    let (north_slope, north_z, south_slope, south_z) = if linear_index(start) <= linear_index(end) {
        (start_slope, start_z, end_slope, end_z)
    } else {
        (end_slope, end_z, start_slope, start_z)
    };
    let (north_surface, north_foundation_dz) = bridge_surface_slope_and_z(north_slope, axis_x);
    let (south_surface, south_foundation_dz) = bridge_surface_slope_and_z(south_slope, axis_x);
    let north_valid_inclined = if axis_x { SLOPE_NE } else { SLOPE_NW };
    let south_valid_inclined = if axis_x { SLOPE_SW } else { SLOPE_SE };
    if (north_surface != 0 && north_surface != north_valid_inclined)
        || (south_surface != 0 && south_surface != south_valid_inclined)
        || u16::from(north_z).saturating_add(u16::from(north_foundation_dz))
            != u16::from(south_z).saturating_add(u16::from(south_foundation_dz))
    {
        return false;
    }
    let bridge_level = u16::from(north_z).saturating_add(u16::from(north_foundation_dz));

    middle.iter().copied().all(|middle_tile| {
        let Some(tile) = map.get(middle_tile) else {
            return false;
        };
        tile.mapt & 0x0C == 0
            && generated_town_bridge_crosses_water(tile, start_slope == 0)
            && tile_slope_and_z(map, middle_tile).is_some_and(|(_, z)| u16::from(z) <= bridge_level)
    })
}

/// Escribe los bytes de `MakeRoadBridgeRamp` y `SetBridgeMiddle` para un
/// puente municipal. Los extremos no llevan `TownID` en MAP2: una rampa usa
/// MAP7 como dueño de la capa road, a diferencia de `MakeRoadNormal`.
fn materialize_generated_town_road_bridge(
    map: &mut crate::map::Map,
    line: &[TileCoord],
    direction: u8,
    bridge_type: BridgeType,
) -> bool {
    if line.len() < 3 {
        return false;
    }
    // `CMD_BUILD_BRIDGE` ejecuta `CMD_LANDSCAPE_CLEAR` sobre ambas rampas
    // antes de `MakeRoadBridgeRamp`. `DoClearSquare` también reinicia el
    // estado non-flooding de las ocho teselas de agua vecinas; omitir esa
    // limpieza deja bytes de inundación viejos alrededor de la orilla.
    clear_neighbour_non_flooding_states(map, line[0]);
    clear_neighbour_non_flooding_states(map, *line.last().expect("line has an end ramp"));
    let axis_y = matches!(direction & 3, 1 | 3);
    for (index, coord) in line.iter().copied().enumerate() {
        let Some(mut tile) = map.get(coord) else {
            return false;
        };
        let is_ramp = index == 0 || index + 1 == line.len();
        if is_ramp {
            let ramp_direction = if index == 0 {
                direction
            } else {
                reverse_town_diag_dir(direction)
            };
            tile.kind = TileKind::RoadBridge;
            tile.mapt = (tile.mapt & 0x0F) | 0x90;
            tile.m1 = crate::company::OWNER_TOWN_M1;
            tile.m2 = 0;
            tile.m2_hi = 0;
            tile.m3 = 0;
            tile.m3hi = 0;
            tile.m5 = 0x80 | 0x04 | (ramp_direction & 0x03);
            tile.m6 = (tile.m6 & 0x03) | ((bridge_type.as_u8() & 0x0F) << 2);
            // `SetRoadOwner` usa MAP7 en una rampa (no MAP1).
            tile.m7 = crate::company::OWNER_TOWN_M1;
            tile.m8 = TOWN_ROAD_INVALID_TRAM_TYPE;
        } else {
            tile.mapt = set_bridge_middle_mapt(tile.mapt, axis_y);
            // `SetBridgeMiddle` sólo marca el eje en MAPT. El tipo de puente
            // pertenece a las rampas (`MP_TUNNELBRIDGE`); copiarlo al agua o
            // a la vía bajo el vano modifica bytes que OpenTTD conserva.
        }
        if map.set_tile(coord, tile).is_err() {
            return false;
        }
    }
    true
}

/// Subconjunto de `GrowTownWithBridge` que inicia una calle municipal sobre
/// un cruce de agua corto desde una rampa plana o la pendiente nativa
/// compatible. La selección de tipo sucede *después* de encontrar ambas
/// orillas y validar la continuidad: mover el `RandomRange` antes de esos
/// gates desalinearía todas las casas y pueblos posteriores.
fn try_grow_generated_town_road_bridge(
    map: &mut crate::map::Map,
    start: TileCoord,
    direction: Option<u8>,
    population: u32,
    context: &GeneratedTownGrowthContext,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> bool {
    let Some(direction) = direction else {
        return false;
    };
    let Some((start_slope, _)) = tile_slope_and_z(map, start) else {
        return false;
    };
    if start_slope != 0 && start_slope & generated_town_inclined_slope(direction) != 0 {
        return false;
    }

    let source = add_town_diag(start, reverse_town_diag_dir(direction));
    if generated_town_road_bits(map, source) & town_diag_dir_to_road_bits(direction) == 0 {
        return false;
    }

    let mut bridge_length = 0_usize;
    let bridge_length_cap = generated_town_bridge_length_cap(start_slope, population);
    let mut end = start;
    loop {
        if bridge_length >= bridge_length_cap {
            return false;
        }
        bridge_length = bridge_length.saturating_add(1);
        end = add_town_diag(end, direction);
        let Some(tile) = map.get(end) else {
            return false;
        };
        if !generated_town_bridge_crosses_water(tile, start_slope == 0) {
            break;
        }
    }
    if bridge_length == 1 || !generated_town_road_can_continue_after_bridge(map, end, direction) {
        return false;
    }

    // `GrowTownWithBridge` deja fuera una rampa inclinada si ya existe otra
    // rampa vial paralela dentro de la espiral de búsqueda. Aunque el vano y
    // el catálogo sean válidos, el comando nativo rechaza el puente sin tomar
    // ningún `RandomRange`; omitir esta frontera construye un puente extra y
    // desplaza todo el RNG urbano (RMAP-128).
    if start_slope != 0
        && generated_town_has_parallel_road_bridge(map, start, bridge_length, direction)
    {
        return false;
    }

    let Some(line) = generated_town_bridge_line(map, start, end, direction) else {
        return false;
    };
    let middle_len = u16::try_from(line.len().saturating_sub(2)).unwrap_or(u16::MAX);
    let command_supported = generated_town_road_bridge_command_supported(map, &line, direction);
    for _ in 0..GENERATED_TOWN_BRIDGE_TYPE_ATTEMPTS {
        let bridge_type = BridgeType::from_u8(
            u8::try_from(rng.random_range(GENERATED_TOWN_BRIDGE_RANDOM_TYPE_LIMIT)).unwrap_or(0),
        )
        .unwrap_or(BridgeType::Wooden);
        if command_supported
            && bridge_available_in(
                &context.bridge_spec_catalog,
                bridge_type,
                context.calendar_year,
                middle_len,
            )
            && materialize_generated_town_road_bridge(map, &line, direction, bridge_type)
        {
            return true;
        }
    }

    false
}

/// Datos de partida que `GrowTown` consulta al filtrar el catálogo de casas.
/// Agruparlos conserva una frontera explícita entre el walker y el contexto de
/// generación, sin convertir la fixture en un supuesto de clima o fecha.
#[derive(Clone)]
struct GeneratedTownGrowthContext {
    climate: crate::world_gen::Climate,
    snow_line_height: u8,
    calendar_year: u32,
    /// El comando de puente del pueblo consulta el catálogo activo, no una
    /// tabla fija: Action0 Bridges ya pudo alterar sus límites antes de la
    /// generación del mapa.
    bridge_spec_catalog: Vec<BridgeSpecDef>,
}

/// Parte vial de `GrowTownInTile` usada por la fundación procedural.
///
/// Esta pieza cubre carretera y la primera bifurcación de casa: una carretera
/// ya existente se recorre con `RandomDiagDir`; al llegar a clear se aplican
/// los dos `Chance16` y se construye el siguiente bloque con la máscara nativa.
/// Para una casa usa el pool mutable de `TryBuildTownHouse`, sin sustituir sus
/// sorteos por una heurística local.
///
/// Mantener las ramas juntas protege el orden de consumo RNG de `GrowTownInTile`;
/// partirlas por caso hace demasiado fácil introducir una frontera accidental.
#[allow(clippy::too_many_lines)]
fn grow_generated_town_road_in_tile(
    map: &mut crate::map::Map,
    town: &mut Town,
    tile: TileCoord,
    cur_rb: u8,
    target_dir: Option<u8>,
    context: &GeneratedTownGrowthContext,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> GeneratedRoadGrowthResult {
    if cur_rb == 0 {
        let Some(mut target_dir) = target_dir else {
            return GeneratedRoadGrowthResult::SearchStopped;
        };
        // `GrowTownInTile` sortea `LevelTownLand` antes de consultar
        // `IsRoadAllowedHere`, incluso cuando la tesela ya está ocupada por
        // una casa, agua o queda fuera del mapa. `LevelTownLand` falla sin
        // mutar esos tipos, pero el `Chance16` sigue siendo observable en el
        // flujo RNG (RMAP-135).
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
        let continuation_allowed =
            generated_town_road_allowed_here(map, town, continuation, target_dir, rng);
        if !continuation_allowed {
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
        let rcmd = clean_up_generated_town_road_bits(map, tile, rcmd);
        if rcmd == 0 {
            return GeneratedRoadGrowthResult::SearchStopped;
        }
        if try_grow_generated_town_road_bridge(
            map,
            tile,
            Some(target_dir),
            town.population,
            context,
            rng,
        ) {
            return GeneratedRoadGrowthResult::Road(tile);
        }
        if let Some(end) = generated_town_road_tunnel_end(map, tile, target_dir, town.population)
            && materialize_generated_town_road_tunnel(map, tile, end, target_dir)
        {
            return GeneratedRoadGrowthResult::Road(tile);
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
        let rcmd = clean_up_generated_town_road_bits(
            map,
            tile,
            town_diag_dir_to_road_bits(reverse_town_diag_dir(dir)),
        );
        if rcmd == 0 {
            return GeneratedRoadGrowthResult::SearchStopped;
        }
        if try_grow_generated_town_road_bridge(map, tile, Some(dir), town.population, context, rng)
        {
            return GeneratedRoadGrowthResult::Road(tile);
        }
        // `GrowTownWithRoad` no puede modificar una boca de túnel/puente;
        // `CMD_BUILD_ROAD` falla allí aunque `CleanUpRoadBits` haya dejado un
        // bit conectivo. Sólo una carretera normal acepta esta extensión
        // parcial (RMAP-128, llamada 109 de la semilla ártica).
        if map.get_kind(tile) == Some(TileKind::Road)
            && add_generated_town_road_bits_to_map(map, tile, rcmd, town.id)
        {
            return GeneratedRoadGrowthResult::Road(tile);
        }
        return GeneratedRoadGrowthResult::SearchStopped;
    }

    // `GrowTownInTile` maneja una boca vial antes de sortear la dirección
    // aleatoria, pero sólo después de la rama de carretera parcial anterior.
    // Si no se respeta ese orden, una boca con `target_dir` pendiente consume
    // una palabra de RNG que el nativo no consume (RMAP-128).
    if matches!(
        map.get_kind(tile),
        Some(TileKind::RoadBridge | TileKind::RoadTunnel)
    ) {
        if (target_dir.is_some() || chance16(rng, 1, 2))
            && let Some(other_end) = generated_town_road_tunnel_bridge_other_end(map, tile)
        {
            return GeneratedRoadGrowthResult::TunnelBridge(other_end);
        }
        return GeneratedRoadGrowthResult::Continue;
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
    if map.get(house_tile).is_none() || is_water_ground(map, house_tile) {
        return GeneratedRoadGrowthResult::Continue;
    }

    // TL_ORIGINAL reserva la casa con probabilidad 6/10 cuando la carretera
    // puede seguir, o siempre si `IsRoadAllowedHere` la rechaza. La esquina
    // de una curva no tiene target vial y mantiene `allow_house = true`.
    let allow_house = road_target_dir.is_none_or(|road_target_dir| {
        let allowed = generated_town_road_allowed_here(map, town, house_tile, road_target_dir, rng);
        let chance = if allowed { chance16(rng, 6, 10) } else { false };
        !allowed || chance
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
            context.snow_line_height,
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
    let rcmd = clean_up_generated_town_road_bits(map, tile, target_bits);
    if rcmd == 0 {
        return GeneratedRoadGrowthResult::SearchStopped;
    }
    if try_grow_generated_town_road_bridge(
        map,
        tile,
        Some(target_dir),
        town.population,
        context,
        rng,
    ) {
        return GeneratedRoadGrowthResult::Road(tile);
    }
    if let Some(end) = generated_town_road_tunnel_end(map, tile, target_dir, town.population)
        && materialize_generated_town_road_tunnel(map, tile, end, target_dir)
    {
        return GeneratedRoadGrowthResult::Road(tile);
    }
    if add_generated_town_road_bits_to_map(map, tile, rcmd, town.id) {
        GeneratedRoadGrowthResult::Road(tile)
    } else {
        GeneratedRoadGrowthResult::SearchStopped
    }
}

/// Recorre una sola llamada a `GrowTown` de la fundación procedural.
///
/// Devuelve la tesela de la carretera o casa que consiguió crear. El
/// constructor de `DoCreateTown` lo invoca dentro de la misma frontera RNG de
/// `GenerateTowns`; las variantes fuera del subconjunto vanilla siguen
/// documentadas en RMAP-030/RMAP-032.
fn grow_generated_town_road_once(
    map: &mut crate::map::Map,
    town: &mut Town,
    context: &GeneratedTownGrowthContext,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> Option<TileCoord> {
    let mut tile = town.pos;
    for &(dx, dy) in &TOWN_GROWTH_COORD_MOD {
        let bits = generated_town_road_bits(map, tile);
        if bits != 0 {
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
    context: &GeneratedTownGrowthContext,
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
        let result =
            grow_generated_town_road_in_tile(map, town, tile, cur_rb, target_dir, context, rng);
        let mut forced_bridge_direction = false;
        match result {
            GeneratedRoadGrowthResult::Road(pos) | GeneratedRoadGrowthResult::House(pos) => {
                return Some(pos);
            }
            GeneratedRoadGrowthResult::TunnelBridge(other_end) => {
                // `GrowTownInTile` actualiza el puntero de tesela y devuelve
                // `Continue`; luego `GrowTownAtRoad` observa la boca opuesta
                // y elige su dirección exterior sin consumir `RandomDiagDir`.
                tile = other_end;
                let outward = generated_town_road_tunnel_bridge_direction(map, tile)
                    .map(reverse_town_diag_dir)?;
                target_dir = Some(outward);
                forced_bridge_direction = true;
            }
            GeneratedRoadGrowthResult::SearchStopped => iterations = 0,
            GeneratedRoadGrowthResult::Continue => {}
        }

        // Incluso cuando la llamada termina en `SearchStopped`, el walker
        // nativo no sortea una salida de la boca actual: observa que la
        // tesela sigue siendo túnel/puente y fuerza la dirección exterior.
        // Esto ocurre, por ejemplo, cuando la extensión parcial no puede
        // materializarse en la boca (RMAP-128, llamada 109).
        if !forced_bridge_direction
            && let Some(bridge_dir) = generated_town_road_tunnel_bridge_direction(map, tile)
        {
            target_dir = Some(reverse_town_diag_dir(bridge_dir));
            forced_bridge_direction = true;
        }

        if !forced_bridge_direction {
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
        }

        let dir = target_dir?;
        tile = add_town_diag(tile, dir);
        if generated_town_road_is_foreign(map, tile, town.id) {
            return None;
        }
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
fn choose_generated_town_house_candidate(
    town: &Town,
    map: &crate::map::Map,
    tile: TileCoord,
    climate: crate::world_gen::Climate,
    snow_line_height: u8,
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
    let required_zones =
        (1_u16 << (zone as u8)) | climate_zone_mask_at_snow_line(climate, max_z, snow_line_height);
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
        // `TownCanBePlacedHere` excluye los árboles sobre suelo rough
        // (`GetTreeGround() == TREE_GROUND_ROUGH`) del cuadro 5×5. El ground
        // vive en MAP2 bits 6..8, incluido el bit que se serializa en
        // `m2_hi`; leer sólo el byte bajo convierte `RoughSnow` (0x0100) en
        // Grass y permite una fundación que OpenTTD rechaza.
        TileKind::Forest => {
            let m2 = u16::from(candidate.m2) | (u16::from(candidate.m2_hi) << 8);
            ((m2 >> 6) & 0x07) != 1
        }
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
    // `GetClosestWaterDistance` recorre un rombo de distancia Manhattan, no
    // el perímetro cuadrado (Chebyshev). El detalle cambia el punto elegido
    // por `FindNearestGoodCoastalTownSpot` cuando varios interiores son
    // válidos, aunque ninguna de estas búsquedas consume RNG.
    let max_x = i32::try_from(map_w).unwrap_or(i32::MAX).saturating_sub(1);
    let max_y = i32::try_from(map_h).unwrap_or(i32::MAX).saturating_sub(1);
    for distance in 1..0x7F_u32 {
        let d = i32::try_from(distance).unwrap_or(i32::MAX);
        let mut x = center.x;
        let mut y = center.y.saturating_sub(d);
        for (dx, dy) in WATER_DISTANCE_DIAMOND_DIRS {
            for _ in 0..distance {
                if x >= 0
                    && y >= 0
                    && x < max_x
                    && y < max_y
                    && is_water_ground(map, TileCoord::new(x, y))
                {
                    return distance;
                }
                x = x.saturating_add(dx);
                y = y.saturating_add(dy);
            }
        }
    }
    0x7F
}

fn is_water_ground(map: &crate::map::Map, tile: TileCoord) -> bool {
    map.get(tile).is_some_and(has_tile_water_ground)
}

/// Espejo de `MirrorRoadBits`: una semicarretera cuesta arriba se completa
/// sobre el eje opuesto antes de que `CMD_BUILD_ROAD` la escriba.
const fn mirror_generated_town_road_bits(bits: u8) -> u8 {
    ((bits & 0x03) << 2) | ((bits & 0x0C) >> 2)
}

const fn generated_town_road_bits_are_straight(bits: u8) -> bool {
    matches!(bits, ROAD_BITS_AXIS_X | ROAD_BITS_AXIS_Y)
}

/// Pendiente que consulta `CheckRoadSlope` para un comando de carretera.
///
/// Una pendiente empinada se reduce a su esquina más alta antes de indexar
/// las tablas nativas. `tile_slope_and_z` conserva el bit `SLOPE_STEEP`, pero
/// las alturas de las cuatro esquinas permiten recuperar sin ambigüedad la
/// esquina usada por `SlopeWithOneCornerRaised(GetHighestSlopeCorner(...))`.
fn generated_town_road_command_slope(map: &crate::map::Map, tile: TileCoord) -> Option<u8> {
    let (slope, _) = tile_slope_and_z(map, tile)?;
    if slope & SLOPE_STEEP == 0 {
        return Some(slope);
    }

    let corners = [
        (SLOPE_CORNER_N, tile),
        (SLOPE_CORNER_W, TileCoord::new(tile.x + 1, tile.y)),
        (SLOPE_CORNER_E, TileCoord::new(tile.x, tile.y + 1)),
        (SLOPE_CORNER_S, TileCoord::new(tile.x + 1, tile.y + 1)),
    ];
    corners
        .into_iter()
        .filter_map(|(corner, coord)| map.get(coord).map(|candidate| (corner, candidate.height)))
        .max_by_key(|&(_, height)| height)
        .map(|(corner, _)| corner)
}

/// Parte sin costes de `CheckRoadSlope` para `CMD_BUILD_ROAD` durante
/// `GenerateTowns`.
///
/// Las calles municipales no llevan tranvía ni un segundo tipo de carretera,
/// por lo que `other == ROAD_NONE`. El valor devuelto son los bits nuevos (no
/// la unión con `existing`), exactamente como el parámetro mutable `pieces`
/// de `OpenTTD`. Esto evita publicar una media carretera imposible en una
/// pendiente: el comando nativo la completa a un eje recto o la rechaza.
fn normalize_generated_town_road_command_bits(
    map: &crate::map::Map,
    tile: TileCoord,
    requested: u8,
    existing: u8,
) -> Option<u8> {
    let mut pieces = requested & !existing;
    if pieces == 0 {
        return None;
    }
    let slope = usize::from(generated_town_road_command_slope(map, tile)?);
    if slope == 0 {
        return Some(pieces);
    }

    let mut combined = existing | pieces;
    if GENERATED_INVALID_ROAD_BITS_ON_LEVELLED_SLOPE.get(slope)? & combined == 0 {
        return Some(pieces);
    }

    pieces |= mirror_generated_town_road_bits(pieces);
    combined = existing | pieces;
    (generated_town_road_bits_are_straight(combined)
        && GENERATED_INVALID_ROAD_BITS_ON_STRAIGHT_SLOPE.get(slope)? & combined == 0)
        .then_some(pieces)
}

/// Escribe `MakeRoadNormal` para una calle creada durante `GenerateTowns`.
///
/// Las rutas de comando interactivas usan la compañía activa. Durante la
/// generación C++ cambia a `OWNER_TOWN`, fija el índice del pueblo y conserva
/// el sentinel de tram ausente; usar esos bytes evita introducir una compañía
/// humana o una capa de tranvía falsa.
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
    let replacing_non_road = tile.kind != TileKind::Road;
    let existing = if replacing_non_road {
        0
    } else {
        tile.m5 & 0x0F
    };
    let Some(pieces) = normalize_generated_town_road_command_bits(map, coord, road_bits, existing)
    else {
        return false;
    };
    if replacing_non_road {
        // `CMD_BUILD_ROAD` primero ejecuta `CMD_LANDSCAPE_CLEAR` sobre una
        // tesela de terreno. `ClearTile_Clear` termina en `DoClearSquare`,
        // que limpia el bit non-flooding de las ocho aguas vecinas aun cuando
        // la calle reemplaza inmediatamente el terreno. La escritura directa
        // debe conservar ese efecto lateral para costas generadas.
        clear_neighbour_non_flooding_states(map, coord);
    }
    let town = u16::try_from(town_id).unwrap_or(u16::MAX).to_le_bytes();
    tile.kind = TileKind::Road;
    // `SetTileType(MP_ROAD)` sólo reemplaza el nibble alto de `MAPT`. En
    // clima tropical el nibble bajo contiene `TropicZone` y debe sobrevivir
    // al despeje de la tesela y a `MakeRoadNormal`.
    tile.mapt = (tile.mapt & 0x0F) | 0x20;
    tile.m1 = crate::company::OWNER_TOWN_M1;
    tile.m2 = town[0];
    tile.m2_hi = town[1];
    tile.m3 = TOWN_ROAD_NO_TRAM_OWNER;
    tile.m3hi = 0;
    tile.m5 = existing | pieces;
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
    let Some(pieces) =
        normalize_generated_town_road_command_bits(map, coord, road_bits, tile.m5 & 0x0F)
    else {
        return false;
    };
    tile.m5 |= pieces;
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
        let context = GeneratedTownGrowthContext {
            climate: Climate::Temperate,
            snow_line_height: state.snow_line_height,
            calendar_year: 1950,
            bridge_spec_catalog: state.bridge_spec_catalog.clone(),
        };
        grow_generated_town_road_once(&mut state.map, town, &context, rng)
    }

    /// Estado justo después del bootstrap de la primera ciudad de la segunda
    /// seed. Se comparte entre fronteras para que el test de `CleanUpRoadBits`
    /// no esconda un consumo previo de `TSZ_RANDOM` o de `GenRandomRoadBits`.
    fn second_seed_first_city_after_bootstrap() -> (GameState, Town, Randomizer) {
        let mut state = clear_phase_state(1_330_935_379);
        let mut rng = Randomizer {
            state: [394_065_499, 3_120_157_675],
        };
        let budget = initial_town_house_budget(&mut rng, true);
        let bootstrap = initial_town_growth_bootstrap(&state.map, TileCoord::new(43, 15), &mut rng)
            .expect("bootstrap second seed");
        assert!(write_generated_town_road(
            &mut state,
            bootstrap.pos,
            bootstrap.bits,
            0,
        ));
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(43, 15),
            num_houses: u16::try_from(budget).unwrap_or(u16::MAX),
            layout: TownLayout::Original,
            ..Town::default()
        };
        update_town_radius(&mut town);
        assert_eq!(budget, 36);
        assert_eq!(bootstrap.pos, TileCoord::new(43, 15));
        assert_eq!(bootstrap.bits, 0x0C);
        assert_eq!(rng.state, [2_880_850_169, 3_894_055_467]);
        (state, town, rng)
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
            industry_platform: 1,
            multiple_industry_per_town: false,
        };

        assert_eq!(
            select_random_town_site(&mut ctx, &[], RANDOM_TOWN_ATTEMPTS),
            Some(TileCoord::new(47, 23))
        );
        assert_eq!(ctx.rng.state, [2_945_732_258, 1_049_486_831]);
    }

    #[test]
    fn closest_water_distance_uses_manhattan_diamond_not_square_ring() {
        let mut map = Map::new_flat(16, 16, 0);
        let water = TileCoord::new(7, 7);
        crate::map::make_water_tile(&mut map, water, crate::map::WaterClass::Sea)
            .expect("water tile");

        // El agua está a dos pasos Chebyshev, pero cuatro Manhattan. El
        // recorrido de `GetClosestWaterDistance` de OpenTTD devuelve cuatro.
        assert_eq!(closest_water_distance(&map, TileCoord::new(5, 5)), 4);
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
            industry_platform: 1,
            multiple_industry_per_town: false,
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
            industry_platform: 1,
            multiple_industry_per_town: false,
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
    fn town_road_command_completes_uphill_half_road_before_writing() {
        // `CheckRoadSlope` transforma ROAD_SE en ROAD_Y sobre SLOPE_NW.
        // Sin esa normalización la tesela parece válida hasta que el walker
        // vuelve por ella varias llamadas después y entonces cambia el RNG.
        let mut map = Map::new_flat(8, 8, 0);
        let tile = TileCoord::new(3, 3);
        map.set_height(tile, 1).expect("north high corner");
        map.set_height(TileCoord::new(4, 3), 1)
            .expect("west high corner");
        assert_eq!(tile_slope_and_z(&map, tile), Some((SLOPE_NW, 0)));

        assert_eq!(
            normalize_generated_town_road_command_bits(&map, tile, ROAD_SE, 0),
            Some(ROAD_BITS_AXIS_Y)
        );
        assert!(write_generated_town_road_to_map(&mut map, tile, ROAD_SE, 0));
        assert_eq!(map.get(tile).expect("road").m5, ROAD_BITS_AXIS_Y);
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
    fn do_create_town_path_replays_first_city_until_next_name() {
        let mut state = clear_phase_state(1_330_935_378);
        // Frontera a la entrada de `DoCreateTown`: la selección costera ya
        // consumió su `RandomTile` y `GenerateTownName` pertenece al caller.
        let mut rng = Randomizer {
            state: [2_945_732_258, 1_049_486_831],
        };
        let mut centers = Vec::new();
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
            multiple_industry_per_town: false,
        };

        assert!(build_selected_town_with_generated_growth(
            &mut ctx,
            &mut centers,
            TileCoord::new(47, 23),
            0,
            true,
        ));

        assert_eq!(centers, [TileCoord::new(47, 23)]);
        let town = ctx.state.towns.first().expect("first generated town");
        // `DoCreateTown` retira las 22 casas temporales después de las 88
        // iteraciones: en el mapa quedan 29 edificios reales y la población
        // de GDB antes de generar el siguiente nombre.
        assert_eq!(town.num_houses, 29);
        assert_eq!(town.population, 1_181);
        assert_eq!(town.layout, TownLayout::Original);
        assert_eq!(ctx.rng.state, [3_221_856_382, 229_218_699]);

        let road = ctx
            .state
            .map
            .get(TileCoord::new(50, 26))
            .expect("final first-city junction");
        assert_eq!(road.kind, TileKind::Road);
        assert_eq!(road.m5 & 0x0F, 0x0F);
    }

    #[test]
    fn do_create_town_path_replays_second_seed_first_city_until_next_name() {
        let mut state = clear_phase_state(1_330_935_379);
        // Frontera tras `GenerateTownName` y `CreateRandomTown`: esta ciudad
        // usa presupuesto temporal 36, por lo que `DoCreateTown` llama 144
        // veces a `GrowTown` antes de retirar ese contador.
        let mut rng = Randomizer {
            state: [394_065_499, 3_120_157_675],
        };
        let mut centers = Vec::new();
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
            multiple_industry_per_town: false,
        };

        assert!(build_selected_town_with_generated_growth(
            &mut ctx,
            &mut centers,
            TileCoord::new(43, 15),
            0,
            true,
        ));

        assert_eq!(centers, [TileCoord::new(43, 15)]);
        let town = ctx.state.towns.first().expect("first generated town");
        assert_eq!(town.num_houses, 42);
        assert_eq!(town.population, 1_862);
        assert_eq!(town.layout, TownLayout::Original);
        assert_eq!(ctx.rng.state, [3_848_068_084, 748_657_221]);

        let house = ctx
            .state
            .map
            .get(TileCoord::new(41, 10))
            .expect("house n=134");
        assert_eq!(house.kind, TileKind::House);
        assert_eq!(house.m1, 71);
        assert_eq!(house.m8 & 0x0FFF, 16);
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
            crate::world_gen::DEF_SNOW_LINE_HEIGHT,
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
            crate::world_gen::DEF_SNOW_LINE_HEIGHT,
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
    fn generated_town_road_preserves_tropic_zone_nibble() {
        let mut state = GameState::new(8, 8);
        let coord = TileCoord::new(3, 4);
        // `TROPICZONE_DESERT` en una tesela clear antes de que `MakeRoadNormal`
        // cambie el nibble alto de MAPT.
        state
            .map
            .set_mapt_m5(coord, 0x21, 0)
            .expect("tropical zone fixture");
        assert!(write_generated_town_road(
            &mut state,
            coord,
            ROAD_BITS_AXIS_Y,
            0
        ));
        assert_eq!(state.map.get(coord).expect("road tile").mapt, 0x21);
    }

    #[test]
    fn flat_town_bridge_replays_rng_and_native_road_bridge_bytes() {
        // Tramo mínimo que aparece en la seed 1330935379: una rampa clear,
        // un único vano de río y la otra orilla clear. El estado RNG es el
        // inmediatamente anterior a `RandomRange(MAX_BRIDGES - 1)` del
        // oráculo; el tipo 0 (madera) se acepta en el primer intento.
        let mut state = GameState::new(10, 10);
        let source = TileCoord::new(4, 6);
        let start = TileCoord::new(4, 5);
        let middle = TileCoord::new(4, 4);
        let end = TileCoord::new(4, 3);
        assert!(write_generated_town_road(
            &mut state,
            source,
            town_diag_dir_to_road_bits(3),
            0,
        ));
        crate::map::make_water_tile(&mut state.map, middle, crate::map::WaterClass::River)
            .expect("river span");

        let context = GeneratedTownGrowthContext {
            climate: Climate::Temperate,
            snow_line_height: state.snow_line_height,
            calendar_year: 1950,
            bridge_spec_catalog: state.bridge_spec_catalog.clone(),
        };
        let mut rng = Randomizer {
            state: [653_263_232, 3_923_936_600],
        };
        assert!(try_grow_generated_town_road_bridge(
            &mut state.map,
            start,
            Some(3),
            0,
            &context,
            &mut rng,
        ));
        assert_eq!(rng.state, [1_994_895_143, 81_657_903]);

        let start_ramp = state.map.get(start).expect("start ramp");
        assert_eq!(start_ramp.kind, TileKind::RoadBridge);
        assert_eq!(start_ramp.mapt, 0x90);
        assert_eq!(start_ramp.m1, crate::company::OWNER_TOWN_M1);
        assert_eq!(start_ramp.m2, 0);
        assert_eq!(start_ramp.m2_hi, 0);
        assert_eq!(start_ramp.m3, 0);
        assert_eq!(start_ramp.m3hi, 0);
        assert_eq!(start_ramp.m5, 0x87);
        assert_eq!(start_ramp.m6, BridgeType::Wooden.as_u8() << 2);
        assert_eq!(start_ramp.m7, crate::company::OWNER_TOWN_M1);
        assert_eq!(start_ramp.m8, TOWN_ROAD_INVALID_TRAM_TYPE);

        let river_under_bridge = state.map.get(middle).expect("bridge middle");
        assert_eq!(river_under_bridge.kind, TileKind::Water);
        assert_eq!(river_under_bridge.mapt, 0x68);
        assert_eq!(river_under_bridge.m1, 0x51);
        assert_eq!(river_under_bridge.m6, 0);

        let end_ramp = state.map.get(end).expect("end ramp");
        assert_eq!(end_ramp.kind, TileKind::RoadBridge);
        assert_eq!(end_ramp.m5, 0x85);
        assert_eq!(
            generated_town_road_bits(&state.map, start),
            ROAD_BITS_AXIS_Y
        );
        assert!(generated_can_follow_town_road(&state.map, source, 3));
    }

    #[test]
    fn flat_town_bridge_rejects_heads_at_different_effective_heights() {
        // `CmdBuildBridge` exige que ambas cabezas queden al mismo nivel
        // después de aplicar sus cimientos. La previsualización anterior sólo
        // comparaba la altura del vano y aceptaba este caso, aunque el
        // comando nativo rechazaba el puente y continuaba con una calle.
        let mut state = GameState::new(10, 10);
        let source = TileCoord::new(4, 2);
        let start = TileCoord::new(4, 3);
        let middle = TileCoord::new(4, 4);
        let end = TileCoord::new(4, 5);
        assert!(write_generated_town_road(
            &mut state,
            source,
            town_diag_dir_to_road_bits(1),
            0,
        ));
        crate::map::make_water_tile(&mut state.map, middle, crate::map::WaterClass::River)
            .expect("river span");
        for vertex in [
            TileCoord::new(4, 3),
            TileCoord::new(5, 3),
            TileCoord::new(4, 4),
            TileCoord::new(5, 4),
        ] {
            state.map.set_height(vertex, 2).expect("start height");
        }
        assert_eq!(tile_slope_and_z(&state.map, start), Some((0, 2)));
        assert_eq!(tile_slope_and_z(&state.map, end), Some((0, 1)));

        assert!(!generated_town_road_bridge_command_supported(
            &state.map,
            &[start, middle, end],
            1,
        ));
    }

    #[test]
    fn flat_town_bridge_accepts_a_sloped_landing_ramp() {
        // RMAP-082, seed 1330935380: `GrowTownWithBridge` llega desde el
        // este, cruza un único río y construye la rampa opuesta sobre una
        // pendiente N. El preflight anterior exigía ambos extremos planos,
        // rechazaba el comando válido y agotaba los 23 sorteos de puente.
        let mut state = GameState::new(10, 10);
        let source = TileCoord::new(6, 4);
        let start = TileCoord::new(5, 4);
        let middle = TileCoord::new(4, 4);
        let end = TileCoord::new(3, 4);
        assert!(write_generated_town_road(
            &mut state,
            source,
            town_diag_dir_to_road_bits(0),
            0,
        ));
        crate::map::make_water_tile(&mut state.map, middle, crate::map::WaterClass::River)
            .expect("river span");
        state
            .map
            .set_height(end, 2)
            .expect("sloped landing north corner");
        assert_eq!(tile_slope_and_z(&state.map, start), Some((0, 1)));
        assert_eq!(tile_slope_and_z(&state.map, end), Some((SLOPE_CORNER_N, 1)));

        let line = [start, middle, end];
        assert!(generated_town_road_bridge_command_supported(
            &state.map, &line, 0
        ));

        let context = GeneratedTownGrowthContext {
            climate: Climate::Temperate,
            snow_line_height: state.snow_line_height,
            calendar_year: 1950,
            bridge_spec_catalog: state.bridge_spec_catalog.clone(),
        };
        let mut rng = Randomizer {
            state: [653_263_232, 3_923_936_600],
        };
        assert!(try_grow_generated_town_road_bridge(
            &mut state.map,
            start,
            Some(0),
            0,
            &context,
            &mut rng,
        ));
        // El primer tipo sorteado es madera, igual que en el comando nativo.
        assert_eq!(rng.state, [1_994_895_143, 81_657_903]);
        assert_eq!(state.map.get(start).expect("start ramp").m5, 0x84);
        assert_eq!(state.map.get(end).expect("sloped end ramp").m5, 0x86);
        assert_eq!(tile_slope_and_z(&state.map, end), Some((SLOPE_CORNER_N, 1)));
    }

    #[test]
    fn town_tunnel_clears_municipal_start_and_levels_steep_exit() {
        let mut map = Map::new_flat(16, 16, 1);
        let direction = 1;
        let source = TileCoord::new(5, 4);
        let start = TileCoord::new(5, 5);
        let end = TileCoord::new(5, 9);

        assert!(write_generated_town_road_to_map(
            &mut map,
            source,
            town_diag_dir_to_road_bits(direction),
            0,
        ));
        assert!(write_generated_town_road_to_map(
            &mut map,
            start,
            ROAD_BITS_AXIS_Y,
            0,
        ));

        // Start slope SE (6), three steep mountain tiles, then an exit with
        // the native 0x1B slope. The tunnel command lowers the shared S
        // corner of the exit before writing either portal.
        for (vertex, height) in [
            (TileCoord::new(5, 6), 2),
            (TileCoord::new(6, 6), 2),
            (TileCoord::new(6, 7), 4),
            (TileCoord::new(5, 7), 2),
            (TileCoord::new(6, 8), 4),
            (TileCoord::new(5, 8), 2),
            (TileCoord::new(6, 9), 3),
            (TileCoord::new(5, 9), 3),
            (TileCoord::new(6, 10), 3),
        ] {
            map.set_height(vertex, height).expect("terrain vertex");
        }
        assert_eq!(tile_slope_and_z(&map, start), Some((SLOPE_SE, 1)));
        assert_eq!(tile_slope_and_z(&map, end), Some((0x1B, 1)));
        assert_eq!(
            generated_town_road_tunnel_end(&map, start, direction, 0),
            Some(end)
        );

        // `CmdBuildTunnel` clears its exit with `Auto`; a municipal road
        // exposing both bits is not an implicit clear candidate. The source
        // mouth remains eligible for the direct materialization test below.
        let mut blocked_exit = map.clone();
        assert!(write_generated_town_road_to_map(
            &mut blocked_exit,
            end,
            ROAD_BITS_AXIS_Y,
            0,
        ));
        assert_eq!(
            generated_town_road_tunnel_end(&blocked_exit, start, direction, 0),
            None
        );

        assert!(materialize_generated_town_road_tunnel(
            &mut map, start, end, direction
        ));

        let start_portal = map.get(start).expect("start portal");
        assert_eq!(start_portal.kind, TileKind::RoadTunnel);
        assert_eq!(start_portal.mapt, 0x90);
        assert_eq!(start_portal.m5, 0x05);
        assert_eq!(start_portal.m1, crate::company::OWNER_TOWN_M1);
        assert_eq!(start_portal.m7, crate::company::OWNER_TOWN_M1);
        assert_eq!(start_portal.m8, TOWN_ROAD_INVALID_TRAM_TYPE);

        let end_portal = map.get(end).expect("end portal");
        assert_eq!(end_portal.kind, TileKind::RoadTunnel);
        assert_eq!(end_portal.m5, 0x07);
        assert_eq!(end_portal.m1, crate::company::OWNER_TOWN_M1);
        assert_eq!(end_portal.m7, crate::company::OWNER_TOWN_M1);
        assert_eq!(end_portal.m8, TOWN_ROAD_INVALID_TRAM_TYPE);
        assert_eq!(
            map.get(TileCoord::new(6, 10))
                .expect("levelled exit corner")
                .height,
            2
        );
        assert_eq!(tile_slope_and_z(&map, end), Some((0x1B, 1)));
    }

    #[test]
    fn town_bridge_accepts_native_sloped_start_ramp_and_rng_boundary() {
        // RMAP-082/086, 256² seed 1330935381: la llamada 452 de
        // `GrowTown` sale por una rampa NE, cruza un río de una tesela hacia
        // SW y elige hormigón en el primer `RandomRange(MAX_BRIDGES - 1)`.
        // Antes sólo se aceptaba un inicio plano y se construía una calle
        // normal sobre la rampa, desplazando toda la secuencia urbana.
        let mut state = GameState::new(10, 10);
        let source = TileCoord::new(3, 4);
        let start = TileCoord::new(4, 4);
        let middle = TileCoord::new(5, 4);
        let end = TileCoord::new(6, 4);
        assert!(write_generated_town_road(
            &mut state,
            source,
            town_diag_dir_to_road_bits(2),
            5,
        ));
        crate::map::make_water_tile(&mut state.map, middle, crate::map::WaterClass::River)
            .expect("river span");
        state
            .map
            .set_height(start, 2)
            .expect("north corner of the start ramp");
        state
            .map
            .set_height(TileCoord::new(start.x, start.y + 1), 2)
            .expect("east corner of the start ramp");
        assert_eq!(tile_slope_and_z(&state.map, start), Some((SLOPE_NE, 1)));

        let line = [start, middle, end];
        assert!(generated_town_road_bridge_command_supported(
            &state.map, &line, 2
        ));

        let context = GeneratedTownGrowthContext {
            climate: Climate::Temperate,
            snow_line_height: state.snow_line_height,
            calendar_year: 1950,
            bridge_spec_catalog: state.bridge_spec_catalog.clone(),
        };
        // Estado tras las cuatro decisiones del walker y justo antes de
        // seleccionar el tipo de puente en la referencia.
        let mut rng = Randomizer {
            state: [0xab7c_ac70, 0xb429_6331],
        };
        assert!(try_grow_generated_town_road_bridge(
            &mut state.map,
            start,
            Some(2),
            190,
            &context,
            &mut rng,
        ));
        assert_eq!(rng.state, [0x48c8_e6db, 0x156f_958d]);

        let start_ramp = state.map.get(start).expect("start ramp");
        assert_eq!(start_ramp.kind, TileKind::RoadBridge);
        assert_eq!(start_ramp.mapt, 0x90);
        assert_eq!(start_ramp.m5, 0x86);
        assert_eq!(start_ramp.m6, BridgeType::Concrete.as_u8() << 2);
        assert_eq!(start_ramp.m7, crate::company::OWNER_TOWN_M1);
        assert_eq!(start_ramp.m8, TOWN_ROAD_INVALID_TRAM_TYPE);

        let span = state.map.get(middle).expect("river span");
        assert_eq!(span.kind, TileKind::Water);
        assert_eq!(span.mapt, 0x64);

        let end_ramp = state.map.get(end).expect("end ramp");
        assert_eq!(end_ramp.kind, TileKind::RoadBridge);
        assert_eq!(end_ramp.m5, 0x84);
        assert_eq!(end_ramp.m6, BridgeType::Concrete.as_u8() << 2);
        assert_eq!(tile_slope_and_z(&state.map, start), Some((SLOPE_NE, 1)));
    }

    #[test]
    fn flat_town_bridge_keeps_nonwood_type_on_ramps_only() {
        let mut map = Map::new_flat(8, 8, 1);
        let line = [
            TileCoord::new(3, 5),
            TileCoord::new(3, 4),
            TileCoord::new(3, 3),
        ];
        crate::map::make_water_tile(&mut map, line[1], crate::map::WaterClass::River)
            .expect("river span");
        let neighbour = TileCoord::new(4, 4);
        crate::map::make_water_tile(&mut map, neighbour, crate::map::WaterClass::River)
            .expect("river neighbour");
        let mut neighbour_tile = map.get(neighbour).expect("neighbour");
        neighbour_tile.m3 = 1;
        map.set_tile(neighbour, neighbour_tile)
            .expect("set flooding state");

        assert!(materialize_generated_town_road_bridge(
            &mut map,
            &line,
            3,
            BridgeType::CantileverSteel,
        ));

        let bridge_type = BridgeType::CantileverSteel.as_u8() << 2;
        assert_eq!(map.get(line[0]).expect("start ramp").m6, bridge_type);
        assert_eq!(map.get(line[2]).expect("end ramp").m6, bridge_type);
        assert_eq!(map.get(line[1]).expect("river under span").m6, 0);
        assert_eq!(map.get(neighbour).expect("neighbour").m3, 0);
    }

    #[test]
    fn flat_town_bridge_does_not_select_a_type_for_sea_water() {
        let mut state = GameState::new(10, 10);
        let source = TileCoord::new(4, 6);
        let start = TileCoord::new(4, 5);
        let sea = TileCoord::new(4, 4);
        assert!(write_generated_town_road(
            &mut state,
            source,
            town_diag_dir_to_road_bits(3),
            0,
        ));
        crate::map::make_water_tile(&mut state.map, sea, crate::map::WaterClass::Sea)
            .expect("sea span");
        let context = GeneratedTownGrowthContext {
            climate: Climate::Temperate,
            snow_line_height: state.snow_line_height,
            calendar_year: 1950,
            bridge_spec_catalog: state.bridge_spec_catalog.clone(),
        };
        let mut rng = Randomizer {
            state: [653_263_232, 3_923_936_600],
        };
        let before = rng;

        assert!(!try_grow_generated_town_road_bridge(
            &mut state.map,
            start,
            Some(3),
            0,
            &context,
            &mut rng,
        ));
        assert_eq!(rng, before);
        assert_eq!(state.map.get(start).expect("start").kind, TileKind::Grass);
        assert_eq!(state.map.get(sea).expect("sea").kind, TileKind::Water);
    }

    #[test]
    fn town_bridge_and_mountain_tunnel_caps_are_independent() {
        // El límite 5 de puentes inclinados no debe confundirse con el mínimo
        // 7 de un túnel bajo montaña: ambos consumen el mismo presupuesto de
        // población en OpenTTD, pero viven en comandos distintos.
        assert_eq!(generated_town_bridge_length_cap(0, 0), 5);
        assert_eq!(generated_town_bridge_length_cap(SLOPE_SE, 0), 5);
        assert_eq!(generated_town_bridge_length_cap(SLOPE_SE, 1_000), 6);
        assert_eq!(generated_town_bridge_length_cap(SLOPE_SE, 10_000), 11);
        assert_eq!(generated_town_mountain_tunnel_length_cap(0), 7);
        assert_eq!(generated_town_mountain_tunnel_length_cap(1_000), 8);
    }

    #[test]
    fn sloped_town_bridge_does_not_count_a_coast_as_plain_water() {
        let mut map = Map::new_flat(8, 8, 1);
        let coast = TileCoord::new(3, 3);
        crate::map::make_water_tile(&mut map, coast, crate::map::WaterClass::Sea)
            .expect("sea tile");
        let mut coast_tile = map.get(coast).expect("coast candidate");
        coast_tile.m5 = 0x10; // WaterTileType::Coast, not IsWaterTile.
        map.set_tile(coast, coast_tile).expect("coast marker");

        assert!(!generated_town_bridge_crosses_water(
            map.get(coast).expect("coast"),
            false,
        ));
        assert!(!generated_town_bridge_crosses_water(
            map.get(coast).expect("coast"),
            true,
        ));

        crate::map::make_water_tile(
            &mut map,
            TileCoord::new(4, 3),
            crate::map::WaterClass::River,
        )
        .expect("river tile");
        assert!(generated_town_bridge_crosses_water(
            map.get(TileCoord::new(4, 3)).expect("river"),
            false,
        ));
    }

    #[test]
    fn sloped_town_bridge_rejects_a_parallel_road_bridge() {
        let mut map = Map::new_flat(16, 16, 1);
        let start = TileCoord::new(8, 8);
        let parallel = TileCoord::new(8, 11);
        map.set_kind(parallel, TileKind::RoadBridge)
            .expect("parallel bridge mouth");
        // SLOPE_NW is the opposite-facing ramp for a bridge growing south
        // (direction 1), exactly the bit checked by SpiralTileSequence.
        map.set_height(parallel, 2).expect("parallel north corner");
        map.set_height(TileCoord::new(parallel.x + 1, parallel.y), 2)
            .expect("parallel west corner");
        assert_eq!(tile_slope_and_z(&map, parallel), Some((SLOPE_NW, 1)));
        assert!(generated_town_has_parallel_road_bridge(&map, start, 4, 1));

        map.set_height(parallel, 1).expect("flatten parallel north");
        map.set_height(TileCoord::new(parallel.x + 1, parallel.y), 1)
            .expect("flatten parallel west");
        assert!(!generated_town_has_parallel_road_bridge(&map, start, 4, 1));
    }

    #[test]
    fn sloped_town_bridge_parallel_scan_excludes_square_corner() {
        let mut map = Map::new_flat(16, 16, 1);
        let start = TileCoord::new(8, 8);
        // Esta boca cae dentro del cuadrado que usaba el preflight anterior,
        // pero no forma parte de `SpiralTileSequence(start, 2, 0, 0)`.
        let outside_spiral = TileCoord::new(6, 8);
        map.set_kind(outside_spiral, TileKind::RoadBridge)
            .expect("corner bridge mouth");
        map.set_height(outside_spiral, 2)
            .expect("corner north height");
        map.set_height(TileCoord::new(outside_spiral.x + 1, outside_spiral.y), 2)
            .expect("corner west height");
        assert_eq!(tile_slope_and_z(&map, outside_spiral), Some((SLOPE_NW, 1)));
        assert!(!generated_town_has_parallel_road_bridge(&map, start, 2, 1));
    }

    #[test]
    fn cleanup_road_bits_removes_house_facing_branch() {
        let mut map = Map::new_flat(8, 8, 1);
        let road = TileCoord::new(4, 4);
        // `DiagDirection::NE` (0) apunta a -X y usa el bit 0x08. Una casa
        // allí no es conectiva; el bit opuesto 0x02 sí puede quedar hacia
        // clear. Es la misma forma que aparece en la llamada 84 de RMAP-050.
        map.set_kind(TileCoord::new(3, 4), TileKind::House)
            .expect("house neighbour");
        assert_eq!(clean_up_generated_town_road_bits(&map, road, 0x0A), 0x02);
    }

    #[test]
    fn cleanup_road_bits_checks_bridge_mouth_direction() {
        let mut map = Map::new_flat(8, 8, 1);
        let road = TileCoord::new(4, 4);
        let mouth = add_town_diag(road, 3);
        let mut bridge = map.get(mouth).expect("bridge mouth");
        bridge.kind = TileKind::RoadBridge;
        bridge.mapt = 0x90;
        // The bridge points toward direction 2, so its exterior bit points
        // opposite direction 0 and cannot connect the direction-3 plan.
        bridge.m5 = 0x04 | 2;
        map.set_tile(mouth, bridge).expect("write bridge mouth");
        assert_eq!(clean_up_generated_town_road_bits(&map, road, ROAD_NW), 0);

        bridge.m5 = 0x04 | 3;
        map.set_tile(mouth, bridge)
            .expect("write matching bridge mouth");
        assert_eq!(
            clean_up_generated_town_road_bits(&map, road, ROAD_NW),
            ROAD_NW
        );
    }

    #[test]
    fn level_town_land_falls_back_to_lowering_when_raise_touches_roads() {
        let mut map = Map::new_flat(8, 8, 1);
        let tile = TileCoord::new(3, 3);
        // Pendiente NE: N/E altas. La alternativa ascendente intentaría
        // cambiar W/S, que son calles; `TerraformTownTile(..., true)` falla
        // y OpenTTD prueba la alternativa descendente sobre N/E.
        map.set_height(tile, 2).expect("north high");
        map.set_height(TileCoord::new(3, 4), 2).expect("east high");
        map.set_kind(TileCoord::new(4, 3), TileKind::Road)
            .expect("west road");
        map.set_kind(TileCoord::new(4, 4), TileKind::Road)
            .expect("south road");

        assert_eq!(tile_slope_and_z(&map, tile), Some((SLOPE_NE, 1)));
        assert!(level_generated_town_land(&mut map, tile));
        assert_eq!(tile_slope_and_z(&map, tile), Some((0, 1)));
        assert_eq!(map.get(tile).expect("north lowered").height, 1);
        assert_eq!(
            map.get(TileCoord::new(3, 4)).expect("east lowered").height,
            1
        );
        assert_eq!(map.get_kind(TileCoord::new(4, 3)), Some(TileKind::Road));
        assert_eq!(map.get_kind(TileCoord::new(4, 4)), Some(TileKind::Road));
    }

    #[test]
    fn level_town_land_propagates_lowering_to_keep_neighbour_slope_valid() {
        // Reproducción reducida de la primera divergencia 256²:
        //
        //   7 7      La calle al sur impide elevar las dos esquinas bajas.
        //   6 6      Al bajar las altas de la tesela actual, el Terraformer
        //   5 5      nativo también baja 7→6 para que la vecina norte siga
        //              en SLOPE_NW, en vez de convertirse en steep NW (25).
        let mut map = Map::new_flat(8, 8, 5);
        let tile = TileCoord::new(3, 3);
        let north = TileCoord::new(3, 2);
        for vertex in [TileCoord::new(3, 2), TileCoord::new(4, 2)] {
            map.set_height(vertex, 7).expect("north high corner");
        }
        for vertex in [TileCoord::new(3, 3), TileCoord::new(4, 3)] {
            map.set_height(vertex, 6).expect("current high corner");
        }
        map.set_kind(TileCoord::new(3, 4), TileKind::Road)
            .expect("road blocks raising the low corners");

        assert_eq!(tile_slope_and_z(&map, tile), Some((SLOPE_NW, 5)));
        assert_eq!(tile_slope_and_z(&map, north), Some((SLOPE_NW, 6)));

        assert!(level_generated_town_land(&mut map, tile));

        assert_eq!(tile_slope_and_z(&map, tile), Some((0, 5)));
        assert_eq!(tile_slope_and_z(&map, north), Some((SLOPE_NW, 5)));
        assert_eq!(map.get(TileCoord::new(3, 2)).expect("north N").height, 6);
        assert_eq!(map.get(TileCoord::new(4, 2)).expect("north W").height, 6);
    }

    #[test]
    fn town_terraform_execute_pass_clears_the_approved_dirty_tile() {
        let mut map = Map::new_flat(8, 8, 1);
        let tile = TileCoord::new(3, 3);
        // La fundación asciende las dos esquinas bajas de una pendiente NE.
        // La tesela es un campo con bytes no nulos para comprobar el contrato
        // completo de `DoClearSquare`, no sólo la altura nivelada.
        let mut field = map.get(tile).expect("field tile");
        field.mapt = 0x0B;
        field.m1 = 0xFE;
        field.m2 = 0xAA;
        field.m2_hi = 0xBB;
        field.m3 = 0x0E;
        field.m3hi = 0xCC;
        field.m5 = clear_ground_m5(CLEAR_GROUND_FIELDS, 3);
        field.m6 = 0xDD;
        field.m7 = 0xEE;
        field.m8 = 0xFFFF;
        map.set_tile(tile, field).expect("install field bytes");
        map.set_height(tile, 2).expect("north high");
        map.set_height(TileCoord::new(3, 4), 2).expect("east high");

        assert_eq!(tile_slope_and_z(&map, tile), Some((SLOPE_NE, 1)));
        assert!(level_generated_town_land(&mut map, tile));

        let cleared = map.get(tile).expect("cleared field");
        assert_eq!(tile_slope_and_z(&map, tile), Some((0, 2)));
        assert_eq!(cleared.kind, TileKind::Grass);
        assert_eq!(cleared.mapt, 0x0B);
        assert_eq!(cleared.m1, OWNER_NONE_M1);
        assert_eq!(cleared.m2, 0);
        assert_eq!(cleared.m2_hi, 0);
        assert_eq!(cleared.m3, 0);
        assert_eq!(cleared.m3hi, 0);
        assert_eq!(cleared.m5, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
        assert_eq!(cleared.m6, 0);
        assert_eq!(cleared.m7, 0);
        assert_eq!(cleared.m8, 0);
    }

    #[test]
    fn town_terraform_accepts_a_flat_coast_and_clears_non_flooding_neighbours() {
        let mut map = Map::new_flat(8, 8, 1);
        let coast = TileCoord::new(3, 3);
        let water_neighbour = TileCoord::new(3, 2);
        let mut coast_tile = map.get(coast).expect("coast tile");
        coast_tile.kind = TileKind::Water;
        coast_tile.mapt = 0x6D;
        coast_tile.m1 = 0x91;
        coast_tile.m2 = 0xAA;
        coast_tile.m2_hi = 0xBB;
        coast_tile.m3 = 0xCE;
        coast_tile.m3hi = 0xCC;
        coast_tile.m5 = 0x10; // WaterTileType::Coast.
        coast_tile.m6 = 0xDD;
        coast_tile.m7 = 0xEE;
        coast_tile.m8 = 0xFFFF;
        map.set_tile(coast, coast_tile).expect("install coast");
        let mut water = map.get(water_neighbour).expect("water neighbour");
        water.kind = TileKind::Water;
        water.mapt = 0x60;
        water.m3 = 1;
        map.set_tile(water_neighbour, water)
            .expect("install non-flooding water");

        assert_eq!(generated_town_terraform_clear_cost(&map, coast), Some(40));
        assert!(clear_generated_town_terraform_tile(&mut map, coast));

        let cleared = map.get(coast).expect("cleared coast");
        assert_eq!(cleared.kind, TileKind::Grass);
        assert_eq!(cleared.mapt, 0x0D);
        assert_eq!(cleared.m1, OWNER_NONE_M1);
        assert_eq!(cleared.m2, 0);
        assert_eq!(cleared.m2_hi, 0);
        assert_eq!(cleared.m3, 0);
        assert_eq!(cleared.m3hi, 0);
        assert_eq!(cleared.m5, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
        assert_eq!(cleared.m6, 0);
        assert_eq!(cleared.m7, 0);
        assert_eq!(cleared.m8, 0);
        assert_eq!(
            map.get(water_neighbour).expect("reactivated water").m3 & 1,
            0
        );
    }

    #[test]
    fn town_terraform_rejects_cost_at_the_native_limit() {
        let mut map = Map::new_flat(8, 8, 1);
        let dirty = TileCoord::new(3, 3);
        map.set_mapt_m5(dirty, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .expect("full grass");
        let mut state = GeneratedTownTerraformState {
            heights: vec![(dirty, 2)],
            dirty_tiles: vec![dirty],
            terraform_cost: GENERATED_TOWN_TERRAFORM_COST_LIMIT - GENERATED_TOWN_CLEAR_GRASS_PRICE,
        };

        assert_eq!(
            generated_town_terraform_cost(&map, &state),
            Some(GENERATED_TOWN_TERRAFORM_COST_LIMIT)
        );
        assert!(
            generated_town_terraform_cost(&map, &state)
                .is_some_and(|cost| cost >= GENERATED_TOWN_TERRAFORM_COST_LIMIT)
        );

        state.terraform_cost -= 1;
        assert_eq!(
            generated_town_terraform_cost(&map, &state),
            Some(GENERATED_TOWN_TERRAFORM_COST_LIMIT - 1)
        );
    }

    #[test]
    fn second_seed_cleanup_matches_call_ninety_rng_boundary() {
        let (mut state, mut town, mut rng) = second_seed_first_city_after_bootstrap();

        let mut last = None;
        for call in 2..=90 {
            let result = grow_first_fixture_town(&mut state, &mut town, &mut rng);
            if call == 84 {
                assert_eq!(result, Some(TileCoord::new(44, 13)));
                assert_eq!(
                    state.map.get(TileCoord::new(44, 13)).expect("road n=84").m5,
                    0x02,
                );
            }
            last = result;
        }
        assert_eq!(last, None);
        assert_eq!(town.num_houses, 69);
        assert_eq!(town.population, 1_237);
        assert_eq!(rng.state, [3_904_639_598, 2_282_438_850]);
    }

    #[test]
    fn second_seed_first_city_replays_post_growth_state() {
        let (mut state, mut town, mut rng) = second_seed_first_city_after_bootstrap();
        for _ in 2..=144 {
            let _ = grow_first_fixture_town(&mut state, &mut town, &mut rng);
        }

        // Tras la llamada 144, C++ entra al segundo `GenerateTownName` con
        // las 78 casas temporales y la población acumulada de la primera
        // ciudad grande. Esta frontera cubre la alternativa descendente de
        // `LevelTownLand` que preserva la casa 38 de la traza.
        assert_eq!(town.num_houses, 78);
        assert_eq!(town.population, 1_862);
        assert_eq!(rng.state, [3_848_068_084, 748_657_221]);
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
                industry_platform: 1,
                multiple_industry_per_town: false,
            };

            assert!(
                place_towns(&mut ctx, target, &mut centers) > 0,
                "seed {seed}"
            );
            assert_eq!(ctx.state.towns[0].pos, expected, "seed {seed}");
        }
    }

    #[test]
    fn integrated_generation_replays_all_seed_1330935378_towns() {
        let mut state = clear_phase_state(1_330_935_378);
        let mut rng = Randomizer {
            state: [1_168_016_413, 2_955_223_551],
        };
        let target = town_generation_target_count(TownDensity::Normal, &state.map, &mut rng);
        let mut centers = Vec::new();
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
            industry_platform: 1,
            multiple_industry_per_town: false,
        };

        assert_eq!(target, 3);
        assert_eq!(place_towns(&mut ctx, target, &mut centers), 3);
        assert_eq!(
            centers,
            vec![
                TileCoord::new(47, 23),
                TileCoord::new(16, 51),
                TileCoord::new(28, 36),
            ]
        );
        assert_eq!(
            ctx.state
                .towns
                .iter()
                .map(|town| (town.pos, town.num_houses, town.population))
                .collect::<Vec<_>>(),
            vec![
                (TileCoord::new(47, 23), 29, 1_181),
                (TileCoord::new(16, 51), 20, 815),
                (TileCoord::new(28, 36), 16, 327),
            ]
        );
        assert_eq!(ctx.rng.state, [11_204_508, 1_784_072_412]);
    }

    #[test]
    fn generated_towns_preserve_industry_rng_boundary_for_control_seeds() {
        // GDB en `GenerateIndustries` fija ambas fronteras C++: ninguna
        // selección o construcción de industria puede compensar un stream
        // que ya llegue desfasado desde `GenerateTowns`.
        for (seed, begin, expected_towns, expected_rng) in [
            (
                1_330_935_378,
                [1_168_016_413, 2_955_223_551],
                3,
                [11_204_508, 1_784_072_412],
            ),
            (
                1_330_935_379,
                [1_179_957_886, 1_700_995_136],
                5,
                [2_992_974_009, 1_778_840_233],
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
                industry_platform: 1,
                multiple_industry_per_town: false,
            };

            assert_eq!(place_towns(&mut ctx, target, &mut centers), expected_towns);
            assert_eq!(ctx.rng.state, expected_rng, "seed {seed}");
        }
    }

    #[test]
    fn generated_town_roads_treat_a_coast_as_clearable_ground() {
        let mut map = crate::map::Map::new_flat(7, 7, 0);
        let coast = TileCoord::new(3, 3);
        let water = TileCoord::new(4, 3);
        assert!(crate::map::make_shore_tile(&mut map, coast).is_ok());
        assert!(crate::map::make_water_tile(&mut map, water, crate::map::WaterClass::Sea).is_ok());
        let town = Town {
            pos: TileCoord::new(2, 3),
            layout: TownLayout::Original,
            ..Default::default()
        };
        let mut coast_rng = Randomizer::default();
        let mut water_rng = Randomizer::default();

        assert!(generated_town_road_allowed_here(
            &map,
            &town,
            coast,
            2,
            &mut coast_rng,
        ));
        assert!(!generated_town_road_allowed_here(
            &map,
            &town,
            water,
            2,
            &mut water_rng,
        ));
        assert!(generated_can_follow_town_road(&map, town.pos, 2));
    }

    #[test]
    fn generated_town_growth_consumes_slope_chance_on_occupied_tile() {
        // `GrowTownInTile` sortea `LevelTownLand` antes de rechazar una casa
        // ocupada en `IsRoadAllowedHere`. Aunque el comando de terraformación
        // no modifique la casa, esa palabra mantiene alineado el RNG de las
        // llamadas siguientes (RMAP-135).
        let mut map = Map::new_flat(8, 8, 1);
        let tile = TileCoord::new(3, 3);
        map.set_kind(tile, TileKind::House).expect("occupied tile");
        let mut town = Town {
            layout: TownLayout::Original,
            ..Town::default()
        };
        let context = GeneratedTownGrowthContext {
            climate: Climate::Temperate,
            snow_line_height: 0,
            calendar_year: 1950,
            bridge_spec_catalog: Vec::new(),
        };
        let mut rng = Randomizer {
            state: [0x1234_5678, 0x9ABC_DEF0],
        };
        let mut expected = rng;
        let _ = chance16(&mut expected, 1, 6);

        let result = grow_generated_town_road_in_tile(
            &mut map,
            &mut town,
            tile,
            0,
            Some(0),
            &context,
            &mut rng,
        );

        assert!(matches!(result, GeneratedRoadGrowthResult::SearchStopped));
        assert_eq!(rng, expected);
        assert_eq!(map.get_kind(tile), Some(TileKind::House));
    }

    #[test]
    fn generated_town_roads_accept_existing_tunnel_mouth() {
        // RMAP-119: `IsRoadAllowedHere` consulta `GetTownRoadBits` antes de
        // intentar limpiar la tesela. Una boca municipal es `MP_TUNNELBRIDGE`
        // (no `MP_ROAD`), pero expone ambos bits del eje y debe dejar que el
        // walker continúe sin consumir RNG adicional.
        let mut map = Map::new_flat(8, 8, 1);
        let mouth = TileCoord::new(3, 3);
        let mut tunnel = map.get(mouth).expect("tunnel mouth");
        tunnel.kind = TileKind::RoadTunnel;
        tunnel.mapt = 0x90;
        tunnel.m1 = crate::company::OWNER_TOWN_M1;
        tunnel.m5 = 0x04 | 2;
        tunnel.m7 = crate::company::OWNER_TOWN_M1;
        tunnel.m8 = TOWN_ROAD_INVALID_TRAM_TYPE;
        map.set_tile(mouth, tunnel).expect("install tunnel mouth");

        let town = Town {
            layout: TownLayout::Original,
            ..Town::default()
        };
        let mut rng = Randomizer::default();
        assert!(generated_town_road_allowed_here(
            &map, &town, mouth, 2, &mut rng
        ));
        assert_eq!(rng, Randomizer::default());
    }

    #[test]
    fn town_walker_stops_after_entering_another_towns_road() {
        let mut map = Map::new_flat(8, 8, 1);
        let tile = TileCoord::new(3, 3);
        let mut road = map.get(tile).expect("foreign road");
        road.kind = TileKind::Road;
        road.mapt = 0x20;
        road.m1 = crate::company::OWNER_TOWN_M1;
        road.m2 = 7;
        road.m5 = ROAD_BITS_AXIS_X;
        map.set_tile(tile, road).expect("install foreign road");

        assert!(generated_town_road_is_foreign(&map, tile, 3));
        assert!(!generated_town_road_is_foreign(&map, tile, 7));
    }
}
